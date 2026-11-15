//! Exécution sûre d'un plan de renommage.
//!
//! Garanties :
//! * jamais d'écrasement : une destination existante fait échouer
//!   l'opération (sauf renommage ne changeant que la casse du même fichier) ;
//! * déplacement entre volumes par copie puis suppression de la source ;
//! * chaque élément du plan est une transaction : en cas d'échec, les
//!   opérations déjà faites sont annulées dans l'ordre inverse (au mieux) ;
//! * les suppressions sont différées : le fichier est mis de côté dans un
//!   dossier de travail et n'est effacé (ou placé dans la corbeille) qu'une
//!   fois l'élément réussi.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering as OrdreAtomique};

use regex::RegexBuilder;

use super::plan::{ElementPlan, Operation, PlanRenommage, meme_chemin, relatif};
use crate::modele::HistoriqueRenommage;

/// Nom du dossier de corbeille créé à la racine de la source.
pub const DOSSIER_CORBEILLE: &str = ".corbeille-renommage";

/// Options d'exécution.
#[derive(Debug, Clone, Default)]
pub struct OptionsExecution {
    /// Les fichiers supprimés sont conservés dans
    /// `<source>/.corbeille-renommage/<chemin relatif>` au lieu d'être effacés.
    pub corbeille: bool,
    /// N'exécute rien : renvoie seulement le rapport attendu.
    pub simulation: bool,
    /// Force la voie « copie puis suppression » pour tous les déplacements
    /// (sert aux tests du déplacement entre volumes).
    #[doc(hidden)]
    pub forcer_copie: bool,
}

/// Action réellement effectuée (journal d'un élément).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntreeJournal {
    DossierCree(PathBuf),
    DossierDeplace {
        de: PathBuf,
        vers: PathBuf,
    },
    FichierDeplace {
        de: PathBuf,
        vers: PathBuf,
    },
    FichierCopie(PathBuf),
    /// Fichier supprimé, mis de côté à `depot` jusqu'à la validation.
    FichierMisDeCote {
        original: PathBuf,
        depot: PathBuf,
    },
    DossierSupprime(PathBuf),
}

/// Erreur d'exécution d'un élément.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErreurExecution {
    /// Opération fautive.
    pub operation: String,
    pub message: String,
}

impl std::fmt::Display for ErreurExecution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} : {}", self.operation, self.message)
    }
}

impl std::error::Error for ErreurExecution {}

/// Résultat d'un élément.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatutElement {
    /// Élément appliqué (l'historique permet l'annulation ultérieure).
    Reussi { historique: HistoriqueRenommage },
    /// Rien à faire.
    Inchange,
    /// Non exécuté (conflits du plan ou dépendance en échec).
    Ignore { raison: String },
    /// Échec ; `retour_complet` indique si l'état initial a été restauré.
    Echoue {
        erreur: ErreurExecution,
        retour_complet: bool,
        erreurs_retour: Vec<String>,
    },
}

/// Résultat de l'exécution d'un élément.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultatElement {
    pub indice: usize,
    pub statut: StatutElement,
    /// Actions effectuées (vide après un retour arrière complet).
    pub journal: Vec<EntreeJournal>,
}

/// Rapport d'exécution d'un plan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapportExecution {
    pub resultats: Vec<ResultatElement>,
}

impl RapportExecution {
    /// Tous les éléments ont réussi ou n'avaient rien à faire.
    pub fn est_complet(&self) -> bool {
        self.resultats.iter().all(|r| {
            matches!(
                r.statut,
                StatutElement::Reussi { .. } | StatutElement::Inchange
            )
        })
    }

    /// Nombre d'éléments en échec.
    pub fn nombre_echecs(&self) -> usize {
        self.resultats
            .iter()
            .filter(|r| matches!(r.statut, StatutElement::Echoue { .. }))
            .count()
    }
}

/// Exécute un plan élément par élément. Les éléments en conflit, ou dont
/// l'élément requis a échoué, sont ignorés.
pub fn executer(plan: &PlanRenommage, options: &OptionsExecution) -> RapportExecution {
    let mut rapport = RapportExecution::default();
    for (i, el) in plan.elements.iter().enumerate() {
        let dependance_ko = el.depend_de.is_some_and(|d| {
            rapport.resultats.get(d).is_none_or(|r| {
                !matches!(
                    r.statut,
                    StatutElement::Reussi { .. } | StatutElement::Inchange
                )
            })
        });
        let statut_journal = if !el.est_executable() {
            let raison = el
                .conflits
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ; ");
            (StatutElement::Ignore { raison }, Vec::new())
        } else if dependance_ko {
            (
                StatutElement::Ignore {
                    raison: "l'élément requis n'a pas été renommé".into(),
                },
                Vec::new(),
            )
        } else if el.operations.is_empty() || (options.simulation && el.est_inchange()) {
            (StatutElement::Inchange, Vec::new())
        } else if options.simulation {
            (
                StatutElement::Reussi {
                    historique: el.historique(),
                },
                Vec::new(),
            )
        } else {
            match executer_element(el, options) {
                Ok(journal) => {
                    let statut = if journal.is_empty() && el.est_inchange() {
                        StatutElement::Inchange
                    } else {
                        StatutElement::Reussi {
                            historique: el.historique(),
                        }
                    };
                    (statut, journal)
                }
                Err(e) => (e.statut, e.journal),
            }
        };
        rapport.resultats.push(ResultatElement {
            indice: i,
            statut: statut_journal.0,
            journal: statut_journal.1,
        });
    }
    rapport
}

/// Échec d'un élément avec l'état du journal après retour arrière.
#[derive(Debug, Clone)]
pub struct EchecElement {
    pub statut: StatutElement,
    pub journal: Vec<EntreeJournal>,
}

/// Exécute un seul élément de façon transactionnelle.
pub fn executer_element(
    el: &ElementPlan,
    options: &OptionsExecution,
) -> Result<Vec<EntreeJournal>, EchecElement> {
    let mut tx = Transaction::nouvelle(el, options);
    for op in &el.operations {
        if let Err(e) = tx.appliquer(op) {
            let erreur = ErreurExecution {
                operation: op.to_string(),
                message: e.to_string(),
            };
            let erreurs_retour = tx.annuler();
            let retour_complet = erreurs_retour.is_empty();
            let journal = if retour_complet {
                Vec::new()
            } else {
                tx.journal.clone()
            };
            tx.nettoyer_depot();
            return Err(EchecElement {
                statut: StatutElement::Echoue {
                    erreur,
                    retour_complet,
                    erreurs_retour,
                },
                journal,
            });
        }
    }
    tx.valider();
    Ok(tx.journal)
}

static COMPTEUR: AtomicUsize = AtomicUsize::new(0);

struct Transaction<'a> {
    element: &'a ElementPlan,
    options: &'a OptionsExecution,
    journal: Vec<EntreeJournal>,
    depot: Option<PathBuf>,
}

impl<'a> Transaction<'a> {
    fn nouvelle(element: &'a ElementPlan, options: &'a OptionsExecution) -> Self {
        Transaction {
            element,
            options,
            journal: Vec::new(),
            depot: None,
        }
    }

    fn racine(&self) -> PathBuf {
        if !self.element.source_donnees.as_os_str().is_empty() {
            self.element.source_donnees.clone()
        } else {
            self.element
                .ancien_dossier
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| self.element.ancien_dossier.clone())
        }
    }

    /// Dossier de travail des fichiers mis de côté (même volume que la source).
    fn depot(&mut self) -> io::Result<PathBuf> {
        if let Some(d) = &self.depot {
            return Ok(d.clone());
        }
        let n = COMPTEUR.fetch_add(1, OrdreAtomique::SeqCst);
        let d = self
            .racine()
            .join(format!(".renommage-en-cours-{}-{n}", std::process::id()));
        fs::create_dir_all(&d)?;
        self.depot = Some(d.clone());
        Ok(d)
    }

    fn appliquer(&mut self, op: &Operation) -> io::Result<()> {
        match op {
            Operation::CreerDossier { chemin } => self.creer_dossiers(chemin),
            Operation::DeplacerDossier { de, vers } => {
                if let Some(p) = vers.parent() {
                    self.creer_dossiers(p)?;
                }
                deplacer_dossier(de, vers, self.options.forcer_copie)?;
                self.journal.push(EntreeJournal::DossierDeplace {
                    de: de.clone(),
                    vers: vers.clone(),
                });
                Ok(())
            }
            Operation::Deplacer { de, vers } => {
                if de == vers {
                    return Ok(());
                }
                if let Some(p) = vers.parent() {
                    self.creer_dossiers(p)?;
                }
                deplacer_fichier(de, vers, self.options.forcer_copie)?;
                self.journal.push(EntreeJournal::FichierDeplace {
                    de: de.clone(),
                    vers: vers.clone(),
                });
                Ok(())
            }
            Operation::Copier { de, vers } => {
                if de == vers {
                    return Ok(());
                }
                if let Some(p) = vers.parent() {
                    self.creer_dossiers(p)?;
                }
                copier_fichier(de, vers)?;
                self.journal.push(EntreeJournal::FichierCopie(vers.clone()));
                Ok(())
            }
            Operation::Supprimer { chemin } => self.mettre_de_cote(chemin),
            Operation::SupprimerDossierVide { chemin } => {
                if est_dossier_vide(chemin) {
                    fs::remove_dir(chemin)?;
                    self.journal
                        .push(EntreeJournal::DossierSupprime(chemin.clone()));
                }
                Ok(())
            }
            Operation::NettoyerSousDossiersVides { racine } => {
                if racine.is_dir() {
                    let mut supprimes = Vec::new();
                    supprimer_vides(racine, racine, &mut supprimes)?;
                    self.journal
                        .extend(supprimes.into_iter().map(EntreeJournal::DossierSupprime));
                }
                Ok(())
            }
            Operation::NettoyerIndesirables { racine, motifs } => {
                let regex: Vec<regex::Regex> = motifs
                    .iter()
                    .filter_map(|m| RegexBuilder::new(m).case_insensitive(true).build().ok())
                    .collect();
                if regex.is_empty() || !racine.is_dir() {
                    return Ok(());
                }
                let depot = self.depot.clone();
                let mut cibles = Vec::new();
                for e in walkdir::WalkDir::new(racine)
                    .into_iter()
                    .filter_entry(|e| depot.as_ref().is_none_or(|d| e.path() != d))
                    .filter_map(Result::ok)
                {
                    if e.file_type().is_file() {
                        let nom = e.file_name().to_string_lossy();
                        if regex.iter().any(|r| r.is_match(&nom)) {
                            cibles.push(e.path().to_path_buf());
                        }
                    }
                }
                for c in cibles {
                    self.mettre_de_cote(&c)?;
                }
                Ok(())
            }
        }
    }

    fn creer_dossiers(&mut self, chemin: &Path) -> io::Result<()> {
        let mut manquants = Vec::new();
        let mut c = Some(chemin);
        while let Some(d) = c {
            if d.as_os_str().is_empty() || d.exists() {
                break;
            }
            manquants.push(d.to_path_buf());
            c = d.parent();
        }
        for d in manquants.into_iter().rev() {
            fs::create_dir(&d)?;
            self.journal.push(EntreeJournal::DossierCree(d));
        }
        Ok(())
    }

    fn mettre_de_cote(&mut self, chemin: &Path) -> io::Result<()> {
        if chemin.symlink_metadata().is_err() {
            return Ok(());
        }
        let depot = self.depot()?;
        let n = self.journal.len();
        let nom = chemin
            .file_name()
            .map(|x| x.to_string_lossy().into_owned())
            .unwrap_or_default();
        let cible = depot.join(format!("{n}-{nom}"));
        deplacer_fichier(chemin, &cible, false)?;
        self.journal.push(EntreeJournal::FichierMisDeCote {
            original: chemin.to_path_buf(),
            depot: cible,
        });
        Ok(())
    }

    /// Retour arrière dans l'ordre inverse ; renvoie les erreurs rencontrées.
    fn annuler(&mut self) -> Vec<String> {
        let mut erreurs = Vec::new();
        let mut restants = Vec::new();
        while let Some(e) = self.journal.pop() {
            let r = match &e {
                EntreeJournal::DossierCree(d) => {
                    if d.exists() {
                        fs::remove_dir(d)
                    } else {
                        Ok(())
                    }
                }
                EntreeJournal::DossierDeplace { de, vers } => deplacer_dossier(vers, de, false),
                EntreeJournal::FichierDeplace { de, vers } => deplacer_fichier(vers, de, false),
                EntreeJournal::FichierCopie(p) => fs::remove_file(p),
                EntreeJournal::FichierMisDeCote { original, depot } => {
                    deplacer_fichier(depot, original, false)
                }
                EntreeJournal::DossierSupprime(d) => fs::create_dir_all(d),
            };
            if let Err(err) = r {
                erreurs.push(format!("{e:?} : {err}"));
                restants.push(e);
            }
        }
        restants.reverse();
        self.journal = restants;
        erreurs
    }

    /// Validation : les fichiers mis de côté sont effacés ou placés dans la
    /// corbeille ; le dossier de travail est retiré.
    fn valider(&mut self) {
        if self.options.corbeille {
            let corbeille = self.racine().join(DOSSIER_CORBEILLE);
            let racine = self.racine();
            for e in &mut self.journal {
                if let EntreeJournal::FichierMisDeCote { original, depot } = e {
                    let rel = relatif(original, &racine);
                    let mut cible = super::plan::joindre(&corbeille, &rel);
                    if cible.exists() {
                        let n = COMPTEUR.fetch_add(1, OrdreAtomique::SeqCst);
                        let nom = cible
                            .file_name()
                            .map(|x| x.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        cible.set_file_name(format!("{n}-{nom}"));
                    }
                    let ok = cible.parent().is_none_or(|p| fs::create_dir_all(p).is_ok())
                        && deplacer_fichier(depot, &cible, false).is_ok();
                    if ok {
                        *depot = cible;
                    }
                }
            }
        }
        self.nettoyer_depot();
    }

    fn nettoyer_depot(&mut self) {
        if let Some(d) = self.depot.take() {
            // Le dépôt ne contient plus que des fichiers à effacer.
            let _ = fs::remove_dir_all(&d);
        }
    }
}

// ---------------------------------------------------------------------------
// Primitives sûres
// ---------------------------------------------------------------------------

fn existe(p: &Path) -> bool {
    p.symlink_metadata().is_ok()
}

fn est_dossier_vide(p: &Path) -> bool {
    fs::read_dir(p).is_ok_and(|mut l| l.next().is_none())
}

/// Vrai si `a` et `b` désignent la même entrée (même inode, ou même chemin
/// canonique) : cas d'un renommage ne changeant que la casse.
fn meme_entree(a: &Path, b: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(x), Ok(y)) = (a.symlink_metadata(), b.symlink_metadata()) {
            return x.dev() == y.dev() && x.ino() == y.ino();
        }
        false
    }
    #[cfg(not(unix))]
    {
        match (fs::canonicalize(a), fs::canonicalize(b)) {
            (Ok(x), Ok(y)) => x == y,
            _ => false,
        }
    }
}

fn deja_present(vers: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("la destination existe déjà : {}", vers.display()),
    )
}

fn est_inter_volumes(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::CrossesDevices || e.raw_os_error() == Some(18)
}

/// Déplace un fichier sans jamais écraser ; copie + suppression entre volumes.
pub fn deplacer_fichier(de: &Path, vers: &Path, forcer_copie: bool) -> io::Result<()> {
    if de == vers {
        return Ok(());
    }
    if existe(vers) {
        if meme_chemin(de, vers) && meme_entree(de, vers) {
            return renommer_par_temporaire(de, vers);
        }
        return Err(deja_present(vers));
    }
    if forcer_copie {
        return copier_puis_supprimer(de, vers);
    }
    match fs::rename(de, vers) {
        Ok(()) => Ok(()),
        Err(e) if est_inter_volumes(&e) => copier_puis_supprimer(de, vers),
        Err(e) => Err(e),
    }
}

fn renommer_par_temporaire(de: &Path, vers: &Path) -> io::Result<()> {
    let n = COMPTEUR.fetch_add(1, OrdreAtomique::SeqCst);
    let tmp = de.with_file_name(format!(".renommage-casse-{}-{n}", std::process::id()));
    fs::rename(de, &tmp)?;
    if let Err(e) = fs::rename(&tmp, vers) {
        let _ = fs::rename(&tmp, de);
        return Err(e);
    }
    Ok(())
}

fn copier_puis_supprimer(de: &Path, vers: &Path) -> io::Result<()> {
    copier_fichier(de, vers)?;
    if let Err(e) = fs::remove_file(de) {
        let _ = fs::remove_file(vers);
        return Err(e);
    }
    Ok(())
}

/// Copie un fichier sans écraser (création exclusive) ; date de
/// modification et permissions conservées au mieux.
pub fn copier_fichier(de: &Path, vers: &Path) -> io::Result<()> {
    let mut source = fs::File::open(de)?;
    let meta = source.metadata()?;
    let mut cible = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(vers)
        .map_err(|e| {
            if e.kind() == io::ErrorKind::AlreadyExists {
                deja_present(vers)
            } else {
                e
            }
        })?;
    let r = io::copy(&mut source, &mut cible).and_then(|_| cible.sync_all());
    if let Err(e) = r {
        drop(cible);
        let _ = fs::remove_file(vers);
        return Err(e);
    }
    if let Ok(m) = meta.modified() {
        let _ = cible.set_modified(m);
    }
    let _ = fs::set_permissions(vers, meta.permissions());
    Ok(())
}

/// Déplace un dossier sans écraser ; copie récursive + suppression entre volumes.
pub fn deplacer_dossier(de: &Path, vers: &Path, forcer_copie: bool) -> io::Result<()> {
    if de == vers {
        return Ok(());
    }
    if existe(vers) {
        if meme_chemin(de, vers) && meme_entree(de, vers) {
            return renommer_par_temporaire(de, vers);
        }
        return Err(deja_present(vers));
    }
    if !de.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("dossier introuvable : {}", de.display()),
        ));
    }
    if forcer_copie {
        return copier_dossier_puis_supprimer(de, vers);
    }
    match fs::rename(de, vers) {
        Ok(()) => Ok(()),
        Err(e) if est_inter_volumes(&e) => copier_dossier_puis_supprimer(de, vers),
        Err(e) => Err(e),
    }
}

fn copier_dossier_puis_supprimer(de: &Path, vers: &Path) -> io::Result<()> {
    if let Err(e) = copier_dossier(de, vers) {
        let _ = fs::remove_dir_all(vers);
        return Err(e);
    }
    fs::remove_dir_all(de)
}

fn copier_dossier(de: &Path, vers: &Path) -> io::Result<()> {
    fs::create_dir(vers)?;
    for e in fs::read_dir(de)? {
        let e = e?;
        let cible = vers.join(e.file_name());
        let t = e.file_type()?;
        if t.is_dir() {
            copier_dossier(&e.path(), &cible)?;
        } else {
            copier_fichier(&e.path(), &cible)?;
        }
    }
    Ok(())
}

/// Supprime les sous-dossiers vides (post-ordre), jamais `racine`.
fn supprimer_vides(d: &Path, racine: &Path, supprimes: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut sous = Vec::new();
    for e in fs::read_dir(d)? {
        let e = e?;
        if e.file_type()?.is_dir() {
            sous.push(e.path());
        }
    }
    for s in sous {
        supprimer_vides(&s, racine, supprimes)?;
    }
    if d != racine && est_dossier_vide(d) {
        fs::remove_dir(d)?;
        supprimes.push(d.to_path_buf());
    }
    Ok(())
}
