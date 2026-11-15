//! Thèmes clair / sombre (palette de la spécification §4.1) appliqués via `egui::Visuals`.

use crate::reglages::Theme;
use eframe::egui::{self, Color32, Stroke};

pub const ACCENT_SOMBRE: Color32 = Color32::from_rgb(0x4A, 0x88, 0xC7);
pub const ACCENT_CLAIR: Color32 = Color32::from_rgb(0x40, 0x83, 0xC9);
pub const BARRE_OUTILS: Color32 = Color32::from_rgb(0x2E, 0x2E, 0x2E);
pub const TEXTE_BARRE_OUTILS: Color32 = Color32::from_rgb(0xD0, 0xD0, 0xD0);
pub const VERT: Color32 = Color32::from_rgb(0x4C, 0xAF, 0x50);
pub const BLEU: Color32 = Color32::from_rgb(0x42, 0x8B, 0xDB);
pub const ROUGE: Color32 = Color32::from_rgb(0xD9, 0x53, 0x4F);
pub const ORANGE: Color32 = Color32::from_rgb(0xE0, 0x9F, 0x2D);

/// Fond des barres (outils, modules) et des bandeaux d'onglets.
pub const FOND_BARRES: Color32 = Color32::from_rgb(0x2B, 0x2B, 0x2B);
/// Orange du module actif et du logotype.
pub const ORANGE_ACTIF: Color32 = Color32::from_rgb(0xF3, 0x9C, 0x12);
/// Gris des icônes au repos.
pub const ICONE_REPOS: Color32 = Color32::from_rgb(0xA8, 0xA8, 0xA8);
/// Onglet sélectionné (bandeaux d'onglets).
pub const ONGLET_ACTIF: Color32 = Color32::from_rgb(0x55, 0x55, 0x55);
/// Filets du tableau (lignes et colonnes).
pub const FILET_TABLEAU: Color32 = Color32::from_rgb(0x4A, 0x4D, 0x50);
/// Poignée du séparateur liste / détails.
pub const POIGNEE: Color32 = Color32::from_rgb(0x9A, 0x9A, 0x9A);
/// Couleurs des cellules d'état.
pub const VERT_ETAT: Color32 = Color32::from_rgb(0x5C, 0xC1, 0x4F);
pub const ROUGE_ETAT: Color32 = Color32::from_rgb(0xE0, 0x45, 0x40);
pub const ORANGE_ETAT: Color32 = Color32::from_rgb(0xF0, 0xA0, 0x30);
pub const BLEU_ETAT: Color32 = Color32::from_rgb(0x3D, 0x9B, 0xF0);
/// Liens (identifiants, chemins) du panneau de détails.
pub const LIEN: Color32 = Color32::from_rgb(0x58, 0x9D, 0xF6);

fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub fn accent(theme: Theme) -> Color32 {
    match theme {
        Theme::Sombre => ACCENT_SOMBRE,
        Theme::Clair => ACCENT_CLAIR,
    }
}

pub fn visuals(theme: Theme) -> egui::Visuals {
    let (mut v, fond, texte, composant, alterne, selection, selection_texte) = match theme {
        Theme::Sombre => (
            egui::Visuals::dark(),
            0x2B2B2C,
            0xC0C0C0,
            0x3C3F41,
            0x3C3F41,
            0x555555,
            0xFFFFFF,
        ),
        Theme::Clair => (
            egui::Visuals::light(),
            0xEBEBEB,
            0x6B6B6B,
            0xFFFFFF,
            0xD5D5D5,
            0xC5CFD5,
            0x000000,
        ),
    };
    v.panel_fill = rgb(fond);
    v.window_fill = rgb(fond);
    v.extreme_bg_color = rgb(composant);
    v.faint_bg_color = match theme {
        Theme::Sombre => rgb(0x323436),
        Theme::Clair => rgb(0xF4F4F4),
    };
    v.override_text_color = Some(rgb(texte));
    v.hyperlink_color = accent(theme);
    v.selection.bg_fill = rgb(selection);
    v.selection.stroke = Stroke::new(1.0, rgb(selection_texte));
    v.widgets.noninteractive.bg_fill = rgb(alterne);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, rgb(alterne));
    v.widgets.inactive.bg_fill = rgb(composant);
    v.widgets.inactive.weak_bg_fill = rgb(composant);
    if theme == Theme::Sombre {
        v.hyperlink_color = LIEN;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, FILET_TABLEAU);
    }
    v
}

/// Couleur de fond de la zone centrale (« fond alterné » de la spécification).
pub fn fond_alterne(theme: Theme) -> Color32 {
    match theme {
        Theme::Sombre => rgb(0x3C3F41),
        Theme::Clair => rgb(0xD5D5D5),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_de_reference() {
        let sombre = visuals(Theme::Sombre);
        assert_eq!(sombre.panel_fill, Color32::from_rgb(0x2B, 0x2B, 0x2C));
        let clair = visuals(Theme::Clair);
        assert_eq!(clair.panel_fill, Color32::from_rgb(0xEB, 0xEB, 0xEB));
        assert!(sombre.dark_mode && !clair.dark_mode);
        assert_eq!(sombre.hyperlink_color, LIEN);
        // Fond des barres proche de #2B2B2B, comme la référence.
        assert_eq!(FOND_BARRES, Color32::from_rgb(0x2B, 0x2B, 0x2B));
    }
}
