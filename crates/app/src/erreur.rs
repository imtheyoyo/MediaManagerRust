//! Erreurs de la couche de services.

use std::path::PathBuf;

/// Erreur renvoyée par les services de l'application.
#[derive(Debug, thiserror::Error)]
pub enum ErreurApp {
    /// Erreur de la couche de stockage (base, réglages, coffre…).
    #[error(transparent)]
    Stockage(#[from] mediamanager_store::ErreurStockage),

    /// Erreur d'un fournisseur de métadonnées.
    #[error("fournisseur de métadonnées : {0}")]
    Scraper(#[from] mediamanager_scrapers::ErreurScraper),

    /// Source de données inutilisable (absente, vide…).
    #[error(transparent)]
    Scan(#[from] mediamanager_core::scan::ErreurScan),

    /// Erreur d'entrée/sortie sur un chemin.
    #[error("erreur d'entrée/sortie sur {chemin:?} : {message}")]
    Io { chemin: PathBuf, message: String },

    /// Aucune clé d'API TMDB n'est enregistrée dans le coffre à secrets.
    #[error(
        "aucune clé d'API TMDB n'est configurée : saisissez votre clé personnelle dans les réglages"
    )]
    CleTmdbAbsente,

    /// Téléchargement d'une image impossible.
    #[error("téléchargement de {url} impossible : {message}")]
    Telechargement { url: String, message: String },

    /// Entité introuvable en base.
    #[error("{0} introuvable")]
    Introuvable(String),

    /// L'entité est verrouillée (ignorée par le scraping et la mise à jour).
    #[error("« {0} » est verrouillé")]
    Verrouille(String),

    /// La tâche a été annulée.
    #[error("tâche annulée")]
    Annulee,

    /// L'application est en cours d'arrêt : aucune nouvelle tâche acceptée.
    #[error("l'application est en cours d'arrêt")]
    ArretEnCours,

    /// Une tâche s'est terminée par une panique (bogue).
    #[error("erreur interne : {0}")]
    Interne(String),
}

impl ErreurApp {
    /// Construit une erreur d'entrée/sortie liée à un chemin.
    pub fn io(chemin: impl Into<PathBuf>, e: impl std::fmt::Display) -> Self {
        ErreurApp::Io {
            chemin: chemin.into(),
            message: e.to_string(),
        }
    }
}

/// Résultat des services.
pub type Resultat<T> = Result<T, ErreurApp>;
