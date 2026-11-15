#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Exécutable de l'interface graphique : services réels par défaut, données de démonstration
//! avec `--demo` (tests visuels).

fn main() -> eframe::Result {
    // Fichiers `.old` laissés par une mise à jour précédente.
    mediamanager_gui::mise_a_jour::nettoyer_au_demarrage();
    if std::env::args()
        .skip(1)
        .any(|a| a == mediamanager_gui::mise_a_jour::ARGUMENT_RELANCE)
    {
        return mediamanager_gui::lancer_services_apres_mise_a_jour();
    }
    if std::env::args().skip(1).any(|a| a == "--demo") {
        return mediamanager_gui::lancer();
    }
    mediamanager_gui::lancer_services(mediamanager_app::OptionsDemarrage::default())
}
