//! Panneau maître du module Films : onglet-titre, barre Recherche + loupe + Filtre ⏷, tableau
//! virtualisé (egui_extras) à filets, en-têtes d'état en icônes, ligne de totaux.
//! Les éléments génériques (onglet-titre, barre de recherche, filets, en-têtes) servent aussi
//! au module Séries.

use super::details::bandeau_onglets;
use super::{Sortie, menus};
use crate::commandes::MENU_CONTEXTUEL;
use crate::etat::{Etat, FiltreRapide, Message};
use crate::i18n::I18n;
use crate::icones::{self, EtatCellule, Icone, cellule_etat, etat_film, icone_entete};
use crate::modele::{Colonne, Film, Ordre, gigaoctets};
use crate::polices;
use crate::reglages::AffichageTableau;
use crate::theme::{BLEU, FILET_TABLEAU, ICONE_REPOS, ROUGE};
use eframe::egui::{
    self, Align, Button, Frame, Id, Label, Layout, Margin, Panel, RichText, Sense, Stroke,
    TextEdit, Vec2, vec2,
};
use egui_extras::{Column, TableBuilder};

pub const ID_CHAMP_RECHERCHE: &str = "champ_recherche";
/// Hauteur d'une ligne du tableau (densité de la référence).
pub const HAUTEUR_LIGNE: f32 = 22.0;
/// Hauteur de l'en-tête du tableau.
pub const HAUTEUR_ENTETE: f32 = 24.0;
/// Largeur des colonnes d'état.
pub const LARGEUR_ETAT: f32 = 27.0;
/// Largeur des colonnes d'état en mode texte.
pub const LARGEUR_ETAT_TEXTE: f32 = 34.0;
/// Marge intérieure des cellules.
const MARGE_CELLULE: f32 = 5.0;

pub fn afficher(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    onglet_module(ui, etat);
    Frame::new()
        .inner_margin(Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ligne_recherche(ui, etat, sortie);
            ui.add_space(6.0);
            Panel::bottom("totaux_films")
                .exact_size(24.0)
                .show_separator_line(false)
                .frame(Frame::new().inner_margin(Margin::symmetric(2, 2)))
                .show(ui, |ui| totaux(ui, etat));
            table(ui, etat, sortie);
        });
}

/// Onglet unique portant le titre du module (« Films », « Séries »…).
pub fn onglet_module(ui: &mut egui::Ui, etat: &Etat) {
    let titre = etat.i18n.t(etat.module.cle());
    bandeau_onglets(ui, "onglet_module", &[titre], 0);
}

/// Barre de recherche : champ, bouton loupe accolé, bouton « Filtre ⏷ ».
pub fn ligne_recherche(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let hauteur = 22.0;
        let largeur_filtre = 100.0;
        let largeur = (ui.available_width() - largeur_filtre - 28.0 - 10.0).max(80.0);
        let mut texte = etat.recherche.clone();
        let reponse = ui.add_sized(
            vec2(largeur, hauteur),
            TextEdit::singleline(&mut texte)
                .id(Id::unique(ID_CHAMP_RECHERCHE))
                .hint_text(etat.i18n.t("gui.search.placeholder"))
                .margin(Margin::symmetric(6, 3)),
        );
        if reponse.changed() {
            sortie.push(Message::Recherche(texte));
        }
        if etat.recherche_invalide {
            reponse.on_hover_text(etat.i18n.t("gui.search.invalid"));
        }
        if ui
            .add(
                Button::image(icones::image_teintee(Icone::Loupe, 15.0, ICONE_REPOS))
                    .min_size(vec2(28.0, hauteur)),
            )
            .clicked()
        {
            sortie.push(Message::FocusRecherche);
        }
        ui.add_space(10.0);
        let mut libelle = etat.i18n.t("movieextendedsearch.filter");
        if etat.filtre_actif() {
            libelle = format!("{} {libelle}", polices::FILTRE_ACTIF);
        }
        let bouton = Button::new(libelle)
            .right_text(RichText::new(polices::DEROULANT).size(11.0))
            .min_size(vec2(largeur_filtre, hauteur));
        egui::containers::menu::MenuButton::from_button(bouton)
            .ui(ui, |ui| menu_filtre(ui, etat, sortie));
    });
}

fn menu_filtre(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    let choix = [
        (FiltreRapide::Vus, "gui.filter.watched"),
        (FiltreRapide::NonVus, "gui.filter.unwatched"),
        (FiltreRapide::Nouveaux, "gui.filter.new"),
    ];
    for (filtre, cle) in choix {
        if ui
            .selectable_label(etat.filtre_rapide == filtre, etat.i18n.t(cle))
            .clicked()
        {
            sortie.push(Message::FiltreRapide(filtre));
            ui.close();
        }
    }
    ui.separator();
    if ui
        .button(format!("{}…", etat.i18n.t("movieextendedsearch.options")))
        .clicked()
    {
        sortie.push(Message::OuvrirFiltres(true));
        ui.close();
    }
    if ui.button(etat.i18n.t("filter.reset")).clicked() {
        sortie.push(Message::EffacerFiltres);
        ui.close();
    }
}

fn totaux(ui: &mut egui::Ui, etat: &Etat) {
    ui.horizontal(|ui| {
        let filtres = etat.visibles.len().to_string();
        let total = etat.films.len().to_string();
        ui.label(etat.i18n.t_args(
            "gui.movies.count",
            &[("filtered", filtres.as_str()), ("total", total.as_str())],
        ));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let n = etat.selection.len() as u64;
            if n > 0 {
                let taille = format!(
                    "{} {}",
                    etat.i18n.decimal(gigaoctets(etat.taille_selection()), 2),
                    etat.i18n.t("gui.unit.gigabyte")
                );
                let texte = etat.i18n.t_pluriel("movie.selected", n, &[]);
                ui.label(format!("{texte} ({taille})"));
            }
        });
    });
}

/// Filets d'une cellule (bas et droite), dans le rectangle de la cellule.
pub fn filets(ui: &egui::Ui) {
    let r = ui.max_rect();
    let trait_ = Stroke::new(1.0, FILET_TABLEAU);
    let p = ui.painter();
    p.hline(r.x_range(), r.bottom() - 0.5, trait_);
    p.vline(r.right() - 0.5, r.y_range(), trait_);
}

/// Prépare une cellule : filets puis marge gauche.
pub fn debut_cellule(ui: &mut egui::Ui) {
    filets(ui);
    ui.add_space(MARGE_CELLULE);
}

/// Texte aligné à droite dans une cellule.
pub fn droite(ui: &mut egui::Ui, texte: impl Into<egui::WidgetText>) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.add_space(MARGE_CELLULE);
        ui.add(Label::new(texte).truncate());
    });
}

/// En-tête d'icône centrée (colonnes d'état).
pub fn entete_icone(ui: &mut egui::Ui, icone: Icone) -> egui::Response {
    filets(ui);
    let taille = vec2(ui.available_width(), ui.available_height());
    let (rect, reponse) = ui.allocate_exact_size(taille, Sense::click());
    icones::image_teintee(icone, 15.0, ICONE_REPOS).paint_at(
        ui,
        egui::Rect::from_center_size(rect.center(), Vec2::splat(15.0)),
    );
    reponse
}

/// En-tête texte centré.
pub fn entete_texte(ui: &mut egui::Ui, texte: String) -> egui::Response {
    filets(ui);
    ui.with_layout(
        Layout::centered_and_justified(egui::Direction::LeftToRight),
        |ui| ui.add(Label::new(texte).truncate().sense(Sense::click())),
    )
    .inner
}

fn table(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    let colonnes = etat.colonnes_visibles();
    let aff = etat.reglages.affichage_films;
    let largeur_etat = largeur_etat(aff);
    let modifs = ui.input(|i| i.modifiers);
    let relache = ui.input(|i| !i.pointer.any_down());
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    // L'état interne du tableau (largeurs) est lié à l'ensemble ordonné des colonnes visibles.
    let ids: Vec<&str> = colonnes.iter().map(|c| c.id()).collect();
    let mut builder = TableBuilder::new(ui)
        .id_salt(("tableau_films", ids.join(",")))
        .striped(aff.lignes_rayees)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(Layout::left_to_right(Align::Center));
    for col in &colonnes {
        let c = if col.est_booleenne() {
            Column::exact(largeur_etat)
        } else if *col == Colonne::Titre {
            // Le titre absorbe l'espace libre du tableau (pas de zone vide à droite).
            Column::remainder().at_least(120.0).clip(true)
        } else {
            Column::initial(etat.largeur_colonne(*col))
                .at_least(40.0)
                .clip(true)
        };
        builder = builder.column(c);
    }
    if let Some(ligne) = etat.scroll_vers {
        builder = builder.scroll_to_row(ligne, None);
    }
    builder
        .header(HAUTEUR_ENTETE, |mut en_tete| {
            for col in &colonnes {
                en_tete.col(|ui| entete(ui, etat, *col, sortie));
            }
        })
        .body(|body| {
            // Largeurs modifiées par l'utilisateur : mémorisées au relâchement de la souris.
            if relache {
                let changees: Vec<(Colonne, f32)> = colonnes
                    .iter()
                    .zip(body.widths())
                    .filter(|(c, w)| {
                        !c.est_booleenne()
                            && **c != Colonne::Titre
                            && (etat.largeur_colonne(**c) - **w).abs() > 0.5
                    })
                    .map(|(c, w)| (*c, *w))
                    .collect();
                if !changees.is_empty() {
                    sortie.push(Message::LargeursColonnes(changees));
                }
            }
            body.rows(aff.hauteur_ligne.px(), etat.visibles.len(), |mut ligne| {
                let film = &etat.films[etat.visibles[ligne.index()]];
                let selectionne = etat.selection.contains(&film.id);
                ligne.set_selected(selectionne);
                for col in &colonnes {
                    ligne.col(|ui| cellule(ui, etat, film, *col, aff));
                }
                let reponse = ligne.response();
                if reponse.clicked() {
                    sortie.push(Message::Selectionner {
                        id: film.id,
                        ctrl: modifs.command,
                        maj: modifs.shift,
                    });
                }
                if reponse.secondary_clicked() && !selectionne {
                    sortie.push(Message::Selectionner {
                        id: film.id,
                        ctrl: false,
                        maj: false,
                    });
                }
                if reponse.double_clicked() {
                    sortie.push(Message::Commande(crate::commandes::Commande::Editer));
                }
                reponse
                    .context_menu(|ui| menus::afficher_entrees(ui, etat, MENU_CONTEXTUEL, sortie));
            });
        });
}

/// Texte d'en-tête d'une colonne texte (libellé + flèche de tri).
pub fn texte_entete(etat: &Etat, col: Colonne) -> String {
    let mut texte = etat.i18n.t(col.cle_i18n());
    if let Some((rang, ordre)) = etat.tri_de(col) {
        texte.push(' ');
        texte.push_str(if ordre == Ordre::Croissant {
            polices::TRI_CROISSANT
        } else {
            polices::TRI_DECROISSANT
        });
        if etat.tri.len() > 1 {
            texte.push_str(&format!("{}", rang + 1));
        }
    }
    texte
}

fn entete(ui: &mut egui::Ui, etat: &Etat, col: Colonne, sortie: &mut Sortie) {
    let libelle = etat.i18n.t(col.cle_i18n());
    let reponse = match icone_entete(col).filter(|_| etat.tri_de(col).is_none()) {
        Some(icone) => entete_icone(ui, icone),
        None => entete_texte(ui, texte_entete(etat, col)),
    }
    .on_hover_text(libelle);
    if reponse.clicked() {
        let additif = ui.input(|i| i.modifiers.command);
        sortie.push(Message::Trier {
            colonne: col,
            additif,
        });
    }
    reponse.context_menu(|ui| {
        ui.horizontal(|ui| {
            if ui
                .button(format!(
                    "{} {}",
                    polices::GAUCHE,
                    etat.i18n.t("column.moveleft")
                ))
                .clicked()
            {
                sortie.push(Message::DeplacerColonne(col, -1));
            }
            if ui
                .button(format!(
                    "{} {}",
                    etat.i18n.t("column.moveright"),
                    polices::DROITE
                ))
                .clicked()
            {
                sortie.push(Message::DeplacerColonne(col, 1));
            }
        });
        ui.separator();
        ui.label(RichText::new(etat.i18n.t("Button.selectvisiblecolumns")).weak());
        ui.separator();
        for c in &etat.colonnes {
            let mut visible = c.visible;
            let actif = c.colonne != Colonne::Titre;
            if ui
                .add_enabled(
                    actif,
                    egui::Checkbox::new(&mut visible, etat.i18n.t(c.colonne.cle_i18n())),
                )
                .changed()
            {
                sortie.push(Message::ColonneVisible(c.colonne, visible));
            }
        }
    });
}

/// Info-bulle d'une cellule d'état (`None` pour une cellule vide).
pub fn infobulle_etat(etat: &Etat, col: Colonne, e: EtatCellule) -> Option<String> {
    let nom = etat.i18n.t(col.cle_i18n());
    let cle = match e {
        EtatCellule::Oui => "gui.state.yes",
        EtatCellule::Non => "gui.state.no",
        EtatCellule::Attention => "gui.state.partial",
        EtatCellule::Nouveau => return Some(etat.i18n.t("gui.state.new")),
        EtatCellule::Vide => return None,
    };
    Some(etat.i18n.t_args(cle, &[("column", nom.as_str())]))
}

/// Largeur des colonnes d'état : plus large quand elles affichent du texte.
pub fn largeur_etat(aff: AffichageTableau) -> f32 {
    if aff.icones_etat {
        LARGEUR_ETAT
    } else {
        LARGEUR_ETAT_TEXTE
    }
}

/// Cellule d'état : pictogramme, ou court texte (Oui, Non...) quand les icônes sont désactivées.
pub fn cellule_etat_affichee(
    ui: &mut egui::Ui,
    i: &I18n,
    e: EtatCellule,
    icones: bool,
) -> egui::Response {
    if icones {
        return cellule_etat(ui, e);
    }
    let taille = vec2(
        ui.available_width().max(12.0),
        ui.available_height().max(12.0),
    );
    let (rect, reponse) = ui.allocate_exact_size(taille, Sense::hover());
    let (texte, couleur) = match e {
        EtatCellule::Oui => (i.t("Button.yes"), ui.visuals().text_color()),
        EtatCellule::Non => (i.t("Button.no"), ROUGE),
        EtatCellule::Attention => (i.t("gui.state.short.partial"), ui.visuals().text_color()),
        EtatCellule::Nouveau => (i.t("gui.state.short.new"), BLEU),
        EtatCellule::Vide => return reponse,
    };
    ui.painter().with_clip_rect(rect).text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        texte,
        egui::FontId::proportional(12.0),
        couleur,
    );
    reponse
}

fn cellule(ui: &mut egui::Ui, etat: &Etat, film: &Film, col: Colonne, aff: AffichageTableau) {
    if let Some(e) = etat_film(film, col) {
        filets(ui);
        let reponse = cellule_etat_affichee(ui, &etat.i18n, e, aff.icones_etat);
        if let Some(texte) = infobulle_etat(etat, col, e) {
            reponse.on_hover_text(texte);
        }
        return;
    }
    let i = &etat.i18n;
    if col.aligne_a_droite() {
        filets(ui);
        let texte = match col {
            Colonne::Annee if film.annee > 0 => RichText::new(film.annee.to_string()),
            Colonne::Annee => RichText::new(""),
            Colonne::Note => {
                let t = RichText::new(i.decimal(film.note as f64, 1));
                if film.note > 0.0 && film.note < 6.0 {
                    t.color(ROUGE)
                } else {
                    t
                }
            }
            Colonne::Votes => RichText::new(film.votes.to_string()),
            Colonne::Duree => {
                RichText::new(i.t_args("column.minutes", &[("count", &film.duree_min.to_string())]))
            }
            _ => RichText::new(format!(
                "{} {}",
                i.decimal(gigaoctets(film.taille_octets), 2),
                i.t("gui.unit.gigabyte")
            )),
        };
        droite(ui, texte);
        return;
    }
    debut_cellule(ui);
    match col {
        Colonne::Titre => {
            if film.verrouille {
                ui.label(RichText::new(polices::VERROU).color(BLEU));
            }
            ui.add(Label::new(&film.titre).truncate());
        }
        Colonne::TitreOriginal => {
            ui.add(Label::new(&film.titre_original).truncate());
        }
        Colonne::DateSortie => {
            ui.label(&film.date_sortie);
        }
        Colonne::Saga => {
            ui.add(Label::new(film.saga.as_deref().unwrap_or("")).truncate());
        }
        Colonne::Certification => {
            ui.label(&film.certification);
        }
        Colonne::DateAjout => {
            ui.label(&film.date_ajout);
        }
        Colonne::FormatVideo => {
            ui.label(film.format_video());
        }
        Colonne::CodecVideo => {
            ui.label(&film.codec_video);
        }
        Colonne::Audio => {
            ui.add(Label::new(&film.audio).truncate());
        }
        Colonne::Chemin => {
            let chemin = film.chemin();
            ui.add(Label::new(&chemin).truncate())
                .on_hover_text(&chemin);
        }
        Colonne::NomFichier => {
            ui.add(Label::new(&film.nom_fichier).truncate());
        }
        _ => {}
    }
}
