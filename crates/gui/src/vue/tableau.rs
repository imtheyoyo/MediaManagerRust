//! Panneau maître : recherche + filtre, tableau virtualisé (egui_extras), ligne de totaux.

use super::{Sortie, menus};
use crate::commandes::MENU_CONTEXTUEL;
use crate::etat::{Etat, FiltreRapide, Message};
use crate::modele::{Colonne, Film, Ordre, gigaoctets};
use crate::polices;
use crate::theme::{BLEU, ROUGE, VERT};
use eframe::egui::{
    self, Align, Frame, Id, Label, Layout, Margin, Panel, RichText, Sense, TextEdit,
};
use egui_extras::{Column, TableBuilder};

pub const ID_CHAMP_RECHERCHE: &str = "champ_recherche";
const HAUTEUR_LIGNE: f32 = 20.0;

pub fn afficher(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    ligne_recherche(ui, etat, sortie);
    ui.add_space(4.0);
    Panel::bottom("totaux_films")
        .exact_size(22.0)
        .frame(Frame::new().inner_margin(Margin::symmetric(4, 2)))
        .show(ui, |ui| totaux(ui, etat));
    table(ui, etat, sortie);
}

fn ligne_recherche(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    ui.horizontal(|ui| {
        let mut texte = etat.recherche.clone();
        let largeur = (ui.available_width() - 110.0).max(80.0);
        let reponse = ui.add(
            TextEdit::singleline(&mut texte)
                .id(Id::unique(ID_CHAMP_RECHERCHE))
                .hint_text(format!(
                    "{} {}",
                    polices::RECHERCHE,
                    etat.i18n.t("gui.search.placeholder")
                ))
                .desired_width(largeur),
        );
        if reponse.changed() {
            sortie.push(Message::Recherche(texte));
        }
        if etat.recherche_invalide {
            reponse.on_hover_text(etat.i18n.t("gui.search.invalid"));
        }
        let libelle = if etat.filtre_actif() {
            format!(
                "{} {} {}",
                polices::FILTRE_ACTIF,
                etat.i18n.t("movieextendedsearch.filter"),
                polices::DEROULANT
            )
        } else {
            format!(
                "{} {}",
                etat.i18n.t("movieextendedsearch.filter"),
                polices::DEROULANT
            )
        };
        ui.menu_button(libelle, |ui| {
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
        });
    });
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

fn table(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    let colonnes = etat.colonnes_visibles();
    let modifs = ui.input(|i| i.modifiers);
    let relache = ui.input(|i| !i.pointer.any_down());
    // L'état interne du tableau (largeurs) est lié à l'ensemble ordonné des colonnes visibles.
    let ids: Vec<&str> = colonnes.iter().map(|c| c.id()).collect();
    let mut builder = TableBuilder::new(ui)
        .id_salt(("tableau_films", ids.join(",")))
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(Layout::left_to_right(Align::Center));
    for col in &colonnes {
        let c = if col.est_booleenne() {
            Column::exact(30.0)
        } else if *col == Colonne::Titre {
            Column::initial(etat.largeur_colonne(*col))
                .at_least(120.0)
                .clip(true)
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
        .header(22.0, |mut en_tete| {
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
                        !c.est_booleenne() && (etat.largeur_colonne(**c) - **w).abs() > 0.5
                    })
                    .map(|(c, w)| (*c, *w))
                    .collect();
                if !changees.is_empty() {
                    sortie.push(Message::LargeursColonnes(changees));
                }
            }
            body.rows(HAUTEUR_LIGNE, etat.visibles.len(), |mut ligne| {
                let film = &etat.films[etat.visibles[ligne.index()]];
                let selectionne = etat.selection.contains(&film.id);
                ligne.set_selected(selectionne);
                for col in &colonnes {
                    ligne.col(|ui| cellule(ui, etat, film, *col));
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

fn entete(ui: &mut egui::Ui, etat: &Etat, col: Colonne, sortie: &mut Sortie) {
    let libelle = etat.i18n.t(col.cle_i18n());
    let mut texte = col
        .entete_court()
        .map(str::to_string)
        .unwrap_or_else(|| libelle.clone());
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
    let reponse = ui
        .add(Label::new(RichText::new(texte).strong()).sense(Sense::click()))
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

fn coche(ui: &mut egui::Ui, valeur: bool) {
    if valeur {
        ui.label(RichText::new(polices::COCHE).color(VERT));
    }
}

fn cellule(ui: &mut egui::Ui, etat: &Etat, film: &Film, col: Colonne) {
    let i = &etat.i18n;
    match col {
        Colonne::Titre => {
            if film.nouveau {
                ui.label(RichText::new(polices::PASTILLE).color(VERT));
            }
            if film.verrouille {
                ui.label(RichText::new(polices::VERROU).color(BLEU));
            }
            ui.add(Label::new(&film.titre).truncate());
        }
        Colonne::TitreOriginal => {
            ui.add(Label::new(&film.titre_original).truncate());
        }
        Colonne::Annee => droite(ui, film.annee.to_string()),
        Colonne::DateSortie => {
            ui.label(&film.date_sortie);
        }
        Colonne::Saga => {
            ui.label(film.saga.as_deref().unwrap_or(""));
        }
        Colonne::Note => {
            let texte = RichText::new(i.decimal(film.note as f64, 1));
            let texte = if film.note < 6.0 {
                texte.color(ROUGE)
            } else {
                texte
            };
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(texte));
        }
        Colonne::Votes => droite(ui, film.votes.to_string()),
        Colonne::Certification => {
            ui.label(&film.certification);
        }
        Colonne::DateAjout => {
            ui.label(&film.date_ajout);
        }
        Colonne::Duree => droite(
            ui,
            i.t_args("column.minutes", &[("count", &film.duree_min.to_string())]),
        ),
        Colonne::FormatVideo => {
            ui.label(film.format_video());
        }
        Colonne::CodecVideo => {
            ui.label(&film.codec_video);
        }
        Colonne::Audio => {
            ui.add(Label::new(&film.audio).truncate());
        }
        Colonne::TailleFichier => droite(
            ui,
            format!(
                "{} {}",
                i.decimal(gigaoctets(film.taille_octets), 2),
                i.t("gui.unit.gigabyte")
            ),
        ),
        Colonne::Chemin => {
            ui.add(Label::new(film.chemin()).truncate());
        }
        Colonne::NomFichier => {
            ui.add(Label::new(&film.nom_fichier).truncate());
        }
        Colonne::Nouveau => {
            if film.nouveau {
                ui.label(RichText::new(polices::PASTILLE).color(VERT));
            }
        }
        Colonne::Metadonnees => coche(ui, film.metadonnees_ok),
        Colonne::Images => coche(ui, film.images_ok),
        Colonne::BandeAnnonce => coche(ui, film.bande_annonce),
        Colonne::SousTitres => coche(ui, film.sous_titres),
        Colonne::Vu => coche(ui, film.vu),
    }
}

fn droite(ui: &mut egui::Ui, texte: String) {
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(texte));
}
