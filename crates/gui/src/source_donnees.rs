//! Frontière d'intégration : tout ce que l'interface attend des services de l'application
//! (bibliothèque, images, recherche, tâches, renommage, réglages, coffre à secrets).
//!
//! Deux implémentations :
//! - [`SourceApplication`](crate::source_application::SourceApplication) : services réels
//!   (`mediamanager_app::Application`) ;
//! - [`SourceFactice`] : démonstration et tests visuels (données de `donnees.rs`, images
//!   générées localement, recherche et tâches simulées).

use crate::details_film::DetailsFilm;
use crate::details_serie::DetailsSerie;
use crate::dialogues::appliquer_resultat;
use crate::donnees::films_factices;
use crate::etat::{Message, Niveau, NoeudSerie};
use crate::modele::{Film, normaliser};
use crate::renommage::{self, ApercuFilm, LigneFichier};
use crate::taches::{IssueTache, Repaint};
use mediamanager_store::reglages::{
    QualiteCacheImages, ReglagesFilms, ReglagesGeneraux, ReglagesSeries, TailleCacheImages,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Nom du secret de la clé d'API TMDB transmis par l'interface.
pub const SECRET_CLE_TMDB: &str = "tmdb.cle_api";

/// Types d'illustration affichés par le panneau de détails.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GenreImage {
    Affiche,
    Fanart,
    Banniere,
    Miniature,
}

impl GenreImage {
    pub const TOUS: [GenreImage; 4] = [
        GenreImage::Affiche,
        GenreImage::Fanart,
        GenreImage::Banniere,
        GenreImage::Miniature,
    ];

    pub fn cle(self) -> &'static str {
        match self {
            GenreImage::Affiche => "gui.artwork.poster",
            GenreImage::Fanart => "gui.artwork.fanart",
            GenreImage::Banniere => "gui.artwork.banner",
            GenreImage::Miniature => "gui.artwork.thumb",
        }
    }

    /// Rang dans [`GenreImage::TOUS`].
    pub fn rang(self) -> usize {
        self as usize
    }

    fn suffixe(self) -> &'static str {
        match self {
            GenreImage::Affiche => "affiche",
            GenreImage::Fanart => "fanart",
            GenreImage::Banniere => "banniere",
            GenreImage::Miniature => "miniature",
        }
    }

    /// Dimensions des images de démonstration.
    fn dimensions_demo(self) -> (u32, u32) {
        match self {
            GenreImage::Affiche => (200, 300),
            GenreImage::Fanart => (480, 270),
            GenreImage::Banniere => (500, 92),
            GenreImage::Miniature => (320, 180),
        }
    }
}

/// Paramètres d'une recherche d'extracteur (dialogue de choix d'un résultat).
#[derive(Clone, Debug, PartialEq)]
pub struct RequeteRecherche {
    /// Film pour lequel on cherche.
    pub film: u64,
    pub extracteur: String,
    pub langue: String,
    pub texte: String,
    pub annee: Option<u32>,
}

/// Un résultat de recherche tel qu'affiché par le dialogue.
#[derive(Clone, Debug, PartialEq)]
pub struct ResultatRecherche {
    pub id: String,
    pub titre: String,
    pub titre_original: String,
    pub annee: u32,
    /// Score de similarité dans [0, 1].
    pub score: f32,
    pub resume: String,
    pub affiche: Option<PathBuf>,
}

/// Canal de retour vers l'interface : messages appliqués par l'état à la prochaine image.
#[derive(Clone)]
pub struct Retour {
    pub tx: Sender<Message>,
    pub repaint: Repaint,
}

impl Retour {
    /// Envoie un message et réveille l'interface ; `false` si l'interface a disparu.
    pub fn envoyer(&self, m: Message) -> bool {
        let ok = self.tx.send(m).is_ok();
        (self.repaint)();
        ok
    }
}

/// Traitements longs demandés par l'interface (exécutés en tâches par la source).
#[derive(Clone, Debug, PartialEq)]
pub enum Travail {
    /// Toutes les sources : films puis séries.
    MettreAJourSources,
    /// Sources de séries uniquement.
    MettreAJourSeries,
    /// Mise à jour limitée aux dossiers des films choisis.
    MettreAJourSelection {
        ids: Vec<u64>,
    },
    /// Récupération automatique (meilleur résultat) ; `artwork` = images comprises.
    RecupererAuto {
        ids: Vec<u64>,
        artwork: bool,
    },
    /// Résultat choisi dans le dialogue de recherche.
    AppliquerResultat {
        film: u64,
        resultat: ResultatRecherche,
        ne_pas_remplacer: bool,
    },
    EcrireNfo {
        ids: Vec<u64>,
    },
    Renommer {
        ids: Vec<u64>,
    },
    /// Annule le dernier renommage des films choisis.
    AnnulerRenommage {
        ids: Vec<u64>,
    },
    /// Analyse des flux (ffprobe, MediaInfo) des fichiers des films et des séries choisis.
    AnalyserMedias {
        films: Vec<u64>,
        series: Vec<String>,
    },
    /// Récupération automatique pour des séries ; `artwork` = images comprises.
    RecupererSeries {
        ids: Vec<String>,
        artwork: bool,
    },
    /// Résultat choisi dans le dialogue de recherche d'une série.
    AppliquerResultatSerie {
        serie: String,
        resultat: ResultatRecherche,
        ne_pas_remplacer: bool,
    },
    EcrireNfoSeries {
        ids: Vec<String>,
    },
    RenommerSeries {
        ids: Vec<String>,
    },
    /// Export de la liste (modèle, dossier de sortie, vignettes).
    Exporter {
        cible: CibleExport,
        /// Identifiant du modèle (`ModeleExportAffiche::id`).
        modele: String,
        dossier: PathBuf,
        /// `Some(largeur)` : copie des affiches (0 = taille d'origine).
        vignettes: Option<u32>,
        titre: Option<String>,
    },
    /// Déplace des fichiers inutiles dans la corbeille de leur source.
    Nettoyer {
        liste: Vec<FichierInutileAffiche>,
    },
    /// Rafraîchit les notes des films choisis.
    RecupererEvaluations {
        ids: Vec<u64>,
    },
    /// Édition globale de films.
    EditionLotFilms {
        ids: Vec<u64>,
        modifs: ModifsLot,
    },
    /// Édition globale de séries.
    EditionLotSeries {
        ids: Vec<String>,
        modifs: ModifsLot,
    },
}

/// Éléments d'un export : films (identifiants affichés) ou séries ; `None` = tous.
#[derive(Clone, Debug, PartialEq)]
pub enum CibleExport {
    Films(Option<Vec<u64>>),
    Series(Option<Vec<String>>),
}

/// Modèle d'export proposé par le dialogue.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModeleExportAffiche {
    pub id: String,
    pub nom: String,
    pub description: String,
    /// Nom du fichier de liste, sans extension.
    pub nom_fichier: String,
    pub extension: String,
}

/// Bilan d'un export, tel qu'affiché à la fin du dialogue.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RapportExportAffiche {
    /// Fichier de liste produit.
    pub fichier_liste: PathBuf,
    pub exportes: usize,
    pub vignettes: usize,
    pub avertissements: Vec<String>,
}

impl RapportExportAffiche {
    /// Dossier contenant le fichier de liste.
    pub fn dossier(&self) -> PathBuf {
        self.fichier_liste
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }
}

/// Pourquoi un fichier est jugé inutile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RaisonNettoyage {
    /// Nom conforme à un motif des réglages.
    Motif(String),
    Parasite,
    Extrait,
    NfoOrphelin,
}

/// Fichier ou dossier inutile d'un film, proposé au nettoyage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FichierInutileAffiche {
    /// Film concerné (identifiant affiché).
    pub id_film: u64,
    pub titre_film: String,
    pub chemin: String,
    pub est_dossier: bool,
    pub raison: RaisonNettoyage,
    pub taille: u64,
}

/// Éditions proposées par l'édition globale (libellés techniques, non traduits).
pub const EDITIONS: [&str; 13] = [
    "Director's Cut",
    "Extended Edition",
    "Theatrical Edition",
    "Unrated",
    "Uncut",
    "IMAX",
    "Remastered",
    "Collectors Edition",
    "Ultimate Edition",
    "Final Edition",
    "Special Edition",
    "Criterion Collection",
    "Open Matte",
];

/// Sources médias proposées par l'édition globale : (code, libellé).
pub const SOURCES_MEDIA: [(&str, &str); 17] = [
    ("UHD_BLURAY", "UHD Blu-ray"),
    ("BLURAY", "Blu-ray"),
    ("DVD", "DVD"),
    ("HDDVD", "HD DVD"),
    ("TV", "TV"),
    ("VHS", "VHS"),
    ("LASERDISC", "LaserDisc"),
    ("D_VHS", "D-VHS"),
    ("HDRIP", "HDRip"),
    ("CAM", "CAM"),
    ("TS", "TS"),
    ("TC", "TC"),
    ("DVDSCR", "DVDSCR"),
    ("R5", "R5"),
    ("WEBRIP", "WEBRip"),
    ("WEB_DL", "WEB-DL"),
    ("STREAM", "Stream"),
];

/// Modifications appliquées à un lot de films ou de séries. Une liste vide ou un `None`
/// laisse la valeur existante intacte.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModifsLot {
    pub genres_ajouter: Vec<String>,
    pub genres_retirer: Vec<String>,
    pub tags_ajouter: Vec<String>,
    pub tags_retirer: Vec<String>,
    pub certification: Option<String>,
    pub studio: Option<String>,
    pub pays: Option<String>,
    /// Langue d'origine : codes ISO 639-1 séparés par une virgule (films).
    pub langue: Option<String>,
    /// Libellé d'une entrée de [`EDITIONS`] (films).
    pub edition: Option<String>,
    /// Code d'une entrée de [`SOURCES_MEDIA`] (films).
    pub source_media: Option<String>,
    /// Marque vu / non vu (films).
    pub vu: Option<bool>,
    /// Note personnelle (texte libre).
    pub note: Option<String>,
}

impl ModifsLot {
    /// Aucune modification demandée.
    pub fn est_vide(&self) -> bool {
        *self == Self::default()
    }
}

impl Travail {
    /// Clé de traduction du nom de la tâche.
    pub fn cle_nom(&self) -> &'static str {
        match self {
            Travail::MettreAJourSources => "gui.task.update",
            Travail::MettreAJourSeries => "gui.task.updatetv",
            Travail::MettreAJourSelection { .. } => "gui.task.updateselection",
            Travail::RecupererAuto { .. } | Travail::AppliquerResultat { .. } => "gui.task.scrape",
            Travail::RecupererSeries { .. } | Travail::AppliquerResultatSerie { .. } => {
                "gui.task.scrapetv"
            }
            Travail::EcrireNfo { .. } => "gui.task.nfo",
            Travail::EcrireNfoSeries { .. } => "gui.task.nfotv",
            Travail::Renommer { .. } => "gui.task.rename",
            Travail::RenommerSeries { .. } => "gui.task.renametv",
            Travail::AnnulerRenommage { .. } => "gui.task.undorename",
            Travail::AnalyserMedias { .. } => "gui.task.mediainfo",
            Travail::Exporter { .. } => "gui.task.export",
            Travail::Nettoyer { .. } => "gui.task.cleanup",
            Travail::RecupererEvaluations { .. } => "gui.task.ratings",
            Travail::EditionLotFilms { .. } | Travail::EditionLotSeries { .. } => {
                "gui.task.bulkedit"
            }
        }
    }
}

/// Réglages des services modifiables depuis l'interface (miroir d'une partie du fichier de
/// réglages du stockage ; les secrets restent dans le coffre).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReglagesServices {
    pub sources_films: Vec<String>,
    pub sources_series: Vec<String>,
    /// Dossiers ignorés par le scan des films.
    pub dossiers_ignores: Vec<String>,
    pub langue_scraping: String,
    pub gabarit_dossier_film: String,
    pub gabarit_fichier_film: String,
    pub remplacer_espaces: bool,
    pub caractere_espace: String,
    pub ascii_seulement: bool,
    pub caractere_remplacement: String,
    pub supprimer_dossiers_vides: bool,
    pub renommer_apres_scraping: bool,
    pub proxy_hote: String,
    pub proxy_port: String,
    pub proxy_utilisateur: String,
    /// Secret : confié au coffre à l'enregistrement, jamais écrit dans `ui.json`.
    pub proxy_mot_de_passe: String,
    pub ignorer_erreurs_ssl: bool,
    pub threads_telechargement: u32,
    pub corbeille: bool,
    pub cache_images: bool,
    /// Chemin de ffprobe / MediaInfo (fichier ou dossier) ; vide = recherche automatique
    /// (dossier `outils/` de l'application, puis `PATH`).
    pub chemin_outils: String,
    pub gabarit_dossier_serie: String,
    pub gabarit_dossier_saison: String,
    pub gabarit_fichier_episode: String,
    /// Épisodes scrapés sans fichier local affichés dans l'arbre des séries.
    pub afficher_episodes_manquants: bool,
    pub afficher_speciaux_manquants: bool,
    pub afficher_non_diffuses: bool,
    /// Réglages complets des films du stockage. Les champs déjà portés par les champs
    /// ci-dessus (sources, dossiers ignorés, langue, renommage automatique) y sont ignorés à
    /// l'enregistrement : les champs plats font foi.
    pub films: ReglagesFilms,
    /// Réglages complets des séries (mêmes règles : sources et affichage des épisodes
    /// manquants sont portés par les champs plats).
    pub series: ReglagesSeries,
    pub generales: OptionsGenerales,
}

/// Réglages généraux modifiables depuis l'interface, hors réseau et outils d'analyse
/// (déjà portés par [`ReglagesServices`]). Aucun secret : la clé d'API n'y figure que sous
/// forme d'un indicateur de présence.
#[derive(Clone, Debug, PartialEq)]
pub struct OptionsGenerales {
    pub extensions_video: Vec<String>,
    pub extensions_audio: Vec<String>,
    pub extensions_sous_titres: Vec<String>,
    /// Expressions régulières des fichiers indésirables.
    pub motifs_nettoyage: Vec<String>,
    /// Articles déplacés en fin de titre pour le tri.
    pub prefixes_titre: Vec<String>,
    pub chemin_yt_dlp: String,
    pub lecteur_media: String,
    pub vider_corbeille_a_la_sortie: bool,
    pub taille_cache_images: TailleCacheImages,
    pub qualite_cache_images: QualiteCacheImages,
    /// Une clé d'API TMDB est enregistrée dans le coffre (information seule, non enregistrée).
    pub cle_tmdb_definie: bool,
}

impl Default for OptionsGenerales {
    fn default() -> Self {
        crate::conversion::generales(&ReglagesGeneraux::default())
    }
}

/// Épisode affiché dans l'arbre des séries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EpisodeAffichage {
    pub episode: i32,
    pub titre: String,
    pub fichier: String,
    pub vu: bool,
    /// Épisode connu du fournisseur mais sans fichier local.
    pub manquant: bool,
    /// Résumé technique du fichier (« 1080p H264 · AC3 6ch · 44 min ») ; vide sans analyse.
    pub infos_media: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SaisonAffichage {
    pub numero: i32,
    pub titre: String,
    pub episodes: Vec<EpisodeAffichage>,
}

/// Statut de diffusion d'une série.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatutSerie {
    #[default]
    Inconnu,
    EnCours,
    Terminee,
    Annulee,
}

impl StatutSerie {
    pub const TOUS: [StatutSerie; 4] = [
        StatutSerie::Inconnu,
        StatutSerie::EnCours,
        StatutSerie::Terminee,
        StatutSerie::Annulee,
    ];

    /// Clé de traduction du libellé.
    pub fn cle(self) -> &'static str {
        match self {
            StatutSerie::Inconnu => "gui.series.status.unknown",
            StatutSerie::EnCours => "gui.series.status.running",
            StatutSerie::Terminee => "gui.series.status.ended",
            StatutSerie::Annulee => "gui.series.status.cancelled",
        }
    }
}

/// Série affichée (arbre série → saisons → épisodes) avec les champs modifiables par
/// l'éditeur de série.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SerieAffichage {
    pub id: String,
    pub titre: String,
    pub titre_original: String,
    pub annee: u32,
    pub chemin: String,
    pub resume: String,
    pub genres: Vec<String>,
    pub statut: StatutSerie,
    /// Note sur 10 (0 = absente).
    pub note: f32,
    pub imdb: String,
    pub tmdb: String,
    pub tvdb: String,
    pub verrouillee: bool,
    pub saisons: Vec<SaisonAffichage>,
}

impl SerieAffichage {
    /// Épisodes présents sur le disque.
    pub fn nombre_episodes(&self) -> usize {
        self.saisons
            .iter()
            .flat_map(|s| &s.episodes)
            .filter(|e| !e.manquant)
            .count()
    }

    /// Épisodes connus du fournisseur mais absents du disque (affichés selon les réglages).
    pub fn nombre_manquants(&self) -> usize {
        self.saisons
            .iter()
            .flat_map(|s| &s.episodes)
            .filter(|e| e.manquant)
            .count()
    }

    /// Métadonnées déjà récupérées auprès d'un fournisseur.
    pub fn traitee(&self) -> bool {
        !self.tmdb.is_empty() || !self.tvdb.is_empty() || !self.imdb.is_empty()
    }
}

/// Recherche d'une série (dialogue de choix d'un résultat).
#[derive(Clone, Debug, PartialEq)]
pub struct RequeteRechercheSerie {
    /// Identifiant de la série cherchée.
    pub serie: String,
    pub texte: String,
    pub annee: Option<u32>,
    pub langue: String,
}

/// Élément absent de la bibliothèque (partie d'une saga, épisode).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ElementManquant {
    pub titre: String,
    pub titre_original: String,
    /// Année (parties de saga) ou « S01E03 » (épisodes).
    pub detail: String,
    pub resume: String,
}

/// Éléments manquants d'une saga ou d'une série.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GroupeManquants {
    pub titre: String,
    pub elements: Vec<ElementManquant>,
}

/// Un outil d'analyse localisé.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutilAffiche {
    pub chemin: String,
    /// Version annoncée par l'outil ; `None` s'il ne répond pas.
    pub version: Option<String>,
}

/// Résultat du test des outils d'analyse (réglage « Chemin de ffprobe / MediaInfo »).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiagnosticOutils {
    pub ffprobe: Option<OutilAffiche>,
    pub mediainfo: Option<OutilAffiche>,
}

impl DiagnosticOutils {
    pub fn disponible(&self) -> bool {
        self.ffprobe.is_some() || self.mediainfo.is_some()
    }
}

/// Saga (collection de films) affichée par le module Collections.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SagaAffichage {
    pub id: String,
    pub titre: String,
    pub titre_original: String,
    pub resume: String,
    /// Identifiants affichés des films de la saga (ceux absents de la bibliothèque sont ignorés).
    pub films: Vec<u64>,
    /// Illustrations locales existantes de la saga.
    pub affiche: Option<PathBuf>,
    pub fanart: Option<PathBuf>,
    pub banniere: Option<PathBuf>,
}

/// Issue de la création d'un film hors ligne.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreationHorsLigne {
    /// Le film est déjà dans la bibliothèque.
    Ajoute,
    /// Le dossier et son marqueur `.disc` sont créés : une mise à jour des sources les importe.
    AAnalyser,
}

/// Nom de dossier d'un film hors ligne : « Titre (année) », sans caractère interdit dans un nom
/// de fichier ; `None` si le titre ne laisse rien d'utilisable.
pub fn nom_film_hors_ligne(titre: &str, annee: u32) -> Option<String> {
    let propre: String = titre
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let propre = propre.trim().trim_matches('.').trim().to_string();
    if propre.is_empty() {
        return None;
    }
    Some(if annee > 0 {
        format!("{propre} ({annee})")
    } else {
        propre
    })
}

/// Crée, dans `source`, le dossier « Titre (année) » et le marqueur vide `Titre (année).disc`
/// d'un film hors ligne (il est importé par la mise à jour des sources). Renvoie le dossier.
pub fn creer_marqueur_hors_ligne(
    source: &Path,
    titre: &str,
    annee: u32,
) -> Result<PathBuf, String> {
    let nom = nom_film_hors_ligne(titre, annee).ok_or_else(|| "titre invalide".to_string())?;
    if !source.is_dir() {
        return Err(format!("dossier source introuvable : {}", source.display()));
    }
    let dossier = source.join(&nom);
    if dossier.exists() {
        return Err(format!("{} existe déjà", dossier.display()));
    }
    std::fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::write(dossier.join(format!("{nom}.disc")), b"") {
        let _ = std::fs::remove_dir(&dossier);
        return Err(e.to_string());
    }
    Ok(dossier)
}

/// Ce que l'interface attend des services. Les méthodes bloquantes (`rechercher`) sont
/// appelées hors du thread de l'interface ; les autres sont courtes.
pub trait SourceDonnees: Send + Sync {
    /// Instantané de la bibliothèque de films.
    fn films(&self) -> Vec<Film>;
    /// Instantané des séries.
    fn series(&self) -> Vec<SerieAffichage>;
    /// Instantané des sagas (par défaut : aucune).
    fn sagas(&self) -> Vec<SagaAffichage> {
        Vec::new()
    }
    /// Chemin local d'une illustration du film, s'il existe.
    fn chemin_image(&self, film: &Film, genre: GenreImage) -> Option<PathBuf>;
    /// Recherche auprès d'un extracteur (bloquant).
    fn rechercher(&self, requete: &RequeteRecherche) -> Result<Vec<ResultatRecherche>, String>;
    /// Extracteurs proposés dans le dialogue de recherche.
    fn extracteurs(&self) -> Vec<String> {
        vec!["TMDB".into()]
    }
    /// Écrit des films modifiés dans l'interface (base, puis NFO en tâche).
    fn enregistrer_films(&self, films: &[Film]) -> Result<(), String>;
    /// Données de la fenêtre « Éditer un film » qui ne figurent pas dans [`Film`] (notes par
    /// source, identifiants, équipe, illustrations, flux...). Par défaut : déduites du film.
    fn details_film(&self, film: &Film) -> DetailsFilm {
        DetailsFilm::depuis_film(film)
    }
    /// Enregistre un film édité avec ses détails (base, puis NFO en tâche). Par défaut, seuls
    /// les champs du film d'affichage sont écrits.
    fn enregistrer_film_detaille(&self, film: &Film, details: &DetailsFilm) -> Result<(), String> {
        let _ = details;
        self.enregistrer_films(std::slice::from_ref(film))
    }
    /// Crée un film hors ligne (sans fichier vidéo) dans la source de films `source`. Par défaut :
    /// dossier et marqueur `.disc` sur le disque, importés par la mise à jour des sources.
    fn creer_film_hors_ligne(
        &self,
        source: &str,
        titre: &str,
        annee: u32,
    ) -> Result<CreationHorsLigne, String> {
        creer_marqueur_hors_ligne(Path::new(source), titre, annee)?;
        Ok(CreationHorsLigne::AAnalyser)
    }
    /// Retire des films de la base ; avec `deplacer_fichiers`, leurs fichiers sont déplacés
    /// dans la corbeille de leur source (`.corbeille`). Renvoie les films retirés
    /// immédiatement ; ceux qui le sont par une tâche de fond arrivent par `FilmRetire`.
    fn retirer_films(&self, ids: &[u64], deplacer_fichiers: bool) -> Result<Vec<u64>, String>;
    /// Transmet un secret (clé d'API...) au coffre à secrets. Jamais écrit dans `ui.json`.
    fn transmettre_secret(&self, nom: &str, valeur: &str) -> Result<(), String>;
    /// Réglages courants des services.
    fn reglages_services(&self) -> ReglagesServices;
    /// Enregistre les réglages des services.
    fn enregistrer_reglages_services(&self, r: &ReglagesServices) -> Result<(), String>;
    /// Plan de renommage d'une sélection (rien n'est écrit).
    fn apercu_renommage(&self, ids: &[u64]) -> Result<Vec<ApercuFilm>, String>;
    /// Plan de renommage de séries et de leurs épisodes (un aperçu par élément du plan ;
    /// l'identifiant d'un aperçu n'est que son rang).
    fn apercu_renommage_series(&self, ids: &[String]) -> Result<Vec<ApercuFilm>, String>;
    /// Recherche d'une série auprès du fournisseur (bloquant).
    fn rechercher_serie(
        &self,
        requete: &RequeteRechercheSerie,
    ) -> Result<Vec<ResultatRecherche>, String>;
    /// Écrit une série modifiée dans l'éditeur (base, puis NFO en tâche).
    fn enregistrer_serie(&self, serie: &SerieAffichage) -> Result<(), String>;
    /// Données de la fenêtre « Éditer une série » qui ne figurent pas dans [`SerieAffichage`]
    /// (titres, notes par source, identifiants, distribution, illustrations...). Par défaut :
    /// déduites de la série.
    fn details_serie(&self, serie: &SerieAffichage) -> DetailsSerie {
        DetailsSerie::depuis_serie(serie)
    }
    /// Enregistre une série éditée avec ses détails (base, puis NFO en tâche). Par défaut, seuls
    /// les champs de la série d'affichage sont écrits.
    fn enregistrer_serie_detaillee(
        &self,
        serie: &SerieAffichage,
        details: &DetailsSerie,
    ) -> Result<(), String> {
        let _ = details;
        self.enregistrer_serie(serie)
    }
    /// Films de la saga présents chez le fournisseur mais absents de la bibliothèque
    /// (bloquant : appelé hors du thread de l'interface).
    fn films_manquants(&self, saga: &str) -> Result<Vec<ElementManquant>, String>;
    /// Épisodes connus du fournisseur mais sans fichier local.
    fn episodes_manquants(&self, serie: &str) -> Result<Vec<ElementManquant>, String>;
    /// Localise ffprobe / MediaInfo pour un chemin donné (vide = recherche automatique) et
    /// interroge leur version (bloquant : quelques secondes au plus).
    fn tester_outils(&self, chemin: &str) -> Result<DiagnosticOutils, String>;
    /// Modèles d'export disponibles pour les films ou, avec `series`, pour les séries.
    fn modeles_export(&self, series: bool) -> Vec<ModeleExportAffiche>;
    /// Fichiers inutiles des dossiers des films choisis (lecture seule ; bloquant : appelé
    /// hors du thread de l'interface).
    fn analyser_nettoyage(&self, ids: &[u64]) -> Result<Vec<FichierInutileAffiche>, String>;
    /// Marque vu / non vu les séries, saisons ou épisodes désignés : tout devient vu, sauf si
    /// tout l'était déjà.
    fn basculer_vu_series(&self, cibles: &[NoeudSerie]) -> Result<(), String>;
    /// Retire des séries de la base ; avec `deplacer_fichiers`, leurs fichiers vont dans la
    /// corbeille de leur source. Renvoie les séries retirées immédiatement ; celles qui le
    /// sont par une tâche de fond arrivent par `SeriesModifiees`.
    fn retirer_series(
        &self,
        ids: &[String],
        deplacer_fichiers: bool,
    ) -> Result<Vec<String>, String>;
    /// Lance un traitement long ; son cycle de vie arrive par `retour` (ou par le canal
    /// installé par [`brancher`](Self::brancher)).
    fn lancer(&self, travail: Travail, retour: &Retour) -> Result<(), String>;
    /// Demande l'annulation d'une tâche.
    fn annuler_tache(&self, id: u64);
    /// Installe le canal de retour des événements spontanés des services.
    fn brancher(&self, _retour: Retour) {}
}

/// Similarité de deux titres : coefficient de Dice sur les bigrammes de caractères normalisés.
pub fn similarite(a: &str, b: &str) -> f32 {
    let nettoyer = |s: &str| -> Vec<char> {
        normaliser(s)
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect()
    };
    let (a, b) = (nettoyer(a), nettoyer(b));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    let bigrammes =
        |v: &[char]| -> Vec<(char, char)> { v.windows(2).map(|w| (w[0], w[1])).collect() };
    let (ba, mut bb) = (bigrammes(&a), bigrammes(&b));
    if ba.is_empty() || bb.is_empty() {
        return 0.0;
    }
    let total = (ba.len() + bb.len()) as f32;
    let mut communs = 0usize;
    for g in &ba {
        if let Some(p) = bb.iter().position(|x| x == g) {
            bb.swap_remove(p);
            communs += 1;
        }
    }
    2.0 * communs as f32 / total
}

/// Score d'un résultat : similarité du titre (meilleure entre titre et titre original), ajustée par l'année.
pub fn score_resultat(requete: &RequeteRecherche, titre: &str, original: &str, annee: u32) -> f32 {
    let base = similarite(&requete.texte, titre).max(similarite(&requete.texte, original));
    let ajuste = match requete.annee {
        Some(a) if a == annee => base + 0.1,
        Some(a) if a.abs_diff(annee) > 1 => base - 0.1,
        _ => base,
    };
    ajuste.clamp(0.0, 1.0)
}

/// Titre « nettoyé » d'un nom de dossier/fichier : séparateurs remplacés, année et balises retirées.
pub fn titre_nettoye(nom: &str) -> (String, Option<u32>) {
    let sans_ext = match nom.rsplit_once('.') {
        Some((base, ext)) if ext.len() <= 4 && ext.chars().all(|c| c.is_ascii_alphanumeric()) => {
            base
        }
        _ => nom,
    };
    let jetons: Vec<&str> = sans_ext
        .split(['.', '_', ' ', '(', ')', '[', ']'])
        .filter(|j| !j.is_empty())
        .collect();
    // Les balises techniques terminent le titre.
    let fin = jetons
        .iter()
        .position(|j| {
            matches!(
                j.to_lowercase().as_str(),
                "1080p" | "720p" | "2160p" | "480p" | "bluray" | "x264" | "x265" | "hevc" | "4k"
            )
        })
        .unwrap_or(jetons.len());
    let est_annee =
        |j: &str| j.len() == 4 && j.parse::<u32>().is_ok_and(|a| (1880..=2100).contains(&a));
    // La DERNIÈRE année (hors premier mot) est celle du film : « Blade Runner 2049 2017 ».
    match (1..fin).rev().find(|&n| est_annee(jetons[n])) {
        Some(n) => (jetons[..n].join(" "), jetons[n].parse().ok()),
        None => (jetons[..fin].join(" "), None),
    }
}

/// Source factice : films de démonstration, images générées dans un dossier temporaire,
/// recherche et tâches simulées (les effets sont appliqués à la fin de la tâche).
pub struct SourceFactice {
    dossier_images: PathBuf,
    films: Arc<Mutex<Vec<Film>>>,
    series: Arc<Mutex<Vec<SerieAffichage>>>,
    services: Arc<Mutex<ReglagesServices>>,
    secrets: Mutex<HashMap<String, String>>,
    annulations: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>,
    /// Détails d'édition enregistrés par film.
    details: Mutex<HashMap<u64, DetailsFilm>>,
    /// Détails d'édition enregistrés par série.
    details_series: Mutex<HashMap<String, DetailsSerie>>,
    prochain_id: AtomicU64,
    /// Latence simulée de la recherche.
    pub latence: Duration,
    /// Durée d'une unité de travail des tâches simulées.
    pub pas: Duration,
}

impl Default for SourceFactice {
    fn default() -> Self {
        Self::new(std::env::temp_dir().join("mediamanagerrust-demo"))
    }
}

/// Réglages de démonstration.
pub fn reglages_demo() -> ReglagesServices {
    ReglagesServices {
        sources_films: vec!["D:\\Films".into(), "E:\\Films 4K".into()],
        sources_series: vec!["D:\\Séries".into()],
        dossiers_ignores: vec!["@eaDir".into(), "$RECYCLE.BIN".into()],
        langue_scraping: "fr".into(),
        gabarit_dossier_film: "${title} (${year})".into(),
        gabarit_fichier_film: "${title} (${year})".into(),
        caractere_espace: "_".into(),
        supprimer_dossiers_vides: true,
        threads_telechargement: 2,
        corbeille: true,
        cache_images: true,
        gabarit_dossier_serie: "${showTitle} (${showYear})".into(),
        gabarit_dossier_saison: "Saison ${seasonNr}".into(),
        gabarit_fichier_episode: "${showTitle} - S${seasonNr2}E${episodeNr2} - ${title}".into(),
        ..ReglagesServices::default()
    }
}

impl SourceFactice {
    pub fn new(dossier_images: PathBuf) -> Self {
        Self {
            dossier_images,
            films: Arc::new(Mutex::new(films_factices())),
            series: Arc::new(Mutex::new(series_demo())),
            services: Arc::new(Mutex::new(reglages_demo())),
            secrets: Mutex::new(HashMap::new()),
            annulations: Arc::default(),
            details: Mutex::new(HashMap::new()),
            details_series: Mutex::new(HashMap::new()),
            prochain_id: AtomicU64::new(1),
            latence: Duration::from_millis(250),
            pas: Duration::from_millis(80),
        }
    }

    /// Secret reçu (tests uniquement).
    pub fn secret(&self, nom: &str) -> Option<String> {
        self.secrets.lock().ok()?.get(nom).cloned()
    }

    fn catalogue() -> Vec<(String, String, u32, String)> {
        let mut v: Vec<(String, String, u32, String)> = films_factices()
            .into_iter()
            .map(|f| (f.titre, f.titre_original, f.annee, f.resume))
            .collect();
        for (t, o, a) in [
            ("Blade Runner", "Blade Runner", 1982),
            ("Alien, le retour", "Aliens", 1986),
            ("Alien³", "Alien 3", 1992),
            ("Dune : Deuxième partie", "Dune: Part Two", 2024),
            ("Matrix Reloaded", "The Matrix Reloaded", 2003),
            ("Mad Max", "Mad Max", 1979),
        ] {
            v.push((
                t.into(),
                o.into(),
                a,
                format!("Résumé factice de « {t} », renvoyé par la recherche de démonstration."),
            ));
        }
        v
    }

    fn image_demo(&self, cle: &str, genre: GenreImage, graine: u64) -> Option<PathBuf> {
        let chemin = self
            .dossier_images
            .join(format!("{cle}-{}.png", genre.suffixe()));
        if !chemin.exists() {
            let (l, h) = genre.dimensions_demo();
            generer_image_demo(&chemin, l, h, graine).ok()?;
        }
        Some(chemin)
    }
}

/// Ce qu'une tâche simulée change à sa fin.
#[derive(Default)]
struct Effets {
    films: Vec<Film>,
    series_modifiees: bool,
    notifications: Vec<(Niveau, String)>,
    messages: Vec<Message>,
}

/// Ajoute les valeurs absentes (sans tenir compte de la casse).
fn ajouter_valeurs(liste: &mut Vec<String>, valeurs: &[String]) -> bool {
    let mut change = false;
    for v in valeurs.iter().map(|v| v.trim()).filter(|v| !v.is_empty()) {
        if !liste.iter().any(|x| x.eq_ignore_ascii_case(v)) {
            liste.push(v.to_string());
            change = true;
        }
    }
    change
}

/// Retire les valeurs présentes (sans tenir compte de la casse).
fn retirer_valeurs(liste: &mut Vec<String>, valeurs: &[String]) -> bool {
    let avant = liste.len();
    liste.retain(|x| !valeurs.iter().any(|v| v.trim().eq_ignore_ascii_case(x)));
    liste.len() != avant
}

/// Applique une édition globale à un film affiché (seuls les champs que le modèle affiché
/// porte : genres, certification, production, pays, état « vu »). Vrai si le film a changé.
pub fn appliquer_lot_film(f: &mut Film, m: &ModifsLot) -> bool {
    let mut c = ajouter_valeurs(&mut f.genres, &m.genres_ajouter);
    c |= retirer_valeurs(&mut f.genres, &m.genres_retirer);
    if let Some(x) = &m.certification
        && *x != f.certification
    {
        f.certification = x.clone();
        c = true;
    }
    if let Some(x) = &m.studio {
        let v = vec![x.clone()];
        if v != f.production {
            f.production = v;
            c = true;
        }
    }
    if let Some(x) = &m.pays {
        let v: Vec<String> = x
            .split('/')
            .map(|p| p.trim().to_string())
            .filter(|p| !p.is_empty())
            .collect();
        if v != f.pays {
            f.pays = v;
            c = true;
        }
    }
    if let Some(vu) = m.vu
        && vu != f.vu
    {
        f.vu = vu;
        c = true;
    }
    c
}

/// Applique une édition globale à une série affichée (genres). Vrai si elle a changé.
pub fn appliquer_lot_serie(s: &mut SerieAffichage, m: &ModifsLot) -> bool {
    let mut c = ajouter_valeurs(&mut s.genres, &m.genres_ajouter);
    c |= retirer_valeurs(&mut s.genres, &m.genres_retirer);
    c
}

/// Modèles de la source de démonstration : (identifiant, nom, extension).
const MODELES_DEMO: [(&str, &str, &str); 3] = [
    ("demo:html", "Liste HTML", "html"),
    ("demo:csv", "Tableau CSV", "csv"),
    ("demo:txt", "Liste texte", "txt"),
];

/// Écrit la liste de démonstration d'un export et renvoie son bilan.
fn exporter_demo(
    modele: &str,
    dossier: &Path,
    titres: &[String],
    vignettes: Option<u32>,
    titre: Option<&str>,
    nom_fichier: &str,
) -> Result<RapportExportAffiche, String> {
    let extension = MODELES_DEMO
        .iter()
        .find(|(id, _, _)| *id == modele)
        .map(|(_, _, e)| *e)
        .ok_or_else(|| format!("modèle {modele} inconnu"))?;
    std::fs::create_dir_all(dossier).map_err(|e| e.to_string())?;
    let fichier = dossier.join(format!("{nom_fichier}.{extension}"));
    let mut texte = String::new();
    if let Some(t) = titre.filter(|t| !t.is_empty()) {
        texte.push_str(t);
        texte.push('\n');
    }
    for t in titres {
        texte.push_str(t);
        texte.push('\n');
    }
    std::fs::write(&fichier, texte).map_err(|e| e.to_string())?;
    Ok(RapportExportAffiche {
        fichier_liste: fichier,
        exportes: titres.len(),
        vignettes: if vignettes.is_some() { titres.len() } else { 0 },
        avertissements: Vec::new(),
    })
}

/// Effet d'une tâche simulée sur les films et séries de démonstration.
fn effet_simule(
    travail: &Travail,
    films: &mut [Film],
    series: &mut [SerieAffichage],
    services: &ReglagesServices,
) -> Effets {
    let mut effets = Effets::default();
    match travail {
        Travail::AppliquerResultat {
            film,
            resultat,
            ne_pas_remplacer,
        } => {
            if let Some(f) = films.iter_mut().find(|f| f.id == *film) {
                appliquer_resultat(f, resultat, *ne_pas_remplacer);
                effets.films.push(f.clone());
            }
        }
        Travail::RecupererAuto { ids, .. } => {
            for f in films.iter_mut().filter(|f| ids.contains(&f.id)) {
                f.metadonnees_ok = true;
                effets.films.push(f.clone());
            }
        }
        Travail::AppliquerResultatSerie {
            serie,
            resultat,
            ne_pas_remplacer,
        } => {
            if let Some(s) = series.iter_mut().find(|s| s.id == *serie) {
                if !*ne_pas_remplacer || s.resume.trim().is_empty() {
                    s.resume = resultat.resume.clone();
                }
                if !*ne_pas_remplacer {
                    s.titre = resultat.titre.clone();
                    s.titre_original = resultat.titre_original.clone();
                    s.annee = resultat.annee;
                }
                s.tmdb = resultat.id.clone();
                effets.series_modifiees = true;
            }
        }
        Travail::RecupererSeries { ids, .. } => {
            for s in series.iter_mut().filter(|s| ids.contains(&s.id)) {
                if s.tmdb.is_empty() {
                    s.tmdb = format!("{}", 70_000 + s.titre.len());
                }
                effets.series_modifiees = true;
            }
        }
        Travail::Renommer { ids } => {
            let refs: Vec<&Film> = films.iter().filter(|f| ids.contains(&f.id)).collect();
            let apercus = renommage::calculer(
                &refs,
                &services.gabarit_dossier_film,
                &services.gabarit_fichier_film,
            );
            for a in apercus.into_iter().filter(ApercuFilm::executable) {
                if let Some(f) = films.iter_mut().find(|f| f.id == a.id) {
                    f.dossier = a.nouveau_dossier.clone();
                    for (fic, ligne) in f.fichiers.iter_mut().zip(&a.fichiers) {
                        fic.nom = ligne.nouveau.clone();
                    }
                    if let Some(v) = f
                        .fichiers
                        .iter()
                        .find(|x| x.type_fichier == crate::modele::TypeFichier::Video)
                    {
                        f.nom_fichier = v.nom.clone();
                    }
                    effets.films.push(f.clone());
                }
            }
        }
        Travail::AnalyserMedias { films: ids, series } => {
            let n = films.iter().filter(|f| ids.contains(&f.id)).count() + series.len();
            effets.notifications.push((
                Niveau::Info,
                format!("Analyse : {n} élément(s) (démonstration)"),
            ));
        }
        Travail::Exporter {
            cible,
            modele,
            dossier,
            vignettes,
            titre,
        } => {
            let (titres, nom) = match cible {
                CibleExport::Films(ids) => (
                    films
                        .iter()
                        .filter(|f| ids.as_ref().is_none_or(|v| v.contains(&f.id)))
                        .map(|f| f.titre.clone())
                        .collect::<Vec<_>>(),
                    "films",
                ),
                CibleExport::Series(ids) => (
                    series
                        .iter()
                        .filter(|s| ids.as_ref().is_none_or(|v| v.contains(&s.id)))
                        .map(|s| s.titre.clone())
                        .collect::<Vec<_>>(),
                    "series",
                ),
            };
            effets.messages.push(Message::ExportTermine(exporter_demo(
                modele,
                dossier,
                &titres,
                *vignettes,
                titre.as_deref(),
                nom,
            )));
        }
        Travail::Nettoyer { liste } => effets.notifications.push((
            Niveau::Info,
            format!("Nettoyage : {} élément(s) (démonstration)", liste.len()),
        )),
        Travail::RecupererEvaluations { ids } => effets.notifications.push((
            Niveau::Info,
            format!("Évaluations : {} film(s) (démonstration)", ids.len()),
        )),
        Travail::EditionLotFilms { ids, modifs } => {
            for f in films.iter_mut().filter(|f| ids.contains(&f.id)) {
                if !f.verrouille && appliquer_lot_film(f, modifs) {
                    effets.films.push(f.clone());
                }
            }
        }
        Travail::EditionLotSeries { ids, modifs } => {
            for s in series.iter_mut().filter(|s| ids.contains(&s.id)) {
                if !s.verrouillee && appliquer_lot_serie(s, modifs) {
                    effets.series_modifiees = true;
                }
            }
        }
        Travail::MettreAJourSources
        | Travail::MettreAJourSeries
        | Travail::MettreAJourSelection { .. }
        | Travail::AnnulerRenommage { .. }
        | Travail::EcrireNfo { .. }
        | Travail::EcrireNfoSeries { .. }
        | Travail::RenommerSeries { .. } => {}
    }
    effets
}

/// Séries de démonstration (un épisode manquant, affiché selon les réglages).
pub fn series_demo() -> Vec<SerieAffichage> {
    let ep = |episode: i32, titre: &str, vu: bool| EpisodeAffichage {
        episode,
        titre: titre.into(),
        fichier: format!("S01E{episode:02}.mkv"),
        vu,
        infos_media: if episode == 1 {
            "1080p H264 · AC3 6ch · 44 min".into()
        } else {
            String::new()
        },
        ..EpisodeAffichage::default()
    };
    vec![
        SerieAffichage {
            id: "demo-1".into(),
            titre: "Série Exemple".into(),
            titre_original: "Example Show".into(),
            annee: 2019,
            chemin: "D:\\Séries\\Série Exemple (2019)".into(),
            resume: "Résumé factice de la série de démonstration.".into(),
            genres: vec!["Drame".into(), "Science-fiction".into()],
            statut: StatutSerie::Terminee,
            note: 7.8,
            tmdb: "1399".into(),
            saisons: vec![
                SaisonAffichage {
                    numero: 1,
                    titre: String::new(),
                    episodes: vec![ep(1, "Pilote", true), ep(2, "La suite", false)],
                },
                SaisonAffichage {
                    numero: 2,
                    titre: String::new(),
                    episodes: vec![
                        ep(1, "Le retour", false),
                        EpisodeAffichage {
                            episode: 2,
                            titre: "L'épisode absent".into(),
                            manquant: true,
                            ..EpisodeAffichage::default()
                        },
                    ],
                },
            ],
            ..SerieAffichage::default()
        },
        SerieAffichage {
            id: "demo-2".into(),
            titre: "Autre Série".into(),
            annee: 2023,
            chemin: "D:\\Séries\\Autre Série (2023)".into(),
            saisons: vec![SaisonAffichage {
                numero: 1,
                titre: String::new(),
                episodes: vec![ep(1, "Commencement", false)],
            }],
            ..SerieAffichage::default()
        },
    ]
}

/// Catalogue de la recherche factice de séries : (titre, titre original, année, résumé).
fn catalogue_series() -> Vec<(&'static str, &'static str, u32, &'static str)> {
    vec![
        ("Série Exemple", "Example Show", 2019, "Résumé factice."),
        ("Autre Série", "Another Show", 2023, "Autre résumé factice."),
        (
            "Autre Série",
            "Another Show (remake)",
            1998,
            "Version ancienne.",
        ),
        (
            "Les Enquêteurs",
            "The Investigators",
            2011,
            "Enquêtes en série.",
        ),
        (
            "Station Orbitale",
            "Orbital Station",
            2021,
            "Huis clos spatial.",
        ),
    ]
}

/// Image de démonstration : dégradé dont la teinte dépend de la graine, bandeau sombre en bas.
pub fn generer_image_demo(
    chemin: &Path,
    largeur: u32,
    hauteur: u32,
    graine: u64,
) -> Result<(), String> {
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let base = [
        (graine.wrapping_mul(97) % 200) as u8 + 30,
        (graine.wrapping_mul(53) % 200) as u8 + 30,
        (graine.wrapping_mul(29) % 200) as u8 + 30,
    ];
    let img = image::RgbImage::from_fn(largeur, hauteur, |x, y| {
        let t = y as f32 / hauteur.max(1) as f32;
        let u = x as f32 / largeur.max(1) as f32;
        let sombre = if t > 0.8 { 0.35 } else { 1.0 };
        let c = |v: u8, k: f32| ((v as f32 * (1.0 - 0.5 * t) + 60.0 * k) * sombre).min(255.0) as u8;
        image::Rgb([c(base[0], u), c(base[1], 1.0 - u), c(base[2], t)])
    });
    // Écriture atomique : fichier temporaire puis renommage (deux threads peuvent générer la même image).
    let tmp = chemin.with_extension(format!("{}.tmp", std::process::id()));
    img.save_with_format(&tmp, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, chemin).map_err(|e| e.to_string())
}

impl SourceDonnees for SourceFactice {
    fn films(&self) -> Vec<Film> {
        self.films.lock().map(|f| f.clone()).unwrap_or_default()
    }

    fn series(&self) -> Vec<SerieAffichage> {
        let manquants = self
            .services
            .lock()
            .map(|s| s.afficher_episodes_manquants)
            .unwrap_or(false);
        let mut v = self.series.lock().map(|s| s.clone()).unwrap_or_default();
        if !manquants {
            for s in &mut v {
                for saison in &mut s.saisons {
                    saison.episodes.retain(|e| !e.manquant);
                }
                s.saisons.retain(|x| !x.episodes.is_empty());
            }
        }
        v
    }

    fn sagas(&self) -> Vec<SagaAffichage> {
        let films = self.films();
        [
            ("demo-saga-dune", crate::donnees::SAGA_DUNE, 901u64),
            ("demo-saga-matrix", crate::donnees::SAGA_MATRIX, 902),
        ]
        .into_iter()
        .map(|(id, titre, graine)| SagaAffichage {
            id: id.into(),
            titre: titre.into(),
            titre_original: String::new(),
            resume: format!("Résumé factice de la collection « {titre} »."),
            films: films
                .iter()
                .filter(|f| f.saga.as_deref() == Some(titre))
                .map(|f| f.id)
                .collect(),
            affiche: self.image_demo(id, GenreImage::Affiche, graine),
            fanart: self.image_demo(id, GenreImage::Fanart, graine),
            banniere: None,
        })
        .collect()
    }

    fn chemin_image(&self, film: &Film, genre: GenreImage) -> Option<PathBuf> {
        // Comme une vraie bibliothèque, certains films n'ont pas d'images.
        if !film.images_ok && genre != GenreImage::Affiche {
            return None;
        }
        self.image_demo(&format!("film{}", film.id), genre, film.id)
    }

    fn rechercher(&self, requete: &RequeteRecherche) -> Result<Vec<ResultatRecherche>, String> {
        std::thread::sleep(self.latence);
        if requete.texte.trim().is_empty() {
            return Err("recherche vide".into());
        }
        let mut v: Vec<ResultatRecherche> = Self::catalogue()
            .into_iter()
            .enumerate()
            .map(|(n, (titre, original, annee, resume))| {
                let score = score_resultat(requete, &titre, &original, annee);
                ResultatRecherche {
                    id: format!("{}", 500_000 + n * 17),
                    affiche: self.image_demo(
                        &format!("resultat{n}"),
                        GenreImage::Affiche,
                        n as u64 + 100,
                    ),
                    titre,
                    titre_original: original,
                    annee,
                    score,
                    resume,
                }
            })
            .filter(|r| r.score >= 0.3)
            .collect();
        v.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.annee.cmp(&b.annee)));
        v.truncate(20);
        Ok(v)
    }

    fn enregistrer_films(&self, films: &[Film]) -> Result<(), String> {
        let mut tous = self.films.lock().map_err(|e| e.to_string())?;
        for f in films {
            if let Some(x) = tous.iter_mut().find(|x| x.id == f.id) {
                *x = f.clone();
            }
        }
        Ok(())
    }

    fn details_film(&self, film: &Film) -> DetailsFilm {
        self.details
            .lock()
            .ok()
            .and_then(|d| d.get(&film.id).cloned())
            .unwrap_or_else(|| DetailsFilm::depuis_film(film))
    }

    fn enregistrer_film_detaille(&self, film: &Film, details: &DetailsFilm) -> Result<(), String> {
        self.enregistrer_films(std::slice::from_ref(film))?;
        self.details
            .lock()
            .map_err(|e| e.to_string())?
            .insert(film.id, details.clone());
        Ok(())
    }

    fn creer_film_hors_ligne(
        &self,
        source: &str,
        titre: &str,
        annee: u32,
    ) -> Result<CreationHorsLigne, String> {
        let nom = nom_film_hors_ligne(titre, annee).ok_or_else(|| "titre invalide".to_string())?;
        let mut tous = self.films.lock().map_err(|e| e.to_string())?;
        let dossier = format!("{source}\\{nom}");
        if tous.iter().any(|f| f.dossier == dossier) {
            return Err(format!("{dossier} existe déjà"));
        }
        let id = tous.iter().map(|f| f.id).max().unwrap_or(0) + 1;
        tous.push(Film {
            id,
            titre: titre.trim().to_string(),
            titre_triable: titre.trim().to_string(),
            annee,
            dossier,
            source: source.to_string(),
            nom_fichier: format!("{nom}.disc"),
            nouveau: true,
            date_ajout: "2026-11-16 12:00".into(),
            ..Film::default()
        });
        Ok(CreationHorsLigne::Ajoute)
    }

    fn retirer_films(&self, ids: &[u64], _deplacer_fichiers: bool) -> Result<Vec<u64>, String> {
        let mut tous = self.films.lock().map_err(|e| e.to_string())?;
        let retires: Vec<u64> = tous
            .iter()
            .map(|f| f.id)
            .filter(|id| ids.contains(id))
            .collect();
        tous.retain(|f| !ids.contains(&f.id));
        Ok(retires)
    }

    fn transmettre_secret(&self, nom: &str, valeur: &str) -> Result<(), String> {
        let mut s = self.secrets.lock().map_err(|e| e.to_string())?;
        s.insert(nom.to_string(), valeur.to_string());
        Ok(())
    }

    fn reglages_services(&self) -> ReglagesServices {
        self.services.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn enregistrer_reglages_services(&self, r: &ReglagesServices) -> Result<(), String> {
        *self.services.lock().map_err(|e| e.to_string())? = r.clone();
        Ok(())
    }

    fn apercu_renommage(&self, ids: &[u64]) -> Result<Vec<ApercuFilm>, String> {
        let services = self.reglages_services();
        let films = self.films.lock().map_err(|e| e.to_string())?;
        let refs: Vec<&Film> = films.iter().filter(|f| ids.contains(&f.id)).collect();
        Ok(renommage::calculer(
            &refs,
            &services.gabarit_dossier_film,
            &services.gabarit_fichier_film,
        ))
    }

    fn apercu_renommage_series(&self, ids: &[String]) -> Result<Vec<ApercuFilm>, String> {
        let series = self.series.lock().map_err(|e| e.to_string())?;
        let mut v = Vec::new();
        for s in series.iter().filter(|s| ids.contains(&s.id)) {
            let ancien = s
                .chemin
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or_default()
                .to_string();
            let nouveau = renommage::assainir(&format!("{} ({})", s.titre, s.annee));
            let mut fichiers = Vec::new();
            for saison in &s.saisons {
                for e in saison.episodes.iter().filter(|e| !e.manquant) {
                    let cible = renommage::assainir(&format!(
                        "{} - S{:02}E{:02} - {}.mkv",
                        s.titre, saison.numero, e.episode, e.titre
                    ));
                    fichiers.push(LigneFichier {
                        type_fichier: "VIDEO".into(),
                        inchange: e.fichier == cible,
                        ancien: e.fichier.clone(),
                        nouveau: cible,
                    });
                }
            }
            v.push(ApercuFilm {
                id: v.len() as u64 + 1,
                titre: s.titre.clone(),
                source: "D:\\Séries".into(),
                inchange: ancien == nouveau && fichiers.iter().all(LigneFichier::inchange),
                ancien_dossier: ancien,
                nouveau_dossier: nouveau,
                fichiers,
                conflits: Vec::new(),
                avertissements: Vec::new(),
            });
        }
        Ok(v)
    }

    fn rechercher_serie(
        &self,
        requete: &RequeteRechercheSerie,
    ) -> Result<Vec<ResultatRecherche>, String> {
        std::thread::sleep(self.latence);
        if requete.texte.trim().is_empty() {
            return Err("recherche vide".into());
        }
        let pour_score = RequeteRecherche {
            film: 0,
            extracteur: "TMDB".into(),
            langue: requete.langue.clone(),
            texte: requete.texte.clone(),
            annee: requete.annee,
        };
        let mut v: Vec<ResultatRecherche> = catalogue_series()
            .into_iter()
            .enumerate()
            .map(|(n, (titre, original, annee, resume))| ResultatRecherche {
                id: format!("{}", 800_000 + n * 13),
                titre: titre.into(),
                titre_original: original.into(),
                annee,
                score: score_resultat(&pour_score, titre, original, annee),
                resume: resume.into(),
                affiche: self.image_demo(&format!("serie{n}"), GenreImage::Affiche, n as u64 + 300),
            })
            .filter(|r| r.score >= 0.3)
            .collect();
        v.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.annee.cmp(&b.annee)));
        Ok(v)
    }

    fn enregistrer_serie(&self, serie: &SerieAffichage) -> Result<(), String> {
        let mut tous = self.series.lock().map_err(|e| e.to_string())?;
        if let Some(x) = tous.iter_mut().find(|x| x.id == serie.id) {
            let saisons = std::mem::take(&mut x.saisons);
            *x = serie.clone();
            x.saisons = saisons;
        }
        Ok(())
    }

    fn details_serie(&self, serie: &SerieAffichage) -> DetailsSerie {
        self.details_series
            .lock()
            .ok()
            .and_then(|d| d.get(&serie.id).cloned())
            .unwrap_or_else(|| DetailsSerie::depuis_serie(serie))
    }

    fn enregistrer_serie_detaillee(
        &self,
        serie: &SerieAffichage,
        details: &DetailsSerie,
    ) -> Result<(), String> {
        self.enregistrer_serie(serie)?;
        self.details_series
            .lock()
            .map_err(|e| e.to_string())?
            .insert(serie.id.clone(), details.clone());
        Ok(())
    }

    fn films_manquants(&self, saga: &str) -> Result<Vec<ElementManquant>, String> {
        let element = |titre: &str, original: &str, annee: u32| ElementManquant {
            titre: titre.into(),
            titre_original: original.into(),
            detail: annee.to_string(),
            resume: format!("Résumé factice de « {titre} »."),
        };
        match saga {
            "demo-saga-dune" => Ok(vec![element(
                "Dune : Troisième partie",
                "Dune: Part Three",
                2026,
            )]),
            "demo-saga-matrix" => Ok(vec![
                element("Matrix Revolutions", "The Matrix Revolutions", 2003),
                element("Matrix Resurrections", "The Matrix Resurrections", 2021),
            ]),
            autre => Err(format!("saga {autre} inconnue")),
        }
    }

    fn episodes_manquants(&self, serie: &str) -> Result<Vec<ElementManquant>, String> {
        let series = self.series.lock().map_err(|e| e.to_string())?;
        let s = series
            .iter()
            .find(|s| s.id == serie)
            .ok_or_else(|| format!("série {serie} inconnue"))?;
        Ok(s.saisons
            .iter()
            .flat_map(|sa| sa.episodes.iter().map(move |e| (sa.numero, e)))
            .filter(|(_, e)| e.manquant)
            .map(|(n, e)| ElementManquant {
                titre: e.titre.clone(),
                titre_original: String::new(),
                detail: format!("S{n:02}E{:02}", e.episode),
                resume: String::new(),
            })
            .collect())
    }

    fn tester_outils(&self, chemin: &str) -> Result<DiagnosticOutils, String> {
        std::thread::sleep(self.latence);
        let chemin = chemin.trim();
        Ok(DiagnosticOutils {
            ffprobe: (!chemin.is_empty()).then(|| OutilAffiche {
                chemin: chemin.into(),
                version: Some("ffprobe version démonstration".into()),
            }),
            mediainfo: None,
        })
    }

    fn modeles_export(&self, series: bool) -> Vec<ModeleExportAffiche> {
        MODELES_DEMO
            .iter()
            .map(|(id, nom, ext)| ModeleExportAffiche {
                id: (*id).into(),
                nom: (*nom).into(),
                description: format!("Export {} (démonstration)", ext.to_uppercase()),
                nom_fichier: if series { "series" } else { "films" }.into(),
                extension: (*ext).into(),
            })
            .collect()
    }

    fn analyser_nettoyage(&self, ids: &[u64]) -> Result<Vec<FichierInutileAffiche>, String> {
        std::thread::sleep(self.latence);
        let films = self.films.lock().map_err(|e| e.to_string())?;
        let mut v = Vec::new();
        for f in films
            .iter()
            .filter(|f| ids.contains(&f.id) && !f.verrouille)
        {
            v.push(FichierInutileAffiche {
                id_film: f.id,
                titre_film: f.titre.clone(),
                chemin: format!("{}\\Thumbs.db", f.dossier),
                est_dossier: false,
                raison: RaisonNettoyage::Parasite,
                taille: 24_576,
            });
            v.push(FichierInutileAffiche {
                id_film: f.id,
                titre_film: f.titre.clone(),
                chemin: format!("{}\\Sample", f.dossier),
                est_dossier: true,
                raison: RaisonNettoyage::Extrait,
                taille: 38_000_000,
            });
            v.push(FichierInutileAffiche {
                id_film: f.id,
                titre_film: f.titre.clone(),
                chemin: format!("{}\\release.nfo", f.dossier),
                est_dossier: false,
                raison: RaisonNettoyage::Motif(r"(?i)^release\.nfo$".into()),
                taille: 1_200,
            });
        }
        Ok(v)
    }

    fn basculer_vu_series(&self, cibles: &[NoeudSerie]) -> Result<(), String> {
        let mut series = self.series.lock().map_err(|e| e.to_string())?;
        // Épisodes visés : (série, saison, rang dans la saison des épisodes affichés).
        let mut vises: Vec<(usize, usize, usize)> = Vec::new();
        for c in cibles {
            let Some(s) = series.iter().position(|s| s.id == c.id_serie()) else {
                return Err(format!("série {} introuvable", c.id_serie()));
            };
            for (n_saison, saison) in series[s].saisons.iter().enumerate() {
                for (rang, e) in saison.episodes.iter().enumerate() {
                    let vise = match c {
                        NoeudSerie::Serie(_) => true,
                        NoeudSerie::Saison(_, n) => *n == saison.numero,
                        NoeudSerie::Episode(_, n, r) => *n == saison.numero && *r == rang,
                    };
                    if vise && !e.manquant {
                        vises.push((s, n_saison, rang));
                    }
                }
            }
        }
        let tous_vus = vises
            .iter()
            .all(|&(s, n, r)| series[s].saisons[n].episodes[r].vu);
        for (s, n, r) in vises {
            series[s].saisons[n].episodes[r].vu = !tous_vus;
        }
        Ok(())
    }

    fn retirer_series(
        &self,
        ids: &[String],
        _deplacer_fichiers: bool,
    ) -> Result<Vec<String>, String> {
        let mut tous = self.series.lock().map_err(|e| e.to_string())?;
        let retirees: Vec<String> = tous
            .iter()
            .filter(|s| ids.contains(&s.id) && !s.verrouillee)
            .map(|s| s.id.clone())
            .collect();
        tous.retain(|s| !retirees.contains(&s.id));
        Ok(retirees)
    }

    fn lancer(&self, travail: Travail, retour: &Retour) -> Result<(), String> {
        let id = self.prochain_id.fetch_add(1, Ordering::SeqCst);
        let annulation = Arc::new(AtomicBool::new(false));
        if let Ok(mut a) = self.annulations.lock() {
            a.insert(id, annulation.clone());
        }
        let total: u32 = match travail {
            Travail::MettreAJourSources => 40,
            Travail::RecupererAuto { .. } => 25,
            _ => 10,
        };
        let (pas, films, series, services, annulations, retour) = (
            self.pas,
            self.films.clone(),
            self.series.clone(),
            self.services.clone(),
            self.annulations.clone(),
            retour.clone(),
        );
        std::thread::spawn(move || {
            retour.envoyer(Message::TacheDemarree {
                id,
                nom: travail.cle_nom().to_string(),
                principale: true,
            });
            let mut issue = IssueTache::Reussie;
            for fait in 1..=total {
                std::thread::sleep(pas);
                if annulation.load(Ordering::Relaxed) {
                    issue = IssueTache::Annulee;
                    break;
                }
                if !retour.envoyer(Message::TacheProgression {
                    id,
                    fait,
                    total,
                    description: String::new(),
                }) {
                    return;
                }
            }
            if issue == IssueTache::Reussie {
                let services = services.lock().map(|s| s.clone()).unwrap_or_default();
                let effets = match (films.lock(), series.lock()) {
                    (Ok(mut f), Ok(mut s)) => effet_simule(&travail, &mut f, &mut s, &services),
                    _ => Effets::default(),
                };
                for f in effets.films {
                    retour.envoyer(Message::FilmActualise(Box::new(f)));
                }
                if effets.series_modifiees {
                    retour.envoyer(Message::SeriesModifiees);
                }
                for m in effets.messages {
                    retour.envoyer(m);
                }
                for (niveau, texte) in effets.notifications {
                    retour.envoyer(Message::Notifier(niveau, texte));
                }
            }
            if let Ok(mut a) = annulations.lock() {
                a.remove(&id);
            }
            retour.envoyer(Message::TacheTerminee { id, issue });
        });
        Ok(())
    }

    fn annuler_tache(&self, id: u64) {
        if let Some(a) = self
            .annulations
            .lock()
            .ok()
            .and_then(|a| a.get(&id).cloned())
        {
            a.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similarite_bornee_et_ordonnee() {
        assert_eq!(similarite("Matrix", "matrix"), 1.0);
        assert_eq!(similarite("", "x"), 0.0);
        let proche = similarite("blade runner 2049", "Blade Runner 2049");
        let moyen = similarite("blade runner 2049", "Blade Runner");
        let loin = similarite("blade runner 2049", "Parasite");
        assert!(proche > moyen && moyen > loin, "{proche} {moyen} {loin}");
    }

    #[test]
    fn titre_nettoye_depuis_un_dossier() {
        assert_eq!(
            titre_nettoye("Blade.Runner.2049.2017.1080p"),
            ("Blade Runner 2049".into(), Some(2017))
        );
        assert_eq!(titre_nettoye("Alien (1979)"), ("Alien".into(), Some(1979)));
        assert_eq!(
            titre_nettoye("mad.max.fury.road.mkv"),
            ("mad max fury road".into(), None)
        );
    }

    #[test]
    fn recherche_factice_triee_par_score() {
        let dossier = std::env::temp_dir().join(format!("mmr-test-rech-{}", std::process::id()));
        let mut s = SourceFactice::new(dossier.clone());
        s.latence = std::time::Duration::ZERO;
        let r = s
            .rechercher(&RequeteRecherche {
                film: 2,
                extracteur: "TMDB".into(),
                langue: "fr".into(),
                texte: "Blade Runner 2049".into(),
                annee: Some(2017),
            })
            .unwrap();
        assert_eq!(r[0].titre, "Blade Runner 2049");
        assert!(r.windows(2).all(|w| w[0].score >= w[1].score));
        assert!(r.iter().any(|x| x.titre == "Blade Runner"));
        assert!(r[0].affiche.as_ref().unwrap().exists());
        let _ = std::fs::remove_dir_all(dossier);
    }

    #[test]
    fn secrets_conserves_en_memoire() {
        let s = SourceFactice::default();
        s.transmettre_secret("tmdb", "abc").unwrap();
        assert_eq!(s.secret("tmdb").as_deref(), Some("abc"));
    }
}

#[cfg(test)]
mod tests_hors_ligne {
    use super::*;

    #[test]
    fn nom_de_dossier_sans_caracteres_interdits() {
        assert_eq!(
            nom_film_hors_ligne("Mission: Impossible / 2", 1996).as_deref(),
            Some("Mission- Impossible - 2 (1996)")
        );
        assert_eq!(nom_film_hors_ligne(" Alien ", 0).as_deref(), Some("Alien"));
        assert_eq!(nom_film_hors_ligne("  ...  ", 2000), None);
    }

    #[test]
    fn marqueur_cree_sur_le_disque_sans_ecraser() {
        let d = std::env::temp_dir().join(format!("mmr-horsligne-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let dossier = creer_marqueur_hors_ligne(&d, "Le Film", 2010).unwrap();
        assert_eq!(dossier, d.join("Le Film (2010)"));
        assert!(dossier.join("Le Film (2010).disc").is_file());
        assert!(creer_marqueur_hors_ligne(&d, "Le Film", 2010).is_err());
        assert!(creer_marqueur_hors_ligne(&d.join("absent"), "X", 0).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
