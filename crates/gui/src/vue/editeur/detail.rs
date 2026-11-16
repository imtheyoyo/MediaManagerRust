//! Onglet « Détails » : titres, citation, résumé, contour, année et date de sortie, production,
//! pays, langues, certification, évaluations (tableau par source), note ; colonne de droite :
//! affiche et fanart.

use super::composants::{
    ColonneEdition, Entete, Glyphe, bouton, boutons_liste, cellule_crayon, cellule_decimal,
    cellule_entier, cellule_texte, champ_date, champ_texte, entete_illustration, etiquette,
    etiquette_haut, illustration, liste_deroulante, nombre_entier, tableau_edition, zone_texte,
};
use super::{Contexte, chemin_art, dimensions_art};
use crate::details_film::{NoteSource, TypeArt};
use crate::dialogues::{ActionEditeur, ChampEditeur, FilmEdite};
use crate::editeur_film::retirer;
use crate::etat::{ActionDialogue, Message};
use crate::icones::{self, Icone};
use crate::images::GestionnaireImages;
use crate::theme::ICONE_REPOS;
use crate::vue::Sortie;
use crate::vue::details::largeur_libelles;
use eframe::egui::{self, Align, Layout, RichText, ScrollArea, Ui, vec2};

/// Hauteurs de référence des zones extensibles : résumé, contour, tableau des évaluations
/// (en-tête + 5 lignes) et note.
const BASE: [f32; 4] = [116.0, 80.0, 134.0, 68.0];
/// Hauteurs minimales de ces zones (quand l'écran est petit).
const MINI: [f32; 4] = [56.0, 40.0, 90.0, 38.0];
/// Hauteur occupée par les lignes de hauteur fixe de la colonne de gauche.
const FIXE: f32 = 360.0;

/// Hauteurs des zones extensibles : celles de référence si la place suffit, sinon réduites
/// proportionnellement (jusqu'au minimum) pour que tout reste visible.
fn hauteurs(disponible: f32) -> [f32; 4] {
    let base: f32 = BASE.iter().sum();
    let mini: f32 = MINI.iter().sum();
    let place = (disponible - FIXE).clamp(mini, base);
    let t = (place - mini) / (base - mini);
    std::array::from_fn(|k| (MINI[k] + (BASE[k] - MINI[k]) * t).floor())
}

pub fn afficher(
    ui: &mut Ui,
    ct: &Contexte<'_>,
    b: &mut FilmEdite,
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
        "editor.outline",
        "metatag.year",
        "metatag.production",
        "metatag.country",
        "editor.languages",
        "metatag.certification",
        "editor.userrating",
        "editor.ratings",
        "editor.note",
    ]
    .iter()
    .map(|c| i.t(c))
    .collect();
    let etiq = largeur_libelles(ui, &libelles) + 6.0;
    let total = ui.available_width() - 14.0;
    let l_droite = (total * 0.24).clamp(200.0, 330.0);
    let l_gauche = total - l_droite - 16.0;
    let l_champ = (l_gauche - etiq - 6.0).max(120.0);
    let h = hauteurs(ui.available_height());
    ScrollArea::vertical()
        .id_salt("ed_detail")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(l_gauche);
                    colonne_gauche(ui, ct, b, (etiq, l_champ), h, sortie);
                });
                ui.add_space(16.0);
                ui.vertical(|ui| {
                    ui.set_width(l_droite);
                    colonne_droite(ui, ct, b, images, l_droite, sortie);
                });
            });
        });
}

fn colonne_gauche(
    ui: &mut Ui,
    ct: &Contexte<'_>,
    b: &mut FilmEdite,
    (etiq, l_champ): (f32, f32),
    h: [f32; 4],
    sortie: &mut Sortie,
) {
    let i = ct.i;
    ui.spacing_mut().item_spacing = vec2(6.0, 7.0);
    // Titre, avec le bouton de recherche.
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("metatag.title"));
        champ_texte(
            ui,
            &mut b.titre,
            l_champ - 32.0,
            ct.erreur(ChampEditeur::Titre),
        );
        if bouton(ui, Glyphe::Lire, true, &i.t("editor.search")).clicked() {
            sortie.push(Message::Dialogue(ActionDialogue::Editeur(
                ActionEditeur::Rechercher,
            )));
        }
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
        &mut b.titre_triable,
        l_champ,
    );
    ligne_texte(ui, etiq, &i.t("editor.quote"), &mut b.slogan, l_champ);
    ui.horizontal_top(|ui| {
        etiquette_haut(ui, etiq, &i.t("metatag.plot"));
        zone_texte(ui, &mut b.resume, l_champ, h[0]);
    });
    ui.horizontal_top(|ui| {
        etiquette_haut(ui, etiq, &i.t("editor.outline"));
        zone_texte(ui, &mut b.details.contour, l_champ, h[1]);
    });
    // Année et date de sortie.
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("metatag.year"));
        let mut annee: i64 = b.annee.trim().parse().unwrap_or(0);
        if nombre_entier(ui, &mut annee, 132.0, 0, 2100).changed() {
            b.annee = annee.to_string();
        }
        ui.add_space(26.0);
        etiquette(ui, 100.0, &i.t("metatag.releasedate"));
        champ_date(
            ui,
            i,
            "ed_date_sortie",
            &mut b.date_sortie,
            190.0,
            false,
            ct.erreur(ChampEditeur::DateSortie),
        );
    });
    ligne_texte(
        ui,
        etiq,
        &i.t("metatag.production"),
        &mut b.production,
        l_champ,
    );
    ligne_texte(ui, etiq, &i.t("metatag.country"), &mut b.pays, l_champ);
    ligne_texte(
        ui,
        etiq,
        &i.t("editor.languages"),
        &mut b.details.langues_parlees,
        l_champ,
    );
    // Certification (liste déroulante).
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
            "ed_certification",
            132.0,
            &mut b.certification,
            &options,
        );
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
    // Cote d'évaluation : tableau par source.
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(vec2(etiq, h[2]), Layout::top_down(Align::Max), |ui| {
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
            "ed_notes",
            l_champ,
            h[2],
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
    ui.horizontal_top(|ui| {
        etiquette_haut(ui, etiq, &i.t("editor.note"));
        zone_texte(ui, &mut b.details.note_libre, l_champ, h[3]);
    });
}

/// Ligne « libellé / champ de texte ».
fn ligne_texte(ui: &mut Ui, etiq: f32, libelle: &str, valeur: &mut String, largeur: f32) {
    ui.horizontal(|ui| {
        etiquette(ui, etiq, libelle);
        champ_texte(ui, valeur, largeur, false);
    });
}

fn colonne_droite(
    ui: &mut Ui,
    ct: &Contexte<'_>,
    b: &mut FilmEdite,
    images: &mut GestionnaireImages,
    largeur: f32,
    sortie: &mut Sortie,
) {
    let i = ct.i;
    for (t, hauteur) in [
        (TypeArt::Affiche, (largeur * 1.5).min(430.0)),
        (TypeArt::Fanart, largeur * 9.0 / 16.0),
    ] {
        let chemin = chemin_art(ct, b, t);
        let dims = dimensions_art(ui.ctx(), b, t, chemin.as_deref());
        let (ouvrir, supprimer) = entete_illustration(
            ui,
            i,
            &i.t(t.cle()),
            dims.filter(|_| chemin.is_some()),
            true,
        );
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
