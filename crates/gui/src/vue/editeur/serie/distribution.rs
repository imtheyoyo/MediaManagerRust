//! Onglet « Acteur et équipe » de la série : deux tableaux éditables côte à côte (les mêmes que
//! ceux de l'éditeur de film).

use super::super::distribution::deux_tableaux;
use super::Contexte;
use crate::dialogues::SerieEdite;
use eframe::egui::Ui;

pub fn afficher(ui: &mut Ui, ct: &Contexte<'_>, b: &mut SerieEdite) {
    let d = &mut b.details;
    let s = &mut b.selections;
    deux_tableaux(
        ui,
        ct.i,
        (&mut d.acteurs, &mut s.acteur),
        (&mut d.equipe, &mut s.equipe),
    );
}
