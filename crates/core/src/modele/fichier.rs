//! Fichiers média et flux (audio, vidéo, sous-titres).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Type d'un fichier média. **L'ordre des variantes est significatif** : il
/// sert de clé de tri (vidéos d'abord, puis bandes-annonces, etc.).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum TypeFichier {
    Video,
    Trailer,
    Sample,
    /// Obsolète, conservé pour la compatibilité.
    VideoExtra,
    Audio,
    SousTitres,
    Nfo,
    Poster,
    Fanart,
    Banner,
    Clearart,
    Disc,
    /// Obsolète : remplacé par `Clearlogo`.
    Logo,
    Clearlogo,
    Thumb,
    Characterart,
    Keyart,
    Squareart,
    SaisonPoster,
    SaisonFanart,
    SaisonBanner,
    SaisonThumb,
    ExtraFanart,
    ExtraThumb,
    Extra,
    Bif,
    /// Image non classée.
    Graphic,
    /// Fichier `*-mediainfo.xml`.
    MediaInfo,
    /// Métadonnées Synology.
    Vsmeta,
    Theme,
    /// `*.skiptro.json`
    Skiptro,
    /// `*.sp.json`, `*.sp.hash.json`
    Scenepass,
    Texte,
    /// Fichier inconnu dont le nom commence par celui d'une vidéo + extension.
    DoubleExt,
    #[default]
    Inconnu,
}

impl TypeFichier {
    /// Types d'images.
    pub fn est_graphique(self) -> bool {
        use TypeFichier::*;
        matches!(
            self,
            Graphic
                | Banner
                | Fanart
                | Poster
                | Thumb
                | Logo
                | Clearlogo
                | Clearart
                | Characterart
                | Keyart
                | Squareart
                | SaisonPoster
                | SaisonFanart
                | SaisonBanner
                | SaisonThumb
                | ExtraFanart
                | ExtraThumb
                | Disc
        )
    }

    /// Types vidéo (hors `Extra`, dont le statut dépend de l'extension : voir
    /// [`FichierMedia::est_video`]).
    pub fn est_type_video(self) -> bool {
        matches!(
            self,
            TypeFichier::Video
                | TypeFichier::Trailer
                | TypeFichier::Sample
                | TypeFichier::VideoExtra
        )
    }
}

/// Drapeaux d'un flux audio ou de sous-titres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrapeauFlux {
    Defaut,
    Doublage,
    Original,
    Commentaire,
    Paroles,
    Karaoke,
    Force,
    Malentendants,
    Malvoyants,
}

/// Flux audio d'un fichier.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FluxAudio {
    pub codec: String,
    pub id_codec: String,
    pub langue: String,
    pub titre: String,
    pub drapeaux: Vec<DrapeauFlux>,
    pub canaux: u32,
    pub debit: u32,
    pub profondeur_bits: u32,
    /// Configuration des canaux (ex. `3/2/0.1`) si connue.
    pub configuration_canaux: String,
}

/// Flux de sous-titres d'un fichier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FluxSousTitres {
    pub codec: String,
    pub langue: String,
    pub titre: String,
    pub drapeaux: Vec<DrapeauFlux>,
}

/// Flux vidéo d'un fichier (informations techniques principales).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FluxVideo {
    pub codec: String,
    pub id_codec: String,
    pub format_exact: String,
    pub format_3d: String,
    pub largeur: u32,
    pub hauteur: u32,
    pub ratio_aspect: f32,
    pub ratio_aspect2: Option<f32>,
    pub ratio_pixel: f32,
    pub debit: u32,
    pub profondeur_bits: u32,
    pub images_par_seconde: f64,
    pub format_hdr: String,
}

/// Fichier média associé à un film ou à un épisode.
///
/// Identité (égalité, tri, hachage) : chemin absolu complet `chemin/nom`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FichierMedia {
    pub type_fichier: TypeFichier,
    /// Dossier contenant le fichier.
    pub chemin: PathBuf,
    /// Nom du fichier (avec extension).
    pub nom: String,
    /// Taille en octets.
    pub taille: u64,
    /// Date de dernière modification (ms depuis l'époque Unix).
    pub date_fichier: i64,
    pub format_conteneur: String,
    pub video: FluxVideo,
    pub debit_global: u32,
    pub duree_secondes: u32,
    /// Numéro de pile (CD1 = 1, CD2 = 2…), 0 = aucune.
    pub empilement: u32,
    /// Marqueur de pile trouvé dans le nom (`CD1`, `part2`, `a`…).
    pub marqueur_empilement: String,
    pub titre: String,
    pub est_iso: bool,
    pub est_graphique_anime: bool,
    pub sommes_controle: BTreeMap<String, String>,
    pub flux_audio: Vec<FluxAudio>,
    pub sous_titres: Vec<FluxSousTitres>,
}

impl PartialEq for FichierMedia {
    fn eq(&self, autre: &Self) -> bool {
        self.chemin_complet() == autre.chemin_complet()
    }
}
impl Eq for FichierMedia {}

impl std::hash::Hash for FichierMedia {
    fn hash<H: std::hash::Hasher>(&self, etat: &mut H) {
        self.chemin_complet().hash(etat);
    }
}

impl PartialOrd for FichierMedia {
    fn partial_cmp(&self, autre: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(autre))
    }
}
impl Ord for FichierMedia {
    fn cmp(&self, autre: &Self) -> std::cmp::Ordering {
        self.chemin_complet().cmp(&autre.chemin_complet())
    }
}

impl FichierMedia {
    /// Crée un fichier sans toucher au disque.
    pub fn nouveau(chemin_complet: &Path, type_fichier: TypeFichier) -> Self {
        let nom = chemin_complet
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let chemin = chemin_complet
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let ext = extension_minuscule(&nom);
        Self {
            type_fichier,
            chemin,
            nom,
            est_iso: ext == "iso",
            format_conteneur: if ext.is_empty() { String::new() } else { ext },
            ..Self::default()
        }
    }

    /// Chemin absolu complet.
    pub fn chemin_complet(&self) -> PathBuf {
        self.chemin.join(&self.nom)
    }

    /// Extension en minuscules, sans point.
    pub fn extension(&self) -> String {
        extension_minuscule(&self.nom)
    }

    /// Nom sans extension.
    pub fn nom_de_base(&self) -> String {
        nom_de_base(&self.nom)
    }

    /// Vidéo au sens large : types vidéo, ou `Extra` d'extension vidéo.
    pub fn est_video(&self) -> bool {
        self.type_fichier.est_type_video()
            || (self.type_fichier == TypeFichier::Extra
                && crate::scan::extensions::est_video(&self.extension()))
    }

    /// Archive compressée (zip, rar, 7z, `r01`…).
    pub fn est_archive(&self) -> bool {
        let ext = self.extension();
        matches!(ext.as_str(), "zip" | "rar" | "7z")
            || (ext.len() > 1
                && ext.starts_with('r')
                && ext[1..].chars().all(|c| c.is_ascii_digit()))
    }
}

/// Extension en minuscules, sans point (vide si aucune).
pub fn extension_minuscule(nom: &str) -> String {
    match nom.rsplit_once('.') {
        Some((base, ext)) if !base.is_empty() => ext.to_lowercase(),
        _ => String::new(),
    }
}

/// Nom de fichier sans sa dernière extension.
pub fn nom_de_base(nom: &str) -> String {
    match nom.rsplit_once('.') {
        Some((base, _)) if !base.is_empty() => base.to_string(),
        _ => nom.to_string(),
    }
}

/// Comparateur de tri des fichiers d'un film : d'abord l'ordre du type, puis
/// nom croissant pour VIDEO/TRAILER/EXTRAFANART/EXTRATHUMB, longueur de nom
/// décroissante pour les autres types (les noms longs d'abord, afin que
/// `Film.fr.srt` précède `Film.srt` lors du rattachement).
pub fn comparer_fichiers_film(a: &FichierMedia, b: &FichierMedia) -> std::cmp::Ordering {
    use TypeFichier::*;
    a.type_fichier
        .cmp(&b.type_fichier)
        .then_with(|| match a.type_fichier {
            Video | Trailer | ExtraFanart | ExtraThumb => a.nom.cmp(&b.nom),
            _ => b
                .nom
                .chars()
                .count()
                .cmp(&a.nom.chars().count())
                .then_with(|| a.nom.cmp(&b.nom)),
        })
}
