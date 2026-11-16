//! Onglet « Épisodes » de la série : arbre saisons / épisodes en lecture (titre, fichier, média)
//! avec la bascule « vu » par saison et par épisode (appliquée tout de suite, comme dans le
//! tableau des séries).

use super::Contexte;
use crate::dialogues::ActionEditeurSerie;
use crate::etat::NoeudSerie;
use crate::icones::{EtatCellule, peindre_etat, peindre_triangle};
use crate::source_donnees::{EpisodeAffichage, SaisonAffichage};
use crate::vue::tableau::{HAUTEUR_ENTETE, HAUTEUR_LIGNE, entete_texte, filets};
use eframe::egui::{Align, Id, Label, Layout, RichText, Sense, Ui, vec2};
use egui_extras::{Column, TableBuilder};

/// Une ligne de l'arbre : saison (déployable) ou épisode.
enum Ligne<'a> {
    Saison(&'a SaisonAffichage, bool),
    Episode(&'a SaisonAffichage, usize, &'a EpisodeAffichage),
}

fn etat_vu<'a>(episodes: impl Iterator<Item = &'a EpisodeAffichage>) -> EtatCellule {
    let mut presents = episodes.filter(|e| !e.manquant).peekable();
    if presents.peek().is_none() {
        EtatCellule::Vide
    } else if presents.all(|e| e.vu) {
        EtatCellule::Oui
    } else {
        EtatCellule::Non
    }
}

fn titre_saison(ct: &Contexte<'_>, s: &SaisonAffichage) -> String {
    let i = ct.i;
    let base = match s.numero {
        0 => i.t("gui.tvshows.specials"),
        n if n < 0 => i.t("gui.tvshows.unknown"),
        n => i.t_args("gui.tvshows.season", &[("number", n.to_string().as_str())]),
    };
    if s.titre.is_empty() {
        base
    } else {
        format!("{base} - {}", s.titre)
    }
}

pub fn afficher(ui: &mut Ui, ct: &Contexte<'_>, pied: &mut Vec<ActionEditeurSerie>) {
    let i = ct.i;
    let largeur = ui.available_width() - 2.0;
    let hauteur = (ui.available_height() - 2.0).max(120.0);
    let Some(serie) = ct.serie() else {
        ui.label(RichText::new(i.t("app.nothingselected")).weak());
        return;
    };
    // Lignes visibles : saisons, et épisodes des saisons déployées (déployées par défaut).
    let ouverte = |ui: &Ui, n: i32| {
        ui.data(|d| d.get_temp::<bool>(Id::unique(("eds_saison_ouverte", &serie.id, n))))
            .unwrap_or(true)
    };
    let mut lignes: Vec<Ligne<'_>> = Vec::new();
    for s in &serie.saisons {
        let o = ouverte(ui, s.numero);
        lignes.push(Ligne::Saison(s, o));
        if o {
            for (rang, e) in s.episodes.iter().enumerate() {
                lignes.push(Ligne::Episode(s, rang, e));
            }
        }
    }
    let mut bascule_ouverture: Option<(i32, bool)> = None;
    ui.allocate_ui_with_layout(vec2(largeur, hauteur), Layout::top_down(Align::Min), |ui| {
        ui.set_min_size(vec2(largeur, hauteur));
        ui.set_max_size(vec2(largeur, hauteur));
        if serie.saisons.is_empty() {
            ui.label(RichText::new(i.t("serieeditor.noepisodes")).weak());
            return;
        }
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        TableBuilder::new(ui)
            .id_salt("eds_episodes")
            .striped(true)
            .resizable(false)
            .sense(Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            .min_scrolled_height(hauteur - HAUTEUR_ENTETE)
            .max_scroll_height(hauteur - HAUTEUR_ENTETE)
            .auto_shrink([false, false])
            .column(Column::remainder().at_least(160.0).clip(true))
            .column(Column::exact((largeur * 0.26).clamp(120.0, 340.0)))
            .column(Column::exact((largeur * 0.26).clamp(120.0, 340.0)))
            .column(Column::exact(44.0))
            .header(HAUTEUR_ENTETE, |mut h| {
                h.col(|ui| {
                    entete_texte(ui, i.t("metatag.title"));
                });
                h.col(|ui| {
                    entete_texte(ui, i.t("metatag.filename"));
                });
                h.col(|ui| {
                    entete_texte(ui, i.t("gui.tvshows.mediainfo"));
                });
                h.col(|ui| {
                    entete_texte(ui, i.t("metatag.watched"));
                });
            })
            .body(|body| {
                body.rows(HAUTEUR_LIGNE, lignes.len(), |mut row| {
                    match &lignes[row.index()] {
                        Ligne::Saison(s, ouverte) => {
                            let vu = etat_vu(s.episodes.iter());
                            row.col(|ui| {
                                filets(ui);
                                ui.add_space(4.0);
                                let (rect, rep) =
                                    ui.allocate_exact_size(vec2(14.0, 14.0), Sense::click());
                                peindre_triangle(
                                    ui.painter(),
                                    rect.center(),
                                    8.0,
                                    *ouverte,
                                    ui.visuals().text_color(),
                                );
                                if rep.clicked() {
                                    bascule_ouverture = Some((s.numero, !*ouverte));
                                }
                                ui.add_space(3.0);
                                ui.add(
                                    Label::new(RichText::new(titre_saison(ct, s)).strong())
                                        .truncate(),
                                );
                            });
                            row.col(|ui| filets(ui));
                            row.col(|ui| filets(ui));
                            cellule_vu(&mut row, vu, || {
                                pied.push(ActionEditeurSerie::BasculerVu(NoeudSerie::Saison(
                                    ct.e.id.clone(),
                                    s.numero,
                                )));
                            });
                        }
                        Ligne::Episode(s, rang, e) => {
                            let numero = if e.episode >= 0 {
                                format!("{:02}", e.episode)
                            } else {
                                "?".into()
                            };
                            let titre = if e.titre.is_empty() {
                                e.fichier.clone()
                            } else {
                                e.titre.clone()
                            };
                            let titre = if e.manquant {
                                format!("{numero} - {titre} ({})", i.t("gui.tvshows.missing"))
                            } else {
                                format!("{numero} - {titre}")
                            };
                            row.col(|ui| {
                                filets(ui);
                                ui.add_space(4.0 + 16.0 + 17.0);
                                let t = RichText::new(titre).size(12.5);
                                let t = if e.manquant { t.weak().italics() } else { t };
                                ui.add(Label::new(t).truncate());
                            });
                            row.col(|ui| {
                                filets(ui);
                                ui.add_space(5.0);
                                ui.add(Label::new(&e.fichier).truncate());
                            });
                            row.col(|ui| {
                                filets(ui);
                                ui.add_space(5.0);
                                ui.add(Label::new(&e.infos_media).truncate());
                            });
                            let vu = if e.manquant {
                                EtatCellule::Vide
                            } else if e.vu {
                                EtatCellule::Oui
                            } else {
                                EtatCellule::Non
                            };
                            let actif = !e.manquant;
                            cellule_vu(&mut row, vu, || {
                                if actif {
                                    pied.push(ActionEditeurSerie::BasculerVu(NoeudSerie::Episode(
                                        ct.e.id.clone(),
                                        s.numero,
                                        *rang,
                                    )));
                                }
                            });
                        }
                    }
                });
            });
    });
    if let Some((n, o)) = bascule_ouverture {
        ui.data_mut(|d| d.insert_temp(Id::unique(("eds_saison_ouverte", &serie.id, n)), o));
    }
}

/// Cellule « vu » cliquable : pictogramme d'état centré ; `agir` est appelé au clic.
fn cellule_vu(row: &mut egui_extras::TableRow<'_, '_>, vu: EtatCellule, agir: impl FnOnce()) {
    let mut clic = false;
    row.col(|ui| {
        filets(ui);
        let taille = vec2(
            ui.available_width().max(12.0),
            ui.available_height().max(12.0),
        );
        let (rect, rep) = ui.allocate_exact_size(taille, Sense::click());
        peindre_etat(ui.painter(), rect.center(), vu);
        clic = rep.clicked() && vu != EtatCellule::Vide;
    });
    if clic {
        agir();
    }
}
