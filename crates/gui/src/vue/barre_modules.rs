//! Barre de modules verticale (gauche) : Films / Collections / Séries (module actif en orange) ;
//! Outils, Paramètres, Info en bas.

use super::Sortie;
use crate::etat::{Etat, Message, Module, Niveau, TypeDialogue};
use crate::icones::{self, Icone};
use crate::theme::{FOND_BARRES, ICONE_REPOS, ORANGE_ACTIF, TEXTE_BARRE_OUTILS};
use eframe::egui::containers::menu::MenuButton;
use eframe::egui::{self, Align, Button, Frame, Layout, Margin, Panel, Vec2};

/// Largeur de la barre de modules.
pub const LARGEUR: f32 = 44.0;
const COTE_ICONE: f32 = 26.0;

/// Icône d'un module.
pub fn icone(module: Module) -> Icone {
    match module {
        Module::Films => Icone::Films,
        Module::Collections => Icone::Collections,
        Module::Series => Icone::Series,
    }
}

/// Teinte d'une icône de module (orange si actif).
pub fn teinte(actif: bool) -> egui::Color32 {
    if actif { ORANGE_ACTIF } else { ICONE_REPOS }
}

fn bouton(icone: Icone, actif: bool) -> Button<'static> {
    Button::image(icones::image_teintee(icone, COTE_ICONE, teinte(actif)))
        .frame_when_inactive(false)
        .min_size(Vec2::splat(LARGEUR - 6.0))
}

pub fn afficher(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    Panel::left("barre_modules")
        .exact_size(LARGEUR)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(FOND_BARRES)
                .inner_margin(Margin::symmetric(3, 10)),
        )
        .show(ui, |ui| {
            ui.visuals_mut().override_text_color = Some(TEXTE_BARRE_OUTILS);
            ui.spacing_mut().item_spacing.y = 14.0;
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                for module in Module::TOUS {
                    let actif = etat.module == module;
                    let reponse = ui
                        .add(bouton(icone(module), actif))
                        .on_hover_text(etat.i18n.t(module.cle()));
                    if reponse.clicked() {
                        sortie.push(Message::ChoisirModule(module));
                    }
                }
            });
            ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                // Du bas vers le haut : Info, Paramètres, Outils.
                MenuButton::from_button(bouton(Icone::Info, false))
                    .ui(ui, |ui| {
                        if ui.button(etat.i18n.t("gui.update.check")).clicked() {
                            sortie.push(Message::RechercherMiseAJour { manuel: true });
                            ui.close();
                        }
                        if ui.button(etat.i18n.t("gui.help.about")).clicked() {
                            sortie.push(Message::Notifier(
                                Niveau::Info,
                                etat.i18n.t("gui.about.title"),
                            ));
                            ui.close();
                        }
                    })
                    .0
                    .on_hover_text(etat.i18n.t("Toolbar.help"));
                if ui
                    .add(bouton(Icone::Reglages, false))
                    .on_hover_text(etat.i18n.t("Toolbar.settings"))
                    .clicked()
                {
                    sortie.push(Message::OuvrirDialogue(TypeDialogue::Reglages));
                }
                MenuButton::from_button(bouton(Icone::Outils, false))
                    .ui(ui, |ui| {
                        if ui.button(etat.i18n.t("gui.tools.messages")).clicked() {
                            sortie.push(Message::OuvrirDialogue(TypeDialogue::Messages));
                            ui.close();
                        }
                    })
                    .0
                    .on_hover_text(etat.i18n.t("Toolbar.tools"));
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_actif_en_orange() {
        assert_eq!(teinte(true), ORANGE_ACTIF);
        assert_eq!(teinte(false), ICONE_REPOS);
        assert_eq!(icone(Module::Series), Icone::Series);
        for m in Module::TOUS {
            assert!(icone(m).teintable());
        }
    }
}
