//! Interface graphique (egui) de MediaManagerRust : coque, état, i18n, dialogues de démonstration.

pub mod app;
pub mod commandes;
pub mod donnees;
pub mod etat;
pub mod i18n;
pub mod modele;
pub mod reglages;
pub mod renommage;
pub mod taches;
pub mod theme;
pub mod vue;

/// Lance la fenêtre principale (1100x720, minimum 1050x700).
pub fn lancer() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(i18n::NOM_PRODUIT)
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([1050.0, 700.0]),
        ..Default::default()
    };
    eframe::run_native(
        i18n::NOM_PRODUIT,
        options,
        Box::new(|cc| Ok(Box::new(app::Application::nouvelle(cc)))),
    )
}
