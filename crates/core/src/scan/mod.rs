//! Scan d'une source de données : détection des films (dossier par film,
//! fichiers à plat, disques), de leurs fichiers associés, et extraction du
//! titre, de l'année et des identifiants depuis les noms.
//!
//! Point d'entrée : [`scanner_source`].

pub mod extensions;
pub mod ignore;
pub mod pile;
pub mod regroupement;
pub mod titre;
pub mod types;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::modele::{Film, TypeFichier};
use ignore::{MotifExclusion, contient_marqueur, est_ignore};
use regroupement::{ResultatDossier, creer_fichier, film_depuis_dossier, films_a_plat};
use types::{est_fichier_identifiant_disque, est_nom_dossier_disque, type_fichier};

/// Réglages du scan.
#[derive(Debug, Clone)]
pub struct OptionsScan {
    /// Motifs d'exclusion de dossiers (regex sur le nom, ou chemin complet).
    pub dossiers_ignores: Vec<String>,
    /// Ignorer les dossiers contenant un fichier `.nomedia`.
    pub ignorer_nomedia: bool,
    /// Expressions rationnelles retirées des noms avant détection du titre.
    pub mots_interdits: Vec<String>,
    /// Articles déplacés par « Matrix, The » ⇒ « The Matrix ».
    pub prefixes_titre: Vec<String>,
    /// Autres sources de données imbriquées dans celle-ci (ignorées).
    pub sources_exclues: Vec<PathBuf>,
}

impl Default for OptionsScan {
    fn default() -> Self {
        Self {
            dossiers_ignores: ignore::DOSSIERS_IGNORES_DEFAUT
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ignorer_nomedia: true,
            mots_interdits: Vec::new(),
            prefixes_titre: titre::prefixes_titre_defaut(),
            sources_exclues: Vec::new(),
        }
    }
}

/// Erreur rendant la source inutilisable (il ne faut alors rien supprimer de
/// la base : disque non monté, par exemple).
#[derive(Debug, thiserror::Error)]
pub enum ErreurScan {
    #[error("source de données introuvable : {0}")]
    SourceIntrouvable(PathBuf),
    #[error("la source de données n'est pas un dossier : {0}")]
    PasUnDossier(PathBuf),
    #[error("source de données vide (disque non monté ?) : {0}")]
    SourceVide(PathBuf),
}

/// Problème non bloquant rencontré pendant le scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Avertissement {
    /// Dossier `BDMV` / `VIDEO_TS` / `HVDVD_TS` directement à la racine.
    DossierDisqueALaRacine(PathBuf),
    /// Erreur de lecture (droits, boucle de liens…).
    Lecture {
        chemin: Option<PathBuf>,
        message: String,
    },
}

/// Résultat d'un scan.
#[derive(Debug, Clone, Default)]
pub struct ResultatScan {
    /// Films détectés, triés par dossier puis par nom du premier fichier vidéo.
    pub films: Vec<Film>,
    pub avertissements: Vec<Avertissement>,
}

/// Arborescence explorée.
struct Arbre {
    /// Fichiers (non cachés) par dossier.
    fichiers: BTreeMap<PathBuf, Vec<PathBuf>>,
    /// Dossiers de films de type disque (parents de `BDMV`/`VIDEO_TS`/`HVDVD_TS`).
    dossiers_disque: BTreeSet<PathBuf>,
    avertissements: Vec<Avertissement>,
}

/// Fichiers d'un dossier de structure de disque qu'on conserve : `.nfo`,
/// fichiers `*trailer*` et fichiers identifiants.
fn fichiers_disque_utiles(dossier: &Path) -> Vec<PathBuf> {
    let Ok(entrees) = std::fs::read_dir(dossier) else {
        return Vec::new();
    };
    let mut v: Vec<PathBuf> = entrees
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| {
            let n = p
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            n.ends_with(".nfo") || n.contains("trailer") || est_fichier_identifiant_disque(&n)
        })
        .collect();
    v.sort();
    v
}

fn explorer(racine: &Path, options: &OptionsScan, motifs: &[MotifExclusion]) -> Arbre {
    let mut arbre = Arbre {
        fichiers: BTreeMap::new(),
        dossiers_disque: BTreeSet::new(),
        avertissements: Vec::new(),
    };
    let mut it = WalkDir::new(racine)
        .follow_links(true)
        .sort_by_file_name()
        .into_iter();
    while let Some(entree) = it.next() {
        let e = match entree {
            Ok(e) => e,
            Err(err) => {
                arbre.avertissements.push(Avertissement::Lecture {
                    chemin: err.path().map(Path::to_path_buf),
                    message: err.to_string(),
                });
                continue;
            }
        };
        let chemin = e.path();
        if e.depth() == 0 {
            if contient_marqueur(chemin, options.ignorer_nomedia) {
                it.skip_current_dir();
            }
            continue;
        }
        if e.file_type().is_dir() {
            if est_ignore(chemin, motifs)
                || options.sources_exclues.iter().any(|s| s == chemin)
                || contient_marqueur(chemin, options.ignorer_nomedia)
            {
                it.skip_current_dir();
                continue;
            }
            let nom = chemin
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if est_nom_dossier_disque(&nom) {
                // Un film disque est le dossier PARENT ; on ne descend pas.
                it.skip_current_dir();
                let parent = chemin.parent().unwrap_or(racine).to_path_buf();
                if parent == racine {
                    arbre
                        .avertissements
                        .push(Avertissement::DossierDisqueALaRacine(chemin.to_path_buf()));
                    continue;
                }
                arbre.dossiers_disque.insert(parent);
                let utiles = fichiers_disque_utiles(chemin);
                arbre
                    .fichiers
                    .entry(chemin.to_path_buf())
                    .or_default()
                    .extend(utiles);
            }
        } else if e.file_type().is_file() {
            if est_ignore(chemin, &[]) {
                continue;
            }
            if let Some(parent) = chemin.parent() {
                arbre
                    .fichiers
                    .entry(parent.to_path_buf())
                    .or_default()
                    .push(chemin.to_path_buf());
            }
        }
    }
    arbre
}

/// Scanne une source de données et renvoie les films détectés.
///
/// Règles principales :
/// * un dossier contenant des vidéos (au sens `TypeFichier::Video`, donc ni
///   bande-annonce, ni échantillon, ni extra) est un film ;
/// * `Film/CD1`, `Film/CD2` : le dossier parent est le film ;
/// * un dossier contenant des vidéos ET des sous-dossiers de films, ou
///   plusieurs vidéos de noms différents, est un dossier multi-films : chaque
///   vidéo forme un film avec ses fichiers associés ;
/// * les fichiers à la racine de la source sont traités comme un dossier
///   multi-films ;
/// * les dossiers `BDMV`, `VIDEO_TS`, `HVDVD_TS` ne sont jamais un film : le
///   film disque est leur dossier parent ;
/// * dossiers cachés, dossiers système, motifs d'exclusion et dossiers
///   marqués (`.nomedia`…) sont ignorés avec tout leur sous-arbre.
pub fn scanner_source(racine: &Path, options: &OptionsScan) -> Result<ResultatScan, ErreurScan> {
    if !racine.exists() {
        return Err(ErreurScan::SourceIntrouvable(racine.to_path_buf()));
    }
    if !racine.is_dir() {
        return Err(ErreurScan::PasUnDossier(racine.to_path_buf()));
    }
    // Hors Windows, une source vide est considérée comme indisponible.
    if !cfg!(windows)
        && std::fs::read_dir(racine)
            .map(|mut d| d.next().is_none())
            .unwrap_or(true)
    {
        return Err(ErreurScan::SourceVide(racine.to_path_buf()));
    }

    let motifs: Vec<MotifExclusion> = options
        .dossiers_ignores
        .iter()
        .map(|m| MotifExclusion::nouveau(m))
        .collect();
    let arbre = explorer(racine, options, &motifs);
    let mut resultat = ResultatScan {
        films: Vec::new(),
        avertissements: arbre.avertissements.clone(),
    };

    let est_dossier_structure = |d: &Path| {
        d.file_name()
            .is_some_and(|n| est_nom_dossier_disque(&n.to_string_lossy()))
    };

    // Dossiers « vidéo » : au moins un fichier de type VIDEO, ou film disque.
    let mut dossiers_video: BTreeSet<PathBuf> = arbre.dossiers_disque.clone();
    for (dossier, fichiers) in &arbre.fichiers {
        if est_dossier_structure(dossier) || dossier == racine {
            continue;
        }
        if fichiers
            .iter()
            .any(|f| type_fichier(f, racine) == TypeFichier::Video)
        {
            dossiers_video.insert(dossier.clone());
        }
    }

    let fichiers_directs =
        |d: &Path| -> Vec<PathBuf> { arbre.fichiers.get(d).cloned().unwrap_or_default() };
    let fichiers_sous = |d: &Path| -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = arbre
            .fichiers
            .range(d.to_path_buf()..)
            .take_while(|(k, _)| k.starts_with(d))
            .flat_map(|(_, f)| f.iter().cloned())
            .collect();
        v.sort();
        v
    };

    let mut traites: BTreeSet<PathBuf> = BTreeSet::new();

    for dossier in &dossiers_video {
        if traites.contains(dossier) {
            continue;
        }
        let nom = dossier
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        // Film disque : le dossier est le film ; les sous-films sont ignorés.
        if arbre.dossiers_disque.contains(dossier) {
            for autre in dossiers_video.iter().filter(|o| o.starts_with(dossier)) {
                traites.insert(autre.clone());
            }
            if let ResultatDossier::Film(f) =
                film_depuis_dossier(dossier, &fichiers_sous(dossier), racine, options, true)
            {
                resultat.films.push(*f);
            }
            continue;
        }

        // Dossier de pile (`Film/CD1`) : on traite le parent, une seule fois.
        if pile::est_dossier_de_pile(&nom)
            && let Some(parent) = dossier
                .parent()
                .filter(|p| *p != racine && p.starts_with(racine))
        {
            if traites.insert(parent.to_path_buf()) {
                traiter_dossier(
                    parent,
                    &dossiers_video,
                    racine,
                    options,
                    &fichiers_directs,
                    &fichiers_sous,
                    &mut resultat,
                );
            }
            traites.insert(dossier.clone());
            continue;
        }
        traites.insert(dossier.clone());
        traiter_dossier(
            dossier,
            &dossiers_video,
            racine,
            options,
            &fichiers_directs,
            &fichiers_sous,
            &mut resultat,
        );
    }

    // Fichiers posés directement à la racine de la source.
    let a_la_racine = fichiers_directs(racine);
    if a_la_racine
        .iter()
        .any(|f| type_fichier(f, racine) == TypeFichier::Video)
    {
        resultat
            .films
            .extend(films_a_plat(racine, &a_la_racine, racine, options));
    }

    resultat.films.sort_by(|a, b| {
        let cle = |f: &Film| {
            (
                f.chemin.clone(),
                f.fichier_principal()
                    .map(|p| p.nom.clone())
                    .unwrap_or_default(),
            )
        };
        cle(a).cmp(&cle(b))
    });
    Ok(resultat)
}

/// Traite un dossier « vidéo » ordinaire (ni disque, ni racine).
fn traiter_dossier(
    dossier: &Path,
    dossiers_video: &BTreeSet<PathBuf>,
    racine: &Path,
    options: &OptionsScan,
    fichiers_directs: &dyn Fn(&Path) -> Vec<PathBuf>,
    fichiers_sous: &dyn Fn(&Path) -> Vec<PathBuf>,
    resultat: &mut ResultatScan,
) {
    let directs = fichiers_directs(dossier);

    // Un autre dossier vidéo dessous (hors dossiers de pile) ⇒ multi-films,
    // limité aux fichiers directs du dossier.
    let autre_dossier_video = dossiers_video.iter().any(|o| {
        o != dossier
            && o.starts_with(dossier)
            && !(o.parent() == Some(dossier)
                && o.file_name()
                    .is_some_and(|n| pile::est_dossier_de_pile(&n.to_string_lossy())))
    });
    if autre_dossier_video {
        resultat
            .films
            .extend(films_a_plat(dossier, &directs, racine, options));
        return;
    }

    // Plusieurs noms de vidéos distincts (après retrait du marqueur de pile) ⇒ multi-films.
    let mut bases: BTreeSet<String> = BTreeSet::new();
    for f in directs
        .iter()
        .filter(|f| type_fichier(f, racine) == TypeFichier::Video)
    {
        let nom = creer_fichier(f, racine).nom_de_base();
        bases.insert(pile::nettoyer_marqueur_pile(&nom).to_lowercase());
    }
    if bases.len() > 1 {
        resultat
            .films
            .extend(films_a_plat(dossier, &directs, racine, options));
        return;
    }

    match film_depuis_dossier(dossier, &fichiers_sous(dossier), racine, options, false) {
        ResultatDossier::Film(f) => resultat.films.push(*f),
        ResultatDossier::Regroupement => {
            resultat
                .films
                .extend(films_a_plat(dossier, &directs, racine, options));
        }
        ResultatDossier::Aucun => {}
    }
}

#[cfg(test)]
mod tests;
