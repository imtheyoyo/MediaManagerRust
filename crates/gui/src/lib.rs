//! Interface graphique (egui) de MediaManagerRust : coque, état, i18n, dialogues, filtres,
//! images, persistance de l'interface.

pub mod app;
pub mod commandes;
pub mod dialogues;
pub mod donnees;
pub mod etat;
pub mod filtres;
pub mod i18n;
pub mod images;
pub mod modele;
pub mod persistance;
pub mod polices;
pub mod reglages;
pub mod renommage;
pub mod source_donnees;
pub mod taches;
pub mod theme;
pub mod vue;

use std::path::PathBuf;
use std::sync::Arc;

/// Lance la fenêtre principale avec le fichier d'interface par défaut et la source factice.
pub fn lancer() -> eframe::Result {
    lancer_avec(
        persistance::chemin_par_defaut(),
        Arc::new(source_donnees::SourceFactice::default()),
    )
}

/// Lance la fenêtre principale : `chemin_ui` = fichier `ui.json` (chargé au démarrage, écrit
/// atomiquement à la fermeture) ; `source` = services de l'application (frontière d'intégration).
pub fn lancer_avec(
    chemin_ui: PathBuf,
    source: Arc<dyn source_donnees::SourceDonnees>,
) -> eframe::Result {
    let (etat_ui, erreur) = match persistance::charger(&chemin_ui) {
        Ok(e) => (e, None),
        Err(e) => (persistance::EtatInterface::default(), Some(e.to_string())),
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(i18n::NOM_PRODUIT)
            .with_inner_size(etat_ui.fenetre.taille_bornee())
            .with_maximized(etat_ui.fenetre.maximisee)
            .with_min_inner_size([1050.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        i18n::NOM_PRODUIT,
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::Application::nouvelle(
                cc, source, chemin_ui, etat_ui, erreur,
            )))
        }),
    )
}
