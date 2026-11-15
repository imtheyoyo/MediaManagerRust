//! Interface graphique (egui) de MediaManagerRust : coque, état, i18n, dialogues, filtres,
//! images, persistance de l'interface, branchement sur les services.

pub mod app;
pub mod commandes;
pub mod conversion;
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
pub mod source_application;
pub mod source_donnees;
pub mod taches;
pub mod theme;
pub mod vue;

use mediamanager_app::{Application as Services, ErreurApp, OptionsDemarrage};
use mediamanager_store::ErreurStockage;
use std::path::PathBuf;
use std::sync::Arc;

/// Lance la fenêtre principale avec le fichier d'interface par défaut et la source factice
/// (démonstration, tests visuels).
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

/// Démarre les services réels (mode portable par défaut), puis la fenêtre ; `ui.json` est
/// rangé dans le dossier de données du stockage. Arrêt propre des services à la fermeture.
/// Un démarrage impossible (autre instance lancée…) est signalé par une boîte de message.
pub fn lancer_services(options: OptionsDemarrage) -> eframe::Result {
    match Services::demarrer(options) {
        Ok(app) => {
            let source = Arc::new(source_application::SourceApplication::nouvelle(app));
            let chemin_ui = source
                .dossier_donnees()
                .map(|d| d.join(persistance::NOM_FICHIER))
                .unwrap_or_else(persistance::chemin_par_defaut);
            let resultat = lancer_avec(chemin_ui, source.clone());
            if let Err(e) = source.arreter() {
                tracing::error!("arrêt des services : {e}");
                eprintln!("arrêt des services : {e}");
            }
            resultat
        }
        Err(e) => {
            let i18n = i18n::I18n::new(langue_systeme());
            let texte = message_erreur_demarrage(&e, &i18n);
            eprintln!("{texte}");
            afficher_message(
                &i18n.t("gui.startup.title"),
                &texte,
                &i18n.t("Button.close"),
            )
        }
    }
}

/// Langue de l'interface déduite de l'environnement (repli : français).
fn langue_systeme() -> i18n::Langue {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find_map(|v| i18n::Langue::depuis_code(&v))
        .unwrap_or(i18n::Langue::Francais)
}

/// Texte présenté quand le démarrage des services échoue.
pub fn message_erreur_demarrage(e: &ErreurApp, i18n: &i18n::I18n) -> String {
    match e {
        ErreurApp::Stockage(ErreurStockage::InstanceDejaLancee(_)) => {
            i18n.t("gui.startup.alreadyrunning")
        }
        autre => i18n.t_args(
            "gui.startup.error",
            &[("error", autre.to_string().as_str())],
        ),
    }
}

/// Petite fenêtre de message (aucune dépendance native supplémentaire).
struct BoiteMessage {
    texte: String,
    bouton: String,
}

impl eframe::App for BoiteMessage {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        use eframe::egui::{Align, CentralPanel, Layout, ViewportCommand};
        CentralPanel::default().show(ui, |ui| {
            ui.add_space(8.0);
            ui.label(&self.texte);
            ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
                if ui.button(&self.bouton).clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
            });
        });
    }
}

/// Affiche un message bloquant dans une petite fenêtre.
pub fn afficher_message(titre: &str, texte: &str, bouton: &str) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(titre)
            .with_inner_size([460.0, 160.0])
            .with_resizable(false),
        ..Default::default()
    };
    let boite = BoiteMessage {
        texte: texte.to_string(),
        bouton: bouton.to_string(),
    };
    eframe::run_native(titre, options, Box::new(move |_| Ok(Box::new(boite))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_deja_lancee_signalee_proprement() {
        let d = tempfile::tempdir().unwrap();
        let premiere = source_application::tests::demarrer(d.path());
        let dossiers = premiere.dossiers();
        let options = || OptionsDemarrage {
            dossiers: Some(dossiers.clone()),
            coffre: Some(Box::new(mediamanager_store::CoffreMemoire::default())),
            journal: mediamanager_app::OptionsJournal {
                actif: false,
                ..Default::default()
            },
            services: Default::default(),
        };
        let e = Services::demarrer(options()).unwrap_err();
        let i18n = i18n::I18n::new(i18n::Langue::Francais);
        let texte = message_erreur_demarrage(&e, &i18n);
        assert!(texte.contains("déjà lancé"), "{texte}");
        premiere.arreter().unwrap();
        // Verrou libéré par l'arrêt : un nouveau démarrage réussit.
        Services::demarrer(options()).unwrap().arreter().unwrap();
    }
}
