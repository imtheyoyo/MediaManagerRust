//! Fenêtre « Éditer un film » : fenêtre modale très grande (≈ 90 % de l'écran) à six onglets
//! (Détails, Détails (2), Acteur et équipe, Illustrations supplémentaires, URL de l'artwork de
//! bande annonce, Fichiers médias), édition sur une copie, validation à la confirmation
//! (Ctrl+Entrée ou Ctrl+S), annulation par Échap, édition à la suite de plusieurs films.
//!
//! La taille de la fenêtre ne dépend que de la taille de l'écran (jamais de l'espace
//! « disponible », qui ferait grandir la modale à chaque image) ; chaque onglet dessine dans un
//! rectangle de taille fixe.

pub(crate) mod composants;
mod detail;
mod detail2;
mod distribution;
mod fichiers;
mod illustrations;
pub mod serie;
mod urls;

use super::Sortie;
use super::details::{HAUTEUR_ENCOCHE, HAUTEUR_ONGLETS, bandeau_onglets, taille_lisible};
use crate::details_film::TypeArt;
use crate::dialogues::{ActionEditeur, ChampEditeur, EtatEditeur, FilmEdite, OngletEditeur};
use crate::etat::{ActionDialogue, Etat, Message};
use crate::i18n::I18n;
use crate::images::GestionnaireImages;
use crate::modele::Film;
use crate::source_donnees::GenreImage;
use crate::theme::{FOND_BARRES, ROUGE};
use eframe::egui::{
    self, Align, Frame, Id, Key, KeyboardShortcut, Layout, Margin, Modal, Modifiers, Rect,
    RichText, Sense, Stroke, UiBuilder, Vec2, pos2, vec2,
};
use std::path::PathBuf;

/// Hauteur de la ligne de titre de la fenêtre.
pub(super) const HAUTEUR_TITRE: f32 = 28.0;
/// Hauteur du pied (boutons Annuler / Valider).
pub(super) const HAUTEUR_PIED: f32 = 42.0;
/// Marges intérieures du corps d'un onglet.
pub(super) const MARGE_X: f32 = 12.0;
pub(super) const MARGE_Y: f32 = 8.0;

/// Taille de la fenêtre : ≈ 90 % de l'écran, bornée, et toujours contenue dans l'écran.
pub fn taille_fenetre(ecran: Vec2) -> Vec2 {
    let l = (ecran.x * 0.9).clamp(720.0, 2000.0).min(ecran.x - 24.0);
    let h = (ecran.y * 0.9).clamp(520.0, 1300.0).min(ecran.y - 24.0);
    vec2(l.max(200.0), h.max(200.0))
}

/// Ce que partagent les onglets : traductions, état de l'éditeur, film d'affichage.
pub(super) struct Contexte<'a> {
    pub i: &'a I18n,
    pub etat: &'a Etat,
    pub e: &'a EtatEditeur,
}

impl Contexte<'_> {
    /// Message d'erreur d'un champ invalide (texte rouge).
    pub fn erreur(&self, champ: ChampEditeur) -> bool {
        self.e.erreur(champ).is_some()
    }

    pub fn film(&self) -> Option<&Film> {
        self.etat.film(self.e.id)
    }
}

/// Chemin de l'illustration à afficher : celle des détails (sauf suppression demandée), à défaut
/// l'illustration connue de la liste des films.
pub(super) fn chemin_art(ct: &Contexte<'_>, b: &FilmEdite, t: TypeArt) -> Option<PathBuf> {
    if b.details.images.supprimes[t.rang()] {
        return None;
    }
    if let Some(p) = &b.details.images.chemins[t.rang()] {
        return Some(p.clone());
    }
    let genre = match t {
        TypeArt::Affiche => GenreImage::Affiche,
        TypeArt::Fanart => GenreImage::Fanart,
        TypeArt::Banniere => GenreImage::Banniere,
        TypeArt::Miniature => GenreImage::Miniature,
        _ => return None,
    };
    ct.etat.source.chemin_image(ct.film()?, genre)
}

/// Dimensions de l'illustration (celles des détails, sinon lues dans le fichier).
pub(super) fn dimensions_art(
    ctx: &egui::Context,
    b: &FilmEdite,
    t: TypeArt,
    chemin: Option<&std::path::Path>,
) -> Option<(u32, u32)> {
    b.details
        .images
        .dimensions(t)
        .or_else(|| chemin.and_then(|p| composants::dimensions_image(ctx, p)))
}

fn envoyer(sortie: &mut Sortie, a: ActionEditeur) {
    sortie.push(Message::Dialogue(ActionDialogue::Editeur(a)));
}

/// Texte du titre de la fenêtre : « Éditer un film  < chemin (taille) > ».
fn titre_fenetre(ct: &Contexte<'_>) -> String {
    let base = ct.i.t("movie.edit");
    let Some(f) = ct.film() else { return base };
    let chemin = f
        .chemin_video()
        .unwrap_or_else(|| std::path::PathBuf::from(f.chemin()));
    let taille = if f.taille_octets > 0 {
        format!(" ({})", taille_lisible(ct.etat, f.taille_octets))
    } else {
        String::new()
    };
    let rang = if ct.e.total > 1 {
        format!("  ({} / {})", ct.e.position(), ct.e.total)
    } else {
        String::new()
    };
    format!("{base}{rang}   < {}{taille} >", chemin.display())
}

pub fn afficher(
    ctx: &egui::Context,
    etat: &Etat,
    images: &mut GestionnaireImages,
    e: &EtatEditeur,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    let ct = Contexte { i, etat, e };
    let mut b = e.brouillon.clone();
    let taille = taille_fenetre(ctx.content_rect().size());
    // Actions du pied appliquées APRÈS le brouillon modifié dans la même image.
    let mut pied: Vec<ActionEditeur> = Vec::new();
    let mut onglet = None;
    let cadre = Frame::window(&ctx.global_style()).inner_margin(Margin::ZERO);
    let reponse = Modal::new(Id::unique("dlg_editeur"))
        .frame(cadre)
        .show(ctx, |ui| {
            ui.set_min_size(taille);
            ui.set_max_size(taille);
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            ligne_titre(ui, &titre_fenetre(&ct), taille.x, &mut pied);
            let libelles: Vec<String> = OngletEditeur::TOUS.iter().map(|o| i.t(o.cle())).collect();
            let actif = OngletEditeur::TOUS
                .iter()
                .position(|o| *o == e.onglet)
                .unwrap_or(0);
            onglet = bandeau_onglets(ui, "ed_onglets", &libelles, actif);
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
                OngletEditeur::Details => detail::afficher(&mut corps, &ct, &mut b, images, sortie),
                OngletEditeur::Details2 => detail2::afficher(&mut corps, &ct, &mut b),
                OngletEditeur::Distribution => distribution::afficher(&mut corps, &ct, &mut b),
                OngletEditeur::Illustrations => {
                    illustrations::afficher(&mut corps, &ct, &mut b, images, sortie)
                }
                OngletEditeur::Urls => urls::afficher(&mut corps, &ct, &mut b, sortie),
                OngletEditeur::Fichiers => fichiers::afficher(&mut corps, &ct, &mut b),
            }
            pied_fenetre(ui, &ct, taille.x, &mut pied);
        });
    if let Some(n) = onglet
        && OngletEditeur::TOUS[n] != e.onglet
    {
        envoyer(sortie, ActionEditeur::Onglet(OngletEditeur::TOUS[n]));
    }
    if b != e.brouillon {
        envoyer(sortie, ActionEditeur::Brouillon(Box::new(b)));
    }
    for a in pied {
        envoyer(sortie, a);
    }
    let valider = ctx.input_mut(|inp| {
        inp.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S))
            || inp.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Enter))
    });
    if valider {
        envoyer(sortie, ActionEditeur::Valider);
    } else if reponse.should_close() {
        envoyer(sortie, ActionEditeur::Abandonner);
    }
}

/// Ligne de titre : texte à gauche, croix de fermeture à droite.
fn ligne_titre(ui: &mut egui::Ui, titre: &str, largeur: f32, pied: &mut Vec<ActionEditeur>) {
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
    let r = ui.interact(croix, Id::unique("ed_fermer"), Sense::click());
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
        pied.push(ActionEditeur::Abandonner);
    }
}

/// Pied : message d'erreur et file d'attente à gauche, Annuler / Valider à droite.
fn pied_fenetre(ui: &mut egui::Ui, ct: &Contexte<'_>, largeur: f32, pied: &mut Vec<ActionEditeur>) {
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
        pied.push(ActionEditeur::Valider);
    }
    if composants::bouton_pied(
        &mut pied_ui,
        composants::GLYPHE_ANNULER,
        &i.t("Button.cancel"),
        110.0,
    )
    .clicked()
    {
        pied.push(ActionEditeur::Ignorer);
    }
    if ct.e.total > 1 && pied_ui.button(i.t("Button.abortqueue")).clicked() {
        pied.push(ActionEditeur::Abandonner);
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
