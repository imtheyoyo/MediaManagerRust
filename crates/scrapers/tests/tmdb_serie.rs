//! Tests du scraper TMDB séries contre un faux serveur HTTP local (aucun appel réseau réel).

use std::time::Duration;

use mediamanager_scrapers::*;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CLE_TEST: &str = "cle-de-test-factice";

async fn serveur() -> MockServer {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/3/configuration"))
        .and(query_param("api_key", CLE_TEST))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"images": {"secure_base_url": "https://img.test/t/p/"}})),
        )
        .mount(&s)
        .await;
    s
}

fn scraper(s: &MockServer) -> TmdbSerie {
    let mut cfg = ConfigTmdb::nouvelle(CLE_TEST);
    cfg.url_api = Some(format!("{}/3/", s.uri()));
    let http = ClientHttp::new(ConfigHttp {
        delai_429_defaut: Duration::from_millis(10),
        marge_429: Duration::from_millis(5),
        ..ConfigHttp::default()
    })
    .unwrap();
    TmdbSerie::new(cfg, http).unwrap()
}

fn detail_serie() -> serde_json::Value {
    json!({
        "id": 1399, "name": "Le Trône de fer", "original_name": "Game of Thrones", "original_language": "en",
        "overview": "Résumé.", "tagline": "L'hiver vient.", "first_air_date": "2011-04-17", "last_air_date": "2019-05-19",
        "episode_run_time": [60], "origin_country": ["US"], "status": "Ended",
        "networks": [{"name": "HBO"}], "production_companies": [{"name": "Bighead"}, {"name": "HBO"}],
        "genres": [{"id": 10765, "name": "Sci-Fi & Fantasy"}, {"id": 18, "name": "Drame"}],
        "vote_average": 8.4, "vote_count": 20000, "poster_path": "/p.jpg", "number_of_seasons": 1,
        "seasons": [
            {"id": 3627, "season_number": 1, "name": "Saison 1", "overview": "", "air_date": "2011-04-17", "episode_count": 2, "poster_path": "/s1.jpg"}
        ],
        "credits": {
            "cast": [{"id": 22970, "name": "Peter Dinklage", "character": "Tyrion", "profile_path": "/pd.jpg"}],
            "crew": [{"id": 9813, "name": "David Benioff", "job": "Executive Producer", "department": "Production"}]
        },
        "external_ids": {"imdb_id": "tt0944947", "tvdb_id": 121361, "tvrage_id": 24493, "wikidata_id": "Q23572"},
        "content_ratings": {"results": [{"iso_3166_1": "US", "rating": "TV-MA"}, {"iso_3166_1": "FR", "rating": "16"}]},
        "keywords": {"results": [{"name": "dragon"}]},
        "translations": {"translations": [{"iso_639_1": "en", "iso_3166_1": "US", "data": {"name": "Game of Thrones", "overview": "Overview."}}]},
        "episode_groups": {"results": [
            {"id": "grp-dvd", "name": "DVD", "type": 3, "episode_count": 2},
            {"id": "grp-vide", "name": "Vide", "type": 1, "episode_count": 0},
            {"id": "grp-inconnu", "name": "Inconnu", "type": 42, "episode_count": 2}
        ]}
    })
}

async fn monter_detail(s: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/3/tv/1399"))
        .and(query_param(
            "append_to_response",
            "translations,credits,external_ids,content_ratings,keywords,episode_groups",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(detail_serie()))
        .mount(s)
        .await;
    Mock::given(method("GET"))
        .and(path("/3/tv/episode_group/grp-dvd"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "grp-dvd", "name": "DVD", "type": 3,
            "groups": [
                {"name": "Disque vide", "order": 0, "episodes": []},
                {"name": "Volume 1", "order": 1, "episodes": [{"id": 64, "order": 1}, {"id": 63, "order": 0}]}
            ]
        })))
        .expect(1..)
        .mount(s)
        .await;
}

async fn monter_saison(s: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/3/tv/1399/season/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "episodes": [
                {"id": 63, "name": "L'hiver vient", "overview": "Ép. 1", "air_date": "2011-04-17",
                 "season_number": 1, "episode_number": 1, "runtime": 62, "vote_average": 7.9, "vote_count": 100,
                 "still_path": "/e1.jpg", "guest_stars": [{"id": 5, "name": "Invité Un", "character": "Garde"}],
                 "crew": [{"id": 6, "name": "Tim Van Patten", "job": "Director", "department": "Directing"}]},
                {"id": 64, "name": "Épisode 2", "overview": "", "air_date": "2011-04-24",
                 "season_number": 1, "episode_number": 2, "vote_count": 0}
            ]
        })))
        .mount(s)
        .await;
}

#[tokio::test]
async fn cle_manquante_refusee() {
    let http = ClientHttp::new(ConfigHttp::default()).unwrap();
    assert!(matches!(
        TmdbSerie::new(ConfigTmdb::nouvelle(""), http),
        Err(ErreurScraper::CleApiManquante)
    ));
}

#[tokio::test]
async fn recherche_texte_triee() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/search/tv"))
        .and(query_param("query", "Game of Thrones"))
        .and(query_param("language", "fr-FR"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "page": 1, "total_pages": 1,
            "results": [
                {"id": 2, "name": "Thrones", "original_name": "Thrones", "first_air_date": "2020-01-01"},
                {"id": 1399, "name": "Le Trône de fer", "original_name": "Game of Thrones",
                 "first_air_date": "2011-04-17", "poster_path": "/p.jpg"}
            ]
        })))
        .expect(1)
        .mount(&s)
        .await;
    let t = scraper(&s);
    let mut o = OptionsRecherche::avec_requete("Game of Thrones");
    o.langue = "fr".into();
    o.annee = Some(2011);
    let r = FournisseurSerie::rechercher(&t, &o).await.unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].ids["tmdb"], "1399");
    assert_eq!(r[0].annee, 2011);
    assert_eq!(r[0].url_affiche, "https://img.test/t/p/w342/p.jpg");
    assert!((r[0].score - 1.0).abs() < 1e-6);
    assert!(correspondance_serie::choisir_meilleure_serie(&r, SEUIL_SERIE).is_some());
}

#[tokio::test]
async fn recherche_par_tvdb_via_find() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/find/121361"))
        .and(query_param("external_source", "tvdb_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tv_results": [{"id": 1399, "name": "Game of Thrones"}]
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/3/tv/1399"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 1399, "name": "Game of Thrones", "first_air_date": "2011-04-17"
        })))
        .mount(&s)
        .await;
    let t = scraper(&s);
    let o = OptionsRecherche::avec_requete("autre chose").avec_id("tvdb", "121361");
    let r = FournisseurSerie::rechercher(&t, &o).await.unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].score, 1.0);
}

#[tokio::test]
async fn detail_serie_complet() {
    let s = serveur().await;
    monter_detail(&s).await;
    let t = scraper(&s);
    let mut o = OptionsRecherche::default().avec_id("tmdb", "1399");
    o.langue = "fr".into();
    let m = t.metadonnees_serie(&o).await.unwrap();
    assert_eq!(m.titre, "Le Trône de fer");
    assert_eq!(m.titre_original, "Game of Thrones");
    assert_eq!(m.titre_anglais, "Game of Thrones");
    assert_eq!((m.annee, m.annee_fin), (2011, 2019));
    assert_eq!(m.statut, StatutDiffusion::Terminee);
    assert_eq!(m.duree, 60);
    assert_eq!(m.ids["imdb"], "tt0944947");
    assert_eq!(m.ids["tvdb"], "121361");
    assert_eq!(m.ids["tvrage"], "24493");
    assert_eq!(m.societes_production, ["HBO", "Bighead"]);
    assert_eq!(m.genres, ["SCIENCE_FICTION", "DRAMA"]);
    assert_eq!(m.classifications.len(), 1);
    assert_eq!(m.classifications[0].valeur, "TV-MA");
    assert_eq!(m.mots_cles, ["dragon"]);
    assert_eq!(m.evaluations[0].votes, 20000);
    assert!(
        m.personnes
            .iter()
            .any(|p| p.nom == "Peter Dinklage" && p.role == "Tyrion")
    );
    assert!(
        m.personnes
            .iter()
            .any(|p| p.nom == "David Benioff" && p.type_personne == TypePersonne::Producteur)
    );
    // Groupes : diffusion + DVD (vide et type inconnu ignorés).
    assert_eq!(m.groupes_episodes.len(), 2);
    assert_eq!(m.groupes_episodes[1].type_groupe, TypeGroupeEpisodes::Dvd);
    let dvd = m.groupes_episodes[1].clone();
    let saisons_dvd: Vec<_> = m.saisons_du_groupe(&dvd).collect();
    assert_eq!(saisons_dvd.len(), 1);
    assert_eq!(saisons_dvd[0].nom, "Volume 1");
    let diffusion = GroupeEpisodes::diffusion();
    let s1: Vec<_> = m.saisons_du_groupe(&diffusion).collect();
    assert_eq!(s1[0].url_affiche, "https://img.test/t/p/w342/s1.jpg");
}

#[tokio::test]
async fn liste_episodes_avec_ordres_et_cache() {
    let s = serveur().await;
    monter_detail(&s).await;
    monter_saison(&s).await;
    let t = scraper(&s);
    let o = OptionsRecherche::default().avec_id("tmdb", "1399");
    let l = t.liste_episodes(&o).await.unwrap();
    assert_eq!(l.len(), 2);
    let e1 = &l[0];
    assert_eq!(e1.titre, "L'hiver vient");
    assert_eq!(e1.numero_diffusion(), Some((1, 1)));
    assert_eq!(e1.numero_dvd(), Some((1, 1)));
    assert_eq!(l[1].numero_dvd(), Some((1, 2)));
    assert_eq!(e1.duree, 62);
    assert_eq!(e1.evaluations.len(), 1);
    assert!(l[1].evaluations.is_empty());
    assert!(
        e1.personnes
            .iter()
            .any(|p| p.type_personne == TypePersonne::Invite && p.nom == "Invité Un")
    );
    assert!(
        e1.personnes
            .iter()
            .any(|p| p.type_personne == TypePersonne::Realisateur)
    );
    assert_eq!(e1.artwork[0].type_artwork, TypeArtworkEtendu::Vignette);
    // Deuxième appel servi par le cache mémoire (groupe DVD appelé une seule fois de plus au total).
    let l2 = t.liste_episodes(&o).await.unwrap();
    assert_eq!(l, l2);
    let appels = s.received_requests().await.unwrap();
    let n_saison = appels
        .iter()
        .filter(|r| r.url.path() == "/3/tv/1399/season/1")
        .count();
    assert_eq!(n_saison, 1);
}

#[tokio::test]
async fn repli_des_titres_generiques() {
    let s = serveur().await;
    monter_detail(&s).await;
    monter_saison(&s).await;
    Mock::given(method("GET"))
        .and(path("/3/tv/1399/season/1"))
        .and(query_param("language", "en-US"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "episodes": [{"id": 64, "name": "The Kingsroad", "overview": "Overview 2", "season_number": 1, "episode_number": 2}]
        })))
        .with_priority(1)
        .mount(&s)
        .await;
    let mut cfg = ConfigTmdb::nouvelle(CLE_TEST);
    cfg.url_api = Some(format!("{}/3/", s.uri()));
    cfg.repli_titre = true;
    let t = TmdbSerie::new(cfg, ClientHttp::new(ConfigHttp::default()).unwrap()).unwrap();
    let mut o = OptionsRecherche::default().avec_id("tmdb", "1399");
    o.langue = "fr".into();
    let l = t.liste_episodes(&o).await.unwrap();
    assert_eq!(l[1].titre, "The Kingsroad");
    assert_eq!(l[1].resume, "Overview 2");
    assert_eq!(l[0].titre, "L'hiver vient");
}

fn detail_episode() -> serde_json::Value {
    json!({
        "id": 64, "name": "La Route royale", "overview": "Résumé 2", "air_date": "2011-04-24",
        "season_number": 1, "episode_number": 2, "runtime": 56, "vote_average": 7.8, "vote_count": 50,
        "external_ids": {"imdb_id": "tt1668746", "tvdb_id": 3436411},
        "credits": {"cast": [{"id": 1, "name": "Sean Bean", "character": "Ned"}], "crew": []},
        "images": {"stills": [
            {"file_path": "/st-en.jpg", "width": 1920, "height": 1080, "iso_639_1": null, "vote_average": 5.0, "vote_count": 3}
        ]}
    })
}

#[tokio::test]
async fn episode_par_numero_de_diffusion_direct() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/tv/1399/season/1/episode/2"))
        .and(query_param(
            "append_to_response",
            "external_ids,credits,images",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(detail_episode()))
        .expect(1)
        .mount(&s)
        .await;
    let t = scraper(&s);
    let o = OptionsEpisode::diffusion(OptionsRecherche::default().avec_id("tmdb", "1399"), 1, 2);
    let e = t.metadonnees_episode(&o).await.unwrap();
    assert_eq!(e.titre, "La Route royale");
    assert_eq!(e.ids["imdb"], "tt1668746");
    assert_eq!(e.ids["tvdb"], "3436411");
    assert_eq!(e.duree, 56);
    assert_eq!(e.date_diffusion.as_deref(), Some("2011-04-24"));
    assert_eq!(e.artwork.len(), 1);
    assert_eq!(
        e.artwork[0].url_apercu,
        "https://img.test/t/p/w300/st-en.jpg"
    );
    assert_eq!(e.artwork[0].tailles.len(), 3);
}

#[tokio::test]
async fn episode_par_ordre_dvd_via_liste() {
    let s = serveur().await;
    monter_detail(&s).await;
    monter_saison(&s).await;
    Mock::given(method("GET"))
        .and(path("/3/tv/1399/season/1/episode/2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(detail_episode()))
        .mount(&s)
        .await;
    let t = scraper(&s);
    let o = OptionsEpisode {
        serie: OptionsRecherche::default().avec_id("tmdb", "1399"),
        groupe: GroupeEpisodes::nouveau(TypeGroupeEpisodes::Dvd, "DVD"),
        saison: Some(1),
        episode: Some(2),
        ..Default::default()
    };
    let e = t.metadonnees_episode(&o).await.unwrap();
    assert_eq!(e.titre, "La Route royale");
    assert_eq!(e.numero_dvd(), Some((1, 2)));
    assert_eq!(e.numero_diffusion(), Some((1, 2)));
}

#[tokio::test]
async fn episode_par_date_et_par_imdb() {
    let s = serveur().await;
    monter_detail(&s).await;
    monter_saison(&s).await;
    Mock::given(method("GET"))
        .and(path("/3/tv/1399/season/1/episode/2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(detail_episode()))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/3/find/tt1668746"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "tv_episode_results": [{"id": 64, "show_id": 1399, "season_number": 1, "episode_number": 2}]
        })))
        .mount(&s)
        .await;
    let t = scraper(&s);
    let o = OptionsEpisode {
        serie: OptionsRecherche::default().avec_id("tmdb", "1399"),
        date_diffusion: Some("2011-04-24".into()),
        ..Default::default()
    };
    assert_eq!(
        t.metadonnees_episode(&o).await.unwrap().titre,
        "La Route royale"
    );
    let mut o = OptionsEpisode {
        serie: OptionsRecherche::default(),
        ..Default::default()
    };
    o.ids_episode.insert("imdb".into(), "tt1668746".into());
    assert_eq!(
        t.metadonnees_episode(&o).await.unwrap().titre,
        "La Route royale"
    );
    // Rien d'exploitable.
    let o = OptionsEpisode {
        serie: OptionsRecherche::default().avec_id("tmdb", "1399"),
        ..Default::default()
    };
    assert!(matches!(
        t.metadonnees_episode(&o).await,
        Err(ErreurScraper::IdentifiantManquant(_))
    ));
}

#[tokio::test]
async fn episode_introuvable() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/tv/1399/season/9/episode/9"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&s)
        .await;
    let t = scraper(&s);
    let o = OptionsEpisode::diffusion(OptionsRecherche::default().avec_id("tmdb", "1399"), 9, 9);
    assert!(matches!(
        t.metadonnees_episode(&o).await,
        Err(ErreurScraper::AucunResultat)
    ));
}

fn images(chemin: &str) -> serde_json::Value {
    json!({
        "posters": [
            {"file_path": format!("/{chemin}-en.jpg"), "width": 1000, "height": 1500, "iso_639_1": "en", "vote_average": 5.0, "vote_count": 2},
            {"file_path": format!("/{chemin}-fr.jpg"), "width": 2000, "height": 3000, "iso_639_1": "fr", "vote_average": 1.0, "vote_count": 1}
        ],
        "backdrops": [{"file_path": format!("/{chemin}-bg.jpg"), "width": 3840, "height": 2160, "iso_639_1": null}],
        "logos": [{"file_path": format!("/{chemin}-logo.svg"), "width": 500, "height": 100, "iso_639_1": "en"}]
    })
}

#[tokio::test]
async fn artwork_serie_et_saisons() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/tv/1399"))
        .and(query_param("append_to_response", "images"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 1399, "number_of_seasons": 1, "images": images("serie")
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/3/tv/1399/season/0/images"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/3/tv/1399/season/1/images"))
        .respond_with(ResponseTemplate::new(200).set_body_json(images("s1")))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/3/tv/1399/images"))
        .respond_with(ResponseTemplate::new(200).set_body_json(images("serie")))
        .mount(&s)
        .await;
    let t = scraper(&s);
    let mut o = OptionsRecherche::default().avec_id("tmdb", "1399");
    o.langue = "fr".into();
    let tout = t.artwork_serie(&o, None).await.unwrap();
    let affiches: Vec<_> = tout
        .iter()
        .filter(|a| a.type_artwork == TypeArtworkEtendu::Affiche)
        .collect();
    assert_eq!(
        affiches[0].url_originale,
        "https://img.test/t/p/original/serie-fr.jpg"
    );
    let saisons: Vec<_> = tout
        .iter()
        .filter(|a| a.type_artwork == TypeArtworkEtendu::AfficheSaison)
        .collect();
    assert_eq!(saisons.len(), 2);
    assert!(saisons.iter().all(|a| a.saison == Some(1)));
    assert!(
        tout.iter()
            .any(|a| a.type_artwork == TypeArtworkEtendu::LogoTransparent
                && a.url_originale.ends_with("serie-logo.png"))
    );
    // Filtre sur un type de base : un seul appel `images`.
    let fonds = t
        .artwork_serie(&o, Some(TypeArtworkEtendu::Fond))
        .await
        .unwrap();
    assert_eq!(fonds.len(), 1);
    assert_eq!(fonds[0].saison, None);
    assert!(
        t.artwork_serie(&o, Some(TypeArtworkEtendu::Banniere))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn artwork_episode() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/tv/1399/season/1/episode/2/images"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "stills": [{"file_path": "/x.jpg", "width": 1280, "height": 720}]
        })))
        .mount(&s)
        .await;
    let t = scraper(&s);
    let o = OptionsEpisode::diffusion(OptionsRecherche::default().avec_id("tmdb", "1399"), 1, 2);
    let a = t.artwork_episode(&o).await.unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].type_artwork, TypeArtworkEtendu::Vignette);
    assert_eq!(a[0].tailles.len(), 2);
}

#[tokio::test]
async fn bandes_annonces_serie() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/tv/1399/videos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {"name": "Bande-annonce", "key": "abc", "site": "YouTube", "size": 1080, "type": "Trailer",
                 "iso_639_1": "fr", "iso_3166_1": "FR", "published_at": "2011-03-01T00:00:00.000Z"},
                {"name": "Coulisses", "key": "def", "site": "YouTube", "size": 720, "type": "Featurette"}
            ]
        })))
        .expect(2)
        .mount(&s)
        .await;
    let t = scraper(&s);
    let b = t
        .bandes_annonces(&OptionsRecherche::default().avec_id("tmdb", "1399"))
        .await
        .unwrap();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].url, "https://www.youtube.com/watch?v=abc&hd=1");
    assert_eq!(b[0].langue, "fr-FR");
}

#[tokio::test]
async fn identifiant_manquant() {
    let s = serveur().await;
    let t = scraper(&s);
    assert!(matches!(
        t.metadonnees_serie(&OptionsRecherche::avec_requete("x"))
            .await,
        Err(ErreurScraper::IdentifiantManquant(_))
    ));
}
