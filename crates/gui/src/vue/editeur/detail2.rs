//! Onglet « Détails (2) » : date d'ajout, vu, source, 3D, édition, durée, saga, liens vers les
//! séries, identifiants (tableau), genres et mots clés (listes avec champ d'ajout).

use super::Contexte;
use super::composants::{
    ColonneEdition, Entete, Glyphe, boutons_liste, cellule_crayon, cellule_texte, champ_avec_liste,
    champ_date, champ_texte, etiquette, etiquette_haut, liste_deroulante, liste_selection,
    nombre_entier, tableau_edition,
};
use crate::dialogues::{ChampEditeur, FilmEdite};
use crate::editeur_film::{ajouter_texte, deplacer, retirer};
use crate::source_donnees::{EDITIONS, SOURCES_MEDIA};
use eframe::egui::{Align, Id, Layout, ScrollArea, Ui, vec2};

/// Largeur des libellés des deux colonnes.
const ETIQ_GAUCHE: f32 = 112.0;
const ETIQ_DROITE: f32 = 84.0;
/// Hauteur du tableau des identifiants (en-tête + 6 lignes).
const HAUTEUR_IDS: f32 = 24.0 + 6.0 * 22.0 + 2.0;

pub fn afficher(ui: &mut Ui, ct: &Contexte<'_>, b: &mut FilmEdite) {
    let total = ui.available_width() - 14.0;
    let demi = ((total - 24.0) / 2.0).floor();
    let hauteur_listes = (ui.available_height() - 360.0).clamp(150.0, 380.0);
    ScrollArea::vertical()
        .id_salt("ed_detail2")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 7.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(demi);
                    haut_gauche(ui, ct, b, demi);
                });
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.set_width(demi);
                    identifiants(ui, ct, b, demi);
                });
            });
            ui.add_space(14.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(demi);
                    let propositions = ct.e.options.genres.clone();
                    liste_avec_ajout(
                        ui,
                        ct,
                        ListeEdition {
                            id: "ed_genres",
                            libelle: ct.i.t("metatag.genre"),
                            etiq: ETIQ_GAUCHE,
                            largeur: demi - ETIQ_GAUCHE - 6.0,
                            hauteur: hauteur_listes,
                            propositions: &propositions,
                        },
                        &mut b.genres,
                        &mut b.selections.genre,
                        &mut b.selections.ajout_genre,
                    );
                });
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.set_width(demi);
                    let propositions = ct.e.options.mots_cles.clone();
                    liste_avec_ajout(
                        ui,
                        ct,
                        ListeEdition {
                            id: "ed_mots_cles",
                            libelle: ct.i.t("editor.keywords"),
                            etiq: ETIQ_DROITE + 24.0,
                            largeur: demi - ETIQ_DROITE - 30.0,
                            hauteur: hauteur_listes,
                            propositions: &propositions,
                        },
                        &mut b.details.mots_cles,
                        &mut b.selections.mot_cle,
                        &mut b.selections.ajout_mot_cle,
                    );
                });
            });
        });
}

fn haut_gauche(ui: &mut Ui, ct: &Contexte<'_>, b: &mut FilmEdite, largeur: f32) {
    let i = ct.i;
    let l_champ = largeur - ETIQ_GAUCHE - 6.0;
    // Date d'ajout, vu.
    ui.horizontal(|ui| {
        etiquette(ui, ETIQ_GAUCHE, &i.t("editor.dateadded"));
        champ_date(
            ui,
            i,
            "ed_date_ajout",
            &mut b.details.date_ajout,
            215.0,
            true,
            ct.erreur(ChampEditeur::DateAjout),
        );
        ui.add_space(24.0);
        etiquette(ui, 26.0, &i.t("metatag.watched"));
        ui.checkbox(&mut b.vu, "");
        champ_date(
            ui,
            i,
            "ed_date_vu",
            &mut b.details.date_vu,
            150.0,
            true,
            ct.erreur(ChampEditeur::DateVu),
        );
    });
    // Source et 3D.
    ui.horizontal(|ui| {
        etiquette(ui, ETIQ_GAUCHE, &i.t("editor.source"));
        let mut options: Vec<(String, String)> = vec![(String::new(), String::new())];
        options.extend(
            SOURCES_MEDIA
                .iter()
                .map(|(c, l)| ((*c).to_string(), (*l).to_string())),
        );
        liste_deroulante(
            ui,
            "ed_source",
            215.0,
            &mut b.details.source_media,
            &options,
        );
        ui.add_space(24.0);
        etiquette(ui, 26.0, &i.t("editor.3d"));
        ui.checkbox(&mut b.details.video_3d, "");
    });
    // Édition.
    ui.horizontal(|ui| {
        etiquette(ui, ETIQ_GAUCHE, &i.t("editor.edition"));
        let mut options: Vec<(String, String)> = vec![(String::new(), String::new())];
        options.extend(
            EDITIONS
                .iter()
                .map(|e| ((*e).to_string(), (*e).to_string())),
        );
        liste_deroulante(ui, "ed_edition", 215.0, &mut b.details.edition, &options);
    });
    // Durée.
    ui.horizontal(|ui| {
        etiquette(ui, ETIQ_GAUCHE, &i.t("metatag.runtime"));
        let mut duree: i64 = b.duree.trim().parse().unwrap_or(0);
        if nombre_entier(ui, &mut duree, 175.0, 0, 100_000).changed() {
            b.duree = duree.to_string();
        }
        ui.label(i.t("editor.minutes"));
    });
    // Saga : liste déroulante et saisie d'une nouvelle saga.
    ui.horizontal(|ui| {
        etiquette(ui, ETIQ_GAUCHE, &i.t("editor.saga"));
        let mut options: Vec<(String, String)> = vec![(String::new(), String::new())];
        options.extend(ct.e.options.sagas.iter().map(|s| (s.clone(), s.clone())));
        liste_deroulante(ui, "ed_saga", l_champ - 32.0, &mut b.saga, &options);
        let id_saisie = Id::unique("ed_saga_saisie");
        let ouverte = ui.data(|d| d.get_temp::<bool>(id_saisie)).unwrap_or(false);
        if super::composants::bouton(ui, Glyphe::Plus, true, &i.t("editor.saga.add")).clicked() {
            ui.data_mut(|d| d.insert_temp(id_saisie, !ouverte));
        }
    });
    if ui
        .data(|d| d.get_temp::<bool>(Id::unique("ed_saga_saisie")))
        .unwrap_or(false)
    {
        ui.horizontal(|ui| {
            etiquette(ui, ETIQ_GAUCHE, "");
            champ_texte(ui, &mut b.saga, l_champ - 32.0, false);
        });
    }
    // Liens vers les séries.
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            vec2(ETIQ_GAUCHE, 80.0),
            Layout::top_down(Align::Max),
            |ui| {
                ui.label(eframe::egui::RichText::new(i.t("editor.serieslink")).strong());
                let sel = &mut b.selections.serie;
                let c = boutons_liste(ui, i, false, sel.is_some(), false, false);
                if c.retirer {
                    retirer(&mut b.details.liens_series, sel);
                }
                if c.ajouter {
                    ajouter_texte(
                        &mut b.details.liens_series,
                        sel,
                        &b.selections.ajout_serie.clone(),
                    );
                    b.selections.ajout_serie.clear();
                }
            },
        );
        ui.vertical(|ui| {
            liste_selection(
                ui,
                "ed_liens_series",
                l_champ,
                76.0,
                &b.details.liens_series,
                &mut b.selections.serie,
            );
            let (_, entree) = champ_avec_liste(
                ui,
                &mut b.selections.ajout_serie,
                l_champ,
                &ct.e.options.series,
            );
            if entree {
                ajouter_texte(
                    &mut b.details.liens_series,
                    &mut b.selections.serie,
                    &b.selections.ajout_serie.clone(),
                );
                b.selections.ajout_serie.clear();
            }
        });
    });
}

fn identifiants(ui: &mut Ui, ct: &Contexte<'_>, b: &mut FilmEdite, largeur: f32) {
    let i = ct.i;
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            vec2(ETIQ_DROITE, HAUTEUR_IDS),
            Layout::top_down(Align::Max),
            |ui| {
                ui.label(eframe::egui::RichText::new(i.t("editor.ids")).strong());
                let sel = &mut b.selections.identifiant;
                let c = boutons_liste(ui, i, false, sel.is_some(), false, false);
                if c.ajouter {
                    b.details.ids.push((String::new(), String::new()));
                    *sel = Some(b.details.ids.len() - 1);
                }
                if c.retirer {
                    retirer(&mut b.details.ids, sel);
                }
            },
        );
        let l_table = largeur - ETIQ_DROITE - 6.0;
        let n = b.details.ids.len();
        let ids = &mut b.details.ids;
        let sel = &mut b.selections.identifiant;
        tableau_edition(
            ui,
            "ed_ids",
            l_table,
            HAUTEUR_IDS,
            vec![
                ColonneEdition::souple(l_table * 0.42, i.t("editor.ids.key")),
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

/// Paramètres d'une liste de textes avec champ d'ajout.
struct ListeEdition<'a> {
    id: &'a str,
    libelle: String,
    etiq: f32,
    largeur: f32,
    hauteur: f32,
    propositions: &'a [String],
}

/// Liste (genres, mots clés) : boutons plus, moins, haut et bas à gauche, liste sélectionnable, champ d'ajout à
/// liste déroulante en dessous.
fn liste_avec_ajout(
    ui: &mut Ui,
    ct: &Contexte<'_>,
    p: ListeEdition<'_>,
    elements: &mut Vec<String>,
    selection: &mut Option<usize>,
    ajout: &mut String,
) {
    let i = ct.i;
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            vec2(p.etiq, p.hauteur),
            Layout::top_down(Align::Max),
            |ui| {
                etiquette_haut(ui, p.etiq, &p.libelle);
                let n = elements.len();
                let c = boutons_liste(
                    ui,
                    i,
                    true,
                    selection.is_some(),
                    selection.is_some_and(|s| s > 0),
                    selection.is_some_and(|s| s + 1 < n),
                );
                if c.ajouter {
                    ajouter_texte(elements, selection, &ajout.clone());
                    ajout.clear();
                }
                if c.retirer {
                    retirer(elements, selection);
                }
                if c.monter {
                    deplacer(elements, selection, true);
                }
                if c.descendre {
                    deplacer(elements, selection, false);
                }
            },
        );
        ui.vertical(|ui| {
            liste_selection(ui, p.id, p.largeur, p.hauteur, elements, selection);
            let (_, entree) = champ_avec_liste(ui, ajout, p.largeur, p.propositions);
            if entree {
                ajouter_texte(elements, selection, &ajout.clone());
                ajout.clear();
            }
        });
    });
}
