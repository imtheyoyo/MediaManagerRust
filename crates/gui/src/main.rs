#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Exécutable de l'interface graphique : services réels par défaut, données de démonstration
//! avec `--demo` (tests visuels).

fn main() -> eframe::Result {
    if std::env::args().skip(1).any(|a| a == "--demo") {
        return mediamanager_gui::lancer();
    }
    mediamanager_gui::lancer_services(mediamanager_app::OptionsDemarrage::default())
}
