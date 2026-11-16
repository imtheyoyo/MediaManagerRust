//! Onglet « Illustrations supplémentaires » de la série : même grille de vignettes que celle de
//! l'éditeur de film (Clearlogo, ClearArt, Keyart, Bannière, Miniature, disque, Square art).

use super::super::composants;
use super::super::illustrations::grille;
use crate::details_film::TypeArt;
use crate::dialogues::SerieEdite;
use crate::i18n::I18n;
use crate::images::GestionnaireImages;
use crate::vue::Sortie;
use eframe::egui::Ui;
use std::path::PathBuf;

pub fn afficher(
    ui: &mut Ui,
    b: &mut SerieEdite,
    i: &I18n,
    images: &mut GestionnaireImages,
    sortie: &mut Sortie,
) {
    let img = &b.details.images;
    let chemins: [Option<PathBuf>; 9] =
        std::array::from_fn(|k| img.chemin(TypeArt::TOUS[k]).map(|p| p.to_path_buf()));
    let dims: [Option<(u32, u32)>; 9] = std::array::from_fn(|k| {
        chemins[k].as_deref().and_then(|p| {
            img.dimensions(TypeArt::TOUS[k])
                .or_else(|| composants::dimensions_image(ui.ctx(), p))
        })
    });
    grille(
        ui,
        i,
        images,
        (&chemins, &dims),
        &mut b.details.images.supprimes,
        sortie,
    );
}
