//! Erreurs de la couche de stockage.

use std::path::PathBuf;

/// Erreur de la couche de stockage.
#[derive(Debug, thiserror::Error)]
pub enum ErreurStockage {
    /// Erreur d'entrée/sortie, avec le chemin concerné si connu.
    #[error("erreur d'entrée/sortie sur {chemin:?} : {source}")]
    Io {
        chemin: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Erreur SQLite.
    #[error("erreur de base de données : {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Erreur de (dé)sérialisation JSON.
    #[error("erreur JSON : {0}")]
    Json(#[from] serde_json::Error),

    /// Erreur de lecture ou d'écriture d'archive.
    #[error("erreur d'archive : {0}")]
    Archive(#[from] zip::result::ZipError),

    /// Une autre instance de l'application utilise déjà le dossier de données.
    #[error("une autre instance de l'application est déjà en cours d'exécution (verrou {0:?})")]
    InstanceDejaLancee(PathBuf),

    /// Aucun dossier utilisateur n'a pu être déterminé.
    #[error("impossible de déterminer le dossier utilisateur de l'application")]
    DossierUtilisateurIntrouvable,

    /// La base a été créée par une version plus récente de l'application.
    #[error("schéma de base en version {trouvee}, version maximale prise en charge {maximale}")]
    SchemaTropRecent { trouvee: i64, maximale: i64 },

    /// La base est corrompue (contrôle d'intégrité en échec).
    #[error("base de données corrompue : {0}")]
    BaseCorrompue(String),

    /// Erreur du magasin de secrets.
    #[error("magasin de secrets : {0}")]
    Secrets(String),

    /// Archive de sauvegarde invalide.
    #[error("sauvegarde invalide : {0}")]
    SauvegardeInvalide(String),
}

impl ErreurStockage {
    /// Construit une erreur d'entrée/sortie liée à un chemin.
    pub(crate) fn io(chemin: impl Into<PathBuf>, source: std::io::Error) -> Self {
        ErreurStockage::Io {
            chemin: chemin.into(),
            source,
        }
    }
}

/// Résultat de la couche de stockage.
pub type Resultat<T> = Result<T, ErreurStockage>;
