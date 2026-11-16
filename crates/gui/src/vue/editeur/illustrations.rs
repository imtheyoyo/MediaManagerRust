//! Onglet « Illustrations supplémentaires » : grille de vignettes (Clearlogo, ClearArt,
//! Keyart, Bannière, Miniature, Illustration de disque, Square art) ; emplacement gris avec
//! pictogramme quand l'illustration est absente, dimensions et corbeille quand elle existe.

use super::composants::{emplacement_vide, entete_illustration, illustration};
use super::{Contexte, chemin_art, dimensions_art};
use crate::details_film::TypeArt;
use crate::dialogues::FilmEdite;
use crate::etat::Message;
use crate::i18n::I18n;
use crate::images::GestionnaireImages;
use crate::vue::Sortie;
use eframe::egui::{Rect, Ui, UiBuilder, pos2, vec2};
use std::path::PathBuf;

/// Hauteur de la ligne de titre d'une cellule (libellé, dimensions, corbeille).
const HAUTEUR_ENTETE: f32 = 24.0;
const ECART: f32 = 22.0;

/// Cases de la grille (colonne, ligne, nombre de colonnes, nombre de lignes).
const CASES: [(TypeArt, usize, usize, usize, usize); 7] = [
    (TypeArt::Clearlogo, 0, 0, 1, 1),
    (TypeArt::ClearArt, 1, 0, 1, 1),
    (TypeArt::Keyart, 2, 0, 1, 2),
    (TypeArt::Banniere, 0, 1, 2, 1),
    (TypeArt::Miniature, 0, 2, 1, 1),
    (TypeArt::Disque, 1, 2, 1, 1),
    (TypeArt::SquareArt, 2, 2, 1, 1),
];

pub fn afficher(
    ui: &mut Ui,
    ct: &Contexte<'_>,
    b: &mut FilmEdite,
    images: &mut GestionnaireImages,
    sortie: &mut Sortie,
) {
    let chemins: [Option<PathBuf>; 9] =
        std::array::from_fn(|k| chemin_art(ct, b, TypeArt::TOUS[k]));
    let dims: [Option<(u32, u32)>; 9] = std::array::from_fn(|k| {
        dimensions_art(ui.ctx(), b, TypeArt::TOUS[k], chemins[k].as_deref())
    });
    grille(
        ui,
        ct.i,
        images,
        (&chemins, &dims),
        &mut b.details.images.supprimes,
        sortie,
    );
}

/// Chemins et dimensions des illustrations, un par `TypeArt`.
pub(super) type Chemins = [Option<PathBuf>; 9];
pub(super) type Dimensions = [Option<(u32, u32)>; 9];

/// Grille des sept emplacements : `chemins` et `dims` (un par `TypeArt`), `supprimes` reçoit les
/// suppressions demandées.
pub(super) fn grille(
    ui: &mut Ui,
    i: &I18n,
    images: &mut GestionnaireImages,
    (chemins, dims): (&Chemins, &Dimensions),
    supprimes: &mut [bool; 9],
    sortie: &mut Sortie,
) {
    let zone = ui.available_rect_before_wrap();
    let larg_col = ((zone.width() - 2.0 * ECART) / 3.0).floor();
    // Hauteur d'une vignette : trois lignes, chacune avec son titre.
    let haut_img = ((zone.height() - 3.0 * HAUTEUR_ENTETE - 2.0 * 10.0) / 3.0)
        .floor()
        .clamp(50.0, 260.0);
    let haut_ligne = HAUTEUR_ENTETE + haut_img;
    for (t, col, ligne, nb_col, nb_ligne) in CASES {
        let x = zone.left() + col as f32 * (larg_col + ECART);
        let y = zone.top() + ligne as f32 * (haut_ligne + 10.0);
        let largeur = nb_col as f32 * larg_col + (nb_col - 1) as f32 * ECART;
        let hauteur = nb_ligne as f32 * haut_ligne + (nb_ligne - 1) as f32 * 10.0;
        let cellule = Rect::from_min_size(pos2(x, y), vec2(largeur, hauteur));
        let mut cu = ui.new_child(UiBuilder::new().max_rect(cellule));
        cu.set_clip_rect(cellule.intersect(ui.clip_rect()));
        cu.spacing_mut().item_spacing = vec2(6.0, 2.0);
        let chemin = chemins[t.rang()].clone();
        let dims = dims[t.rang()];
        let (ouvrir, supprimer) = entete_illustration(
            &mut cu,
            i,
            &i.t(t.cle()),
            dims.filter(|_| chemin.is_some()),
            true,
        );
        let taille_image = vec2(largeur, hauteur - HAUTEUR_ENTETE);
        let rep = match (&chemin, dims) {
            // Le logo présent garde ses proportions (bandeau étroit).
            (Some(_), Some((l, h))) if t == TypeArt::Clearlogo && l > 0 => {
                let proportionnelle = (largeur * h as f32 / l as f32).min(taille_image.y);
                illustration(
                    &mut cu,
                    images,
                    chemin.as_deref(),
                    vec2(largeur, proportionnelle.max(20.0)),
                    true,
                )
            }
            (Some(_), _) => illustration(&mut cu, images, chemin.as_deref(), taille_image, false),
            (None, _) => emplacement_vide(&mut cu, taille_image),
        };
        if (rep.clicked() || ouvrir)
            && let Some(p) = chemin
        {
            sortie.push(Message::OuvrirVisionneuse(p));
        }
        if supprimer {
            supprimes[t.rang()] = true;
        }
    }
    ui.advance_cursor_after_rect(zone);
}
