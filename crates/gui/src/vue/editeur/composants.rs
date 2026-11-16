//! Éléments partagés des onglets de l'éditeur de film : boutons à pictogramme dessinés au
//! pinceau (plus, moins, haut, bas, lecture...), libellés alignés, listes déroulantes, sélecteur de date avec
//! calendrier, emplacements d'illustration et tableaux éditables (egui_extras).

use crate::dates;
use crate::i18n::I18n;
use crate::icones::{self, Icone};
use crate::images::{CleImage, EtatImage, GestionnaireImages};
use crate::polices;
use crate::theme::{BLEU_ETAT, FILET_TABLEAU, LIEN};
use crate::vue::ajuster;
use crate::vue::tableau::{entete_icone, entete_texte, filets};
use eframe::egui::{
    self, Align, Align2, Color32, ComboBox, CornerRadius, FontId, Frame, Id, Layout, Margin,
    Painter, Popup, PopupCloseBehavior, Pos2, Rect, Response, RichText, Sense, Shape, Stroke,
    StrokeKind, TextEdit, Ui, Vec2, pos2, vec2,
};
use egui_extras::{Column, TableBuilder, TableRow};
use std::path::Path;

/// Hauteur d'un champ de saisie.
pub const HAUTEUR_CHAMP: f32 = 22.0;
/// Hauteur d'une ligne de tableau (lignes denses).
pub const HAUTEUR_LIGNE: f32 = 22.0;
/// Hauteur de l'en-tête d'un tableau.
pub const HAUTEUR_ENTETE: f32 = 24.0;
/// Côté des boutons à pictogramme.
pub const COTE_BOUTON: Vec2 = Vec2::new(26.0, 24.0);

const FOND_BOUTON: Color32 = Color32::from_rgb(0x4B, 0x4E, 0x50);
const FOND_BOUTON_SURVOL: Color32 = Color32::from_rgb(0x5B, 0x5F, 0x62);
const TRAIT_BOUTON: Color32 = Color32::from_rgb(0xDD, 0xDD, 0xDD);
/// Fond des emplacements d'illustration vides.
const FOND_EMPLACEMENT: Color32 = Color32::from_rgb(0x8D, 0xA5, 0xB5);
const ENCRE_EMPLACEMENT: Color32 = Color32::from_rgb(0x2B, 0x2B, 0x2C);

// ------------------------------------------------------------------ pictogrammes

/// Pictogramme dessiné au pinceau (indépendant des polices du système).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyphe {
    Plus,
    Moins,
    Haut,
    Bas,
    Lire,
    Telecharger,
    Crayon,
    Calendrier,
    Corbeille,
    FlecheBas,
}

/// Peint le pictogramme `g` centré sur `c` (côté ≈ 12 px).
pub fn peindre_glyphe(p: &Painter, c: Pos2, g: Glyphe, couleur: Color32) {
    let t = Stroke::new(1.7, couleur);
    let fin = Stroke::new(1.2, couleur);
    let s = 5.0;
    match g {
        Glyphe::Plus => {
            p.line_segment([c + vec2(-s, 0.0), c + vec2(s, 0.0)], t);
            p.line_segment([c + vec2(0.0, -s), c + vec2(0.0, s)], t);
        }
        Glyphe::Moins => {
            p.line_segment([c + vec2(-s, 0.0), c + vec2(s, 0.0)], t);
        }
        Glyphe::Haut => {
            p.line(
                vec![
                    c + vec2(-s, s * 0.45),
                    c + vec2(0.0, -s * 0.45),
                    c + vec2(s, s * 0.45),
                ],
                t,
            );
        }
        Glyphe::Bas | Glyphe::FlecheBas => {
            p.line(
                vec![
                    c + vec2(-s, -s * 0.45),
                    c + vec2(0.0, s * 0.45),
                    c + vec2(s, -s * 0.45),
                ],
                if g == Glyphe::Bas { t } else { fin },
            );
        }
        Glyphe::Lire => {
            p.add(Shape::closed_line(
                vec![
                    c + vec2(-s * 0.6, -s),
                    c + vec2(s * 0.9, 0.0),
                    c + vec2(-s * 0.6, s),
                ],
                fin,
            ));
        }
        Glyphe::Telecharger => {
            p.line_segment([c + vec2(0.0, -s), c + vec2(0.0, s * 0.4)], fin);
            p.line(
                vec![
                    c + vec2(-s * 0.55, -s * 0.15),
                    c + vec2(0.0, s * 0.4),
                    c + vec2(s * 0.55, -s * 0.15),
                ],
                fin,
            );
            p.line(
                vec![
                    c + vec2(-s, s * 0.2),
                    c + vec2(-s, s),
                    c + vec2(s, s),
                    c + vec2(s, s * 0.2),
                ],
                fin,
            );
        }
        Glyphe::Crayon => {
            // Cadre ouvert et trait de plume.
            p.line(
                vec![
                    c + vec2(s * 0.5, -s),
                    c + vec2(-s, -s),
                    c + vec2(-s, s),
                    c + vec2(s, s),
                    c + vec2(s, -s * 0.5),
                ],
                fin,
            );
            p.line_segment([c + vec2(-s * 0.2, s * 0.2), c + vec2(s, -s)], t);
        }
        Glyphe::Calendrier => {
            let r = Rect::from_center_size(c + vec2(0.0, 1.0), vec2(2.0 * s, 2.0 * s - 1.0));
            p.rect_stroke(r, CornerRadius::same(1), fin, StrokeKind::Inside);
            p.hline(r.x_range(), r.top() + 3.0, fin);
            for dx in [-s * 0.5, s * 0.5] {
                p.line_segment(
                    [pos2(c.x + dx, r.top() - 2.0), pos2(c.x + dx, r.top() + 1.0)],
                    fin,
                );
            }
            for (dx, dy) in [(-2.5, 2.5), (0.0, 2.5), (2.5, 2.5), (-2.5, 5.5), (0.0, 5.5)] {
                p.circle_filled(c + vec2(dx, dy), 0.7, couleur);
            }
        }
        Glyphe::Corbeille => {
            p.line_segment([c + vec2(-s, -s * 0.6), c + vec2(s, -s * 0.6)], fin);
            p.line_segment(
                [c + vec2(-s * 0.35, -s * 0.6), c + vec2(-s * 0.35, -s)],
                fin,
            );
            p.line_segment([c + vec2(s * 0.35, -s * 0.6), c + vec2(s * 0.35, -s)], fin);
            p.line_segment([c + vec2(-s * 0.35, -s), c + vec2(s * 0.35, -s)], fin);
            p.line(
                vec![
                    c + vec2(-s * 0.75, -s * 0.6),
                    c + vec2(-s * 0.55, s),
                    c + vec2(s * 0.55, s),
                    c + vec2(s * 0.75, -s * 0.6),
                ],
                fin,
            );
        }
    }
}

/// Bouton carré sombre à coins arrondis portant un pictogramme (plus, moins, haut, bas...).
pub fn bouton(ui: &mut Ui, g: Glyphe, actif: bool, infobulle: &str) -> Response {
    bouton_taille(ui, g, COTE_BOUTON, actif, infobulle)
}

pub fn bouton_taille(
    ui: &mut Ui,
    g: Glyphe,
    taille: Vec2,
    actif: bool,
    infobulle: &str,
) -> Response {
    let sens = if actif {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, reponse) = ui.allocate_exact_size(taille, sens);
    let fond = if actif && reponse.hovered() {
        FOND_BOUTON_SURVOL
    } else {
        FOND_BOUTON
    };
    let couleur = if actif {
        TRAIT_BOUTON
    } else {
        TRAIT_BOUTON.gamma_multiply(0.35)
    };
    ui.painter().rect_filled(rect, CornerRadius::same(4), fond);
    peindre_glyphe(ui.painter(), rect.center(), g, couleur);
    if infobulle.is_empty() {
        reponse
    } else {
        reponse.on_hover_text(infobulle)
    }
}

/// Pictogramme cliquable sans fond (cellules de tableau, bouton de lecture).
pub fn bouton_nu(
    ui: &mut Ui,
    g: Glyphe,
    couleur: Color32,
    actif: bool,
    infobulle: &str,
) -> Response {
    let sens = if actif {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, reponse) = ui.allocate_exact_size(vec2(18.0, HAUTEUR_LIGNE - 2.0), sens);
    let couleur = if !actif {
        couleur.gamma_multiply(0.35)
    } else if reponse.hovered() {
        couleur.gamma_multiply(1.35)
    } else {
        couleur
    };
    peindre_glyphe(ui.painter(), rect.center(), g, couleur);
    if infobulle.is_empty() {
        reponse
    } else {
        reponse.on_hover_text(infobulle)
    }
}

/// Colonne de boutons plus, moins (haut, bas) à gauche d'une liste ; renvoie les boutons cliqués.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClicsListe {
    pub ajouter: bool,
    pub retirer: bool,
    pub monter: bool,
    pub descendre: bool,
}

pub fn boutons_liste(
    ui: &mut Ui,
    i: &I18n,
    avec_deplacement: bool,
    peut_retirer: bool,
    peut_monter: bool,
    peut_descendre: bool,
) -> ClicsListe {
    let mut c = ClicsListe::default();
    ui.spacing_mut().item_spacing.y = 4.0;
    c.ajouter = bouton(ui, Glyphe::Plus, true, &i.t("Button.add")).clicked();
    c.retirer = bouton(ui, Glyphe::Moins, peut_retirer, &i.t("Button.remove")).clicked();
    if avec_deplacement {
        c.monter = bouton(ui, Glyphe::Haut, peut_monter, &i.t("Button.moveup")).clicked();
        c.descendre = bouton(ui, Glyphe::Bas, peut_descendre, &i.t("Button.movedown")).clicked();
    }
    c
}

/// Crayon bleu des colonnes d'édition.
pub fn crayon(ui: &mut Ui, infobulle: &str) -> Response {
    bouton_nu(ui, Glyphe::Crayon, BLEU_ETAT, true, infobulle)
}

// ------------------------------------------------------------------ libellés et champs

/// Libellé en gras aligné à droite dans une cellule de largeur fixe.
pub fn etiquette(ui: &mut Ui, largeur: f32, texte: &str) {
    ui.allocate_ui_with_layout(
        vec2(largeur, HAUTEUR_CHAMP),
        Layout::right_to_left(Align::Center),
        |ui| {
            ui.label(RichText::new(texte).strong());
        },
    );
}

/// Libellé en gras aligné à droite et en haut (zones de texte, listes).
pub fn etiquette_haut(ui: &mut Ui, largeur: f32, texte: &str) {
    ui.allocate_ui_with_layout(
        vec2(largeur, HAUTEUR_CHAMP),
        Layout::right_to_left(Align::Min),
        |ui| {
            ui.add_space(0.0);
            ui.label(RichText::new(texte).strong());
        },
    );
}

/// Champ de saisie d'une ligne de largeur donnée ; le texte devient rouge si `erreur`.
pub fn champ_texte(ui: &mut Ui, texte: &mut String, largeur: f32, erreur: bool) -> Response {
    let mut t = TextEdit::singleline(texte)
        .desired_width(largeur)
        .margin(Margin::symmetric(5, 3));
    if erreur {
        t = t.text_color(crate::theme::ROUGE);
    }
    ui.add_sized(vec2(largeur, HAUTEUR_CHAMP), t)
}

/// Zone de texte de plusieurs lignes, de hauteur fixe.
pub fn zone_texte(ui: &mut Ui, texte: &mut String, largeur: f32, hauteur: f32) -> Response {
    let lignes = ((hauteur - 8.0) / 15.0).floor().max(1.0) as usize;
    ui.add_sized(
        vec2(largeur, hauteur),
        TextEdit::multiline(texte)
            .desired_width(largeur)
            .desired_rows(lignes)
            .margin(Margin::symmetric(5, 4)),
    )
}

/// Nombre entier éditable (flèches de la souris, glisser).
pub fn nombre_entier(ui: &mut Ui, valeur: &mut i64, largeur: f32, min: i64, max: i64) -> Response {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = HAUTEUR_CHAMP;
        ui.add_sized(
            vec2(largeur, HAUTEUR_CHAMP),
            egui::DragValue::new(valeur).range(min..=max).speed(1.0),
        )
    })
    .inner
}

/// Liste déroulante non éditable : `options` = (valeur, libellé) ; renvoie vrai si changée.
pub fn liste_deroulante(
    ui: &mut Ui,
    id: &str,
    largeur: f32,
    valeur: &mut String,
    options: &[(String, String)],
) -> bool {
    let mut change = false;
    let affiche = options
        .iter()
        .find(|(v, _)| v == valeur)
        .map(|(_, l)| l.clone())
        .unwrap_or_else(|| valeur.clone());
    ComboBox::from_id_salt(id)
        .width(largeur - 18.0)
        .selected_text(affiche)
        .show_ui(ui, |ui| {
            for (v, l) in options {
                let texte = if l.is_empty() { " " } else { l.as_str() };
                if ui.selectable_label(v == valeur, texte).clicked() && v != valeur {
                    *valeur = v.clone();
                    change = true;
                }
            }
        });
    change
}

/// Champ de saisie avec flèche de liste déroulante : la sélection d'une proposition remplit
/// le texte. Renvoie (réponse du champ, vrai si Entrée a été pressée dans le champ).
pub fn champ_avec_liste(
    ui: &mut Ui,
    texte: &mut String,
    largeur: f32,
    propositions: &[String],
) -> (Response, bool) {
    let r = champ_texte(ui, texte, largeur - 22.0, false);
    let entree = r.lost_focus() && ui.input(|inp| inp.key_pressed(egui::Key::Enter));
    let (rect, fleche) = ui.allocate_exact_size(vec2(22.0, HAUTEUR_CHAMP), Sense::click());
    let fond = if fleche.hovered() {
        FOND_BOUTON_SURVOL
    } else {
        FOND_BOUTON
    };
    ui.painter().rect_filled(rect, CornerRadius::same(3), fond);
    peindre_glyphe(ui.painter(), rect.center(), Glyphe::FlecheBas, TRAIT_BOUTON);
    Popup::menu(&fleche)
        .close_behavior(PopupCloseBehavior::CloseOnClick)
        .width(largeur)
        .show(|ui| {
            egui::ScrollArea::vertical()
                .max_height(240.0)
                .show(ui, |ui| {
                    for p in propositions {
                        if ui.selectable_label(p == texte, p).clicked() {
                            *texte = p.clone();
                        }
                    }
                });
        });
    (r, entree)
}

// ------------------------------------------------------------------ sélecteur de date

/// Champ de date `jj/mm/aaaa` (ou avec heure si `avec_heure`) et bouton calendrier. Le calendrier
/// ne modifie que la date : l'heure saisie est conservée.
pub fn champ_date(
    ui: &mut Ui,
    i: &I18n,
    id: &str,
    texte: &mut String,
    largeur: f32,
    avec_heure: bool,
    erreur: bool,
) {
    ui.spacing_mut().item_spacing.x = 0.0;
    champ_texte(ui, texte, largeur - 26.0, erreur);
    let (rect, bouton_cal) = ui.allocate_exact_size(vec2(26.0, HAUTEUR_CHAMP), Sense::click());
    let fond = if bouton_cal.hovered() {
        FOND_BOUTON_SURVOL
    } else {
        FOND_BOUTON
    };
    ui.painter().rect_filled(rect, CornerRadius::same(3), fond);
    peindre_glyphe(
        ui.painter(),
        rect.center(),
        Glyphe::Calendrier,
        TRAIT_BOUTON,
    );
    let bouton_cal = bouton_cal.on_hover_text(i.t("editor.pickdate"));
    let id_etat = Id::unique(("calendrier_mois", id));
    Popup::from_toggle_button_response(&bouton_cal)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            let (date, heure) = match texte.trim().split_once(' ') {
                Some((d, h)) => (d.to_string(), h.to_string()),
                None => (texte.trim().to_string(), String::new()),
            };
            let courante = dates::composants_fr(&date);
            let (mut annee, mut mois) = ui
                .data(|d| d.get_temp::<(i32, u32)>(id_etat))
                .or_else(|| courante.map(|(a, m, _)| (a, m)))
                .unwrap_or((2026, 1));
            let mois_noms: Vec<String> = i
                .t("editor.cal.months")
                .split(',')
                .map(str::to_string)
                .collect();
            let jours_noms: Vec<String> = i
                .t("editor.cal.weekdays")
                .split(',')
                .map(str::to_string)
                .collect();
            ui.horizontal(|ui| {
                if bouton_taille(ui, Glyphe::Haut, vec2(22.0, 20.0), true, "").clicked() {
                    (annee, mois) = dates::decaler_mois(annee, mois, -1);
                }
                let nom = mois_noms
                    .get(mois as usize - 1)
                    .cloned()
                    .unwrap_or_default();
                ui.label(RichText::new(format!("{nom} {annee}")).strong());
                if bouton_taille(ui, Glyphe::Bas, vec2(22.0, 20.0), true, "").clicked() {
                    (annee, mois) = dates::decaler_mois(annee, mois, 1);
                }
            });
            ui.data_mut(|d| d.insert_temp(id_etat, (annee, mois)));
            let decalage = dates::jour_semaine(annee, mois, 1);
            let nb = dates::jours_dans_mois(annee, mois);
            let mut choisi = None;
            egui::Grid::new(("calendrier_grille", id))
                .spacing([2.0, 2.0])
                .show(ui, |ui| {
                    for nom in &jours_noms {
                        ui.label(RichText::new(nom).weak().small());
                    }
                    ui.end_row();
                    for case in 0..42u32 {
                        let jour = case as i32 - decalage as i32 + 1;
                        if jour >= 1 && jour as u32 <= nb {
                            let est_courant = courante == Some((annee, mois, jour as u32));
                            let b = ui.add_sized(
                                vec2(26.0, 20.0),
                                egui::Button::new(jour.to_string()).selected(est_courant),
                            );
                            if b.clicked() {
                                choisi = Some(jour as u32);
                            }
                        } else {
                            ui.allocate_space(vec2(26.0, 20.0));
                        }
                        if case % 7 == 6 {
                            ui.end_row();
                        }
                    }
                });
            if let Some(j) = choisi {
                *texte = if avec_heure {
                    format!(
                        "{j:02}/{mois:02}/{annee:04} {}",
                        if heure.is_empty() { "00:00" } else { &heure }
                    )
                } else {
                    format!("{j:02}/{mois:02}/{annee:04}")
                };
                ui.data_mut(|d| d.remove::<(i32, u32)>(id_etat));
                Popup::close_all(ui.ctx());
            }
        });
}

// ------------------------------------------------------------------ illustrations

/// Emplacement gris-bleu avec le pictogramme « image » (illustration absente).
pub fn emplacement_vide(ui: &mut Ui, taille: Vec2) -> Response {
    let (rect, reponse) = ui.allocate_exact_size(taille, Sense::hover());
    peindre_emplacement_vide(ui.painter(), rect, ui);
    reponse
}

fn peindre_emplacement_vide(p: &Painter, rect: Rect, ui: &Ui) {
    p.rect_filled(rect, CornerRadius::ZERO, FOND_EMPLACEMENT);
    let cote = (rect.width().min(rect.height()) * 0.42).clamp(20.0, 130.0);
    icones::image_teintee(Icone::Image, cote, ENCRE_EMPLACEMENT)
        .paint_at(ui, Rect::from_center_size(rect.center(), Vec2::splat(cote)));
}

/// Illustration dans un cadre de `taille` (cadre blanc fin, image ajustée) ; un emplacement
/// gris avec le pictogramme « image » tient lieu d'image absente. Clic : visionneuse.
pub fn illustration(
    ui: &mut Ui,
    images: &mut GestionnaireImages,
    chemin: Option<&Path>,
    taille: Vec2,
    avec_cadre_blanc: bool,
) -> Response {
    let Some(chemin) = chemin else {
        return emplacement_vide(ui, taille);
    };
    let (rect, reponse) = ui.allocate_exact_size(taille, Sense::click());
    match images.image(&CleImage::miniature(chemin)) {
        EtatImage::Prete(tex) => {
            if avec_cadre_blanc {
                ui.painter()
                    .rect_filled(rect, CornerRadius::ZERO, Color32::WHITE);
            }
            let interieur = if avec_cadre_blanc {
                rect.shrink(2.0)
            } else {
                rect
            };
            let cible = ajuster(interieur, tex.size_vec2());
            ui.painter().image(
                tex.id(),
                cible,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        EtatImage::EnCours => {
            ui.painter()
                .rect_filled(rect, CornerRadius::ZERO, FOND_EMPLACEMENT);
            egui::Spinner::new()
                .paint_at(ui, Rect::from_center_size(rect.center(), Vec2::splat(20.0)));
        }
        EtatImage::Erreur => peindre_emplacement_vide(ui.painter(), rect, ui),
    }
    reponse.on_hover_cursor(egui::CursorIcon::ZoomIn)
}

/// Dimensions d'une image locale (lecture de l'en-tête, mémorisée entre les images).
pub fn dimensions_image(ctx: &egui::Context, chemin: &Path) -> Option<(u32, u32)> {
    let id = Id::unique(("editeur_dimensions", chemin));
    if let Some(d) = ctx.data(|d| d.get_temp::<Option<(u32, u32)>>(id)) {
        return d;
    }
    let d = image::image_dimensions(chemin).ok();
    ctx.data_mut(|m| m.insert_temp(id, d));
    d
}

/// Ligne d'en-tête d'une illustration : libellé en gras, dimensions (lien), corbeille.
/// Renvoie (clic sur les dimensions, clic sur la corbeille).
pub fn entete_illustration(
    ui: &mut Ui,
    i: &I18n,
    libelle: &str,
    dimensions: Option<(u32, u32)>,
    avec_corbeille: bool,
) -> (bool, bool) {
    let mut ouvrir = false;
    let mut supprimer = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(libelle).strong());
        if let Some((l, h)) = dimensions {
            ouvrir = ui
                .add(
                    egui::Label::new(RichText::new(format!("{l}x{h}")).color(LIEN).underline())
                        .sense(Sense::click()),
                )
                .on_hover_text(i.t("editor.art.open"))
                .clicked();
            if avec_corbeille {
                supprimer = bouton_nu(
                    ui,
                    Glyphe::Corbeille,
                    TRAIT_BOUTON,
                    true,
                    &i.t("editor.art.remove"),
                )
                .clicked();
            }
        }
    });
    (ouvrir, supprimer)
}

// ------------------------------------------------------------------ tableaux éditables

/// En-tête d'une colonne de tableau éditable.
#[derive(Clone, Debug)]
pub enum Entete {
    Texte(String),
    Icone(Icone),
    Glyphe(Glyphe),
    Vide,
}

/// Colonne d'un tableau éditable : largeur et en-tête.
pub struct ColonneEdition {
    pub colonne: Column,
    pub entete: Entete,
}

impl ColonneEdition {
    pub fn fixe(largeur: f32, titre: impl Into<String>) -> Self {
        Self {
            colonne: Column::exact(largeur),
            entete: Entete::Texte(titre.into()),
        }
    }

    pub fn souple(largeur: f32, titre: impl Into<String>) -> Self {
        Self {
            colonne: Column::initial(largeur).at_least(40.0).clip(true),
            entete: Entete::Texte(titre.into()),
        }
    }

    pub fn reste(titre: impl Into<String>) -> Self {
        Self {
            colonne: Column::remainder().at_least(40.0).clip(true),
            entete: Entete::Texte(titre.into()),
        }
    }

    /// Colonne étroite à pictogramme (en-tête `entete`).
    pub fn pictogramme(entete: Entete) -> Self {
        Self {
            colonne: Column::exact(26.0),
            entete,
        }
    }
}

/// Tableau éditable à lignes sélectionnables et à filets, de hauteur fixe `hauteur` : des lignes
/// vides complètent la hauteur. `ligne(rang, ligne)` dessine les cellules d'une ligne et
/// renvoie vrai si l'utilisateur a agi dans une de ses cellules (la ligne est alors
/// sélectionnée). `largeur` est la largeur totale du tableau.
#[allow(clippy::too_many_arguments)]
pub fn tableau_edition(
    ui: &mut Ui,
    id: &str,
    largeur: f32,
    hauteur: f32,
    colonnes: Vec<ColonneEdition>,
    nb_lignes: usize,
    selection: &mut Option<usize>,
    mut ligne: impl FnMut(&mut TableRow<'_, '_>, usize) -> bool,
) {
    let nb_col = colonnes.len();
    let visibles = ((hauteur - HAUTEUR_ENTETE) / HAUTEUR_LIGNE)
        .floor()
        .max(1.0) as usize;
    let total = nb_lignes.max(visibles);
    let entetes: Vec<Entete> = colonnes.iter().map(|c| c.entete.clone()).collect();
    ui.allocate_ui_with_layout(vec2(largeur, hauteur), Layout::top_down(Align::Min), |ui| {
        ui.set_min_size(vec2(largeur, hauteur));
        ui.set_max_size(vec2(largeur, hauteur));
        let mut t = TableBuilder::new(ui)
            .id_salt(id)
            .striped(true)
            .resizable(false)
            .sense(Sense::click())
            .min_scrolled_height(hauteur - HAUTEUR_ENTETE)
            .max_scroll_height(hauteur - HAUTEUR_ENTETE)
            .auto_shrink([false, false]);
        for c in colonnes {
            t = t.column(c.colonne);
        }
        t.header(HAUTEUR_ENTETE, |mut h| {
            for e in &entetes {
                h.col(|ui| match e {
                    Entete::Texte(t) => {
                        entete_texte(ui, t.clone());
                    }
                    Entete::Icone(ic) => {
                        entete_icone(ui, *ic);
                    }
                    Entete::Glyphe(g) => {
                        filets(ui);
                        let (rect, _) = ui.allocate_exact_size(
                            vec2(ui.available_width(), ui.available_height()),
                            Sense::hover(),
                        );
                        peindre_glyphe(ui.painter(), rect.center(), *g, TRAIT_BOUTON);
                    }
                    Entete::Vide => filets(ui),
                });
            }
        })
        .body(|body| {
            body.rows(HAUTEUR_LIGNE, total, |mut row| {
                let n = row.index();
                if n < nb_lignes {
                    row.set_selected(*selection == Some(n));
                    let agi = ligne(&mut row, n);
                    if agi || row.response().clicked() {
                        *selection = Some(n);
                    }
                } else {
                    for _ in 0..nb_col {
                        row.col(|ui| filets(ui));
                    }
                }
            });
        });
    });
}

fn cellule_sans_cadre(ui: &mut Ui) {
    let v = ui.visuals_mut();
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.active.weak_bg_fill = Color32::TRANSPARENT;
    v.extreme_bg_color = Color32::TRANSPARENT;
}

/// Cellule de texte éditable.
pub fn cellule_texte(row: &mut TableRow<'_, '_>, texte: &mut String) -> bool {
    let mut agi = false;
    row.col(|ui| {
        filets(ui);
        cellule_sans_cadre(ui);
        let l = (ui.available_width() - 2.0).max(10.0);
        let r = ui.add(
            TextEdit::singleline(texte)
                .frame(Frame::NONE)
                .desired_width(l)
                .margin(Margin::symmetric(5, 2)),
        );
        agi = r.gained_focus() || r.clicked() || r.changed();
    });
    agi
}

/// Cellule numérique éditable (entier non négatif), alignée à droite.
pub fn cellule_entier(row: &mut TableRow<'_, '_>, valeur: &mut u32) -> bool {
    let mut agi = false;
    row.col(|ui| {
        filets(ui);
        cellule_sans_cadre(ui);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(4.0);
            let mut v = i64::from(*valeur);
            let r = ui.add(egui::DragValue::new(&mut v).range(0..=u32::MAX).speed(1.0));
            *valeur = u32::try_from(v.max(0)).unwrap_or(u32::MAX);
            agi = r.gained_focus() || r.clicked() || r.changed() || r.dragged();
        });
    });
    agi
}

/// Cellule décimale éditable, alignée à droite.
pub fn cellule_decimal(row: &mut TableRow<'_, '_>, valeur: &mut f32) -> bool {
    let mut agi = false;
    row.col(|ui| {
        filets(ui);
        cellule_sans_cadre(ui);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(4.0);
            let r = ui.add(
                egui::DragValue::new(valeur)
                    .range(0.0..=1_000_000.0)
                    .speed(0.1)
                    .max_decimals(2),
            );
            agi = r.gained_focus() || r.clicked() || r.changed() || r.dragged();
        });
    });
    agi
}

/// Cellule à case à cocher centrée.
pub fn cellule_case(row: &mut TableRow<'_, '_>, valeur: &mut bool) -> bool {
    let mut agi = false;
    row.col(|ui| {
        filets(ui);
        ui.with_layout(
            Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| {
                agi = ui.checkbox(valeur, "").changed();
            },
        );
    });
    agi
}

/// Cellule à pictogramme d'état (coche verte / croix rouge).
pub fn cellule_etat_icone(row: &mut TableRow<'_, '_>, oui: bool) {
    row.col(|ui| {
        filets(ui);
        icones::cellule_etat(
            ui,
            if oui {
                icones::EtatCellule::Oui
            } else {
                icones::EtatCellule::Non
            },
        );
    });
}

/// Cellule à crayon : renvoie vrai au clic.
pub fn cellule_crayon(row: &mut TableRow<'_, '_>, infobulle: &str) -> bool {
    let mut clic = false;
    row.col(|ui| {
        filets(ui);
        ui.with_layout(
            Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| clic = crayon(ui, infobulle).clicked(),
        );
    });
    clic
}

/// Cellule à pictogramme cliquable.
pub fn cellule_glyphe(
    row: &mut TableRow<'_, '_>,
    g: Glyphe,
    couleur: Color32,
    actif: bool,
    infobulle: &str,
) -> bool {
    let mut clic = false;
    row.col(|ui| {
        filets(ui);
        ui.with_layout(
            Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| clic = bouton_nu(ui, g, couleur, actif, infobulle).clicked(),
        );
    });
    clic
}

/// Liste simple (texte, ligne sélectionnable) de hauteur fixe, avec filets : éléments des
/// genres, mots clés et liens vers les séries.
pub fn liste_selection(
    ui: &mut Ui,
    id: &str,
    largeur: f32,
    hauteur: f32,
    elements: &[String],
    selection: &mut Option<usize>,
) {
    let (rect, _) = ui.allocate_exact_size(vec2(largeur, hauteur), Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(2), ui.visuals().extreme_bg_color);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(1.0, FILET_TABLEAU),
        StrokeKind::Inside,
    );
    let mut interieur = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink(1.0))
            .layout(Layout::top_down(Align::Min)),
    );
    interieur.set_clip_rect(rect.intersect(ui.clip_rect()));
    egui::ScrollArea::vertical()
        .id_salt(id)
        .max_height(hauteur - 2.0)
        .auto_shrink([false, false])
        .show(&mut interieur, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (n, e) in elements.iter().enumerate() {
                let (r_ligne, reponse) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::click());
                if *selection == Some(n) {
                    ui.painter().rect_filled(
                        r_ligne,
                        CornerRadius::ZERO,
                        ui.visuals().selection.bg_fill,
                    );
                } else if reponse.hovered() {
                    ui.painter().rect_filled(
                        r_ligne,
                        CornerRadius::ZERO,
                        ui.visuals()
                            .widgets
                            .hovered
                            .weak_bg_fill
                            .gamma_multiply(0.4),
                    );
                }
                ui.painter().text(
                    r_ligne.left_center() + vec2(5.0, 0.0),
                    Align2::LEFT_CENTER,
                    e,
                    FontId::proportional(13.0),
                    ui.visuals().text_color(),
                );
                if reponse.clicked() {
                    *selection = Some(n);
                }
            }
        });
}

/// Bouton texte de pied (icône + libellé) : « Annuler » et « Valider ».
pub fn bouton_pied(ui: &mut Ui, glyphe: &str, texte: &str, largeur: f32) -> Response {
    ui.add_sized(
        vec2(largeur, 24.0),
        egui::Button::new(format!("{glyphe} {texte}")),
    )
}

/// Pictogrammes de pied.
pub const GLYPHE_ANNULER: &str = polices::CROIX;
pub const GLYPHE_VALIDER: &str = polices::COCHE;

/// Cellule à crayon ouvrant une fenêtre flottante dont `contenu` dessine les champs.
pub fn cellule_crayon_popup(
    row: &mut TableRow<'_, '_>,
    infobulle: &str,
    contenu: impl FnOnce(&mut Ui),
) -> bool {
    let mut clic = false;
    row.col(|ui| {
        filets(ui);
        ui.with_layout(
            Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| {
                let r = crayon(ui, infobulle);
                clic = r.clicked();
                Popup::from_toggle_button_response(&r)
                    .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
                    .show(contenu);
            },
        );
    });
    clic
}
