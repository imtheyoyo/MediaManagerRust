//! Dialogue de recherche d'extracteur et de choix d'un résultat (§2.7.1, maquette 5.3) :
//! extracteur, texte pré-rempli, langue ; résultats (titre, année, score) et fiche détaillée.

use super::{Sortie, image_ou_emplacement};
use crate::dialogues::{ActionRecherche, EtatRecherche};
use crate::etat::{ActionDialogue, Etat, Message};
use crate::i18n::Langue;
use crate::images::GestionnaireImages;
use crate::polices;
use crate::theme::{ORANGE, ROUGE, VERT};
use eframe::egui::{
    self, Align, Button, ComboBox, Key, Layout, Modal, ProgressBar, RichText, ScrollArea, Sense,
    TextEdit, Vec2,
};
use egui_extras::{Column, TableBuilder};

/// Largeur de la liste des résultats (miniature, titre, année, score).
const LARGEUR_LISTE: f32 = 440.0;
const LARGEUR_MINIATURE: f32 = 40.0;
const HAUTEUR_MINIATURE: f32 = 60.0;
const HAUTEUR_LIGNE: f32 = 66.0;

fn envoyer(sortie: &mut Sortie, a: ActionRecherche) {
    sortie.push(Message::Dialogue(ActionDialogue::Recherche(a)));
}

fn couleur_score(score: f32) -> egui::Color32 {
    if score >= 0.75 {
        VERT
    } else if score >= 0.5 {
        ORANGE
    } else {
        ROUGE
    }
}

pub fn afficher(
    ctx: &egui::Context,
    etat: &Etat,
    images: &mut GestionnaireImages,
    r: &EtatRecherche,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    let reponse = Modal::new(egui::Id::unique("dlg_recherche")).show(ctx, |ui| {
        ui.set_min_size(Vec2::new(900.0, 500.0));
        ui.horizontal(|ui| {
            ui.heading(i.t(if r.serie.is_some() {
                "chooser.titletv"
            } else {
                "chooser.title"
            }));
            if let Some(id) = &r.serie {
                if let Some(s) = etat.series.iter().find(|s| s.id == *id) {
                    ui.label(RichText::new(format!("- {}", s.titre)).weak());
                }
            } else if let Some(f) = etat.film(r.film_id) {
                ui.label(RichText::new(format!("- {}", f.dossier)).weak());
            }
            if r.total > 1 {
                ui.label(RichText::new(format!("({} / {})", r.position(), r.total)).weak());
            }
        });
        egui::Grid::new("rech_entete")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                ui.label(i.t("chooser.scraper"));
                ui.horizontal(|ui| {
                    ComboBox::from_id_salt("rech_extracteur")
                        .selected_text(&r.requete.extracteur)
                        .show_ui(ui, |ui| {
                            for x in etat.source.extracteurs() {
                                if ui.selectable_label(x == r.requete.extracteur, &x).clicked() {
                                    envoyer(sortie, ActionRecherche::Extracteur(x));
                                }
                            }
                        });
                    let mut texte = r.requete.texte.clone();
                    let champ = ui.add(TextEdit::singleline(&mut texte).desired_width(420.0));
                    if champ.changed() {
                        envoyer(sortie, ActionRecherche::Texte(texte));
                    }
                    let mut annee = r.requete.annee.map(|a| a.to_string()).unwrap_or_default();
                    if ui
                        .add(
                            TextEdit::singleline(&mut annee)
                                .hint_text(i.t("metatag.year"))
                                .desired_width(50.0),
                        )
                        .changed()
                    {
                        envoyer(sortie, ActionRecherche::Annee(annee.trim().parse().ok()));
                    }
                    let entree = champ.lost_focus() && ui.input(|inp| inp.key_pressed(Key::Enter));
                    if ui
                        .add(Button::new(format!(
                            "{} {}",
                            polices::RECHERCHE,
                            i.t("Button.search")
                        )))
                        .clicked()
                        || entree
                    {
                        envoyer(sortie, ActionRecherche::Lancer);
                    }
                });
                ui.end_row();
                ui.label(i.t("Settings.language"));
                ComboBox::from_id_salt("rech_langue")
                    .selected_text(&r.requete.langue)
                    .show_ui(ui, |ui| {
                        for l in Langue::TOUTES {
                            if ui
                                .selectable_label(r.requete.langue == l.code(), l.nom_natif())
                                .clicked()
                            {
                                envoyer(sortie, ActionRecherche::Langue(l.code().into()));
                            }
                        }
                    });
                ui.end_row();
            });
        ui.separator();
        ui.horizontal_top(|ui| {
            // Résultats (gauche)
            ui.vertical(|ui| {
                ui.set_width(LARGEUR_LISTE);
                ui.set_min_height(320.0);
                TableBuilder::new(ui)
                    .id_salt("rech_resultats")
                    .striped(true)
                    .sense(Sense::click())
                    .max_scroll_height(320.0)
                    .column(Column::exact(LARGEUR_MINIATURE + 8.0))
                    .column(Column::remainder().clip(true))
                    .column(Column::exact(48.0))
                    .column(Column::exact(70.0))
                    .header(22.0, |mut h| {
                        h.col(|_| {});
                        for cle in ["metatag.title", "metatag.year", "app.similarityscore"] {
                            h.col(|ui| {
                                ui.strong(i.t(cle)).on_hover_text(i.t(cle));
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(HAUTEUR_LIGNE, r.resultats.len(), |mut ligne| {
                            let n = ligne.index();
                            let x = &r.resultats[n];
                            ligne.set_selected(r.selection == Some(n));
                            ligne.col(|ui| {
                                // Jaquette en miniature (emplacement gris pendant le chargement).
                                image_ou_emplacement(
                                    ui,
                                    images,
                                    x.affiche.as_deref(),
                                    Vec2::new(LARGEUR_MINIATURE, HAUTEUR_MINIATURE),
                                    "",
                                );
                            });
                            ligne.col(|ui| {
                                ui.label(&x.titre);
                            });
                            ligne.col(|ui| {
                                ui.label(x.annee.to_string());
                            });
                            ligne.col(|ui| {
                                ui.add(
                                    ProgressBar::new(x.score)
                                        .desired_width(60.0)
                                        .fill(couleur_score(x.score))
                                        .text(format!("{:.0} %", x.score * 100.0)),
                                );
                            });
                            let rep = ligne.response();
                            if rep.clicked() {
                                envoyer(sortie, ActionRecherche::Choisir(n));
                            }
                            if rep.double_clicked() {
                                envoyer(sortie, ActionRecherche::Choisir(n));
                                envoyer(sortie, ActionRecherche::Valider);
                            }
                        });
                    });
                if r.en_cours {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(i.t("chooser.searching"));
                    });
                } else if let Some(e) = &r.erreur {
                    ui.label(RichText::new(format!("{} {e}", polices::AVERTISSEMENT)).color(ROUGE));
                } else if r.resultats.is_empty() {
                    ui.label(RichText::new(i.t("chooser.noresult")).weak());
                }
            });
            super::separateur_vertical(ui, 330.0);
            // Fiche du résultat choisi (droite)
            ui.vertical(|ui| {
                ui.set_min_width(440.0);
                if let Some(x) = r.resultat_choisi() {
                    ui.horizontal_top(|ui| {
                        image_ou_emplacement(
                            ui,
                            images,
                            x.affiche.as_deref(),
                            Vec2::new(140.0, 210.0),
                            &i.t("gui.artwork.poster"),
                        );
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&x.titre).heading().strong());
                            ui.label(
                                RichText::new(format!(
                                    "{} : {}",
                                    i.t("gui.original"),
                                    x.titre_original
                                ))
                                .weak(),
                            );
                            ui.label(format!("{} : {}", i.t("metatag.year"), x.annee));
                            ui.label(format!("ID : {}", x.id));
                            ui.add_space(6.0);
                            ScrollArea::vertical()
                                .id_salt("rech_resume")
                                .max_height(180.0)
                                .show(ui, |ui| {
                                    ui.label(&x.resume);
                                });
                        });
                    });
                }
            });
        });
        ui.separator();
        let mut npr = r.ne_pas_remplacer;
        if ui
            .checkbox(&mut npr, i.t("message.scrape.donotoverwrite"))
            .changed()
        {
            envoyer(sortie, ActionRecherche::NePasRemplacer(npr));
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add_enabled(r.selection.is_some(), Button::new(i.t("Button.ok")))
                .clicked()
            {
                envoyer(sortie, ActionRecherche::Valider);
            }
            if ui.button(i.t("Button.cancel")).clicked() {
                envoyer(sortie, ActionRecherche::Ignorer);
            }
            if r.total > 1 && ui.button(i.t("Button.abortqueue")).clicked() {
                envoyer(sortie, ActionRecherche::Abandonner);
            }
        });
    });
    if reponse.should_close() {
        envoyer(sortie, ActionRecherche::Abandonner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogues::EtatRecherche;
    use crate::etat::Dialogue;
    use crate::source_donnees::{RequeteRecherche, ResultatRecherche};
    use crate::vue::essais::verifier_stable;
    use std::path::PathBuf;

    fn etat_avec(nb: usize) -> (Etat, EtatRecherche) {
        let etat = Etat::nouveau();
        let affiche = std::env::temp_dir().join(format!("mmr-rech-{}.png", std::process::id()));
        crate::source_donnees::generer_image_demo(&affiche, 80, 120, 3).unwrap();
        let r = EtatRecherche {
            film_id: etat.films[0].id,
            serie: None,
            requete: RequeteRecherche {
                film: etat.films[0].id,
                extracteur: "TMDb".into(),
                langue: "fr".into(),
                texte: "Alien".into(),
                annee: None,
            },
            resultats: (0..nb)
                .map(|k| ResultatRecherche {
                    id: format!("id{k}"),
                    titre: format!("Résultat {k}"),
                    titre_original: format!("Result {k}"),
                    annee: 1979 + k as u32,
                    score: 0.9 - k as f32 * 0.05,
                    resume: "Un long résumé. ".repeat(40),
                    affiche: match k % 3 {
                        0 => Some(affiche.clone()),
                        1 => Some(PathBuf::from("http://127.0.0.1:9/absente.jpg")),
                        _ => None,
                    },
                })
                .collect(),
            selection: Some(0),
            en_cours: false,
            id_requete: 1,
            erreur: None,
            ne_pas_remplacer: false,
            file: Vec::new(),
            total: 1,
        };
        (etat, r)
    }

    #[test]
    fn liste_avec_jaquettes_taille_stable() {
        for nb in [0usize, 4, 25] {
            let (mut etat, r) = etat_avec(nb);
            etat.dialogue = Some(Dialogue::Recherche(Box::new(r.clone())));
            let attendus = if nb > 0 {
                vec![etat.i18n.t("metatag.title"), "Résultat 0".to_string()]
            } else {
                vec![etat.i18n.t("metatag.title")]
            };
            let mut images = GestionnaireImages::new(std::sync::Arc::new(|| {}));
            verifier_stable(&etat, "dlg_recherche", &attendus, |ctx, sortie| {
                images.recevoir(ctx);
                afficher(ctx, &etat, &mut images, &r, sortie)
            });
        }
    }
}
