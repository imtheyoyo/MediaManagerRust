//! Barre d'outils : logo + quatre grands boutons avec menu déroulant, cloche de notifications.

use super::{Sortie, menus};
use crate::commandes::{
    Commande, Entree, MENU_EDITION, MENU_MISE_A_JOUR, MENU_RECHERCHE, MENU_RENOMMAGE,
};
use crate::etat::{Etat, Message, TypeDialogue};
use crate::polices;
use crate::theme::{BARRE_OUTILS, ORANGE, TEXTE_BARRE_OUTILS};
use eframe::egui::{self, Align, Frame, Layout, Margin, Panel, RichText};

/// Description d'un grand bouton de la barre d'outils et de son menu déroulant.
struct Groupe {
    glyphe: &'static str,
    cle_bouton: &'static str,
    cle_menu: &'static str,
    action: Commande,
    entrees: &'static [Entree],
}

const GROUPES: [Groupe; 4] = [
    Groupe {
        glyphe: "⟳",
        cle_bouton: "Toolbar.update",
        cle_menu: "Toolbar.menu.update",
        action: Commande::MettreAJourToutesSources,
        entrees: MENU_MISE_A_JOUR,
    },
    Groupe {
        glyphe: "🔍",
        cle_bouton: "Toolbar.search",
        cle_menu: "Toolbar.menu.search",
        action: Commande::ScrapeInteractif,
        entrees: MENU_RECHERCHE,
    },
    Groupe {
        glyphe: polices::EDITER,
        cle_bouton: "Toolbar.edit",
        cle_menu: "Toolbar.menu.edit",
        action: Commande::Editer,
        entrees: MENU_EDITION,
    },
    Groupe {
        glyphe: polices::RENOMMER,
        cle_bouton: "Toolbar.rename",
        cle_menu: "Toolbar.menu.rename",
        action: Commande::Renommer,
        entrees: MENU_RENOMMAGE,
    },
];

fn groupe(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie, g: &Groupe) {
    let Groupe {
        glyphe,
        cle_bouton,
        cle_menu,
        action,
        entrees,
    } = *g;
    ui.vertical_centered(|ui| {
        let texte = RichText::new(format!("{glyphe}  {}", etat.i18n.t(cle_bouton))).size(13.0);
        if ui.button(texte).clicked() {
            sortie.push(Message::Commande(action));
        }
        ui.menu_button(
            RichText::new(format!("{} {}", etat.i18n.t(cle_menu), polices::DEROULANT)).size(11.0),
            |ui| {
                menus::afficher_entrees(ui, etat, entrees, sortie);
            },
        );
    });
}

pub fn afficher(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    Panel::top("barre_outils")
        .frame(
            Frame::new()
                .fill(BARRE_OUTILS)
                .inner_margin(Margin::symmetric(10, 6)),
        )
        .show(ui, |ui| {
            ui.visuals_mut().override_text_color = Some(TEXTE_BARRE_OUTILS);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(etat.i18n.t("app.title")).strong().size(16.0));
                    ui.label(RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).size(11.0));
                });
                ui.add_space(16.0);
                for g in &GROUPES {
                    groupe(ui, etat, sortie, g);
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mut texte = RichText::new("🔔").size(18.0);
                    if etat.non_lus > 0 {
                        texte = RichText::new(format!("🔔 {}", etat.non_lus))
                            .size(18.0)
                            .color(ORANGE);
                    }
                    if ui
                        .button(texte)
                        .on_hover_text(etat.i18n.t("app.messages"))
                        .clicked()
                    {
                        sortie.push(Message::OuvrirDialogue(TypeDialogue::Messages));
                    }
                });
            });
        });
}
