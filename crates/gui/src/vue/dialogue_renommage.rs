//! Aperçu du renommage (maquette 5.2) : profil, table des films, détail dossier/fichiers.

use super::Sortie;
use crate::etat::{ActionDialogue, Etat, EtatApercu, Message};
use crate::renommage::LigneFichier;
use crate::theme::{ORANGE, ROUGE, VERT};
use eframe::egui::{self, Align, Layout, Modal, RichText, Sense, Vec2};
use egui_extras::{Column, TableBuilder};

pub fn afficher(ctx: &egui::Context, etat: &Etat, d: &EtatApercu, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let apercus = etat.apercu_courant();
    let reponse = Modal::new(egui::Id::unique("dlg_renommage")).show(ctx, |ui| {
        ui.set_min_size(Vec2::new(900.0, 460.0));
        ui.heading(i.t("renamer.title"));
        ui.horizontal(|ui| {
            ui.label(i.t("Settings.renamer.profile"));
            let nom = etat
                .reglages
                .profils
                .get(d.profil)
                .map(|p| p.nom.as_str())
                .unwrap_or("");
            egui::ComboBox::from_id_salt("profil_renommage")
                .selected_text(nom)
                .show_ui(ui, |ui| {
                    for (n, p) in etat.reglages.profils.iter().enumerate() {
                        if ui.selectable_label(d.profil == n, &p.nom).clicked() {
                            sortie.push(Message::Dialogue(ActionDialogue::ApercuProfil(n)));
                        }
                    }
                });
        });
        ui.separator();
        if apercus.is_empty() {
            ui.label(i.t("renamer.nothing"));
        }
        ui.horizontal_top(|ui| {
            // Table des films (gauche)
            ui.vertical(|ui| {
                ui.set_width(280.0);
                TableBuilder::new(ui)
                    .id_salt("table_apercu_films")
                    .striped(true)
                    .sense(Sense::click())
                    .max_scroll_height(330.0)
                    .column(Column::exact(24.0))
                    .column(Column::remainder().clip(true))
                    .header(22.0, |mut h| {
                        h.col(|ui| {
                            ui.label("⚠");
                        });
                        h.col(|ui| {
                            ui.strong(i.t("renamer.film"));
                        });
                    })
                    .body(|body| {
                        body.rows(20.0, apercus.len(), |mut ligne| {
                            let a = &apercus[ligne.index()];
                            ligne.set_selected(d.film_actif == Some(a.id));
                            ligne.col(|ui| {
                                if a.probleme {
                                    ui.label(RichText::new("⚠").color(ORANGE))
                                        .on_hover_text(i.t("renamer.problem"));
                                }
                            });
                            ligne.col(|ui| {
                                ui.label(&a.titre);
                            });
                            if ligne.response().clicked() {
                                sortie.push(Message::Dialogue(ActionDialogue::ApercuFilm(a.id)));
                            }
                        });
                    });
            });
            ui.separator();
            // Détail du film (droite)
            ui.vertical(|ui| {
                ui.set_min_width(560.0);
                let actif = apercus
                    .iter()
                    .find(|a| Some(a.id) == d.film_actif)
                    .or(apercus.first());
                let Some(a) = actif else { return };
                ui.label(RichText::new(&a.titre).heading().strong());
                egui::Grid::new("grille_apercu")
                    .num_columns(2)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new(i.t("metatag.datasource")).strong());
                        ui.label(&a.source);
                        ui.end_row();
                        ui.label(RichText::new(i.t("renamer.oldfolder")).strong());
                        ui.label(&a.ancien_dossier);
                        ui.end_row();
                        ui.label(RichText::new(i.t("renamer.newfolder")).strong());
                        let couleur = if a.probleme { ROUGE } else { VERT };
                        ui.label(RichText::new(&a.nouveau_dossier).color(couleur));
                        ui.end_row();
                    });
                if a.probleme {
                    ui.colored_label(ROUGE, i.t("renamer.duplicate"));
                }
                ui.add_space(6.0);
                let lignes: Vec<&LigneFichier> = a
                    .fichiers
                    .iter()
                    .filter(|l| !(d.masquer_inchanges && l.inchange()))
                    .collect();
                TableBuilder::new(ui)
                    .id_salt("table_apercu_fichiers")
                    .striped(true)
                    .max_scroll_height(240.0)
                    .column(Column::exact(80.0))
                    .column(Column::remainder().clip(true))
                    .column(Column::remainder().clip(true))
                    .header(22.0, |mut h| {
                        for cle in ["renamer.type", "renamer.oldfiles", "renamer.newfiles"] {
                            h.col(|ui| {
                                ui.strong(i.t(cle));
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(20.0, lignes.len(), |mut ligne| {
                            let l = lignes[ligne.index()];
                            ligne.col(|ui| {
                                ui.label(l.type_fichier.libelle());
                            });
                            ligne.col(|ui| {
                                ui.label(&l.ancien);
                            });
                            ligne.col(|ui| {
                                if l.inchange() {
                                    ui.label(
                                        RichText::new(format!(
                                            "{} {}",
                                            l.nouveau,
                                            i.t("renamer.unchanged")
                                        ))
                                        .weak(),
                                    );
                                } else {
                                    ui.label(RichText::new(&l.nouveau).color(VERT));
                                }
                            });
                        });
                    });
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            let mut masquer = d.masquer_inchanges;
            if ui
                .checkbox(&mut masquer, i.t("renamer.hideunchanged"))
                .changed()
            {
                sortie.push(Message::Dialogue(ActionDialogue::ApercuMasquer(masquer)));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(i.t("Button.close")).clicked() {
                    sortie.push(Message::FermerDialogue);
                }
                let actif = apercus.iter().any(|a| !a.probleme && !a.rien_a_renommer());
                if ui
                    .add_enabled(actif, egui::Button::new(i.t("Button.rename")))
                    .clicked()
                {
                    sortie.push(Message::ApercuAppliquer);
                }
            });
        });
    });
    if reponse.should_close() {
        sortie.push(Message::FermerDialogue);
    }
}
