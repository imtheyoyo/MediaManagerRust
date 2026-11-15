//! Tests du fournisseur Fanart.tv contre un faux serveur HTTP local (aucun appel réseau réel).

use mediamanager_scrapers::*;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CLE_TEST: &str = "cle-projet-factice";

fn scraper(s: &MockServer, cle_client: &str) -> FanartTv {
    let mut cfg = ConfigFanart::nouvelle(CLE_TEST);
    cfg.cle_client = cle_client.into();
    cfg.url_api = Some(format!("{}/v3.2/", s.uri()));
    FanartTv::new(cfg, ClientHttp::new(ConfigHttp::default()).unwrap()).unwrap()
}

fn img(url: &str, lang: &str) -> serde_json::Value {
    json!({"id": "1", "url": url, "lang": lang, "likes": "7"})
}

#[tokio::test]
async fn cle_manquante_refusee() {
    let http = ClientHttp::new(ConfigHttp::default()).unwrap();
    assert!(matches!(
        FanartTv::new(ConfigFanart::default(), http),
        Err(ErreurScraper::CleApiManquante)
    ));
}

#[tokio::test]
async fn film_par_imdb_avec_en_tetes() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3.2/movies/tt1375666"))
        .and(header("api-key", CLE_TEST))
        .and(header("client-key", "cle-perso"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "Inception", "tmdb_id": "27205", "imdb_id": "tt1375666",
            "movieposter": [
                img("https://assets.test/fanart/movies/27205/movieposter/a.jpg", "en"),
                img("https://assets.test/fanart/movies/27205/movieposter/b.jpg", "00")
            ],
            "moviebackground": [{"url": "https://assets.test/fanart/movies/27205/moviebackground/c.jpg", "lang": "", "likes": "3", "width": "3840", "height": "2160"}],
            "moviedisc": [img("https://assets.test/fanart/movies/27205/moviedisc/d.png", "fr")],
            "hdmovielogo": [img("https://assets.test/fanart/movies/27205/hdmovielogo/e.png", "en")],
            "movielogo": [img("https://assets.test/fanart/movies/27205/movielogo/f.png", "en")]
        })))
        .expect(2)
        .mount(&s)
        .await;
    let f = scraper(&s, "cle-perso");
    let o = OptionsRecherche::default()
        .avec_id("imdb", "tt1375666")
        .avec_id("tmdb", "27205");
    let tout = f.artwork_film_etendu(&o, None).await.unwrap();
    let compte = |t| tout.iter().filter(|a| a.type_artwork == t).count();
    assert_eq!(compte(TypeArtworkEtendu::Affiche), 2);
    assert_eq!(compte(TypeArtworkEtendu::ArtCle), 1);
    assert_eq!(compte(TypeArtworkEtendu::Fond), 1);
    assert_eq!(compte(TypeArtworkEtendu::Disque), 1);
    // Anciens logos exclus par défaut.
    assert_eq!(compte(TypeArtworkEtendu::LogoTransparent), 1);
    let fond = tout
        .iter()
        .find(|a| a.type_artwork == TypeArtworkEtendu::Fond)
        .unwrap();
    assert_eq!(fond.tailles[0].largeur, 3840);
    assert_eq!(fond.tailles[0].ordre, 16);
    assert_eq!(fond.votes, 3);
    assert!(fond.url_apercu.contains("/preview/"));
    // Trait de base : types étendus écartés.
    let base = f.artwork(&o, None).await.unwrap();
    assert_eq!(base.len(), 4); // 2 affiches, 1 fond, 1 logo
    assert!(base.iter().all(|a| a.fournisseur == "fanarttv"));
}

#[tokio::test]
async fn film_repli_tmdb_si_imdb_sans_nom() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3.2/movies/tt0000001"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v3.2/movies/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "Film", "movieposter": [img("https://assets.test/fanart/p.jpg", "fr")]
        })))
        .mount(&s)
        .await;
    let f = scraper(&s, "");
    let o = OptionsRecherche::default()
        .avec_id("imdb", "tt0000001")
        .avec_id("tmdb", "42");
    let a = f.artwork(&o, Some(TypeArtwork::Affiche)).await.unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].langue, "fr");
    // Aucun en-tête `client-key` quand la clé personnelle est absente.
    let appels = s.received_requests().await.unwrap();
    assert!(appels.iter().all(|r| !r.headers.contains_key("client-key")));
    assert!(matches!(
        f.artwork(&OptionsRecherche::default(), None).await,
        Err(ErreurScraper::IdentifiantManquant(_))
    ));
}

#[tokio::test]
async fn serie_par_tvdb_et_saisons() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v3.2/tv/121361"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "Game of Thrones",
            "tvposter": [img("https://assets.test/fanart/tv/p.jpg", "en")],
            "tvbanner": [img("https://assets.test/fanart/tv/b.jpg", "en")],
            "seasonposter": [
                {"url": "https://assets.test/fanart/tv/s1.jpg", "lang": "en", "likes": "1", "season": "1"},
                {"url": "https://assets.test/fanart/tv/sall.jpg", "lang": "en", "likes": "1", "season": "all"}
            ],
            "characterart": [img("https://assets.test/fanart/tv/c.png", "")]
        })))
        .mount(&s)
        .await;
    let f = scraper(&s, "");
    let o = OptionsRecherche::default().avec_id("tvdb", "121361");
    let tout = f.artwork_serie(&o, None).await.unwrap();
    assert_eq!(tout.len(), 5);
    let saisons: Vec<_> = tout
        .iter()
        .filter(|a| a.type_artwork == TypeArtworkEtendu::AfficheSaison)
        .map(|a| a.saison)
        .collect();
    assert_eq!(saisons, [Some(1), Some(0)]);
    let bannieres = f
        .artwork_serie(&o, Some(TypeArtworkEtendu::Banniere))
        .await
        .unwrap();
    assert_eq!(bannieres.len(), 1);
    assert_eq!(bannieres[0].tailles[0].hauteur, 185);
    // Sans identifiant TVDB : liste vide, aucun appel.
    let vide = f
        .artwork_serie(&OptionsRecherche::default().avec_id("tmdb", "1399"), None)
        .await
        .unwrap();
    assert!(vide.is_empty());
}

#[tokio::test]
async fn cle_refusee() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&s)
        .await;
    let f = scraper(&s, "");
    assert!(matches!(
        f.artwork_serie(&OptionsRecherche::default().avec_id("tvdb", "1"), None)
            .await,
        Err(ErreurScraper::CleApiInvalide)
    ));
}
