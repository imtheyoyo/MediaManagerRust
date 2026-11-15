//! Historique des messages (cloche de la barre d'outils).

use super::Sortie;
use crate::etat::{Etat, Message, Niveau};
use crate::theme::{ORANGE, ROUGE};
use eframe::egui::{self, Modal, RichText, ScrollArea};

pub fn afficher(ctx: &egui::Context, etat: &Etat, sortie: &mut Sortie) {
    let reponse = Modal::new(egui::Id::unique("dlg_messages")).show(ctx, |ui| {
        ui.set_width(520.0);
        ui.heading(etat.i18n.t("gui.messages.title"));
        ui.separator();
        ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            if etat.notifications.is_empty() {
                ui.label(RichText::new(etat.i18n.t("gui.messages.empty")).weak());
            }
            for n in etat.notifications.iter().rev() {
                let (glyphe, couleur) = match n.niveau {
                    Niveau::Info => ("ℹ", ui.visuals().text_color()),
                    Niveau::Avertissement => ("⚠", ORANGE),
                    Niveau::Erreur => ("✖", ROUGE),
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(glyphe).color(couleur));
                    ui.label(&n.texte);
                });
            }
        });
        ui.separator();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(etat.i18n.t("Button.close")).clicked() {
                sortie.push(Message::FermerDialogue);
            }
        });
    });
    if reponse.should_close() {
        sortie.push(Message::FermerDialogue);
    }
}
