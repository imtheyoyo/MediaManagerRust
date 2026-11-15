//! Modèles de renommage : champs, variables disponibles, insertion d'une variable à la
//! position du curseur et aperçu sur un exemple (film ou série de démonstration, ou film
//! sélectionné). Logique pure, sans egui : la page des réglages ne fait que l'afficher.

use crate::conversion;
use crate::modele::{Film as FilmAffiche, normaliser};
use crate::source_donnees::ReglagesServices;
use mediamanager_app::ConfigArtwork;
use mediamanager_app::renommage::profil_renommage_film;
use mediamanager_app::renommage_serie::profil_renommage_serie;
use mediamanager_core::modele::fichier::{FluxAudio, FluxVideo};
use mediamanager_core::modele::{
    Episode, FichierMedia, Film, GroupeEpisodes, NumeroEpisode, Serie, TypeFichier,
};
use mediamanager_core::renommage::profil::{
    GABARIT_DOSSIER_FILM, GABARIT_DOSSIER_SAISON, GABARIT_DOSSIER_SERIE, GABARIT_FICHIER_EPISODE,
    GABARIT_FICHIER_FILM,
};
use mediamanager_core::renommage::{
    Conflit, JETONS_FILM, JETONS_SERIE, nom_dossier_film, nom_dossier_saison, nom_dossier_serie,
    nom_fichier_episodes, nom_fichier_film,
};
use mediamanager_store::Reglages;

/// Champ de modèle de la page du renommeur.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChampGabarit {
    DossierFilm,
    FichierFilm,
    DossierSerie,
    DossierSaison,
    FichierEpisode,
}

impl ChampGabarit {
    pub const FILMS: [ChampGabarit; 2] = [ChampGabarit::DossierFilm, ChampGabarit::FichierFilm];
    pub const SERIES: [ChampGabarit; 3] = [
        ChampGabarit::DossierSerie,
        ChampGabarit::DossierSaison,
        ChampGabarit::FichierEpisode,
    ];

    /// Clé de traduction du libellé du champ (la clé `.tip` donne son infobulle).
    pub fn cle(self) -> &'static str {
        match self {
            ChampGabarit::DossierFilm | ChampGabarit::DossierSerie => "Settings.renamer.folder",
            ChampGabarit::FichierFilm | ChampGabarit::FichierEpisode => "Settings.renamer.file",
            ChampGabarit::DossierSaison => "gui.renamer.seasonfolder",
        }
    }

    /// Clé du résultat dans l'aperçu.
    pub fn cle_apercu(self) -> &'static str {
        match self {
            ChampGabarit::DossierFilm | ChampGabarit::DossierSerie => {
                "gui.renamer.preview.folderresult"
            }
            ChampGabarit::FichierFilm => "gui.renamer.preview.fileresult",
            ChampGabarit::DossierSaison => "gui.renamer.preview.seasonresult",
            ChampGabarit::FichierEpisode => "gui.renamer.preview.episoderesult",
        }
    }

    pub fn est_film(self) -> bool {
        Self::FILMS.contains(&self)
    }

    pub fn valeur(self, s: &ReglagesServices) -> &str {
        match self {
            ChampGabarit::DossierFilm => &s.gabarit_dossier_film,
            ChampGabarit::FichierFilm => &s.gabarit_fichier_film,
            ChampGabarit::DossierSerie => &s.gabarit_dossier_serie,
            ChampGabarit::DossierSaison => &s.gabarit_dossier_saison,
            ChampGabarit::FichierEpisode => &s.gabarit_fichier_episode,
        }
    }

    pub fn valeur_mut(self, s: &mut ReglagesServices) -> &mut String {
        match self {
            ChampGabarit::DossierFilm => &mut s.gabarit_dossier_film,
            ChampGabarit::FichierFilm => &mut s.gabarit_fichier_film,
            ChampGabarit::DossierSerie => &mut s.gabarit_dossier_serie,
            ChampGabarit::DossierSaison => &mut s.gabarit_dossier_saison,
            ChampGabarit::FichierEpisode => &mut s.gabarit_fichier_episode,
        }
    }

    /// Modèle proposé par défaut.
    pub fn defaut(self) -> &'static str {
        match self {
            ChampGabarit::DossierFilm => GABARIT_DOSSIER_FILM,
            ChampGabarit::FichierFilm => GABARIT_FICHIER_FILM,
            ChampGabarit::DossierSerie => GABARIT_DOSSIER_SERIE,
            ChampGabarit::DossierSaison => GABARIT_DOSSIER_SAISON,
            ChampGabarit::FichierEpisode => GABARIT_FICHIER_EPISODE,
        }
    }

    /// Variables utilisables dans ce champ.
    pub fn variables(self) -> Vec<Variable> {
        if self.est_film() {
            variables(JETONS_FILM)
        } else {
            variables(JETONS_SERIE)
        }
    }
}

/// Rétablit les modèles par défaut des champs donnés.
pub fn retablir_defauts(s: &mut ReglagesServices, champs: &[ChampGabarit]) {
    for c in champs {
        *c.valeur_mut(s) = c.defaut().to_string();
    }
}

/// Variable de modèle : son nom (`title`) et l'expression du modèle de données qu'elle
/// désigne (`movie.title`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Variable {
    pub nom: &'static str,
    pub expression: &'static str,
}

impl Variable {
    /// Texte inséré dans un modèle : `${nom}`.
    pub fn jeton(&self) -> String {
        format!("${{{}}}", self.nom)
    }
}

fn variables(table: &'static [(&'static str, &'static str)]) -> Vec<Variable> {
    let mut v: Vec<Variable> = Vec::with_capacity(table.len());
    for (nom, expression) in table {
        if !v.iter().any(|x| x.nom == *nom) {
            v.push(Variable { nom, expression });
        }
    }
    v
}

/// Variables dont le nom ou l'expression contient `motif` (sans tenir compte de la casse
/// ni des accents) ; toutes si le motif est vide.
pub fn filtrer<'a>(toutes: &'a [Variable], motif: &str) -> Vec<&'a Variable> {
    let m = normaliser(motif.trim());
    toutes
        .iter()
        .filter(|v| {
            m.is_empty() || normaliser(v.nom).contains(&m) || normaliser(v.expression).contains(&m)
        })
        .collect()
}

/// Insère `${variable}` dans `texte` à la place de la sélection `[a ; b[` (indices en
/// caractères, dans n'importe quel ordre ; hors limites = fin du texte). Renvoie le nouveau
/// texte et la position du curseur juste après la variable insérée.
pub fn inserer_variable(texte: &str, a: usize, b: usize, nom: &str) -> (String, usize) {
    let chars: Vec<char> = texte.chars().collect();
    let (debut, fin) = (a.min(b).min(chars.len()), a.max(b).min(chars.len()));
    let jeton = format!("${{{nom}}}");
    let mut sortie: String = chars[..debut].iter().collect();
    sortie.push_str(&jeton);
    sortie.extend(chars[fin..].iter());
    (sortie, debut + jeton.chars().count())
}

/// Exemples sur lesquels les modèles sont évalués.
#[derive(Clone, Debug)]
pub struct Exemples {
    pub film: Film,
    /// Le film est celui de la sélection (sinon : exemple de démonstration).
    pub film_reel: bool,
    pub serie: Serie,
}

impl Default for Exemples {
    fn default() -> Self {
        Self {
            film: film_demonstration(),
            film_reel: false,
            serie: serie_demonstration(),
        }
    }
}

impl Exemples {
    /// Exemples construits sur le film sélectionné, s'il y en a un.
    pub fn avec_film(film: Option<&FilmAffiche>) -> Self {
        match film {
            Some(f) if !f.titre.trim().is_empty() => Self {
                film: film_depuis(f),
                film_reel: true,
                ..Self::default()
            },
            _ => Self::default(),
        }
    }

    pub fn titre_film(&self) -> &str {
        &self.film.titre
    }
}

/// Film de démonstration.
pub fn film_demonstration() -> Film {
    let mut f = Film {
        id: "demo".into(),
        titre: "Le Parrain".into(),
        titre_original: "The Godfather".into(),
        titre_anglais: "The Godfather".into(),
        annee: 1972,
        certification: "PG-13".into(),
        genres: vec!["Drame".into(), "Crime".into()],
        ..Film::default()
    };
    f.fichiers.push(fichier_video(
        "The.Godfather.1972.1080p.BluRay.mkv",
        1080,
        "h264",
        "ac3",
        6,
    ));
    f
}

fn fichier_video(
    nom: &str,
    hauteur: u32,
    codec_video: &str,
    codec_audio: &str,
    canaux: u32,
) -> FichierMedia {
    FichierMedia {
        type_fichier: TypeFichier::Video,
        nom: nom.into(),
        video: FluxVideo {
            codec: codec_video.into(),
            largeur: hauteur * 16 / 9,
            hauteur,
            ..FluxVideo::default()
        },
        flux_audio: vec![FluxAudio {
            codec: codec_audio.into(),
            canaux,
            ..FluxAudio::default()
        }],
        ..FichierMedia::default()
    }
}

/// Film du modèle des services construit depuis un film affiché (titre, année, flux).
pub fn film_depuis(f: &FilmAffiche) -> Film {
    let mut g = Film {
        id: f.id.to_string(),
        titre: f.titre.clone(),
        titre_original: f.titre_original.clone(),
        annee: f.annee,
        certification: f.certification.clone(),
        genres: f.genres.clone(),
        ..Film::default()
    };
    let mut mots = f.audio.split_whitespace();
    let codec_audio = mots.next().unwrap_or("");
    let canaux: u32 = mots
        .next()
        .map(|m| m.chars().filter(char::is_ascii_digit).collect::<String>())
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    let nom = if f.nom_fichier.is_empty() {
        format!("{}.mkv", f.titre)
    } else {
        f.nom_fichier.clone()
    };
    g.fichiers.push(fichier_video(
        &nom,
        f.hauteur_video,
        &f.codec_video,
        codec_audio,
        canaux,
    ));
    g
}

/// Série de démonstration : un épisode (saison 1, épisode 2).
pub fn serie_demonstration() -> Serie {
    let diffusion = GroupeEpisodes::diffusion();
    let mut ep = Episode {
        id: "demo-s01e02".into(),
        id_serie: "demo".into(),
        numeros: vec![NumeroEpisode {
            groupe: diffusion.clone(),
            saison: 1,
            episode: 2,
        }],
        titre: "Les Chats dans le sac".into(),
        titre_original: "Cat's in the Bag...".into(),
        premiere_diffusion: Some("2008-01-27".into()),
        nom_fichier_origine: "Breaking.Bad.S01E02.720p.mkv".into(),
        ..Episode::default()
    };
    ep.fichiers.push(fichier_video(
        "Breaking.Bad.S01E02.720p.mkv",
        720,
        "h264",
        "ac3",
        6,
    ));
    Serie {
        id: "demo".into(),
        titre: "Breaking Bad".into(),
        titre_original: "Breaking Bad".into(),
        annee: 2008,
        groupe_actif: diffusion,
        episodes: vec![ep],
        ..Serie::default()
    }
}

/// Résultat de l'évaluation d'un modèle : `Ok(None)` si le modèle est vide (élément non
/// renommé), `Ok(Some(nom))`, ou `Err(message)` si le modèle est invalide.
pub type ResultatApercu = Result<Option<String>, String>;

fn message(c: Conflit) -> String {
    match c {
        Conflit::GabaritInvalide { message, .. } => message,
        autre => autre.to_string(),
    }
}

/// Réglages du stockage équivalents aux réglages affichés (pour réutiliser les profils de
/// renommage des services : l'aperçu et le renommage réel partagent le même calcul).
fn reglages_stockage(s: &ReglagesServices) -> Reglages {
    let mut r = Reglages::default();
    conversion::appliquer_services(s, &mut r);
    r
}

/// Évalue le modèle d'un champ sur l'exemple.
pub fn apercu(champ: ChampGabarit, s: &ReglagesServices, ex: &Exemples) -> ResultatApercu {
    if champ.valeur(s).trim().is_empty() {
        return Ok(None);
    }
    let r = reglages_stockage(s);
    let resultat = match champ {
        ChampGabarit::DossierFilm | ChampGabarit::FichierFilm => {
            let profil = profil_renommage_film(&r, &ConfigArtwork::default());
            if champ == ChampGabarit::DossierFilm {
                nom_dossier_film(&ex.film, None, &profil)
            } else {
                nom_fichier_film(&ex.film, None, &profil)
            }
        }
        ChampGabarit::DossierSerie => nom_dossier_serie(&ex.serie, &profil_renommage_serie(&r)),
        ChampGabarit::DossierSaison | ChampGabarit::FichierEpisode => {
            let profil = profil_renommage_serie(&r);
            let Some(ep) = ex.serie.episodes.first() else {
                return Ok(None);
            };
            if champ == ChampGabarit::DossierSaison {
                nom_dossier_saison(&ex.serie, ep, &profil)
            } else {
                nom_fichier_episodes(&ex.serie, &[ep], &profil)
            }
        }
    };
    resultat.map(Some).map_err(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Réglages affichés correspondant aux valeurs par défaut du stockage.
    fn reglages() -> ReglagesServices {
        conversion::services(&Reglages::default())
    }

    #[test]
    fn insertion_au_curseur() {
        assert_eq!(inserer_variable("", 0, 0, "title"), ("${title}".into(), 8));
        assert_eq!(
            inserer_variable("ab (cd)", 3, 3, "year"),
            ("ab ${year}(cd)".into(), 3 + 7)
        );
        // Fin de texte, curseur hors limites.
        assert_eq!(inserer_variable("ab", 99, 99, "x"), ("ab${x}".into(), 6));
    }

    #[test]
    fn insertion_remplace_la_selection_dans_les_deux_sens() {
        assert_eq!(
            inserer_variable("un deux trois", 3, 7, "title"),
            ("un ${title} trois".into(), 3 + 8)
        );
        assert_eq!(
            inserer_variable("un deux trois", 7, 3, "title"),
            ("un ${title} trois".into(), 3 + 8)
        );
    }

    #[test]
    fn insertion_compte_en_caracteres_pas_en_octets() {
        // « é » et « ï » sont codés sur deux octets : les indices sont des caractères.
        let (t, c) = inserer_variable("éïa", 2, 2, "year");
        assert_eq!(t, "éï${year}a");
        assert_eq!(c, 2 + 7);
    }

    #[test]
    fn variables_des_films_et_des_series() {
        let films = ChampGabarit::DossierFilm.variables();
        let series = ChampGabarit::FichierEpisode.variables();
        assert!(films.iter().any(|v| v.nom == "title"));
        assert!(films.iter().any(|v| v.nom == "year"));
        assert!(series.iter().any(|v| v.nom == "showTitle"));
        assert!(series.iter().any(|v| v.nom == "seasonNr2"));
        for liste in [&films, &series] {
            let mut noms: Vec<&str> = liste.iter().map(|v| v.nom).collect();
            noms.sort_unstable();
            let n = noms.len();
            noms.dedup();
            assert_eq!(n, noms.len(), "variables en double");
        }
        assert_eq!(films[0].jeton(), format!("${{{}}}", films[0].nom));
    }

    #[test]
    fn filtre_des_variables() {
        let v = ChampGabarit::DossierFilm.variables();
        assert_eq!(filtrer(&v, "").len(), v.len());
        let r = filtrer(&v, "  TITLE ");
        assert!(r.iter().any(|x| x.nom == "title"));
        assert!(r.iter().all(|x| {
            x.nom.to_lowercase().contains("title") || x.expression.to_lowercase().contains("title")
        }));
        assert!(filtrer(&v, "zzzzinexistant").is_empty());
    }

    #[test]
    fn apercu_des_modeles_par_defaut() {
        let s = reglages();
        let ex = Exemples::default();
        let dossier = apercu(ChampGabarit::DossierFilm, &s, &ex).unwrap().unwrap();
        assert_eq!(dossier, "Le Parrain (1972)");
        let fichier = apercu(ChampGabarit::FichierFilm, &s, &ex).unwrap().unwrap();
        assert!(fichier.starts_with("Le Parrain (1972)"), "{fichier}");
        let serie = apercu(ChampGabarit::DossierSerie, &s, &ex)
            .unwrap()
            .unwrap();
        assert_eq!(serie, "Breaking Bad (2008)");
        let episode = apercu(ChampGabarit::FichierEpisode, &s, &ex)
            .unwrap()
            .unwrap();
        assert_eq!(episode, "Breaking Bad - S01E02 - Les Chats dans le sac");
        let saison = apercu(ChampGabarit::DossierSaison, &s, &ex)
            .unwrap()
            .unwrap();
        assert!(saison.contains('1'), "{saison}");
    }

    #[test]
    fn apercu_suit_la_modification_d_un_modele() {
        let mut s = reglages();
        let ex = Exemples::default();
        s.gabarit_dossier_film.clear();
        s.gabarit_dossier_film.push_str("${title} [${year}]");
        assert_eq!(
            apercu(ChampGabarit::DossierFilm, &s, &ex),
            Ok(Some("Le Parrain [1972]".into()))
        );
        // Variable insérée à la fin par la logique d'insertion.
        let n = s.gabarit_dossier_film.chars().count();
        let (t, _) = inserer_variable(&s.gabarit_dossier_film, n, n, "originalTitle");
        s.gabarit_dossier_film = t;
        assert_eq!(
            apercu(ChampGabarit::DossierFilm, &s, &ex),
            Ok(Some("Le Parrain [1972]The Godfather".into()))
        );
    }

    #[test]
    fn modele_vide_ou_invalide() {
        let mut s = reglages();
        let ex = Exemples::default();
        s.gabarit_fichier_film = "  ".into();
        assert_eq!(apercu(ChampGabarit::FichierFilm, &s, &ex), Ok(None));
        s.gabarit_fichier_film = "${title".into();
        let r = apercu(ChampGabarit::FichierFilm, &s, &ex);
        assert!(matches!(&r, Err(m) if !m.is_empty()), "{r:?}");
    }

    #[test]
    fn exemple_d_apres_le_film_selectionne() {
        let mut f = FilmAffiche {
            titre: "Alien".into(),
            annee: 1979,
            hauteur_video: 2160,
            codec_video: "hevc".into(),
            audio: "DTS 6ch".into(),
            ..FilmAffiche::default()
        };
        f.nom_fichier = "alien.mkv".into();
        let ex = Exemples::avec_film(Some(&f));
        assert!(ex.film_reel);
        assert_eq!(ex.titre_film(), "Alien");
        let s = reglages();
        assert_eq!(
            apercu(ChampGabarit::DossierFilm, &s, &ex),
            Ok(Some("Alien (1979)".into()))
        );
        assert!(!Exemples::avec_film(None).film_reel);
    }

    #[test]
    fn retablir_les_defauts() {
        let mut s = reglages();
        s.gabarit_dossier_film = "x".into();
        s.gabarit_dossier_serie = "y".into();
        retablir_defauts(&mut s, &ChampGabarit::FILMS);
        assert_eq!(s.gabarit_dossier_film, GABARIT_DOSSIER_FILM);
        assert_eq!(s.gabarit_dossier_serie, "y");
    }
}
