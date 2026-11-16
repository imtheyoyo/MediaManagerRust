//! Vues egui : elles lisent un `&Etat` et émettent des `Message` (aucune logique métier).

pub mod barre_etat;
pub mod barre_menus;
pub mod barre_modules;
pub mod barre_outils;
pub mod collections;
pub mod details;
pub mod dialogue_assistant;
pub mod dialogue_confirmation;
pub mod dialogue_editeur_serie;
pub mod dialogue_edition_lot;
pub mod dialogue_export;
pub mod dialogue_filtres;
pub mod dialogue_manquants;
pub mod dialogue_messages;
pub mod dialogue_mise_a_jour;
pub mod dialogue_nettoyage;
pub mod dialogue_recherche;
pub mod dialogue_reglages;
pub mod dialogue_renommage;
pub mod editeur;
pub mod menus;
pub mod reglages_pages;
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

/// Filet vertical de hauteur fixe. Un `ui.separator()` placé dans une ligne horizontale prend
/// toute la hauteur disponible, ce qui fait grandir indéfiniment les fenêtres modales.
pub fn separateur_vertical(ui: &mut eframe::egui::Ui, hauteur: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(8.0, hauteur), eframe::egui::Sense::hover());
    let x = rect.center().x;
    ui.painter().line_segment(
        [
            eframe::egui::pos2(x, rect.top()),
            eframe::egui::pos2(x, rect.bottom()),
        ],
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
}

/// Outils des tests de rendu hors écran des dialogues modaux.
#[cfg(test)]
pub(crate) mod essais {
    use super::Sortie;
    use crate::etat::Etat;
    use eframe::egui::{self, Pos2, RawInput, Rect, Vec2, epaint::Shape};

    /// Écrans essayés : grand, portable courant, moyen, petit.
    pub const ECRANS: [Vec2; 4] = [
        Vec2::new(1914.0, 1026.0),
        Vec2::new(1366.0, 768.0),
        Vec2::new(1280.0, 800.0),
        Vec2::new(1050.0, 700.0),
    ];

    /// Rend `images` images du dialogue `id` ; renvoie les textes dessinés à la dernière image
    /// et le rectangle de la fenêtre.
    pub fn rendre(
        etat: &Etat,
        id: &'static str,
        ecran: Vec2,
        images: usize,
        mut dessiner: impl FnMut(&egui::Context, &mut Sortie),
    ) -> (Vec<String>, Rect) {
        let ctx = egui::Context::default();
        ctx.set_visuals(crate::theme::visuals(etat.reglages.theme));
        let mut textes = Vec::new();
        let mut sortie = Vec::new();
        for _ in 0..images {
            let entree = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, ecran)),
                ..Default::default()
            };
            let mut out = ctx.run_ui(entree, |ui| dessiner(ui.ctx(), &mut sortie));
            out.textures_delta.clear();
            textes = out
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    Shape::Text(t) => Some(t.galley.text().to_string()),
                    _ => None,
                })
                .collect();
        }
        let rect = ctx
            .memory(|m| m.area_rect(egui::Id::unique(id)))
            .expect("fenêtre affichée");
        (textes, rect)
    }

    /// Vérifie, à chaque taille d'écran : les textes `attendus` sont dessinés, la fenêtre est
    /// la même à l'image 4 et à l'image 20 et tient dans l'écran.
    pub fn verifier_stable(
        etat: &Etat,
        id: &'static str,
        attendus: &[String],
        dessiner: impl FnMut(&egui::Context, &mut Sortie),
    ) {
        verifier_stable_sur(etat, id, &ECRANS, attendus, dessiner);
    }

    /// Comme [`verifier_stable`], sur les `ecrans` donnés.
    pub fn verifier_stable_sur(
        etat: &Etat,
        id: &'static str,
        ecrans: &[Vec2],
        attendus: &[String],
        mut dessiner: impl FnMut(&egui::Context, &mut Sortie),
    ) {
        for &ecran in ecrans {
            let (textes, r4) = rendre(etat, id, ecran, 4, &mut dessiner);
            let (_, r20) = rendre(etat, id, ecran, 20, &mut dessiner);
            for a in attendus {
                assert!(
                    textes.iter().any(|t| t.contains(a.as_str())),
                    "{id} : texte « {a} » absent à {ecran:?} ({textes:?})"
                );
            }
            assert_eq!(r4.size(), r20.size(), "{id} grandit à {ecran:?}");
            assert!(
                r20.width() <= ecran.x && r20.height() <= ecran.y,
                "{id} : {r20:?} plus grande que l'écran {ecran:?}"
            );
        }
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
