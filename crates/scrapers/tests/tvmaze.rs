//! Tests du fournisseur TVmaze contre un faux serveur HTTP local (aucun appel réseau réel).

use mediamanager_scrapers::*;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn scraper(s: &MockServer) -> TvMaze {
    TvMaze::new(
        ConfigTvmaze {
            url_api: Some(format!("{}/", s.uri())),
        },
        ClientHttp::new(ConfigHttp::default()).unwrap(),
    )
}

fn emission() -> serde_json::Value {
    json!({
        "id": 82, "url": "https://www.tvmaze.com/shows/82", "name": "Game of Thrones", "language": "English",
        "genres": ["Drama", "Fantasy"], "status": "Ended", "runtime": 60,
        "premiered": "2011-04-17", "ended": "2019-05-19",
        "rating": {"average": 8.9},
        "externals": {"tvrage": 24493, "thetvdb": 121361, "imdb": "tt0944947"},
        "image": {"medium": "https://img.test/m/p.jpg", "original": "https://img.test/o/p.jpg"},
        "summary": "<p>Un <b>trône</b> &amp; des dragons.</p>",
        "network": {"name": "HBO", "country": {"name": "United States", "code": "US"}},
        "webChannel": null,
        "_embedded": {
            "seasons": [
                {"id": 307, "number": 1, "name": "", "episodeOrder": 2, "premiereDate": "2011-04-17",
                 "image": {"medium": "https://img.test/m/s1.jpg", "original": "https://img.test/o/s1.jpg"}, "summary": null}
            ],
            "cast": [{"person": {"id": 1, "name": "Peter Dinklage", "url": "https://www.tvmaze.com/people/1",
                                  "image": {"medium": "https://img.test/m/pd.jpg"}},
                      "character": {"name": "Tyrion Lannister"}}],
            "crew": [
                {"type": "Creator", "person": {"id": 2, "name": "David Benioff"}},
                {"type": "Casting", "person": {"id": 3, "name": "Ignoré"}}
            ],
            "images": [
                {"type": "background", "main": true, "resolutions": {
                    "original": {"url": "https://img.test/o/bg.jpg", "width": 1920, "height": 1080},
                    "medium": {"url": "https://img.test/m/bg.jpg", "width": 640, "height": 360}}},
                {"type": "banner", "resolutions": {"original": {"url": "https://img.test/o/b.jpg", "width": 758, "height": 140}}},
                {"type": "typography", "resolutions": {"original": {"url": "https://img.test/o/t.png", "width": 1, "height": 1}}}
            ]
        }
    })
}

async fn monter_emission(s: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/shows/82"))
        .and(query_param("embed[]", "seasons"))
        .respond_with(ResponseTemplate::new(200).set_body_json(emission()))
        .mount(s)
        .await;
}

async fn monter_episodes(s: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/seasons/307/episodes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id": 4952, "name": "Winter Is Coming", "season": 1, "number": 1, "type": "regular",
             "airdate": "2011-04-17", "runtime": 60, "rating": {"average": 8.1},
             "image": {"medium": "https://img.test/m/e1.jpg", "original": "https://img.test/o/e1.jpg"},
             "summary": "<p>Résumé 1</p>",
             "_embedded": {"guestcast": [{"person": {"id": 9, "name": "Invité"}, "character": {"name": "Garde"}}],
                           "guestcrew": [{"guestCrewType": "Director", "person": {"id": 10, "name": "Tim Van Patten"}}]}},
            {"id": 4953, "name": "The Kingsroad est un long titre", "season": 1, "number": 2, "type": "regular",
             "airdate": "2011-04-24"},
            {"id": 9999, "name": "Coulisses", "season": 1, "number": null, "type": "significant_special",
             "airdate": "2011-04-24"}
        ])))
        .mount(s)
        .await;
    Mock::given(method("GET"))
        .and(path("/shows/82/alternatelists"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id": 11, "dvd_release": true},
            {"id": 12}
        ])))
        .mount(s)
        .await;
    Mock::given(method("GET"))
        .and(path("/alternatelists/11/alternateepisodes"))
        .and(query_param("embed", "episodes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"id": 1, "season": 1, "number": 2, "_embedded": {"episodes": [{"id": 4952}]}},
            {"id": 2, "season": 1, "number": 1, "_embedded": {"episodes": [{"id": 4953}]}}
        ])))
        .mount(s)
        .await;
}

#[tokio::test]
async fn recherche_texte_score_tvmaze() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search/shows"))
        .and(query_param("q", "thrones"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"score": 0.5, "show": {"id": 7, "name": "Autre", "premiered": "2001-01-01"}},
            {"score": 0.9, "show": {"id": 82, "name": "Game of Thrones", "premiered": "2011-04-17",
                                     "summary": "<p>Texte</p>", "image": {"medium": "https://img.test/m/p.jpg"},
                                     "externals": {"imdb": "tt0944947", "thetvdb": 121361, "tvrage": null}}}
        ])))
        .mount(&s)
        .await;
    let t = scraper(&s);
    let r = t
        .rechercher(&OptionsRecherche::avec_requete("thrones"))
        .await
        .unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].ids["tvmaze"], "82");
    assert_eq!(r[0].ids["tvdb"], "121361");
    assert!(!r[0].ids.contains_key("tvrage"));
    assert_eq!(r[0].resume, "Texte");
    assert!((r[0].score - 0.9).abs() < 1e-6);
    assert_eq!(r[0].annee, 2011);
}

#[tokio::test]
async fn recherche_par_imdb_lookup_et_404() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/lookup/shows"))
        .and(query_param("imdb", "tt0944947"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": 82, "name": "Game of Thrones"})),
        )
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/lookup/shows"))
        .and(query_param("thetvdb", "1"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/search/shows"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&s)
        .await;
    let t = scraper(&s);
    let r = t
        .rechercher(&OptionsRecherche::avec_requete("x").avec_id("imdb", "tt0944947"))
        .await
        .unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].score, 1.0);
    // Lookup 404 : pas d'erreur, repli sur la recherche texte.
    let r = t
        .rechercher(&OptionsRecherche::avec_requete("x").avec_id("tvdb", "1"))
        .await
        .unwrap();
    assert!(r.is_empty());
}

#[tokio::test]
async fn detail_emission() {
    let s = MockServer::start().await;
    monter_emission(&s).await;
    monter_episodes(&s).await;
    let t = scraper(&s);
    let m = t
        .metadonnees_serie(&OptionsRecherche::default().avec_id("tvmaze", "82"))
        .await
        .unwrap();
    assert_eq!(m.titre, "Game of Thrones");
    assert_eq!(m.statut, StatutDiffusion::Terminee);
    assert_eq!((m.annee, m.annee_fin), (2011, 2019));
    assert_eq!(m.resume, "Un trône & des dragons.");
    assert_eq!(m.ids["imdb"], "tt0944947");
    assert_eq!(m.ids["tvrage"], "24493");
    assert_eq!(m.evaluations[0].valeur, 8.9);
    assert_eq!(m.societes_production, ["HBO"]);
    assert_eq!(m.pays, ["US"]);
    assert_eq!(m.personnes.len(), 2);
    assert_eq!(m.personnes[0].role, "Tyrion Lannister");
    assert_eq!(m.personnes[1].type_personne, TypePersonne::Scenariste);
    assert_eq!(
        m.groupes_episodes,
        [GroupeEpisodes::diffusion(), GroupeEpisodes::dvd()]
    );
    assert_eq!(m.saisons.len(), 1);
    assert_eq!(m.saisons[0].nombre_episodes, 2);
    let types: Vec<_> = m.artwork.iter().map(|a| a.type_artwork).collect();
    assert_eq!(
        types,
        [
            TypeArtworkEtendu::Affiche,
            TypeArtworkEtendu::Fond,
            TypeArtworkEtendu::Banniere,
            TypeArtworkEtendu::AfficheSaison
        ]
    );
    let saison = m.artwork.last().unwrap();
    assert_eq!(saison.saison, Some(1));
    // Artwork filtré via le trait dédié.
    let fonds = t
        .artwork_serie(
            &OptionsRecherche::default().avec_id("tvmaze", "82"),
            Some(TypeArtworkEtendu::Fond),
        )
        .await
        .unwrap();
    assert_eq!(fonds.len(), 1);
    assert_eq!(fonds[0].tailles.len(), 2);
}

#[tokio::test]
async fn liste_et_appariement_d_episodes() {
    let s = MockServer::start().await;
    monter_emission(&s).await;
    monter_episodes(&s).await;
    let t = scraper(&s);
    let serie = OptionsRecherche::default().avec_id("tvmaze", "82");
    let l = t.liste_episodes(&serie).await.unwrap();
    assert_eq!(l.len(), 3);
    assert_eq!(l[0].numero_diffusion(), Some((1, 1)));
    assert_eq!(l[0].numero_dvd(), Some((1, 2)));
    assert_eq!(l[1].numero_dvd(), Some((1, 1)));
    assert_eq!(l[2].numero_diffusion(), Some((0, 1)));
    assert_eq!(l[0].resume, "Résumé 1");
    assert!(
        l[0].personnes
            .iter()
            .any(|p| p.type_personne == TypePersonne::Invite)
    );
    assert!(
        l[0].personnes
            .iter()
            .any(|p| p.type_personne == TypePersonne::Realisateur)
    );
    // Ordre DVD.
    let o = OptionsEpisode {
        serie: serie.clone(),
        groupe: GroupeEpisodes::dvd(),
        saison: Some(1),
        episode: Some(1),
        ..Default::default()
    };
    assert_eq!(
        t.metadonnees_episode(&o).await.unwrap().titre,
        "The Kingsroad est un long titre"
    );
    // Titre long prioritaire sur le numéro.
    let o = OptionsEpisode {
        serie: serie.clone(),
        titre: "The Kingsroad est un long titre".into(),
        saison: Some(1),
        episode: Some(1),
        ..Default::default()
    };
    assert_eq!(
        t.metadonnees_episode(&o).await.unwrap().titre,
        "The Kingsroad est un long titre"
    );
    // Date ambiguë (deux épisodes le même jour) : rejet.
    let o = OptionsEpisode {
        serie: serie.clone(),
        date_diffusion: Some("2011-04-24".into()),
        ..Default::default()
    };
    assert!(matches!(
        t.metadonnees_episode(&o).await,
        Err(ErreurScraper::AucunResultat)
    ));
    // Artwork d'épisode.
    let o = OptionsEpisode::diffusion(serie, 1, 1);
    let a = t.artwork_episode(&o).await.unwrap();
    assert_eq!(a[0].type_artwork, TypeArtworkEtendu::Vignette);
}

#[tokio::test]
async fn sans_identifiant() {
    let s = MockServer::start().await;
    let t = scraper(&s);
    assert!(matches!(
        t.liste_episodes(&OptionsRecherche::avec_requete("x")).await,
        Err(ErreurScraper::IdentifiantManquant(_))
    ));
}
