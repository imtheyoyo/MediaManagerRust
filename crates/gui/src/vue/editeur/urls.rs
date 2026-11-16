//! Onglet « URL de l'artwork de bande annonce » : adresses des illustrations (affiche, fanart,
//! clearlogo, bannière, ClearArt, miniature, disque, keyart, square art) et tableau des
//! bandes-annonces (téléchargement, lecture, NFO, nom, source, qualité, langue, adresse).

use super::Contexte;
use super::composants::{
    ColonneEdition, Entete, Glyphe, bouton, boutons_liste, cellule_case, cellule_crayon,
    cellule_glyphe, cellule_texte, champ_texte, etiquette, etiquette_haut, tableau_edition,
};
use crate::details_film::{BandeAnnonceEdit, TypeArt};
use crate::dialogues::FilmEdite;
use crate::editeur_film::retirer;
use crate::etat::Message;
use crate::ouverture::CibleOuverture;
use crate::theme::{BLEU_ETAT, ICONE_REPOS};
use crate::vue::Sortie;
use crate::vue::details::largeur_libelles;
use eframe::egui::{Align, Layout, Ui, vec2};

/// Ordre des champs d'adresse.
const ORDRE: [TypeArt; 9] = [
    TypeArt::Affiche,
    TypeArt::Fanart,
    TypeArt::Clearlogo,
    TypeArt::Banniere,
    TypeArt::ClearArt,
    TypeArt::Miniature,
    TypeArt::Disque,
    TypeArt::Keyart,
    TypeArt::SquareArt,
];

pub fn afficher(ui: &mut Ui, ct: &Contexte<'_>, b: &mut FilmEdite, sortie: &mut Sortie) {
    let i = ct.i;
    let libelles: Vec<String> = ORDRE
        .iter()
        .map(|t| i.t(t.cle()))
        .chain([i.t("editor.trailer")])
        .collect();
    let etiq = largeur_libelles(ui, &libelles) + 6.0;
    let l_champ = ui.available_width() - etiq - 12.0;
    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
    for t in ORDRE {
        ui.horizontal(|ui| {
            etiquette(ui, etiq, &i.t(t.cle()));
            champ_texte(ui, &mut b.details.urls[t.rang()], l_champ, false);
        });
    }
    ui.add_space(14.0);
    let hauteur = (ui.available_height() - 4.0).max(110.0);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(vec2(etiq, hauteur), Layout::top_down(Align::Max), |ui| {
            etiquette_haut(ui, etiq, &i.t("editor.trailer"));
            let bandes = &mut b.details.bandes_annonces;
            let sel = &mut b.selections.bande_annonce;
            let c = boutons_liste(ui, i, false, sel.is_some(), false, false);
            if c.ajouter {
                bandes.push(BandeAnnonceEdit::default());
                *sel = Some(bandes.len() - 1);
            }
            if c.retirer {
                retirer(bandes, sel);
            }
            let url = sel
                .and_then(|n| bandes.get(n))
                .map(|x| x.url.clone())
                .unwrap_or_default();
            if bouton(
                ui,
                Glyphe::Lire,
                !url.trim().is_empty(),
                &i.t("editor.trailer.play"),
            )
            .clicked()
            {
                sortie.push(Message::Ouvrir(CibleOuverture::Adresse(url)));
            }
        });
        let l_table = l_champ;
        let n = b.details.bandes_annonces.len();
        let bandes = &mut b.details.bandes_annonces;
        let sel = &mut b.selections.bande_annonce;
        tableau_edition(
            ui,
            "ed_bandes",
            l_table,
            hauteur,
            vec![
                ColonneEdition::pictogramme(Entete::Vide),
                ColonneEdition::pictogramme(Entete::Vide),
                ColonneEdition::fixe(44.0, i.t("editor.trailer.nfo")),
                ColonneEdition::souple(l_table * 0.30, i.t("editor.trailer.name")),
                ColonneEdition::souple(68.0, i.t("editor.trailer.source")),
                ColonneEdition::souple(58.0, i.t("editor.trailer.quality")),
                ColonneEdition::souple(52.0, i.t("editor.trailer.language")),
                ColonneEdition::reste(i.t("editor.trailer.url")),
                ColonneEdition::pictogramme(Entete::Glyphe(Glyphe::Crayon)),
            ],
            n,
            sel,
            |row, k| {
                let x = &mut bandes[k];
                // Le téléchargement n'est pas pris en charge par les services.
                cellule_glyphe(
                    row,
                    Glyphe::Telecharger,
                    ICONE_REPOS,
                    false,
                    &i.t("editor.trailer.download"),
                );
                if cellule_glyphe(
                    row,
                    Glyphe::Lire,
                    BLEU_ETAT,
                    !x.url.trim().is_empty(),
                    &i.t("editor.trailer.play"),
                ) {
                    sortie.push(Message::Ouvrir(CibleOuverture::Adresse(x.url.clone())));
                }
                let mut agi = cellule_case(row, &mut x.dans_nfo);
                agi |= cellule_texte(row, &mut x.nom);
                agi |= cellule_texte(row, &mut x.source);
                agi |= cellule_texte(row, &mut x.qualite);
                agi |= cellule_texte(row, &mut x.langue);
                agi |= cellule_texte(row, &mut x.url);
                agi |= cellule_crayon(row, &i.t("editor.trailer.edit"));
                agi
            },
        );
    });
}
