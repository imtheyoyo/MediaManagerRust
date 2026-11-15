//! Vues egui : elles lisent un `&Etat` et émettent des `Message` (aucune logique métier).

pub mod barre_etat;
pub mod barre_menus;
pub mod barre_modules;
pub mod barre_outils;
pub mod details;
pub mod dialogue_assistant;
pub mod dialogue_confirmation;
pub mod dialogue_editeur;
pub mod dialogue_filtres;
pub mod dialogue_messages;
pub mod dialogue_recherche;
pub mod dialogue_reglages;
pub mod dialogue_renommage;
pub mod menus;
pub mod series;
pub mod tableau;
pub mod visionneuse;

use crate::etat::Message;
use crate::images::{CleImage, EtatImage, GestionnaireImages};
use eframe::egui::{
    self, Color32, CornerRadius, Rect, Response, Sense, Stroke, StrokeKind, Vec2, pos2,
};
use std::path::Path;

/// File des messages émis pendant le dessin, appliqués après la frame.
pub type Sortie = Vec<Message>;

/// Rectangle de proportions `taille_image` centré et contenu dans `cadre`.
pub fn ajuster(cadre: Rect, taille_image: Vec2) -> Rect {
    if taille_image.x <= 0.0 || taille_image.y <= 0.0 {
        return cadre;
    }
    let echelle = (cadre.width() / taille_image.x).min(cadre.height() / taille_image.y);
    Rect::from_center_size(cadre.center(), taille_image * echelle)
}

/// Emplacement d'illustration : image chargée en tâche de fond (placeholder immédiat).
/// Renvoie la réponse (clic = visionneuse, géré par l'appelant).
pub fn image_ou_emplacement(
    ui: &mut egui::Ui,
    images: &mut GestionnaireImages,
    chemin: Option<&Path>,
    taille: Vec2,
    libelle: &str,
) -> Response {
    let (rect, reponse) = ui.allocate_exact_size(taille, Sense::click());
    let visuals = ui.visuals().clone();
    let peintre = ui.painter();
    let arrondi = CornerRadius::same(6);
    peintre.rect_filled(rect, arrondi, visuals.extreme_bg_color);
    let mut texte = Some(libelle.to_string());
    if let Some(p) = chemin {
        match images.image(&CleImage::miniature(p)) {
            EtatImage::Prete(tex) => {
                let cible = ajuster(rect, tex.size_vec2());
                peintre.image(
                    tex.id(),
                    cible,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                texte = None;
            }
            EtatImage::EnCours => {
                egui::Spinner::new()
                    .paint_at(ui, Rect::from_center_size(rect.center(), Vec2::splat(18.0)));
                texte = None;
            }
            EtatImage::Erreur => {}
        }
    }
    let peintre = ui.painter();
    peintre.rect_stroke(
        rect,
        arrondi,
        Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
        StrokeKind::Inside,
    );
    if let Some(t) = texte {
        peintre.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            t,
            egui::FontId::proportional(12.0),
            visuals.weak_text_color(),
        );
    }
    if chemin.is_some() {
        reponse.on_hover_cursor(egui::CursorIcon::ZoomIn)
    } else {
        reponse
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ajustement_proportionnel_centre() {
        let cadre = Rect::from_min_size(pos2(0.0, 0.0), Vec2::new(100.0, 100.0));
        let r = ajuster(cadre, Vec2::new(200.0, 100.0));
        assert_eq!(r.size(), Vec2::new(100.0, 50.0));
        assert_eq!(r.center(), cadre.center());
        assert_eq!(ajuster(cadre, Vec2::ZERO), cadre);
    }
}
