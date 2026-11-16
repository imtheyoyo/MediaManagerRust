//! Onglet « URL de l'artwork » de la série : adresses des illustrations (affiche, fanart,
//! clearlogo, bannière, ClearArt, miniature, disque, keyart, square art).

use super::super::composants::{champ_texte, etiquette};
use super::Contexte;
use crate::details_film::TypeArt;
use crate::dialogues::SerieEdite;
use crate::vue::details::largeur_libelles;
use eframe::egui::{Ui, vec2};

pub fn afficher(ui: &mut Ui, ct: &Contexte<'_>, b: &mut SerieEdite) {
    let i = ct.i;
    let libelles: Vec<String> = TypeArt::TOUS.iter().map(|t| i.t(t.cle())).collect();
    let etiq = largeur_libelles(ui, &libelles) + 6.0;
    let l_champ = ui.available_width() - etiq - 12.0;
    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
    for t in TypeArt::TOUS {
        ui.horizontal(|ui| {
            etiquette(ui, etiq, &i.t(t.cle()));
            champ_texte(ui, &mut b.details.urls[t.rang()], l_champ, false);
        });
    }
}
