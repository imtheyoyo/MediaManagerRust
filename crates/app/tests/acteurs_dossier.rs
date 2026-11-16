//! Tests d'intégration du dossier `.actors` : emplacement, renommage,
//! nettoyage, mise à jour des sources, rapport de téléchargement. Faux serveur
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

#[allow(dead_code)]
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

fn lire(p: &Path) -> String {
    String::from_utf8_lossy(&std::fs::read(p).unwrap_or_default()).into_owned()
}

fn sans_artwork() -> ConfigMetadonnees {
    ConfigMetadonnees {
        artwork: false,
        ..ConfigMetadonnees::tout()
    }
}

fn noms_dossier(d: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(d)
        .map(|r| {
            r.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Aucun dossier `.actors` ailleurs que dans `attendus`, sous `racine`.
fn verifier_actors_uniquement_dans(racine: &Path, attendus: &[PathBuf]) {
    let trouves: Vec<PathBuf> = walkdir_actors(racine);
    for t in &trouves {
        assert!(attendus.contains(t), "`.actors` inattendu : {t:?}");
    }
    for a in attendus {
        assert!(trouves.contains(a), "`.actors` absent : {a:?}");
    }
}

fn walkdir_actors(racine: &Path) -> Vec<PathBuf> {
    let mut sortie = Vec::new();
    let mut pile = vec![racine.to_path_buf()];
    while let Some(d) = pile.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                if e.file_name() == ".actors" {
                    sortie.push(p.clone());
                }
                pile.push(p);
            }
        }
    }
    sortie
}

#[test]
fn film_acteurs_a_cote_de_la_video_meme_sans_artwork() {
    let b = banc(|r| r.films.images_acteurs = true);
    let id = film_id(&b);
    let rapport = b
        .app
        .scraper_films(vec![id.clone()], sans_artwork())
        .attendre()
        .unwrap();
    assert_eq!(rapport.reussis, vec![id], "{rapport:?}");
    let d = b.films.join("The Matrix (1999)");
    // Deux photos écrites, une en 404.
    assert_eq!(rapport.photos_acteurs, 2, "{rapport:?}");
    assert_eq!(rapport.photos_acteurs_echecs, 1, "{rapport:?}");
    assert!(lire(&d.join(".actors/Keanu_Reeves.jpg")).contains("keanu"));
    assert!(lire(&d.join(".actors/Carrie-Anne_Moss.jpg")).contains("carrie"));
    // Pas d'affiche (artwork désactivé) mais NFO à côté de la vidéo.
    assert!(!d.join("The Matrix (1999)-poster.jpg").exists());
    assert!(d.join("The Matrix (1999).nfo").exists());
    // Uniquement dans le dossier du film : ni racine de source, ni données.
    verifier_actors_uniquement_dans(b._tmp.path(), &[d.join(".actors")]);
    assert!(
        lire(&d.join("The Matrix (1999).nfo")).contains("<thumb>.actors/Keanu_Reeves.jpg</thumb>")
    );
}

#[test]
fn film_acteurs_avec_artwork_meme_dossier_que_les_images() {
    let b = banc(|r| r.films.images_acteurs = true);
    let id = film_id(&b);
    b.app
        .scraper_films(vec![id], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    let d = b.films.join("The Matrix (1999)");
    assert!(d.join("The Matrix (1999)-poster.jpg").exists());
    assert!(d.join(".actors/Keanu_Reeves.jpg").exists());
    verifier_actors_uniquement_dans(b._tmp.path(), &[d.join(".actors")]);
}

#[test]
fn film_option_acteurs_inactive_aucun_dossier() {
    let b = banc(|_| {});
    let id = film_id(&b);
    let r = b
        .app
        .scraper_films(vec![id], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    assert_eq!((r.photos_acteurs, r.photos_acteurs_echecs), (0, 0));
    verifier_actors_uniquement_dans(b._tmp.path(), &[]);
}

#[test]
fn choix_manuel_telecharge_les_acteurs_sans_artwork() {
    let b = banc(|r| r.films.images_acteurs = true);
    let id = film_id(&b);
    let choix = b
        .app
        .rechercher_film(&id, None)
        .attendre()
        .unwrap()
        .remove(0);
    let film = b
        .app
        .appliquer_resultat(&id, choix, sans_artwork())
        .attendre()
        .unwrap();
    let d = b.films.join("The Matrix (1999)");
    assert_eq!(film.chemin, d);
    assert!(lire(&d.join(".actors/Keanu_Reeves.jpg")).contains("keanu"));
    assert!(lire(&d.join(".actors/Carrie-Anne_Moss.jpg")).contains("carrie"));
    verifier_actors_uniquement_dans(b._tmp.path(), &[d.join(".actors")]);
}

#[test]
fn renommage_du_dossier_emporte_actors_et_le_nfo_le_reference() {
    let b = banc(|r| {
        r.films.images_acteurs = true;
        r.films.renommer_apres_scraping = true;
    });
    let id = film_id(&b);
    let choix = b
        .app
        .rechercher_film(&id, None)
        .attendre()
        .unwrap()
        .remove(0);
    let (film, rapport) = b
        .app
        .appliquer_resultat_avec_renommage(&id, choix, sans_artwork())
        .attendre()
        .unwrap();
    assert_eq!(rapport.unwrap().renommes, vec![id.clone()]);
    let nouveau = b.films.join("Matrix (1999)");
    assert_eq!(film.chemin, nouveau);
    assert!(!b.films.join("The Matrix (1999)").exists());
    assert!(lire(&nouveau.join(".actors/Keanu_Reeves.jpg")).contains("keanu"));
    let nfo = lire(&nouveau.join("Matrix (1999).nfo"));
    assert!(
        nfo.contains("<thumb>.actors/Keanu_Reeves.jpg</thumb>"),
        "{nfo}"
    );
    assert!(
        nfo.contains("<thumb>.actors/Carrie-Anne_Moss.jpg</thumb>"),
        "{nfo}"
    );
    verifier_actors_uniquement_dans(b._tmp.path(), &[nouveau.join(".actors")]);
}

#[test]
fn renommage_automatique_emporte_actors() {
    let b = banc(|r| {
        r.films.images_acteurs = true;
        r.films.renommer_apres_scraping = true;
    });
    let id = film_id(&b);
    let rapport = b
        .app
        .scraper_films(vec![id], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    assert!(rapport.renommage.is_some());
    let nouveau = b.films.join("Matrix (1999)");
    assert!(nouveau.join(".actors/Keanu_Reeves.jpg").exists());
    verifier_actors_uniquement_dans(b._tmp.path(), &[nouveau.join(".actors")]);
    // Le plan d'un renommage ultérieur ne touche pas à `.actors` (déjà bon).
    let apercu = b.app.apercu_renommage_films(&[film_id(&b)]).unwrap();
    assert!(
        apercu.elements.iter().all(|e| e.est_inchange()),
        "{apercu:?}"
    );
}

#[test]
fn renommage_des_fichiers_seuls_laisse_actors_en_place() {
    let b = banc(|r| {
        r.films.images_acteurs = true;
        r.renommeur.gabarit_dossier_film = String::new();
        r.renommeur.gabarit_fichier_film = "${title}".into();
    });
    let id = film_id(&b);
    b.app
        .scraper_films(vec![id.clone()], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    let d = b.films.join("The Matrix (1999)");
    let r = b.app.renommer_films(vec![id.clone()]).attendre().unwrap();
    assert_eq!(r.renommes, vec![id], "{r:?}");
    assert!(d.join("Matrix.mkv").exists(), "{:?}", noms_dossier(&d));
    assert!(d.join(".actors/Keanu_Reeves.jpg").exists());
    let nfo = lire(&d.join("Matrix.nfo"));
    assert!(
        nfo.contains("<thumb>.actors/Keanu_Reeves.jpg</thumb>"),
        "{nfo}"
    );
    verifier_actors_uniquement_dans(b._tmp.path(), &[d.join(".actors")]);
}

#[test]
fn nettoyage_ne_propose_ni_ne_supprime_actors() {
    let b = banc(|r| r.films.images_acteurs = true);
    let id = film_id(&b);
    b.app
        .scraper_films(vec![id.clone()], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    let d = b.films.join("The Matrix (1999)");
    ecrire(&d.join("notes.txt"), b"x");
    let liste = b.app.analyser_nettoyage(std::slice::from_ref(&id)).unwrap();
    assert!(
        liste
            .iter()
            .all(|f| !f.chemin.starts_with(d.join(".actors"))),
        "{liste:?}"
    );
    assert!(liste.iter().any(|f| f.chemin.ends_with("notes.txt")));
    // Même demandé explicitement, `.actors` n'est pas mis à la corbeille.
    let force = vec![
        mediamanager_app::nettoyage::FichierInutile {
            id_film: id.clone(),
            chemin: d.join(".actors"),
            est_dossier: true,
            raison: mediamanager_core::nettoyage::RaisonInutile::Parasite,
            taille: 0,
        },
        mediamanager_app::nettoyage::FichierInutile {
            id_film: id,
            chemin: d.join(".actors/Keanu_Reeves.jpg"),
            est_dossier: false,
            raison: mediamanager_core::nettoyage::RaisonInutile::Parasite,
            taille: 0,
        },
    ];
    let _ = b.app.executer_nettoyage(force).attendre();
    assert!(d.join(".actors/Keanu_Reeves.jpg").exists());
    assert!(!d.join(".corbeille").exists());
}

#[test]
fn mise_a_jour_des_sources_ignore_actors() {
    let b = banc(|r| r.films.images_acteurs = true);
    let id = film_id(&b);
    b.app
        .scraper_films(vec![id.clone()], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    let d = b.films.join("The Matrix (1999)");
    // Un `.actors` parasite à la racine de la source ne crée pas de film.
    ecrire(&b.films.join(".actors/Quelquun.jpg"), b"x");
    b.app.mettre_a_jour_sources_films().attendre().unwrap();
    b.app
        .mettre_a_jour_selection_films(vec![id.clone()])
        .attendre()
        .unwrap();
    let films = b.app.films().unwrap();
    assert_eq!(films.len(), 1, "film fantôme : {films:?}");
    let f = b.app.film(&id).unwrap().unwrap();
    assert!(
        f.fichiers
            .iter()
            .all(|x| !x.chemin_complet().starts_with(d.join(".actors"))),
        "{:?}",
        f.fichiers
    );
    assert!(d.join(".actors/Keanu_Reeves.jpg").exists());
}

#[test]
fn films_a_plat_actors_dans_le_dossier_commun_sans_ecrasement() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let serveur = monter_tmdb(&rt);
    let d = tempfile::tempdir().unwrap();
    let films = d.path().join("films");
    ecrire(&films.join("The Matrix (1999).mkv"), b"video");
    let app = Application::demarrer(options(d.path(), format!("{}/3/", serveur.uri()))).unwrap();
    app.definir_cle_tmdb(CLE_TEST).unwrap();
    let f = films.clone();
    app.modifier_reglages(move |r| {
        r.films.sources = vec![f];
        r.films.images_acteurs = true;
        r.renommeur.gabarit_dossier_film = String::new();
        r.renommeur.gabarit_fichier_film = String::new();
    })
    .unwrap();
    app.mettre_a_jour_sources_films().attendre().unwrap();
    let id = app.films().unwrap().remove(0).id;
    ecrire(&films.join(".actors/Carrie-Anne_Moss.jpg"), b"ancienne");
    let r = app
        .scraper_films(vec![id.clone()], sans_artwork())
        .attendre()
        .unwrap();
    assert_eq!(r.reussis, vec![id.clone()], "{r:?}");
    // Convention classique : `.actors` dans le dossier de la vidéo.
    assert!(lire(&films.join(".actors/Keanu_Reeves.jpg")).contains("keanu"));
    assert_eq!(
        lire(&films.join(".actors/Carrie-Anne_Moss.jpg")),
        "ancienne"
    );
    assert_eq!(r.photos_acteurs, 1, "{r:?}");
    verifier_actors_uniquement_dans(d.path(), &[films.join(".actors")]);
    // Pas de film fantôme après une mise à jour.
    app.mettre_a_jour_sources_films().attendre().unwrap();
    assert_eq!(app.films().unwrap().len(), 1);
    assert!(
        app.film(&id)
            .unwrap()
            .unwrap()
            .fichiers
            .iter()
            .all(|x| !x.chemin_complet().starts_with(films.join(".actors")))
    );
}

#[test]
fn serie_acteurs_dans_le_dossier_de_la_serie_meme_sans_artwork() {
    let b = banc(|r| r.series.images_acteurs = true);
    let id = serie_id(&b);
    let config = ConfigSerie {
        artwork: false,
        artwork_episodes: false,
        ..ConfigSerie::default()
    };
    let rapport = b.app.scraper_series(vec![id], config).attendre().unwrap();
    let d = b.series.join("Ma Serie (2020)");
    assert_eq!(rapport.photos_acteurs, 1, "{rapport:?}");
    assert_eq!(rapport.photos_acteurs_echecs, 0);
    assert!(lire(&d.join(".actors/Actrice_Une.jpg")).contains("act"));
    assert!(!d.join("poster.jpg").exists());
    verifier_actors_uniquement_dans(b._tmp.path(), &[d.join(".actors")]);
}

#[test]
fn serie_renommage_emporte_actors_et_nfo_episodes_relatifs() {
    let b = banc(|r| {
        r.series.images_acteurs = true;
        r.series.nfo_tous_acteurs = true;
        r.series.renommer_apres_scraping = true;
        r.renommeur.gabarit_dossier_serie = "${showTitle} (${showYear})".into();
    });
    let id = serie_id(&b);
    let rapport = b
        .app
        .scraper_series(vec![id.clone()], ConfigSerie::default())
        .attendre()
        .unwrap();
    assert_eq!(rapport.reussis, vec![id.clone()], "{rapport:?}");
    let s = b.app.serie(&id).unwrap().unwrap();
    assert!(
        s.chemin.join(".actors/Actrice_Une.jpg").exists(),
        "{:?}",
        s.chemin
    );
    assert!(lire(&s.chemin.join("tvshow.nfo")).contains("<thumb>.actors/Actrice_Une.jpg</thumb>"));
    verifier_actors_uniquement_dans(b._tmp.path(), &[s.chemin.join(".actors")]);
    // NFO d'épisode : chemin relatif correct depuis son dossier.
    let mut nfos = Vec::new();
    let mut pile = vec![s.chemin.clone()];
    while let Some(x) = pile.pop() {
        for e in std::fs::read_dir(&x).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() && e.file_name() != ".actors" {
                pile.push(p);
            } else if p.extension().is_some_and(|x| x == "nfo") && !p.ends_with("tvshow.nfo") {
                nfos.push(p);
            }
        }
    }
    assert!(!nfos.is_empty());
    let mut verifies = 0;
    for n in nfos {
        let txt = lire(&n);
        if txt.contains("<episodedetails") && txt.contains("Actrice Une") {
            let niveau = n.parent().unwrap() == s.chemin;
            let attendu = if niveau {
                "<thumb>.actors/Actrice_Une.jpg</thumb>"
            } else {
                "<thumb>../.actors/Actrice_Une.jpg</thumb>"
            };
            assert!(txt.contains(attendu), "{n:?} : {txt}");
            verifies += 1;
        }
    }
    assert!(verifies > 0, "aucun NFO d'épisode vérifié");
}
