//! Barre d'état (22 px) : libellé de la tâche active, progression (indéterminée si <= 1 unité), bouton Stop.

use super::Sortie;
use crate::etat::{Etat, Message};
use eframe::egui::{self, Align, Frame, Layout, Margin, Panel, ProgressBar};

pub fn afficher(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    Panel::bottom("barre_etat")
        .exact_size(24.0)
        .frame(
            Frame::new()
                .fill(ui.visuals().panel_fill)
                .inner_margin(Margin::symmetric(8, 2)),
        )
        .show(ui, |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                match etat.tache_active() {
                    Some(tache) => {
                        if tache.principale
                            && ui
                                .small_button("■")
                                .on_hover_text(etat.i18n.t("Button.stop"))
                                .clicked()
                        {
                            sortie.push(Message::ArreterTache(tache.id));
                        }
                        let barre = if tache.indeterminee() {
                            ProgressBar::new(0.0).animate(true)
                        } else {
                            ProgressBar::new(tache.fraction())
                        };
                        ui.add(barre.desired_width(100.0).desired_height(12.0));
                        ui.label(etat.i18n.t(tache.cle_nom));
                    }
                    None => {
                        ui.label(etat.i18n.t("gui.status.idle"));
                    }
                }
            });
        });
}
