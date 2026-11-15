//! Films et sagas (collections).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::commun::{
    BandeAnnonce, Edition, Identifiants, Note, Personne, SourceMedia, TypePersonne,
};
use super::fichier::{FichierMedia, TypeFichier};

/// Sources de notes essayées dans cet ordre pour la note « principale ».
pub const SOURCES_NOTE_PRIORITAIRES: [&str; 5] = [
    "tmdb",
    "imdb",
    "metacritic",
    "tomatometerallcritics",
    "trakt",
];

/// Historique d'un renommage (permet l'annulation).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoriqueRenommage {
    pub ancien_chemin: PathBuf,
    pub nouveau_chemin: PathBuf,
    /// Paires (ancien nom, nouveau nom) relatives au dossier.
    pub fichiers: Vec<(String, String)>,
}

/// Film de la médiathèque.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Film {
    /// Identifiant interne (attribué par la couche de stockage).
    pub id: String,
    /// Verrouillé : ignoré par la mise à jour des sources et le scraping.
    pub verrouille: bool,
    /// Racine de la source de données qui contient le film.
    pub source_donnees: PathBuf,
    pub ids: Identifiants,
    pub titre: String,
    pub titre_original: String,
    pub titre_anglais: String,
    pub titre_tri: String,
    /// Année (0 = inconnue).
    pub annee: u32,
    pub resume: String,
    /// Résumé court.
    pub synopsis: String,
    pub slogan: String,
    /// Dossier du film (absolu).
    pub chemin: PathBuf,
    /// Date d'ajout (ms depuis l'époque Unix).
    pub date_ajout: i64,
    /// Sociétés de production séparées par ` / `.
    pub societe_production: String,
    /// Note libre de l'utilisateur.
    pub note_utilisateur: String,
    /// Notes par source (`imdb`, `tmdb`, `user`…).
    pub notes: BTreeMap<String, Note>,
    pub fichiers: Vec<FichierMedia>,
    pub tags: Vec<String>,
    /// URL d'artwork par type.
    pub urls_artwork: BTreeMap<TypeFichier, String>,
    /// Nom du fichier vidéo d'origine (avant renommage).
    pub nom_fichier_origine: String,
    pub dernier_scraper: String,
    pub derniere_langue_scraping: String,
    pub historique_renommage: Option<HistoriqueRenommage>,

    /// Durée en minutes (0 = inconnue).
    pub duree_minutes: u32,
    pub vu: bool,
    pub nombre_lectures: u32,
    /// Dernière lecture (ms depuis l'époque Unix).
    pub derniere_lecture: Option<i64>,
    /// Dossier DVD / Blu-ray / HD-DVD.
    pub est_disque: bool,
    /// Codes ISO2 séparés par `, `.
    pub langues_parlees: String,
    /// Pays séparés par `/`.
    pub pays: String,
    /// Date de sortie `aaaa-mm-jj`.
    pub date_sortie: Option<String>,
    /// Le dossier contient plusieurs films (ou est la racine de la source).
    pub dossier_multi_films: bool,
    pub top250: u32,
    pub source_media: SourceMedia,
    pub video_3d: bool,
    /// Classification (texte libre, ex. `US:PG-13`).
    pub certification: String,
    /// Identifiant interne de la saga.
    pub saga_id: Option<String>,
    pub edition: Edition,
    /// Plusieurs fichiers vidéo (CD1/CD2…).
    pub empile: bool,
    /// Vidéo `.disc` : film hors ligne.
    pub hors_ligne: bool,
    pub genres: Vec<String>,
    pub vignettes_extra: Vec<String>,
    pub fanarts_extra: Vec<String>,
    pub acteurs: Vec<Personne>,
    /// Équipe technique (réalisateurs, scénaristes, producteurs…).
    pub equipe: Vec<Personne>,
    pub bandes_annonces: Vec<BandeAnnonce>,
    pub liens_series: Vec<String>,
}

impl Default for Film {
    fn default() -> Self {
        Self {
            id: String::new(),
            verrouille: false,
            source_donnees: PathBuf::new(),
            ids: Identifiants::default(),
            titre: String::new(),
            titre_original: String::new(),
            titre_anglais: String::new(),
            titre_tri: String::new(),
            annee: 0,
            resume: String::new(),
            synopsis: String::new(),
            slogan: String::new(),
            chemin: PathBuf::new(),
            date_ajout: maintenant_ms(),
            societe_production: String::new(),
            note_utilisateur: String::new(),
            notes: BTreeMap::new(),
            fichiers: Vec::new(),
            tags: Vec::new(),
            urls_artwork: BTreeMap::new(),
            nom_fichier_origine: String::new(),
            dernier_scraper: String::new(),
            derniere_langue_scraping: String::new(),
            historique_renommage: None,
            duree_minutes: 0,
            vu: false,
            nombre_lectures: 0,
            derniere_lecture: None,
            est_disque: false,
            langues_parlees: String::new(),
            pays: String::new(),
            date_sortie: None,
            dossier_multi_films: false,
            top250: 0,
            source_media: SourceMedia::Inconnue,
            video_3d: false,
            certification: String::new(),
            saga_id: None,
            edition: Edition::Aucune,
            empile: false,
            hors_ligne: false,
            genres: Vec::new(),
            vignettes_extra: Vec::new(),
            fanarts_extra: Vec::new(),
            acteurs: Vec::new(),
            equipe: Vec::new(),
            bandes_annonces: Vec::new(),
            liens_series: Vec::new(),
        }
    }
}

/// Date courante en millisecondes depuis l'époque Unix.
pub fn maintenant_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

impl Film {
    /// Film vide.
    pub fn nouveau() -> Self {
        Self::default()
    }

    /// Fichiers d'un type donné.
    pub fn fichiers_de_type(&self, t: TypeFichier) -> impl Iterator<Item = &FichierMedia> {
        self.fichiers.iter().filter(move |f| f.type_fichier == t)
    }

    pub fn a_fichier_video(&self) -> bool {
        self.fichiers_de_type(TypeFichier::Video).next().is_some()
    }

    /// Fichier vidéo principal : l'unique vidéo ; sinon, pour un film empilé,
    /// la pile de plus petit numéro ; sinon le plus gros fichier.
    pub fn fichier_principal(&self) -> Option<&FichierMedia> {
        let videos: Vec<&FichierMedia> = self.fichiers_de_type(TypeFichier::Video).collect();
        match videos.len() {
            0 => None,
            1 => Some(videos[0]),
            _ => {
                if videos.iter().any(|v| v.empilement > 0) {
                    videos
                        .iter()
                        .filter(|v| v.empilement > 0)
                        .min_by_key(|v| v.empilement)
                        .copied()
                } else {
                    // `>=` côté original : en cas d'égalité, le dernier l'emporte.
                    videos
                        .iter()
                        .copied()
                        .fold(None, |m: Option<&FichierMedia>, v| match m {
                            Some(x) if x.taille > v.taille => Some(x),
                            _ => Some(v),
                        })
                }
            }
        }
    }

    /// Nom de base de la première vidéo, sans marqueur de pile.
    pub fn nom_video_sans_pile(&self) -> String {
        match self.fichiers_de_type(TypeFichier::Video).next() {
            Some(f) => crate::scan::pile::nettoyer_marqueur_pile(&f.nom_de_base()),
            None => String::new(),
        }
    }

    /// Première note disponible selon [`SOURCES_NOTE_PRIORITAIRES`], sinon la
    /// première note d'une autre source que `user`.
    pub fn note_principale(&self) -> Option<&Note> {
        for s in SOURCES_NOTE_PRIORITAIRES {
            if let Some(n) = self.notes.get(s) {
                return Some(n);
            }
        }
        self.notes
            .iter()
            .filter(|(k, _)| k.as_str() != "user")
            .map(|(_, n)| n)
            .next_back()
    }

    fn personnes_de_type(&self, t: TypePersonne) -> impl Iterator<Item = &Personne> {
        self.equipe.iter().filter(move |p| p.type_personne == t)
    }

    pub fn realisateurs(&self) -> Vec<&Personne> {
        self.personnes_de_type(TypePersonne::Realisateur).collect()
    }

    pub fn scenaristes(&self) -> Vec<&Personne> {
        self.personnes_de_type(TypePersonne::Scenariste).collect()
    }

    pub fn producteurs(&self) -> Vec<&Personne> {
        self.personnes_de_type(TypePersonne::Producteur).collect()
    }

    /// Décennie courte (`1979` ⇒ `1970s`), vide si l'année est inconnue.
    pub fn decennie_courte(&self) -> String {
        if self.annee == 0 {
            String::new()
        } else {
            format!("{}0s", self.annee / 10)
        }
    }

    /// Décennie longue (`1979` ⇒ `1970-1979`), vide si l'année est inconnue.
    pub fn decennie_longue(&self) -> String {
        if self.annee == 0 {
            String::new()
        } else {
            let d = self.annee / 10 * 10;
            format!("{}-{}", d, d + 9)
        }
    }
}

/// Saga (collection de films).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Saga {
    pub id: String,
    pub titre: String,
    pub titre_original: String,
    pub resume: String,
    pub ids: Identifiants,
    pub tags: Vec<String>,
    pub notes: BTreeMap<String, Note>,
    pub urls_artwork: BTreeMap<TypeFichier, String>,
    /// Fichiers d'artwork de la saga (dossier de données des sagas).
    pub fichiers: Vec<FichierMedia>,
    /// Identifiants internes des films de la saga.
    pub films: Vec<String>,
}
