//! Plan de renommage : opérations à appliquer, aperçu fichier par fichier et
//! conflits. La construction du plan est pure ; seule une [`SondeDisque`]
//! facultative interroge le disque (existence des destinations).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

use crate::modele::{FichierMedia, HistoriqueRenommage, TypeFichier, comparer_fichiers_film};

// ---------------------------------------------------------------------------
// Sonde
// ---------------------------------------------------------------------------

/// Lecture facultative du disque pendant la planification.
pub trait SondeDisque {
    /// Le chemin existe (fichier ou dossier).
    fn existe(&self, chemin: &Path) -> bool;
    /// Le chemin est un dossier contenant au moins une entrée.
    fn est_dossier_non_vide(&self, chemin: &Path) -> bool;
}

/// Sonde « pure » : rien n'existe sur le disque.
#[derive(Debug, Clone, Copy, Default)]
pub struct SondeVide;

impl SondeDisque for SondeVide {
    fn existe(&self, _: &Path) -> bool {
        false
    }
    fn est_dossier_non_vide(&self, _: &Path) -> bool {
        false
    }
}

/// Sonde du système de fichiers réel.
#[derive(Debug, Clone, Copy, Default)]
pub struct SondeSystemeFichiers;

impl SondeDisque for SondeSystemeFichiers {
    fn existe(&self, chemin: &Path) -> bool {
        chemin.symlink_metadata().is_ok()
    }
    fn est_dossier_non_vide(&self, chemin: &Path) -> bool {
        std::fs::read_dir(chemin).is_ok_and(|mut l| l.next().is_some())
    }
}

/// Sonde en mémoire : liste de chemins existants (un dossier est « non vide »
/// si un chemin listé se trouve dessous).
#[derive(Debug, Clone, Default)]
pub struct SondeListe {
    chemins: HashSet<PathBuf>,
}

impl SondeListe {
    pub fn nouvelle<I: IntoIterator<Item = P>, P: Into<PathBuf>>(chemins: I) -> Self {
        SondeListe {
            chemins: chemins.into_iter().map(Into::into).collect(),
        }
    }
}

impl SondeDisque for SondeListe {
    fn existe(&self, chemin: &Path) -> bool {
        self.chemins.contains(chemin) || self.chemins.iter().any(|c| c.starts_with(chemin))
    }
    fn est_dossier_non_vide(&self, chemin: &Path) -> bool {
        self.chemins
            .iter()
            .any(|c| c != chemin && c.starts_with(chemin))
    }
}

// ---------------------------------------------------------------------------
// Opérations, conflits, aperçu
// ---------------------------------------------------------------------------

/// Opération élémentaire, dans l'ordre d'exécution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    /// Crée un dossier (et ses parents manquants) ; sans effet s'il existe.
    CreerDossier { chemin: PathBuf },
    /// Déplace un dossier entier ; échoue si la destination existe.
    DeplacerDossier { de: PathBuf, vers: PathBuf },
    /// Déplace (renomme) un fichier ; jamais d'écrasement.
    Deplacer { de: PathBuf, vers: PathBuf },
    /// Copie un fichier ; jamais d'écrasement.
    Copier { de: PathBuf, vers: PathBuf },
    /// Supprime un fichier obsolète (mis de côté jusqu'à la fin, voire placé
    /// dans la corbeille).
    Supprimer { chemin: PathBuf },
    /// Supprime le dossier s'il existe et est vide (sinon sans effet).
    SupprimerDossierVide { chemin: PathBuf },
    /// Supprime récursivement les sous-dossiers vides (la racine est gardée).
    NettoyerSousDossiersVides { racine: PathBuf },
    /// Supprime les fichiers dont le nom correspond à l'une des regex.
    NettoyerIndesirables {
        racine: PathBuf,
        motifs: Vec<String>,
    },
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operation::CreerDossier { chemin } => write!(f, "créer {}", chemin.display()),
            Operation::DeplacerDossier { de, vers } => {
                write!(f, "déplacer dossier {} → {}", de.display(), vers.display())
            }
            Operation::Deplacer { de, vers } => {
                write!(f, "déplacer {} → {}", de.display(), vers.display())
            }
            Operation::Copier { de, vers } => {
                write!(f, "copier {} → {}", de.display(), vers.display())
            }
            Operation::Supprimer { chemin } => write!(f, "supprimer {}", chemin.display()),
            Operation::SupprimerDossierVide { chemin } => {
                write!(f, "supprimer dossier vide {}", chemin.display())
            }
            Operation::NettoyerSousDossiersVides { racine } => {
                write!(f, "nettoyer sous-dossiers vides de {}", racine.display())
            }
            Operation::NettoyerIndesirables { racine, .. } => {
                write!(f, "nettoyer indésirables de {}", racine.display())
            }
        }
    }
}

/// Problème empêchant (ou compromettant) le renommage d'un élément.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conflit {
    /// Gabarit invalide (erreur de syntaxe ou d'évaluation).
    GabaritInvalide { gabarit: String, message: String },
    /// Données nécessaires absentes (source, titre, numéros…).
    DonneesManquantes(String),
    /// Le dossier de destination existe déjà.
    DossierExistant(PathBuf),
    /// Un fichier existe déjà à la destination.
    FichierExistant(PathBuf),
    /// Plusieurs fichiers aboutiraient au même chemin.
    DestinationDupliquee {
        chemin: PathBuf,
        sources: Vec<PathBuf>,
    },
    /// Plusieurs éléments déplaceraient leur dossier au même endroit.
    DossierDuplique(PathBuf),
    /// L'élément dépend d'un autre élément bloqué.
    Dependance(usize),
}

impl Conflit {
    /// Tous les conflits sont bloquants : l'élément n'est pas exécuté.
    pub fn est_bloquant(&self) -> bool {
        true
    }
}

impl fmt::Display for Conflit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Conflit::GabaritInvalide { gabarit, message } => {
                write!(f, "gabarit invalide « {gabarit} » : {message}")
            }
            Conflit::DonneesManquantes(m) => write!(f, "données manquantes : {m}"),
            Conflit::DossierExistant(p) => write!(f, "le dossier existe déjà : {}", p.display()),
            Conflit::FichierExistant(p) => write!(f, "le fichier existe déjà : {}", p.display()),
            Conflit::DestinationDupliquee { chemin, sources } => write!(
                f,
                "{} fichiers aboutiraient à {}",
                sources.len(),
                chemin.display()
            ),
            Conflit::DossierDuplique(p) => {
                write!(f, "plusieurs éléments visent le dossier {}", p.display())
            }
            Conflit::Dependance(i) => write!(f, "dépend de l'élément bloqué n° {i}"),
        }
    }
}

/// Sort d'un fichier existant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionFichier {
    /// Nom et dossier inchangés.
    Inchange,
    /// Suit seulement le déplacement du dossier (nom inchangé).
    SuitDossier,
    /// Déplacé ou renommé (éventuellement aussi copié vers d'autres noms).
    Deplace,
    /// Laissé en place et copié vers d'autres noms.
    Copie,
    /// Supprimé (obsolète).
    Supprime,
}

/// Ligne d'aperçu : un fichier existant et ses chemins finaux.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LigneFichier {
    pub type_fichier: TypeFichier,
    /// Chemin actuel (avant toute opération).
    pub ancien: PathBuf,
    /// Chemins après renommage (vide si supprimé).
    pub nouveaux: Vec<PathBuf>,
    pub action: ActionFichier,
    /// Un des nouveaux chemins est aussi produit par un autre fichier.
    pub doublon: bool,
}

impl LigneFichier {
    /// Vrai si le fichier garde exactement son chemin.
    pub fn est_inchange(&self) -> bool {
        self.action == ActionFichier::Inchange
    }
}

/// Nature d'un élément de plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NatureElement {
    Film {
        id: String,
    },
    Serie {
        id: String,
    },
    /// Un ou plusieurs épisodes partageant les mêmes fichiers.
    Episodes {
        ids: Vec<String>,
    },
}

/// Plan d'un élément (film, série ou groupe d'épisodes), exécuté de façon
/// transactionnelle.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementPlan {
    pub nature: NatureElement,
    pub libelle: String,
    pub source_donnees: PathBuf,
    pub ancien_dossier: PathBuf,
    pub nouveau_dossier: PathBuf,
    /// Le dossier de destination contient (ou contiendra) plusieurs films.
    pub dossier_multi_films: bool,
    pub lignes: Vec<LigneFichier>,
    pub operations: Vec<Operation>,
    pub conflits: Vec<Conflit>,
    /// Remarques non bloquantes.
    pub avertissements: Vec<String>,
    /// Fichiers de l'élément après renommage (pour mettre le modèle à jour).
    pub fichiers_apres: Vec<FichierMedia>,
    /// Indice d'un élément qui doit réussir avant celui-ci.
    pub depend_de: Option<usize>,
}

impl ElementPlan {
    pub(crate) fn nouveau(
        nature: NatureElement,
        libelle: String,
        source: &Path,
        dossier: &Path,
    ) -> Self {
        ElementPlan {
            nature,
            libelle,
            source_donnees: source.to_path_buf(),
            ancien_dossier: dossier.to_path_buf(),
            nouveau_dossier: dossier.to_path_buf(),
            dossier_multi_films: false,
            lignes: Vec::new(),
            operations: Vec::new(),
            conflits: Vec::new(),
            avertissements: Vec::new(),
            fichiers_apres: Vec::new(),
            depend_de: None,
        }
    }

    /// Aucun conflit.
    pub fn est_executable(&self) -> bool {
        !self.conflits.iter().any(Conflit::est_bloquant)
    }

    /// Rien ne change.
    pub fn est_inchange(&self) -> bool {
        self.operations.iter().all(|o| {
            matches!(
                o,
                Operation::NettoyerSousDossiersVides { .. }
                    | Operation::NettoyerIndesirables { .. }
                    | Operation::SupprimerDossierVide { .. }
            )
        })
    }

    /// Historique (annulation) : dossiers et paires de noms relatifs changés.
    pub fn historique(&self) -> HistoriqueRenommage {
        let mut fichiers = Vec::new();
        for l in &self.lignes {
            if l.action != ActionFichier::Deplace {
                continue;
            }
            let Some(dernier) = l.nouveaux.last() else {
                continue;
            };
            let a = relatif(&l.ancien, &self.ancien_dossier);
            let n = relatif(dernier, &self.nouveau_dossier);
            if a != n {
                fichiers.push((a, n));
            }
        }
        HistoriqueRenommage {
            ancien_chemin: self.ancien_dossier.clone(),
            nouveau_chemin: self.nouveau_dossier.clone(),
            fichiers,
        }
    }
}

/// Chemin relatif (séparateur `/`) ; chemin complet s'il n'est pas sous `base`.
pub fn relatif(chemin: &Path, base: &Path) -> String {
    match chemin.strip_prefix(base) {
        Ok(r) => r
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"),
        Err(_) => chemin.to_string_lossy().into_owned(),
    }
}

/// Plan complet : éléments exécutés dans l'ordre.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlanRenommage {
    pub elements: Vec<ElementPlan>,
}

impl PlanRenommage {
    /// Vrai si au moins un élément est en conflit.
    pub fn a_des_conflits(&self) -> bool {
        self.elements.iter().any(|e| !e.est_executable())
    }

    /// Tous les conflits avec l'indice de leur élément.
    pub fn conflits(&self) -> Vec<(usize, &Conflit)> {
        self.elements
            .iter()
            .enumerate()
            .flat_map(|(i, e)| e.conflits.iter().map(move |c| (i, c)))
            .collect()
    }

    /// Rien à faire.
    pub fn est_vide(&self) -> bool {
        self.elements.iter().all(ElementPlan::est_inchange)
    }

    /// Nombre total d'opérations.
    pub fn nombre_operations(&self) -> usize {
        self.elements.iter().map(|e| e.operations.len()).sum()
    }

    /// Détecte les doublons entre éléments (fichiers finaux et dossiers) et
    /// propage les blocages aux éléments dépendants.
    pub fn verifier(&mut self) {
        // Fichiers finaux.
        let mut finaux: HashMap<String, Vec<(usize, PathBuf)>> = HashMap::new();
        for (i, e) in self.elements.iter().enumerate() {
            for l in &e.lignes {
                for n in &l.nouveaux {
                    finaux
                        .entry(cle(n))
                        .or_default()
                        .push((i, l.ancien.clone()));
                }
            }
        }
        let mut ajouts: BTreeMap<usize, Vec<Conflit>> = BTreeMap::new();
        for (k, occurrences) in &finaux {
            let elements: HashSet<usize> = occurrences.iter().map(|o| o.0).collect();
            if elements.len() < 2 {
                continue;
            }
            let chemin = self
                .elements
                .iter()
                .flat_map(|e| e.lignes.iter())
                .flat_map(|l| l.nouveaux.iter())
                .find(|n| &cle(n) == k)
                .cloned()
                .unwrap_or_default();
            let sources: Vec<PathBuf> = occurrences.iter().map(|o| o.1.clone()).collect();
            for &i in &elements {
                ajouts
                    .entry(i)
                    .or_default()
                    .push(Conflit::DestinationDupliquee {
                        chemin: chemin.clone(),
                        sources: sources.clone(),
                    });
                for l in &mut self.elements[i].lignes {
                    if l.nouveaux.iter().any(|n| &cle(n) == k) {
                        l.doublon = true;
                    }
                }
            }
        }
        // Dossiers : un déplacement de dossier ne peut viser le dossier
        // final d'un autre élément.
        let mut dossiers: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, e) in self.elements.iter().enumerate() {
            if matches!(e.nature, NatureElement::Episodes { .. }) {
                continue;
            }
            dossiers.entry(cle(&e.nouveau_dossier)).or_default().push(i);
        }
        for indices in dossiers.values() {
            if indices.len() < 2 {
                continue;
            }
            let deplace = indices.iter().any(|&i| {
                self.elements[i]
                    .operations
                    .iter()
                    .any(|o| matches!(o, Operation::DeplacerDossier { .. }))
            });
            let tous_multi = indices
                .iter()
                .all(|&i| self.elements[i].dossier_multi_films);
            if deplace && !tous_multi {
                for &i in indices {
                    let d = self.elements[i].nouveau_dossier.clone();
                    ajouts
                        .entry(i)
                        .or_default()
                        .push(Conflit::DossierDuplique(d));
                }
            }
        }
        for (i, c) in ajouts {
            for x in c {
                if !self.elements[i].conflits.contains(&x) {
                    self.elements[i].conflits.push(x);
                }
            }
        }
        // Dépendances.
        for i in 0..self.elements.len() {
            if let Some(d) = self.elements[i].depend_de
                && d < i
                && !self.elements[d].est_executable()
                && !self.elements[i].conflits.contains(&Conflit::Dependance(d))
            {
                self.elements[i].conflits.push(Conflit::Dependance(d));
            }
        }
    }
}

/// Clé de comparaison des chemins : insensible à la casse (prudence pour
/// les systèmes de fichiers qui l'ignorent).
/// Clé de comparaison d'un chemin : insensible à la casse et aux séparateurs
/// (`/` et `\` sont équivalents, un séparateur final est ignoré).
pub(crate) fn cle(p: &Path) -> String {
    let mut s = p.to_string_lossy().replace('\\', "/").to_lowercase();
    while s.len() > 1 && s.ends_with('/') {
        s.pop();
    }
    s
}

pub(crate) fn meme_chemin(a: &Path, b: &Path) -> bool {
    a == b || cle(a) == cle(b)
}

#[cfg(test)]
mod tests_cle {
    use super::meme_chemin;
    use std::path::Path;

    #[test]
    fn separateurs_et_casse_equivalents() {
        assert!(meme_chemin(
            Path::new("F:/Films/the matrix (1999)"),
            Path::new("F:\\Films\\The Matrix (1999)\\")
        ));
        assert!(!meme_chemin(
            Path::new("F:/Films/A"),
            Path::new("F:/Films/B")
        ));
    }
}

// ---------------------------------------------------------------------------
// Construction des opérations à partir des dispositions
// ---------------------------------------------------------------------------

/// Sort décidé pour un fichier existant.
#[derive(Debug, Clone)]
pub(crate) struct Disposition {
    pub fichier: FichierMedia,
    /// Emplacement du fichier au moment des opérations sur les fichiers
    /// (après l'éventuel déplacement du dossier).
    pub courant: PathBuf,
    /// Chemins finaux voulus ; vide ⇒ le fichier reste à `courant`.
    pub cibles: Vec<PathBuf>,
    pub supprimer: bool,
}

/// Remplit `lignes`, `operations` (fichiers), `fichiers_apres` et les
/// conflits de doublons / destinations existantes d'un élément.
/// `operations` doit déjà contenir les opérations de dossier.
pub(crate) fn construire(
    element: &mut ElementPlan,
    dispositions: Vec<Disposition>,
    dossier_deplace: bool,
    sonde: &dyn SondeDisque,
) {
    let courants: HashSet<String> = dispositions.iter().map(|d| cle(&d.courant)).collect();
    let mut transferts: Vec<Operation> = Vec::new();
    let mut suppressions: Vec<Operation> = Vec::new();
    let mut produits: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for d in &dispositions {
        let ancien = d.fichier.chemin_complet();
        if d.supprimer {
            suppressions.push(Operation::Supprimer {
                chemin: d.courant.clone(),
            });
            element.lignes.push(LigneFichier {
                type_fichier: d.fichier.type_fichier,
                ancien,
                nouveaux: Vec::new(),
                action: ActionFichier::Supprime,
                doublon: false,
            });
            continue;
        }
        let mut cibles: Vec<PathBuf> = Vec::new();
        for c in &d.cibles {
            if !cibles.contains(c) {
                cibles.push(c.clone());
            }
        }
        if cibles.is_empty() {
            cibles.push(d.courant.clone());
        }
        let garde = cibles.iter().any(|c| c == &d.courant);
        let action = if garde {
            for c in cibles.iter().filter(|c| *c != &d.courant) {
                transferts.push(Operation::Copier {
                    de: d.courant.clone(),
                    vers: c.clone(),
                });
            }
            if cibles.len() > 1 {
                ActionFichier::Copie
            } else if dossier_deplace && d.courant != ancien {
                ActionFichier::SuitDossier
            } else {
                ActionFichier::Inchange
            }
        } else {
            let (dernier, autres) = cibles.split_last().expect("au moins une cible");
            for c in autres {
                transferts.push(Operation::Copier {
                    de: d.courant.clone(),
                    vers: c.clone(),
                });
            }
            transferts.push(Operation::Deplacer {
                de: d.courant.clone(),
                vers: dernier.clone(),
            });
            ActionFichier::Deplace
        };
        for c in &cibles {
            produits.entry(cle(c)).or_default().push(ancien.clone());
            let mut f = d.fichier.clone();
            f.chemin = c.parent().map(Path::to_path_buf).unwrap_or_default();
            f.nom = c
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            element.fichiers_apres.push(f);
        }
        element.lignes.push(LigneFichier {
            type_fichier: d.fichier.type_fichier,
            ancien,
            nouveaux: cibles,
            action,
            doublon: false,
        });
    }

    // Doublons internes.
    for (k, sources) in &produits {
        if sources.len() > 1 {
            let chemin = element
                .lignes
                .iter()
                .flat_map(|l| l.nouveaux.iter())
                .find(|n| &cle(n) == k)
                .cloned()
                .unwrap_or_default();
            for l in &mut element.lignes {
                if l.nouveaux.iter().any(|n| &cle(n) == k) {
                    l.doublon = true;
                }
            }
            element.conflits.push(Conflit::DestinationDupliquee {
                chemin,
                sources: sources.clone(),
            });
        }
    }
    // Destinations déjà présentes sur le disque (hors fichiers de l'élément).
    for op in &transferts {
        if let Operation::Deplacer { de, vers } | Operation::Copier { de, vers } = op
            && !meme_chemin(de, vers)
            && !courants.contains(&cle(vers))
            && sonde.existe(vers)
        {
            let c = Conflit::FichierExistant(vers.clone());
            if !element.conflits.contains(&c) {
                element.conflits.push(c);
            }
        }
    }
    element.conflits.sort_by_key(|c| format!("{c:?}"));
    element.conflits.dedup();

    element.fichiers_apres.sort_by(comparer_fichiers_film);
    let ordonnees = ordonner_transferts(transferts, &suppressions);
    element.operations.extend(ordonnees);
}

/// Ordonne les suppressions qui libèrent une destination, puis les
/// transferts (une destination n'est écrite qu'après le départ du fichier qui
/// l'occupe ; une copie précède le déplacement de sa source ; les cycles
/// passent par un nom temporaire), puis les autres suppressions.
pub(crate) fn ordonner_transferts(
    transferts: Vec<Operation>,
    suppressions: &[Operation],
) -> Vec<Operation> {
    let destinations: HashSet<String> = transferts
        .iter()
        .filter_map(|o| match o {
            Operation::Deplacer { vers, .. } | Operation::Copier { vers, .. } => Some(cle(vers)),
            _ => None,
        })
        .collect();
    let mut sortie: Vec<Operation> = Vec::new();
    let mut fin: Vec<Operation> = Vec::new();
    for s in suppressions {
        if let Operation::Supprimer { chemin } = s
            && destinations.contains(&cle(chemin))
        {
            sortie.push(s.clone());
        } else {
            fin.push(s.clone());
        }
    }

    let mut attente: Vec<Operation> = transferts;
    let mut temporaires = 0usize;
    while !attente.is_empty() {
        let pret = attente.iter().position(|op| {
            let (de, vers, deplacement) = match op {
                Operation::Deplacer { de, vers } => (de, vers, true),
                Operation::Copier { de, vers } => (de, vers, false),
                _ => return true,
            };
            let occupe = attente.iter().any(|autre| {
                !std::ptr::eq(autre, op)
                    && matches!(autre, Operation::Deplacer { de: d2, .. } if cle(d2) == cle(vers) && cle(d2) != cle(de))
            });
            let copie_en_attente = deplacement
                && attente.iter().any(|autre| {
                    !std::ptr::eq(autre, op)
                        && matches!(autre, Operation::Copier { de: d2, .. } if d2 == de)
                });
            !occupe && !copie_en_attente
        });
        match pret {
            Some(i) => sortie.push(attente.remove(i)),
            None => {
                // Cycle : on libère la source d'un déplacement via un nom temporaire.
                let i = attente
                    .iter()
                    .position(|o| matches!(o, Operation::Deplacer { .. }))
                    .unwrap_or(0);
                let op = attente.remove(i);
                if let Operation::Deplacer { de, vers } = op {
                    temporaires += 1;
                    let nom = de
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let tmp = de.with_file_name(format!(".renommage-{temporaires}-{nom}"));
                    sortie.push(Operation::Deplacer {
                        de: de.clone(),
                        vers: tmp.clone(),
                    });
                    attente.push(Operation::Deplacer { de: tmp, vers });
                } else {
                    sortie.push(op);
                }
            }
        }
    }
    sortie.extend(fin);
    sortie
}

/// Ajoute les opérations de nettoyage des dossiers devenus vides : le
/// dossier quitté puis ses parents jusqu'à la source (exclue).
pub(crate) fn nettoyer_ancetres(
    operations: &mut Vec<Operation>,
    depart: &Path,
    inclure_depart: bool,
    borne: &Path,
    garder: &Path,
) {
    let mut d = if inclure_depart {
        Some(depart.to_path_buf())
    } else {
        depart.parent().map(Path::to_path_buf)
    };
    while let Some(dossier) = d {
        if !dossier.starts_with(borne) || dossier == borne || garder.starts_with(&dossier) {
            break;
        }
        operations.push(Operation::SupprimerDossierVide {
            chemin: dossier.clone(),
        });
        d = dossier.parent().map(Path::to_path_buf);
    }
}

/// Joint un chemin relatif à séparateurs `/` (produit par le gabarit).
pub(crate) fn joindre(base: &Path, relatif: &str) -> PathBuf {
    let mut p = base.to_path_buf();
    for c in relatif.split('/').filter(|c| !c.is_empty()) {
        p.push(c);
    }
    p
}

/// Remplace le préfixe `de` par `vers` (sinon chemin inchangé).
pub(crate) fn reloger(chemin: &Path, de: &Path, vers: &Path) -> PathBuf {
    match chemin.strip_prefix(de) {
        Ok(r) if r.as_os_str().is_empty() => vers.to_path_buf(),
        Ok(r) => vers.join(r),
        Err(_) => chemin.to_path_buf(),
    }
}
