//! Module Collections : arbre saga → films présenté en tableau (triangles d'expansion,
//! colonnes Films / Année), totaux, panneau de détails de la saga ou du film sélectionné.

use super::Sortie;
use super::details::{
    afficher_film, bandeau_onglets, colonne_images, largeur_libelles, ligne, taille_lisible, titres,
};
use super::tableau::{
    HAUTEUR_ENTETE, HAUTEUR_LIGNE, droite, entete_texte, filets, ligne_recherche, onglet_module,
};
use crate::collections::{NoeudSaga, films_de_saga, lignes_sagas, sagas_filtrees, taille_saga};
use crate::etat::{Etat, Message};
use crate::icones::peindre_triangle;
use crate::images::GestionnaireImages;
use crate::source_donnees::SagaAffichage;
use eframe::egui::{
    self, Align, Frame, Label, Layout, Margin, Panel, RichText, ScrollArea, Sense, Vec2, vec2,
};
use egui_extras::{Column, TableBuilder};

/// Largeur des colonnes « Films » et « Année ».
const LARGEUR_NOMBRE: f32 = 56.0;

/// Panneau maître du module Collections.
pub fn afficher(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    onglet_module(ui, etat);
    Frame::new()
        .inner_margin(Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ligne_recherche(ui, etat, sortie);
            ui.add_space(6.0);
            Panel::bottom("totaux_collections")
                .exact_size(24.0)
                .show_separator_line(false)
                .frame(Frame::new().inner_margin(Margin::symmetric(2, 2)))
                .show(ui, |ui| totaux(ui, etat));
            if etat.sagas.is_empty() {
                ui.add_space(12.0);
                ui.add(Label::new(RichText::new(etat.i18n.t("gui.moviesets.empty")).weak()).wrap());
                return;
            }
            table(ui, etat, sortie);
        });
}

fn totaux(ui: &mut egui::Ui, etat: &Etat) {
    let i = &etat.i18n;
    ui.horizontal(|ui| {
        let filtrees = sagas_filtrees(&etat.sagas, &etat.films, &etat.recherche).len();
        ui.label(i.t_args(
            "gui.moviesets.count",
            &[
                ("filtered", filtrees.to_string().as_str()),
                ("total", etat.sagas.len().to_string().as_str()),
            ],
        ));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let saga = etat
                .noeud_saga
                .as_ref()
                .and_then(|n| etat.saga(n.id_saga()));
            if let Some(s) = saga {
                ui.label(resume_taille(etat, s));
            }
        });
    });
}

/// « 3 film(s) (12,40 G) » pour une saga.
fn resume_taille(etat: &Etat, s: &SagaAffichage) -> String {
    let n = films_de_saga(&etat.films, s).len().to_string();
    let taille = taille_lisible(etat, taille_saga(&etat.films, s));
    etat.i18n.t_args(
        "gui.moviesets.size",
        &[("count", n.as_str()), ("size", taille.as_str())],
    )
}

fn table(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    let lignes = lignes_sagas(
        &etat.sagas,
        &etat.films,
        &etat.recherche,
        &etat.sagas_deployees,
    );
    let i = &etat.i18n;
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    TableBuilder::new(ui)
        .id_salt("tableau_collections")
        .striped(false)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::remainder().at_least(160.0).clip(true))
        .column(Column::exact(LARGEUR_NOMBRE))
        .column(Column::exact(LARGEUR_NOMBRE))
        .header(HAUTEUR_ENTETE, |mut h| {
            h.col(|ui| {
                entete_texte(ui, i.t("metatag.title"));
            });
            h.col(|ui| {
                entete_texte(ui, i.t("gui.moviesets.films"));
            });
            h.col(|ui| {
                entete_texte(ui, i.t("metatag.year"));
            });
        })
        .body(|body| {
            body.rows(HAUTEUR_LIGNE, lignes.len(), |mut rang| {
                let l = &lignes[rang.index()];
                rang.set_selected(etat.noeud_saga.as_ref() == Some(&l.noeud));
                rang.col(|ui| {
                    filets(ui);
                    ui.add_space(4.0 + 16.0 * l.profondeur as f32);
                    let (rect, rep) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::click());
                    if let Some(ouvert) = l.deploiement {
                        let couleur = ui.visuals().text_color();
                        peindre_triangle(ui.painter(), rect.center(), 8.0, ouvert, couleur);
                        if rep.clicked() {
                            sortie.push(Message::BasculerSaga(l.noeud.id_saga().to_string()));
                        }
                    }
                    ui.add_space(3.0);
                    ui.add(Label::new(&l.titre).truncate());
                });
                rang.col(|ui| {
                    filets(ui);
                    if let Some(n) = l.films {
                        droite(ui, n.to_string());
                    }
                });
                rang.col(|ui| {
                    filets(ui);
                    if l.annee > 0 {
                        droite(ui, l.annee.to_string());
                    }
                });
                let reponse = rang.response();
                if reponse.clicked() {
                    sortie.push(Message::SelectionnerNoeudSaga(l.noeud.clone()));
                }
                if reponse.double_clicked() && l.deploiement.is_some() {
                    sortie.push(Message::BasculerSaga(l.noeud.id_saga().to_string()));
                }
            });
        });
}

/// Panneau de détails du module Collections : film (cinq onglets) ou saga.
pub fn afficher_details(
    ui: &mut egui::Ui,
    etat: &Etat,
    images: &mut GestionnaireImages,
    sortie: &mut Sortie,
) {
    match &etat.noeud_saga {
        Some(NoeudSaga::Film(..)) => {
            afficher_film(ui, etat, images, etat.film_collection(), sortie)
        }
        noeud => {
            let i = &etat.i18n;
            bandeau_onglets(ui, "onglets_details_saga", &[i.t("metatag.details")], 0);
            let saga = noeud.as_ref().and_then(|n| etat.saga(n.id_saga()));
            Frame::new()
                .inner_margin(Margin {
                    left: 10,
                    right: 10,
                    top: 8,
                    bottom: 6,
                })
                .show(ui, |ui| match saga {
                    Some(s) => details_saga(ui, etat, images, s, sortie),
                    None => {
                        ui.add_space(12.0);
                        ui.label(RichText::new(i.t("app.nothingselected")).weak());
                    }
                });
        }
    }
}

fn details_saga(
    ui: &mut egui::Ui,
    etat: &Etat,
    images: &mut GestionnaireImages,
    saga: &SagaAffichage,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    ScrollArea::vertical()
        .id_salt("defilement_details_saga")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                let chemins = [
                    saga.affiche.clone(),
                    saga.fanart.clone(),
                    saga.banniere.clone(),
                ];
                colonne_images(ui, etat, images, chemins, sortie);
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    titres(ui, &saga.titre, &saga.titre_original);
                    let libelles = vec![i.t("gui.moviesets.films"), i.t("metatag.filesize")];
                    let l = largeur_libelles(ui, &libelles);
                    let films = films_de_saga(&etat.films, saga);
                    egui::Grid::new("grille_saga")
                        .num_columns(2)
                        .min_col_width(l)
                        .spacing([14.0, 3.0])
                        .show(ui, |ui| {
                            ligne(ui, &libelles[0], &films.len().to_string());
                            ligne(
                                ui,
                                &libelles[1],
                                &taille_lisible(etat, taille_saga(&etat.films, saga)),
                            );
                        });
                    ui.add_space(12.0);
                    ui.label(RichText::new(i.t("metatag.plot")).strong());
                    ui.add(Label::new(&saga.resume).wrap());
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(6.0);
                    ui.label(RichText::new(i.t("gui.moviesets.movies")).strong());
                    ui.add_space(2.0);
                    egui::Grid::new("grille_films_saga")
                        .num_columns(3)
                        .striped(true)
                        .spacing([14.0, 4.0])
                        .show(ui, |ui| {
                            for f in films {
                                let r = ui
                                    .add(Label::new(&f.titre).sense(Sense::click()))
                                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                                if r.clicked() {
                                    sortie.push(Message::SelectionnerNoeudSaga(NoeudSaga::Film(
                                        saga.id.clone(),
                                        f.id,
                                    )));
                                }
                                ui.label(if f.annee > 0 {
                                    f.annee.to_string()
                                } else {
                                    String::new()
                                });
                                ui.label(
                                    RichText::new(taille_lisible(etat, f.taille_octets)).weak(),
                                );
                                ui.end_row();
                            }
                        });
                });
            });
        });
}
