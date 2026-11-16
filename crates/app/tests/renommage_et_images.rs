//! Tests d'intégration : renommage après scraping (automatique et choix
//! manuel), nombre d'affiches et de fonds, photos des acteurs. Faux serveur
//! TMDB local (wiremock), arborescences temporaires : aucun appel réseau réel.

use std::path::{Path, PathBuf};

use mediamanager_app::*;
use mediamanager_store::{CoffreMemoire, DossiersApplication, ModeDossiers};
use serde_json::json;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const CLE_TEST: &str = "cle-de-test-factice";

fn options(racine: &Path, url: String) -> OptionsDemarrage {
    OptionsDemarrage {
        dossiers: Some(DossiersApplication::depuis_racine(
            racine.join("app"),
            ModeDossiers::Explicite,
        )),
        coffre: Some(Box::new(CoffreMemoire::default())),
        journal: OptionsJournal::default(),
        services: ConfigServices {
            url_api_tmdb: Some(url),
            cache_http: false,
            horodatage_nfo: false,
            ..ConfigServices::default()
        },
    }
}

fn ecrire(p: &Path, contenu: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, contenu).unwrap();
}

fn img(chemin: &str, langue: Option<&str>, votes: u32, largeur: u32) -> serde_json::Value {
    json!({"file_path": chemin, "iso_639_1": langue, "vote_count": votes,
           "width": largeur, "height": largeur * 3 / 2})
}

/// Le corps d'une image est son propre chemin : permet de vérifier laquelle
/// a été choisie.
fn serveur_images() -> impl Fn(&Request) -> ResponseTemplate {
    |req: &Request| ResponseTemplate::new(200).set_body_bytes(req.url.path().as_bytes().to_vec())
}

fn monter_tmdb(rt: &tokio::runtime::Runtime) -> MockServer {
    rt.block_on(async {
        let s = MockServer::start().await;
        let base_images = format!("{}/img/", s.uri());
        let rep = |v: serde_json::Value| ResponseTemplate::new(200).set_body_json(v);
        Mock::given(method("GET"))
            .and(path("/3/configuration"))
            .respond_with(rep(json!({"images": {"secure_base_url": base_images}})))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/3/search/movie"))
            .respond_with(rep(json!({
                "page": 1, "total_pages": 1,
                "results": [{"id": 603, "title": "Matrix", "original_title": "The Matrix",
                             "original_language": "en", "release_date": "1999-03-30",
                             "poster_path": "/m.jpg", "overview": "Néo..."}]
            })))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/3/movie/603"))
            .respond_with(rep(json!({
                "id": 603, "title": "Matrix", "original_title": "The Matrix",
                "original_language": "en", "overview": "Un pirate.", "runtime": 136,
                "release_date": "1999-03-30", "poster_path": "/m.jpg",
                "genres": [{"id": 28, "name": "Action"}],
                "credits": {
                    "cast": [
                        {"id": 6384, "name": "Keanu Reeves", "character": "Neo", "profile_path": "/keanu.jpg"},
                        {"id": 6385, "name": "Carrie-Anne Moss", "character": "Trinity", "profile_path": "/carrie.jpg"},
                        {"id": 6386, "name": "Hugo Weaving", "character": "Smith", "profile_path": "/bad.jpg"},
                        {"id": 6387, "name": "Sans Photo", "character": "X"}
                    ],
                    "crew": []
                },
                "external_ids": {"imdb_id": "tt0133093"}
            })))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/3/movie/603/images"))
            .respond_with(rep(json!({
                "posters": [
                    img("/p-en.jpg", Some("en"), 90, 1000),
                    img("/p-fr-faible.jpg", Some("fr"), 1, 1000),
                    img("/p-fr-top.jpg", Some("fr"), 20, 1000),
                    img("/p-fr-moyen.jpg", Some("fr"), 5, 1000)
                ],
                "backdrops": [
                    img("/f-texte.jpg", Some("fr"), 99, 1920),
                    img("/f-a.jpg", None, 3, 1920),
                    img("/f-b.jpg", None, 8, 1920),
                    img("/f-c.jpg", None, 5, 1920),
                    img("/f-d.jpg", None, 1, 1920)
                ],
                "logos": []
            })))
            .mount(&s)
            .await;
        // Photo en échec (prioritaire sur la règle générale, montée avant).
        Mock::given(method("GET"))
            .and(path_regex(r"^/img/.*/bad\.jpg$"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path_regex("^/img/.+"))
            .respond_with(serveur_images())
            .mount(&s)
            .await;
        // Série.
        Mock::given(method("GET"))
            .and(path("/3/search/tv"))
            .respond_with(rep(json!({
                "page": 1, "total_pages": 1,
                "results": [{"id": 1399, "name": "Ma Serie", "original_name": "Ma Serie",
                             "first_air_date": "2020-01-05", "poster_path": "/p.jpg"}]
            })))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/3/tv/1399"))
            .respond_with(rep(json!({
                "id": 1399, "name": "Ma Serie", "original_name": "Ma Serie",
                "original_language": "fr", "overview": "Une série.",
                "first_air_date": "2020-01-05", "status": "Ended", "episode_run_time": [42],
                "genres": [{"id": 18, "name": "Drame"}], "poster_path": "/p.jpg",
                "number_of_seasons": 1,
                "seasons": [{"id": 11, "season_number": 1, "name": "Saison 1", "episode_count": 2}],
                "credits": {"cast": [
                    {"id": 1, "name": "Actrice Une", "character": "H", "profile_path": "/act.jpg"}
                ], "crew": []},
                "external_ids": {"imdb_id": "tt1234567"},
                "episode_groups": {"results": []},
                "images": {"posters": [img("/sp1.jpg", Some("fr"), 9, 1000),
                                       img("/sp2.jpg", Some("fr"), 4, 1000)],
                           "backdrops": [img("/sf1.jpg", None, 9, 1920),
                                         img("/sf2.jpg", None, 4, 1920)],
                           "logos": []}
            })))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/3/tv/1399/season/1"))
            .respond_with(rep(json!({"episodes": [
                {"id": 101, "name": "Pilote", "air_date": "2020-01-05",
                 "season_number": 1, "episode_number": 1},
                {"id": 102, "name": "Suite", "air_date": "2020-01-12",
                 "season_number": 1, "episode_number": 2}
            ]})))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/3/tv/1399/season/\d+/images$"))
            .respond_with(rep(json!({"posters": []})))
            .mount(&s)
            .await;
        s
    })
}

struct Banc {
    app: Application,
    films: PathBuf,
    series: PathBuf,
    _tmp: tempfile::TempDir,
    _rt: tokio::runtime::Runtime,
    _serveur: MockServer,
}

fn banc(reglages: impl FnOnce(&mut mediamanager_store::Reglages) + Send + 'static) -> Banc {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let serveur = monter_tmdb(&rt);
    let d = tempfile::tempdir().unwrap();
    let films = d.path().join("films");
    let series = d.path().join("series");
    ecrire(
        &films.join("The Matrix (1999)/The Matrix (1999).mkv"),
        b"video",
    );
    ecrire(&series.join("Ma Serie (2020)/Ma Serie - S01E01.mkv"), b"e1");
    ecrire(&series.join("Ma Serie (2020)/Ma Serie - S01E02.mkv"), b"e2");
    let app = Application::demarrer(options(d.path(), format!("{}/3/", serveur.uri()))).unwrap();
    app.definir_cle_tmdb(CLE_TEST).unwrap();
    let (f, s) = (films.clone(), series.clone());
    app.modifier_reglages(move |r| {
        r.films.sources = vec![f];
        r.series.sources = vec![s];
        r.renommeur.gabarit_dossier_film = "${title} (${year})".into();
        r.renommeur.gabarit_fichier_film = "${title} (${year})".into();
        r.general.corbeille_activee = false;
        reglages(r);
    })
    .unwrap();
    app.mettre_a_jour_sources_films().attendre().unwrap();
    app.mettre_a_jour_sources_series().attendre().unwrap();
    Banc {
        app,
        films,
        series,
        _tmp: d,
        _rt: rt,
        _serveur: serveur,
    }
}

fn film_id(b: &Banc) -> String {
    b.app.films().unwrap().remove(0).id
}

fn serie_id(b: &Banc) -> String {
    b.app.series().unwrap().remove(0).id
}

#[test]
fn renommage_apres_scrap_automatique() {
    let b = banc(|r| r.films.renommer_apres_scraping = true);
    let id = film_id(&b);
    let rapport = b
        .app
        .scraper_films(vec![id.clone()], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    assert_eq!(rapport.reussis, vec![id.clone()], "{rapport:?}");
    let renommage = rapport.renommage.expect("renommage enchaîné");
    assert_eq!(renommage.renommes, vec![id.clone()], "{renommage:?}");
    let nouveau = b.films.join("Matrix (1999)");
    assert!(nouveau.join("Matrix (1999).mkv").exists());
    assert!(nouveau.join("Matrix (1999)-poster.jpg").exists());
    assert!(!b.films.join("The Matrix (1999)").exists());
    assert_eq!(b.app.film(&id).unwrap().unwrap().chemin, nouveau);
}

#[test]
fn pas_de_renommage_si_le_reglage_est_inactif() {
    let b = banc(|_| {});
    let id = film_id(&b);
    let rapport = b
        .app
        .scraper_films(vec![id.clone()], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    assert!(rapport.renommage.is_none());
    assert!(b.films.join("The Matrix (1999)").exists());
    let (_, r) = b
        .app
        .appliquer_resultat_avec_renommage(&id, resultat_matrix(&b, &id), ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    assert!(r.is_none());
    assert!(b.films.join("The Matrix (1999)").exists());
}

fn resultat_matrix(b: &Banc, id: &str) -> ResultatRecherche {
    b.app
        .rechercher_film(id, None)
        .attendre()
        .unwrap()
        .remove(0)
}

#[test]
fn renommage_apres_choix_manuel_film() {
    let b = banc(|r| r.films.renommer_apres_scraping = true);
    let id = film_id(&b);
    let choix = resultat_matrix(&b, &id);
    let (film, rapport) = b
        .app
        .appliquer_resultat_avec_renommage(&id, choix, ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    let rapport = rapport.expect("rapport de renommage");
    assert_eq!(rapport.renommes, vec![id.clone()], "{rapport:?}");
    let nouveau = b.films.join("Matrix (1999)");
    assert_eq!(film.chemin, nouveau);
    assert!(nouveau.join("Matrix (1999).mkv").exists());
    assert!(nouveau.join("Matrix (1999).nfo").exists());
    assert_eq!(b.app.film(&id).unwrap().unwrap().chemin, nouveau);
}

#[test]
fn renommage_apres_choix_manuel_api_historique() {
    let b = banc(|r| r.films.renommer_apres_scraping = true);
    let id = film_id(&b);
    let choix = resultat_matrix(&b, &id);
    let film = b
        .app
        .appliquer_resultat(&id, choix, ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    assert_eq!(film.chemin, b.films.join("Matrix (1999)"));
    assert!(b.films.join("Matrix (1999)/Matrix (1999).mkv").exists());
}

#[test]
fn film_verrouille_jamais_renomme() {
    let b = banc(|r| r.films.renommer_apres_scraping = true);
    let id = film_id(&b);
    let choix = resultat_matrix(&b, &id);
    b.app.avec_stockage(|s| {
        use mediamanager_core::modele::Film;
        let mut f = s.bibliotheque.obtenir::<Film>(&id).unwrap().unwrap();
        f.verrouille = true;
        s.bibliotheque.enregistrer(&mut f).unwrap();
    });
    let err = b
        .app
        .appliquer_resultat(&id, choix, ConfigMetadonnees::tout())
        .attendre()
        .unwrap_err();
    assert!(matches!(err, ErreurApp::Verrouille(_)), "{err:?}");
    assert!(b.films.join("The Matrix (1999)").exists());
}

#[test]
fn renommage_apres_choix_manuel_serie() {
    let b = banc(|r| r.series.renommer_apres_scraping = true);
    let id = serie_id(&b);
    let choix = b
        .app
        .rechercher_serie(&id, None)
        .attendre()
        .unwrap()
        .remove(0);
    let (_, rapport) = b
        .app
        .appliquer_resultat_serie_avec_renommage(&id, choix, ConfigSerie::default())
        .attendre()
        .unwrap();
    let rapport = rapport.expect("rapport de renommage");
    assert_eq!(rapport.renommes, vec![id.clone()], "{rapport:?}");
    let saison = b.series.join("Ma Serie (2020)/Season 1");
    assert!(
        saison.join("Ma Serie - S01E01 - Pilote.mkv").exists(),
        "{:?}",
        std::fs::read_dir(&saison).map(|d| d.flatten().map(|e| e.file_name()).collect::<Vec<_>>())
    );
}

fn lire(p: &Path) -> String {
    String::from_utf8_lossy(&std::fs::read(p).unwrap_or_default()).into_owned()
}

#[test]
fn nombre_d_affiches_et_de_fonds_respecte() {
    let b = banc(|r| {
        r.films.nombre_affiches = 2;
        r.films.nombre_fanarts = 3;
    });
    let id = film_id(&b);
    b.app
        .scraper_films(vec![id.clone()], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    let d = b.films.join("The Matrix (1999)");
    // Meilleures affiches : langue préférée (fr) puis votes.
    assert!(lire(&d.join("The Matrix (1999)-poster.jpg")).contains("p-fr-top"));
    assert!(lire(&d.join("The Matrix (1999)-poster1.jpg")).contains("p-fr-moyen"));
    assert!(!d.join("The Matrix (1999)-poster2.jpg").exists());
    // Fonds : sans texte d'abord, par votes.
    assert!(lire(&d.join("The Matrix (1999)-fanart.jpg")).contains("f-b"));
    assert!(lire(&d.join("The Matrix (1999)-fanart1.jpg")).contains("f-c"));
    assert!(lire(&d.join("The Matrix (1999)-fanart2.jpg")).contains("f-a"));
    assert!(!d.join("The Matrix (1999)-fanart3.jpg").exists());
    // L'affiche principale reste en tête dans la base.
    let f = b.app.film(&id).unwrap().unwrap();
    let premiere = f
        .fichiers_de_type(mediamanager_core::modele::TypeFichier::Poster)
        .next()
        .unwrap();
    assert!(premiere.nom.ends_with("-poster.jpg"), "{}", premiere.nom);
}

#[test]
fn par_defaut_une_affiche_et_un_fond() {
    let b = banc(|_| {});
    let id = film_id(&b);
    b.app
        .scraper_films(vec![id], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    let d = b.films.join("The Matrix (1999)");
    assert!(d.join("The Matrix (1999)-poster.jpg").exists());
    assert!(d.join("The Matrix (1999)-fanart.jpg").exists());
    assert!(!d.join("The Matrix (1999)-poster1.jpg").exists());
    assert!(!d.join("The Matrix (1999)-fanart1.jpg").exists());
}

#[test]
fn anciens_reglages_fanarts_supplementaires() {
    let b = banc(|r| {
        r.films.fanarts_extra = true;
        r.films.nombre_fanarts_extra = 1;
    });
    let id = film_id(&b);
    b.app
        .scraper_films(vec![id], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    let d = b.films.join("The Matrix (1999)");
    assert!(d.join("The Matrix (1999)-fanart.jpg").exists());
    assert!(d.join("The Matrix (1999)-fanart1.jpg").exists());
    assert!(!d.join("The Matrix (1999)-fanart2.jpg").exists());
}

#[test]
fn photos_des_acteurs_films() {
    let b = banc(|r| r.films.images_acteurs = true);
    let id = film_id(&b);
    let d = b.films.join("The Matrix (1999)");
    // Photo déjà présente : jamais écrasée.
    ecrire(&d.join(".actors/Carrie-Anne_Moss.jpg"), b"ancienne");
    let rapport = b
        .app
        .scraper_films(vec![id.clone()], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    assert_eq!(rapport.reussis, vec![id.clone()], "{rapport:?}");
    assert!(lire(&d.join(".actors/Keanu_Reeves.jpg")).contains("keanu"));
    assert_eq!(lire(&d.join(".actors/Carrie-Anne_Moss.jpg")), "ancienne");
    assert!(!d.join(".actors/Hugo_Weaving.jpg").exists(), "404 toléré");
    assert!(!d.join(".actors/Sans_Photo.jpg").exists());
    let nfo = lire(&d.join("The Matrix (1999).nfo"));
    assert!(
        nfo.contains("<thumb>.actors/Keanu_Reeves.jpg</thumb>"),
        "{nfo}"
    );
    assert!(
        nfo.contains("<thumb>.actors/Carrie-Anne_Moss.jpg</thumb>"),
        "{nfo}"
    );
    assert!(
        nfo.contains("/img/h632/bad.jpg"),
        "URL conservée si pas de fichier : {nfo}"
    );
}

#[test]
fn photos_des_acteurs_desactivees_par_defaut() {
    let b = banc(|_| {});
    let id = film_id(&b);
    b.app
        .scraper_films(vec![id], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    assert!(!b.films.join("The Matrix (1999)/.actors").exists());
}

#[test]
fn photos_acteurs_et_images_supplementaires_series() {
    let b = banc(|r| {
        r.series.images_acteurs = true;
        r.series.nombre_affiches = 2;
        r.series.nombre_fanarts = 2;
    });
    let id = serie_id(&b);
    let rapport = b
        .app
        .scraper_series(vec![id.clone()], ConfigSerie::default())
        .attendre()
        .unwrap();
    assert_eq!(rapport.reussis, vec![id.clone()], "{rapport:?}");
    let d = b.series.join("Ma Serie (2020)");
    assert!(lire(&d.join(".actors/Actrice_Une.jpg")).contains("act"));
    assert!(lire(&d.join("poster.jpg")).contains("sp1"));
    assert!(lire(&d.join("poster1.jpg")).contains("sp2"));
    assert!(lire(&d.join("fanart.jpg")).contains("sf1"));
    assert!(lire(&d.join("extrafanart/fanart1.jpg")).contains("sf2"));
    let nfo = lire(&d.join("tvshow.nfo"));
    assert!(
        nfo.contains("<thumb>.actors/Actrice_Une.jpg</thumb>"),
        "{nfo}"
    );
}
