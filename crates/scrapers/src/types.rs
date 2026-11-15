//! Types propres au crate : options, résultats de recherche, métadonnées, images.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Clés d'identifiants usuelles.
pub mod cles_id {
    pub const TMDB: &str = "tmdb";
    pub const TMDB_COLLECTION: &str = "tmdbSet";
    pub const IMDB: &str = "imdb";
    pub const TVDB: &str = "tvdb";
}

/// Options d'une recherche ou d'un scrap.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OptionsRecherche {
    /// Texte recherché.
    pub requete: String,
    /// Année recherchée (<= 1900 ou `None` = ignorée).
    pub annee: Option<i32>,
    /// Identifiants connus par fournisseur (`tmdb`, `imdb`, `tmdbSet`...).
    pub ids: BTreeMap<String, String>,
    /// Langue de scrap (`fr`, `pt_BR`...).
    pub langue: String,
    /// Pays de la classification d'âge (ISO 3166-1 alpha-2).
    pub pays_classification: String,
    /// Pays de la date de sortie locale ("" = date mondiale).
    pub pays_sortie: String,
}

impl Default for OptionsRecherche {
    fn default() -> Self {
        Self {
            requete: String::new(),
            annee: None,
            ids: BTreeMap::new(),
            langue: "en".into(),
            pays_classification: "US".into(),
            pays_sortie: String::new(),
        }
    }
}

impl OptionsRecherche {
    /// Options avec une requête texte.
    pub fn avec_requete(requete: impl Into<String>) -> Self {
        Self {
            requete: requete.into(),
            ..Self::default()
        }
    }

    /// Ajoute un identifiant (builder).
    pub fn avec_id(mut self, fournisseur: &str, id: impl Into<String>) -> Self {
        self.ids.insert(fournisseur.to_string(), id.into());
        self
    }

    /// Identifiant TMDB (0 si absent ou illisible).
    pub fn id_tmdb(&self) -> u64 {
        self.ids
            .get(cles_id::TMDB)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    }

    /// Identifiant de collection TMDB (0 si absent).
    pub fn id_collection(&self) -> u64 {
        self.ids
            .get(cles_id::TMDB_COLLECTION)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    }

    /// Identifiant IMDb valide ("" sinon).
    pub fn id_imdb(&self) -> String {
        match self.ids.get(cles_id::IMDB) {
            Some(v) if crate::correspondance::est_id_imdb(v.trim()) => v.trim().to_string(),
            _ => String::new(),
        }
    }
}

/// Résultat d'une recherche.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ResultatRecherche {
    pub ids: BTreeMap<String, String>,
    pub fournisseur: String,
    pub titre: String,
    pub titre_original: String,
    pub langue_originale: String,
    pub resume: String,
    /// Année de sortie (0 = inconnue).
    pub annee: i32,
    /// Score de correspondance (peut être négatif après pénalités).
    pub score: f32,
    pub url_affiche: String,
}

/// Rôle d'une personne au générique.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TypePersonne {
    Acteur,
    Realisateur,
    Scenariste,
    Producteur,
    Monteur,
    Compositeur,
    Photographie,
    Invite,
    Autre,
}

/// Personne du générique.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Personne {
    pub type_personne: TypePersonne,
    pub nom: String,
    /// Personnage (acteurs) ou fonction.
    pub role: String,
    pub ids: BTreeMap<String, String>,
    pub url_photo: String,
    pub url_profil: String,
}

/// Note attribuée par une source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Evaluation {
    pub source: String,
    pub valeur: f32,
    pub votes: u32,
    pub maximum: f32,
}

/// Classification d'âge brute (pays + libellé tel que fourni).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Classification {
    pub pays: String,
    pub valeur: String,
}

/// Métadonnées d'un film.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MetadonneesFilm {
    pub fournisseur: String,
    pub ids: BTreeMap<String, String>,
    pub titre: String,
    pub titre_original: String,
    pub titre_anglais: String,
    pub langue_originale: String,
    pub annee: i32,
    /// Date de sortie `AAAA-MM-JJ`.
    pub date_sortie: Option<String>,
    pub resume: String,
    pub accroche: String,
    /// Durée en minutes (0 = inconnue).
    pub duree: u32,
    pub nom_collection: String,
    pub evaluations: Vec<Evaluation>,
    pub personnes: Vec<Personne>,
    /// Genres : identifiant interne (`ADVENTURE`...) ou nom brut si inconnu.
    pub genres: Vec<String>,
    pub classifications: Vec<Classification>,
    pub societes_production: Vec<String>,
    pub langues_parlees: Vec<String>,
    pub pays: Vec<String>,
    pub mots_cles: Vec<String>,
    pub artwork: Vec<Artwork>,
}

/// Type d'image.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TypeArtwork {
    Affiche,
    Fond,
    Vignette,
    LogoTransparent,
}

/// Une déclinaison de taille d'une image.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TailleImage {
    pub largeur: u32,
    pub hauteur: u32,
    pub url: String,
    /// Ordre de taille (puissance de 2, 16 = très grande... 1 = petite).
    pub ordre: u32,
}

/// Image (affiche, fond...).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Artwork {
    pub fournisseur: String,
    pub type_artwork: TypeArtwork,
    pub url_apercu: String,
    pub url_originale: String,
    /// Code ISO-2 ("" = sans texte).
    pub langue: String,
    pub votes: u32,
    pub tailles: Vec<TailleImage>,
}

/// Bande-annonce.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BandeAnnonce {
    pub nom: String,
    pub url: String,
    /// `youtube`, `vimeo`...
    pub hebergeur: String,
    /// Ex. `1080p`.
    pub qualite: String,
    /// Ex. `fr-FR`.
    pub langue: String,
    pub date: String,
}

/// Un film d'une collection.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PartieCollection {
    pub ids: BTreeMap<String, String>,
    pub titre: String,
    pub titre_original: String,
    pub resume: String,
    pub date_sortie: Option<String>,
    pub annee: i32,
    pub url_affiche: String,
    /// Métadonnées complètes si elles ont été récupérées.
    pub metadonnees: Option<MetadonneesFilm>,
}

/// Collection (saga).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CollectionFilm {
    pub ids: BTreeMap<String, String>,
    pub titre: String,
    pub titre_anglais: String,
    pub resume: String,
    pub url_affiche: String,
    pub url_fond: String,
    /// Films déjà sortis, ordre du fournisseur.
    pub parties: Vec<PartieCollection>,
}
