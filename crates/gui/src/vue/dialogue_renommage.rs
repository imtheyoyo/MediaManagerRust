//! Aperçu du renommage (maquette 5.2) : table des films, détail dossier/fichiers (ancien vers
//! nouveau), conflits du plan calculé par les services.

use super::Sortie;
use crate::etat::{ActionDialogue, Etat, EtatApercu, Message};
use crate::renommage::LigneFichier;
use crate::theme::{ORANGE, ROUGE, VERT};
use eframe::egui::{self, Align, Layout, Modal, RichText, Sense, Vec2};
use egui_extras::{Column, TableBuilder};

pub fn afficher(ctx: &egui::Context, etat: &Etat, d: &EtatApercu, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let apercus = &d.apercus;
    let reponse = Modal::new(egui::Id::unique("dlg_renommage")).show(ctx, |ui| {
        ui.set_min_size(Vec2::new(900.0, 460.0));
        ui.heading(i.t("renamer.title"));
        ui.horizontal_wrapped(|ui| {
            let sv = &etat.services;
            if d.series.is_some() {
                ui.label(RichText::new(i.t("Settings.renamer.folder")).strong());
                ui.monospace(&sv.gabarit_dossier_serie);
                ui.add_space(12.0);
                ui.label(RichText::new(i.t("gui.renamer.seasonfolder")).strong());
                ui.monospace(&sv.gabarit_dossier_saison);
                ui.add_space(12.0);
                ui.label(RichText::new(i.t("Settings.renamer.file")).strong());
                ui.monospace(&sv.gabarit_fichier_episode);
            } else {
                ui.label(RichText::new(i.t("Settings.renamer.folder")).strong());
                ui.monospace(&sv.gabarit_dossier_film);
                ui.add_space(12.0);
                ui.label(RichText::new(i.t("Settings.renamer.file")).strong());
                ui.monospace(&sv.gabarit_fichier_film);
            }
        });
        ui.separator();
        if let Some(e) = &d.erreur {
            ui.colored_label(ROUGE, e);
        } else if apercus.is_empty() {
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
                            ui.strong(i.t(if d.series.is_some() {
                                "gui.renamer.element"
                            } else {
                                "renamer.film"
                            }));
                        });
                    })
                    .body(|body| {
                        body.rows(20.0, apercus.len(), |mut ligne| {
                            let a = &apercus[ligne.index()];
                            ligne.set_selected(d.film_actif == Some(a.id));
                            ligne.col(|ui| {
                                if a.probleme() {
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
            super::separateur_vertical(ui, 340.0);
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
                        let couleur = if a.probleme() { ROUGE } else { VERT };
                        ui.label(RichText::new(&a.nouveau_dossier).color(couleur));
                        ui.end_row();
                    });
                for c in &a.conflits {
                    ui.colored_label(ROUGE, format!("⚠ {c}"));
                }
                for c in &a.avertissements {
                    ui.colored_label(ORANGE, c);
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
                                ui.label(&l.type_fichier);
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
                let actif = apercus.iter().any(|a| a.executable());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commandes::Commande;
    use crate::etat::{Dialogue, TypeDialogue};
    use eframe::egui::{Pos2, RawInput, Rect, epaint::Shape};

    /// Rend `images` images du dialogue ouvert dans `etat` ; renvoie le nombre de textes dessinés
    /// et le rectangle de la fenêtre à la dernière image.
    fn rendre(etat: &Etat, id: &'static str, ecran: Vec2, images: usize) -> (usize, Rect) {
        let ctx = egui::Context::default();
        ctx.set_visuals(crate::theme::visuals(etat.reglages.theme));
        let mut nb = 0;
        let mut sortie = Vec::new();
        for _ in 0..images {
            let entree = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, ecran)),
                ..Default::default()
            };
            let mut out = ctx.run_ui(entree, |ui| match &etat.dialogue {
                Some(Dialogue::ApercuRenommage(d)) => afficher(ui.ctx(), etat, d, &mut sortie),
                Some(Dialogue::Recherche(d)) => {
                    let mut images =
                        crate::images::GestionnaireImages::new(std::sync::Arc::new(|| {}));
                    crate::vue::dialogue_recherche::afficher(
                        ui.ctx(),
                        etat,
                        &mut images,
                        d,
                        &mut sortie,
                    )
                }
                _ => panic!("dialogue inattendu"),
            });
            out.textures_delta.clear();
            nb = out
                .shapes
                .iter()
                .filter(|c| matches!(&c.shape, Shape::Text(_)))
                .count();
        }
        let rect = ctx
            .memory(|m| m.area_rect(egui::Id::unique(id)))
            .expect("fenêtre affichée");
        (nb, rect)
    }

    fn verifier_stable(etat: &Etat, id: &'static str) {
        for ecran in [
            Vec2::new(1914.0, 1026.0),
            Vec2::new(1280.0, 800.0),
            Vec2::new(1050.0, 700.0),
        ] {
            let (nb, r4) = rendre(etat, id, ecran, 4);
            let (_, r20) = rendre(etat, id, ecran, 20);
            assert!(nb > 10, "{id} : fenêtre vide ({nb} textes) à {ecran:?}");
            assert_eq!(r4.size(), r20.size(), "{id} grandit à {ecran:?}");
            let e = Rect::from_min_size(Pos2::ZERO, ecran);
            assert!(
                r20.width() <= ecran.x && r20.height() <= ecran.y,
                "{id} : {r20:?} plus grande que {e:?}"
            );
        }
    }

    #[test]
    fn apercu_du_renommage_taille_stable() {
        let mut etat = Etat::nouveau();
        let id = etat.films[0].id;
        etat.appliquer(Message::Selectionner {
            id,
            ctrl: false,
            maj: false,
        });
        etat.appliquer(Message::OuvrirDialogue(TypeDialogue::ApercuRenommage));
        verifier_stable(&etat, "dlg_renommage");
    }

    #[test]
    fn recherche_manuelle_taille_stable() {
        let mut etat = Etat::nouveau();
        let id = etat.films[0].id;
        etat.appliquer(Message::Selectionner {
            id,
            ctrl: false,
            maj: false,
        });
        etat.appliquer(Message::Commande(Commande::ScrapeInteractif));
        verifier_stable(&etat, "dlg_recherche");
    }
}
