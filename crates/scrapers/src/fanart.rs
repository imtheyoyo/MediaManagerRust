//! Fournisseur Fanart.tv (images de films et de séries uniquement).
//!
//! La clé de projet (`api-key`) est fournie par l'utilisateur ; la clé
//! personnelle (`client-key`) est facultative. Les deux sont transmises par
//! en-tête et ne sont jamais mises en cache ni journalisées.

use async_trait::async_trait;
use serde::{Deserialize, Deserializer};

use crate::correspondance::est_id_imdb;
use crate::erreur::{ErreurScraper, Resultat};
use crate::fournisseur::{FournisseurArtwork, FournisseurArtworkSerie};
use crate::http::ClientHttp;
use crate::tmdb::ordre_taille;
use crate::types::*;

const URL_API: &str = "https://webservice.fanart.tv/v3.2/";
const ID_FOURNISSEUR: &str = "fanarttv";

/// Configuration du fournisseur Fanart.tv.
#[derive(Clone, Default)]
pub struct ConfigFanart {
    /// Clé de projet fournie par l'utilisateur (obligatoire pour activer le fournisseur).
    pub cle_api: String,
    /// Clé personnelle facultative ("" = absente).
    pub cle_client: String,
    /// Surcharge de l'URL de base (tests, miroir). Se termine par `/`.
    pub url_api: Option<String>,
    /// Inclut aussi les anciens logos basse définition (`clearlogo`).
    pub anciens_logos: bool,
}

impl ConfigFanart {
    /// Configuration avec la clé de l'utilisateur.
    pub fn nouvelle(cle_api: impl Into<String>) -> Self {
        Self {
            cle_api: cle_api.into(),
            ..Self::default()
        }
    }
}

impl std::fmt::Debug for ConfigFanart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let masque = |s: &str| if s.is_empty() { "<vide>" } else { "<masquée>" };
        f.debug_struct("ConfigFanart")
            .field("cle_api", &masque(&self.cle_api))
            .field("cle_client", &masque(&self.cle_client))
            .field("url_api", &self.url_api)
            .field("anciens_logos", &self.anciens_logos)
            .finish()
    }
}

/// Scraper Fanart.tv.
#[derive(Debug)]
pub struct FanartTv {
    config: ConfigFanart,
    http: ClientHttp,
}

// ---------------------------------------------------------------- JSON

/// Entier transmis sous forme de chaîne, de nombre ou absent (0 si illisible).
fn entier_souple<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Number(n) => n.as_u64().unwrap_or(0).min(u32::MAX as u64) as u32,
        serde_json::Value::String(s) => s.trim().parse().unwrap_or(0),
        _ => 0,
    })
}

/// Saison : `"all"` -> 0, entier -> n, sinon `None`.
fn saison_souple<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(match v {
        serde_json::Value::Number(n) => n.as_u64().map(|n| n as u32),
        serde_json::Value::String(s) if s.trim().eq_ignore_ascii_case("all") => Some(0),
        serde_json::Value::String(s) => s.trim().parse().ok(),
        _ => None,
    })
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct ImageFanart {
    url: String,
    lang: String,
    #[serde(deserialize_with = "entier_souple")]
    likes: u32,
    #[serde(deserialize_with = "entier_souple")]
    width: u32,
    #[serde(deserialize_with = "entier_souple")]
    height: u32,
    #[serde(deserialize_with = "saison_souple")]
    season: Option<u32>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ReponseFanart {
    name: String,
    // films
    movieposter: Vec<ImageFanart>,
    moviebackground: Vec<ImageFanart>,
    moviebanner: Vec<ImageFanart>,
    moviethumb: Vec<ImageFanart>,
    moviedisc: Vec<ImageFanart>,
    hdmovielogo: Vec<ImageFanart>,
    movielogo: Vec<ImageFanart>,
    hdmovieclearart: Vec<ImageFanart>,
    moviesquare: Vec<ImageFanart>,
    // séries
    tvposter: Vec<ImageFanart>,
    showbackground: Vec<ImageFanart>,
    tvbanner: Vec<ImageFanart>,
    tvthumb: Vec<ImageFanart>,
    hdtvlogo: Vec<ImageFanart>,
    clearlogo: Vec<ImageFanart>,
    hdclearart: Vec<ImageFanart>,
    tvsquare: Vec<ImageFanart>,
    seasonposter: Vec<ImageFanart>,
    seasonthumb: Vec<ImageFanart>,
    seasonbanner: Vec<ImageFanart>,
    characterart: Vec<ImageFanart>,
}

// ---------------------------------------------------------------- utilitaires

/// Ordre de taille d'une image de type étendu.
pub fn ordre_taille_etendu(t: TypeArtworkEtendu, largeur: u32) -> u32 {
    use TypeArtworkEtendu as T;
    match t {
        T::Affiche | T::AfficheSaison | T::ArtCle => ordre_taille(TypeArtwork::Affiche, largeur),
        T::Fond | T::FondSaison => ordre_taille(TypeArtwork::Fond, largeur),
        T::Vignette | T::VignetteSaison | T::ClearArt => {
            ordre_taille(TypeArtwork::Vignette, largeur)
        }
        T::LogoTransparent => ordre_taille(TypeArtwork::LogoTransparent, largeur),
        T::Banniere | T::BanniereSaison | T::Disque | T::Carre | T::ArtPersonnage => {
            if largeur >= 1000 {
                8
            } else if largeur >= 500 {
                2
            } else {
                1
            }
        }
    }
}

/// Dimensions par défaut d'un type lorsque la réponse n'en fournit pas.
fn dimensions_defaut(t: TypeArtworkEtendu, ancien_logo: bool) -> (u32, u32) {
    use TypeArtworkEtendu as T;
    match t {
        T::Affiche | T::AfficheSaison | T::ArtCle => (1000, 1426),
        T::Fond | T::FondSaison => (1920, 1080),
        T::Banniere | T::BanniereSaison => (1000, 185),
        T::LogoTransparent if ancien_logo => (400, 155),
        T::LogoTransparent => (800, 310),
        T::ClearArt | T::Vignette | T::VignetteSaison => (1000, 562),
        T::Disque | T::Carre => (1000, 1000),
        T::ArtPersonnage => (512, 512),
    }
}

fn convertir(
    images: &[ImageFanart],
    t: TypeArtworkEtendu,
    serie: bool,
    ancien_logo: bool,
) -> Vec<ArtworkEtendu> {
    images
        .iter()
        .filter(|i| !i.url.trim().is_empty())
        .map(|i| {
            let (ld, hd) = dimensions_defaut(t, ancien_logo);
            let (l, h) = if i.width > 0 && i.height > 0 {
                (i.width, i.height)
            } else {
                (ld, hd)
            };
            let langue = match i.lang.trim() {
                "00" | "" => String::new(),
                autre => autre.to_string(),
            };
            let saison = match t {
                TypeArtworkEtendu::AfficheSaison
                | TypeArtworkEtendu::FondSaison
                | TypeArtworkEtendu::BanniereSaison
                | TypeArtworkEtendu::VignetteSaison
                    if serie =>
                {
                    i.season
                }
                _ => None,
            };
            ArtworkEtendu {
                fournisseur: ID_FOURNISSEUR.into(),
                type_artwork: t,
                url_apercu: i.url.replace("/fanart/", "/preview/"),
                url_originale: i.url.clone(),
                langue,
                votes: i.likes,
                saison,
                tailles: vec![TailleImage {
                    largeur: l,
                    hauteur: h,
                    url: i.url.clone(),
                    ordre: ordre_taille_etendu(t, l),
                }],
            }
        })
        .collect()
}

/// Les affiches sans texte (`lang == "00"`).
fn sans_texte(images: &[ImageFanart]) -> Vec<ImageFanart> {
    images
        .iter()
        .filter(|i| i.lang.trim() == "00")
        .cloned()
        .collect()
}

impl FanartTv {
    /// Crée le scraper. Échoue si la clé de projet est vide.
    pub fn new(config: ConfigFanart, http: ClientHttp) -> Resultat<Self> {
        if config.cle_api.trim().is_empty() {
            return Err(ErreurScraper::CleApiManquante);
        }
        Ok(Self { config, http })
    }

    fn base(&self) -> &str {
        self.config.url_api.as_deref().unwrap_or(URL_API)
    }

    /// GET ; un 404 signifie « aucune image » (réponse vide).
    async fn get(&self, chemin: &str) -> Resultat<ReponseFanart> {
        let url = format!("{}{}", self.base(), chemin);
        let mut entetes = vec![("api-key", self.config.cle_api.trim())];
        if !self.config.cle_client.trim().is_empty() {
            entetes.push(("client-key", self.config.cle_client.trim()));
        }
        match self
            .http
            .obtenir_json_avec_entetes::<ReponseFanart>(&url, &[], &entetes)
            .await
        {
            Ok(r) => Ok(r),
            Err(ErreurScraper::Http { statut: 404, .. }) => Ok(ReponseFanart::default()),
            Err(e) => Err(e),
        }
    }

    /// Réponse brute d'un film : essai par IMDb puis par TMDB si vide ou sans nom.
    async fn reponse_film(&self, options: &OptionsRecherche) -> Resultat<ReponseFanart> {
        let imdb = options.id_imdb();
        let tmdb = options.id_tmdb();
        if imdb.is_empty() && tmdb == 0 {
            return Err(ErreurScraper::IdentifiantManquant("imdb, tmdb".into()));
        }
        if est_id_imdb(&imdb) {
            let r = self.get(&format!("movies/{imdb}")).await?;
            if !r.name.trim().is_empty() || tmdb == 0 {
                return Ok(r);
            }
        }
        self.get(&format!("movies/{tmdb}")).await
    }

    /// Toutes les images d'un film (types étendus : bannière, disque, clearart...).
    pub async fn artwork_film_etendu(
        &self,
        options: &OptionsRecherche,
        type_filtre: Option<TypeArtworkEtendu>,
    ) -> Resultat<Vec<ArtworkEtendu>> {
        let r = self.reponse_film(options).await?;
        Ok(self.images_film(&r, type_filtre))
    }

    fn images_film(
        &self,
        r: &ReponseFanart,
        filtre: Option<TypeArtworkEtendu>,
    ) -> Vec<ArtworkEtendu> {
        use TypeArtworkEtendu as T;
        let mut logos = r.hdmovielogo.clone();
        let mut anciens = Vec::new();
        if self.config.anciens_logos {
            anciens = r.movielogo.clone();
        }
        logos.retain(|i| !i.url.is_empty());
        let cle = sans_texte(&r.movieposter);
        let mut sortie = Vec::new();
        let mut ajouter = |images: &[ImageFanart], t: T, ancien: bool| {
            if filtre.is_none_or(|f| f == t) {
                sortie.extend(convertir(images, t, false, ancien));
            }
        };
        ajouter(&r.movieposter, T::Affiche, false);
        ajouter(&cle, T::ArtCle, false);
        ajouter(&r.moviebackground, T::Fond, false);
        ajouter(&r.moviebanner, T::Banniere, false);
        ajouter(&r.moviethumb, T::Vignette, false);
        ajouter(&r.moviedisc, T::Disque, false);
        ajouter(&logos, T::LogoTransparent, false);
        ajouter(&anciens, T::LogoTransparent, true);
        ajouter(&r.hdmovieclearart, T::ClearArt, false);
        ajouter(&r.moviesquare, T::Carre, false);
        sortie
    }

    fn images_serie(
        &self,
        r: &ReponseFanart,
        filtre: Option<TypeArtworkEtendu>,
    ) -> Vec<ArtworkEtendu> {
        use TypeArtworkEtendu as T;
        let cle = sans_texte(&r.tvposter);
        let anciens = if self.config.anciens_logos {
            r.clearlogo.clone()
        } else {
            Vec::new()
        };
        let mut sortie = Vec::new();
        let mut ajouter = |images: &[ImageFanart], t: T, ancien: bool| {
            if filtre.is_none_or(|f| f == t) {
                sortie.extend(convertir(images, t, true, ancien));
            }
        };
        ajouter(&r.tvposter, T::Affiche, false);
        ajouter(&cle, T::ArtCle, false);
        ajouter(&r.showbackground, T::Fond, false);
        ajouter(&r.tvbanner, T::Banniere, false);
        ajouter(&r.tvthumb, T::Vignette, false);
        ajouter(&r.hdtvlogo, T::LogoTransparent, false);
        ajouter(&anciens, T::LogoTransparent, true);
        ajouter(&r.hdclearart, T::ClearArt, false);
        ajouter(&r.tvsquare, T::Carre, false);
        ajouter(&r.seasonposter, T::AfficheSaison, false);
        ajouter(&r.seasonthumb, T::VignetteSaison, false);
        ajouter(&r.seasonbanner, T::BanniereSaison, false);
        ajouter(&r.characterart, T::ArtPersonnage, false);
        sortie
    }
}

// ---------------------------------------------------------------- traits

#[async_trait]
impl FournisseurArtwork for FanartTv {
    /// Images d'un film limitées aux types de base (affiche, fond, vignette, logo).
    async fn artwork(
        &self,
        options: &OptionsRecherche,
        type_filtre: Option<TypeArtwork>,
    ) -> Resultat<Vec<Artwork>> {
        let r = self.reponse_film(options).await?;
        Ok(self
            .images_film(&r, type_filtre.map(TypeArtworkEtendu::from))
            .iter()
            .filter_map(ArtworkEtendu::en_artwork)
            .collect())
    }
}

#[async_trait]
impl FournisseurArtworkSerie for FanartTv {
    /// Images d'une série : identifiant TVDB obligatoire (sinon liste vide).
    async fn artwork_serie(
        &self,
        options: &OptionsRecherche,
        type_filtre: Option<TypeArtworkEtendu>,
    ) -> Resultat<Vec<ArtworkEtendu>> {
        let Some(tvdb) = options
            .ids
            .get(cles_id::TVDB)
            .and_then(|s| s.trim().parse::<u64>().ok())
            .filter(|n| *n > 0)
        else {
            return Ok(Vec::new());
        };
        let r = self.get(&format!("tv/{tvdb}")).await?;
        Ok(self.images_serie(&r, type_filtre))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entiers_et_saisons_souples() {
        let i: ImageFanart = serde_json::from_str(
            r#"{"url":"u","lang":"00","likes":"12","width":"1000","height":1426,"season":"all"}"#,
        )
        .unwrap();
        assert_eq!(
            (i.likes, i.width, i.height, i.season),
            (12, 1000, 1426, Some(0))
        );
        let i: ImageFanart =
            serde_json::from_str(r#"{"url":"u","likes":"x","season":"3"}"#).unwrap();
        assert_eq!((i.likes, i.season), (0, Some(3)));
        let i: ImageFanart = serde_json::from_str(r#"{"url":"u","season":null}"#).unwrap();
        assert_eq!(i.season, None);
    }

    #[test]
    fn conversion_langue_apercu_dimensions() {
        let i = ImageFanart {
            url: "https://assets.fanart.tv/fanart/movies/1/movieposter/a.jpg".into(),
            lang: "00".into(),
            ..Default::default()
        };
        let a = &convertir(&[i], TypeArtworkEtendu::Affiche, false, false)[0];
        assert_eq!(a.langue, "");
        assert_eq!(
            a.url_apercu,
            "https://assets.fanart.tv/preview/movies/1/movieposter/a.jpg"
        );
        assert_eq!((a.tailles[0].largeur, a.tailles[0].hauteur), (1000, 1426));
        assert_eq!(a.tailles[0].ordre, 8);
    }

    #[test]
    fn debug_masque_les_cles() {
        let mut c = ConfigFanart::nouvelle("secret-projet");
        c.cle_client = "secret-perso".into();
        let s = format!("{c:?}");
        assert!(!s.contains("secret"));
    }
}
