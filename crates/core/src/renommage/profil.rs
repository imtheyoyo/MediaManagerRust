//! Profils de renommage (films et séries) et noms des fichiers associés.
//!
//! Les noms d'artwork, de NFO et de bandes-annonces sont décrits par des
//! **motifs** sans extension : `<b>` est remplacé par le nouveau nom de base
//! de la vidéo (sans marqueur de pile) et `<n>` par l'indice d'un extrafanart.
//! Exemples : `<b>-poster`, `poster`, `folder`, `extrafanart/fanart<n>`,
//! `trailers/<b>-trailer`. Un motif contenant `/` place le fichier dans un
//! sous-dossier du dossier du film.

use serde::{Deserialize, Serialize};

use mediamanager_gabarit::{OptionsDestination, OptionsRendu, Plateforme};

use super::langues::StyleLangue;

/// Marqueur du nom de base dans un motif de nom.
pub const MARQUE_BASE: &str = "<b>";
/// Marqueur de l'indice d'un extrafanart.
pub const MARQUE_INDICE: &str = "<n>";

/// Nom du profil obligatoire.
pub const NOM_PROFIL_DEFAUT: &str = "Default";

/// Gabarit de dossier de film par défaut.
pub const GABARIT_DOSSIER_FILM: &str = "${title}${ - ,edition,} (${year})";
/// Gabarit de fichier de film par défaut.
pub const GABARIT_FICHIER_FILM: &str =
    "${title}${ - ,edition,} (${year}) ${videoFormat} ${audioCodec}";
/// Gabarit de dossier de série par défaut.
pub const GABARIT_DOSSIER_SERIE: &str = "${showTitle} (${showYear})";
/// Gabarit de dossier de saison par défaut.
pub const GABARIT_DOSSIER_SAISON: &str = "Season ${seasonNr}";
/// Gabarit de fichier d'épisode par défaut.
pub const GABARIT_FICHIER_EPISODE: &str = "${showTitle} - S${seasonNr2}E${episodeNr2} - ${title}";

/// Motifs « indésirables » supprimés par l'option de nettoyage (regex
/// appliquées au nom de fichier, insensibles à la casse).
pub fn motifs_indesirables_defaut() -> Vec<String> {
    crate::scan::extensions::MOTIFS_NETTOYAGE
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

fn v(l: &[&str]) -> Vec<String> {
    l.iter().map(|s| (*s).to_string()).collect()
}

fn plateforme_courante() -> Plateforme {
    Plateforme::courante()
}

/// Noms (motifs) des fichiers associés à un film.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NomsFichiersFilm {
    pub poster: Vec<String>,
    pub fanart: Vec<String>,
    pub extrafanart: Vec<String>,
    pub banner: Vec<String>,
    pub clearart: Vec<String>,
    pub clearlogo: Vec<String>,
    /// Vignette (« landscape »).
    pub thumb: Vec<String>,
    pub disc: Vec<String>,
    pub keyart: Vec<String>,
    pub squareart: Vec<String>,
    pub trailer: Vec<String>,
    /// Motifs des NFO (extension `.nfo` ajoutée).
    pub nfo: Vec<String>,
}

impl Default for NomsFichiersFilm {
    fn default() -> Self {
        NomsFichiersFilm {
            poster: v(&["<b>-poster"]),
            fanart: v(&["<b>-fanart"]),
            extrafanart: v(&["<b>-fanart<n>"]),
            banner: v(&["<b>-banner"]),
            clearart: v(&["<b>-clearart"]),
            clearlogo: v(&["<b>-clearlogo"]),
            thumb: v(&["<b>-landscape"]),
            disc: v(&["<b>-discart"]),
            keyart: v(&["<b>-keyart"]),
            squareart: v(&["<b>-squareart"]),
            trailer: v(&["<b>-trailer"]),
            nfo: v(&["<b>"]),
        }
    }
}

/// Profil de renommage des films.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfilRenommageFilm {
    pub nom: String,
    /// Renommer / déplacer le dossier du film.
    pub dossier_actif: bool,
    /// Gabarit du dossier (peut contenir `/` ⇒ sous-dossiers).
    pub gabarit_dossier: String,
    /// Renommer les fichiers.
    pub fichier_actif: bool,
    /// Gabarit du nom de fichier (sans extension).
    pub gabarit_fichier: String,
    pub remplacer_espaces_dossier: bool,
    pub remplacement_espaces_dossier: String,
    /// Sert aussi de séparateur du marqueur de pile.
    pub remplacer_espaces_fichier: bool,
    pub remplacement_espaces_fichier: String,
    pub remplacement_deux_points: String,
    /// Supprimer les NFO non retenus après renommage.
    pub nettoyage_nfo: bool,
    /// Supprimer les fichiers indésirables (voir `motifs_indesirables`).
    pub nettoyer_indesirables: bool,
    pub motifs_indesirables: Vec<String>,
    /// Exposer `movieSet` même si la saga ne compte qu'un film.
    pub saga_pour_film_seul: bool,
    /// Résultat de `;first` pour un chiffre ou un symbole.
    pub remplacement_premier_caractere: String,
    /// Translittération ASCII.
    pub ascii: bool,
    /// Caractères interdits remplacés par des équivalents Unicode.
    pub unicode: bool,
    /// Autoriser la fusion dans un dossier existant non vide.
    pub autoriser_multi_films_meme_dossier: bool,
    /// Noms des fichiers associés.
    pub noms: NomsFichiersFilm,
    /// Écriture du code de langue des sous-titres.
    pub style_langue_sous_titres: StyleLangue,
    /// Articles déplacés par `titleSortable`.
    pub prefixes_titre: Vec<String>,
    /// Plateforme cible de l'assainissement (non persistée).
    #[serde(skip, default = "plateforme_courante")]
    pub plateforme: Plateforme,
}

impl Default for ProfilRenommageFilm {
    fn default() -> Self {
        ProfilRenommageFilm {
            nom: NOM_PROFIL_DEFAUT.to_string(),
            dossier_actif: true,
            gabarit_dossier: GABARIT_DOSSIER_FILM.to_string(),
            fichier_actif: true,
            gabarit_fichier: GABARIT_FICHIER_FILM.to_string(),
            remplacer_espaces_dossier: false,
            remplacement_espaces_dossier: "_".to_string(),
            remplacer_espaces_fichier: false,
            remplacement_espaces_fichier: "_".to_string(),
            remplacement_deux_points: "-".to_string(),
            nettoyage_nfo: false,
            nettoyer_indesirables: false,
            motifs_indesirables: motifs_indesirables_defaut(),
            saga_pour_film_seul: false,
            remplacement_premier_caractere: "#".to_string(),
            ascii: false,
            unicode: false,
            autoriser_multi_films_meme_dossier: false,
            noms: NomsFichiersFilm::default(),
            style_langue_sous_titres: StyleLangue::Iso3T,
            prefixes_titre: crate::scan::titre::prefixes_titre_defaut(),
            plateforme: Plateforme::courante(),
        }
    }
}

impl ProfilRenommageFilm {
    /// Options du moteur de gabarits correspondant au profil.
    pub fn options_destination(&self) -> OptionsDestination {
        options_destination(
            &self.remplacement_deux_points,
            self.remplacer_espaces_dossier
                .then(|| self.remplacement_espaces_dossier.clone()),
            self.remplacer_espaces_fichier
                .then(|| self.remplacement_espaces_fichier.clone()),
            self.ascii,
            self.unicode,
            self.plateforme,
            &self.remplacement_premier_caractere,
        )
    }

    /// Séparateur placé devant le marqueur de pile.
    pub fn separateur_pile(&self) -> String {
        if self.remplacer_espaces_fichier {
            self.remplacement_espaces_fichier.clone()
        } else {
            " ".to_string()
        }
    }
}

/// Noms (motifs) des fichiers associés à un épisode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NomsFichiersEpisode {
    /// Vignettes d'épisode (`<b>-thumb`, `<b>`…).
    pub vignette: Vec<String>,
    pub fanart: Vec<String>,
    pub nfo: Vec<String>,
}

impl Default for NomsFichiersEpisode {
    fn default() -> Self {
        NomsFichiersEpisode {
            vignette: v(&["<b>-thumb"]),
            fanart: v(&["<b>-fanart"]),
            nfo: v(&["<b>"]),
        }
    }
}

/// Style multi-épisodes (sérialisable).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleMulti {
    /// `S01E01 S01E02`
    #[default]
    Repeter,
    /// `S01E01-E02`
    Plage,
}

impl From<StyleMulti> for mediamanager_gabarit::StyleMultiEpisodes {
    fn from(s: StyleMulti) -> Self {
        match s {
            StyleMulti::Repeter => mediamanager_gabarit::StyleMultiEpisodes::Repeat,
            StyleMulti::Plage => mediamanager_gabarit::StyleMultiEpisodes::Range,
        }
    }
}

impl From<crate::modele::StyleMultiEpisodes> for StyleMulti {
    fn from(s: crate::modele::StyleMultiEpisodes) -> Self {
        match s {
            crate::modele::StyleMultiEpisodes::Repeter => StyleMulti::Repeter,
            crate::modele::StyleMultiEpisodes::Plage => StyleMulti::Plage,
        }
    }
}

/// Profil de renommage des séries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfilRenommageSerie {
    pub nom: String,
    pub dossier_serie_actif: bool,
    pub gabarit_dossier_serie: String,
    pub dossier_saison_actif: bool,
    pub gabarit_dossier_saison: String,
    pub fichier_actif: bool,
    pub gabarit_fichier: String,
    pub style_multi_episodes: StyleMulti,
    pub remplacer_espaces_serie: bool,
    pub remplacement_espaces_serie: String,
    pub remplacer_espaces_saison: bool,
    pub remplacement_espaces_saison: String,
    pub remplacer_espaces_fichier: bool,
    pub remplacement_espaces_fichier: String,
    /// Vide par défaut pour les séries.
    pub remplacement_deux_points: String,
    pub nettoyer_indesirables: bool,
    pub motifs_indesirables: Vec<String>,
    pub remplacement_premier_caractere: String,
    pub ascii: bool,
    pub unicode: bool,
    /// Saison 0 ⇒ dossier « Specials ».
    pub saison_speciale: bool,
    pub noms: NomsFichiersEpisode,
    pub style_langue_sous_titres: StyleLangue,
    pub prefixes_titre: Vec<String>,
    #[serde(skip, default = "plateforme_courante")]
    pub plateforme: Plateforme,
}

impl Default for ProfilRenommageSerie {
    fn default() -> Self {
        ProfilRenommageSerie {
            nom: NOM_PROFIL_DEFAUT.to_string(),
            dossier_serie_actif: true,
            gabarit_dossier_serie: GABARIT_DOSSIER_SERIE.to_string(),
            dossier_saison_actif: true,
            gabarit_dossier_saison: GABARIT_DOSSIER_SAISON.to_string(),
            fichier_actif: true,
            gabarit_fichier: GABARIT_FICHIER_EPISODE.to_string(),
            style_multi_episodes: StyleMulti::Repeter,
            remplacer_espaces_serie: false,
            remplacement_espaces_serie: "_".to_string(),
            remplacer_espaces_saison: false,
            remplacement_espaces_saison: "_".to_string(),
            remplacer_espaces_fichier: false,
            remplacement_espaces_fichier: "_".to_string(),
            remplacement_deux_points: String::new(),
            nettoyer_indesirables: false,
            motifs_indesirables: motifs_indesirables_defaut(),
            remplacement_premier_caractere: "#".to_string(),
            ascii: false,
            unicode: false,
            saison_speciale: true,
            noms: NomsFichiersEpisode::default(),
            style_langue_sous_titres: StyleLangue::Iso3T,
            prefixes_titre: crate::scan::titre::prefixes_titre_defaut(),
            plateforme: Plateforme::courante(),
        }
    }
}

impl ProfilRenommageSerie {
    /// Options pour le dossier de série et les fichiers d'épisode.
    pub fn options_destination(&self) -> OptionsDestination {
        options_destination(
            &self.remplacement_deux_points,
            self.remplacer_espaces_serie
                .then(|| self.remplacement_espaces_serie.clone()),
            self.remplacer_espaces_fichier
                .then(|| self.remplacement_espaces_fichier.clone()),
            self.ascii,
            self.unicode,
            self.plateforme,
            &self.remplacement_premier_caractere,
        )
    }

    /// Options pour le dossier de saison.
    pub fn options_destination_saison(&self) -> OptionsDestination {
        let mut o = self.options_destination();
        o.remplacement_espaces_dossier = self
            .remplacer_espaces_saison
            .then(|| self.remplacement_espaces_saison.clone());
        o
    }

    /// Séparateur placé devant le marqueur de pile d'un épisode.
    pub fn separateur_pile(&self) -> String {
        if self.remplacer_espaces_fichier {
            self.remplacement_espaces_fichier.clone()
        } else {
            ".".to_string()
        }
    }
}

fn options_destination(
    deux_points: &str,
    espaces_dossier: Option<String>,
    espaces_fichier: Option<String>,
    ascii: bool,
    unicode: bool,
    plateforme: Plateforme,
    premier: &str,
) -> OptionsDestination {
    OptionsDestination {
        remplacement_deux_points: deux_points.to_string(),
        remplacement_espaces_dossier: espaces_dossier,
        remplacement_espaces_fichier: espaces_fichier,
        ascii,
        unicode,
        plateforme,
        rendu: OptionsRendu {
            remplacement_premier_caractere: premier.to_string(),
            ..OptionsRendu::default()
        },
    }
}

/// Profil nommé (films ou séries).
pub trait ProfilNomme: Default + Clone {
    fn nom(&self) -> &str;
}

impl ProfilNomme for ProfilRenommageFilm {
    fn nom(&self) -> &str {
        &self.nom
    }
}

impl ProfilNomme for ProfilRenommageSerie {
    fn nom(&self) -> &str {
        &self.nom
    }
}

/// Ensemble de profils ; le profil « Default » existe toujours.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfilsRenommage<P> {
    profils: Vec<P>,
}

impl<P: ProfilNomme> Default for ProfilsRenommage<P> {
    fn default() -> Self {
        ProfilsRenommage {
            profils: vec![P::default()],
        }
    }
}

impl<P: ProfilNomme> ProfilsRenommage<P> {
    /// Ensemble contenant seulement le profil par défaut.
    pub fn nouveau() -> Self {
        Self::default()
    }

    /// Ajoute ou remplace un profil (par nom, sensible à la casse).
    pub fn definir(&mut self, profil: P) {
        match self.profils.iter_mut().find(|p| p.nom() == profil.nom()) {
            Some(p) => *p = profil,
            None => self.profils.push(profil),
        }
        self.garantir_defaut();
    }

    /// Supprime un profil ; le profil par défaut ne peut pas être supprimé.
    pub fn supprimer(&mut self, nom: &str) -> bool {
        if nom == NOM_PROFIL_DEFAUT {
            return false;
        }
        let avant = self.profils.len();
        self.profils.retain(|p| p.nom() != nom);
        avant != self.profils.len()
    }

    /// Profil demandé, sinon le profil par défaut.
    pub fn obtenir(&self, nom: &str) -> P {
        self.profils
            .iter()
            .find(|p| p.nom() == nom)
            .or_else(|| self.profils.iter().find(|p| p.nom() == NOM_PROFIL_DEFAUT))
            .cloned()
            .unwrap_or_default()
    }

    /// Noms des profils, dans l'ordre d'insertion.
    pub fn noms(&self) -> Vec<String> {
        self.profils.iter().map(|p| p.nom().to_string()).collect()
    }

    fn garantir_defaut(&mut self) {
        if !self.profils.iter().any(|p| p.nom() == NOM_PROFIL_DEFAUT) {
            self.profils.insert(0, P::default());
        }
    }
}
