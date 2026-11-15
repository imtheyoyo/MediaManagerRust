//! Sauvegardes quotidiennes de `data/` et restauration de la base.
//!
//! - Une archive par jour : `backup/data.AAAA-MM-JJ.zip` (réécrite dans la
//!   journée), entrées relatives au parent de `data/` (`data/reglages.json`…).
//! - La base y figure sous forme de **copie cohérente** (`VACUUM INTO`) et
//!   seulement si son contrôle d'intégrité réussit : une base corrompue n'est
//!   jamais sauvegardée et n'écrase donc jamais la sauvegarde du jour. Il n'y a
//!   ainsi pas lieu d'ignorer l'archive la plus récente lors d'une restauration.
//! - Exclus : verrou, journaux WAL/SHM, fichiers temporaires, fichiers
//!   `.corrompu`, fichier de secrets.
//! - Rotation : les [`SAUVEGARDES_CONSERVEES`] plus récentes sont gardées ; le
//!   tri se fait sur la **date contenue dans le nom** (pas sur la date de
//!   modification, sujette aux copies et aux dépassements).

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use rusqlite::{Connection, OpenFlags};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::atomique::chemin_temporaire;
use crate::base::{self, Bibliotheque, NOM_BASE};
use crate::dossiers::DossiersApplication;
use crate::erreur::{ErreurStockage, Resultat};
use crate::secrets::NOM_FICHIER_SECRETS;
use crate::verrou::NOM_VERROU;

/// Nombre d'archives conservées par la rotation.
pub const SAUVEGARDES_CONSERVEES: usize = 5;
const PREFIXE: &str = "data.";
const SUFFIXE: &str = ".zip";

/// Nom de l'archive d'un jour donné.
pub fn nom_archive(date: NaiveDate) -> String {
    format!("{PREFIXE}{}{SUFFIXE}", date.format("%Y-%m-%d"))
}

/// Date d'une archive d'après son nom (`None` si le nom ne correspond pas).
fn date_archive(nom: &str) -> Option<NaiveDate> {
    let milieu = nom.strip_prefix(PREFIXE)?.strip_suffix(SUFFIXE)?;
    NaiveDate::parse_from_str(milieu, "%Y-%m-%d").ok()
}

/// Résultat de la sauvegarde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EtatSauvegarde {
    /// Archive écrite.
    Creee(PathBuf),
    /// Pas de dossier `data/` : rien à sauvegarder.
    RienASauvegarder,
    /// Base corrompue : aucune archive écrite (l'existante est préservée).
    BaseCorrompue(String),
}

/// Sauvegarde du jour (date locale) puis rotation.
pub fn creer_sauvegarde_quotidienne(dossiers: &DossiersApplication) -> Resultat<EtatSauvegarde> {
    let etat = creer_sauvegarde(dossiers, chrono::Local::now().date_naive())?;
    if matches!(etat, EtatSauvegarde::Creee(_)) {
        rotation(&dossiers.sauvegardes, SAUVEGARDES_CONSERVEES)?;
    }
    Ok(etat)
}

/// Écrit l'archive de `date` (remplacée si elle existe). Utilisable que la
/// base soit ouverte ou non.
pub fn creer_sauvegarde(
    dossiers: &DossiersApplication,
    date: NaiveDate,
) -> Resultat<EtatSauvegarde> {
    let donnees = &dossiers.donnees;
    if !donnees.is_dir() {
        return Ok(EtatSauvegarde::RienASauvegarder);
    }
    fs::create_dir_all(&dossiers.sauvegardes)
        .map_err(|e| ErreurStockage::io(&dossiers.sauvegardes, e))?;

    // Copie cohérente et vérifiée de la base.
    let base_src = donnees.join(NOM_BASE);
    let copie = dossiers.sauvegardes.join(format!(".{NOM_BASE}.copie"));
    let avec_base = if base_src.is_file() {
        match copier_base_verifiee(&base_src, &copie) {
            Ok(()) => true,
            Err(ErreurStockage::BaseCorrompue(m)) => {
                tracing::error!("base corrompue, sauvegarde du jour non écrite : {m}");
                return Ok(EtatSauvegarde::BaseCorrompue(m));
            }
            Err(e) => return Err(e),
        }
    } else {
        false
    };

    let cible = dossiers.sauvegardes.join(nom_archive(date));
    let tmp = chemin_temporaire(&cible);
    let resultat = ecrire_archive(donnees, &tmp, avec_base.then_some(copie.as_path()))
        .and_then(|()| fs::rename(&tmp, &cible).map_err(|e| ErreurStockage::io(&cible, e)));
    let _ = fs::remove_file(&copie);
    if resultat.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    resultat?;
    tracing::info!("sauvegarde écrite : {cible:?}");
    Ok(EtatSauvegarde::Creee(cible))
}

fn copier_base_verifiee(base_src: &Path, copie: &Path) -> Resultat<()> {
    let conn = Connection::open_with_flags(
        base_src,
        // Lecture-écriture sans création : l'ouverture en lecture seule d'une
        // base en mode WAL échoue si le journal n'existe pas.
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    base::verifier_integrite(&conn)?;
    base::copier_base(&conn, copie).map_err(base::qualifier_corruption)
}

/// Faut-il exclure ce fichier de `data/` de l'archive ?
fn exclu(nom: &str) -> bool {
    nom == NOM_VERROU
        || nom == NOM_FICHIER_SECRETS
        || nom == NOM_BASE
        || nom.ends_with("-wal")
        || nom.ends_with("-shm")
        || nom.ends_with("-journal")
        || nom.ends_with(".tmp")
        || nom.ends_with(".corrompu")
}

fn ecrire_archive(donnees: &Path, cible: &Path, copie_base: Option<&Path>) -> Resultat<()> {
    let f = File::create(cible).map_err(|e| ErreurStockage::io(cible, e))?;
    let mut zip = ZipWriter::new(f);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .large_file(true);

    let mut pile = vec![donnees.to_path_buf()];
    while let Some(dossier) = pile.pop() {
        let mut entrees: Vec<_> = fs::read_dir(&dossier)
            .map_err(|e| ErreurStockage::io(&dossier, e))?
            .filter_map(Result::ok)
            .collect();
        entrees.sort_by_key(|e| e.file_name());
        for e in entrees {
            let chemin = e.path();
            let nom = e.file_name().to_string_lossy().into_owned();
            let type_f = e
                .file_type()
                .map_err(|err| ErreurStockage::io(&chemin, err))?;
            if type_f.is_dir() {
                pile.push(chemin);
                continue;
            }
            if !type_f.is_file() || exclu(&nom) {
                continue;
            }
            let relatif = chemin.strip_prefix(donnees).unwrap_or(&chemin);
            ajouter_fichier(&mut zip, &chemin, &nom_entree(relatif), options)?;
        }
    }
    if let Some(copie) = copie_base {
        ajouter_fichier(&mut zip, copie, &format!("data/{NOM_BASE}"), options)?;
    }
    let f = zip.finish()?;
    f.sync_all().map_err(|e| ErreurStockage::io(cible, e))?;
    Ok(())
}

/// `data/a/b` avec séparateurs `/` quel que soit le système.
fn nom_entree(relatif: &Path) -> String {
    let mut s = String::from("data");
    for c in relatif.components() {
        s.push('/');
        s.push_str(&c.as_os_str().to_string_lossy());
    }
    s
}

fn ajouter_fichier(
    zip: &mut ZipWriter<File>,
    source: &Path,
    nom: &str,
    options: SimpleFileOptions,
) -> Resultat<()> {
    let mut f = File::open(source).map_err(|e| ErreurStockage::io(source, e))?;
    zip.start_file(nom, options)?;
    io::copy(&mut f, zip).map_err(|e| ErreurStockage::io(source, e))?;
    Ok(())
}

/// Archives quotidiennes présentes, de la plus récente à la plus ancienne.
pub fn lister_sauvegardes(dossier: &Path) -> Resultat<Vec<PathBuf>> {
    let lecture = match fs::read_dir(dossier) {
        Ok(l) => l,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(ErreurStockage::io(dossier, e)),
    };
    let mut v: Vec<(NaiveDate, PathBuf)> = lecture
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let d = date_archive(&e.file_name().to_string_lossy())?;
            Some((d, e.path()))
        })
        .collect();
    v.sort_by_key(|e| std::cmp::Reverse(e.0));
    Ok(v.into_iter().map(|(_, p)| p).collect())
}

/// Supprime les archives au-delà des `conserver` plus récentes ; renvoie les
/// fichiers supprimés.
pub fn rotation(dossier: &Path, conserver: usize) -> Resultat<Vec<PathBuf>> {
    let mut supprimees = Vec::new();
    for p in lister_sauvegardes(dossier)?.into_iter().skip(conserver) {
        fs::remove_file(&p).map_err(|e| ErreurStockage::io(&p, e))?;
        supprimees.push(p);
    }
    Ok(supprimees)
}

// ---------------------------------------------------------------------------
// Restauration
// ---------------------------------------------------------------------------

/// Issue de [`ouvrir_ou_restaurer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EtatOuverture {
    /// Base ouverte normalement.
    Normale,
    /// Base corrompue mise de côté et restaurée depuis cette archive.
    Restauree {
        archive: PathBuf,
        base_corrompue: PathBuf,
    },
    /// Base corrompue mise de côté, aucune archive utilisable : base vide.
    Reinitialisee { base_corrompue: PathBuf },
}

/// Ouvre la base ; si elle est corrompue, la met de côté
/// (`backup/bibliotheque.db.<horodatage>.corrompu`) puis tente les archives
/// de la plus récente à la plus ancienne ; à défaut, crée une base vide.
/// L'issue est à signaler à l'utilisateur.
pub fn ouvrir_ou_restaurer(
    dossiers: &DossiersApplication,
) -> Resultat<(Bibliotheque, EtatOuverture)> {
    let chemin = dossiers.base();
    match Bibliotheque::ouvrir(&chemin) {
        Ok(b) => return Ok((b, EtatOuverture::Normale)),
        Err(ErreurStockage::BaseCorrompue(m)) => {
            tracing::error!("base corrompue ({m}) : tentative de restauration");
        }
        Err(e) => return Err(e),
    }
    let base_corrompue = mettre_base_de_cote(&chemin, &dossiers.sauvegardes)?;

    for archive in lister_sauvegardes(&dossiers.sauvegardes)? {
        match extraire_base(&archive, &chemin) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(e) => {
                tracing::warn!("archive {archive:?} inutilisable : {e}");
                continue;
            }
        }
        match Bibliotheque::ouvrir(&chemin) {
            Ok(b) => {
                tracing::warn!("base restaurée depuis {archive:?}");
                return Ok((
                    b,
                    EtatOuverture::Restauree {
                        archive,
                        base_corrompue,
                    },
                ));
            }
            Err(ErreurStockage::BaseCorrompue(m)) => {
                tracing::warn!("base de {archive:?} corrompue elle aussi : {m}");
                supprimer_base(&chemin)?;
            }
            Err(e) => return Err(e),
        }
    }
    tracing::error!("aucune sauvegarde utilisable : nouvelle base vide");
    let b = Bibliotheque::ouvrir(&chemin)?;
    Ok((b, EtatOuverture::Reinitialisee { base_corrompue }))
}

fn fichiers_annexes(base: &Path) -> [PathBuf; 2] {
    let annexe = |s: &str| {
        let mut n = base.file_name().unwrap_or_default().to_os_string();
        n.push(s);
        base.with_file_name(n)
    };
    [annexe("-wal"), annexe("-shm")]
}

fn supprimer_base(base: &Path) -> Resultat<()> {
    for p in std::iter::once(base.to_path_buf()).chain(fichiers_annexes(base)) {
        match fs::remove_file(&p) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(ErreurStockage::io(&p, e)),
        }
    }
    Ok(())
}

fn mettre_base_de_cote(base: &Path, dossier_sauvegardes: &Path) -> Resultat<PathBuf> {
    fs::create_dir_all(dossier_sauvegardes)
        .map_err(|e| ErreurStockage::io(dossier_sauvegardes, e))?;
    let horodatage = chrono::Local::now().format("%Y%m%d-%H%M%S%.3f");
    let cible = dossier_sauvegardes.join(format!("{NOM_BASE}.{horodatage}.corrompu"));
    // `rename` peut échouer entre volumes : repli copie + suppression.
    if fs::rename(base, &cible).is_err() {
        fs::copy(base, &cible).map_err(|e| ErreurStockage::io(base, e))?;
    }
    supprimer_base(base)?;
    Ok(cible)
}

/// Extrait `data/<base>` de l'archive vers `cible` ; `false` si absente.
fn extraire_base(archive: &Path, cible: &Path) -> Resultat<bool> {
    let f = File::open(archive).map_err(|e| ErreurStockage::io(archive, e))?;
    let mut zip = ZipArchive::new(f)?;
    let mut entree = match zip.by_name(&format!("data/{NOM_BASE}")) {
        Ok(e) => e,
        Err(zip::result::ZipError::FileNotFound) => return Ok(false),
        Err(e) => return Err(e.into()),
    };
    supprimer_base(cible)?;
    ecrire_depuis(&mut entree, cible)?;
    Ok(true)
}

fn ecrire_depuis(source: &mut impl Read, cible: &Path) -> Resultat<()> {
    if let Some(parent) = cible.parent() {
        fs::create_dir_all(parent).map_err(|e| ErreurStockage::io(parent, e))?;
    }
    let tmp = chemin_temporaire(cible);
    let r = (|| -> io::Result<()> {
        let mut f = File::create(&tmp)?;
        io::copy(source, &mut f)?;
        f.flush()?;
        f.sync_all()?;
        fs::rename(&tmp, cible)
    })();
    r.map_err(|e| {
        let _ = fs::remove_file(&tmp);
        ErreurStockage::io(cible, e)
    })
}

/// Restauration manuelle complète : remplace le contenu de `data/` par celui
/// de l'archive (hors verrou). La base doit être **fermée**. Renvoie le
/// nombre de fichiers restaurés.
pub fn restaurer_archive(archive: &Path, dossiers: &DossiersApplication) -> Resultat<usize> {
    let f = File::open(archive).map_err(|e| ErreurStockage::io(archive, e))?;
    let mut zip = ZipArchive::new(f)?;
    // Contrôle préalable : toutes les entrées sous `data/`, sans évasion.
    let mut plan = Vec::new();
    for i in 0..zip.len() {
        let e = zip.by_index(i)?;
        if e.is_dir() {
            continue;
        }
        let nom = e.enclosed_name().ok_or_else(|| {
            ErreurStockage::SauvegardeInvalide(format!("entrée suspecte : {}", e.name()))
        })?;
        let relatif = nom
            .strip_prefix("data")
            .map_err(|_| {
                ErreurStockage::SauvegardeInvalide(format!("entrée hors de data/ : {}", e.name()))
            })?
            .to_path_buf();
        if relatif.as_os_str().is_empty() || relatif == Path::new(NOM_VERROU) {
            continue;
        }
        plan.push((i, relatif));
    }
    if plan.is_empty() {
        return Err(ErreurStockage::SauvegardeInvalide("archive vide".into()));
    }
    // Un journal WAL résiduel corromprait la base restaurée.
    supprimer_base(&dossiers.base())?;
    for (i, relatif) in &plan {
        let mut e = zip.by_index(*i)?;
        ecrire_depuis(&mut e, &dossiers.donnees.join(relatif))?;
    }
    Ok(plan.len())
}
