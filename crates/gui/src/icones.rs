//! Icônes vectorielles originales (fichiers SVG du dossier `icones/`, embarqués par
//! `include_bytes` et rendus par le chargeur SVG d'egui_extras) et pictogrammes d'état des
//! cellules (coche, croix, avertissement) dessinés au pinceau pour rester nets à toute taille.
//!
//! Les icônes de la barre d'outils, le cadenas et le logo ont leurs couleurs propres ; les
//! autres sont dessinées en blanc et teintées à l'affichage (gris au repos, orange si actives).

use crate::modele::{Colonne, Film};
use crate::theme::{BLEU_ETAT, ORANGE_ETAT, ROUGE_ETAT, VERT_ETAT};
use eframe::egui::{self, Color32, ImageSource, Painter, Pos2, Response, Sense, Shape, Stroke};
use eframe::egui::{Vec2, pos2, vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icone {
    Logo,
    MiseAJour,
    Recherche,
    Modifier,
    Renommer,
    Cadenas,
    Cloche,
    Films,
    Collections,
    Series,
    Outils,
    Reglages,
    Info,
    Nouveau,
    Nfo,
    Image,
    BandeAnnonce,
    SousTitres,
    Vu,
    Etoile,
    Loupe,
}

impl Icone {
    pub const TOUTES: [Icone; 21] = [
        Icone::Logo,
        Icone::MiseAJour,
        Icone::Recherche,
        Icone::Modifier,
        Icone::Renommer,
        Icone::Cadenas,
        Icone::Cloche,
        Icone::Films,
        Icone::Collections,
        Icone::Series,
        Icone::Outils,
        Icone::Reglages,
        Icone::Info,
        Icone::Nouveau,
        Icone::Nfo,
        Icone::Image,
        Icone::BandeAnnonce,
        Icone::SousTitres,
        Icone::Vu,
        Icone::Etoile,
        Icone::Loupe,
    ];

    /// Source embarquée (URI `bytes://…svg`).
    pub fn source(self) -> ImageSource<'static> {
        match self {
            Icone::Logo => egui::include_image!("../icones/logo.svg"),
            Icone::MiseAJour => egui::include_image!("../icones/mise_a_jour.svg"),
            Icone::Recherche => egui::include_image!("../icones/recherche.svg"),
            Icone::Modifier => egui::include_image!("../icones/modifier.svg"),
            Icone::Renommer => egui::include_image!("../icones/renommer.svg"),
            Icone::Cadenas => egui::include_image!("../icones/cadenas.svg"),
            Icone::Cloche => egui::include_image!("../icones/cloche.svg"),
            Icone::Films => egui::include_image!("../icones/films.svg"),
            Icone::Collections => egui::include_image!("../icones/collections.svg"),
            Icone::Series => egui::include_image!("../icones/series.svg"),
            Icone::Outils => egui::include_image!("../icones/outils.svg"),
            Icone::Reglages => egui::include_image!("../icones/reglages.svg"),
            Icone::Info => egui::include_image!("../icones/info.svg"),
            Icone::Nouveau => egui::include_image!("../icones/nouveau.svg"),
            Icone::Nfo => egui::include_image!("../icones/nfo.svg"),
            Icone::Image => egui::include_image!("../icones/image.svg"),
            Icone::BandeAnnonce => egui::include_image!("../icones/bande_annonce.svg"),
            Icone::SousTitres => egui::include_image!("../icones/sous_titres.svg"),
            Icone::Vu => egui::include_image!("../icones/vu.svg"),
            Icone::Etoile => egui::include_image!("../icones/etoile.svg"),
            Icone::Loupe => egui::include_image!("../icones/loupe.svg"),
        }
    }

    /// Vrai pour les icônes dessinées en blanc, destinées à être teintées.
    pub fn teintable(self) -> bool {
        !matches!(
            self,
            Icone::Logo
                | Icone::MiseAJour
                | Icone::Recherche
                | Icone::Modifier
                | Icone::Renommer
                | Icone::Cadenas
        )
    }
}

/// Image d'icône de côté `cote` (couleurs d'origine).
pub fn image(icone: Icone, cote: f32) -> egui::Image<'static> {
    egui::Image::new(icone.source()).fit_to_exact_size(Vec2::splat(cote))
}

/// Image d'icône teintée (multiplication : une icône blanche prend la couleur `teinte`).
pub fn image_teintee(icone: Icone, cote: f32, teinte: Color32) -> egui::Image<'static> {
    image(icone, cote).tint(teinte)
}

/// Contenu d'une cellule d'état du tableau.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EtatCellule {
    /// Coche verte.
    Oui,
    /// Croix rouge.
    Non,
    /// Triangle orange : présent mais incomplet.
    Attention,
    /// Pastille bleue (« nouveau »).
    Nouveau,
    /// Cellule vide.
    Vide,
}

/// Icône d'en-tête d'une colonne (colonnes d'état et note) ; `None` = libellé texte.
pub fn icone_entete(col: Colonne) -> Option<Icone> {
    match col {
        Colonne::Nouveau => Some(Icone::Nouveau),
        Colonne::Metadonnees => Some(Icone::Nfo),
        Colonne::Images => Some(Icone::Image),
        Colonne::BandeAnnonce => Some(Icone::BandeAnnonce),
        Colonne::SousTitres => Some(Icone::SousTitres),
        Colonne::Vu => Some(Icone::Vu),
        Colonne::Note => Some(Icone::Etoile),
        _ => None,
    }
}

/// État affiché dans une colonne booléenne pour un film (`None` : colonne non booléenne).
///
/// Métadonnées : sans NFO mais avec un identifiant connu (IMDb / TMDB), le film a déjà été
/// identifié : avertissement plutôt que croix.
pub fn etat_film(film: &Film, col: Colonne) -> Option<EtatCellule> {
    let oui_non = |v: bool| {
        if v {
            EtatCellule::Oui
        } else {
            EtatCellule::Non
        }
    };
    Some(match col {
        Colonne::Nouveau => {
            if film.nouveau {
                EtatCellule::Nouveau
            } else {
                EtatCellule::Vide
            }
        }
        Colonne::Metadonnees => {
            if film.metadonnees_ok {
                EtatCellule::Oui
            } else if !film.imdb.is_empty() || !film.tmdb.is_empty() {
                EtatCellule::Attention
            } else {
                EtatCellule::Non
            }
        }
        Colonne::Images => oui_non(film.images_ok),
        Colonne::BandeAnnonce => oui_non(film.bande_annonce),
        Colonne::SousTitres => oui_non(film.sous_titres),
        Colonne::Vu => oui_non(film.vu),
        _ => return None,
    })
}

/// Couleur d'un état (`None` pour une cellule vide).
pub fn couleur_etat(etat: EtatCellule) -> Option<Color32> {
    match etat {
        EtatCellule::Oui => Some(VERT_ETAT),
        EtatCellule::Non => Some(ROUGE_ETAT),
        EtatCellule::Attention => Some(ORANGE_ETAT),
        EtatCellule::Nouveau => Some(BLEU_ETAT),
        EtatCellule::Vide => None,
    }
}

/// Peint un pictogramme d'état de côté ≈ 12 px centré sur `centre`.
pub fn peindre_etat(peintre: &Painter, centre: Pos2, etat: EtatCellule) {
    let Some(c) = couleur_etat(etat) else {
        return;
    };
    let s = 5.0;
    match etat {
        EtatCellule::Oui => {
            let trait_ = Stroke::new(1.6, c);
            peintre.line(
                vec![
                    centre + vec2(-s, 0.0),
                    centre + vec2(-s * 0.3, s * 0.75),
                    centre + vec2(s, -s * 0.8),
                ],
                trait_,
            );
        }
        EtatCellule::Non => {
            let trait_ = Stroke::new(1.4, c);
            let d = s * 0.75;
            peintre.line_segment([centre + vec2(-d, -d), centre + vec2(d, d)], trait_);
            peintre.line_segment([centre + vec2(-d, d), centre + vec2(d, -d)], trait_);
        }
        EtatCellule::Attention => {
            let haut = centre + vec2(0.0, -s - 1.0);
            let triangle = vec![haut, centre + vec2(s + 1.5, s), centre + vec2(-s - 1.5, s)];
            peintre.add(Shape::closed_line(triangle, Stroke::new(1.3, c)));
            peintre.line_segment(
                [centre + vec2(0.0, -s * 0.35), centre + vec2(0.0, s * 0.35)],
                Stroke::new(1.3, c),
            );
            peintre.circle_filled(centre + vec2(0.0, s * 0.72), 0.8, c);
        }
        EtatCellule::Nouveau => {
            peintre.circle_filled(centre, 3.5, c);
        }
        EtatCellule::Vide => {}
    }
}

/// Alloue une cellule d'état (toute la place disponible) et y peint le pictogramme centré.
pub fn cellule_etat(ui: &mut egui::Ui, etat: EtatCellule) -> Response {
    let taille = vec2(
        ui.available_width().max(12.0),
        ui.available_height().max(12.0),
    );
    let (rect, reponse) = ui.allocate_exact_size(taille, Sense::hover());
    peindre_etat(ui.painter(), rect.center(), etat);
    reponse
}

/// Petit triangle plein pointant vers le bas (flèche de menu) ou vers la droite.
pub fn peindre_triangle(peintre: &Painter, centre: Pos2, cote: f32, vers_bas: bool, c: Color32) {
    let h = cote * 0.5;
    let points = if vers_bas {
        vec![
            pos2(centre.x - h, centre.y - h * 0.5),
            pos2(centre.x + h, centre.y - h * 0.5),
            pos2(centre.x, centre.y + h * 0.6),
        ]
    } else {
        vec![
            pos2(centre.x - h * 0.5, centre.y - h),
            pos2(centre.x + h * 0.6, centre.y),
            pos2(centre.x - h * 0.5, centre.y + h),
        ]
    };
    peintre.add(Shape::convex_polygon(points, c, Stroke::NONE));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::donnees;
    use eframe::egui::load::{ImagePoll, SizeHint};

    #[test]
    fn icones_d_en_tete_des_colonnes_d_etat() {
        for col in Colonne::TOUTES {
            if col.est_booleenne() {
                assert!(icone_entete(col).is_some(), "{col:?}");
            }
        }
        assert_eq!(icone_entete(Colonne::Vu), Some(Icone::Vu));
        assert_eq!(icone_entete(Colonne::Metadonnees), Some(Icone::Nfo));
        assert_eq!(icone_entete(Colonne::Titre), None);
        assert_eq!(icone_entete(Colonne::Annee), None);
    }

    #[test]
    fn selection_des_etats_de_cellule() {
        let mut f = donnees::films_factices().remove(0);
        assert_eq!(etat_film(&f, Colonne::Titre), None);
        f.vu = true;
        f.sous_titres = false;
        f.nouveau = false;
        assert_eq!(etat_film(&f, Colonne::Vu), Some(EtatCellule::Oui));
        assert_eq!(etat_film(&f, Colonne::SousTitres), Some(EtatCellule::Non));
        assert_eq!(etat_film(&f, Colonne::Nouveau), Some(EtatCellule::Vide));
        f.nouveau = true;
        assert_eq!(etat_film(&f, Colonne::Nouveau), Some(EtatCellule::Nouveau));
        f.metadonnees_ok = true;
        assert_eq!(etat_film(&f, Colonne::Metadonnees), Some(EtatCellule::Oui));
        f.metadonnees_ok = false;
        f.imdb = "tt0000001".into();
        assert_eq!(
            etat_film(&f, Colonne::Metadonnees),
            Some(EtatCellule::Attention)
        );
        f.imdb.clear();
        f.tmdb.clear();
        assert_eq!(etat_film(&f, Colonne::Metadonnees), Some(EtatCellule::Non));
        assert_eq!(couleur_etat(EtatCellule::Oui), Some(VERT_ETAT));
        assert_eq!(couleur_etat(EtatCellule::Non), Some(ROUGE_ETAT));
        assert_eq!(couleur_etat(EtatCellule::Vide), None);
    }

    /// Chaque SVG se rastérise ; les icônes de couleur gardent leur teinte, les autres sont blanches.
    #[test]
    fn icones_rasterisees_et_colorees() {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        for icone in Icone::TOUTES {
            let ImageSource::Bytes { uri, bytes } = icone.source() else {
                panic!("source non embarquée");
            };
            ctx.include_bytes(uri.clone(), bytes);
            let image = match ctx.try_load_image(
                &uri,
                SizeHint::Size {
                    width: 48,
                    height: 48,
                    maintain_aspect_ratio: true,
                },
            ) {
                Ok(ImagePoll::Ready { image }) => image,
                Ok(ImagePoll::Pending { .. }) => panic!("{icone:?} : chargement différé"),
                Err(e) => panic!("{icone:?} : {e}"),
            };
            let opaques: Vec<_> = image.pixels.iter().filter(|p| p.a() > 200).collect();
            assert!(opaques.len() > 40, "{icone:?} presque vide");
            let blanche = opaques
                .iter()
                .all(|p| p.r() > 200 && p.g() > 200 && p.b() > 200);
            if icone == Icone::Logo {
                continue;
            }
            assert_eq!(blanche, icone.teintable(), "{icone:?}");
        }
    }

    #[test]
    fn aucun_svg_ne_mentionne_un_autre_programme() {
        for icone in Icone::TOUTES {
            let ImageSource::Bytes { bytes, .. } = icone.source() else {
                unreachable!()
            };
            let texte = String::from_utf8(bytes.to_vec()).unwrap().to_lowercase();
            assert!(texte.starts_with("<svg"), "{icone:?}");
            assert!(!texte.contains("<text"), "{icone:?}");
            assert!(!texte.contains("<!--"), "{icone:?}");
        }
    }
}
