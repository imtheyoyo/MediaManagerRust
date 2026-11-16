//! Éditeur de film (modal) : onglets de champs, édition sur une copie, validation à la
//! confirmation (Ctrl+S ou Ctrl+Entrée), édition à la suite de plusieurs films.

use super::{Sortie, image_ou_emplacement};
use crate::dialogues::{ActionEditeur, ChampEditeur, EtatEditeur, FilmEdite, OngletEditeur};
use crate::etat::{ActionDialogue, Etat, Message};
use crate::i18n::I18n;
use crate::images::GestionnaireImages;
use crate::polices;
use crate::source_donnees::GenreImage;
use crate::theme::{ROUGE, accent};
use eframe::egui::{
    self, Align, Button, Grid, Key, KeyboardShortcut, Layout, Modal, Modifiers, RichText,
    ScrollArea, TextEdit, Vec2,
};

fn envoyer(sortie: &mut Sortie, a: ActionEditeur) {
    sortie.push(Message::Dialogue(ActionDialogue::Editeur(a)));
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

fn onglet_details(ui: &mut egui::Ui, i: &I18n, e: &EtatEditeur, b: &mut FilmEdite) {
    Grid::new("ed_details")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            champ(
                ui,
                i,
                "metatag.title",
                &mut b.titre,
                e.erreur(ChampEditeur::Titre),
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
                "metatag.sorttitle",
                &mut b.titre_triable,
                None,
                360.0,
            );
            champ(
                ui,
                i,
                "metatag.year",
                &mut b.annee,
                e.erreur(ChampEditeur::Annee),
                60.0,
            );
            champ(
                ui,
                i,
                "metatag.releasedate",
                &mut b.date_sortie,
                e.erreur(ChampEditeur::DateSortie),
                100.0,
            );
            champ(
                ui,
                i,
                "metatag.runtime",
                &mut b.duree,
                e.erreur(ChampEditeur::Duree),
                60.0,
            );
            champ(
                ui,
                i,
                "metatag.certification",
                &mut b.certification,
                None,
                80.0,
            );
            champ(
                ui,
                i,
                "metatag.rating",
                &mut b.note,
                e.erreur(ChampEditeur::Note),
                60.0,
            );
            ui.label("");
            ui.horizontal(|ui| {
                ui.checkbox(&mut b.vu, i.t("metatag.watched"));
                ui.checkbox(&mut b.verrouille, i.t("filter.locked"));
            });
            ui.end_row();
            champ(ui, i, "metatag.tagline", &mut b.slogan, None, 360.0);
            ui.label(i.t("metatag.plot"));
            ui.add(
                TextEdit::multiline(&mut b.resume)
                    .desired_rows(5)
                    .desired_width(360.0),
            );
            ui.end_row();
        });
}

fn onglet_classement(ui: &mut egui::Ui, i: &I18n, b: &mut FilmEdite) {
    ui.label(RichText::new(i.t("metatag.genre")).strong());
    let mut action: Option<(usize, i32)> = None;
    for (n, g) in b.genres.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(g);
            if ui
                .small_button(polices::HAUT)
                .on_hover_text(i.t("Button.moveup"))
                .clicked()
            {
                action = Some((n, -1));
            }
            if ui
                .small_button(polices::BAS)
                .on_hover_text(i.t("Button.movedown"))
                .clicked()
            {
                action = Some((n, 1));
            }
            if ui
                .small_button(polices::CORBEILLE)
                .on_hover_text(i.t("Button.remove"))
                .clicked()
            {
                action = Some((n, 0));
            }
        });
    }
    match action {
        Some((n, 0)) => {
            b.genres.remove(n);
        }
        Some((n, d)) => b.deplacer_genre(n, d < 0),
        None => {}
    }
    ui.horizontal(|ui| {
        let r = ui.add(TextEdit::singleline(&mut b.nouveau_genre).desired_width(200.0));
        let entree = r.lost_focus() && ui.input(|inp| inp.key_pressed(Key::Enter));
        if ui.button(format!("+ {}", i.t("Button.add"))).clicked() || entree {
            b.ajouter_genre();
        }
    });
    ui.add_space(10.0);
    Grid::new("ed_classement")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            champ(ui, i, "metatag.director", &mut b.realisateurs, None, 360.0);
            champ(ui, i, "metatag.country", &mut b.pays, None, 360.0);
            champ(ui, i, "metatag.production", &mut b.production, None, 360.0);
            champ(ui, i, "metatag.movieset", &mut b.saga, None, 360.0);
        });
    ui.label(RichText::new(i.t("editor.commalist")).weak().small());
}

/// Largeur minimale de la colonne « acteur » et de la colonne « rôle ».
const LARGEUR_MIN_ACTEUR: f32 = 260.0;
const LARGEUR_MIN_ROLE: f32 = 300.0;
/// Largeur réservée aux trois boutons (monter, descendre, retirer) et aux espacements.
const LARGEUR_BOUTONS: f32 = 3.0 * 34.0 + 4.0 * 8.0;
/// Hauteur des cases de saisie de la distribution.
const HAUTEUR_SAISIE: f32 = 30.0;

/// Largeurs (acteur, rôle) des champs de saisie pour la largeur `disponible` : les minima sont
/// respectés, le surplus est réparti à 45 % / 55 % ; une marge évite l'ascenseur horizontal.
fn largeurs_distribution(disponible: f32) -> (f32, f32) {
    let utile = (disponible - LARGEUR_BOUTONS - 24.0).max(LARGEUR_MIN_ACTEUR + LARGEUR_MIN_ROLE);
    let surplus = utile - LARGEUR_MIN_ACTEUR - LARGEUR_MIN_ROLE;
    (
        LARGEUR_MIN_ACTEUR + (surplus * 0.45).floor(),
        LARGEUR_MIN_ROLE + (surplus * 0.55).floor(),
    )
}

/// Action demandée sur une ligne de la distribution.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ActionActeur {
    Monter(usize),
    Descendre(usize),
    Retirer(usize),
}

fn case_saisie(ui: &mut egui::Ui, valeur: &mut String, largeur: f32) {
    ui.add(
        TextEdit::singleline(valeur)
            .desired_width(largeur)
            .min_size(Vec2::new(largeur, HAUTEUR_SAISIE))
            .margin(egui::Margin::symmetric(8, 6))
            .align(egui::Align2::LEFT_CENTER),
    );
}

/// Largeur de la fenêtre : fonction de l'écran seulement (jamais de l'espace « disponible »,
/// sinon la modale grandirait à chaque image), de 760 à 1100 px.
fn largeur_fenetre(ecran: f32) -> f32 {
    (ecran - 120.0).clamp(760.0, 1100.0)
}

fn onglet_distribution(ui: &mut egui::Ui, i: &I18n, b: &mut FilmEdite, largeur: f32) {
    // Marge pour l'ascenseur vertical et le cadre.
    let (l_nom, l_role) = largeurs_distribution(largeur - 24.0);
    let mut action: Option<ActionActeur> = None;
    let dernier = b.acteurs.len().saturating_sub(1);
    Grid::new("ed_acteurs")
        .num_columns(4)
        .spacing([8.0, 8.0])
        .striped(true)
        .show(ui, |ui| {
            ui.strong(i.t("metatag.actor"));
            ui.strong(i.t("metatag.role"));
            ui.label("");
            ui.label("");
            ui.end_row();
            for (n, (nom, role)) in b.acteurs.iter_mut().enumerate() {
                case_saisie(ui, nom, l_nom);
                case_saisie(ui, role, l_role);
                ui.horizontal(|ui| {
                    let bouton = |ui: &mut egui::Ui, texte: &str, actif: bool, tip: String| {
                        ui.add_enabled(
                            actif,
                            Button::new(texte).min_size(Vec2::new(30.0, HAUTEUR_SAISIE)),
                        )
                        .on_hover_text(tip)
                        .clicked()
                    };
                    if bouton(ui, polices::HAUT, n > 0, i.t("Button.moveup")) {
                        action = Some(ActionActeur::Monter(n));
                    }
                    if bouton(ui, polices::BAS, n < dernier, i.t("Button.movedown")) {
                        action = Some(ActionActeur::Descendre(n));
                    }
                    if bouton(ui, polices::CORBEILLE, true, i.t("Button.remove")) {
                        action = Some(ActionActeur::Retirer(n));
                    }
                });
                ui.end_row();
            }
        });
    match action {
        Some(ActionActeur::Monter(n)) => b.deplacer_acteur(n, true),
        Some(ActionActeur::Descendre(n)) => b.deplacer_acteur(n, false),
        Some(ActionActeur::Retirer(n)) => {
            b.acteurs.remove(n);
        }
        None => {}
    }
    ui.add_space(6.0);
    if ui
        .add(
            Button::new(format!("+ {}", i.t("Button.add")))
                .min_size(Vec2::new(120.0, HAUTEUR_SAISIE)),
        )
        .clicked()
    {
        b.acteurs.push((String::new(), String::new()));
    }
}

fn onglet_identifiants(ui: &mut egui::Ui, i: &I18n, e: &EtatEditeur, b: &mut FilmEdite) {
    Grid::new("ed_ids")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            champ(
                ui,
                i,
                "filter.id.imdb",
                &mut b.imdb,
                e.erreur(ChampEditeur::Imdb),
                140.0,
            );
            champ(
                ui,
                i,
                "filter.id.tmdb",
                &mut b.tmdb,
                e.erreur(ChampEditeur::Tmdb),
                140.0,
            );
        });
}

pub fn afficher(
    ctx: &egui::Context,
    etat: &Etat,
    images: &mut GestionnaireImages,
    e: &EtatEditeur,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    let mut b = e.brouillon.clone();
    let largeur = largeur_fenetre(ctx.content_rect().width());
    // Actions du pied appliquées APRÈS le brouillon modifié dans la même image.
    let mut pied: Vec<ActionEditeur> = Vec::new();
    let reponse = Modal::new(egui::Id::unique("dlg_editeur")).show(ctx, |ui| {
        ui.set_min_size(Vec2::new(largeur, 520.0));
        ui.horizontal(|ui| {
            ui.heading(format!("{} - {}", i.t("movie.edit"), e.brouillon.titre));
            if e.total > 1 {
                ui.label(RichText::new(format!("({} / {})", e.position(), e.total)).weak());
            }
        });
        ui.horizontal(|ui| {
            for o in OngletEditeur::TOUS {
                let texte = RichText::new(i.t(o.cle()));
                let texte = if o == e.onglet {
                    texte.strong().color(accent(etat.reglages.theme))
                } else {
                    texte
                };
                if ui.selectable_label(o == e.onglet, texte).clicked() {
                    envoyer(sortie, ActionEditeur::Onglet(o));
                }
            }
        });
        ui.separator();
        ScrollArea::vertical()
            .id_salt("ed_corps")
            .max_height(400.0)
            .auto_shrink([false, false])
            .show(ui, |ui| match e.onglet {
                OngletEditeur::Details => onglet_details(ui, i, e, &mut b),
                OngletEditeur::Classement => onglet_classement(ui, i, &mut b),
                OngletEditeur::Distribution => onglet_distribution(ui, i, &mut b, largeur),
                OngletEditeur::Identifiants => onglet_identifiants(ui, i, e, &mut b),
                OngletEditeur::Illustration => {
                    if let Some(film) = etat.film(e.id) {
                        ui.horizontal_wrapped(|ui| {
                            for (genre, taille) in [
                                (GenreImage::Affiche, Vec2::new(160.0, 240.0)),
                                (GenreImage::Fanart, Vec2::new(320.0, 180.0)),
                            ] {
                                let chemin = etat.source.chemin_image(film, genre);
                                let r = image_ou_emplacement(
                                    ui,
                                    images,
                                    chemin.as_deref(),
                                    taille,
                                    &i.t(genre.cle()),
                                );
                                if r.clicked()
                                    && let Some(p) = chemin
                                {
                                    sortie.push(Message::OuvrirVisionneuse(p));
                                }
                            }
                        });
                        ui.label(RichText::new(i.t("editor.artwork.readonly")).weak().small());
                    }
                }
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
                pied.push(ActionEditeur::Valider);
            }
            if ui.button(i.t("Button.cancel")).clicked() {
                pied.push(ActionEditeur::Ignorer);
            }
            if e.total > 1 && ui.button(i.t("Button.abortqueue")).clicked() {
                pied.push(ActionEditeur::Abandonner);
            }
        });
    });
    if b != e.brouillon {
        envoyer(sortie, ActionEditeur::Brouillon(Box::new(b)));
    }
    for a in pied {
        envoyer(sortie, a);
    }
    let valider = ctx.input_mut(|inp| {
        inp.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S))
            || inp.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Enter))
    });
    if valider {
        envoyer(sortie, ActionEditeur::Valider);
    } else if reponse.should_close() {
        envoyer(sortie, ActionEditeur::Abandonner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::etat::Dialogue;
    use crate::vue::essais::verifier_stable;

    #[test]
    fn largeurs_minimales_et_extensibles() {
        // Fenêtre minimale : minima respectés et tient dans la largeur disponible.
        let (n, r) = largeurs_distribution(740.0);
        assert!(n >= 260.0 && r >= 300.0, "{n} {r}");
        assert!(n + r + LARGEUR_BOUTONS <= 740.0, "{n} {r}");
        // Très étroit : on ne descend jamais sous les minima.
        assert_eq!(largeurs_distribution(100.0), (260.0, 300.0));
        assert_eq!(largeur_fenetre(500.0), 760.0);
        assert_eq!(largeur_fenetre(3000.0), 1100.0);
        // Fenêtre large : les colonnes s'étendent, sans dépasser l'espace disponible.
        let (n2, r2) = largeurs_distribution(1800.0);
        assert!(n2 > 600.0 && r2 > 700.0, "{n2} {r2}");
        assert!(n2 + r2 + LARGEUR_BOUTONS <= 1800.0);
    }

    #[test]
    fn deplacement_des_acteurs() {
        let etat = Etat::nouveau();
        let mut b = FilmEdite::depuis(&etat.films[0]);
        b.acteurs = vec![
            ("A".into(), "a".into()),
            ("B".into(), "b".into()),
            ("C".into(), "c".into()),
        ];
        b.deplacer_acteur(0, true);
        b.deplacer_acteur(2, false);
        assert_eq!(b.acteurs[0].0, "A");
        b.deplacer_acteur(2, true);
        assert_eq!(
            b.acteurs.iter().map(|a| a.0.as_str()).collect::<Vec<_>>(),
            ["A", "C", "B"]
        );
        b.deplacer_acteur(0, false);
        assert_eq!(b.acteurs[0].0, "C");
        b.deplacer_acteur(9, true);
        assert_eq!(b.acteurs.len(), 3);
    }

    #[test]
    fn distribution_taille_stable() {
        for nb in [0usize, 3, 40] {
            let mut etat = Etat::nouveau();
            let mut b = FilmEdite::depuis(&etat.films[0]);
            b.acteurs = (0..nb)
                .map(|k| (format!("Actrice numéro {k}"), format!("Rôle {k}")))
                .collect();
            etat.dialogue = Some(Dialogue::Editeur(Box::new(EtatEditeur {
                id: etat.films[0].id,
                onglet: OngletEditeur::Distribution,
                brouillon: b,
                erreurs: Vec::new(),
                file: Vec::new(),
                total: 1,
            })));
            let attendus = vec![etat.i18n.t("metatag.actor"), etat.i18n.t("metatag.role")];
            let mut images = GestionnaireImages::new(std::sync::Arc::new(|| {}));
            let Some(Dialogue::Editeur(e)) = etat.dialogue.clone() else {
                unreachable!()
            };
            verifier_stable(&etat, "dlg_editeur", &attendus, |ctx, sortie| {
                afficher(ctx, &etat, &mut images, &e, sortie)
            });
        }
    }
}
