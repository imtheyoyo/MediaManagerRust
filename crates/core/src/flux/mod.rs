//! Analyse des flux audio/vidéo/sous-titres des fichiers médias par outil externe.
//!
//! Déroulement :
//! 1. [`trouver_outils`] localise `ffprobe` et `mediainfo` (réglage, dossier
//!    de l'application, `PATH`) ;
//! 2. [`analyser_fichier`] lance l'outil (sans fenêtre, avec délai maximal) et
//!    convertit sa sortie texte en [`InfosMedia`] normalisées ;
//! 3. [`appliquer_au_fichier`] reporte ces informations dans un
//!    [`FichierMedia`](crate::modele::FichierMedia).
//!
//! Les sorties sont lues dans des formats « clé = valeur » (`ffprobe -of flat`,
//! `mediainfo --Language=raw --Full`), ce qui évite toute dépendance JSON.

mod application;
mod execution;
pub mod ffprobe;
pub mod mediainfo;
pub mod normalisation;
mod outils;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::modele::{FluxAudio, FluxSousTitres, FluxVideo};

pub use application::appliquer_au_fichier;
pub use outils::{ConfigOutils, Outils, trouver_outils, version_outil};

/// Délai maximal d'analyse par défaut.
pub const DELAI_PAR_DEFAUT: Duration = Duration::from_secs(120);

/// Outil ayant produit une analyse.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutilAnalyse {
    #[default]
    Ffprobe,
    MediaInfo,
}

/// Erreurs de l'analyse des flux.
#[derive(Debug, thiserror::Error)]
pub enum ErreurFlux {
    #[error("aucun outil d'analyse disponible (ffprobe ou mediainfo)")]
    AucunOutil,
    #[error("fichier introuvable : {0}")]
    FichierIntrouvable(PathBuf),
    #[error("impossible de lancer {outil} : {source}")]
    Lancement {
        outil: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{outil} n'a pas répondu en {secondes} s")]
    DelaiDepasse { outil: String, secondes: u64 },
    #[error("{outil} a échoué (code {code:?}) : {message}")]
    Echec {
        outil: String,
        code: Option<i32>,
        message: String,
    },
    #[error("sortie de {outil} inexploitable : {raison}")]
    SortieInvalide { outil: String, raison: String },
}

/// Informations techniques normalisées d'un fichier média.
///
/// Les débits sont en kbit/s, la durée en secondes, les langues en
/// ISO 639-2/T (`fra`, `eng`), vides si inconnues.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InfosMedia {
    /// Outil à l'origine de l'analyse.
    pub source: OutilAnalyse,
    /// Conteneur en forme courte (`mkv`, `mp4`, `avi`, `ts`…).
    pub conteneur: String,
    pub duree_secondes: u32,
    /// Débit global en kbit/s.
    pub debit_global: u32,
    /// Titre inscrit dans le conteneur.
    pub titre: String,
    /// Flux vidéo (hors images jointes), le principal en premier.
    pub videos: Vec<FluxVideo>,
    /// Tous les formats HDR reconnus pour la vidéo principale, du plus riche
    /// au plus simple (`Dolby Vision`, `HDR10+`, `HDR10`, `HLG`).
    pub formats_hdr: Vec<String>,
    pub audio: Vec<FluxAudio>,
    pub sous_titres: Vec<FluxSousTitres>,
}

impl InfosMedia {
    /// Flux vidéo principal.
    pub fn video(&self) -> Option<&FluxVideo> {
        self.videos.first()
    }

    /// Catégorie de hauteur de la vidéo principale (`1080p`…), vide sans vidéo.
    pub fn format_video(&self) -> String {
        self.video()
            .map(|v| normalisation::format_video(v.largeur, v.hauteur))
            .unwrap_or_default()
    }

    /// Vrai si aucun flux n'a été reconnu.
    pub fn est_vide(&self) -> bool {
        self.videos.is_empty() && self.audio.is_empty() && self.sous_titres.is_empty()
    }
}

/// Analyse un fichier avec le premier outil disponible (`ffprobe`, sinon
/// `mediainfo`). Si `ffprobe` échoue et que `mediainfo` est présent, ce
/// dernier est essayé avant de renvoyer l'erreur d'origine.
pub fn analyser_fichier(chemin: &Path, outils: &Outils) -> Result<InfosMedia, ErreurFlux> {
    if outils.ffprobe.is_none() && outils.mediainfo.is_none() {
        return Err(ErreurFlux::AucunOutil);
    }
    if !chemin.is_file() {
        return Err(ErreurFlux::FichierIntrouvable(chemin.to_path_buf()));
    }
    let extension = chemin
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let mut erreur = None;
    if let Some(exe) = &outils.ffprobe {
        match analyser_avec_ffprobe(exe, chemin, &extension, outils.delai) {
            Ok(infos) => return Ok(infos),
            Err(e) => erreur = Some(e),
        }
    }
    if let Some(exe) = &outils.mediainfo {
        match analyser_avec_mediainfo(exe, chemin, &extension, outils.delai) {
            Ok(infos) => return Ok(infos),
            Err(e) => {
                erreur.get_or_insert(e);
            }
        }
    }
    Err(erreur.unwrap_or(ErreurFlux::AucunOutil))
}

fn analyser_avec_ffprobe(
    exe: &Path,
    chemin: &Path,
    extension: &str,
    delai: Duration,
) -> Result<InfosMedia, ErreurFlux> {
    let sortie = execution::executer(exe, &ffprobe::arguments(chemin), delai)?;
    let mut infos = ffprobe::analyser_sortie(&sortie, extension)?;
    // Les métadonnées HDR dynamiques ne figurent que dans les trames : une
    // seconde lecture, limitée à la première image, n'est faite que pour une
    // vidéo PQ. Son échec n'invalide pas l'analyse.
    if infos.formats_hdr.iter().any(|f| f == normalisation::HDR10)
        && !infos
            .formats_hdr
            .iter()
            .any(|f| f == normalisation::HDR10_PLUS)
        && let Ok(trames) = execution::executer(exe, &ffprobe::arguments_trame(chemin), delai)
        && ffprobe::trame_hdr10_plus(&trames)
    {
        infos
            .formats_hdr
            .push(normalisation::HDR10_PLUS.to_string());
        normalisation::trier_formats_hdr(&mut infos.formats_hdr);
        if let Some(v) = infos.videos.first_mut() {
            v.format_hdr = infos.formats_hdr[0].clone();
        }
    }
    Ok(infos)
}

fn analyser_avec_mediainfo(
    exe: &Path,
    chemin: &Path,
    extension: &str,
    delai: Duration,
) -> Result<InfosMedia, ErreurFlux> {
    let sortie = execution::executer(exe, &mediainfo::arguments(chemin), delai)?;
    mediainfo::analyser_sortie(&sortie, extension)
}
