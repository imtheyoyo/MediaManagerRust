//! Erreurs du crate.

use thiserror::Error;

/// Erreur renvoyée par les fournisseurs et le client HTTP.
#[derive(Debug, Error)]
pub enum ErreurScraper {
    /// Statut HTTP >= 400 (l'URL fournie ne contient jamais la clé d'API).
    #[error("erreur HTTP {statut} pour {url}")]
    Http { statut: u16, url: String },
    /// Limitation de débit : `Retry-After` supérieur au maximum toléré.
    #[error("limitation de débit : attente demandée de {secondes} s (trop longue)")]
    TropDeRequetes { secondes: u64 },
    /// Erreur réseau (connexion, délai dépassé, TLS...).
    #[error("erreur réseau : {0}")]
    Reseau(String),
    /// Erreur d'entrée/sortie (cache disque).
    #[error("erreur d'entrée/sortie : {0}")]
    Io(String),
    /// Réponse illisible.
    #[error("réponse illisible : {0}")]
    Analyse(String),
    /// Aucune clé d'API n'a été configurée.
    #[error("clé d'API manquante : elle doit être fournie par l'utilisateur")]
    CleApiManquante,
    /// Clé d'API refusée par le service (HTTP 401).
    #[error("clé d'API invalide")]
    CleApiInvalide,
    /// Aucun identifiant exploitable dans les options.
    #[error("aucun identifiant exploitable (attendu : {0})")]
    IdentifiantManquant(String),
    /// Rien trouvé pour la demande.
    #[error("aucun résultat")]
    AucunResultat,
}

/// Alias de résultat du crate.
pub type Resultat<T> = Result<T, ErreurScraper>;
