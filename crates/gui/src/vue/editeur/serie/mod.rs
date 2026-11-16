//! Fenêtre « Éditer une série » : fenêtre modale très grande (≈ 90 % de l'écran) à cinq onglets
//! (Détails, Acteur et équipe, Illustrations supplémentaires, URL de l'artwork, Épisodes),
//! même modèle que l'éditeur de film (composants partagés) : édition sur une copie, validation
//! à la confirmation (Ctrl+Entrée ou Ctrl+S), annulation par Échap.
//!
//! La taille de la fenêtre ne dépend que de la taille de l'écran (jamais de l'espace
//! « disponible », qui ferait grandir la modale à chaque image) ; chaque onglet dessine dans un
//! rectangle de taille fixe.

mod detail;
mod distribution;
mod episodes;
mod illustrations;
mod urls;

use super::composants;
use super::{HAUTEUR_PIED, HAUTEUR_TITRE, MARGE_X, MARGE_Y, taille_fenetre};
use crate::dialogues::{ActionEditeurSerie, ChampSerie, EtatEditeurSerie, OngletSerie, SerieEdite};
use crate::etat::{ActionDialogue, Etat, Message};
use crate::i18n::I18n;
use crate::images::GestionnaireImages;
use crate::source_donnees::SerieAffichage;
use crate::theme::{FOND_BARRES, ROUGE};
use crate::vue::Sortie;
use crate::vue::details::{HAUTEUR_ENCOCHE, HAUTEUR_ONGLETS, bandeau_onglets};
use eframe::egui::{
    self, Align, Frame, Id, Key, KeyboardShortcut, Layout, Margin, Modal, Modifiers, Rect,
    RichText, Sense, Stroke, UiBuilder, pos2, vec2,
};

/// Ce que partagent les onglets : traductions, état de l'éditeur, série d'affichage.
pub(super) struct Contexte<'a> {
    pub i: &'a I18n,
    pub etat: &'a Etat,
    pub e: &'a EtatEditeurSerie,
}

impl Contexte<'_> {
    /// Vrai si le champ est invalide (texte rouge).
    pub fn erreur(&self, champ: ChampSerie) -> bool {
        self.e.erreur(champ).is_some()
    }

    /// Série d'affichage courante (l'arbre des épisodes se lit ici : la bascule « vu » est
    /// appliquée tout de suite).
    pub fn serie(&self) -> Option<&SerieAffichage> {
        self.etat.series.iter().find(|s| s.id == self.e.id)
    }
}

fn envoyer(sortie: &mut Sortie, a: ActionEditeurSerie) {
    sortie.push(Message::Dialogue(ActionDialogue::EditeurSerie(a)));
}

/// Texte du titre de la fenêtre : « Éditer une série  < chemin > ».
fn titre_fenetre(ct: &Contexte<'_>) -> String {
    let base = ct.i.t("gui.series.edit");
    match ct.serie() {
        Some(s) if !s.chemin.is_empty() => format!("{base}   < {} >", s.chemin),
        _ => base,
    }
}

pub fn afficher(
    ctx: &egui::Context,
    etat: &Etat,
    images: &mut GestionnaireImages,
    e: &EtatEditeurSerie,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    let ct = Contexte { i, etat, e };
    let mut b: SerieEdite = e.brouillon.clone();
    let taille = taille_fenetre(ctx.content_rect().size());
    let mut pied: Vec<ActionEditeurSerie> = Vec::new();
    let mut onglet = None;
    let cadre = Frame::window(&ctx.global_style()).inner_margin(Margin::ZERO);
    let reponse = Modal::new(Id::unique("dlg_editeur_serie"))
        .frame(cadre)
        .show(ctx, |ui| {
            ui.set_min_size(taille);
            ui.set_max_size(taille);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            ligne_titre(ui, &titre_fenetre(&ct), taille.x, &mut pied);
            let libelles: Vec<String> = OngletSerie::TOUS.iter().map(|o| i.t(o.cle())).collect();
            let actif = OngletSerie::TOUS
                .iter()
                .position(|o| *o == e.onglet)
                .unwrap_or(0);
            onglet = bandeau_onglets(ui, "eds_onglets", &libelles, actif);
            let hauteur_corps =
                taille.y - HAUTEUR_TITRE - HAUTEUR_ONGLETS - HAUTEUR_ENCOCHE - HAUTEUR_PIED;
            let (rect, _) = ui.allocate_exact_size(vec2(taille.x, hauteur_corps), Sense::hover());
            let mut corps = ui.new_child(
                UiBuilder::new()
                    .max_rect(rect.shrink2(vec2(MARGE_X, MARGE_Y)))
                    .layout(Layout::top_down(Align::Min)),
            );
            corps.set_clip_rect(rect.intersect(ui.clip_rect()));
            corps.spacing_mut().item_spacing = vec2(6.0, 6.0);
            match e.onglet {
                OngletSerie::Details => detail::afficher(&mut corps, &ct, &mut b, images, sortie),
                OngletSerie::Distribution => distribution::afficher(&mut corps, &ct, &mut b),
                OngletSerie::Illustrations => {
                    illustrations::afficher(&mut corps, &mut b, i, images, sortie)
                }
                OngletSerie::Urls => urls::afficher(&mut corps, &ct, &mut b),
                OngletSerie::Episodes => episodes::afficher(&mut corps, &ct, &mut pied),
            }
            pied_fenetre(ui, &ct, taille.x, &mut pied);
        });
    if let Some(n) = onglet
        && OngletSerie::TOUS[n] != e.onglet
    {
        envoyer(sortie, ActionEditeurSerie::Onglet(OngletSerie::TOUS[n]));
    }
    if b != e.brouillon {
        envoyer(sortie, ActionEditeurSerie::Brouillon(Box::new(b)));
    }
    for a in pied {
        envoyer(sortie, a);
    }
    let valider = ctx.input_mut(|inp| {
        inp.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S))
            || inp.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Enter))
    });
    if valider {
        envoyer(sortie, ActionEditeurSerie::Valider);
    } else if reponse.should_close() {
        envoyer(sortie, ActionEditeurSerie::Abandonner);
    }
}

/// Ligne de titre : texte à gauche, croix de fermeture à droite.
fn ligne_titre(ui: &mut egui::Ui, titre: &str, largeur: f32, pied: &mut Vec<ActionEditeurSerie>) {
    let (rect, _) = ui.allocate_exact_size(vec2(largeur, HAUTEUR_TITRE), Sense::hover());
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    p.rect_filled(rect, 0.0, FOND_BARRES);
    p.text(
        rect.left_center() + vec2(12.0, 0.0),
        egui::Align2::LEFT_CENTER,
        titre,
        egui::FontId::proportional(13.0),
        ui.visuals().text_color(),
    );
    let croix = Rect::from_center_size(rect.right_center() - vec2(16.0, 0.0), vec2(22.0, 22.0));
    let r = ui.interact(croix, Id::unique("eds_fermer"), Sense::click());
    let c = if r.hovered() {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().text_color()
    };
    let t = Stroke::new(1.4, c);
    let d = 5.0;
    p.line_segment(
        [croix.center() + vec2(-d, -d), croix.center() + vec2(d, d)],
        t,
    );
    p.line_segment(
        [croix.center() + vec2(-d, d), croix.center() + vec2(d, -d)],
        t,
    );
    if r.clicked() {
        pied.push(ActionEditeurSerie::Abandonner);
    }
}

/// Pied : message d'erreur à gauche, Annuler / Valider à droite.
fn pied_fenetre(
    ui: &mut egui::Ui,
    ct: &Contexte<'_>,
    largeur: f32,
    pied: &mut Vec<ActionEditeurSerie>,
) {
    let i = ct.i;
    let (rect, _) = ui.allocate_exact_size(vec2(largeur, HAUTEUR_PIED), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.top() + 0.5,
        Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
    );
    let zone = Rect::from_min_max(
        pos2(rect.left() + 12.0, rect.top() + 8.0),
        pos2(rect.right() - 12.0, rect.bottom() - 8.0),
    );
    let mut pied_ui = ui.new_child(
        UiBuilder::new()
            .max_rect(zone)
            .layout(Layout::right_to_left(Align::Center)),
    );
    pied_ui.set_clip_rect(zone.intersect(ui.clip_rect()));
    pied_ui.spacing_mut().item_spacing.x = 8.0;
    if composants::bouton_pied(
        &mut pied_ui,
        composants::GLYPHE_VALIDER,
        &i.t("Button.ok"),
        110.0,
    )
    .on_hover_text("Ctrl+Entrée")
    .clicked()
    {
        pied.push(ActionEditeurSerie::Valider);
    }
    if composants::bouton_pied(
        &mut pied_ui,
        composants::GLYPHE_ANNULER,
        &i.t("Button.cancel"),
        110.0,
    )
    .clicked()
    {
        pied.push(ActionEditeurSerie::Abandonner);
    }
    if !ct.e.erreurs.is_empty() {
        pied_ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    crate::polices::AVERTISSEMENT,
                    i.t("editor.invalid")
                ))
                .color(ROUGE),
            );
        });
    }
}

#[cfg(test)]
mod tests;
