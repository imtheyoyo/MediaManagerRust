//! Barre d'outils horizontale compacte (80 px) : logotype + version à gauche, quatre gros
//! boutons (icône colorée = action par défaut, libellé ⏷ = menu déroulant), cadenas et cloche
//! des messages à droite.
//!
//! Chaque groupe reçoit une zone de largeur fixe calculée d'après son libellé : aucune
//! zone ne peut être écrasée (le libellé ne passe jamais à la ligne).

use super::{Sortie, menus};
use crate::commandes::{
    Commande, Entree, MENU_EDITION, MENU_MISE_A_JOUR, MENU_RECHERCHE, MENU_RENOMMAGE,
};
use crate::etat::{Etat, Message, Niveau, TypeDialogue};
use crate::icones::{self, Icone};
use crate::polices;
use crate::theme::{FOND_BARRES, ICONE_REPOS, ORANGE_ACTIF, ROUGE_ETAT, TEXTE_BARRE_OUTILS};
use eframe::egui::containers::menu::MenuButton;
use eframe::egui::{
    self, Align, Align2, Button, Color32, FontId, FontSelection, Frame, Layout, Margin, Panel,
    Rect, RichText, TextWrapMode, Vec2, WidgetText, vec2,
};

/// Hauteur de la barre d'outils.
pub const HAUTEUR: f32 = 80.0;
/// Côté des icônes des quatre gros boutons.
const COTE_ICONE: f32 = 30.0;
/// Largeur réservée à la flèche du menu déroulant et aux marges du libellé.
const MARGE_LIBELLE: f32 = 26.0;

/// Description d'un grand bouton de la barre d'outils et de son menu déroulant.
struct Groupe {
    icone: Icone,
    cle_bouton: &'static str,
    action: Commande,
    entrees: &'static [Entree],
}

const GROUPES: [Groupe; 4] = [
    Groupe {
        icone: Icone::MiseAJour,
        cle_bouton: "Toolbar.update",
        action: Commande::MettreAJourToutesSources,
        entrees: MENU_MISE_A_JOUR,
    },
    Groupe {
        icone: Icone::Recherche,
        cle_bouton: "Toolbar.search",
        action: Commande::ScrapeInteractif,
        entrees: MENU_RECHERCHE,
    },
    Groupe {
        icone: Icone::Modifier,
        cle_bouton: "Toolbar.edit",
        action: Commande::Editer,
        entrees: MENU_EDITION,
    },
    Groupe {
        icone: Icone::Renommer,
        cle_bouton: "Toolbar.rename",
        action: Commande::Renommer,
        entrees: MENU_RENOMMAGE,
    },
];

/// Zone occupée par un élément de la barre (relevée pour les tests de disposition).
#[derive(Clone, Copy, Debug)]
pub struct ZoneBarre {
    /// Zone allouée à l'élément.
    pub zone: Rect,
    /// Rectangle réel du libellé (bouton de menu ou texte).
    pub libelle: Rect,
    /// Largeur du texte du libellé sur une seule ligne.
    pub largeur_texte: f32,
}

/// Largeur d'un texte sur une seule ligne.
fn largeur_texte(ui: &egui::Ui, texte: RichText) -> f32 {
    WidgetText::from(texte)
        .into_galley(
            ui,
            Some(TextWrapMode::Extend),
            f32::INFINITY,
            FontSelection::Default,
        )
        .size()
        .x
}

fn libelle_groupe(etat: &Etat, g: &Groupe) -> RichText {
    RichText::new(etat.i18n.t(g.cle_bouton))
        .size(12.0)
        .color(TEXTE_BARRE_OUTILS)
}

/// Logotype coloré (texte) et numéro de version dessous.
fn logotype(ui: &mut egui::Ui, etat: &Etat) -> ZoneBarre {
    let nom = etat.i18n.t("app.title");
    // Le nom est coupé avant « Rust » pour une seconde couleur (logotype en deux tons).
    let (debut, fin) = match nom.strip_suffix("Rust") {
        Some(d) if !d.is_empty() => (d.to_string(), "Rust".to_string()),
        _ => (nom.clone(), String::new()),
    };
    let texte_debut = RichText::new(&debut)
        .size(18.0)
        .strong()
        .italics()
        .color(ORANGE_ACTIF);
    let texte_fin = RichText::new(&fin)
        .size(18.0)
        .strong()
        .italics()
        .color(Color32::from_rgb(0xE8, 0xE8, 0xE8));
    let largeur =
        22.0 + 4.0 + largeur_texte(ui, texte_debut.clone()) + largeur_texte(ui, texte_fin.clone());
    let hauteur = ui.available_height();
    let mut libelle = Rect::NOTHING;
    let reponse = ui.allocate_ui_with_layout(
        vec2(largeur + 4.0, hauteur),
        Layout::top_down(Align::Min),
        |ui| {
            ui.add_space((hauteur - 44.0).max(0.0) * 0.5);
            let ligne = ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add(icones::image(Icone::Logo, 22.0));
                ui.add_space(4.0);
                ui.add(egui::Label::new(texte_debut).extend());
                ui.add(egui::Label::new(texte_fin).extend());
            });
            libelle = ligne.response.rect;
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_space(30.0);
                ui.label(
                    RichText::new(env!("CARGO_PKG_VERSION"))
                        .size(11.0)
                        .color(ICONE_REPOS),
                );
            });
        },
    );
    ZoneBarre {
        zone: reponse.response.rect,
        libelle,
        largeur_texte: largeur,
    }
}

fn groupe(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie, g: &Groupe) -> ZoneBarre {
    let libelle = libelle_groupe(etat, g);
    let texte = largeur_texte(ui, libelle.clone());
    let largeur = (texte + MARGE_LIBELLE).max(COTE_ICONE + 20.0);
    let hauteur = ui.available_height();
    let mut rect_libelle = Rect::NOTHING;
    let reponse = ui.allocate_ui_with_layout(
        vec2(largeur, hauteur),
        Layout::top_down(Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.spacing_mut().button_padding = vec2(4.0, 2.0);
            ui.add_space(((hauteur - 62.0) * 0.5).max(0.0));
            let icone = Button::image(icones::image(g.icone, COTE_ICONE))
                .frame_when_inactive(false)
                .min_size(Vec2::new(COTE_ICONE + 14.0, COTE_ICONE + 8.0));
            if ui
                .add(icone)
                .on_hover_text(etat.i18n.t(g.action.cle()))
                .clicked()
            {
                sortie.push(Message::Commande(g.action));
            }
            ui.add_space(4.0);
            let bouton = Button::new(libelle)
                .right_text(
                    RichText::new(polices::DEROULANT)
                        .size(10.0)
                        .color(TEXTE_BARRE_OUTILS),
                )
                .wrap_mode(TextWrapMode::Extend)
                .frame_when_inactive(false);
            let (r, _) = MenuButton::from_button(bouton).ui(ui, |ui| {
                menus::afficher_entrees(ui, etat, g.entrees, sortie)
            });
            rect_libelle = r.rect;
        },
    );
    ZoneBarre {
        zone: reponse.response.rect,
        libelle: rect_libelle,
        largeur_texte: texte,
    }
}

/// Cloche des messages, avec pastille rouge du nombre de messages non lus.
fn cloche(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    let teinte = if etat.non_lus > 0 {
        ORANGE_ACTIF
    } else {
        ICONE_REPOS
    };
    let reponse = ui
        .add(
            Button::image(icones::image_teintee(Icone::Cloche, 24.0, teinte))
                .frame_when_inactive(false),
        )
        .on_hover_text(etat.i18n.t("app.messages"));
    if etat.non_lus > 0 {
        let centre = reponse.rect.right_top() + vec2(-7.0, 7.0);
        ui.painter().circle_filled(centre, 7.0, ROUGE_ETAT);
        let n = etat.non_lus.min(99).to_string();
        ui.painter().text(
            centre,
            Align2::CENTER_CENTER,
            n,
            FontId::proportional(9.5),
            Color32::WHITE,
        );
    }
    if reponse.clicked() {
        sortie.push(Message::OuvrirDialogue(TypeDialogue::Messages));
    }
}

/// Dessine la barre ; renvoie les zones de ses éléments (logotype puis quatre groupes).
pub fn afficher(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) -> Vec<ZoneBarre> {
    let mut zones = Vec::new();
    Panel::top("barre_outils")
        .exact_size(HAUTEUR)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            Frame::new()
                .fill(FOND_BARRES)
                .inner_margin(Margin::symmetric(12, 2)),
        )
        .show(ui, |ui| {
            ui.visuals_mut().override_text_color = Some(TEXTE_BARRE_OUTILS);
            let hauteur = ui.available_height();
            ui.allocate_ui_with_layout(
                vec2(ui.available_width(), hauteur),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    zones.push(logotype(ui, etat));
                    ui.add_space(14.0);
                    for g in &GROUPES {
                        zones.push(groupe(ui, etat, sortie, g));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 18.0;
                        cloche(ui, etat, sortie);
                        let texte = etat.i18n.t("gui.toolbar.unlocked");
                        if ui
                            .add(
                                Button::image(icones::image(Icone::Cadenas, 24.0))
                                    .frame_when_inactive(false),
                            )
                            .on_hover_text(&texte)
                            .clicked()
                        {
                            sortie.push(Message::Notifier(Niveau::Info, texte));
                        }
                    });
                },
            );
        });
    zones
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dessine la barre seule dans une fenêtre de largeur donnée ; renvoie les zones et le
    /// rectangle de la barre.
    fn mesurer(largeur: f32) -> Vec<ZoneBarre> {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let etat = Etat::nouveau();
        let mut zones = Vec::new();
        for _ in 0..3 {
            let entree = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(largeur, 720.0))),
                ..Default::default()
            };
            let mut sortie_egui = ctx.run_ui(entree, |ui| {
                let mut sortie = Vec::new();
                zones = afficher(ui, &etat, &mut sortie);
            });
            sortie_egui.textures_delta.clear();
        }
        zones
    }

    #[test]
    fn aucune_zone_ecrasee_quelle_que_soit_la_largeur() {
        for largeur in [1050.0, 1100.0, 1280.0, 1600.0, 1920.0, 2560.0] {
            let zones = mesurer(largeur);
            assert_eq!(zones.len(), 5, "logotype + quatre groupes");
            let mut droite_precedente = 0.0;
            for (i, z) in zones.iter().enumerate() {
                assert!(
                    z.libelle.width() + 0.5 >= z.largeur_texte,
                    "{largeur} px, zone {i} : libellé {} < texte {}",
                    z.libelle.width(),
                    z.largeur_texte
                );
                assert!(
                    z.libelle.height() < 30.0,
                    "{largeur} px, zone {i} : libellé sur plusieurs lignes ({})",
                    z.libelle.height()
                );
                assert!(z.zone.height() <= HAUTEUR, "zone {i} trop haute");
                assert!(z.zone.max.y <= HAUTEUR + 1.0, "zone {i} déborde");
                assert!(z.zone.max.x <= largeur, "zone {i} hors fenêtre");
                assert!(z.zone.min.x >= droite_precedente, "zones {i} chevauchées");
                droite_precedente = z.zone.max.x;
            }
            // Les quatre groupes tiennent largement dans la largeur minimale.
            assert!(droite_precedente < 1050.0 - 100.0, "{droite_precedente}");
        }
    }
}
