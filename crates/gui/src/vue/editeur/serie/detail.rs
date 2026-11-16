//! Onglet « Détails » de la série : titres, citation, résumé, année et première diffusion,
//! statut, production, pays, langues, certification, durée, évaluations (tableau par source),
//! note, identifiants (tableau), genres et mots clés ; colonne de droite : affiche, fanart et
//! bannière avec leurs dimensions et leur corbeille.

use super::super::composants::{
    ColonneEdition, Entete, Glyphe, boutons_liste, cellule_crayon, cellule_decimal, cellule_entier,
    cellule_texte, champ_date, champ_texte, entete_illustration, etiquette, etiquette_haut,
    illustration, liste_deroulante, nombre_entier, tableau_edition, zone_texte,
};
use super::super::detail2::{ListeEdition, liste_avec_ajout};
use super::{Contexte, composants};
use crate::details_film::{NoteSource, TypeArt};
use crate::dialogues::{ChampSerie, SerieEdite};
use crate::editeur_film::retirer;
use crate::etat::Message;
use crate::icones::{self, Icone};
use crate::images::GestionnaireImages;
use crate::source_donnees::StatutSerie;
use crate::theme::ICONE_REPOS;
use crate::vue::Sortie;
use crate::vue::details::largeur_libelles;
use eframe::egui::{self, Align, Layout, RichText, ScrollArea, Ui, vec2};

/// Hauteurs des zones de la colonne de gauche : résumé, tableau des évaluations (en-tête +
/// 4 lignes), note, tableau des identifiants (en-tête + 4 lignes) et listes.
const H_RESUME: f32 = 96.0;
const H_NOTES: f32 = 24.0 + 4.0 * 22.0 + 2.0;
const H_NOTE: f32 = 56.0;
const H_IDS: f32 = 24.0 + 4.0 * 22.0 + 2.0;
const H_LISTES: f32 = 130.0;

pub fn afficher(
    ui: &mut Ui,
    ct: &Contexte<'_>,
    b: &mut SerieEdite,
    images: &mut GestionnaireImages,
    sortie: &mut Sortie,
) {
    let i = ct.i;
    let libelles: Vec<String> = [
        "metatag.title",
        "metatag.originaltitle",
        "editor.englishtitle",
        "editor.sorttitle",
        "editor.quote",
        "metatag.plot",
        "metatag.year",
        "metatag.production",
        "metatag.country",
        "editor.languages",
        "metatag.certification",
        "editor.userrating",
        "editor.ratings",
        "editor.note",
        "editor.ids",
        "metatag.genre",
    ]
    .iter()
    .map(|c| i.t(c))
    .collect();
    let etiq = largeur_libelles(ui, &libelles) + 6.0;
    let total = ui.available_width() - 14.0;
    let l_droite = (total * 0.24).clamp(200.0, 330.0);
    let l_gauche = total - l_droite - 16.0;
    let l_champ = (l_gauche - etiq - 6.0).max(120.0);
    ScrollArea::vertical()
        .id_salt("eds_detail")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(l_gauche);
                    colonne_gauche(ui, ct, b, (etiq, l_champ));
                });
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.set_width(l_droite);
                    colonne_droite(ui, ct, b, images, l_droite, sortie);
                });
            });
        });
}

/// Ligne « libellé / champ de texte ».
fn ligne_texte(ui: &mut Ui, etiq: f32, libelle: &str, valeur: &mut String, largeur: f32) {
    ui.horizontal(|ui| {
        etiquette(ui, etiq, libelle);
        champ_texte(ui, valeur, largeur, false);
    });
}

fn colonne_gauche(ui: &mut Ui, ct: &Contexte<'_>, b: &mut SerieEdite, (etiq, l_champ): (f32, f32)) {
    let i = ct.i;
    ui.spacing_mut().item_spacing = vec2(6.0, 7.0);
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("metatag.title"));
        champ_texte(ui, &mut b.titre, l_champ, ct.erreur(ChampSerie::Titre));
    });
    ligne_texte(
        ui,
        etiq,
        &i.t("metatag.originaltitle"),
        &mut b.titre_original,
        l_champ,
    );
    ligne_texte(
        ui,
        etiq,
        &i.t("editor.englishtitle"),
        &mut b.details.titre_anglais,
        l_champ,
    );
    ligne_texte(
        ui,
        etiq,
        &i.t("editor.sorttitle"),
        &mut b.details.titre_triable,
        l_champ,
    );
    ligne_texte(
        ui,
        etiq,
        &i.t("editor.quote"),
        &mut b.details.slogan,
        l_champ,
    );
    ui.horizontal_top(|ui| {
        etiquette_haut(ui, etiq, &i.t("metatag.plot"));
        zone_texte(ui, &mut b.resume, l_champ, H_RESUME);
    });
    // Année et première diffusion.
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("metatag.year"));
        let mut annee: i64 = b.annee.trim().parse().unwrap_or(0);
        if nombre_entier(ui, &mut annee, 132.0, 0, 2100).changed() {
            b.annee = if annee > 0 {
                annee.to_string()
            } else {
                String::new()
            };
        }
        ui.add_space(26.0);
        etiquette(ui, 130.0, &i.t("serieeditor.firstaired"));
        champ_date(
            ui,
            i,
            "eds_premiere_diffusion",
            &mut b.details.premiere_diffusion,
            190.0,
            false,
            ct.erreur(ChampSerie::PremiereDiffusion),
        );
    });
    // Statut et verrou.
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("gui.series.status"));
        let options: Vec<(String, String)> = StatutSerie::TOUS
            .iter()
            .map(|s| (s.cle().to_string(), i.t(s.cle())))
            .collect();
        let mut valeur = b.statut.cle().to_string();
        if liste_deroulante(ui, "eds_statut", 160.0, &mut valeur, &options)
            && let Some(s) = StatutSerie::TOUS.iter().find(|s| s.cle() == valeur)
        {
            b.statut = *s;
        }
        ui.add_space(26.0);
        ui.checkbox(&mut b.verrouillee, i.t("filter.locked"));
    });
    ligne_texte(
        ui,
        etiq,
        &i.t("serieeditor.production"),
        &mut b.details.production,
        l_champ,
    );
    ligne_texte(
        ui,
        etiq,
        &i.t("metatag.country"),
        &mut b.details.pays,
        l_champ,
    );
    // Langues parlées : non portées par les services.
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("editor.languages"));
        ui.add_enabled_ui(false, |ui| {
            champ_texte(ui, &mut String::new(), l_champ, false);
        })
        .response
        .on_disabled_hover_text(i.t("serieeditor.unsupported"));
    });
    // Certification et durée.
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("metatag.certification"));
        let mut options: Vec<(String, String)> = vec![(String::new(), String::new())];
        options.extend(
            ct.e.options
                .certifications
                .iter()
                .map(|c| (c.clone(), c.clone())),
        );
        liste_deroulante(
            ui,
            "eds_certification",
            132.0,
            &mut b.details.certification,
            &options,
        );
        ui.add_space(26.0);
        etiquette(ui, 130.0, &i.t("metatag.runtime"));
        let mut duree = i64::from(b.details.duree);
        if nombre_entier(ui, &mut duree, 100.0, 0, 100_000).changed() {
            b.details.duree = duree.max(0) as u32;
        }
        ui.label(i.t("editor.minutes"));
    });
    // Évaluation des utilisateurs et Top 250.
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("editor.userrating"));
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = 22.0;
            ui.add_sized(
                vec2(132.0, 22.0),
                egui::DragValue::new(&mut b.details.evaluation_utilisateur)
                    .range(0.0..=10.0)
                    .speed(0.1)
                    .max_decimals(1),
            );
        });
        ui.add(icones::image_teintee(Icone::Info, 15.0, ICONE_REPOS).sense(egui::Sense::hover()))
            .on_hover_text(i.t("editor.userrating.info"));
        ui.add_space(24.0);
        etiquette(ui, 64.0, &i.t("editor.top250"));
        let mut top = i64::from(b.details.top250);
        if nombre_entier(ui, &mut top, 90.0, 0, 250).changed() {
            b.details.top250 = top.max(0) as u32;
        }
    });
    notes(ui, ct, b, etiq, l_champ);
    ui.horizontal_top(|ui| {
        etiquette_haut(ui, etiq, &i.t("editor.note"));
        zone_texte(ui, &mut b.details.note_libre, l_champ, H_NOTE);
    });
    identifiants(ui, ct, b, etiq, l_champ);
    listes(ui, ct, b, etiq, l_champ);
}

/// Cote d'évaluation : tableau par source.
fn notes(ui: &mut Ui, ct: &Contexte<'_>, b: &mut SerieEdite, etiq: f32, l_champ: f32) {
    let i = ct.i;
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(vec2(etiq, H_NOTES), Layout::top_down(Align::Max), |ui| {
            ui.label(RichText::new(i.t("editor.ratings")).strong());
            let d = &mut b.details;
            let sel = &mut b.selections.note;
            let c = boutons_liste(ui, i, false, sel.is_some(), false, false);
            if c.ajouter {
                d.notes.push(NoteSource::default());
                *sel = Some(d.notes.len() - 1);
            }
            if c.retirer {
                retirer(&mut d.notes, sel);
            }
        });
        let d = &mut b.details;
        let sel = &mut b.selections.note;
        let n = d.notes.len();
        tableau_edition(
            ui,
            "eds_notes",
            l_champ,
            H_NOTES,
            vec![
                ColonneEdition::souple(l_champ * 0.4, i.t("editor.ratings.source")),
                ColonneEdition::fixe(70.0, i.t("editor.ratings.value")),
                ColonneEdition::fixe(110.0, i.t("editor.ratings.max")),
                ColonneEdition::fixe(130.0, i.t("editor.ratings.votes")),
                ColonneEdition::pictogramme(Entete::Glyphe(Glyphe::Crayon)),
            ],
            n,
            sel,
            |row, k| {
                let note = &mut d.notes[k];
                let mut agi = cellule_texte(row, &mut note.source);
                agi |= cellule_decimal(row, &mut note.valeur);
                agi |= cellule_entier(row, &mut note.maximum);
                agi |= cellule_entier(row, &mut note.votes);
                agi |= cellule_crayon(row, &i.t("editor.ratings"));
                agi
            },
        );
    });
}

/// Identifiants externes (tmdb, imdb, tvdb...) : tableau fournisseur / valeur.
fn identifiants(ui: &mut Ui, ct: &Contexte<'_>, b: &mut SerieEdite, etiq: f32, l_champ: f32) {
    let i = ct.i;
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(vec2(etiq, H_IDS), Layout::top_down(Align::Max), |ui| {
            ui.label(RichText::new(i.t("editor.ids")).strong());
            let sel = &mut b.selections.identifiant;
            let c = boutons_liste(ui, i, false, sel.is_some(), false, false);
            if c.ajouter {
                b.details.ids.push((String::new(), String::new()));
                *sel = Some(b.details.ids.len() - 1);
            }
            if c.retirer {
                retirer(&mut b.details.ids, sel);
            }
        });
        let n = b.details.ids.len();
        let ids = &mut b.details.ids;
        let sel = &mut b.selections.identifiant;
        tableau_edition(
            ui,
            "eds_ids",
            l_champ,
            H_IDS,
            vec![
                ColonneEdition::souple(l_champ * 0.3, i.t("editor.ids.key")),
                ColonneEdition::reste(i.t("editor.ids.value")),
                ColonneEdition::pictogramme(Entete::Glyphe(Glyphe::Crayon)),
            ],
            n,
            sel,
            |row, k| {
                let (cle, valeur) = &mut ids[k];
                let mut agi = cellule_texte(row, cle);
                agi |= cellule_texte(row, valeur);
                agi |= cellule_crayon(row, &i.t("editor.ids"));
                agi
            },
        );
    });
}

/// Genres et mots clés : listes avec champ d'ajout, côte à côte.
fn listes(ui: &mut Ui, ct: &Contexte<'_>, b: &mut SerieEdite, etiq: f32, l_champ: f32) {
    let i = ct.i;
    let demi = ((etiq + l_champ + 6.0 - 24.0) / 2.0).floor();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(demi);
            liste_avec_ajout(
                ui,
                i,
                ListeEdition {
                    id: "eds_genres",
                    libelle: i.t("metatag.genre"),
                    etiq,
                    largeur: (demi - etiq - 6.0).max(80.0),
                    hauteur: H_LISTES,
                    propositions: &ct.e.options.genres,
                },
                &mut b.genres,
                &mut b.selections.genre,
                &mut b.selections.ajout_genre,
            );
        });
        ui.add_space(24.0);
        ui.vertical(|ui| {
            ui.set_width(demi);
            liste_avec_ajout(
                ui,
                i,
                ListeEdition {
                    id: "eds_mots_cles",
                    libelle: i.t("editor.keywords"),
                    etiq: 84.0,
                    largeur: (demi - 84.0 - 6.0).max(80.0),
                    hauteur: H_LISTES,
                    propositions: &ct.e.options.mots_cles,
                },
                &mut b.details.mots_cles,
                &mut b.selections.mot_cle,
                &mut b.selections.ajout_mot_cle,
            );
        });
    });
}

fn colonne_droite(
    ui: &mut Ui,
    ct: &Contexte<'_>,
    b: &mut SerieEdite,
    images: &mut GestionnaireImages,
    largeur: f32,
    sortie: &mut Sortie,
) {
    let i = ct.i;
    for (t, hauteur) in [
        (TypeArt::Affiche, (largeur * 1.5).min(360.0)),
        (TypeArt::Fanart, largeur * 9.0 / 16.0),
        (TypeArt::Banniere, (largeur * 0.185).max(40.0)),
    ] {
        let chemin = b.details.images.chemin(t).map(|p| p.to_path_buf());
        let dims = chemin.as_deref().and_then(|p| {
            b.details
                .images
                .dimensions(t)
                .or_else(|| composants::dimensions_image(ui.ctx(), p))
        });
        let (ouvrir, supprimer) = entete_illustration(ui, i, &i.t(t.cle()), dims, true);
        let r = illustration(ui, images, chemin.as_deref(), vec2(largeur, hauteur), true);
        if (r.clicked() || ouvrir)
            && let Some(p) = chemin
        {
            sortie.push(Message::OuvrirVisionneuse(p));
        }
        if supprimer {
            b.details.images.supprimes[t.rang()] = true;
        }
        ui.add_space(14.0);
    }
}
