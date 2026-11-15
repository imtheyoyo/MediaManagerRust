//! Panneau de détails à onglets (Détails, Distribution, Fichiers médias, Illustration, Bande annonce)
//! et éléments partagés avec le module Séries (bandeau d'onglets, illustrations légendées,
//! grilles libellé / valeur, liens).

use super::{Sortie, image_ou_emplacement};
use crate::etat::{Etat, Message, OngletDetails};
use crate::images::GestionnaireImages;
use crate::modele::{Film, gigaoctets};
use crate::polices;
use crate::source_donnees::GenreImage;
use crate::theme::{FOND_BARRES, ONGLET_ACTIF, fond_alterne};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Id, Label, Layout, Margin, Rect, RichText,
    ScrollArea, Sense, Shape, Stroke, Vec2, pos2, vec2,
};
use egui_extras::{Column, TableBuilder};
use std::path::{Path, PathBuf};

/// Hauteur de la partie « onglets » du bandeau.
pub const HAUTEUR_ONGLETS: f32 = 30.0;
/// Bande sous les onglets (couleur du contenu) où se dessine l'encoche de l'onglet actif.
const HAUTEUR_ENCOCHE: f32 = 7.0;
const TEXTE_TITRE: Color32 = Color32::from_rgb(0xE6, 0xE6, 0xE6);

/// Bandeau d'onglets sombre : onglet actif gris avec encoche dessous. Renvoie l'onglet cliqué.
pub fn bandeau_onglets(
    ui: &mut egui::Ui,
    sel: &str,
    libelles: &[String],
    actif: usize,
) -> Option<usize> {
    let largeur = ui.available_width();
    let (zone, _) = ui.allocate_exact_size(
        vec2(largeur, HAUTEUR_ONGLETS + HAUTEUR_ENCOCHE),
        Sense::hover(),
    );
    let bandeau = Rect::from_min_size(zone.min, vec2(largeur, HAUTEUR_ONGLETS));
    let peintre = ui.painter().clone();
    peintre.rect_filled(bandeau, 0.0, FOND_BARRES);
    let police = FontId::proportional(13.5);
    let mut x = zone.min.x + 12.0;
    let mut clic = None;
    for (i, libelle) in libelles.iter().enumerate() {
        let galet = peintre.layout_no_wrap(libelle.clone(), police.clone(), Color32::WHITE);
        let w = galet.size().x + 40.0;
        let rect =
            Rect::from_min_size(pos2(x, bandeau.min.y + 2.0), vec2(w, HAUTEUR_ONGLETS - 2.0));
        let reponse = ui.interact(rect, Id::unique((sel, i)), Sense::click());
        let est_actif = i == actif;
        if est_actif {
            peintre.rect_filled(rect, CornerRadius::ZERO, ONGLET_ACTIF);
            let c = pos2(rect.center().x, rect.max.y);
            peintre.add(Shape::convex_polygon(
                vec![
                    pos2(c.x - 7.0, c.y),
                    pos2(c.x + 7.0, c.y),
                    pos2(c.x, c.y + HAUTEUR_ENCOCHE),
                ],
                ONGLET_ACTIF,
                Stroke::NONE,
            ));
        } else if reponse.hovered() {
            peintre.rect_filled(
                rect,
                CornerRadius::ZERO,
                Color32::from_rgb(0x3A, 0x3A, 0x3A),
            );
        }
        let couleur = if est_actif {
            Color32::WHITE
        } else {
            Color32::from_rgb(0xC8, 0xC8, 0xC8)
        };
        // Texte « gras » : le libellé est peint deux fois avec un décalage d'un demi-pixel.
        for dx in [0.0, 0.5] {
            peintre.text(
                rect.center() + vec2(dx, 0.0),
                Align2::CENTER_CENTER,
                libelle,
                police.clone(),
                couleur,
            );
        }
        if reponse.clicked() {
            clic = Some(i);
        }
        x += w + 2.0;
    }
    clic
}

pub fn afficher(
    ui: &mut egui::Ui,
    etat: &Etat,
    images: &mut GestionnaireImages,
    sortie: &mut Sortie,
) {
    let libelles: Vec<String> = OngletDetails::TOUS
        .iter()
        .map(|o| etat.i18n.t(o.cle()))
        .collect();
    let actif = OngletDetails::TOUS
        .iter()
        .position(|o| *o == etat.onglet)
        .unwrap_or(0);
    if let Some(i) = bandeau_onglets(ui, "onglets_details", &libelles, actif) {
        sortie.push(Message::ChoisirOnglet(OngletDetails::TOUS[i]));
    }
    egui::Frame::new()
        .inner_margin(Margin {
            left: 10,
            right: 10,
            top: 8,
            bottom: 6,
        })
        .show(ui, |ui| {
            let Some(film) = etat.film_principal() else {
                ui.add_space(12.0);
                ui.label(RichText::new(etat.i18n.t("gui.selection.none")).weak());
                return;
            };
            match etat.onglet {
                OngletDetails::Details => onglet_details(ui, etat, images, film, sortie),
                OngletDetails::Distribution => onglet_distribution(ui, etat, film),
                OngletDetails::FichiersMedias => onglet_fichiers(ui, etat, film),
                OngletDetails::Illustration => onglet_illustration(ui, etat, images, film, sortie),
                OngletDetails::BandeAnnonce => onglet_bande_annonce(ui, etat, film),
            }
        });
}

fn champ(ui: &mut egui::Ui, etat: &Etat, cle: &str, valeur: &str) {
    ui.label(RichText::new(etat.i18n.t(cle)).strong());
    ui.label(valeur);
    ui.end_row();
}

/// Ligne « libellé / valeur » d'une grille (valeur renvoyée à la ligne si nécessaire).
pub fn ligne(ui: &mut egui::Ui, libelle: &str, valeur: &str) {
    ui.label(RichText::new(libelle).strong());
    ui.add(Label::new(valeur).wrap());
    ui.end_row();
}

/// Largeur du plus long libellé (gras) : colonne commune à plusieurs grilles.
pub fn largeur_libelles(ui: &egui::Ui, libelles: &[String]) -> f32 {
    libelles
        .iter()
        .map(|l| {
            egui::WidgetText::from(RichText::new(l).strong())
                .into_galley(
                    ui,
                    Some(egui::TextWrapMode::Extend),
                    f32::INFINITY,
                    egui::FontSelection::Default,
                )
                .size()
                .x
        })
        .fold(0.0, f32::max)
}

/// URL `file://` d'un chemin local (Windows ou Unix).
pub fn url_fichier(chemin: &str) -> String {
    let p = chemin.replace('\\', "/");
    if let Some(reste) = p.strip_prefix("//") {
        format!("file://{reste}")
    } else if p.starts_with('/') {
        format!("file://{p}")
    } else {
        format!("file:///{p}")
    }
}

/// Chemin affiché comme un lien (ouvre le dossier).
pub fn lien_chemin(ui: &mut egui::Ui, chemin: &str) {
    if chemin.is_empty() {
        ui.label("");
    } else {
        ui.add(egui::Hyperlink::from_label_and_url(
            chemin,
            url_fichier(chemin),
        ));
    }
}

/// Dimensions réelles d'une image (lecture de l'en-tête, mémorisée par chemin).
fn dimensions(ui: &egui::Ui, chemin: &Path) -> Option<(u32, u32)> {
    let id = Id::unique(("dimensions_image", chemin));
    if let Some(d) = ui.data(|d| d.get_temp::<Option<(u32, u32)>>(id)) {
        return d;
    }
    let d = image::image_dimensions(chemin).ok();
    ui.data_mut(|m| m.insert_temp(id, d));
    d
}

/// Légende « Affiche - 680x1000 » (sans dimensions si l'image est absente).
pub fn legende(nom: &str, dimensions: Option<(u32, u32)>) -> String {
    match dimensions {
        Some((l, h)) => format!("{nom} - {l}x{h}"),
        None => nom.to_string(),
    }
}

/// Illustration de largeur fixe avec sa légende dessous ; clic = visionneuse plein écran.
pub fn illustration_legendee(
    ui: &mut egui::Ui,
    etat: &Etat,
    images: &mut GestionnaireImages,
    chemin: Option<PathBuf>,
    genre: GenreImage,
    taille: Vec2,
    sortie: &mut Sortie,
) {
    let nom = etat.i18n.t(genre.cle());
    let reponse = image_ou_emplacement(ui, images, chemin.as_deref(), taille, &nom);
    let dims = chemin.as_deref().and_then(|p| dimensions(ui, p));
    ui.add_sized(
        vec2(taille.x, 18.0),
        Label::new(RichText::new(legende(&nom, dims)).size(12.0)).truncate(),
    );
    if reponse.clicked()
        && let Some(p) = chemin
    {
        sortie.push(Message::OuvrirVisionneuse(p));
    }
}

/// Largeur de la colonne d'illustrations selon la largeur du panneau.
pub fn largeur_colonne_images(largeur_panneau: f32) -> f32 {
    (largeur_panneau * 0.22).clamp(120.0, 190.0)
}

/// Colonne d'illustrations : affiche, fanart, bannière.
pub fn colonne_images(
    ui: &mut egui::Ui,
    etat: &Etat,
    images: &mut GestionnaireImages,
    chemins: [Option<PathBuf>; 3],
    sortie: &mut Sortie,
) {
    let l = largeur_colonne_images(ui.available_width());
    ui.allocate_ui_with_layout(vec2(l, 0.0), Layout::top_down(Align::Min), |ui| {
        ui.set_width(l);
        let [affiche, fanart, banniere] = chemins;
        let genres = [
            (GenreImage::Affiche, affiche, l * 1.47),
            (GenreImage::Fanart, fanart, l * 0.5625),
            (GenreImage::Banniere, banniere, l * 0.185),
        ];
        for (genre, chemin, h) in genres {
            illustration_legendee(ui, etat, images, chemin, genre, vec2(l, h), sortie);
            ui.add_space(12.0);
        }
    });
}

/// Titre en gras et titre original dessous, puis filet.
pub fn titres(ui: &mut egui::Ui, titre: &str, original: &str) {
    ui.add(Label::new(RichText::new(titre).size(17.0).strong().color(TEXTE_TITRE)).wrap());
    if !original.is_empty() && original != titre {
        ui.label(RichText::new(original).size(13.0));
    }
    ui.add_space(2.0);
    ui.separator();
    ui.add_space(4.0);
}

/// Note avec pastille de la source et nombre de votes.
fn note(ui: &mut egui::Ui, source: &str, valeur: String, votes: u32) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(40.0, 24.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(
            rect,
            CornerRadius::same(5),
            Color32::from_rgb(0x1E, 0x6F, 0x8C),
        );
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            source,
            FontId::proportional(10.0),
            Color32::WHITE,
        );
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(RichText::new(valeur).size(15.0).strong().color(TEXTE_TITRE));
            if votes > 0 {
                ui.label(RichText::new(votes.to_string()).size(10.0));
            }
        });
    });
}

fn onglet_details(
    ui: &mut egui::Ui,
    etat: &Etat,
    images: &mut GestionnaireImages,
    film: &Film,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    ScrollArea::vertical()
        .id_salt("defilement_details")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                let chemins = [
                    etat.source.chemin_image(film, GenreImage::Affiche),
                    etat.source.chemin_image(film, GenreImage::Fanart),
                    etat.source.chemin_image(film, GenreImage::Banniere),
                ];
                colonne_images(ui, etat, images, chemins, sortie);
                ui.add_space(14.0);
                ui.vertical(|ui| {
                    titres(ui, &film.titre, &film.titre_original);
                    let libelles: Vec<String> = [
                        "metatag.year",
                        "metatag.releasedate",
                        "metatag.certification",
                        "metatag.runtime",
                        "metatag.genre",
                        "metatag.director",
                        "metatag.production",
                        "metatag.country",
                        "metatag.movieset",
                    ]
                    .iter()
                    .map(|c| i.t(c))
                    .collect();
                    let l = largeur_libelles(ui, &libelles);
                    // Trois premières lignes : identifiants en liens dans une seconde colonne.
                    egui::Grid::new("grille_details_haut")
                        .num_columns(4)
                        .min_col_width(l)
                        .spacing([14.0, 3.0])
                        .show(ui, |ui| {
                            let ids: [(&str, &str, String); 3] = [
                                (
                                    "IMDb:",
                                    &film.imdb,
                                    format!("https://www.imdb.com/title/{}", film.imdb),
                                ),
                                (
                                    "TMDB:",
                                    &film.tmdb,
                                    format!("https://www.themoviedb.org/movie/{}", film.tmdb),
                                ),
                                ("", "", String::new()),
                            ];
                            let valeurs = [
                                if film.annee > 0 {
                                    film.annee.to_string()
                                } else {
                                    String::new()
                                },
                                film.date_sortie.clone(),
                                film.certification.clone(),
                            ];
                            for (k, ((nom, id, url), valeur)) in
                                ids.into_iter().zip(valeurs).enumerate()
                            {
                                ui.label(RichText::new(&libelles[k]).strong());
                                ui.label(valeur);
                                ui.label(RichText::new(nom).strong());
                                if id.is_empty() {
                                    ui.label("");
                                } else {
                                    ui.hyperlink_to(id, url);
                                }
                                ui.end_row();
                            }
                        });
                    egui::Grid::new("grille_details_bas")
                        .num_columns(2)
                        .min_col_width(l)
                        .spacing([14.0, 3.0])
                        .show(ui, |ui| {
                            let duree = if film.duree_min > 0 {
                                i.t_args(
                                    "column.minutes",
                                    &[("count", &film.duree_min.to_string())],
                                )
                            } else {
                                String::new()
                            };
                            ligne(ui, &libelles[3], &duree);
                            ligne(ui, &libelles[4], &film.genres.join(", "));
                            ligne(ui, &libelles[5], &film.realisateurs.join(", "));
                            ligne(ui, &libelles[6], &film.production.join(" / "));
                            ligne(ui, &libelles[7], &film.pays.join(", "));
                            if let Some(saga) = &film.saga {
                                ligne(ui, &libelles[8], saga);
                            }
                        });
                    ui.add_space(10.0);
                    let source = film
                        .notes_sources
                        .first()
                        .map(|(s, _)| s.to_uppercase())
                        .unwrap_or_else(|| "TMDB".into());
                    note(ui, &source, i.decimal(film.note as f64, 1), film.votes);
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(14.0);
                    ui.label(RichText::new(i.t("gui.details.quote")).strong());
                    ui.add(Label::new(RichText::new(&film.slogan).italics()).wrap());
                    ui.add_space(12.0);
                    ui.label(RichText::new(i.t("metatag.plot")).strong());
                    ui.add(Label::new(&film.resume).wrap());
                    ui.add_space(6.0);
                    ui.separator();
                    egui::Grid::new("grille_details_fin")
                        .num_columns(2)
                        .min_col_width(l)
                        .spacing([14.0, 4.0])
                        .show(ui, |ui| {
                            ligne(ui, &i.t("metatag.tags"), "");
                            ui.label(RichText::new(i.t("metatag.path")).strong());
                            lien_chemin(ui, &film.chemin());
                            ui.end_row();
                            ligne(ui, &i.t("metatag.dateadded"), &film.date_ajout);
                        });
                });
            });
        });
}

fn onglet_distribution(ui: &mut egui::Ui, etat: &Etat, film: &Film) {
    TableBuilder::new(ui)
        .id_salt("table_distribution")
        .striped(true)
        .column(Column::remainder().at_least(120.0))
        .column(Column::remainder().at_least(120.0))
        .header(22.0, |mut h| {
            h.col(|ui| {
                ui.strong(etat.i18n.t("metatag.actor"));
            });
            h.col(|ui| {
                ui.strong(etat.i18n.t("metatag.role"));
            });
        })
        .body(|body| {
            body.rows(22.0, film.acteurs.len(), |mut ligne| {
                let (nom, role) = &film.acteurs[ligne.index()];
                ligne.col(|ui| {
                    ui.label(nom);
                });
                ligne.col(|ui| {
                    ui.label(role);
                });
            });
        });
}

fn onglet_fichiers(ui: &mut egui::Ui, etat: &Etat, film: &Film) {
    let i = &etat.i18n;
    egui::Grid::new("grille_fichiers")
        .num_columns(2)
        .spacing([14.0, 4.0])
        .show(ui, |ui| {
            ui.label(RichText::new(i.t("metatag.path")).strong());
            lien_chemin(ui, &film.chemin());
            ui.end_row();
            champ(ui, etat, "metatag.datasource", &film.source);
            champ(ui, etat, "metatag.dateadded", &film.date_ajout);
            champ(ui, etat, "gui.media.resolution", &film.format_video());
            champ(ui, etat, "metatag.videocodec", &film.codec_video);
            champ(ui, etat, "metatag.audio", &film.audio);
            champ(
                ui,
                etat,
                "metatag.watched",
                if film.vu {
                    i.t("Button.yes")
                } else {
                    i.t("Button.no")
                }
                .as_str(),
            );
        });
    ui.add_space(8.0);
    ui.label(RichText::new(i.t("gui.media.files")).strong());
    TableBuilder::new(ui)
        .id_salt("table_fichiers")
        .striped(true)
        .column(Column::initial(80.0))
        .column(Column::remainder().clip(true))
        .column(Column::initial(90.0))
        .header(22.0, |mut h| {
            for cle in ["renamer.type", "metatag.filename", "metatag.filesize"] {
                h.col(|ui| {
                    ui.strong(i.t(cle));
                });
            }
        })
        .body(|body| {
            body.rows(22.0, film.fichiers.len(), |mut ligne| {
                let f = &film.fichiers[ligne.index()];
                ligne.col(|ui| {
                    ui.label(f.type_fichier.libelle());
                });
                ligne.col(|ui| {
                    ui.label(&f.nom);
                });
                ligne.col(|ui| {
                    ui.label(format!(
                        "{} {}",
                        i.decimal(gigaoctets(f.taille_octets), 3),
                        i.t("gui.unit.gigabyte")
                    ));
                });
            });
        });
}

fn onglet_illustration(
    ui: &mut egui::Ui,
    etat: &Etat,
    images: &mut GestionnaireImages,
    film: &Film,
    sortie: &mut Sortie,
) {
    ScrollArea::vertical()
        .id_salt("illustrations")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let tailles = [
                    (GenreImage::Affiche, Vec2::new(140.0, 210.0)),
                    (GenreImage::Fanart, Vec2::new(280.0, 158.0)),
                    (GenreImage::Banniere, Vec2::new(280.0, 52.0)),
                    (GenreImage::Miniature, Vec2::new(200.0, 112.0)),
                ];
                for (genre, taille) in tailles {
                    ui.vertical(|ui| {
                        let chemin = etat.source.chemin_image(film, genre);
                        illustration_legendee(ui, etat, images, chemin, genre, taille, sortie);
                    });
                }
            });
        });
}

fn onglet_bande_annonce(ui: &mut egui::Ui, etat: &Etat, film: &Film) {
    if film.bandes_annonces.is_empty() {
        ui.label(RichText::new(etat.i18n.t("gui.trailer.none")).weak());
        return;
    }
    for b in &film.bandes_annonces {
        ui.horizontal(|ui| {
            ui.label(b);
            if ui
                .button(format!("{} {}", polices::LIRE, etat.i18n.t("Button.play")))
                .clicked()
            {}
        });
    }
}

/// Couleur de fond des zones de contenu (liste et détails).
pub fn fond_contenu(etat: &Etat) -> Color32 {
    fond_alterne(etat.reglages.theme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legendes_et_liens() {
        assert_eq!(legende("Affiche", Some((680, 1000))), "Affiche - 680x1000");
        assert_eq!(legende("Bannière", None), "Bannière");
        assert_eq!(url_fichier("/srv/films/A"), "file:///srv/films/A");
        assert_eq!(url_fichier("G:\\Films\\A"), "file:///G:/Films/A");
        assert_eq!(url_fichier("\\\\NAS\\Films"), "file://NAS/Films");
        assert_eq!(largeur_colonne_images(400.0), 120.0);
        assert_eq!(largeur_colonne_images(5000.0), 190.0);
    }

    #[test]
    fn cles_de_l_interface_traduites() {
        use crate::i18n::{I18n, Langue};
        for langue in [Langue::Francais, Langue::Anglais] {
            let i = I18n::new(langue);
            for cle in [
                "gui.details.quote",
                "gui.toolbar.unlocked",
                "gui.tvshows.total",
                "gui.tvshows.episodestotal",
                "gui.tvshows.seasons",
                "gui.tvshows.episodecount",
                "metatag.tags",
                "app.nothingselected",
            ] {
                assert!(!i.t(cle).starts_with("???"), "{cle}");
            }
            assert!(
                !i.t_pluriel("gui.tvshows.selected", 1, &[])
                    .starts_with("???")
            );
            assert!(
                !i.t_pluriel("gui.tvshows.episodesselected", 3, &[])
                    .contains("???")
            );
        }
    }
}
