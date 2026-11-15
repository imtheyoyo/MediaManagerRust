//! Visionneuse plein écran (lightbox) : image sur voile sombre ; clic ou Échap pour fermer.

use super::{Sortie, ajuster};
use crate::etat::{Etat, Message};
use crate::images::{CleImage, EtatImage, GestionnaireImages};
use eframe::egui::{self, Align2, Color32, FontId, Key, Order, Rect, Sense, Vec2, pos2};
use std::path::Path;

pub fn afficher(
    ctx: &egui::Context,
    etat: &Etat,
    images: &mut GestionnaireImages,
    chemin: &Path,
    sortie: &mut Sortie,
) {
    let ecran = ctx.content_rect();
    egui::Area::new(egui::Id::unique("visionneuse"))
        .order(Order::Foreground)
        .fixed_pos(ecran.min)
        .show(ctx, |ui| {
            let (rect, reponse) = ui.allocate_exact_size(ecran.size(), Sense::click());
            ui.painter()
                .rect_filled(rect, 0.0, Color32::from_black_alpha(220));
            let cadre = rect.shrink(32.0);
            // La miniature (déjà en cache) sert d'aperçu pendant le chargement de l'original.
            let tex = match images.image(&CleImage::pleine(chemin)) {
                EtatImage::Prete(t) => Some(t),
                _ => match images.image(&CleImage::miniature(chemin)) {
                    EtatImage::Prete(t) => Some(t),
                    _ => None,
                },
            };
            match tex {
                Some(t) => {
                    ui.painter().image(
                        t.id(),
                        ajuster(cadre, t.size_vec2()),
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                None => {
                    egui::Spinner::new().size(32.0).paint_at(
                        ui,
                        Rect::from_center_size(cadre.center(), Vec2::splat(32.0)),
                    );
                }
            }
            ui.painter().text(
                rect.center_bottom() - Vec2::new(0.0, 12.0),
                Align2::CENTER_BOTTOM,
                format!(
                    "{}  —  {}",
                    chemin.display(),
                    etat.i18n.t("gui.lightbox.close")
                ),
                FontId::proportional(12.0),
                Color32::from_gray(200),
            );
            if reponse.clicked() {
                sortie.push(Message::FermerVisionneuse);
            }
        });
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
        sortie.push(Message::FermerVisionneuse);
    }
}
