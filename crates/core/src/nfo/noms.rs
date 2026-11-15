//! Noms des fichiers NFO selon les réglages (fonctions pures, chemins
//! relatifs au dossier du film / de la série, séparateur `/`).

use std::path::Path;

use crate::modele::fichier::nom_de_base;
use crate::modele::{Episode, Film, TypeFichier};
use crate::scan::pile::nettoyer_marqueur_pile;

/// Nommage des NFO de films.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NommageNfoFilm {
    /// `<nom de la vidéo sans marqueur de pile>.nfo` (défaut).
    NomFichier,
    /// `movie.nfo`.
    Film,
}

/// Nommage des NFO de séries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum NommageNfoSerie {
    /// `tvshow.nfo`.
    #[default]
    TvShow,
}

/// Nommage des NFO d'épisodes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum NommageNfoEpisode {
    /// `<nom de la vidéo sans marqueur de pile>.nfo`.
    #[default]
    NomFichier,
}

/// Nommage des NFO de saisons (aucun par défaut dans les réglages).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NommageNfoSaison {
    /// `season01.nfo` à la racine de la série (`season-all.nfo` pour -1).
    Saison,
    /// `<dossier de saison>/season.nfo`.
    DossierSaison,
}

/// Chemin relatif d'un fichier par rapport à un dossier (`/` comme séparateur).
fn relatif(dossier: &Path, chemin: &Path) -> String {
    let rel = chemin.strip_prefix(dossier).unwrap_or(chemin);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn nom_fichier_nfo_film(film: &Film, dans_dossier_disque: bool) -> Option<String> {
    if film.est_disque {
        if !dans_dossier_disque {
            return Some("movie.nfo".to_string());
        }
        let principal = film.fichier_principal()?;
        let base = nom_de_base(&principal.nom);
        let dossier = relatif(&film.chemin, &principal.chemin);
        return Some(if dossier.is_empty() {
            format!("{base}.nfo")
        } else {
            format!("{dossier}/{base}.nfo")
        });
    }
    let base = film.nom_video_sans_pile();
    if base.is_empty() {
        None
    } else {
        Some(format!("{base}.nfo"))
    }
}

/// Noms des NFO à écrire pour un film. Liste vide ⇒ rien n'est écrit. Pour
/// un dossier multi-films ou un disque, seul le nom basé sur la vidéo est
/// produit (jamais `movie.nfo` dans un dossier partagé).
pub fn noms_nfo_film(
    film: &Film,
    nommages: &[NommageNfoFilm],
    dans_dossier_disque: bool,
) -> Vec<String> {
    if nommages.is_empty() {
        return Vec::new();
    }
    if film.dossier_multi_films || film.est_disque {
        return nom_fichier_nfo_film(film, dans_dossier_disque)
            .into_iter()
            .collect();
    }
    let mut noms: Vec<String> = Vec::new();
    for n in nommages {
        let nom = match n {
            NommageNfoFilm::NomFichier => nom_fichier_nfo_film(film, dans_dossier_disque),
            NommageNfoFilm::Film => Some("movie.nfo".to_string()),
        };
        if let Some(nom) = nom
            && !noms.contains(&nom)
        {
            noms.push(nom);
        }
    }
    noms
}

/// Nom du NFO de série.
pub fn nom_nfo_serie(nommage: NommageNfoSerie) -> String {
    match nommage {
        NommageNfoSerie::TvShow => "tvshow.nfo".to_string(),
    }
}

/// Nom du NFO d'un épisode (à côté de sa première vidéo). `None` sans vidéo.
pub fn nom_nfo_episode(episode: &Episode, nommage: NommageNfoEpisode) -> Option<String> {
    match nommage {
        NommageNfoEpisode::NomFichier => {
            let video = episode
                .fichiers
                .iter()
                .find(|f| f.type_fichier == TypeFichier::Video)?;
            let base = nettoyer_marqueur_pile(&nom_de_base(&video.nom));
            if base.is_empty() {
                None
            } else {
                Some(format!("{base}.nfo"))
            }
        }
    }
}

/// Nom du NFO d'une saison, relatif au dossier de la série.
pub fn nom_nfo_saison(numero: i32, nommage: NommageNfoSaison, dossier_saison: &str) -> String {
    match nommage {
        NommageNfoSaison::Saison if numero < 0 => "season-all.nfo".to_string(),
        NommageNfoSaison::Saison => format!("season{numero:02}.nfo"),
        NommageNfoSaison::DossierSaison => {
            let d = dossier_saison.trim_matches(['/', '\\']);
            if d.is_empty() {
                "season.nfo".to_string()
            } else {
                format!("{d}/season.nfo")
            }
        }
    }
}
