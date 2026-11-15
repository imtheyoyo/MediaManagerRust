//! Sélecteur de dossiers natif (dialogue « Sélectionner un dossier » de Windows, portail XDG
//! ailleurs). Le dialogue est bloquant : il est ouvert dans un thread dédié et le résultat
//! revient à l'interface par message (`Message::DossierChoisi`).

use std::path::PathBuf;
use std::sync::Arc;

/// Destination du dossier choisi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CibleDossier {
    /// Source de films (réglages, commande « Ajouter une source »).
    SourceFilms,
    /// Source de séries (réglages).
    SourceSeries,
    /// Source de films de l'assistant de premier lancement.
    Assistant,
}

/// Ouvre le sélecteur (titre de la fenêtre) ; `None` si l'utilisateur annule. Remplaçable
/// dans les tests.
pub type SelecteurDossier = Arc<dyn Fn(&str) -> Option<PathBuf> + Send + Sync>;

/// Sélecteur natif.
pub fn natif() -> SelecteurDossier {
    Arc::new(|titre: &str| rfd::FileDialog::new().set_title(titre).pick_folder())
}

/// Sélecteur des tests : n'ouvre aucune fenêtre.
pub fn inactif() -> SelecteurDossier {
    Arc::new(|_: &str| None)
}

/// Sélecteur par défaut (natif, sauf dans les tests unitaires).
pub fn par_defaut() -> SelecteurDossier {
    if cfg!(test) { inactif() } else { natif() }
}
