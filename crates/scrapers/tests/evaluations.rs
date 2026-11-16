//! Tests des fournisseurs de notes (OMDb, Trakt, MDBList) contre un faux serveur local.

use mediamanager_scrapers::*;
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn http() -> ClientHttp {
    ClientHttp::new(ConfigHttp {
        essais_429: 1,
        ..ConfigHttp::default()
    })
    .unwrap()
}

fn options() -> OptionsRecherche {
    OptionsRecherche::default().avec_id("imdb", "tt0133093")
}

fn trouve<'a>(v: &'a [Evaluation], source: &str) -> &'a Evaluation {
    v.iter().find(|e| e.source == source).unwrap()
}

fn omdb(s: &MockServer, cle: &str) -> Omdb {
    let mut c = ConfigOmdb::nouvelle(cle);
    c.url_api = Some(format!("{}/omdb/", s.uri()));
    Omdb::new(c, http()).unwrap()
}

#[tokio::test]
async fn omdb_notes_imdb_rt_metacritic() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/omdb/"))
        .and(query_param("i", "tt0133093"))
        .and(query_param("apikey", "cle-omdb"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Response": "True", "imdbRating": "8.7", "imdbVotes": "2,000,123",
            "Metascore": "73",
            "Ratings": [
                {"Source": "Internet Movie Database", "Value": "8.7/10"},
                {"Source": "Rotten Tomatoes", "Value": "83%"},
                {"Source": "Metacritic", "Value": "73/100"}
            ]
        })))
        .expect(1)
        .mount(&s)
        .await;
    let v = omdb(&s, "cle-omdb").evaluations(&options()).await.unwrap();
    assert_eq!(v.len(), 3);
    let imdb = trouve(&v, "imdb");
    assert_eq!(
        (imdb.valeur, imdb.votes, imdb.maximum),
        (8.7, 2_000_123, 10.0)
    );
    assert_eq!(trouve(&v, "rottenTomatoes").valeur, 83.0);
    assert_eq!(trouve(&v, "rottenTomatoes").maximum, 100.0);
    assert_eq!(trouve(&v, "metacritic").valeur, 73.0);
}

#[tokio::test]
async fn omdb_valeurs_absentes_ignorees() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Response": "True", "imdbRating": "N/A", "imdbVotes": "N/A",
            "Metascore": "N/A", "Ratings": [{"Source": "Rotten Tomatoes", "Value": "40%"}]
        })))
        .mount(&s)
        .await;
    let v = omdb(&s, "k").evaluations(&options()).await.unwrap();
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].source, "rottenTomatoes");
}

#[tokio::test]
async fn omdb_erreurs_typees() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(query_param("i", "tt0000001"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"Response": "False", "Error": "Incorrect IMDb ID."})),
        )
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(query_param("i", "tt0000002"))
        .respond_with(
            ResponseTemplate::new(401)
                .set_body_json(json!({"Response": "False", "Error": "Invalid API key!"})),
        )
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(query_param("i", "tt0000003"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&s)
        .await;
    let f = omdb(&s, "cle-secrete-omdb");
    let o = |id: &str| OptionsRecherche::default().avec_id("imdb", id);
    assert!(matches!(
        f.evaluations(&o("tt0000001")).await,
        Err(ErreurScraper::AucunResultat)
    ));
    assert!(matches!(
        f.evaluations(&o("tt0000002")).await,
        Err(ErreurScraper::CleApiInvalide)
    ));
    let e = f.evaluations(&o("tt0000003")).await.unwrap_err();
    assert!(matches!(e, ErreurScraper::Http { statut: 429, .. }));
    // Aucune clé dans les messages d'erreur.
    assert!(!e.to_string().contains("cle-secrete-omdb"));
    assert!(matches!(
        f.evaluations(&OptionsRecherche::default()).await,
        Err(ErreurScraper::IdentifiantManquant(_))
    ));
}

#[tokio::test]
async fn omdb_cle_vide_refusee() {
    assert!(matches!(
        Omdb::new(ConfigOmdb::default(), http()),
        Err(ErreurScraper::CleApiManquante)
    ));
}

fn trakt(s: &MockServer, serie: bool) -> Trakt {
    let mut c = ConfigTrakt::nouvelle("client-id-trakt");
    c.url_api = Some(format!("{}/trakt/", s.uri()));
    c.serie = serie;
    Trakt::new(c, http()).unwrap()
}

#[tokio::test]
async fn trakt_note_et_votes_par_imdb() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/trakt/movies/tt0133093/ratings"))
        .and(header("trakt-api-key", "client-id-trakt"))
        .and(header("trakt-api-version", "2"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"rating": 8.51, "votes": 12345, "distribution": {"10": 5}})),
        )
        .expect(1)
        .mount(&s)
        .await;
    let v = trakt(&s, false).evaluations(&options()).await.unwrap();
    assert_eq!(v.len(), 1);
    assert_eq!(
        (v[0].source.as_str(), v[0].votes, v[0].maximum),
        ("trakt", 12345, 10.0)
    );
    assert!((v[0].valeur - 8.51).abs() < 1e-4);
}

#[tokio::test]
async fn trakt_serie_par_id_trakt_et_erreurs() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/trakt/shows/1390/ratings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"rating": 9.0, "votes": 10})))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/trakt/shows/tt0000404/ratings"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/trakt/shows/tt0000403/ratings"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&s)
        .await;
    let f = trakt(&s, true);
    let o = OptionsRecherche::default().avec_id("trakt", "1390");
    assert_eq!(f.evaluations(&o).await.unwrap()[0].valeur, 9.0);
    let o = |id: &str| OptionsRecherche::default().avec_id("imdb", id);
    assert!(matches!(
        f.evaluations(&o("tt0000404")).await,
        Err(ErreurScraper::AucunResultat)
    ));
    assert!(matches!(
        f.evaluations(&o("tt0000403")).await,
        Err(ErreurScraper::CleApiInvalide)
    ));
    assert!(matches!(
        f.evaluations(&OptionsRecherche::default()).await,
        Err(ErreurScraper::IdentifiantManquant(_))
    ));
}

#[tokio::test]
async fn mdblist_notes_normalisees() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/mdb/imdb/movie/tt0133093"))
        .and(query_param("apikey", "cle-mdb"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "ratings": [
                {"source": "imdb", "value": 8.7, "score": 87, "votes": 2000000},
                {"source": "tomatoes", "value": 83, "score": 83, "votes": null},
                {"source": "metacritic", "value": null, "score": 73},
                {"source": "letterboxd", "value": 4.1, "score": 82},
                {"source": "trakt", "value": null, "score": 85, "votes": 500}
            ]
        })))
        .expect(1)
        .mount(&s)
        .await;
    let mut c = ConfigMdbList::nouvelle("cle-mdb");
    c.url_api = Some(format!("{}/mdb/", s.uri()));
    let f = MdbList::new(c, http()).unwrap();
    let v = f.evaluations(&options()).await.unwrap();
    assert_eq!(v.len(), 4);
    assert_eq!(trouve(&v, "imdb").votes, 2_000_000);
    assert_eq!(trouve(&v, "rottenTomatoes").valeur, 83.0);
    assert_eq!(trouve(&v, "metacritic").valeur, 73.0);
    assert_eq!(trouve(&v, "trakt").valeur, 8.5);
}

#[tokio::test]
async fn test_de_cles() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/simkl/search/id"))
        .and(header("simkl-api-key", "cle-simkl"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/mdb/imdb/movie/tt0133093"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&s)
        .await;
    let h = http();
    tester_cle_service(
        "simkl",
        "cle-simkl",
        &h,
        Some(format!("{}/simkl/", s.uri())),
    )
    .await
    .unwrap();
    assert!(matches!(
        tester_cle_service("mdblist", "mauvaise", &h, Some(format!("{}/mdb/", s.uri()))).await,
        Err(ErreurScraper::CleApiInvalide)
    ));
    assert!(tester_cle_service("tvdb", "x", &h, None).await.is_err());
}
