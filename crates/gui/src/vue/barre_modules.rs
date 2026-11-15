//! Barre de modules verticale (gauche) : Films / Collections / Séries ; Outils, Paramètres, Info en bas.

use super::Sortie;
use crate::etat::{Etat, Message, Module, TypeDialogue};
use crate::theme::{ACCENT_SOMBRE, BARRE_OUTILS, TEXTE_BARRE_OUTILS};
use eframe::egui::{self, Align, Button, Frame, Layout, Margin, Panel, RichText, Vec2};

fn icone(module: Module) -> &'static str {
    match module {
        Module::Films => "🎬",
        Module::Collections => "📚",
        Module::Series => "📺",
    }
}

fn gros_bouton(texte: &str, actif: bool) -> Button<'static> {
    let couleur = if actif {
        ACCENT_SOMBRE
    } else {
        TEXTE_BARRE_OUTILS
    };
    Button::new(RichText::new(texte.to_string()).size(22.0).color(couleur))
        .min_size(Vec2::new(44.0, 44.0))
        .selected(actif)
        .frame(actif)
}

pub fn afficher(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    Panel::left("barre_modules")
        .exact_size(56.0)
        .resizable(false)
        .frame(
            Frame::new()
                .fill(BARRE_OUTILS)
                .inner_margin(Margin::symmetric(6, 8)),
        )
        .show(ui, |ui| {
            ui.visuals_mut().override_text_color = Some(TEXTE_BARRE_OUTILS);
            ui.vertical_centered(|ui| {
                for module in Module::TOUS {
                    let actif = etat.module == module;
                    let reponse = ui
                        .add(gros_bouton(icone(module), actif))
                        .on_hover_text(etat.i18n.t(module.cle()));
                    if reponse.clicked() {
                        sortie.push(Message::ChoisirModule(module));
                    }
                }
            });
            ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                // Du bas vers le haut : Info, Paramètres, Outils.
                ui.menu_button(RichText::new("ℹ").size(22.0), |ui| {
                    if ui.button(etat.i18n.t("gui.help.about")).clicked() {
                        sortie.push(Message::Notifier(
                            crate::etat::Niveau::Info,
                            etat.i18n.t("gui.about.title"),
                        ));
                        ui.close();
                    }
                })
                .response
                .on_hover_text(etat.i18n.t("Toolbar.help"));
                if ui
                    .add(gros_bouton("⚙", false))
                    .on_hover_text(etat.i18n.t("Toolbar.settings"))
                    .clicked()
                {
                    sortie.push(Message::OuvrirDialogue(TypeDialogue::Reglages));
                }
                ui.menu_button(RichText::new("🔧").size(22.0), |ui| {
                    if ui.button(etat.i18n.t("gui.tools.messages")).clicked() {
                        sortie.push(Message::OuvrirDialogue(TypeDialogue::Messages));
                        ui.close();
                    }
                })
                .response
                .on_hover_text(etat.i18n.t("Toolbar.tools"));
            });
        });
}
