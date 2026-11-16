//! Dialogue « Édition globale » : mêmes modifications pour plusieurs films (ou pour la série
//! choisie). Un champ vide ou « inchangé » conserve la valeur actuelle ; l'aperçu indique le
//! nombre d'éléments concernés.

use super::Sortie;
use crate::dialogues::{ActionEditionLot, BrouillonLot, EtatEditionLot};
use crate::etat::{ActionDialogue, Etat, Message};
use crate::i18n::I18n;
use crate::source_donnees::{EDITIONS, SOURCES_MEDIA};
use crate::theme::ORANGE;
use eframe::egui::{
    self, Align, ComboBox, Grid, Layout, Modal, RichText, ScrollArea, TextEdit, Vec2,
};

const LARGEUR: f32 = 620.0;
const HAUTEUR: f32 = 440.0;
const LARGEUR_CHAMP: f32 = 360.0;

fn champ(ui: &mut egui::Ui, i: &I18n, cle: &str, valeur: &mut String) {
    ui.label(i.t(cle));
    ui.add(TextEdit::singleline(valeur).desired_width(LARGEUR_CHAMP));
    ui.end_row();
}

/// Liste déroulante « inchangé » + valeurs (code, libellé).
fn choix(
    ui: &mut egui::Ui,
    i: &I18n,
    id: &str,
    valeur: &mut Option<String>,
    options: &[(String, String)],
) {
    let texte = match valeur {
        Some(v) => options
            .iter()
            .find(|(code, _)| code == v)
            .map(|(_, l)| l.clone())
            .unwrap_or_else(|| v.clone()),
        None => i.t("gui.lot.unchanged"),
    };
    ComboBox::from_id_salt(id)
        .selected_text(texte)
        .width(LARGEUR_CHAMP)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(valeur.is_none(), i.t("gui.lot.unchanged"))
                .clicked()
            {
                *valeur = None;
            }
            for (code, libelle) in options {
                if ui
                    .selectable_label(valeur.as_deref() == Some(code.as_str()), libelle)
                    .clicked()
                {
                    *valeur = Some(code.clone());
                }
            }
        });
}

pub fn afficher(ctx: &egui::Context, etat: &Etat, e: &EtatEditionLot, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let mut b: BrouillonLot = e.brouillon.clone();
    let mut appliquer = false;
    let reponse = Modal::new(egui::Id::unique("dlg_edition_lot")).show(ctx, |ui| {
        ui.set_min_size(Vec2::new(LARGEUR, HAUTEUR));
        ui.heading(i.t("movie.bulkedit"));
        ui.label(
            RichText::new(i.t_args(
                if e.series {
                    "gui.lot.count.tv"
                } else {
                    "gui.lot.count.movies"
                },
                &[("count", e.nb_modifies().to_string().as_str())],
            ))
            .strong(),
        );
        if e.nb_verrouilles > 0 {
            ui.label(
                RichText::new(i.t_args(
                    "gui.lot.locked",
                    &[("count", e.nb_verrouilles.to_string().as_str())],
                ))
                .color(ORANGE),
            );
        }
        ui.label(RichText::new(i.t("gui.lot.hint")).weak());
        ui.separator();
        ScrollArea::vertical()
            .id_salt("lot_corps")
            .max_height(HAUTEUR - 130.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                Grid::new("lot_champs")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        champ(ui, i, "gui.lot.genres.add", &mut b.genres_ajouter);
                        champ(ui, i, "gui.lot.genres.remove", &mut b.genres_retirer);
                        champ(ui, i, "gui.lot.tags.add", &mut b.tags_ajouter);
                        champ(ui, i, "gui.lot.tags.remove", &mut b.tags_retirer);
                        champ(ui, i, "metatag.certification", &mut b.certification);
                        champ(ui, i, "gui.lot.studio", &mut b.studio);
                        champ(ui, i, "metatag.country", &mut b.pays);
                        if !e.series {
                            champ(ui, i, "gui.lot.language", &mut b.langue);
                            ui.label(i.t("metatag.edition"));
                            let editions: Vec<(String, String)> = EDITIONS
                                .iter()
                                .map(|x| ((*x).to_string(), (*x).to_string()))
                                .collect();
                            choix(ui, i, "lot_edition", &mut b.edition, &editions);
                            ui.end_row();
                            ui.label(i.t("gui.lot.mediasource"));
                            let sources: Vec<(String, String)> = SOURCES_MEDIA
                                .iter()
                                .map(|(c, l)| ((*c).to_string(), (*l).to_string()))
                                .collect();
                            choix(ui, i, "lot_source", &mut b.source_media, &sources);
                            ui.end_row();
                            ui.label(i.t("metatag.watched"));
                            let mut vu = b.vu.map(|v| v.to_string());
                            let valeurs = [
                                ("true".to_string(), i.t("gui.lot.watched.yes")),
                                ("false".to_string(), i.t("gui.lot.watched.no")),
                            ];
                            choix(ui, i, "lot_vu", &mut vu, &valeurs);
                            b.vu = vu.map(|v| v == "true");
                            ui.end_row();
                        }
                        champ(ui, i, "gui.lot.note", &mut b.note);
                    });
            });
        ui.add_space(8.0);
        ui.separator();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button(i.t("Button.cancel")).clicked() {
                sortie.push(Message::FermerDialogue);
            }
            let actif = !e.modifications().est_vide() && e.nb_modifies() > 0;
            if ui
                .add_enabled(actif, egui::Button::new(i.t("Button.apply")))
                .clicked()
            {
                appliquer = true;
            }
        });
    });
    if b != e.brouillon {
        sortie.push(Message::Dialogue(ActionDialogue::EditionLot(
            ActionEditionLot::Brouillon(Box::new(b)),
        )));
    }
    if appliquer {
        sortie.push(Message::Dialogue(ActionDialogue::EditionLot(
            ActionEditionLot::Appliquer,
        )));
    }
    if reponse.should_close() {
        sortie.push(Message::FermerDialogue);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commandes::Commande;
    use crate::etat::{Dialogue, Module, NoeudSerie};
    use crate::vue::essais::verifier_stable;

    fn verifier(etat: &Etat, cles: &[&str]) {
        verifier_avec(etat, cles, &[]);
    }

    /// Comme `verifier`, avec des textes déjà composés en plus des clés.
    fn verifier_avec(etat: &Etat, cles: &[&str], textes: &[String]) {
        let mut attendus: Vec<String> = cles.iter().map(|c| etat.i18n.t(c)).collect();
        attendus.extend(textes.iter().cloned());
        verifier_stable(etat, "dlg_edition_lot", &attendus, |ctx, sortie| {
            if let Some(Dialogue::EditionLot(d)) = &etat.dialogue {
                afficher(ctx, etat, d, sortie);
            }
        });
    }

    #[test]
    fn films_taille_stable_avec_et_sans_saisie() {
        let mut etat = Etat::nouveau();
        etat.appliquer(Message::SelectionnerTout);
        etat.appliquer(Message::Commande(Commande::EditionGlobale));
        verifier(
            &etat,
            &[
                "movie.bulkedit",
                "gui.lot.genres.add",
                "gui.lot.tags.remove",
                "metatag.certification",
                "gui.lot.mediasource",
                "metatag.watched",
                "gui.lot.note",
                "Button.apply",
            ],
        );
        if let Some(Dialogue::EditionLot(d)) = &mut etat.dialogue {
            d.nb_verrouilles = 2;
            d.brouillon.genres_ajouter = "Action, ".repeat(60);
            d.brouillon.edition = Some("IMAX".into());
            d.brouillon.source_media = Some("BLURAY".into());
            d.brouillon.vu = Some(false);
        }
        let verrouilles = etat.i18n.t_args("gui.lot.locked", &[("count", "2")]);
        verifier_avec(&etat, &[], &[verrouilles]);
    }

    #[test]
    fn serie_taille_stable_sans_champs_des_films() {
        let mut etat = Etat::nouveau();
        etat.appliquer(Message::ChoisirModule(Module::Series));
        etat.appliquer(Message::SelectionnerNoeudSerie(NoeudSerie::Serie(
            "demo-1".into(),
        )));
        etat.appliquer(Message::Commande(Commande::EditionGlobale));
        let compte = etat.i18n.t_args("gui.lot.count.tv", &[("count", "1")]);
        verifier_avec(&etat, &["movie.bulkedit", "gui.lot.studio"], &[compte]);
        let (textes, _) = crate::vue::essais::rendre(
            &etat,
            "dlg_edition_lot",
            Vec2::new(1280.0, 800.0),
            4,
            |ctx, sortie| {
                if let Some(Dialogue::EditionLot(d)) = &etat.dialogue {
                    afficher(ctx, &etat, d, sortie);
                }
            },
        );
        let langue = etat.i18n.t("gui.lot.language");
        assert!(!textes.iter().any(|t| t.contains(&langue)));
    }
}
