//! Fournisseurs de notes (évaluations) : OMDb, Trakt et MDBList.
//!
//! Chaque fournisseur est autonome et tolérant : un identifiant absent, un
//! film inconnu, une clé refusée ou une limitation de débit sont des erreurs
//! typées ([`ErreurScraper`]) que l'appelant peut transformer en
//! avertissement sans interrompre le traitement des autres sources. Les clés
//! d'API ne sont jamais journalisées (masquées dans `Debug`, transmises par
//! paramètre ou en-tête secret du client HTTP, donc absentes des erreurs et du
//! cache).
//!
//! Sources produites (noms stables) : `imdb`, `rottenTomatoes`, `metacritic`,
//! `trakt`.

use async_trait::async_trait;
use serde::Deserialize;

use crate::erreur::{ErreurScraper, Resultat};
use crate::fournisseur::FournisseurEvaluations;
use crate::http::ClientHttp;
use crate::types::{Evaluation, OptionsRecherche, cles_id};

/// Noms des sources de notes.
pub mod sources {
    pub const TMDB: &str = "tmdb";
    pub const IMDB: &str = "imdb";
    pub const ROTTEN_TOMATOES: &str = "rottenTomatoes";
    pub const METACRITIC: &str = "metacritic";
    pub const TRAKT: &str = "trakt";
}

const URL_OMDB: &str = "https://www.omdbapi.com/";
const URL_TRAKT: &str = "https://api.trakt.tv/";
const URL_MDBLIST: &str = "https://api.mdblist.com/";
const URL_SIMKL: &str = "https://api.simkl.com/";

/// Identifiant de test (film connu de tous les services).
const IMDB_TEST: &str = "tt0133093";

fn masque(s: &str) -> &'static str {
    if s.is_empty() { "<vide>" } else { "<masquée>" }
}

fn base(url: &Option<String>, defaut: &'static str) -> String {
    let mut b = url.clone().unwrap_or_else(|| defaut.to_string());
    if !b.ends_with('/') {
        b.push('/');
    }
    b
}

/// Nombre lu dans un texte tolérant (`"1,987,654"`, `"83%"`, `"73/100"`, `"N/A"`).
fn nombre(texte: &str) -> Option<f32> {
    let t = texte.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("n/a") {
        return None;
    }
    let t = t.split('/').next().unwrap_or(t);
    let nettoye: String = t
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    nettoye.parse().ok()
}

fn evaluation(source: &str, valeur: f32, votes: u32, maximum: f32) -> Evaluation {
    Evaluation {
        source: source.into(),
        valeur,
        votes,
        maximum,
    }
}

// ---------------------------------------------------------------- OMDb

/// Configuration d'OMDb.
#[derive(Clone, Default)]
pub struct ConfigOmdb {
    /// Clé d'API fournie par l'utilisateur.
    pub cle_api: String,
    /// Surcharge de l'URL de base (tests, miroir).
    pub url_api: Option<String>,
}

impl ConfigOmdb {
    pub fn nouvelle(cle_api: impl Into<String>) -> Self {
        Self {
            cle_api: cle_api.into(),
            url_api: None,
        }
    }
}

impl std::fmt::Debug for ConfigOmdb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigOmdb")
            .field("cle_api", &masque(&self.cle_api))
            .field("url_api", &self.url_api)
            .finish()
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ReponseOmdb {
    #[serde(rename = "Response")]
    reponse: String,
    #[serde(rename = "Error")]
    erreur: String,
    #[serde(rename = "imdbRating")]
    note_imdb: String,
    #[serde(rename = "imdbVotes")]
    votes_imdb: String,
    #[serde(rename = "Metascore")]
    metascore: String,
    #[serde(rename = "Ratings")]
    notes: Vec<NoteOmdb>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct NoteOmdb {
    #[serde(rename = "Source")]
    source: String,
    #[serde(rename = "Value")]
    valeur: String,
}

/// Notes IMDb, Rotten Tomatoes et Metacritic via OMDb (par identifiant IMDb).
#[derive(Debug)]
pub struct Omdb {
    config: ConfigOmdb,
    http: ClientHttp,
}

impl Omdb {
    /// Échoue si la clé est vide.
    pub fn new(config: ConfigOmdb, http: ClientHttp) -> Resultat<Self> {
        if config.cle_api.trim().is_empty() {
            return Err(ErreurScraper::CleApiManquante);
        }
        Ok(Self { config, http })
    }

    async fn par_imdb(&self, imdb: &str) -> Resultat<Vec<Evaluation>> {
        let url = base(&self.config.url_api, URL_OMDB);
        let r: ReponseOmdb = self
            .http
            .obtenir_json(
                &url,
                &[("i", Some(imdb))],
                &[("apikey", self.config.cle_api.trim())],
            )
            .await?;
        if !r.reponse.eq_ignore_ascii_case("true") {
            let e = r.erreur.to_lowercase();
            return Err(if e.contains("api key") {
                ErreurScraper::CleApiInvalide
            } else if e.contains("limit") {
                ErreurScraper::Http {
                    statut: 429,
                    url: "omdb".into(),
                }
            } else {
                ErreurScraper::AucunResultat
            });
        }
        let mut sortie = Vec::new();
        if let Some(v) = nombre(&r.note_imdb) {
            let votes = nombre(&r.votes_imdb).unwrap_or(0.0) as u32;
            sortie.push(evaluation(sources::IMDB, v, votes, 10.0));
        }
        let dans_liste = |nom: &str| {
            r.notes
                .iter()
                .find(|n| n.source.eq_ignore_ascii_case(nom))
                .and_then(|n| nombre(&n.valeur))
        };
        if let Some(v) = dans_liste("Rotten Tomatoes") {
            sortie.push(evaluation(sources::ROTTEN_TOMATOES, v, 0, 100.0));
        }
        if let Some(v) = nombre(&r.metascore).or_else(|| dans_liste("Metacritic")) {
            sortie.push(evaluation(sources::METACRITIC, v, 0, 100.0));
        }
        if sortie.is_empty() {
            return Err(ErreurScraper::AucunResultat);
        }
        Ok(sortie)
    }

    /// Vérifie la clé par une requête sur un titre connu.
    pub async fn tester(&self) -> Resultat<()> {
        self.par_imdb(IMDB_TEST).await.map(|_| ())
    }
}

#[async_trait]
impl FournisseurEvaluations for Omdb {
    fn identifiant(&self) -> &'static str {
        "omdb"
    }
    async fn evaluations(&self, options: &OptionsRecherche) -> Resultat<Vec<Evaluation>> {
        let imdb = options.id_imdb();
        if imdb.is_empty() {
            return Err(ErreurScraper::IdentifiantManquant("imdb".into()));
        }
        self.par_imdb(&imdb).await
    }
}

// ---------------------------------------------------------------- Trakt

/// Configuration de Trakt.
#[derive(Clone, Default)]
pub struct ConfigTrakt {
    /// Identifiant client (« client id ») fourni par l'utilisateur.
    pub cle_client: String,
    pub url_api: Option<String>,
    /// Vrai pour les séries (`shows/…`), faux pour les films (`movies/…`).
    pub serie: bool,
}

impl ConfigTrakt {
    pub fn nouvelle(cle_client: impl Into<String>) -> Self {
        Self {
            cle_client: cle_client.into(),
            ..Self::default()
        }
    }
}

impl std::fmt::Debug for ConfigTrakt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigTrakt")
            .field("cle_client", &masque(&self.cle_client))
            .field("url_api", &self.url_api)
            .field("serie", &self.serie)
            .finish()
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ReponseTrakt {
    rating: f32,
    votes: u32,
}

/// Note et votes Trakt (par identifiant IMDb, sinon identifiant Trakt).
#[derive(Debug)]
pub struct Trakt {
    config: ConfigTrakt,
    http: ClientHttp,
}

impl Trakt {
    /// Échoue si l'identifiant client est vide.
    pub fn new(config: ConfigTrakt, http: ClientHttp) -> Resultat<Self> {
        if config.cle_client.trim().is_empty() {
            return Err(ErreurScraper::CleApiManquante);
        }
        Ok(Self { config, http })
    }

    async fn par_id(&self, id: &str) -> Resultat<Vec<Evaluation>> {
        let genre = if self.config.serie { "shows" } else { "movies" };
        let url = format!(
            "{}{genre}/{id}/ratings",
            base(&self.config.url_api, URL_TRAKT)
        );
        let entetes = [
            ("trakt-api-key", self.config.cle_client.trim()),
            ("trakt-api-version", "2"),
        ];
        let r: ReponseTrakt = match self
            .http
            .obtenir_json_avec_entetes(&url, &[], &entetes)
            .await
        {
            Ok(r) => r,
            Err(ErreurScraper::Http { statut: 404, .. }) => {
                return Err(ErreurScraper::AucunResultat);
            }
            // Trakt répond 403 à un identifiant client invalide.
            Err(ErreurScraper::Http { statut: 403, .. }) => {
                return Err(ErreurScraper::CleApiInvalide);
            }
            Err(e) => return Err(e),
        };
        if r.votes == 0 && r.rating <= 0.0 {
            return Err(ErreurScraper::AucunResultat);
        }
        Ok(vec![evaluation(sources::TRAKT, r.rating, r.votes, 10.0)])
    }

    /// Vérifie l'identifiant client par une requête sur un titre connu.
    pub async fn tester(&self) -> Resultat<()> {
        let mut essai = Trakt {
            config: self.config.clone(),
            http: self.http.clone(),
        };
        essai.config.serie = false;
        essai.par_id(IMDB_TEST).await.map(|_| ())
    }
}

#[async_trait]
impl FournisseurEvaluations for Trakt {
    fn identifiant(&self) -> &'static str {
        "trakt"
    }
    async fn evaluations(&self, options: &OptionsRecherche) -> Resultat<Vec<Evaluation>> {
        let imdb = options.id_imdb();
        if !imdb.is_empty() {
            return self.par_id(&imdb).await;
        }
        match options
            .ids
            .get("trakt")
            .and_then(|s| s.trim().parse::<u64>().ok())
            .filter(|n| *n > 0)
        {
            Some(n) => self.par_id(&n.to_string()).await,
            None => Err(ErreurScraper::IdentifiantManquant("imdb, trakt".into())),
        }
    }
}

// ---------------------------------------------------------------- MDBList

/// Configuration de MDBList.
#[derive(Clone, Default)]
pub struct ConfigMdbList {
    pub cle_api: String,
    pub url_api: Option<String>,
    /// Vrai pour les séries (`show`), faux pour les films (`movie`).
    pub serie: bool,
}

impl ConfigMdbList {
    pub fn nouvelle(cle_api: impl Into<String>) -> Self {
        Self {
            cle_api: cle_api.into(),
            ..Self::default()
        }
    }
}

impl std::fmt::Debug for ConfigMdbList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigMdbList")
            .field("cle_api", &masque(&self.cle_api))
            .field("url_api", &self.url_api)
            .field("serie", &self.serie)
            .finish()
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ReponseMdbList {
    ratings: Vec<NoteMdbList>,
    /// Message d'erreur (`{"response": false, "error": "..."}`).
    error: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct NoteMdbList {
    source: String,
    value: Option<f32>,
    score: Option<f32>,
    votes: Option<u32>,
}

/// Notes agrégées par MDBList (IMDb, Rotten Tomatoes, Metacritic, Trakt).
#[derive(Debug)]
pub struct MdbList {
    config: ConfigMdbList,
    http: ClientHttp,
}

impl MdbList {
    /// Échoue si la clé est vide.
    pub fn new(config: ConfigMdbList, http: ClientHttp) -> Resultat<Self> {
        if config.cle_api.trim().is_empty() {
            return Err(ErreurScraper::CleApiManquante);
        }
        Ok(Self { config, http })
    }

    async fn par_chemin(&self, chemin: &str) -> Resultat<Vec<Evaluation>> {
        let url = format!("{}{chemin}", base(&self.config.url_api, URL_MDBLIST));
        let r: ReponseMdbList = match self
            .http
            .obtenir_json(&url, &[], &[("apikey", self.config.cle_api.trim())])
            .await
        {
            Ok(r) => r,
            Err(ErreurScraper::Http { statut: 404, .. }) => {
                return Err(ErreurScraper::AucunResultat);
            }
            Err(e) => return Err(e),
        };
        if let Some(e) = r.error.as_deref().map(str::to_lowercase) {
            return Err(if e.contains("api key") || e.contains("apikey") {
                ErreurScraper::CleApiInvalide
            } else if e.contains("limit") {
                ErreurScraper::Http {
                    statut: 429,
                    url: "mdblist".into(),
                }
            } else {
                ErreurScraper::AucunResultat
            });
        }
        let mut sortie = Vec::new();
        for n in &r.ratings {
            let votes = n.votes.unwrap_or(0);
            let (source, valeur, maximum) = match n.source.to_lowercase().as_str() {
                "imdb" | "trakt" => {
                    let s = if n.source.eq_ignore_ascii_case("imdb") {
                        sources::IMDB
                    } else {
                        sources::TRAKT
                    };
                    let v = n
                        .value
                        .filter(|v| *v > 0.0)
                        .or_else(|| n.score.filter(|s| *s > 0.0).map(|s| s / 10.0));
                    (s, v, 10.0)
                }
                "tomatoes" | "rottentomatoes" => (
                    sources::ROTTEN_TOMATOES,
                    n.score
                        .filter(|s| *s > 0.0)
                        .or(n.value.filter(|v| *v > 0.0)),
                    100.0,
                ),
                "metacritic" => (
                    sources::METACRITIC,
                    n.score
                        .filter(|s| *s > 0.0)
                        .or(n.value.filter(|v| *v > 0.0)),
                    100.0,
                ),
                _ => continue,
            };
            if let Some(v) = valeur {
                sortie.push(evaluation(source, v, votes, maximum));
            }
        }
        if sortie.is_empty() {
            return Err(ErreurScraper::AucunResultat);
        }
        Ok(sortie)
    }

    /// Vérifie la clé par une requête sur un titre connu.
    pub async fn tester(&self) -> Resultat<()> {
        self.par_chemin(&format!("imdb/movie/{IMDB_TEST}"))
            .await
            .map(|_| ())
    }
}

#[async_trait]
impl FournisseurEvaluations for MdbList {
    fn identifiant(&self) -> &'static str {
        "mdblist"
    }
    async fn evaluations(&self, options: &OptionsRecherche) -> Resultat<Vec<Evaluation>> {
        let genre = if self.config.serie { "show" } else { "movie" };
        let imdb = options.id_imdb();
        if !imdb.is_empty() {
            return self.par_chemin(&format!("imdb/{genre}/{imdb}")).await;
        }
        let tmdb = options.id_tmdb();
        if tmdb > 0 {
            return self.par_chemin(&format!("tmdb/{genre}/{tmdb}")).await;
        }
        Err(ErreurScraper::IdentifiantManquant("imdb, tmdb".into()))
    }
}

// ---------------------------------------------------------------- test de clés

/// Services dont la clé peut être testée par [`tester_cle_service`].
pub const SERVICES_TESTABLES: [&str; 5] = ["omdb", "trakt", "mdblist", "simkl", "fanart"];

/// Teste la clé d'un service par une requête minimale (aucun secret n'est
/// journalisé). `url_api` : surcharge de l'URL de base (tests).
///
/// Services gérés : `omdb`, `trakt`, `mdblist`, `simkl`, `fanart`. Autre
/// service : [`ErreurScraper::IdentifiantManquant`] (« service non testable »).
pub async fn tester_cle_service(
    service: &str,
    cle: &str,
    http: &ClientHttp,
    url_api: Option<String>,
) -> Resultat<()> {
    match service {
        "omdb" => {
            let mut c = ConfigOmdb::nouvelle(cle);
            c.url_api = url_api;
            Omdb::new(c, http.clone())?.tester().await
        }
        "trakt" => {
            let mut c = ConfigTrakt::nouvelle(cle);
            c.url_api = url_api;
            Trakt::new(c, http.clone())?.tester().await
        }
        "mdblist" => {
            let mut c = ConfigMdbList::nouvelle(cle);
            c.url_api = url_api;
            MdbList::new(c, http.clone())?.tester().await
        }
        "fanart" => {
            let mut c = crate::fanart::ConfigFanart::nouvelle(cle);
            c.url_api = url_api;
            let f = crate::fanart::FanartTv::new(c, http.clone())?;
            let o = OptionsRecherche::default().avec_id(cles_id::IMDB, IMDB_TEST);
            f.artwork_film_etendu(&o, None).await.map(|_| ())
        }
        "simkl" => {
            if cle.trim().is_empty() {
                return Err(ErreurScraper::CleApiManquante);
            }
            let url = format!("{}search/id", base(&url_api, URL_SIMKL));
            http.obtenir_texte_avec_entetes(
                &url,
                &[("imdb", Some(IMDB_TEST))],
                &[],
                &[("simkl-api-key", cle.trim())],
            )
            .await
            .map(|_| ())
        }
        autre => Err(ErreurScraper::IdentifiantManquant(format!(
            "service non testable : {autre}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres_tolerants() {
        assert_eq!(nombre("1,987,654"), Some(1_987_654.0));
        assert_eq!(nombre("83%"), Some(83.0));
        assert_eq!(nombre("73/100"), Some(73.0));
        assert_eq!(nombre("N/A"), None);
        assert_eq!(nombre(""), None);
    }

    #[test]
    fn debug_masque_les_cles() {
        let c = ConfigOmdb::nouvelle("secret-omdb");
        assert!(!format!("{c:?}").contains("secret-omdb"));
        let c = ConfigTrakt::nouvelle("secret-trakt");
        assert!(!format!("{c:?}").contains("secret-trakt"));
        let c = ConfigMdbList::nouvelle("secret-mdb");
        assert!(!format!("{c:?}").contains("secret-mdb"));
    }
}
