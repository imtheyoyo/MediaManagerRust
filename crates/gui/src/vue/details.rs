//! Panneau de détails à onglets (Détails, Distribution, Fichiers médias, Illustration, Bande annonce).

use super::Sortie;
use crate::etat::{Etat, Message, OngletDetails};
use crate::modele::{Film, gigaoctets};
use crate::theme::accent;
use eframe::egui::{
    self, CornerRadius, Frame, Margin, RichText, ScrollArea, Sense, Stroke, StrokeKind, Vec2,
};
use egui_extras::{Column, TableBuilder};

pub fn afficher(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    ui.horizontal(|ui| {
        for onglet in OngletDetails::TOUS {
            let actif = etat.onglet == onglet;
            let texte = RichText::new(etat.i18n.t(onglet.cle()));
            let texte = if actif {
                texte.strong().color(accent(etat.reglages.theme))
            } else {
                texte
            };
            if ui.selectable_label(actif, texte).clicked() {
                sortie.push(Message::ChoisirOnglet(onglet));
            }
        }
    });
    ui.separator();
    let Some(film) = etat.film_principal() else {
        ui.add_space(12.0);
        ui.label(RichText::new(etat.i18n.t("gui.selection.none")).weak());
        return;
    };
    match etat.onglet {
        OngletDetails::Details => onglet_details(ui, etat, film),
        OngletDetails::Distribution => onglet_distribution(ui, etat, film),
        OngletDetails::FichiersMedias => onglet_fichiers(ui, etat, film),
        OngletDetails::Illustration => onglet_illustration(ui, etat, film),
        OngletDetails::BandeAnnonce => onglet_bande_annonce(ui, etat, film),
    }
}

fn champ(ui: &mut egui::Ui, etat: &Etat, cle: &str, valeur: &str) {
    ui.label(RichText::new(etat.i18n.t(cle)).strong());
    ui.label(valeur);
    ui.end_row();
}

fn pastille(ui: &mut egui::Ui, texte: &str) {
    Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(Stroke::new(
            1.0,
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        ))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(texte).small());
        });
}

fn emplacement_image(ui: &mut egui::Ui, etat: &Etat, cle: &str, taille: Vec2) {
    let (rect, _) = ui.allocate_exact_size(taille, Sense::click());
    let peintre = ui.painter();
    peintre.rect_filled(rect, CornerRadius::same(6), ui.visuals().extreme_bg_color);
    peintre.rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        StrokeKind::Inside,
    );
    peintre.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        etat.i18n.t(cle),
        egui::FontId::proportional(12.0),
        ui.visuals().weak_text_color(),
    );
}

fn onglet_details(ui: &mut egui::Ui, etat: &Etat, film: &Film) {
    let i = &etat.i18n;
    ui.horizontal_top(|ui| {
        // Colonne gauche : pile d'illustrations (emplacements : les images viendront du service d'images).
        ui.vertical(|ui| {
            emplacement_image(ui, etat, "gui.artwork.poster", Vec2::new(120.0, 180.0));
            emplacement_image(ui, etat, "gui.artwork.fanart", Vec2::new(120.0, 68.0));
            emplacement_image(ui, etat, "gui.artwork.banner", Vec2::new(120.0, 24.0));
            emplacement_image(ui, etat, "gui.artwork.thumb", Vec2::new(120.0, 68.0));
        });
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&film.titre).heading().strong());
                if ui.button(format!("▶ {}", i.t("Button.play"))).clicked() {}
            });
            ui.label(
                RichText::new(format!("{} : {}", i.t("gui.original"), film.titre_original)).weak(),
            );
            ui.separator();
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("grille_details")
                        .num_columns(2)
                        .spacing([14.0, 4.0])
                        .show(ui, |ui| {
                            champ(ui, etat, "metatag.year", &film.annee.to_string());
                            champ(ui, etat, "metatag.releasedate", &film.date_sortie);
                            champ(ui, etat, "metatag.certification", &film.certification);
                            champ(
                                ui,
                                etat,
                                "metatag.runtime",
                                &i.t_args(
                                    "column.minutes",
                                    &[("count", &film.duree_min.to_string())],
                                ),
                            );
                            ui.label(RichText::new("IMDb").strong());
                            ui.hyperlink_to(
                                &film.imdb,
                                format!("https://www.imdb.com/title/{}", film.imdb),
                            );
                            ui.end_row();
                            ui.label(RichText::new("TMDB").strong());
                            ui.hyperlink_to(
                                &film.tmdb,
                                format!("https://www.themoviedb.org/movie/{}", film.tmdb),
                            );
                            ui.end_row();
                            champ(ui, etat, "metatag.genre", &film.genres.join(", "));
                            champ(ui, etat, "metatag.director", &film.realisateurs.join(", "));
                            champ(ui, etat, "metatag.production", &film.production.join(", "));
                            champ(ui, etat, "metatag.country", &film.pays.join(", "));
                            if let Some(saga) = &film.saga {
                                champ(ui, etat, "metatag.movieset", saga);
                            }
                            champ(ui, etat, "metatag.path", &film.chemin());
                            champ(ui, etat, "metatag.dateadded", &film.date_ajout);
                        });
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        for (source, valeur) in &film.notes_sources {
                            pastille(ui, &format!("{source} {}", i.decimal(*valeur as f64, 1)));
                        }
                    });
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        pastille(ui, &film.format_video());
                        pastille(ui, &film.codec_video);
                        if let Some(hdr) = &film.hdr {
                            pastille(ui, hdr);
                        }
                        pastille(ui, &film.audio);
                    });
                    ui.add_space(8.0);
                    ui.label(RichText::new(&film.slogan).italics());
                    ui.add_space(6.0);
                    ui.label(RichText::new(i.t("metatag.plot")).strong());
                    ui.label(&film.resume);
                });
        });
    });
}

fn onglet_distribution(ui: &mut egui::Ui, etat: &Etat, film: &Film) {
    TableBuilder::new(ui)
        .id_salt("table_distribution")
        .striped(true)
        .column(Column::remainder().at_least(120.0))
        .column(Column::remainder().at_least(120.0))
        .header(22.0, |mut h| {
            h.col(|ui| {
                ui.strong(etat.i18n.t("metatag.actor"));
            });
            h.col(|ui| {
                ui.strong(etat.i18n.t("metatag.role"));
            });
        })
        .body(|body| {
            body.rows(20.0, film.acteurs.len(), |mut ligne| {
                let (nom, role) = &film.acteurs[ligne.index()];
                ligne.col(|ui| {
                    ui.label(nom);
                });
                ligne.col(|ui| {
                    ui.label(role);
                });
            });
        });
}

fn onglet_fichiers(ui: &mut egui::Ui, etat: &Etat, film: &Film) {
    let i = &etat.i18n;
    egui::Grid::new("grille_fichiers")
        .num_columns(2)
        .spacing([14.0, 4.0])
        .show(ui, |ui| {
            champ(ui, etat, "metatag.path", &film.chemin());
            champ(ui, etat, "metatag.datasource", &film.source);
            champ(ui, etat, "metatag.dateadded", &film.date_ajout);
            champ(ui, etat, "gui.media.resolution", &film.format_video());
            champ(ui, etat, "metatag.videocodec", &film.codec_video);
            champ(ui, etat, "metatag.audio", &film.audio);
            champ(
                ui,
                etat,
                "metatag.watched",
                if film.vu {
                    i.t("Button.yes")
                } else {
                    i.t("Button.no")
                }
                .as_str(),
            );
        });
    ui.add_space(8.0);
    ui.label(RichText::new(i.t("gui.media.files")).strong());
    TableBuilder::new(ui)
        .id_salt("table_fichiers")
        .striped(true)
        .column(Column::initial(80.0))
        .column(Column::remainder().clip(true))
        .column(Column::initial(90.0))
        .header(22.0, |mut h| {
            for cle in ["renamer.type", "metatag.filename", "metatag.filesize"] {
                h.col(|ui| {
                    ui.strong(i.t(cle));
                });
            }
        })
        .body(|body| {
            body.rows(20.0, film.fichiers.len(), |mut ligne| {
                let f = &film.fichiers[ligne.index()];
                ligne.col(|ui| {
                    ui.label(f.type_fichier.libelle());
                });
                ligne.col(|ui| {
                    ui.label(&f.nom);
                });
                ligne.col(|ui| {
                    ui.label(format!(
                        "{} {}",
                        i.decimal(gigaoctets(f.taille_octets), 3),
                        i.t("gui.unit.gigabyte")
                    ));
                });
            });
        });
}

fn onglet_illustration(ui: &mut egui::Ui, etat: &Etat, _film: &Film) {
    ui.horizontal_wrapped(|ui| {
        emplacement_image(ui, etat, "gui.artwork.poster", Vec2::new(140.0, 210.0));
        emplacement_image(ui, etat, "gui.artwork.fanart", Vec2::new(280.0, 158.0));
        emplacement_image(ui, etat, "gui.artwork.banner", Vec2::new(280.0, 52.0));
        emplacement_image(ui, etat, "gui.artwork.thumb", Vec2::new(200.0, 112.0));
    });
}

fn onglet_bande_annonce(ui: &mut egui::Ui, etat: &Etat, film: &Film) {
    if film.bandes_annonces.is_empty() {
        ui.label(RichText::new(etat.i18n.t("gui.trailer.none")).weak());
        return;
    }
    for b in &film.bandes_annonces {
        ui.horizontal(|ui| {
            ui.label(b);
            if ui
                .button(format!("▶ {}", etat.i18n.t("Button.play")))
                .clicked()
            {}
        });
    }
}
