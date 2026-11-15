//! Éditeur de série (modal) : titre, année, résumé, genres, statut, note, verrou et
//! identifiants. Édition sur une copie, validation par « Valider » (Ctrl+S ou Ctrl+Entrée).

use super::Sortie;
use crate::dialogues::{ActionEditeurSerie, ChampSerie, EtatEditeurSerie};
use crate::etat::{ActionDialogue, Etat, Message};
use crate::i18n::I18n;
use crate::polices;
use crate::source_donnees::StatutSerie;
use crate::theme::ROUGE;
use eframe::egui::{
    self, Align, Button, ComboBox, Grid, Key, KeyboardShortcut, Layout, Modal, Modifiers, RichText,
    ScrollArea, TextEdit, Vec2,
};

fn envoyer(sortie: &mut Sortie, a: ActionEditeurSerie) {
    sortie.push(Message::Dialogue(ActionDialogue::EditeurSerie(a)));
}

/// Champ texte sur une ligne de grille, avec message d'erreur éventuel.
fn champ(
    ui: &mut egui::Ui,
    i: &I18n,
    cle: &str,
    valeur: &mut String,
    erreur: Option<&'static str>,
    largeur: f32,
) {
    ui.label(i.t(cle));
    ui.horizontal(|ui| {
        let mut edit = TextEdit::singleline(valeur).desired_width(largeur);
        if erreur.is_some() {
            edit = edit.text_color(ROUGE);
        }
        ui.add(edit);
        if let Some(e) = erreur {
            ui.label(RichText::new(format!("{} {}", polices::AVERTISSEMENT, i.t(e))).color(ROUGE));
        }
    });
    ui.end_row();
}

pub fn afficher(ctx: &egui::Context, etat: &Etat, e: &EtatEditeurSerie, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let mut b = e.brouillon.clone();
    let mut valider = false;
    let mut annuler = false;
    let reponse = Modal::new(egui::Id::unique("dlg_editeur_serie")).show(ctx, |ui| {
        ui.set_min_size(Vec2::new(640.0, 460.0));
        ui.heading(format!(
            "{} - {}",
            i.t("gui.series.edit"),
            e.brouillon.titre
        ));
        ui.separator();
        ScrollArea::vertical()
            .id_salt("eds_corps")
            .max_height(360.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                Grid::new("eds_champs")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        champ(
                            ui,
                            i,
                            "metatag.title",
                            &mut b.titre,
                            e.erreur(ChampSerie::Titre),
                            360.0,
                        );
                        champ(
                            ui,
                            i,
                            "metatag.originaltitle",
                            &mut b.titre_original,
                            None,
                            360.0,
                        );
                        champ(
                            ui,
                            i,
                            "metatag.year",
                            &mut b.annee,
                            e.erreur(ChampSerie::Annee),
                            60.0,
                        );
                        ui.label(i.t("gui.series.status"));
                        ComboBox::from_id_salt("eds_statut")
                            .selected_text(i.t(b.statut.cle()))
                            .show_ui(ui, |ui| {
                                for s in StatutSerie::TOUS {
                                    ui.selectable_value(&mut b.statut, s, i.t(s.cle()));
                                }
                            });
                        ui.end_row();
                        champ(
                            ui,
                            i,
                            "metatag.rating",
                            &mut b.note,
                            e.erreur(ChampSerie::Note),
                            60.0,
                        );
                        ui.label("");
                        ui.checkbox(&mut b.verrouillee, i.t("filter.locked"));
                        ui.end_row();
                        champ(ui, i, "metatag.genre", &mut b.genres, None, 360.0);
                        ui.label(i.t("metatag.plot"));
                        ui.add(
                            TextEdit::multiline(&mut b.resume)
                                .desired_rows(5)
                                .desired_width(360.0),
                        );
                        ui.end_row();
                        champ(
                            ui,
                            i,
                            "filter.id.imdb",
                            &mut b.imdb,
                            e.erreur(ChampSerie::Imdb),
                            140.0,
                        );
                        champ(
                            ui,
                            i,
                            "filter.id.tmdb",
                            &mut b.tmdb,
                            e.erreur(ChampSerie::Tmdb),
                            140.0,
                        );
                        champ(
                            ui,
                            i,
                            "filter.id.tvdb",
                            &mut b.tvdb,
                            e.erreur(ChampSerie::Tvdb),
                            140.0,
                        );
                    });
                ui.label(RichText::new(i.t("editor.commalist")).weak().small());
            });
        ui.separator();
        if !e.erreurs.is_empty() {
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    polices::AVERTISSEMENT,
                    i.t("editor.invalid")
                ))
                .color(ROUGE),
            );
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add(Button::new(i.t("Button.ok")).shortcut_text("Ctrl+S"))
                .clicked()
            {
                valider = true;
            }
            if ui.button(i.t("Button.cancel")).clicked() {
                annuler = true;
            }
        });
    });
    if b != e.brouillon {
        envoyer(sortie, ActionEditeurSerie::Brouillon(Box::new(b)));
    }
    if valider {
        envoyer(sortie, ActionEditeurSerie::Valider);
    }
    let raccourci = ctx.input_mut(|inp| {
        inp.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S))
            || inp.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Enter))
    });
    if raccourci {
        envoyer(sortie, ActionEditeurSerie::Valider);
    } else if annuler || reponse.should_close() {
        envoyer(sortie, ActionEditeurSerie::Abandonner);
    }
}
