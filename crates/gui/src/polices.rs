//! Pictogrammes de l'interface, centralisés ici et vérifiés par un test contre les polices
//! embarquées d'egui 0.37 (Ubuntu-Light, Hack, egui-icons, et — grâce à la fonctionnalité
//! `monochrome_emoji_fonts` — NotoEmoji (OFL) et emoji-icon-font (MIT)).
//!
//! Sans cette fonctionnalité, egui 0.37 n'embarque plus aucune police d'emojis : ⚙ 🔔 ✔ ⚠
//! s'affichaient en carrés vides sur un système sans police de symboles. Certains caractères
//! restent absents même avec elle (▲ ▼ ▾ ● ✎ ⇄) : on utilise des équivalents disponibles.

pub const TRI_CROISSANT: &str = "⏶";
pub const TRI_DECROISSANT: &str = "⏷";
pub const DEROULANT: &str = "⏷";
pub const FILTRE_ACTIF: &str = "🔽";
pub const PASTILLE: &str = "⚫";
pub const VERROU: &str = "🔒";
pub const COCHE: &str = "✔";
pub const CROIX: &str = "✖";
pub const MOINS: &str = "−";
pub const AVERTISSEMENT: &str = "⚠";
pub const INFO: &str = "ℹ";
pub const STOP: &str = "■";
pub const LIRE: &str = "▶";
pub const RECHARGER: &str = "⟳";
pub const RECHERCHE: &str = "🔍";
pub const EDITER: &str = "✏";
pub const RENOMMER: &str = "↔";
pub const CLOCHE: &str = "🔔";
pub const REGLAGES: &str = "⚙";
pub const OUTILS: &str = "🔧";
pub const FILMS: &str = "🎬";
pub const COLLECTIONS: &str = "📚";
pub const SERIES: &str = "📺";
pub const HAUT: &str = "⬆";
pub const BAS: &str = "⬇";
pub const GAUCHE: &str = "⏴";
pub const DROITE: &str = "⏵";
pub const CORBEILLE: &str = "🗑";
pub const IMAGE: &str = "🖻";
pub const ETOILE: &str = "★";

/// Tous les pictogrammes (vérifiés par le test).
pub const TOUS: &[&str] = &[
    TRI_CROISSANT,
    TRI_DECROISSANT,
    DEROULANT,
    FILTRE_ACTIF,
    PASTILLE,
    VERROU,
    COCHE,
    CROIX,
    MOINS,
    AVERTISSEMENT,
    INFO,
    STOP,
    LIRE,
    RECHARGER,
    RECHERCHE,
    EDITER,
    RENOMMER,
    CLOCHE,
    REGLAGES,
    OUTILS,
    FILMS,
    COLLECTIONS,
    SERIES,
    HAUT,
    BAS,
    GAUCHE,
    DROITE,
    CORBEILLE,
    IMAGE,
    ETOILE,
    // symboles d'opérateurs des filtres
    "<",
    "≤",
    "=",
    "≥",
    ">",
    "↔",
];

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    fn contexte() -> egui::Context {
        let ctx = egui::Context::default();
        let mut sortie = ctx.run_ui(Default::default(), |_| {});
        sortie.textures_delta.clear();
        ctx
    }

    fn manquants(ctx: &egui::Context, texte: &str) -> Vec<char> {
        let police = egui::FontId::proportional(14.0);
        let mut v: Vec<char> = texte
            .chars()
            .filter(|c| !c.is_control() && !c.is_whitespace())
            .filter(|&c| !ctx.fonts_mut(|f| f.has_glyph(&police, c)))
            .collect();
        v.sort();
        v.dedup();
        v
    }

    #[test]
    fn pictogrammes_presents_dans_les_polices_embarquees() {
        let ctx = contexte();
        // Garde-fou : le test détecte bien un glyphe absent des polices embarquées.
        assert_eq!(manquants(&ctx, "a▲"), ['▲']);
        let tous: String = TOUS.concat();
        assert!(
            manquants(&ctx, &tous).is_empty(),
            "{:?}",
            manquants(&ctx, &tous)
        );
    }

    #[test]
    fn caracteres_des_traductions_presents() {
        let ctx = contexte();
        for source in [
            include_str!("../i18n/fr.properties"),
            include_str!("../i18n/en.properties"),
        ] {
            let m = manquants(&ctx, source);
            assert!(m.is_empty(), "glyphes absents : {m:?}");
        }
    }

    #[test]
    fn les_vues_n_utilisent_pas_de_glyphe_absent() {
        // Les sources des vues ne doivent contenir que des caractères affichables
        // (les chaînes de démonstration en coréen de `donnees.rs` sont des données, pas l'interface).
        let ctx = contexte();
        for source in [
            include_str!("vue/barre_etat.rs"),
            include_str!("vue/barre_menus.rs"),
            include_str!("vue/barre_modules.rs"),
            include_str!("vue/barre_outils.rs"),
            include_str!("vue/details.rs"),
            include_str!("vue/dialogue_assistant.rs"),
            include_str!("vue/dialogue_confirmation.rs"),
            include_str!("vue/dialogue_editeur.rs"),
            include_str!("vue/dialogue_filtres.rs"),
            include_str!("vue/dialogue_messages.rs"),
            include_str!("vue/dialogue_recherche.rs"),
            include_str!("vue/dialogue_reglages.rs"),
            include_str!("vue/dialogue_renommage.rs"),
            include_str!("vue/tableau.rs"),
            include_str!("vue/visionneuse.rs"),
        ] {
            let m = manquants(&ctx, source);
            assert!(m.is_empty(), "glyphes absents : {m:?}");
        }
    }
}
