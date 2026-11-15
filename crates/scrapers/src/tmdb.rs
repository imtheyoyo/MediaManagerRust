//! Fournisseur TMDB pour les films : recherche, détail, collections, images,
//! bandes-annonces. La clé d'API est fournie par l'utilisateur.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::OnceCell;

use crate::correspondance::{
    calculer_score, est_id_imdb, nettoyer_caracteres, retirer_nom_triable, trier_resultats,
};
use crate::erreur::{ErreurScraper, Resultat};
use crate::fournisseur::{
    FournisseurArtwork, FournisseurBandesAnnonces, FournisseurCollection, FournisseurFilm,
};
use crate::http::ClientHttp;
use crate::types::*;

pub mod serie;

const URL_API: &str = "https://api.themoviedb.org/3/";
const URL_API_ALTERNATIVE: &str = "https://api.tmdb.org/3/";
const URL_IMAGES_DEFAUT: &str = "https://image.tmdb.org/t/p/";
const PAGES_MAX: u32 = 5;
const ID_FOURNISSEUR: &str = "tmdb";
const PREFIXES_ARTICLES: [&str; 12] = [
    "The", "A", "An", "Le", "La", "Les", "L'", "Un", "Une", "Der", "Die", "Das",
];

/// Configuration du scraper TMDB.
#[derive(Clone)]
pub struct ConfigTmdb {
    /// Clé d'API fournie par l'utilisateur (obligatoire).
    pub cle_api: String,
    /// Utilise `api.tmdb.org` au lieu de `api.themoviedb.org`.
    pub serveur_alternatif: bool,
    /// Surcharge de l'URL de base de l'API (tests, miroir). Se termine par `/`.
    pub url_api: Option<String>,
    pub inclure_adulte: bool,
    /// Noms de langues/pays localisés au lieu des codes ISO (non géré : codes ISO conservés si faux).
    pub noms_langues_localises: bool,
    /// Repli sur le titre original / une traduction si le titre localisé est vide.
    pub repli_titre: bool,
    pub langue_repli: String,
    /// Date de sortie propre au pays `pays_sortie`.
    pub date_sortie_locale: bool,
    pub inclure_avant_premiere: bool,
}

impl ConfigTmdb {
    /// Configuration par défaut avec la clé de l'utilisateur.
    pub fn nouvelle(cle_api: impl Into<String>) -> Self {
        Self {
            cle_api: cle_api.into(),
            serveur_alternatif: false,
            url_api: None,
            inclure_adulte: false,
            noms_langues_localises: false,
            repli_titre: false,
            langue_repli: "en-US".into(),
            date_sortie_locale: true,
            inclure_avant_premiere: true,
        }
    }
}

impl std::fmt::Debug for ConfigTmdb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigTmdb")
            .field(
                "cle_api",
                &if self.cle_api.is_empty() {
                    "<vide>"
                } else {
                    "<masquée>"
                },
            )
            .field("serveur_alternatif", &self.serveur_alternatif)
            .field("url_api", &self.url_api)
            .finish_non_exhaustive()
    }
}

/// Scraper TMDB (films).
#[derive(Debug)]
pub struct TmdbFilm {
    config: ConfigTmdb,
    http: ClientHttp,
    url_images: OnceCell<String>,
}

// ---------------------------------------------------------------- JSON TMDB

#[derive(Deserialize, Default)]
#[serde(default)]
struct ReponseConfiguration {
    images: ImagesConfiguration,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct ImagesConfiguration {
    secure_base_url: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PageRecherche {
    page: u32,
    total_pages: u32,
    results: Vec<ElementRecherche>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct ElementRecherche {
    id: u64,
    title: String,
    original_title: String,
    original_language: String,
    overview: String,
    poster_path: Option<String>,
    release_date: Option<String>,
    // collections
    name: String,
    backdrop_path: Option<String>,
    parts: Vec<ElementRecherche>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ReponseTrouver {
    movie_results: Vec<ElementRecherche>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DetailFilm {
    id: u64,
    title: String,
    original_title: String,
    original_language: String,
    overview: String,
    tagline: String,
    runtime: Option<u32>,
    release_date: Option<String>,
    vote_average: f32,
    vote_count: u32,
    poster_path: Option<String>,
    adult: bool,
    genres: Vec<Genre>,
    spoken_languages: Vec<LangueParlee>,
    production_countries: Vec<PaysProduction>,
    production_companies: Vec<Societe>,
    belongs_to_collection: Option<CollectionRef>,
    credits: Credits,
    keywords: Mots,
    release_dates: DatesSortie,
    translations: Traductions,
    external_ids: IdsExternes,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Genre {
    id: u32,
    name: String,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct LangueParlee {
    iso_639_1: String,
    name: String,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct PaysProduction {
    iso_3166_1: String,
    name: String,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct Societe {
    name: String,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct CollectionRef {
    id: u64,
    name: String,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct Credits {
    cast: Vec<MembreDistribution>,
    crew: Vec<MembreEquipe>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct MembreDistribution {
    id: u64,
    name: String,
    character: String,
    profile_path: Option<String>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct MembreEquipe {
    id: u64,
    name: String,
    job: String,
    department: String,
    profile_path: Option<String>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct Mots {
    keywords: Vec<Mot>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct Mot {
    name: String,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct DatesSortie {
    results: Vec<DatesPays>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct DatesPays {
    iso_3166_1: String,
    release_dates: Vec<DateSortie>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct DateSortie {
    certification: String,
    release_date: String,
    #[serde(rename = "type")]
    type_sortie: u32,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct Traductions {
    translations: Vec<Traduction>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct Traduction {
    iso_639_1: String,
    iso_3166_1: String,
    data: DonneesTraduction,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct DonneesTraduction {
    title: String,
    overview: String,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct IdsExternes {
    imdb_id: Option<String>,
    wikidata_id: Option<String>,
    tvdb_id: Option<i64>,
    tvrage_id: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DetailCollection {
    id: u64,
    name: String,
    overview: String,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    parts: Vec<ElementRecherche>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ReponseImages {
    posters: Vec<ImageTmdb>,
    backdrops: Vec<ImageTmdb>,
    logos: Vec<ImageTmdb>,
}
#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct ImageTmdb {
    file_path: String,
    width: u32,
    height: u32,
    iso_639_1: Option<String>,
    vote_average: f32,
    vote_count: u32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ReponseVideos {
    results: Vec<VideoTmdb>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct VideoTmdb {
    name: String,
    key: String,
    site: String,
    size: u32,
    #[serde(rename = "type")]
    type_video: String,
    iso_639_1: String,
    iso_3166_1: String,
    published_at: String,
}

// ---------------------------------------------------------------- utilitaires

/// Langue de requête TMDB : `pt_BR` -> `pt-BR`, `fr` -> `fr-FR`, `en` -> `en-US`.
pub fn langue_requete(langue: &str) -> String {
    let l = langue.trim();
    if l.is_empty() || l.eq_ignore_ascii_case("none") {
        return String::new();
    }
    if l.len() > 2 {
        return l.replace('_', "-");
    }
    let l = l.to_lowercase();
    let region = match l.as_str() {
        "en" => "US",
        "ja" => "JP",
        "ko" => "KR",
        "zh" => "CN",
        "cs" => "CZ",
        "da" => "DK",
        "el" => "GR",
        "sv" => "SE",
        "uk" => "UA",
        "he" => "IL",
        "hi" => "IN",
        _ => "",
    };
    if region.is_empty() {
        format!("{l}-{}", l.to_uppercase())
    } else {
        format!("{l}-{region}")
    }
}

/// Date du jour `AAAA-MM-JJ` (UTC).
pub fn date_du_jour() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let jours = (secs / 86_400) as i64;
    // Algorithme « civil_from_days » (H. Hinnant).
    let z = jours + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

fn annee_de(date: &Option<String>) -> i32 {
    date.as_deref()
        .and_then(|d| d.get(..4))
        .and_then(|a| a.parse().ok())
        .unwrap_or(0)
}

fn date_iso(d: &str) -> Option<String> {
    let d = d.get(..10)?;
    let b = d.as_bytes();
    (b[4] == b'-' && b[7] == b'-' && d.chars().filter(char::is_ascii_digit).count() == 8)
        .then(|| d.to_string())
}

fn non_vide(o: &Option<String>) -> Option<&str> {
    o.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// Genre interne (identifiant en majuscules) pour un id TMDB.
pub fn genre_depuis_id_tmdb(id: u32) -> Option<&'static str> {
    Some(match id {
        12 => "ADVENTURE",
        14 => "FANTASY",
        16 => "ANIMATION",
        18 => "DRAMA",
        22 => "MUSICAL",
        27 => "HORROR",
        28 | 10759 => "ACTION",
        35 => "COMEDY",
        36 => "HISTORY",
        37 => "WESTERN",
        53 => "THRILLER",
        80 => "CRIME",
        82 => "EASTERN",
        99 => "DOCUMENTARY",
        105 => "DISASTER",
        878 | 10765 => "SCIENCE_FICTION",
        1115 => "ROAD_MOVIE",
        2916 => "EROTIC",
        9648 => "MYSTERY",
        9805 => "SPORT",
        10402 => "MUSIC",
        10595 => "HOLIDAY",
        10748 => "SUSPENSE",
        10749 => "ROMANCE",
        10750 => "FAN_FILM",
        10751 => "FAMILY",
        10752 | 10768 => "WAR",
        10753 => "FILM_NOIR",
        10754 => "NEO_NOIR",
        10755 => "SHORT",
        10756 => "INDIE",
        10757 => "SPORTS_FILM",
        10758 => "SPORTING_EVENT",
        10763 => "NEWS",
        10764 => "REALITY_TV",
        10766 => "SOAP",
        10767 => "TALK_SHOW",
        10769 => "FOREIGN",
        10770 => "TV_MOVIE",
        _ => return None,
    })
}

/// Ordre de taille (puissance de 2) d'une image selon son type et sa largeur.
pub fn ordre_taille(type_artwork: TypeArtwork, largeur: u32) -> u32 {
    let seuils: &[(u32, u32)] = match type_artwork {
        TypeArtwork::Affiche => &[(2000, 16), (1000, 8), (500, 4), (342, 2), (0, 1)],
        TypeArtwork::Fond => &[(3840, 16), (1920, 8), (1280, 2), (0, 1)],
        TypeArtwork::Vignette => &[(3840, 16), (1920, 8), (1000, 4), (640, 2), (0, 1)],
        TypeArtwork::LogoTransparent => &[(2000, 16), (1000, 8), (500, 4), (300, 2), (0, 1)],
    };
    seuils
        .iter()
        .find(|(l, _)| largeur >= *l)
        .map(|(_, o)| *o)
        .unwrap_or(1)
}

fn recherche_traduction<'a>(trad: &'a [Traduction], langue: &str) -> Option<&'a Traduction> {
    let (l, p) = match langue.split_once(['_', '-']) {
        Some((l, p)) => (l.to_lowercase(), p.to_uppercase()),
        None => (langue.to_lowercase(), String::new()),
    };
    if !p.is_empty() {
        if let Some(t) = trad
            .iter()
            .find(|t| t.iso_639_1.eq_ignore_ascii_case(&l) && t.iso_3166_1 == p)
        {
            return Some(t);
        }
        if matches!(
            langue.replace('-', "_").as_str(),
            "es_MX" | "pt_BR" | "fr_CA"
        ) {
            return None;
        }
        return trad
            .iter()
            .find(|t| t.iso_639_1.eq_ignore_ascii_case(&l) || t.iso_3166_1 == p);
    }
    trad.iter().find(|t| t.iso_639_1.eq_ignore_ascii_case(&l))
}

impl TmdbFilm {
    /// Crée le scraper. Échoue si la clé est vide (aucune clé par défaut n'existe).
    pub fn new(config: ConfigTmdb, http: ClientHttp) -> Resultat<Self> {
        if config.cle_api.trim().is_empty() {
            return Err(ErreurScraper::CleApiManquante);
        }
        Ok(Self {
            config,
            http,
            url_images: OnceCell::new(),
        })
    }

    fn base(&self) -> &str {
        match (&self.config.url_api, self.config.serveur_alternatif) {
            (Some(u), _) => u,
            (None, true) => URL_API_ALTERNATIVE,
            (None, false) => URL_API,
        }
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        chemin: &str,
        params: &[(&str, Option<&str>)],
    ) -> Resultat<T> {
        let url = format!("{}{}", self.base(), chemin);
        self.http
            .obtenir_json(&url, params, &[("api_key", self.config.cle_api.trim())])
            .await
    }

    /// URL de base des images (appel `configuration`, mémorisé).
    async fn base_images(&self) -> Resultat<&str> {
        self.url_images
            .get_or_try_init(|| async {
                let c: ReponseConfiguration = self.get("configuration", &[]).await?;
                let u = c.images.secure_base_url;
                Ok::<_, ErreurScraper>(if u.is_empty() {
                    URL_IMAGES_DEFAUT.to_string()
                } else {
                    u
                })
            })
            .await
            .map(String::as_str)
    }

    fn url_image(base: &str, taille: &str, chemin: &str) -> String {
        format!("{base}{taille}{chemin}")
    }

    /// Identifiant TMDB : `tmdb`, sinon `imdb` via `find`.
    async fn resoudre_id(&self, options: &OptionsRecherche) -> Resultat<u64> {
        let id = options.id_tmdb();
        if id > 0 {
            return Ok(id);
        }
        let imdb = options.id_imdb();
        if !imdb.is_empty() {
            let langue = langue_requete(&options.langue);
            let r: ReponseTrouver = self
                .get(
                    &format!("find/{imdb}"),
                    &[
                        ("external_source", Some("imdb_id")),
                        ("language", Some(&langue)),
                    ],
                )
                .await?;
            if let Some(e) = r.movie_results.first() {
                return Ok(e.id);
            }
            return Err(ErreurScraper::AucunResultat);
        }
        Err(ErreurScraper::IdentifiantManquant("tmdb, imdb".into()))
    }

    fn resultat_depuis(
        &self,
        e: &ElementRecherche,
        base_images: &str,
        options: &OptionsRecherche,
    ) -> ResultatRecherche {
        let mut r = ResultatRecherche {
            fournisseur: ID_FOURNISSEUR.into(),
            titre: if e.title.is_empty() {
                e.name.clone()
            } else {
                e.title.clone()
            },
            titre_original: e.original_title.clone(),
            langue_originale: e.original_language.clone(),
            resume: e.overview.clone(),
            annee: annee_de(&e.release_date),
            url_affiche: non_vide(&e.poster_path)
                .map(|p| Self::url_image(base_images, "w342", p))
                .unwrap_or_default(),
            ..Default::default()
        };
        r.ids.insert(cles_id::TMDB.into(), e.id.to_string());
        calculer_score(options, &mut r);
        r
    }

    async fn recherche_texte(
        &self,
        chemin: &str,
        requete: &str,
        langue: &str,
        page_unique: bool,
    ) -> Resultat<Vec<ElementRecherche>> {
        let adulte = if self.config.inclure_adulte {
            "true"
        } else {
            "false"
        };
        let mut tout = Vec::new();
        let mut page = 1u32;
        loop {
            let p = page.to_string();
            let r: PageRecherche = self
                .get(
                    chemin,
                    &[
                        ("query", Some(requete)),
                        ("page", Some(&p)),
                        ("language", Some(langue)),
                        ("include_adult", Some(adulte)),
                    ],
                )
                .await?;
            tout.extend(r.results);
            let max = r.total_pages.min(PAGES_MAX);
            if page_unique || page >= max {
                break;
            }
            page += 1;
        }
        Ok(tout)
    }

    /// Détail brut d'un film.
    async fn detail(&self, id: u64, langue: &str) -> Resultat<DetailFilm> {
        self.get(
            &format!("movie/{id}"),
            &[
                ("language", Some(langue)),
                (
                    "append_to_response",
                    Some("credits,keywords,release_dates,translations,external_ids"),
                ),
            ],
        )
        .await
    }

    fn convertir_detail(
        &self,
        d: DetailFilm,
        options: &OptionsRecherche,
        base_images: &str,
    ) -> MetadonneesFilm {
        let langue = options.langue.as_str();
        let mut m = MetadonneesFilm {
            fournisseur: ID_FOURNISSEUR.into(),
            titre: d.title.trim().to_string(),
            titre_original: d.original_title.trim().to_string(),
            langue_originale: d.original_language.clone(),
            resume: d.overview.trim().to_string(),
            accroche: d.tagline.trim().to_string(),
            duree: d.runtime.unwrap_or(0),
            annee: annee_de(&d.release_date),
            date_sortie: non_vide(&d.release_date).and_then(date_iso),
            ..Default::default()
        };
        m.ids.insert(cles_id::TMDB.into(), d.id.to_string());
        if let Some(i) = non_vide(&d.external_ids.imdb_id) {
            m.ids.insert(cles_id::IMDB.into(), i.to_string());
        }
        if let Some(w) = non_vide(&d.external_ids.wikidata_id) {
            m.ids.insert("wikidata".into(), w.to_string());
        }
        for (cle, v) in [
            ("tvdb", d.external_ids.tvdb_id),
            ("tvrage", d.external_ids.tvrage_id),
        ] {
            if let Some(v) = v.filter(|v| *v > 0) {
                m.ids.insert(cle.into(), v.to_string());
            }
        }
        // Repli de titre / résumé.
        if self.config.repli_titre {
            if m.titre.is_empty() {
                if langue.split(['_', '-']).next() == Some(d.original_language.as_str()) {
                    m.titre = m.titre_original.clone();
                }
                if m.titre.is_empty()
                    && let Some(t) = recherche_traduction(
                        &d.translations.translations,
                        &self.config.langue_repli,
                    )
                {
                    m.titre = t.data.title.trim().to_string();
                }
            }
            if m.resume.is_empty()
                && let Some(t) =
                    recherche_traduction(&d.translations.translations, &self.config.langue_repli)
            {
                m.resume = t.data.overview.trim().to_string();
            }
        }
        // Titre anglais.
        m.titre_anglais = d
            .translations
            .translations
            .iter()
            .find(|t| t.iso_639_1 == "en" && !t.data.title.trim().is_empty())
            .map(|t| t.data.title.trim().to_string())
            .or_else(|| (d.original_language == "en").then(|| m.titre_original.clone()))
            .unwrap_or_default();
        if d.vote_count > 0 || d.vote_average > 0.0 {
            m.evaluations.push(Evaluation {
                source: "tmdb".into(),
                valeur: d.vote_average,
                votes: d.vote_count,
                maximum: 10.0,
            });
        }
        if let Some(p) = non_vide(&d.poster_path) {
            m.artwork.push(Artwork {
                fournisseur: ID_FOURNISSEUR.into(),
                type_artwork: TypeArtwork::Affiche,
                url_apercu: Self::url_image(base_images, "w342", p),
                url_originale: Self::url_image(base_images, "original", p),
                langue: langue.split(['_', '-']).next().unwrap_or("").to_string(),
                votes: 0,
                tailles: Vec::new(),
            });
        }
        m.langues_parlees = d
            .spoken_languages
            .iter()
            .map(|l| {
                if self.config.noms_langues_localises && !l.name.is_empty() {
                    l.name.clone()
                } else {
                    l.iso_639_1.clone()
                }
            })
            .filter(|s| !s.is_empty())
            .collect();
        m.pays = d
            .production_countries
            .iter()
            .map(|p| {
                if self.config.noms_langues_localises && !p.name.is_empty() {
                    p.name.clone()
                } else {
                    p.iso_3166_1.clone()
                }
            })
            .filter(|s| !s.is_empty())
            .collect();
        m.societes_production = d
            .production_companies
            .iter()
            .map(|s| s.name.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        // Classifications.
        for pays in d.release_dates.results.iter().filter(|p| {
            p.iso_3166_1
                .eq_ignore_ascii_case(&options.pays_classification)
        }) {
            for dr in &pays.release_dates {
                let c = dr.certification.trim();
                let cl = Classification {
                    pays: pays.iso_3166_1.clone(),
                    valeur: c.to_string(),
                };
                if !c.is_empty() && !m.classifications.contains(&cl) {
                    m.classifications.push(cl);
                }
            }
        }
        // Date de sortie locale : la plus ancienne (<= aujourd'hui) de type > 1, ou 1 si avant-première autorisée.
        if self.config.date_sortie_locale && !options.pays_sortie.is_empty() {
            let aujourdhui = date_du_jour();
            let locale = d
                .release_dates
                .results
                .iter()
                .filter(|p| p.iso_3166_1.eq_ignore_ascii_case(&options.pays_sortie))
                .flat_map(|p| p.release_dates.iter())
                .filter(|r| {
                    r.type_sortie > 1 || (r.type_sortie == 1 && self.config.inclure_avant_premiere)
                })
                .filter_map(|r| date_iso(&r.release_date))
                .filter(|dt| *dt <= aujourdhui)
                .min();
            if let Some(dt) = locale {
                m.annee = dt.get(..4).and_then(|a| a.parse().ok()).unwrap_or(m.annee);
                m.date_sortie = Some(dt);
            }
        }
        // Distribution et équipe.
        for a in &d.credits.cast {
            if a.name.trim().is_empty() {
                continue;
            }
            m.personnes.push(personne(
                TypePersonne::Acteur,
                a.id,
                &a.name,
                &a.character,
                &a.profile_path,
                base_images,
            ));
        }
        for c in &d.credits.crew {
            if let Some(t) = type_equipe(&c.job, &c.department)
                && !c.name.trim().is_empty()
            {
                m.personnes.push(personne(
                    t,
                    c.id,
                    &c.name,
                    &c.job,
                    &c.profile_path,
                    base_images,
                ));
            }
        }
        // Genres.
        for g in &d.genres {
            let nom = genre_depuis_id_tmdb(g.id)
                .map(str::to_string)
                .unwrap_or_else(|| g.name.clone());
            if !nom.is_empty() && !m.genres.contains(&nom) {
                m.genres.push(nom);
            }
        }
        if d.adult && !m.genres.iter().any(|g| g == "EROTIC") {
            m.genres.push("EROTIC".into());
        }
        if let Some(c) = &d.belongs_to_collection
            && c.id > 0
        {
            m.ids
                .insert(cles_id::TMDB_COLLECTION.into(), c.id.to_string());
            m.nom_collection = c.name.clone();
        }
        m.mots_cles = d
            .keywords
            .keywords
            .iter()
            .map(|k| k.name.clone())
            .filter(|k| !k.is_empty())
            .collect();
        m
    }
}

fn type_equipe(job: &str, departement: &str) -> Option<TypePersonne> {
    if job == "Director" {
        return Some(TypePersonne::Realisateur);
    }
    let contient = |s: &str| job.contains(s);
    match departement {
        "Writing" if contient("Screenplay") || contient("Writer") || contient("Story") => {
            Some(TypePersonne::Scenariste)
        }
        "Production" if contient("Producer") => Some(TypePersonne::Producteur),
        "Editing" if contient("Editor") => Some(TypePersonne::Monteur),
        "Sound" if contient("Original Music Composer") => Some(TypePersonne::Compositeur),
        "Camera" if contient("Director of Photography") => Some(TypePersonne::Photographie),
        _ => None,
    }
}

fn personne(
    t: TypePersonne,
    id: u64,
    nom: &str,
    role: &str,
    photo: &Option<String>,
    base_images: &str,
) -> Personne {
    let mut ids = BTreeMap::new();
    if id > 0 {
        ids.insert(cles_id::TMDB.to_string(), id.to_string());
    }
    Personne {
        type_personne: t,
        nom: nom.trim().to_string(),
        role: role.trim().to_string(),
        ids,
        url_photo: non_vide(photo)
            .map(|p| TmdbFilm::url_image(base_images, "h632", p))
            .unwrap_or_default(),
        url_profil: if id > 0 {
            format!("https://www.themoviedb.org/person/{id}")
        } else {
            String::new()
        },
    }
}

/// Tri client des images : langue demandée, puis anglais, puis note décroissante.
fn trier_images(images: &mut [ImageTmdb], langue: &str) {
    let rang = |i: &ImageTmdb| match i.iso_639_1.as_deref() {
        Some(l) if l == langue => 0,
        Some("en") => 1,
        _ => 2,
    };
    images.sort_by(|a, b| {
        rang(a)
            .cmp(&rang(b))
            .then(b.vote_average.total_cmp(&a.vote_average))
    });
}

fn construire_artwork(
    images: &[ImageTmdb],
    type_artwork: TypeArtwork,
    apercu: &str,
    tailles_possibles: &[u32],
    base: &str,
    svg_en_png: bool,
) -> Vec<Artwork> {
    images
        .iter()
        .filter(|i| !i.file_path.is_empty())
        .map(|i| {
            let (chemin, largeur, hauteur) = if svg_en_png && i.file_path.ends_with(".svg") {
                let h = if i.width > 0 {
                    (i.height as f32 * 2000.0 / i.width as f32).round() as u32
                } else {
                    i.height
                };
                (
                    format!("{}.png", i.file_path.trim_end_matches(".svg")),
                    2000,
                    h,
                )
            } else {
                (i.file_path.clone(), i.width, i.height)
            };
            let mut tailles = vec![TailleImage {
                largeur,
                hauteur,
                url: TmdbFilm::url_image(base, "original", &chemin),
                ordre: ordre_taille(type_artwork, largeur),
            }];
            for w in tailles_possibles.iter().filter(|w| **w < largeur) {
                let h = if largeur > 0 {
                    (hauteur as f32 * *w as f32 / largeur as f32).round() as u32
                } else {
                    0
                };
                tailles.push(TailleImage {
                    largeur: *w,
                    hauteur: h,
                    url: TmdbFilm::url_image(base, &format!("w{w}"), &chemin),
                    ordre: ordre_taille(type_artwork, *w),
                });
            }
            Artwork {
                fournisseur: ID_FOURNISSEUR.into(),
                type_artwork,
                url_apercu: TmdbFilm::url_image(base, apercu, &chemin),
                url_originale: TmdbFilm::url_image(base, "original", &chemin),
                langue: i.iso_639_1.clone().unwrap_or_default(),
                votes: i.vote_count,
                tailles,
            }
        })
        .collect()
}

// ---------------------------------------------------------------- traits

#[async_trait]
impl FournisseurFilm for TmdbFilm {
    fn identifiant(&self) -> &'static str {
        ID_FOURNISSEUR
    }

    async fn rechercher(&self, options: &OptionsRecherche) -> Resultat<Vec<ResultatRecherche>> {
        let base_images = self.base_images().await?.to_string();
        let langue = langue_requete(&options.langue);
        let requete =
            nettoyer_caracteres(&retirer_nom_triable(&options.requete, &PREFIXES_ARTICLES))
                .trim()
                .to_string();
        let mut imdb = options.id_imdb();
        if est_id_imdb(&requete.to_lowercase()) {
            imdb = requete.to_lowercase();
        }
        // Les options servent au score : on y injecte les identifiants détectés.
        let mut opts = options.clone();
        if !imdb.is_empty() {
            opts.ids.insert(cles_id::IMDB.into(), imdb.clone());
        }
        let mut elements: Vec<ElementRecherche> = Vec::new();
        let mut derniere_erreur: Option<ErreurScraper> = None;
        let mut depuis_find = false;

        let id = options.id_tmdb();
        if id > 0 {
            match self
                .get::<ElementRecherche>(
                    &format!("movie/{id}"),
                    &[
                        ("language", Some(&langue)),
                        ("append_to_response", Some("translations")),
                    ],
                )
                .await
            {
                Ok(e) => elements.push(e),
                Err(ErreurScraper::CleApiInvalide) => return Err(ErreurScraper::CleApiInvalide),
                Err(e) => derniere_erreur = Some(e),
            }
        }
        if elements.is_empty() && !imdb.is_empty() {
            match self
                .get::<ReponseTrouver>(
                    &format!("find/{imdb}"),
                    &[
                        ("external_source", Some("imdb_id")),
                        ("language", Some(&langue)),
                    ],
                )
                .await
            {
                Ok(r) => {
                    depuis_find = !r.movie_results.is_empty();
                    elements.extend(r.movie_results);
                }
                Err(ErreurScraper::CleApiInvalide) => return Err(ErreurScraper::CleApiInvalide),
                Err(e) => derniere_erreur = Some(e),
            }
        }
        if elements.is_empty() && !requete.is_empty() {
            if requete.chars().all(|c| c.is_ascii_digit())
                && let Ok(e) = self
                    .get::<ElementRecherche>(
                        &format!("movie/{requete}"),
                        &[("language", Some(&langue))],
                    )
                    .await
            {
                elements.push(e);
            }
            match self
                .recherche_texte("search/movie", &requete, &langue, false)
                .await
            {
                Ok(v) => elements.extend(v),
                Err(ErreurScraper::CleApiInvalide) => return Err(ErreurScraper::CleApiInvalide),
                Err(e) => derniere_erreur = Some(e),
            }
            if elements.is_empty() {
                // Repli : retire une année finale « AAAA » et une seule page.
                let sans_annee = {
                    let mots: Vec<&str> = requete.split(' ').collect();
                    match mots.split_last() {
                        Some((dernier, reste))
                            if !reste.is_empty()
                                && dernier.len() == 4
                                && dernier.chars().all(|c| c.is_ascii_digit()) =>
                        {
                            Some(reste.join(" "))
                        }
                        _ => None,
                    }
                };
                if let Some(q) = sans_annee {
                    match self
                        .recherche_texte("search/movie", &q, &langue, true)
                        .await
                    {
                        Ok(v) => elements.extend(v),
                        Err(e) => derniere_erreur = Some(e),
                    }
                }
            }
        }
        if elements.is_empty()
            && let Some(e) = derniere_erreur
        {
            return Err(e);
        }
        let mut vus = std::collections::BTreeSet::new();
        let mut resultats: Vec<ResultatRecherche> = elements
            .iter()
            .filter(|e| e.id > 0 && vus.insert(e.id))
            .map(|e| {
                let mut r = self.resultat_depuis(e, &base_images, &opts);
                if depuis_find {
                    // Résultat obtenu via l'identifiant IMDb : on le conserve et le score est forcé à 1.0.
                    r.ids.insert(cles_id::IMDB.into(), imdb.clone());
                    calculer_score(&opts, &mut r);
                }
                r
            })
            .collect();
        trier_resultats(&mut resultats);
        Ok(resultats)
    }

    async fn metadonnees(&self, options: &OptionsRecherche) -> Resultat<MetadonneesFilm> {
        let base_images = self.base_images().await?.to_string();
        let id = self.resoudre_id(options).await?;
        let langue = langue_requete(&options.langue);
        let d = self.detail(id, &langue).await?;
        Ok(self.convertir_detail(d, options, &base_images))
    }
}

#[async_trait]
impl FournisseurArtwork for TmdbFilm {
    async fn artwork(
        &self,
        options: &OptionsRecherche,
        type_filtre: Option<TypeArtwork>,
    ) -> Resultat<Vec<Artwork>> {
        let base = self.base_images().await?.to_string();
        let id = self.resoudre_id(options).await?;
        let r: ReponseImages = self.get(&format!("movie/{id}/images"), &[]).await?;
        Ok(artwork_depuis_images(r, options, &base, type_filtre))
    }
}

fn artwork_depuis_images(
    mut r: ReponseImages,
    options: &OptionsRecherche,
    base: &str,
    filtre: Option<TypeArtwork>,
) -> Vec<Artwork> {
    let langue = options
        .langue
        .split(['_', '-'])
        .next()
        .unwrap_or("en")
        .to_string();
    let veut = |t: TypeArtwork| filtre.is_none_or(|f| f == t);
    let mut sortie = Vec::new();
    if veut(TypeArtwork::Affiche) {
        trier_images(&mut r.posters, &langue);
        sortie.extend(construire_artwork(
            &r.posters,
            TypeArtwork::Affiche,
            "w342",
            &[780, 500, 342, 185],
            base,
            false,
        ));
    }
    if veut(TypeArtwork::Fond) {
        trier_images(&mut r.backdrops, &langue);
        sortie.extend(construire_artwork(
            &r.backdrops,
            TypeArtwork::Fond,
            "w1280",
            &[1280, 300],
            base,
            false,
        ));
    }
    if veut(TypeArtwork::LogoTransparent) {
        trier_images(&mut r.logos, &langue);
        sortie.extend(construire_artwork(
            &r.logos,
            TypeArtwork::LogoTransparent,
            "w500",
            &[500, 300],
            base,
            true,
        ));
    }
    sortie
}

impl TmdbFilm {
    /// Images d'une collection (identifiant `tmdbSet`).
    pub async fn artwork_collection(
        &self,
        options: &OptionsRecherche,
        type_filtre: Option<TypeArtwork>,
    ) -> Resultat<Vec<Artwork>> {
        let base = self.base_images().await?.to_string();
        let id = options.id_collection();
        if id == 0 {
            return Err(ErreurScraper::IdentifiantManquant("tmdbSet".into()));
        }
        let r: ReponseImages = self.get(&format!("collection/{id}/images"), &[]).await?;
        Ok(artwork_depuis_images(r, options, &base, type_filtre))
    }

    /// Collection avec les métadonnées complètes de chaque film déjà sorti
    /// (un appel par film ; en cas d'échec, la partie minimale est conservée).
    pub async fn collection_complete(
        &self,
        options: &OptionsRecherche,
    ) -> Resultat<CollectionFilm> {
        let mut c = self.collection(options).await?;
        for partie in &mut c.parties {
            let mut o = options.clone();
            o.ids.insert(
                cles_id::TMDB.into(),
                partie.ids.get(cles_id::TMDB).cloned().unwrap_or_default(),
            );
            match self.metadonnees(&o).await {
                Ok(m) => partie.metadonnees = Some(m),
                Err(ErreurScraper::CleApiInvalide) => return Err(ErreurScraper::CleApiInvalide),
                Err(e) => tracing::warn!(
                    "métadonnées de la partie {} indisponibles : {e}",
                    partie.titre
                ),
            }
        }
        Ok(c)
    }
}

#[async_trait]
impl FournisseurBandesAnnonces for TmdbFilm {
    async fn bandes_annonces(&self, options: &OptionsRecherche) -> Resultat<Vec<BandeAnnonce>> {
        let id = self.resoudre_id(options).await?;
        let langue = langue_requete(&options.langue);
        let mut videos = Vec::new();
        for l in [langue.as_str(), ""] {
            let r: ReponseVideos = self
                .get(&format!("movie/{id}/videos"), &[("language", Some(l))])
                .await?;
            videos.extend(r.results);
        }
        Ok(bandes_depuis_videos(videos))
    }
}

/// Bandes-annonces retenues (type `Trailer`, YouTube/Vimeo), dédoublonnées par URL.
fn bandes_depuis_videos(videos: Vec<VideoTmdb>) -> Vec<BandeAnnonce> {
    let mut sortie: Vec<BandeAnnonce> = Vec::new();
    for v in videos {
        if v.type_video != "Trailer" || v.site.eq_ignore_ascii_case("Apple") || v.key.is_empty() {
            continue;
        }
        let (hebergeur, url) = match v.site.to_lowercase().as_str() {
            "youtube" => (
                "youtube",
                format!(
                    "https://www.youtube.com/watch?v={}{}",
                    v.key,
                    if v.size >= 720 { "&hd=1" } else { "" }
                ),
            ),
            "vimeo" => ("vimeo", format!("https://vimeo.com/{}", v.key)),
            _ => continue,
        };
        let langue_v = match (v.iso_639_1.is_empty(), v.iso_3166_1.is_empty()) {
            (false, false) => format!("{}-{}", v.iso_639_1, v.iso_3166_1),
            (false, true) => v.iso_639_1.clone(),
            _ => String::new(),
        };
        let bande = BandeAnnonce {
            nom: v.name,
            url,
            hebergeur: hebergeur.into(),
            qualite: if v.size > 0 {
                format!("{}p", v.size)
            } else {
                String::new()
            },
            langue: langue_v,
            date: v.published_at.get(..10).unwrap_or("").to_string(),
        };
        if !sortie.iter().any(|b| b.url == bande.url) {
            sortie.push(bande);
        }
    }
    sortie
}

#[async_trait]
impl FournisseurCollection for TmdbFilm {
    async fn rechercher_collections(
        &self,
        options: &OptionsRecherche,
    ) -> Resultat<Vec<ResultatRecherche>> {
        let base_images = self.base_images().await?.to_string();
        let langue = langue_requete(&options.langue);
        let requete = nettoyer_caracteres(&options.requete).trim().to_string();
        let r: PageRecherche = self
            .get(
                "search/collection",
                &[
                    ("query", Some(&requete)),
                    ("page", Some("1")),
                    ("language", Some(&langue)),
                ],
            )
            .await?;
        let mut v: Vec<ResultatRecherche> = r
            .results
            .iter()
            .filter(|e| e.id > 0)
            .map(|e| {
                let mut o = self.resultat_depuis(e, &base_images, options);
                o.ids.remove(cles_id::TMDB);
                o.ids
                    .insert(cles_id::TMDB_COLLECTION.into(), e.id.to_string());
                o
            })
            .collect();
        trier_resultats(&mut v);
        Ok(v)
    }

    async fn collection(&self, options: &OptionsRecherche) -> Resultat<CollectionFilm> {
        let base = self.base_images().await?.to_string();
        let id = options.id_collection();
        if id == 0 {
            return Err(ErreurScraper::IdentifiantManquant("tmdbSet".into()));
        }
        let langue = langue_requete(&options.langue);
        let d: DetailCollection = self
            .get(&format!("collection/{id}"), &[("language", Some(&langue))])
            .await?;
        let mut c = CollectionFilm {
            titre: d.name.trim().to_string(),
            resume: d.overview.trim().to_string(),
            url_affiche: non_vide(&d.poster_path)
                .map(|p| Self::url_image(&base, "w342", p))
                .unwrap_or_default(),
            url_fond: non_vide(&d.backdrop_path)
                .map(|p| Self::url_image(&base, "w1280", p))
                .unwrap_or_default(),
            ..Default::default()
        };
        c.ids.insert(
            cles_id::TMDB_COLLECTION.into(),
            if d.id > 0 { d.id } else { id }.to_string(),
        );
        if !langue.starts_with("en") {
            // Titre anglais : second appel dans cette langue ; échec non bloquant.
            if let Ok(en) = self
                .get::<DetailCollection>(
                    &format!("collection/{id}"),
                    &[("language", Some("en-US"))],
                )
                .await
            {
                c.titre_anglais = en.name.trim().to_string();
            }
        } else {
            c.titre_anglais = c.titre.clone();
        }
        let aujourdhui = date_du_jour();
        for p in &d.parts {
            let date = non_vide(&p.release_date).and_then(date_iso);
            // Films à venir ou sans date : ignorés.
            match &date {
                Some(dt) if *dt <= aujourdhui => {}
                _ => continue,
            }
            let mut partie = PartieCollection {
                titre: p.title.clone(),
                titre_original: p.original_title.clone(),
                resume: p.overview.clone(),
                annee: annee_de(&p.release_date),
                date_sortie: date,
                url_affiche: non_vide(&p.poster_path)
                    .map(|x| Self::url_image(&base, "w342", x))
                    .unwrap_or_default(),
                ..Default::default()
            };
            partie.ids.insert(cles_id::TMDB.into(), p.id.to_string());
            c.parties.push(partie);
        }
        Ok(c)
    }
}
