//! Tests d'intégration : clés d'API par service, notes multi-fournisseurs
//! (TMDB, OMDb, Trakt, MDBList) et images Fanart.tv. Faux serveurs locaux
//! (wiremock) : aucun appel réseau réel.

use std::path::{Path, PathBuf};

use mediamanager_app::*;
use mediamanager_core::modele::{Film, Note};
use mediamanager_store::{CoffreMemoire, DossiersApplication, ModeDossiers};
use serde_json::json;
use wiremock::matchers::{header, method, path, path_regex, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const CLE_TMDB: &str = "cle-tmdb-factice";

fn ecrire(p: &Path, contenu: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, contenu).unwrap();
}

struct Banc {
    app: Application,
    films: PathBuf,
    _tmp: tempfile::TempDir,
    _rt: tokio::runtime::Runtime,
    serveur: MockServer,
}

impl Banc {
    fn url(&self, chemin: &str) -> String {
        format!("{}{chemin}", self.serveur.uri())
    }
}

/// TMDB : configuration, film 603 (note 9.1, IMDb `tt0133093`), images vides.
async fn monter_tmdb(s: &MockServer) {
    let rep = |v: serde_json::Value| ResponseTemplate::new(200).set_body_json(v);
    Mock::given(method("GET"))
        .and(path("/3/configuration"))
        .respond_with(rep(
            json!({"images": {"secure_base_url": format!("{}/img/", s.uri())}}),
        ))
        .mount(s)
        .await;
    Mock::given(method("GET"))
        .and(path("/3/search/movie"))
        .respond_with(rep(json!({
            "page": 1, "total_pages": 1,
            "results": [{"id": 603, "title": "Matrix", "original_title": "The Matrix",
                         "original_language": "en", "release_date": "1999-03-30"}]
        })))
        .mount(s)
        .await;
    Mock::given(method("GET"))
        .and(path("/3/movie/603"))
        .respond_with(rep(json!({
            "id": 603, "title": "Matrix", "original_title": "The Matrix",
            "original_language": "en", "overview": "Un pirate.", "runtime": 136,
            "release_date": "1999-03-30", "vote_average": 9.1, "vote_count": 21000,
            "genres": [{"id": 28, "name": "Action"}],
            "credits": {"cast": [], "crew": []},
            "external_ids": {"imdb_id": "tt0133093"}
        })))
        .mount(s)
        .await;
    Mock::given(method("GET"))
        .and(path("/3/movie/603/images"))
        .respond_with(rep(json!({"posters": [], "backdrops": [], "logos": []})))
        .mount(s)
        .await;
    Mock::given(method("GET"))
        .and(path_regex("^/img/.+"))
        .respond_with(|req: &Request| {
            ResponseTemplate::new(200).set_body_bytes(req.url.path().as_bytes().to_vec())
        })
        .mount(s)
        .await;
}

fn banc(avec_cle_tmdb: bool) -> Banc {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let serveur = rt.block_on(async {
        let s = MockServer::start().await;
        monter_tmdb(&s).await;
        s
    });
    let d = tempfile::tempdir().unwrap();
    let films = d.path().join("films");
    ecrire(
        &films.join("The Matrix (1999)/The Matrix (1999).mkv"),
        b"video",
    );
    let mut services = ConfigServices {
        url_api_tmdb: Some(format!("{}/3/", serveur.uri())),
        cache_http: false,
        horodatage_nfo: false,
        ..ConfigServices::default()
    };
    for (s, chemin) in [
        ("omdb", "/omdb/"),
        ("trakt", "/trakt/"),
        ("mdblist", "/mdb/"),
        ("fanart", "/fanart/"),
        ("simkl", "/simkl/"),
    ] {
        services
            .urls_services
            .insert(s.into(), format!("{}{chemin}", serveur.uri()));
    }
    let app = Application::demarrer(OptionsDemarrage {
        dossiers: Some(DossiersApplication::depuis_racine(
            d.path().join("app"),
            ModeDossiers::Explicite,
        )),
        coffre: Some(Box::new(CoffreMemoire::default())),
        journal: OptionsJournal::default(),
        services,
    })
    .unwrap();
    if avec_cle_tmdb {
        app.enregistrer_cle("tmdb", CLE_TMDB).unwrap();
    }
    let f = films.clone();
    app.modifier_reglages(move |r| {
        r.films.sources = vec![f];
        r.general.corbeille_activee = false;
    })
    .unwrap();
    app.mettre_a_jour_sources_films().attendre().unwrap();
    Banc {
        app,
        films,
        _tmp: d,
        _rt: rt,
        serveur,
    }
}

fn monter(b: &Banc, f: impl std::future::Future<Output = ()>) {
    b._rt.block_on(f);
}

fn matrix(b: &Banc) -> Film {
    b.app.films().unwrap().remove(0)
}

fn etat(b: &Banc, id: &str) -> bool {
    b.app
        .etat_services()
        .unwrap()
        .into_iter()
        .find(|e| e.id == id)
        .unwrap()
        .cle_presente
}

#[test]
fn etat_enregistrement_et_suppression_des_cles() {
    let b = banc(false);
    let ids: Vec<String> = b
        .app
        .etat_services()
        .unwrap()
        .into_iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(
        ids,
        [
            "tmdb", "fanart", "tvdb", "omdb", "trakt", "mdblist", "simkl"
        ]
    );
    assert!(!etat(&b, "tmdb") && !etat(&b, "omdb"));
    for s in [
        "tmdb", "fanart", "tvdb", "omdb", "trakt", "mdblist", "simkl",
    ] {
        b.app.enregistrer_cle(s, "  valeur  ").unwrap();
        assert!(b.app.cle_api_enregistree(s).unwrap(), "{s}");
        assert!(etat(&b, s));
    }
    // La clé TMDB enregistrée sert bien au fournisseur (lue dans les réglages).
    assert_eq!(b.app.reglages().general.cle_api_tmdb, "valeur");
    b.app.enregistrer_cle("omdb", "").unwrap();
    b.app.enregistrer_cle("tmdb", "").unwrap();
    assert!(!etat(&b, "omdb") && !etat(&b, "tmdb"));
    assert!(etat(&b, "trakt"));
    assert!(b.app.enregistrer_cle("inconnu", "x").is_err());
    assert!(b.app.cle_api_enregistree("inconnu").is_err());
    b.app.arreter().unwrap();
}

#[test]
fn test_de_cle_par_service() {
    let b = banc(false);
    monter(&b, async {
        Mock::given(method("GET"))
            .and(path("/omdb/"))
            .and(query_param("apikey", "bonne"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    json!({"Response": "True", "imdbRating": "8.7", "imdbVotes": "1"}),
                ),
            )
            .mount(&b.serveur)
            .await;
        Mock::given(method("GET"))
            .and(path("/omdb/"))
            .and(query_param("apikey", "mauvaise"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_json(json!({"Response": "False", "Error": "Invalid API key!"})),
            )
            .mount(&b.serveur)
            .await;
    });
    // Sans clé.
    assert!(matches!(
        b.app.tester_cle_api("omdb").attendre(),
        Err(ErreurApp::Scraper(
            mediamanager_scrapers::ErreurScraper::CleApiManquante
        ))
    ));
    b.app.enregistrer_cle("omdb", "bonne").unwrap();
    b.app.tester_cle_api("omdb").attendre().unwrap();
    b.app.enregistrer_cle("omdb", "mauvaise").unwrap();
    assert!(matches!(
        b.app.tester_cle_api("omdb").attendre(),
        Err(ErreurApp::Scraper(
            mediamanager_scrapers::ErreurScraper::CleApiInvalide
        ))
    ));
    // Test TMDB (film 603 du faux serveur).
    b.app.enregistrer_cle("tmdb", CLE_TMDB).unwrap();
    b.app.tester_cle_api("tmdb").attendre().unwrap();
    // TheTVDB : pas de test.
    b.app.enregistrer_cle("tvdb", "x").unwrap();
    assert!(b.app.tester_cle_api("tvdb").attendre().is_err());
    assert!(b.app.tester_cle_api("inconnu").attendre().is_err());
    b.app.arreter().unwrap();
}

#[test]
fn notes_de_plusieurs_fournisseurs_fusionnees() {
    let b = banc(true);
    monter(&b, async {
        Mock::given(method("GET"))
            .and(path("/omdb/"))
            .and(query_param("i", "tt0133093"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "Response": "True", "imdbRating": "8.8", "imdbVotes": "2,000,000",
                "Metascore": "73",
                "Ratings": [{"Source": "Rotten Tomatoes", "Value": "83%"}]
            })))
            .expect(1)
            .mount(&b.serveur)
            .await;
        // Clé Trakt refusée : simple avertissement.
        Mock::given(method("GET"))
            .and(path("/trakt/movies/tt0133093/ratings"))
            .and(header("trakt-api-key", "client-trakt"))
            .respond_with(ResponseTemplate::new(403))
            .expect(1)
            .mount(&b.serveur)
            .await;
        Mock::given(method("GET"))
            .and(path("/mdb/imdb/movie/tt0133093"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "ratings": [
                    {"source": "imdb", "value": 9.9, "votes": 5},
                    {"source": "trakt", "value": 8.5, "votes": 321}
                ]
            })))
            .expect(1)
            .mount(&b.serveur)
            .await;
    });
    b.app.enregistrer_cle("omdb", "cle-omdb").unwrap();
    b.app.enregistrer_cle("trakt", "client-trakt").unwrap();
    b.app.enregistrer_cle("mdblist", "cle-mdb").unwrap();
    let mut m = matrix(&b);
    m.ids.definir("tmdb", "603");
    m.notes
        .insert("imdb".into(), Note::nouvelle("imdb", 1.0, 1, 10));
    m.notes
        .insert("user".into(), Note::nouvelle("user", 10.0, 1, 10));
    m.notes
        .insert("autre".into(), Note::nouvelle("autre", 3.0, 3, 5));
    b.app.enregistrer_film(m.clone()).unwrap();

    let r = b
        .app
        .recuperer_evaluations(vec![m.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(r.mis_a_jour, vec![m.id.clone()], "{r:?}");
    assert_eq!(r.fournisseurs, ["tmdb", "omdb", "trakt", "mdblist"]);
    assert_eq!(r.avertissements.len(), 1, "{r:?}");
    assert!(r.avertissements[0].starts_with("trakt"));
    assert!(!r.avertissements[0].contains("client-trakt"));
    let apres = b.app.film(&m.id).unwrap().unwrap();
    let n = &apres.notes;
    assert!((n["tmdb"].valeur - 9.1).abs() < 1e-4);
    // OMDb passe avant MDBList pour IMDb.
    assert!((n["imdb"].valeur - 8.8).abs() < 1e-4, "{n:?}");
    assert_eq!(n["imdb"].votes, 2_000_000);
    assert_eq!(
        (n["rottenTomatoes"].valeur, n["rottenTomatoes"].maximum),
        (83.0, 100)
    );
    assert_eq!(
        (n["metacritic"].valeur, n["metacritic"].maximum),
        (73.0, 100)
    );
    // Trakt refusé par sa clé : la note vient de MDBList.
    assert_eq!((n["trakt"].valeur, n["trakt"].votes), (8.5, 321));
    // Note utilisateur et source inconnue intactes.
    assert_eq!(n["user"], m.notes["user"]);
    assert_eq!(n["autre"], m.notes["autre"]);
    b.app.arreter().unwrap();
}

#[test]
fn notes_sans_cle_tmdb_avec_un_autre_fournisseur() {
    let b = banc(false);
    monter(&b, async {
        Mock::given(method("GET"))
            .and(path("/omdb/"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    json!({"Response": "True", "imdbRating": "7.0", "imdbVotes": "10"}),
                ),
            )
            .mount(&b.serveur)
            .await;
    });
    // Aucune clé : erreur avant tout appel réseau.
    let m = matrix(&b);
    assert!(matches!(
        b.app.recuperer_evaluations(vec![m.id.clone()]).attendre(),
        Err(ErreurApp::CleTmdbAbsente)
    ));
    b.app.enregistrer_cle("omdb", "cle").unwrap();
    let mut m = m;
    m.ids.definir("imdb", "tt0133093");
    b.app.enregistrer_film(m.clone()).unwrap();
    let r = b
        .app
        .recuperer_evaluations(vec![m.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(r.mis_a_jour, vec![m.id.clone()], "{r:?}");
    assert_eq!(r.fournisseurs, ["omdb"]);
    assert_eq!(
        b.app.film(&m.id).unwrap().unwrap().notes["imdb"].valeur,
        7.0
    );
    b.app.arreter().unwrap();
}

#[test]
fn limite_de_debit_desactive_le_fournisseur_pour_la_suite() {
    let b = banc(false);
    monter(&b, async {
        Mock::given(method("GET"))
            .and(path("/omdb/"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_json(json!({"Response": "False", "Error": "Invalid API key!"})),
            )
            .expect(1)
            .mount(&b.serveur)
            .await;
    });
    b.app.enregistrer_cle("omdb", "cle").unwrap();
    let mut ids = Vec::new();
    for (i, nom) in ["A", "B"].iter().enumerate() {
        let mut m = matrix(&b);
        m.id = String::new();
        m.titre = (*nom).into();
        m.chemin = b.films.join(nom);
        m.fichiers.clear();
        m.ids.definir("imdb", &format!("tt000000{}", i + 1));
        ids.push(b.app.enregistrer_film(m).unwrap().id);
    }
    let r = b.app.recuperer_evaluations(ids).attendre().unwrap();
    // Une seule requête (le second film n'interroge plus OMDb), pas d'échec.
    assert!(r.erreurs.is_empty(), "{r:?}");
    assert_eq!(r.avertissements.len(), 1, "{r:?}");
    assert_eq!(r.inchanges.len(), 2);
    b.app.arreter().unwrap();
}

fn fanart_film() -> serde_json::Value {
    let img = |u: &str| json!({"id": "1", "url": u, "lang": "en", "likes": "5"});
    json!({"name": "The Matrix", "imdb_id": "tt0133093",
        "hdmovielogo": [img("LOGO")],
        "moviethumb": [img("THUMB")],
        "moviebanner": [img("BANNER")],
        "hdmovieclearart": [img("CLEARART")],
        "moviedisc": [img("DISC")]})
}

#[test]
fn images_fanart_telechargees_avec_cle() {
    let b = banc(true);
    monter(&b, async {
        let rep = fanart_film().to_string();
        let rep = ["LOGO", "THUMB", "BANNER", "CLEARART", "DISC"]
            .iter()
            .fold(rep, |acc, k| {
                let ext = if *k == "THUMB" || *k == "BANNER" {
                    "jpg"
                } else {
                    "png"
                };
                acc.replace(
                    &format!("\"{k}\""),
                    &format!("\"{}/fa/{}.{ext}\"", b.serveur.uri(), k.to_lowercase()),
                )
            });
        Mock::given(method("GET"))
            .and(path("/fanart/movies/tt0133093"))
            .and(header("api-key", "cle-fanart"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(rep.into_bytes(), "application/json"),
            )
            .expect(1)
            .mount(&b.serveur)
            .await;
        Mock::given(method("GET"))
            .and(path_regex("^/fa/.+"))
            .respond_with(|req: &Request| {
                ResponseTemplate::new(200).set_body_bytes(req.url.path().as_bytes().to_vec())
            })
            .mount(&b.serveur)
            .await;
    });
    b.app.enregistrer_cle("fanart", "cle-fanart").unwrap();
    let id = matrix(&b).id;
    let r = b
        .app
        .scraper_films(vec![id.clone()], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    assert!(r.erreurs.is_empty(), "{r:?}");
    let d = b.films.join("The Matrix (1999)");
    let lire =
        |n: &str| std::fs::read_to_string(d.join(n)).unwrap_or_else(|_| panic!("{n} absent"));
    assert_eq!(lire("The Matrix (1999)-clearlogo.png"), "/fa/logo.png");
    assert_eq!(lire("The Matrix (1999)-landscape.jpg"), "/fa/thumb.jpg");
    assert_eq!(lire("The Matrix (1999)-banner.jpg"), "/fa/banner.jpg");
    assert_eq!(lire("The Matrix (1999)-clearart.png"), "/fa/clearart.png");
    assert_eq!(lire("The Matrix (1999)-discart.png"), "/fa/disc.png");
    let f = b.app.film(&id).unwrap().unwrap();
    use mediamanager_core::modele::TypeFichier as T;
    for t in [T::Clearlogo, T::Thumb, T::Banner, T::Clearart, T::Disc] {
        assert_eq!(f.fichiers_de_type(t).count(), 1, "{t:?}");
    }
    b.app.arreter().unwrap();
}

#[test]
fn sans_cle_fanart_aucun_appel_et_comportement_inchange() {
    let b = banc(true);
    monter(&b, async {
        Mock::given(method("GET"))
            .and(path_regex("^/fanart/.*"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&b.serveur)
            .await;
    });
    let id = matrix(&b).id;
    b.app
        .scraper_films(vec![id], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    let d = b.films.join("The Matrix (1999)");
    assert!(!d.join("The Matrix (1999)-banner.jpg").exists());
    assert!(!d.join("The Matrix (1999)-clearart.png").exists());
    let _ = b.url("");
    b.app.arreter().unwrap();
}
