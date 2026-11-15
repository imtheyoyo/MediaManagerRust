//! Module Séries : arbre série > saison > épisode présenté en tableau (triangles d'expansion,
//! colonnes Année / S / E / Vu), totaux sur deux lignes, panneau de détails du nœud sélectionné.

use super::Sortie;
use super::details::{
    bandeau_onglets, colonne_images, largeur_libelles, lien_chemin, ligne, titres,
};
use super::tableau::{
    HAUTEUR_ENTETE, HAUTEUR_LIGNE, LARGEUR_ETAT, droite, entete_icone, entete_texte, filets,
    ligne_recherche, onglet_module,
};
use crate::etat::{Etat, Message, NoeudSerie, OngletDetails};
use crate::icones::{EtatCellule, Icone, cellule_etat, peindre_triangle};
use crate::images::GestionnaireImages;
use crate::source_donnees::{EpisodeAffichage, SaisonAffichage, SerieAffichage};
use eframe::egui::{
    self, Align, Frame, Label, Layout, Margin, Panel, RichText, ScrollArea, Sense, Vec2, vec2,
};
use egui_extras::{Column, TableBuilder};
use std::path::{Path, PathBuf};

/// Une ligne affichée de l'arbre des séries.
#[derive(Clone, Debug, PartialEq)]
pub struct LigneSerie {
    pub noeud: NoeudSerie,
    pub profondeur: u8,
    pub titre: String,
    pub annee: u32,
    pub saisons: Option<usize>,
    pub episodes: usize,
    pub vu: EtatCellule,
    /// `Some(déployé)` si le nœud a des enfants.
    pub deploiement: Option<bool>,
    /// Épisode connu du fournisseur mais sans fichier local (affiché atténué).
    pub manquant: bool,
}

fn titre_saison(etat: &Etat, s: &SaisonAffichage) -> String {
    let base = match s.numero {
        0 => etat.i18n.t("gui.tvshows.specials"),
        n if n < 0 => etat.i18n.t("gui.tvshows.unknown"),
        n => etat
            .i18n
            .t_args("gui.tvshows.season", &[("number", n.to_string().as_str())]),
    };
    if s.titre.is_empty() {
        base
    } else {
        format!("{base} - {}", s.titre)
    }
}

/// État « vu » des épisodes présents (les manquants ne comptent pas).
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

fn episodes_presents(s: &SaisonAffichage) -> usize {
    s.episodes.iter().filter(|e| !e.manquant).count()
}

/// Séries retenues par la recherche (texte contenu dans le titre, sans casse).
pub fn series_filtrees(etat: &Etat) -> Vec<&SerieAffichage> {
    let motif = etat.recherche.trim().to_lowercase();
    etat.series
        .iter()
        .filter(|s| motif.is_empty() || s.titre.to_lowercase().contains(&motif))
        .collect()
}

/// Lignes visibles de l'arbre (séries filtrées, saisons et épisodes des nœuds déployés).
pub fn lignes_series(etat: &Etat) -> Vec<LigneSerie> {
    let mut lignes = Vec::new();
    for s in series_filtrees(etat) {
        let deployee = etat.series_deployees.contains(&(s.id.clone(), None));
        let episodes = s.saisons.iter().flat_map(|x| x.episodes.iter());
        lignes.push(LigneSerie {
            noeud: NoeudSerie::Serie(s.id.clone()),
            profondeur: 0,
            titre: s.titre.clone(),
            annee: s.annee,
            saisons: Some(s.saisons.len()),
            episodes: s.nombre_episodes(),
            vu: etat_vu(episodes),
            deploiement: (!s.saisons.is_empty()).then_some(deployee),
            manquant: false,
        });
        if !deployee {
            continue;
        }
        for saison in &s.saisons {
            let ouverte = etat
                .series_deployees
                .contains(&(s.id.clone(), Some(saison.numero)));
            lignes.push(LigneSerie {
                noeud: NoeudSerie::Saison(s.id.clone(), saison.numero),
                profondeur: 1,
                titre: titre_saison(etat, saison),
                annee: 0,
                saisons: None,
                episodes: episodes_presents(saison),
                vu: etat_vu(saison.episodes.iter()),
                deploiement: (!saison.episodes.is_empty()).then_some(ouverte),
                manquant: false,
            });
            if !ouverte {
                continue;
            }
            for (rang, e) in saison.episodes.iter().enumerate() {
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
                    format!(
                        "{numero} - {titre} ({})",
                        etat.i18n.t("gui.tvshows.missing")
                    )
                } else {
                    format!("{numero} - {titre}")
                };
                lignes.push(LigneSerie {
                    noeud: NoeudSerie::Episode(s.id.clone(), saison.numero, rang),
                    profondeur: 2,
                    titre,
                    annee: 0,
                    saisons: None,
                    episodes: 0,
                    vu: if e.manquant {
                        EtatCellule::Vide
                    } else if e.vu {
                        EtatCellule::Oui
                    } else {
                        EtatCellule::Non
                    },
                    deploiement: None,
                    manquant: e.manquant,
                });
            }
        }
    }
    lignes
}

/// Panneau maître du module Séries.
pub fn afficher(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    onglet_module(ui, etat);
    Frame::new()
        .inner_margin(Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ligne_recherche(ui, etat, sortie);
            ui.add_space(6.0);
            Panel::bottom("totaux_series")
                .exact_size(44.0)
                .show_separator_line(false)
                .frame(Frame::new().inner_margin(Margin::symmetric(2, 2)))
                .show(ui, |ui| totaux(ui, etat, sortie));
            if etat.series.is_empty() {
                ui.add_space(12.0);
                ui.label(etat.i18n.t("gui.tvshows.empty"));
                return;
            }
            table(ui, etat, sortie);
        });
}

fn totaux(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let filtrees = series_filtrees(etat);
    let ep_filtres: usize = filtrees.iter().map(|s| s.nombre_episodes()).sum();
    let ep_total: usize = etat.series.iter().map(|s| s.nombre_episodes()).sum();
    let noeud = etat.noeud_serie.as_ref();
    let ep_selection = noeud.map(|n| episodes_du_noeud(etat, n)).unwrap_or(0);
    ui.spacing_mut().item_spacing.y = 2.0;
    let lignes = [
        (
            i.t_args(
                "gui.tvshows.total",
                &[
                    ("filtered", filtrees.len().to_string().as_str()),
                    ("total", etat.series.len().to_string().as_str()),
                ],
            ),
            noeud
                .map(|_| i.t_pluriel("gui.tvshows.selected", 1, &[]))
                .unwrap_or_default(),
        ),
        (
            i.t_args(
                "gui.tvshows.episodestotal",
                &[
                    ("filtered", ep_filtres.to_string().as_str()),
                    ("total", ep_total.to_string().as_str()),
                ],
            ),
            noeud
                .map(|_| i.t_pluriel("gui.tvshows.episodesselected", ep_selection as u64, &[]))
                .unwrap_or_default(),
        ),
    ];
    for (k, (gauche, a_droite)) in lignes.into_iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(gauche);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if k == 0
                    && ui
                        .small_button(format!(
                            "{}  {}",
                            crate::polices::RECHARGER,
                            i.t("gui.tvshows.update")
                        ))
                        .clicked()
                {
                    sortie.push(Message::MettreAJourSeries);
                }
                ui.label(a_droite);
            });
        });
    }
}

fn episodes_du_noeud(etat: &Etat, noeud: &NoeudSerie) -> usize {
    let Some(s) = etat.series.iter().find(|s| s.id == noeud.id_serie()) else {
        return 0;
    };
    match noeud {
        NoeudSerie::Serie(_) => s.nombre_episodes(),
        NoeudSerie::Saison(_, n) => s
            .saisons
            .iter()
            .find(|x| x.numero == *n)
            .map(episodes_presents)
            .unwrap_or(0),
        NoeudSerie::Episode(..) => 1,
    }
}

fn table(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    let lignes = lignes_series(etat);
    let i = &etat.i18n;
    ui.spacing_mut().item_spacing = Vec2::ZERO;
    TableBuilder::new(ui)
        .id_salt("tableau_series")
        .striped(false)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::remainder().at_least(160.0).clip(true))
        .column(Column::exact(52.0))
        .column(Column::exact(32.0))
        .column(Column::exact(44.0))
        .column(Column::exact(LARGEUR_ETAT))
        .header(HAUTEUR_ENTETE, |mut h| {
            h.col(|ui| {
                entete_texte(ui, i.t("metatag.title"));
            });
            h.col(|ui| {
                entete_texte(ui, i.t("metatag.year"));
            });
            h.col(|ui| {
                entete_texte(ui, "S".into()).on_hover_text(i.t("gui.tvshows.seasons"));
            });
            h.col(|ui| {
                entete_texte(ui, "E".into()).on_hover_text(i.t("gui.tvshows.episodecount"));
            });
            h.col(|ui| {
                entete_icone(ui, Icone::Vu).on_hover_text(i.t("metatag.watched"));
            });
        })
        .body(|body| {
            body.rows(HAUTEUR_LIGNE, lignes.len(), |mut rang| {
                let l = &lignes[rang.index()];
                rang.set_selected(etat.noeud_serie.as_ref() == Some(&l.noeud));
                rang.col(|ui| {
                    filets(ui);
                    ui.add_space(4.0 + 16.0 * l.profondeur as f32);
                    let (rect, rep) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::click());
                    if let Some(ouvert) = l.deploiement {
                        let couleur = ui.visuals().text_color();
                        peindre_triangle(ui.painter(), rect.center(), 8.0, ouvert, couleur);
                        if rep.clicked() {
                            sortie.push(basculer(&l.noeud));
                        }
                    }
                    ui.add_space(3.0);
                    let texte = if l.profondeur == 0 {
                        RichText::new(&l.titre)
                    } else if l.manquant {
                        RichText::new(&l.titre).size(12.5).weak().italics()
                    } else {
                        RichText::new(&l.titre).size(12.5)
                    };
                    ui.add(Label::new(texte).truncate());
                });
                rang.col(|ui| {
                    filets(ui);
                    if l.annee > 0 {
                        droite(ui, l.annee.to_string());
                    }
                });
                rang.col(|ui| {
                    filets(ui);
                    if let Some(n) = l.saisons {
                        droite(ui, n.to_string());
                    }
                });
                rang.col(|ui| {
                    filets(ui);
                    if l.episodes > 0 {
                        droite(ui, l.episodes.to_string());
                    }
                });
                rang.col(|ui| {
                    filets(ui);
                    cellule_etat(ui, l.vu);
                });
                let reponse = rang.response();
                if reponse.clicked() {
                    sortie.push(Message::SelectionnerNoeudSerie(l.noeud.clone()));
                }
                if reponse.double_clicked() && l.deploiement.is_some() {
                    sortie.push(basculer(&l.noeud));
                }
            });
        });
}

fn basculer(noeud: &NoeudSerie) -> Message {
    match noeud {
        NoeudSerie::Serie(id) => Message::BasculerNoeudSerie(id.clone(), None),
        NoeudSerie::Saison(id, n) | NoeudSerie::Episode(id, n, _) => {
            Message::BasculerNoeudSerie(id.clone(), Some(*n))
        }
    }
}

/// Première image existante parmi des noms usuels du dossier de la série.
fn image_dossier(dossier: &str, noms: &[&str]) -> Option<PathBuf> {
    if dossier.is_empty() {
        return None;
    }
    noms.iter()
        .map(|n| Path::new(dossier).join(n))
        .find(|p| p.is_file())
}

/// Panneau de détails du module Séries (nœud sélectionné).
pub fn afficher_details(
    ui: &mut egui::Ui,
    etat: &Etat,
    images: &mut GestionnaireImages,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    let onglets = [OngletDetails::Details, OngletDetails::FichiersMedias];
    let libelles: Vec<String> = onglets.iter().map(|o| i.t(o.cle())).collect();
    let actif = usize::from(etat.onglet == OngletDetails::FichiersMedias);
    if let Some(k) = bandeau_onglets(ui, "onglets_details_series", &libelles, actif) {
        sortie.push(Message::ChoisirOnglet(onglets[k]));
    }
    let serie = etat
        .noeud_serie
        .as_ref()
        .and_then(|n| etat.series.iter().find(|s| s.id == n.id_serie()));
    Frame::new()
        .inner_margin(Margin {
            left: 10,
            right: 10,
            top: 8,
            bottom: 6,
        })
        .show(ui, |ui| {
            let (Some(serie), Some(noeud)) = (serie, etat.noeud_serie.as_ref()) else {
                ui.add_space(12.0);
                ui.label(RichText::new(i.t("app.nothingselected")).weak());
                return;
            };
            ScrollArea::vertical()
                .id_salt("defilement_details_series")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if actif == 1 {
                        fichiers(ui, etat, serie, noeud, sortie);
                        return;
                    }
                    ui.horizontal_top(|ui| {
                        let d = &serie.chemin;
                        let chemins = [
                            image_dossier(d, &["poster.jpg", "poster.png", "folder.jpg"]),
                            image_dossier(d, &["fanart.jpg", "fanart.png", "backdrop.jpg"]),
                            image_dossier(d, &["banner.jpg", "banner.png"]),
                        ];
                        colonne_images(ui, etat, images, chemins, sortie);
                        ui.add_space(14.0);
                        ui.vertical(|ui| infos(ui, etat, serie, noeud, sortie));
                    });
                });
        });
}

fn infos(
    ui: &mut egui::Ui,
    etat: &Etat,
    serie: &SerieAffichage,
    noeud: &NoeudSerie,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    let saison = |n: i32| serie.saisons.iter().find(|s| s.numero == n);
    let libelles = vec![
        i.t("metatag.year"),
        i.t("gui.tvshows.seasons"),
        i.t("gui.tvshows.episodecount"),
        i.t("metatag.watched"),
        i.t("metatag.path"),
        i.t("metatag.filename"),
        i.t("gui.series.status"),
        i.t("metatag.rating"),
        i.t("metatag.genre"),
        i.t("gui.tvshows.missingcount"),
        i.t("gui.tvshows.mediainfo"),
    ];
    let l = largeur_libelles(ui, &libelles);
    match noeud {
        NoeudSerie::Serie(_) => {
            titres(ui, &serie.titre, &serie.titre_original);
            egui::Grid::new("grille_serie")
                .num_columns(2)
                .min_col_width(l)
                .spacing([14.0, 3.0])
                .show(ui, |ui| {
                    let annee = if serie.annee > 0 {
                        serie.annee.to_string()
                    } else {
                        String::new()
                    };
                    ligne(ui, &libelles[0], &annee);
                    ligne(ui, &libelles[6], &i.t(serie.statut.cle()));
                    if serie.note > 0.0 {
                        ligne(ui, &libelles[7], &i.decimal(f64::from(serie.note), 1));
                    }
                    if !serie.genres.is_empty() {
                        ligne(ui, &libelles[8], &serie.genres.join(", "));
                    }
                    ligne(ui, &libelles[1], &serie.saisons.len().to_string());
                    ligne(ui, &libelles[2], &serie.nombre_episodes().to_string());
                    if serie.nombre_manquants() > 0 {
                        ligne(ui, &libelles[9], &serie.nombre_manquants().to_string());
                    }
                    ui.label(RichText::new(&libelles[4]).strong());
                    lien_chemin(ui, &serie.chemin, sortie);
                    ui.end_row();
                });
            if !serie.resume.is_empty() {
                ui.add_space(6.0);
                ui.add(Label::new(&serie.resume).wrap());
            }
        }
        NoeudSerie::Saison(_, n) => {
            let Some(s) = saison(*n) else { return };
            titres(ui, &titre_saison(etat, s), &serie.titre);
            egui::Grid::new("grille_saison")
                .num_columns(2)
                .min_col_width(l)
                .spacing([14.0, 3.0])
                .show(ui, |ui| {
                    ligne(ui, &libelles[2], &episodes_presents(s).to_string());
                });
        }
        NoeudSerie::Episode(_, n, rang) => {
            let Some(e) = saison(*n).and_then(|s| s.episodes.get(*rang)) else {
                return;
            };
            titres(ui, &e.titre, &serie.titre);
            egui::Grid::new("grille_episode")
                .num_columns(2)
                .min_col_width(l)
                .spacing([14.0, 3.0])
                .show(ui, |ui| {
                    if e.manquant {
                        ui.label(RichText::new(i.t("gui.tvshows.missingepisode")).weak());
                        ui.end_row();
                        return;
                    }
                    let vu = if e.vu {
                        i.t("Button.yes")
                    } else {
                        i.t("Button.no")
                    };
                    ligne(ui, &libelles[3], &vu);
                    ligne(ui, &libelles[5], &e.fichier);
                    if !e.infos_media.is_empty() {
                        ligne(ui, &libelles[10], &e.infos_media);
                    }
                });
        }
    }
}

/// Onglet « Fichiers médias » : fichiers des épisodes présents avec leur résumé technique
/// (rempli par la tâche « Analyser les fichiers médias »).
fn fichiers(
    ui: &mut egui::Ui,
    etat: &Etat,
    serie: &SerieAffichage,
    noeud: &NoeudSerie,
    sortie: &mut Sortie,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(etat.i18n.t("metatag.path")).strong());
        lien_chemin(ui, &serie.chemin, sortie);
    });
    ui.separator();
    for s in &serie.saisons {
        if let NoeudSerie::Saison(_, n) | NoeudSerie::Episode(_, n, _) = noeud
            && *n != s.numero
        {
            continue;
        }
        for e in s.episodes.iter().filter(|e| !e.manquant) {
            ui.horizontal(|ui| {
                ui.add_sized(
                    vec2(18.0, 18.0),
                    Label::new(if e.vu { crate::polices::COCHE } else { "" }),
                );
                ui.label(&e.fichier);
                if !e.infos_media.is_empty() {
                    ui.label(RichText::new(&e.infos_media).weak());
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_donnees::EpisodeAffichage;

    fn etat_demo() -> Etat {
        let mut etat = Etat::nouveau();
        let ep = |n: i32, vu: bool| EpisodeAffichage {
            episode: n,
            titre: format!("Épisode {n}"),
            fichier: format!("e{n}.mkv"),
            vu,
            ..EpisodeAffichage::default()
        };
        etat.series = vec![
            SerieAffichage {
                id: "a".into(),
                titre: "Alpha".into(),
                annee: 2001,
                chemin: "/series/Alpha".into(),
                saisons: vec![SaisonAffichage {
                    numero: 1,
                    titre: String::new(),
                    episodes: vec![ep(1, true), ep(2, false)],
                }],
                ..SerieAffichage::default()
            },
            SerieAffichage {
                id: "b".into(),
                titre: "Bêta".into(),
                annee: 0,
                chemin: String::new(),
                saisons: vec![SaisonAffichage {
                    numero: 1,
                    titre: String::new(),
                    episodes: vec![ep(1, true)],
                }],
                ..SerieAffichage::default()
            },
        ];
        etat
    }

    #[test]
    fn arbre_deploye_et_replie() {
        let mut etat = etat_demo();
        let l = lignes_series(&etat);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].deploiement, Some(false));
        assert_eq!(l[0].vu, EtatCellule::Non);
        assert_eq!(l[1].vu, EtatCellule::Oui);
        assert_eq!((l[0].saisons, l[0].episodes), (Some(1), 2));
        etat.appliquer(Message::BasculerNoeudSerie("a".into(), None));
        etat.appliquer(Message::BasculerNoeudSerie("a".into(), Some(1)));
        let l = lignes_series(&etat);
        assert_eq!(l.len(), 2 + 1 + 2);
        assert_eq!(l[1].noeud, NoeudSerie::Saison("a".into(), 1));
        assert_eq!(l[2].noeud, NoeudSerie::Episode("a".into(), 1, 0));
        assert_eq!(l[2].profondeur, 2);
        assert!(l[2].titre.starts_with("01 - "));
        // Replier la série masque saisons et épisodes.
        etat.appliquer(Message::BasculerNoeudSerie("a".into(), None));
        assert_eq!(lignes_series(&etat).len(), 2);
    }

    #[test]
    fn recherche_et_selection() {
        let mut etat = etat_demo();
        etat.recherche = "bêt".into();
        let l = lignes_series(&etat);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].titre, "Bêta");
        etat.appliquer(Message::SelectionnerNoeudSerie(NoeudSerie::Saison(
            "a".into(),
            1,
        )));
        assert_eq!(
            episodes_du_noeud(&etat, etat.noeud_serie.as_ref().unwrap()),
            2
        );
        assert!(matches!(
            basculer(&NoeudSerie::Episode("a".into(), 1, 0)),
            Message::BasculerNoeudSerie(_, Some(1))
        ));
    }
}
