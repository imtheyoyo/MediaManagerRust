//! Onglet « Acteur et équipe » : deux tableaux côte à côte (acteurs ; équipe) à cellules
//! éditables, lignes sélectionnables et boutons plus, moins, haut et bas à gauche de chaque tableau.

use super::Contexte;
use super::composants::{
    ColonneEdition, Entete, boutons_liste, cellule_crayon_popup, cellule_etat_icone, cellule_texte,
    champ_texte, etiquette_haut, tableau_edition,
};
use crate::details_film::{FonctionPersonne, PersonneFilm};
use crate::dialogues::FilmEdite;
use crate::editeur_film::{deplacer, retirer};
use crate::i18n::I18n;
use crate::icones::Icone;
use eframe::egui::{Align, Layout, Ui, vec2};

/// Largeur de la colonne des boutons (libellé + boutons).
const LARGEUR_BOUTONS: f32 = 64.0;

pub fn afficher(ui: &mut Ui, ct: &Contexte<'_>, b: &mut FilmEdite) {
    let d = &mut b.details;
    let s = &mut b.selections;
    deux_tableaux(
        ui,
        ct.i,
        (&mut d.acteurs, &mut s.acteur),
        (&mut d.equipe, &mut s.equipe),
    );
}

/// Les deux tableaux côte à côte (acteurs, équipe) : liste et sélection de chacun.
pub(super) fn deux_tableaux(
    ui: &mut Ui,
    i: &I18n,
    acteurs: (&mut Vec<PersonneFilm>, &mut Option<usize>),
    equipe: (&mut Vec<PersonneFilm>, &mut Option<usize>),
) {
    let total = ui.available_width();
    let bloc = ((total - 20.0) / 2.0).floor();
    let hauteur = (ui.available_height() - 4.0).max(120.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.vertical(|ui| {
            ui.set_width(bloc);
            tableau_personnes(
                ui,
                i,
                Tableau {
                    id: "ed_acteurs",
                    titre: i.t("editor.cast.actors"),
                    bloc,
                    hauteur,
                    colonne_role: i.t("metatag.role"),
                    fonction: FonctionPersonne::Acteur,
                },
                acteurs.0,
                acteurs.1,
            );
        });
        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.set_width(bloc);
            tableau_personnes(
                ui,
                i,
                Tableau {
                    id: "ed_equipe",
                    titre: i.t("editor.cast.crew"),
                    bloc,
                    hauteur,
                    colonne_role: i.t("metatag.role"),
                    fonction: FonctionPersonne::Autre,
                },
                equipe.0,
                equipe.1,
            );
        });
    });
}

struct Tableau {
    id: &'static str,
    titre: String,
    bloc: f32,
    hauteur: f32,
    colonne_role: String,
    /// Fonction donnée aux lignes ajoutées (acteur ; pour l'équipe, déduite du rôle saisi).
    fonction: FonctionPersonne,
}

fn tableau_personnes(
    ui: &mut Ui,
    i: &I18n,
    t: Tableau,
    personnes: &mut Vec<PersonneFilm>,
    selection: &mut Option<usize>,
) {
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            vec2(LARGEUR_BOUTONS, t.hauteur),
            Layout::top_down(Align::Max),
            |ui| {
                etiquette_haut(ui, LARGEUR_BOUTONS, &t.titre);
                let n = personnes.len();
                let c = boutons_liste(
                    ui,
                    i,
                    true,
                    selection.is_some(),
                    selection.is_some_and(|s| s > 0),
                    selection.is_some_and(|s| s + 1 < n),
                );
                if c.ajouter {
                    // La nouvelle ligne s'insère sous la sélection, ou en fin de liste.
                    let rang = selection.map_or(n, |s| s + 1);
                    personnes.insert(rang, PersonneFilm::nouvelle(t.fonction));
                    *selection = Some(rang);
                }
                if c.retirer {
                    retirer(personnes, selection);
                }
                if c.monter {
                    deplacer(personnes, selection, true);
                }
                if c.descendre {
                    deplacer(personnes, selection, false);
                }
            },
        );
        let l_table = t.bloc - LARGEUR_BOUTONS - 6.0;
        let n = personnes.len();
        tableau_edition(
            ui,
            t.id,
            l_table,
            t.hauteur,
            vec![
                ColonneEdition::souple(l_table * 0.42, i.t("editor.cast.name")),
                ColonneEdition::reste(t.colonne_role.clone()),
                ColonneEdition::pictogramme(Entete::Icone(Icone::Image)),
                ColonneEdition::pictogramme(Entete::Icone(Icone::Info)),
                ColonneEdition::pictogramme(Entete::Glyphe(super::composants::Glyphe::Crayon)),
            ],
            n,
            selection,
            |row, k| {
                let p = &mut personnes[k];
                let mut agi = cellule_texte(row, &mut p.nom);
                agi |= cellule_texte(row, &mut p.role);
                cellule_etat_icone(row, p.a_image());
                cellule_etat_icone(row, p.a_profil());
                let (photo, profil) = (&mut p.url_vignette, &mut p.url_profil);
                agi |= cellule_crayon_popup(row, &i.t("editor.cast.edit"), |ui| {
                    ui.set_width(380.0);
                    ui.label(i.t("editor.cast.photo"));
                    champ_texte(ui, photo, 360.0, false);
                    ui.label(i.t("editor.cast.profile"));
                    champ_texte(ui, profil, 360.0, false);
                });
                agi
            },
        );
    });
}
