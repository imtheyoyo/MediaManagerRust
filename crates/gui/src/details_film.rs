//! Données d'un film propres à la fenêtre « Éditer un film » : tout ce que le modèle d'affichage
//! `Film` ne porte pas (notes par source, identifiants, équipe, illustrations supplémentaires,
//! adresses d'artwork, bandes-annonces complètes, flux des fichiers médias...).
//!
//! Ces structures sont orientées édition (champs de texte, booléens, nombres) ; la conversion
//! depuis et vers le modèle des services est faite par `conversion.rs`.

use crate::dates;
use crate::modele::{Film, TypeFichier};
use std::path::PathBuf;

/// Emplacements d'illustration gérés par l'éditeur (ordre d'affichage des onglets).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TypeArt {
    Affiche,
    Fanart,
    Clearlogo,
    Banniere,
    ClearArt,
    Miniature,
    Disque,
    Keyart,
    SquareArt,
}

impl TypeArt {
    pub const TOUS: [TypeArt; 9] = [
        TypeArt::Affiche,
        TypeArt::Fanart,
        TypeArt::Clearlogo,
        TypeArt::Banniere,
        TypeArt::ClearArt,
        TypeArt::Miniature,
        TypeArt::Disque,
        TypeArt::Keyart,
        TypeArt::SquareArt,
    ];

    /// Rang dans [`TypeArt::TOUS`].
    pub fn rang(self) -> usize {
        self as usize
    }

    /// Clé de traduction du libellé.
    pub fn cle(self) -> &'static str {
        match self {
            TypeArt::Affiche => "editor.art.poster",
            TypeArt::Fanart => "editor.art.fanart",
            TypeArt::Clearlogo => "editor.art.clearlogo",
            TypeArt::Banniere => "editor.art.banner",
            TypeArt::ClearArt => "editor.art.clearart",
            TypeArt::Miniature => "editor.art.thumb",
            TypeArt::Disque => "editor.art.disc",
            TypeArt::Keyart => "editor.art.keyart",
            TypeArt::SquareArt => "editor.art.squareart",
        }
    }
}

/// Fonction d'une personne dans la production (type du modèle des services).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FonctionPersonne {
    Acteur,
    Realisateur,
    Scenariste,
    Producteur,
    Invite,
    Compositeur,
    Monteur,
    Camera,
    #[default]
    Autre,
}

impl FonctionPersonne {
    /// Fonction déduite de l'intitulé d'un rôle d'équipe (lignes ajoutées à la main).
    pub fn depuis_role(role: &str) -> Self {
        let r = role.trim().to_lowercase();
        if r.is_empty() {
            return Self::Autre;
        }
        if r.contains("photograph") || r.contains("camera") || r.contains("caméra") {
            Self::Camera
        } else if r.contains("music") || r.contains("compos") {
            Self::Compositeur
        } else if r.contains("editor") || r.contains("monteu") {
            Self::Monteur
        } else if r.contains("producer") || r.contains("produc") {
            Self::Producteur
        } else if r.contains("writer") || r.contains("screenplay") || r.contains("scénar") {
            Self::Scenariste
        } else if r == "director" || r.contains("réalisa") {
            Self::Realisateur
        } else {
            Self::Autre
        }
    }
}

/// Acteur ou membre de l'équipe.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PersonneFilm {
    pub nom: String,
    /// Rôle joué (acteur) ou fonction précise (équipe).
    pub role: String,
    /// Adresse de la photo (vide si absente).
    pub url_vignette: String,
    /// Adresse du profil (vide si absente).
    pub url_profil: String,
    pub fonction: FonctionPersonne,
}

impl PersonneFilm {
    pub fn nouvelle(fonction: FonctionPersonne) -> Self {
        Self {
            fonction,
            ..Self::default()
        }
    }

    /// Fonction enregistrée, ou déduite du rôle quand elle est inconnue.
    pub fn fonction_effective(&self) -> FonctionPersonne {
        match self.fonction {
            FonctionPersonne::Autre => FonctionPersonne::depuis_role(&self.role),
            f => f,
        }
    }

    pub fn a_image(&self) -> bool {
        !self.url_vignette.trim().is_empty()
    }

    pub fn a_profil(&self) -> bool {
        !self.url_profil.trim().is_empty()
    }
}

/// Note attribuée par une source (tableau « Cote d'évaluation »).
#[derive(Clone, Debug, PartialEq)]
pub struct NoteSource {
    pub source: String,
    pub valeur: f32,
    pub maximum: u32,
    pub votes: u32,
}

impl Default for NoteSource {
    fn default() -> Self {
        Self {
            source: String::new(),
            valeur: 0.0,
            maximum: 10,
            votes: 0,
        }
    }
}

/// Bande-annonce (tableau de l'onglet « URL de l'artwork de bande annonce »).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BandeAnnonceEdit {
    /// Identifiant interne (vide pour une ligne ajoutée à la main).
    pub id: String,
    pub nom: String,
    pub source: String,
    pub qualite: String,
    pub langue: String,
    pub url: String,
    /// Écrite dans le NFO.
    pub dans_nfo: bool,
}

/// Flux audio d'un fichier.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AudioEdit {
    /// Rang du flux dans le fichier d'origine (`None` : ligne ajoutée à la main).
    pub origine: Option<usize>,
    pub codec: String,
    pub canaux: u32,
    pub debit: u32,
    pub profondeur: u32,
    pub langue: String,
    pub defaut: bool,
    pub force: bool,
    pub titre: String,
}

/// Flux de sous-titres d'un fichier.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SousTitresEdit {
    /// Rang du flux dans le fichier d'origine (`None` : ligne ajoutée à la main).
    pub origine: Option<usize>,
    pub langue: String,
    pub defaut: bool,
    pub force: bool,
    pub malentendant: bool,
    pub format: String,
    pub titre: String,
}

/// Fichier vidéo et ses informations techniques (onglet « Fichiers médias »).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MediaEdit {
    /// Chemin complet (identité du fichier dans le modèle des services).
    pub chemin: String,
    pub nom: String,
    pub codec: String,
    pub format_conteneur: String,
    pub largeur: u32,
    pub hauteur: u32,
    pub images_par_seconde: f64,
    pub debit: u32,
    pub profondeur: u32,
    pub duree_secondes: u32,
    pub format_3d: String,
    pub format_hdr: String,
    /// Rapport hauteur/largeur (0 = inconnu).
    pub ratio: f32,
    pub ratio2: Option<f32>,
    pub audio: Vec<AudioEdit>,
    pub sous_titres: Vec<SousTitresEdit>,
}

impl MediaEdit {
    /// Durée en minutes (arrondie au-dessus).
    pub fn duree_minutes(&self) -> u32 {
        self.duree_secondes.div_ceil(60)
    }

    /// Fixe la durée en minutes (sans effet si la valeur est celle déjà affichée, pour ne pas
    /// perdre les secondes).
    pub fn definir_duree_minutes(&mut self, minutes: u32) {
        if minutes != self.duree_minutes() {
            self.duree_secondes = minutes * 60;
        }
    }

    /// Rapport d'aspect le plus proche de la forme de l'image (largeur / hauteur).
    pub fn ratio_detecte(&self) -> Option<f32> {
        (self.largeur > 0 && self.hauteur > 0)
            .then(|| ratio_standard_proche(self.largeur as f32 / self.hauteur as f32))
    }
}

/// Rapports d'aspect proposés : (valeur, libellé).
pub const RATIOS: [(f32, &str); 11] = [
    (1.33, "4:3 (1.33:1)"),
    (1.37, "1.37:1"),
    (1.66, "5:3 (1.66:1)"),
    (1.78, "16:9 (1.78:1)"),
    (1.85, "1.85:1"),
    (2.00, "2:1 (2.00:1)"),
    (2.20, "2.20:1"),
    (2.35, "2.35:1"),
    (2.39, "2.39:1"),
    (2.40, "2.40:1"),
    (2.76, "2.76:1"),
];

/// Rapport standard le plus proche de `valeur`.
pub fn ratio_standard_proche(valeur: f32) -> f32 {
    RATIOS
        .iter()
        .map(|(r, _)| *r)
        .min_by(|a, b| (a - valeur).abs().total_cmp(&(b - valeur).abs()))
        .unwrap_or(valeur)
}

/// Libellé d'un rapport d'aspect (vide si inconnu).
pub fn libelle_ratio(valeur: f32) -> String {
    if valeur <= 0.0 {
        return String::new();
    }
    RATIOS
        .iter()
        .find(|(r, _)| (r - valeur).abs() < 0.015)
        .map(|(_, l)| (*l).to_string())
        .unwrap_or_else(|| format!("{valeur:.2}:1"))
}

/// Formats 3D proposés.
pub const FORMATS_3D: [&str; 5] = ["3D SBS", "3D H-SBS", "3D TAB", "3D H-TAB", "3D MVC"];

/// Illustrations locales du film (une par emplacement de [`TypeArt::TOUS`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImagesFilm {
    pub chemins: [Option<PathBuf>; 9],
    pub dimensions: [Option<(u32, u32)>; 9],
    /// Emplacements dont la suppression est demandée (appliquée à la validation).
    pub supprimes: [bool; 9],
}

impl ImagesFilm {
    /// Chemin affiché (`None` si absent ou supprimé).
    pub fn chemin(&self, t: TypeArt) -> Option<&std::path::Path> {
        (!self.supprimes[t.rang()])
            .then(|| self.chemins[t.rang()].as_deref())
            .flatten()
    }

    pub fn dimensions(&self, t: TypeArt) -> Option<(u32, u32)> {
        self.dimensions[t.rang()]
    }
}

/// Données éditables d'un film qui s'ajoutent à celles du modèle d'affichage.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DetailsFilm {
    pub titre_anglais: String,
    /// « Contour » (résumé court).
    pub contour: String,
    /// « Note » : texte libre de l'utilisateur.
    pub note_libre: String,
    pub langues_parlees: String,
    /// « Évaluation des utilisateurs » (0 = aucune).
    pub evaluation_utilisateur: f32,
    pub top250: u32,
    pub notes: Vec<NoteSource>,
    /// `jj/mm/aaaa hh:mm`.
    pub date_ajout: String,
    /// Horodatage d'origine de la date d'ajout (ms ; 0 = inconnu).
    pub date_ajout_ms: i64,
    /// Date de dernière lecture, `jj/mm/aaaa hh:mm` (vide : aucune).
    pub date_vu: String,
    pub date_vu_ms: Option<i64>,
    /// Code d'une entrée de `SOURCES_MEDIA` (vide : inconnue).
    pub source_media: String,
    pub video_3d: bool,
    /// Libellé de l'édition (vide : aucune).
    pub edition: String,
    pub liens_series: Vec<String>,
    /// Identifiants externes (clé, valeur).
    pub ids: Vec<(String, String)>,
    pub mots_cles: Vec<String>,
    pub acteurs: Vec<PersonneFilm>,
    pub equipe: Vec<PersonneFilm>,
    pub images: ImagesFilm,
    /// Adresses d'artwork, une par emplacement de [`TypeArt::TOUS`].
    pub urls: [String; 9],
    pub bandes_annonces: Vec<BandeAnnonceEdit>,
    pub medias: Vec<MediaEdit>,
}

/// Valeurs de départ déduites d'un film d'affichage (sources de démonstration, tests).
impl DetailsFilm {
    pub fn depuis_film(f: &Film) -> Self {
        let mut ids = Vec::new();
        if !f.tmdb.is_empty() {
            ids.push(("tmdb".to_string(), f.tmdb.clone()));
        }
        if !f.imdb.is_empty() {
            ids.push(("imdb".to_string(), f.imdb.clone()));
        }
        let mut equipe: Vec<PersonneFilm> = f
            .realisateurs
            .iter()
            .map(|n| PersonneFilm {
                nom: n.clone(),
                role: "Director".into(),
                fonction: FonctionPersonne::Realisateur,
                ..PersonneFilm::default()
            })
            .collect();
        equipe.extend(f.scenaristes.iter().map(|n| PersonneFilm {
            nom: n.clone(),
            role: "Writer".into(),
            fonction: FonctionPersonne::Scenariste,
            ..PersonneFilm::default()
        }));
        Self {
            date_ajout: match f.date_ajout.split_once(' ') {
                Some((d, h)) => format!("{} {h}", dates::iso_vers_fr(d)),
                None => dates::iso_vers_fr(&f.date_ajout),
            },
            date_ajout_ms: 0,
            notes: f
                .notes_sources
                .iter()
                .map(|(s, v)| NoteSource {
                    source: s.clone(),
                    valeur: *v,
                    maximum: 10,
                    votes: f.votes,
                })
                .collect(),
            ids,
            equipe,
            acteurs: f
                .acteurs
                .iter()
                .map(|(n, r)| PersonneFilm {
                    nom: n.clone(),
                    role: r.clone(),
                    fonction: FonctionPersonne::Acteur,
                    ..PersonneFilm::default()
                })
                .collect(),
            bandes_annonces: f
                .bandes_annonces
                .iter()
                .map(|b| BandeAnnonceEdit {
                    nom: b.nom.clone(),
                    source: b.source.clone(),
                    qualite: b.qualite.clone(),
                    url: b.url.clone(),
                    ..BandeAnnonceEdit::default()
                })
                .collect(),
            medias: f
                .fichiers
                .iter()
                .filter(|x| x.type_fichier == TypeFichier::Video)
                .map(|x| {
                    let (largeur, hauteur) = x
                        .resolution
                        .split_once('x')
                        .and_then(|(l, h)| Some((l.parse().ok()?, h.parse().ok()?)))
                        .unwrap_or((0, 0));
                    MediaEdit {
                        chemin: f.chemin_fichier(x).display().to_string(),
                        nom: x.nom.clone(),
                        codec: x.codec.clone(),
                        largeur,
                        hauteur,
                        ..MediaEdit::default()
                    }
                })
                .collect(),
            ..Self::default()
        }
    }
}

/// Note affichée d'une liste de notes (source prioritaire : tmdb, imdb, metacritic...).
/// Renvoie (note sur 10, votes).
pub fn note_principale(notes: &[NoteSource]) -> (f32, u32) {
    const PRIORITAIRES: [&str; 5] = [
        "tmdb",
        "imdb",
        "metacritic",
        "tomatometerallcritics",
        "trakt",
    ];
    let valide = |n: &&NoteSource| n.valeur > 0.0 && n.maximum > 0;
    let choisie = PRIORITAIRES
        .iter()
        .find_map(|p| notes.iter().filter(valide).find(|n| n.source == *p))
        .or_else(|| notes.iter().rfind(valide));
    choisie
        .map(|n| (n.valeur * 10.0 / n.maximum as f32, n.votes))
        .unwrap_or((0.0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonction_deduite_du_role() {
        use FonctionPersonne::*;
        assert_eq!(
            FonctionPersonne::depuis_role("Director of Photography"),
            Camera
        );
        assert_eq!(
            FonctionPersonne::depuis_role("Original Music Composer"),
            Compositeur
        );
        assert_eq!(FonctionPersonne::depuis_role("Additional Editor"), Monteur);
        assert_eq!(
            FonctionPersonne::depuis_role("Executive Producer"),
            Producteur
        );
        assert_eq!(FonctionPersonne::depuis_role("Writer"), Scenariste);
        assert_eq!(FonctionPersonne::depuis_role("Director"), Realisateur);
        assert_eq!(FonctionPersonne::depuis_role("Cascadeur"), Autre);
        assert_eq!(FonctionPersonne::depuis_role(""), Autre);
    }

    #[test]
    fn rapports_d_aspect() {
        assert_eq!(libelle_ratio(1.78), "16:9 (1.78:1)");
        assert_eq!(libelle_ratio(1.777_777), "16:9 (1.78:1)");
        assert_eq!(libelle_ratio(0.0), "");
        assert_eq!(libelle_ratio(1.5), "1.50:1");
        assert_eq!(ratio_standard_proche(3840.0 / 2160.0), 1.78);
        let m = MediaEdit {
            largeur: 3840,
            hauteur: 1600,
            ..MediaEdit::default()
        };
        assert_eq!(m.ratio_detecte(), Some(2.40));
        assert_eq!(MediaEdit::default().ratio_detecte(), None);
    }

    #[test]
    fn duree_en_minutes_sans_perte() {
        let mut m = MediaEdit {
            duree_secondes: 10_327,
            ..MediaEdit::default()
        };
        assert_eq!(m.duree_minutes(), 173);
        m.definir_duree_minutes(173);
        assert_eq!(m.duree_secondes, 10_327);
        m.definir_duree_minutes(100);
        assert_eq!(m.duree_secondes, 6000);
    }

    #[test]
    fn note_principale_par_priorite() {
        let n = |s: &str, v: f32, m: u32, votes: u32| NoteSource {
            source: s.into(),
            valeur: v,
            maximum: m,
            votes,
        };
        let notes = vec![n("imdb", 7.0, 10, 5), n("tmdb", 8.0, 10, 4801)];
        assert_eq!(note_principale(&notes), (8.0, 4801));
        assert_eq!(note_principale(&[n("autre", 50.0, 100, 3)]), (5.0, 3));
        assert_eq!(note_principale(&[]), (0.0, 0));
    }
}
