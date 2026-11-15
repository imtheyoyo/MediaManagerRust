//! Tests du scraper TMDB contre un faux serveur HTTP local (aucun appel réseau réel).

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

fn scraper(s: &MockServer, cache: Option<std::path::PathBuf>) -> TmdbFilm {
    let mut cfg = ConfigTmdb::nouvelle(CLE_TEST);
    cfg.url_api = Some(format!("{}/3/", s.uri()));
    let http = ClientHttp::new(ConfigHttp {
        dossier_cache: cache,
        delai_429_defaut: Duration::from_millis(10),
        marge_429: Duration::from_millis(5),
        ..ConfigHttp::default()
    })
    .unwrap();
    TmdbFilm::new(cfg, http).unwrap()
}

#[tokio::test]
async fn cle_manquante_refusee() {
    let http = ClientHttp::new(ConfigHttp::default()).unwrap();
    assert!(matches!(
        TmdbFilm::new(ConfigTmdb::nouvelle("  "), http),
        Err(ErreurScraper::CleApiManquante)
    ));
}

#[tokio::test]
async fn debug_masque_la_cle() {
    let c = ConfigTmdb::nouvelle(CLE_TEST);
    assert!(!format!("{c:?}").contains(CLE_TEST));
}

#[tokio::test]
async fn recherche_par_texte_triee_par_score() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/search/movie"))
        .and(query_param("query", "Inception"))
        .and(query_param("language", "fr-FR"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "page": 1, "total_pages": 1,
            "results": [
                {"id": 1, "title": "Inception: les coulisses", "original_title": "Inception Making Of", "release_date": "2011-01-01", "poster_path": null},
                {"id": 27205, "title": "Inception", "original_title": "Inception", "original_language": "en",
                 "overview": "Un voleur...", "release_date": "2010-07-15", "poster_path": "/abc.jpg"}
            ]
        })))
        .expect(1)
        .mount(&s)
        .await;
    let t = scraper(&s, None);
    let mut o = OptionsRecherche::avec_requete("Inception");
    o.langue = "fr".into();
    o.annee = Some(2010);
    let r = t.rechercher(&o).await.unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].ids["tmdb"], "27205");
    assert_eq!(r[0].annee, 2010);
    assert_eq!(r[0].url_affiche, "https://img.test/t/p/w342/abc.jpg");
    assert!(r[0].score > r[1].score);
    assert!((r[0].score - 1.0).abs() < 1e-6);
}

#[tokio::test]
async fn recherche_par_id_imdb_score_force() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/find/tt1375666"))
        .and(query_param("external_source", "imdb_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "movie_results": [{"id": 27205, "title": "Inception", "release_date": "2010-07-15"}]
        })))
        .mount(&s)
        .await;
    let t = scraper(&s, None);
    let o = OptionsRecherche::avec_requete("n'importe quoi").avec_id("imdb", "tt1375666");
    let r = t.rechercher(&o).await.unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].score, 1.0);
}

#[tokio::test]
async fn recherche_pagination_limitee_a_cinq_pages() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/search/movie"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "page": 1, "total_pages": 50, "results": [{"id": 9, "title": "Dune"}]
        })))
        .expect(5)
        .mount(&s)
        .await;
    let t = scraper(&s, None);
    let r = t
        .rechercher(&OptionsRecherche::avec_requete("Dune"))
        .await
        .unwrap();
    assert_eq!(r.len(), 1); // même id dédoublonné
}

fn detail_inception() -> serde_json::Value {
    json!({
        "id": 27205, "title": "Origine", "original_title": "Inception", "original_language": "en",
        "overview": "Résumé.", "tagline": "Votre esprit est la scène du crime.", "runtime": 148,
        "release_date": "2010-07-15", "vote_average": 8.4, "vote_count": 35000, "poster_path": "/p.jpg", "adult": false,
        "genres": [{"id": 28, "name": "Action"}, {"id": 878, "name": "Science-Fiction"}, {"id": 99999, "name": "Inconnu"}],
        "spoken_languages": [{"iso_639_1": "en", "name": "English"}],
        "production_countries": [{"iso_3166_1": "US", "name": "United States of America"}],
        "production_companies": [{"name": " Legendary Pictures "}],
        "belongs_to_collection": {"id": 10, "name": "Saga Inception"},
        "credits": {
            "cast": [{"id": 6193, "name": "Leonardo DiCaprio", "character": "Cobb", "profile_path": "/l.jpg"}],
            "crew": [
                {"id": 525, "name": "Christopher Nolan", "job": "Director", "department": "Directing"},
                {"id": 526, "name": "Hans Zimmer", "job": "Original Music Composer", "department": "Sound"},
                {"id": 527, "name": "Quelqu'un", "job": "Gaffer", "department": "Lighting"}
            ]
        },
        "keywords": {"keywords": [{"name": "dream"}, {"name": "heist"}]},
        "release_dates": {"results": [
            {"iso_3166_1": "US", "release_dates": [{"certification": "PG-13", "release_date": "2010-07-16T00:00:00.000Z", "type": 3}]},
            {"iso_3166_1": "FR", "release_dates": [
                {"certification": "", "release_date": "2010-07-10T00:00:00.000Z", "type": 1},
                {"certification": "TP", "release_date": "2010-07-21T00:00:00.000Z", "type": 3},
                {"certification": "", "release_date": "2010-12-01T00:00:00.000Z", "type": 4}
            ]}
        ]},
        "translations": {"translations": [{"iso_639_1": "en", "iso_3166_1": "US", "data": {"title": "Inception", "overview": "Overview"}}]},
        "external_ids": {"imdb_id": "tt1375666", "wikidata_id": "Q25188", "tvdb_id": 0}
    })
}

#[tokio::test]
async fn metadonnees_completes() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/movie/27205"))
        .and(query_param("language", "fr-FR"))
        .and(query_param(
            "append_to_response",
            "credits,keywords,release_dates,translations,external_ids",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(detail_inception()))
        .expect(1)
        .mount(&s)
        .await;
    let t = scraper(&s, None);
    let mut o = OptionsRecherche::default().avec_id("tmdb", "27205");
    o.langue = "fr".into();
    o.pays_classification = "FR".into();
    o.pays_sortie = "FR".into();
    let m = t.metadonnees(&o).await.unwrap();
    assert_eq!(m.titre, "Origine");
    assert_eq!(m.titre_original, "Inception");
    assert_eq!(m.titre_anglais, "Inception");
    assert_eq!(m.duree, 148);
    assert_eq!(m.annee, 2010);
    assert_eq!(m.ids["imdb"], "tt1375666");
    assert_eq!(m.ids["wikidata"], "Q25188");
    assert!(!m.ids.contains_key("tvdb"));
    assert_eq!(m.ids["tmdbSet"], "10");
    assert_eq!(m.nom_collection, "Saga Inception");
    assert_eq!(m.genres, ["ACTION", "SCIENCE_FICTION", "Inconnu"]);
    assert_eq!(m.societes_production, ["Legendary Pictures"]);
    assert_eq!(m.mots_cles, ["dream", "heist"]);
    assert_eq!(
        m.classifications,
        [Classification {
            pays: "FR".into(),
            valeur: "TP".into()
        }]
    );
    // Date locale : la plus ancienne de type > 1 ou 1 (avant-première) -> 2010-07-10.
    assert_eq!(m.date_sortie.as_deref(), Some("2010-07-10"));
    assert_eq!(m.evaluations[0].maximum, 10.0);
    assert_eq!(m.artwork[0].url_apercu, "https://img.test/t/p/w342/p.jpg");
    let acteur = m
        .personnes
        .iter()
        .find(|p| p.type_personne == TypePersonne::Acteur)
        .unwrap();
    assert_eq!(acteur.role, "Cobb");
    assert_eq!(acteur.url_photo, "https://img.test/t/p/h632/l.jpg");
    assert_eq!(acteur.url_profil, "https://www.themoviedb.org/person/6193");
    assert!(
        m.personnes
            .iter()
            .any(|p| p.type_personne == TypePersonne::Realisateur && p.nom == "Christopher Nolan")
    );
    assert!(
        m.personnes
            .iter()
            .any(|p| p.type_personne == TypePersonne::Compositeur)
    );
    assert_eq!(m.personnes.len(), 3);
}

#[tokio::test]
async fn metadonnees_sans_identifiant() {
    let s = serveur().await;
    let t = scraper(&s, None);
    let e = t
        .metadonnees(&OptionsRecherche::avec_requete("x"))
        .await
        .unwrap_err();
    assert!(matches!(e, ErreurScraper::IdentifiantManquant(_)));
}

#[tokio::test]
async fn metadonnees_via_imdb() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/find/tt1375666"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"movie_results": [{"id": 27205}]})),
        )
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/3/movie/27205"))
        .respond_with(ResponseTemplate::new(200).set_body_json(detail_inception()))
        .mount(&s)
        .await;
    let t = scraper(&s, None);
    let m = t
        .metadonnees(&OptionsRecherche::default().avec_id("imdb", "tt1375666"))
        .await
        .unwrap();
    assert_eq!(m.ids["tmdb"], "27205");
}

#[tokio::test]
async fn cle_invalide_sur_401() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/3/configuration"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&s)
        .await;
    let t = scraper(&s, None);
    let e = t
        .rechercher(&OptionsRecherche::avec_requete("x"))
        .await
        .unwrap_err();
    assert!(matches!(e, ErreurScraper::CleApiInvalide));
}

#[tokio::test]
async fn reprise_apres_429_avec_retry_after() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/movie/1/videos"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .up_to_n_times(1)
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/3/movie/1/videos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"results": []})))
        .mount(&s)
        .await;
    let t = scraper(&s, None);
    let r = t
        .bandes_annonces(&OptionsRecherche::default().avec_id("tmdb", "1"))
        .await
        .unwrap();
    assert!(r.is_empty());
}

#[tokio::test]
async fn abandon_apres_trois_429() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/movie/1/videos"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .expect(3)
        .mount(&s)
        .await;
    let t = scraper(&s, None);
    let e = t
        .bandes_annonces(&OptionsRecherche::default().avec_id("tmdb", "1"))
        .await
        .unwrap_err();
    match e {
        ErreurScraper::Http { statut, url } => {
            assert_eq!(statut, 429);
            assert!(!url.contains("api_key"));
        }
        autre => panic!("erreur inattendue : {autre:?}"),
    }
}

#[tokio::test]
async fn retry_after_trop_long_abandonne_tout_de_suite() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/movie/1/videos"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "120"))
        .expect(1)
        .mount(&s)
        .await;
    let t = scraper(&s, None);
    let e = t
        .bandes_annonces(&OptionsRecherche::default().avec_id("tmdb", "1"))
        .await
        .unwrap_err();
    assert!(matches!(e, ErreurScraper::TropDeRequetes { secondes: 120 }));
}

#[tokio::test]
async fn cache_disque_evite_le_second_appel() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/movie/27205"))
        .respond_with(ResponseTemplate::new(200).set_body_json(detail_inception()))
        .expect(1)
        .mount(&s)
        .await;
    let dossier = std::env::temp_dir().join(format!("mm-scrapers-cache-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dossier);
    let t = scraper(&s, Some(dossier.clone()));
    let o = OptionsRecherche::default().avec_id("tmdb", "27205");
    t.metadonnees(&o).await.unwrap();
    t.metadonnees(&o).await.unwrap();
    // Aucune entrée de cache ne doit contenir la clé.
    for f in std::fs::read_dir(&dossier).unwrap() {
        let f = f.unwrap();
        assert!(!f.file_name().to_string_lossy().contains(CLE_TEST));
    }
    let _ = std::fs::remove_dir_all(&dossier);
}

#[tokio::test]
async fn artwork_trie_et_tailles() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/movie/27205/images"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "posters": [
                {"file_path": "/en.jpg", "width": 2000, "height": 3000, "iso_639_1": "en", "vote_average": 5.0, "vote_count": 10},
                {"file_path": "/fr.jpg", "width": 1000, "height": 1500, "iso_639_1": "fr", "vote_average": 1.0, "vote_count": 2},
                {"file_path": "/xx.jpg", "width": 600, "height": 900, "iso_639_1": null, "vote_average": 9.0, "vote_count": 1}
            ],
            "backdrops": [{"file_path": "/b.jpg", "width": 1920, "height": 1080, "iso_639_1": null, "vote_average": 3.0, "vote_count": 4}],
            "logos": [{"file_path": "/l.svg", "width": 1000, "height": 500, "iso_639_1": "en", "vote_average": 1.0, "vote_count": 1}]
        })))
        .mount(&s)
        .await;
    let t = scraper(&s, None);
    let mut o = OptionsRecherche::default().avec_id("tmdb", "27205");
    o.langue = "fr".into();
    let tout = t.artwork(&o, None).await.unwrap();
    let affiches: Vec<_> = tout
        .iter()
        .filter(|a| a.type_artwork == TypeArtwork::Affiche)
        .collect();
    assert_eq!(affiches.len(), 3);
    assert_eq!(affiches[0].langue, "fr");
    assert_eq!(affiches[1].langue, "en");
    assert_eq!(affiches[2].langue, "");
    // Tailles de l'affiche anglaise 2000x3000 : original + 780, 500, 342, 185.
    assert_eq!(affiches[1].tailles.len(), 5);
    assert_eq!(affiches[1].tailles[0].ordre, 16);
    assert_eq!(
        affiches[1].tailles[1].url,
        "https://img.test/t/p/w780/en.jpg"
    );
    assert_eq!(affiches[1].tailles[1].hauteur, 1170);
    let fond = tout
        .iter()
        .find(|a| a.type_artwork == TypeArtwork::Fond)
        .unwrap();
    assert_eq!(fond.url_apercu, "https://img.test/t/p/w1280/b.jpg");
    assert_eq!(fond.tailles.len(), 3);
    let logo = tout
        .iter()
        .find(|a| a.type_artwork == TypeArtwork::LogoTransparent)
        .unwrap();
    assert!(logo.url_originale.ends_with("/original/l.png"));
    assert_eq!(logo.tailles[0].largeur, 2000);
    let seuls_fonds = t.artwork(&o, Some(TypeArtwork::Fond)).await.unwrap();
    assert_eq!(seuls_fonds.len(), 1);
}

#[tokio::test]
async fn bandes_annonces_filtrees_et_dedoublonnees() {
    let s = serveur().await;
    let corps = json!({"results": [
        {"name": "Bande-annonce VF", "key": "AAA", "site": "YouTube", "size": 1080, "type": "Trailer", "iso_639_1": "fr", "iso_3166_1": "FR", "published_at": "2010-05-01T10:00:00.000Z"},
        {"name": "Teaser", "key": "BBB", "site": "YouTube", "size": 1080, "type": "Teaser", "iso_639_1": "fr", "iso_3166_1": "FR"},
        {"name": "Apple", "key": "CCC", "site": "Apple", "size": 1080, "type": "Trailer"},
        {"name": "SD", "key": "DDD", "site": "YouTube", "size": 480, "type": "Trailer", "iso_639_1": "en", "iso_3166_1": "US"}
    ]});
    Mock::given(method("GET"))
        .and(path("/3/movie/27205/videos"))
        .respond_with(ResponseTemplate::new(200).set_body_json(corps))
        .expect(2)
        .mount(&s)
        .await;
    let t = scraper(&s, None);
    let mut o = OptionsRecherche::default().avec_id("tmdb", "27205");
    o.langue = "fr".into();
    let v = t.bandes_annonces(&o).await.unwrap();
    // Les deux appels (langue puis vide) renvoient la même liste : dédoublonnage.
    assert_eq!(v.len(), 2);
    assert_eq!(v[0].url, "https://www.youtube.com/watch?v=AAA&hd=1");
    assert_eq!(v[0].qualite, "1080p");
    assert_eq!(v[0].langue, "fr-FR");
    assert_eq!(v[0].date, "2010-05-01");
    assert_eq!(v[1].url, "https://www.youtube.com/watch?v=DDD");
}

#[tokio::test]
async fn collection_ignore_les_films_a_venir() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/collection/10"))
        .and(query_param("language", "fr-FR"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 10, "name": "Saga", "overview": "Résumé", "poster_path": "/c.jpg", "backdrop_path": "/cb.jpg",
            "parts": [
                {"id": 1, "title": "Un", "release_date": "2001-01-01", "poster_path": "/1.jpg"},
                {"id": 2, "title": "Futur", "release_date": "2999-01-01"},
                {"id": 3, "title": "Sans date", "release_date": ""}
            ]
        })))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/3/collection/10"))
        .and(query_param("language", "en-US"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": 10, "name": "Saga EN"})),
        )
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/3/movie/1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"id": 1, "title": "Un", "release_date": "2001-01-01"})),
        )
        .mount(&s)
        .await;
    let t = scraper(&s, None);
    let mut o = OptionsRecherche::default().avec_id("tmdbSet", "10");
    o.langue = "fr".into();
    let c = t.collection(&o).await.unwrap();
    assert_eq!(c.titre, "Saga");
    assert_eq!(c.titre_anglais, "Saga EN");
    assert_eq!(c.url_fond, "https://img.test/t/p/w1280/cb.jpg");
    assert_eq!(c.parties.len(), 1);
    assert!(c.parties[0].metadonnees.is_none());
    let c2 = t.collection_complete(&o).await.unwrap();
    assert_eq!(c2.parties[0].metadonnees.as_ref().unwrap().titre, "Un");
}

#[tokio::test]
async fn recherche_de_collections() {
    let s = serveur().await;
    Mock::given(method("GET"))
        .and(path("/3/search/collection"))
        .and(query_param("query", "Saga"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"results": [{"id": 10, "name": "Saga", "poster_path": "/c.jpg"}]}),
        ))
        .mount(&s)
        .await;
    let t = scraper(&s, None);
    let r = t
        .rechercher_collections(&OptionsRecherche::avec_requete("Saga"))
        .await
        .unwrap();
    assert_eq!(r[0].ids["tmdbSet"], "10");
    assert_eq!(r[0].titre, "Saga");
}

#[test]
fn langues_et_dates() {
    use mediamanager_scrapers::tmdb::{date_du_jour, langue_requete};
    assert_eq!(langue_requete("fr"), "fr-FR");
    assert_eq!(langue_requete("pt_BR"), "pt-BR");
    assert_eq!(langue_requete("en"), "en-US");
    assert_eq!(langue_requete("none"), "");
    let d = date_du_jour();
    assert_eq!(d.len(), 10);
    assert!(d.as_str() > "2024-01-01");
}
