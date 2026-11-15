//! Dialogue « Trouver les manquants » : films d'une saga absents de la bibliothèque, ou
//! épisodes d'une série sans fichier local. Liste par groupe et fiche de l'élément choisi.

use super::Sortie;
use crate::dialogues::EtatManquants;
use crate::etat::{ActionDialogue, Etat, Message};
use crate::polices;
use crate::theme::ROUGE;
use eframe::egui::{self, Align, Layout, Modal, RichText, ScrollArea, Vec2};

pub fn afficher(ctx: &egui::Context, etat: &Etat, m: &EtatManquants, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let reponse = Modal::new(egui::Id::unique("dlg_manquants")).show(ctx, |ui| {
        ui.set_min_size(Vec2::new(720.0, 380.0));
        ui.heading(i.t(m.cle_titre));
        ui.separator();
        if m.en_cours {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(i.t("chooser.searching"));
            });
        }
        if let Some(e) = &m.erreur {
            ui.label(RichText::new(format!("{} {e}", polices::AVERTISSEMENT)).color(ROUGE));
        }
        if !m.en_cours && m.total() == 0 && m.erreur.is_none() {
            ui.label(RichText::new(i.t("gui.missing.none")).weak());
        }
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(380.0);
                ScrollArea::vertical()
                    .id_salt("manquants_liste")
                    .max_height(260.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (g, groupe) in m.groupes.iter().enumerate() {
                            if groupe.elements.is_empty() {
                                continue;
                            }
                            ui.label(RichText::new(&groupe.titre).strong());
                            for (n, x) in groupe.elements.iter().enumerate() {
                                let texte = if x.detail.is_empty() {
                                    x.titre.clone()
                                } else {
                                    format!("{} - {}", x.detail, x.titre)
                                };
                                if ui
                                    .selectable_label(m.selection == Some((g, n)), texte)
                                    .clicked()
                                {
                                    sortie.push(Message::Dialogue(
                                        ActionDialogue::ManquantsChoisir(g, n),
                                    ));
                                }
                            }
                            ui.add_space(6.0);
                        }
                    });
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.set_min_width(280.0);
                if let Some(x) = m.element_choisi() {
                    ui.label(RichText::new(&x.titre).heading().strong());
                    if !x.titre_original.is_empty() {
                        ui.label(
                            RichText::new(format!(
                                "{} : {}",
                                i.t("gui.original"),
                                x.titre_original
                            ))
                            .weak(),
                        );
                    }
                    if !x.detail.is_empty() {
                        ui.label(&x.detail);
                    }
                    ui.add_space(6.0);
                    ScrollArea::vertical()
                        .id_salt("manquants_resume")
                        .max_height(200.0)
                        .show(ui, |ui| {
                            ui.label(&x.resume);
                        });
                }
            });
        });
        ui.separator();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button(i.t("Button.close")).clicked() {
                sortie.push(Message::FermerDialogue);
            }
            if m.total() > 0 {
                ui.label(
                    RichText::new(i.t_pluriel("gui.missing.count", m.total() as u64, &[])).weak(),
                );
            }
        });
    });
    if reponse.should_close() {
        sortie.push(Message::FermerDialogue);
    }
}
