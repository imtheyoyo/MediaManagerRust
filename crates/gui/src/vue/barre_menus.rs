//! Barre de menus Windows classique (Fichier, Édition, Films, Outils, Aide), en plus de la
//! barre d'outils ; les entrées de commandes proviennent de la table unique de `commandes`.

use super::{Sortie, menus};
use crate::commandes::{
    Commande, Entree, MENU_EDITION, MENU_MISE_A_JOUR, MENU_RECHERCHE, MENU_RENOMMAGE,
};
use crate::etat::{Etat, Message, Niveau, TypeDialogue};
use eframe::egui::{self, Button, Frame, Margin, Panel};

/// Entrée hors table de commandes : libellé, raccourci affiché, message émis.
fn entree(
    ui: &mut egui::Ui,
    libelle: String,
    raccourci: Option<&str>,
    message: Message,
    sortie: &mut Sortie,
) {
    let mut b = Button::new(libelle);
    if let Some(r) = raccourci {
        b = b.shortcut_text(r);
    }
    if ui.add(b).clicked() {
        sortie.push(message);
        ui.close();
    }
}

fn commandes(ui: &mut egui::Ui, etat: &Etat, liste: &[Commande], sortie: &mut Sortie) {
    let entrees: Vec<Entree> = liste.iter().map(|&c| Entree::Cmd(c)).collect();
    menus::afficher_entrees(ui, etat, &entrees, sortie);
}

pub fn afficher(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    let i = &etat.i18n;
    Panel::top("barre_menus")
        .frame(
            Frame::new()
                .fill(ui.visuals().panel_fill)
                .inner_margin(Margin::symmetric(6, 2)),
        )
        .show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button(i.t("menu.file"), |ui| {
                    commandes(
                        ui,
                        etat,
                        &[Commande::AjouterSource, Commande::MettreAJourToutesSources],
                        sortie,
                    );
                    ui.separator();
                    commandes(ui, etat, &[Commande::Exporter], sortie);
                    ui.separator();
                    entree(
                        ui,
                        format!("{}…", i.t("app.settings")),
                        None,
                        Message::OuvrirDialogue(TypeDialogue::Reglages),
                        sortie,
                    );
                    ui.separator();
                    entree(
                        ui,
                        i.t("menu.exit"),
                        Some("Alt+F4"),
                        Message::DemandeQuitter,
                        sortie,
                    );
                });
                ui.menu_button(i.t("menu.edit"), |ui| {
                    entree(
                        ui,
                        i.t("menu.selectall"),
                        Some("Ctrl+A"),
                        Message::SelectionnerTout,
                        sortie,
                    );
                    entree(
                        ui,
                        i.t("menu.find"),
                        Some("Ctrl+F"),
                        Message::FocusRecherche,
                        sortie,
                    );
                    ui.separator();
                    entree(
                        ui,
                        format!("{}…", i.t("movieextendedsearch.options")),
                        None,
                        Message::OuvrirFiltres(true),
                        sortie,
                    );
                    entree(
                        ui,
                        i.t("filter.reset"),
                        None,
                        Message::EffacerFiltres,
                        sortie,
                    );
                    ui.separator();
                    commandes(
                        ui,
                        etat,
                        &[
                            Commande::Editer,
                            Commande::EditionGlobale,
                            Commande::BasculerVerrou,
                            Commande::BasculerVu,
                        ],
                        sortie,
                    );
                });
                ui.menu_button(i.t("app.movies"), |ui| {
                    for (cle, liste) in [
                        ("Toolbar.update", MENU_MISE_A_JOUR),
                        ("Toolbar.search", MENU_RECHERCHE),
                        ("Toolbar.edit", MENU_EDITION),
                        ("Toolbar.rename", MENU_RENOMMAGE),
                    ] {
                        ui.menu_button(i.t(cle), |ui| {
                            menus::afficher_entrees(ui, etat, liste, sortie)
                        });
                    }
                    ui.separator();
                    commandes(
                        ui,
                        etat,
                        &[
                            Commande::OuvrirDossier,
                            Commande::RetirerBase,
                            Commande::SupprimerFichiers,
                        ],
                        sortie,
                    );
                });
                ui.menu_button(i.t("Toolbar.tools"), |ui| {
                    commandes(
                        ui,
                        etat,
                        &[
                            Commande::Exporter,
                            Commande::Nettoyer,
                            Commande::RecupererEvaluations,
                        ],
                        sortie,
                    );
                    ui.separator();
                    entree(
                        ui,
                        i.t("gui.tools.messages"),
                        None,
                        Message::OuvrirDialogue(TypeDialogue::Messages),
                        sortie,
                    );
                    entree(
                        ui,
                        i.t("menu.clearimagecache"),
                        None,
                        Message::ViderCacheImages,
                        sortie,
                    );
                    ui.separator();
                    entree(
                        ui,
                        format!("{}…", i.t("menu.wizard")),
                        None,
                        Message::OuvrirDialogue(TypeDialogue::Assistant),
                        sortie,
                    );
                });
                ui.menu_button(i.t("Toolbar.help"), |ui| {
                    entree(
                        ui,
                        format!("{}…", i.t("gui.update.check")),
                        None,
                        Message::RechercherMiseAJour { manuel: true },
                        sortie,
                    );
                    ui.separator();
                    entree(
                        ui,
                        i.t("gui.help.about"),
                        None,
                        Message::Notifier(Niveau::Info, i.t("gui.about.title")),
                        sortie,
                    );
                });
            });
        });
}
