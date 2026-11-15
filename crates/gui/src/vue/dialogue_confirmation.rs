//! Incrustation modale de confirmation (voile assombri) avec « Ne pas afficher ce message à
//! l'avenir » ; Entrée = confirmer, Échap = annuler.

use super::Sortie;
use crate::dialogues::EtatConfirmation;
use crate::etat::{ActionDialogue, Etat, Message};
use crate::polices;
use crate::theme::ORANGE;
use eframe::egui::{self, Align, Color32, Key, Layout, Modal, RichText};

pub fn afficher(ctx: &egui::Context, etat: &Etat, c: &EtatConfirmation, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let reponse = Modal::new(egui::Id::unique("dlg_confirmation"))
        .backdrop_color(Color32::from_black_alpha(140))
        .show(ctx, |ui| {
            ui.set_width(440.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(polices::AVERTISSEMENT)
                        .size(22.0)
                        .color(ORANGE),
                );
                ui.heading(&c.titre);
            });
            ui.separator();
            ui.label(&c.texte);
            ui.add_space(8.0);
            if c.masquable {
                let mut v = c.ne_plus_demander;
                if ui.checkbox(&mut v, i.t("app.donotshowagain")).changed() {
                    sortie.push(Message::Dialogue(
                        ActionDialogue::ConfirmationNePlusDemander(v),
                    ));
                }
            }
            ui.separator();
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(i.t("Button.ok")).clicked() {
                    sortie.push(Message::Confirmer);
                }
                if ui.button(i.t("Button.cancel")).clicked() {
                    sortie.push(Message::FermerDialogue);
                }
            });
        });
    if ctx.input_mut(|inp| inp.consume_key(egui::Modifiers::NONE, Key::Enter)) {
        sortie.push(Message::Confirmer);
    } else if reponse.should_close() {
        sortie.push(Message::FermerDialogue);
    }
}
