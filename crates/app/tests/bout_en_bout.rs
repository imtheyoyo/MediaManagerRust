//! Tests d'intégration : arborescences et base temporaires, faux serveur TMDB
//! local (wiremock). Aucun appel réseau réel.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use mediamanager_app::*;
use mediamanager_core::modele::{Film, TypeFichier};
use mediamanager_store::{CoffreMemoire, DossiersApplication, ModeDossiers};
use serde_json::json;
use wiremock::matchers::{method, path, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CLE_TEST: &str = "cle-de-test-factice";

fn options(racine: &Path, services: ConfigServices) -> OptionsDemarrage {
    OptionsDemarrage {
        dossiers: Some(DossiersApplication::depuis_racine(
            racine.join("app"),
            ModeDossiers::Explicite,
        )),
        coffre: Some(Box::new(CoffreMemoire::default())),
        journal: OptionsJournal {
            actif: true,
            ..OptionsJournal::default()
        },
        services,
    }
}

fn services(url: Option<String>) -> ConfigServices {
    ConfigServices {
        url_api_tmdb: url,
        cache_http: false,
        horodatage_nfo: false,
        ..ConfigServices::default()
    }
}

fn ecrire(p: &Path, contenu: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, contenu).unwrap();
}

/// Deux films : un sans NFO, un avec NFO.
fn mediatheque(racine: &Path) -> PathBuf {
    let films = racine.join("films");
    ecrire(
        &films.join("The Matrix (1999)/The Matrix (1999).mkv"),
        b"video matrix",
    );
    ecrire(
        &films.join("Inception (2010)/Inception.2010.mkv"),
        b"video inception",
    );
    ecrire(
        &films.join("Inception (2010)/Inception.2010.nfo"),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<movie><title>Origine (NFO)</title>\
         <year>2010</year><plot>Lu depuis le NFO.</plot>\
         <uniqueid type=\"tmdb\">27205</uniqueid></movie>"
            .as_bytes(),
    );
    films
}

fn demarrer_avec_source(racine: &Path, url: Option<String>) -> (Application, PathBuf) {
    let films = mediatheque(racine);
    let app = Application::demarrer(options(racine, services(url))).unwrap();
    let f = films.clone();
    app.modifier_reglages(move |r| r.films.sources = vec![f])
        .unwrap();
    (app, films)
}

fn film_par_titre(app: &Application, debut: &str) -> Film {
    app.films()
        .unwrap()
        .into_iter()
        .find(|f| f.titre.starts_with(debut))
        .unwrap_or_else(|| panic!("film « {debut} » absent"))
}

fn vider(rx: &Receiver<Evenement>) -> Vec<Evenement> {
    rx.try_iter().collect()
}

#[test]
fn mise_a_jour_des_sources_films() {
    let d = tempfile::tempdir().unwrap();
    let (app, films) = demarrer_avec_source(d.path(), None);
    let abonne = app.abonner();

    let tache = app.mettre_a_jour_sources_films();
    let r = tache.attendre().unwrap();
    assert_eq!(r.ajoutes.len(), 2, "{r:?}");
    assert_eq!(r.sources_traitees, vec![films.clone()]);

    let inception = film_par_titre(&app, "Origine");
    assert_eq!(inception.resume, "Lu depuis le NFO.");
    assert_eq!(inception.ids.tmdb(), 27205);
    assert!(inception.chemin.ends_with("Inception (2010)"));
    let matrix = film_par_titre(&app, "The Matrix");
    assert_eq!(matrix.annee, 1999);
    let mut nouveaux = app.films_nouveaux().unwrap();
    nouveaux.sort();
    let mut attendus = vec![inception.id.clone(), matrix.id.clone()];
    attendus.sort();
    assert_eq!(nouveaux, attendus);

    let evts = vider(&abonne);
    assert!(
        evts.iter()
            .any(|e| matches!(e, Evenement::TacheDemarree { .. }))
    );
    assert!(
        evts.iter()
            .any(|e| matches!(e, Evenement::Progression { .. }))
    );
    assert_eq!(
        evts.iter()
            .filter(|e| matches!(e, Evenement::FilmAjoute { .. }))
            .count(),
        2
    );

    // Métadonnées modifiées par l'utilisateur, puis nouveau scan : conservées.
    app.avec_stockage(|s| {
        let mut m = s.bibliotheque.obtenir::<Film>(&matrix.id).unwrap().unwrap();
        m.resume = "Saisi à la main".into();
        s.bibliotheque.enregistrer(&mut m).unwrap();
    });
    let r2 = app.mettre_a_jour_sources_films().attendre().unwrap();
    assert!(r2.ajoutes.is_empty());
    assert!(r2.supprimes.is_empty());
    assert_eq!(
        app.film(&matrix.id).unwrap().unwrap().resume,
        "Saisi à la main"
    );
    assert!(app.films_nouveaux().unwrap().is_empty());

    // Film verrouillé disparu : conservé ; film non verrouillé disparu : retiré.
    app.avec_stockage(|s| {
        let mut i = s
            .bibliotheque
            .obtenir::<Film>(&inception.id)
            .unwrap()
            .unwrap();
        i.verrouille = true;
        s.bibliotheque.enregistrer(&mut i).unwrap();
    });
    std::fs::remove_dir_all(films.join("Inception (2010)")).unwrap();
    std::fs::remove_dir_all(films.join("The Matrix (1999)")).unwrap();
    ecrire(&films.join("Autre (2001)/Autre.mkv"), b"x");
    let r3 = app.mettre_a_jour_sources_films().attendre().unwrap();
    assert_eq!(r3.supprimes, vec![matrix.id.clone()]);
    assert_eq!(r3.verrouilles, 1);
    assert!(app.film(&inception.id).unwrap().is_some());
    assert!(app.film(&matrix.id).unwrap().is_none());

    // Source indisponible : rien n'est retiré.
    std::fs::remove_dir_all(&films).unwrap();
    let r4 = app.mettre_a_jour_sources_films().attendre().unwrap();
    assert_eq!(r4.sources_indisponibles.len(), 1);
    assert_eq!(app.films().unwrap().len(), 2);
    app.arreter().unwrap();
}

#[test]
fn films_disparus_marques_selon_le_reglage() {
    let d = tempfile::tempdir().unwrap();
    let films = mediatheque(d.path());
    let mut s = services(None);
    s.mise_a_jour.disparus = TraitementDisparus::Marquer;
    let app = Application::demarrer(options(d.path(), s)).unwrap();
    let f = films.clone();
    app.modifier_reglages(move |r| r.films.sources = vec![f])
        .unwrap();
    app.mettre_a_jour_sources_films().attendre().unwrap();
    let matrix = film_par_titre(&app, "The Matrix");
    std::fs::remove_dir_all(films.join("The Matrix (1999)")).unwrap();
    let r = app.mettre_a_jour_sources_films().attendre().unwrap();
    assert!(r.supprimes.is_empty());
    assert_eq!(r.marques_disparus, vec![matrix.id.clone()]);
    assert_eq!(app.films_disparus().unwrap(), vec![matrix.id.clone()]);
    assert!(app.film(&matrix.id).unwrap().is_some());
    // Le film réapparaît : la marque est levée.
    ecrire(
        &films.join("The Matrix (1999)/The Matrix (1999).mkv"),
        b"video matrix",
    );
    app.mettre_a_jour_sources_films().attendre().unwrap();
    assert!(app.films_disparus().unwrap().is_empty());
    app.arreter().unwrap();
}

#[test]
fn mise_a_jour_des_sources_series_avec_nfo() {
    let d = tempfile::tempdir().unwrap();
    let series = d.path().join("series");
    ecrire(
        &series.join("Ma Serie (2020)/Season 1/Ma Serie - S01E01.mkv"),
        b"ep1",
    );
    ecrire(
        &series.join("Ma Serie (2020)/Season 1/Ma Serie - S01E01.nfo"),
        b"<episodedetails><title>Le pilote</title><season>1</season><episode>1</episode></episodedetails>",
    );
    ecrire(
        &series.join("Ma Serie (2020)/tvshow.nfo"),
        b"<tvshow><title>Ma S\xc3\xa9rie (NFO)</title><year>2020</year></tvshow>",
    );
    let app = Application::demarrer(options(d.path(), services(None))).unwrap();
    let s = series.clone();
    app.modifier_reglages(move |r| r.series.sources = vec![s])
        .unwrap();
    let r = app.mettre_a_jour_sources_series().attendre().unwrap();
    assert_eq!(r.ajoutes.len(), 1, "{r:?}");
    let serie = &app.series().unwrap()[0];
    assert_eq!(serie.titre, "Ma Série (NFO)");
    assert_eq!(serie.episodes.len(), 1);
    assert_eq!(serie.episodes[0].titre, "Le pilote");
    assert_eq!(app.series_nouvelles().unwrap(), vec![serie.id.clone()]);

    // Nouvel épisode : la série est modifiée, le titre du premier conservé.
    ecrire(
        &series.join("Ma Serie (2020)/Season 1/Ma Serie - S01E02.mkv"),
        b"ep2",
    );
    let r2 = app.mettre_a_jour_sources_series().attendre().unwrap();
    assert_eq!(r2.modifies, vec![serie.id.clone()]);
    let serie = &app.series().unwrap()[0];
    assert_eq!(serie.episodes.len(), 2);
    assert!(serie.episodes.iter().any(|e| e.titre == "Le pilote"));
    app.arreter().unwrap();
}

#[test]
fn cle_tmdb_absente_erreur_claire() {
    let d = tempfile::tempdir().unwrap();
    let (app, _) = demarrer_avec_source(d.path(), Some("http://127.0.0.1:9/3/".into()));
    app.mettre_a_jour_sources_films().attendre().unwrap();
    let ids: Vec<String> = app.films().unwrap().into_iter().map(|f| f.id).collect();
    let tache = app.scraper_films(ids, ConfigMetadonnees::tout());
    let err = tache.attendre().unwrap_err();
    assert!(matches!(err, ErreurApp::CleTmdbAbsente), "{err:?}");
    assert!(err.to_string().contains("clé d'API TMDB"));
    app.arreter().unwrap();
}

fn monter_tmdb(rt: &tokio::runtime::Runtime) -> MockServer {
    rt.block_on(async {
        let s = MockServer::start().await;
        let base_images = format!("{}/img/", s.uri());
        Mock::given(method("GET"))
            .and(path("/3/configuration"))
            .and(query_param("api_key", CLE_TEST))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                json!({"images": {"secure_base_url": base_images}}),
            ))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/3/search/movie"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "page": 1, "total_pages": 1,
                "results": [
                    {"id": 603, "title": "Matrix", "original_title": "The Matrix",
                     "original_language": "en", "release_date": "1999-03-30",
                     "poster_path": "/m.jpg", "overview": "Néo..."},
                    {"id": 604, "title": "Matrix Reloaded", "original_title": "The Matrix Reloaded",
                     "release_date": "2003-05-15", "poster_path": "/r.jpg"}
                ]
            })))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/3/movie/603"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 603, "title": "Matrix", "original_title": "The Matrix",
                "original_language": "en", "overview": "Un pirate découvre la vérité.",
                "tagline": "Bienvenue dans le monde réel.", "runtime": 136,
                "release_date": "1999-03-30", "vote_average": 8.2, "vote_count": 20000,
                "poster_path": "/m.jpg",
                "genres": [{"id": 28, "name": "Action"}, {"id": 878, "name": "Science-Fiction"}],
                "belongs_to_collection": {"id": 2344, "name": "Matrix - La saga"},
                "credits": {
                    "cast": [{"id": 6384, "name": "Keanu Reeves", "character": "Neo"}],
                    "crew": [{"id": 9339, "name": "Lana Wachowski", "job": "Director", "department": "Directing"}]
                },
                "external_ids": {"imdb_id": "tt0133093"}
            })))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/3/movie/603/images"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "posters": [{"file_path": "/m.jpg", "iso_639_1": "fr", "vote_count": 3, "width": 1000, "height": 1500}],
                "backdrops": [{"file_path": "/f.jpg", "iso_639_1": null, "vote_count": 5, "width": 1920, "height": 1080}],
                "logos": []
            })))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path_regex("^/img/.+"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"\xFF\xD8\xFFimage".to_vec()))
            .mount(&s)
            .await;
        s
    })
}

#[test]
fn scan_scrap_nfo_renommage_bout_en_bout() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let serveur = monter_tmdb(&rt);
    let d = tempfile::tempdir().unwrap();
    let (app, films) = demarrer_avec_source(d.path(), Some(format!("{}/3/", serveur.uri())));
    app.definir_cle_tmdb(CLE_TEST).unwrap();
    app.modifier_reglages(|r| {
        r.renommeur.gabarit_dossier_film = "${title} (${year})".into();
        r.renommeur.gabarit_fichier_film = "${title} (${year})".into();
        r.general.corbeille_activee = false;
    })
    .unwrap();

    // 1. Scan.
    app.mettre_a_jour_sources_films().attendre().unwrap();
    let matrix = film_par_titre(&app, "The Matrix");

    // 2. Recherche pour un dialogue de choix : résultats scorés.
    let resultats = app.rechercher_film(&matrix.id, None).attendre().unwrap();
    assert_eq!(resultats[0].ids["tmdb"], "603");
    assert!(resultats[0].score > resultats[1].score);

    // 3. Récupération automatique.
    let tache = app.scraper_films(vec![matrix.id.clone()], ConfigMetadonnees::tout());
    let rapport = tache.attendre().unwrap();
    assert_eq!(rapport.reussis, vec![matrix.id.clone()], "{rapport:?}");

    let f = app.film(&matrix.id).unwrap().unwrap();
    assert_eq!(f.titre, "Matrix");
    assert_eq!(f.ids.imdb(), "tt0133093");
    assert_eq!(f.duree_minutes, 136);
    assert_eq!(f.genres, vec!["Action", "Science Fiction"]);
    assert_eq!(f.realisateurs()[0].nom, "Lana Wachowski");
    let saga_id = f.saga_id.clone().expect("saga liée");
    let sagas = app.sagas().unwrap();
    assert_eq!(sagas.len(), 1);
    assert_eq!(sagas[0].id, saga_id);
    assert_eq!(sagas[0].films, vec![matrix.id.clone()]);

    let ancien = films.join("The Matrix (1999)");
    let poster = ancien.join("The Matrix (1999)-poster.jpg");
    let fanart = ancien.join("The Matrix (1999)-fanart.jpg");
    let nfo = ancien.join("The Matrix (1999).nfo");
    assert_eq!(std::fs::read(&poster).unwrap(), b"\xFF\xD8\xFFimage");
    assert!(fanart.exists());
    let contenu = std::fs::read_to_string(&nfo).unwrap();
    assert!(contenu.contains("<title>Matrix</title>"), "{contenu}");
    assert!(contenu.contains("tt0133093"));
    assert!(f.fichiers_de_type(TypeFichier::Poster).next().is_some());
    assert!(f.fichiers_de_type(TypeFichier::Nfo).next().is_some());

    // 4. Aperçu puis renommage.
    let plan = app
        .apercu_renommage_films(std::slice::from_ref(&matrix.id))
        .unwrap();
    assert!(!plan.a_des_conflits(), "{:?}", plan.conflits());
    let nouveau = films.join("Matrix (1999)");
    assert_eq!(plan.elements[0].nouveau_dossier, nouveau);
    assert!(ancien.exists(), "l'aperçu ne doit rien écrire");

    let r = app
        .renommer_films(vec![matrix.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(r.renommes, vec![matrix.id.clone()], "{r:?}");
    assert!(!ancien.exists());
    assert!(nouveau.join("Matrix (1999).mkv").exists());
    assert!(nouveau.join("Matrix (1999)-poster.jpg").exists());
    let nfo2 = nouveau.join("Matrix (1999).nfo");
    assert!(
        std::fs::read_to_string(&nfo2)
            .unwrap()
            .contains("<title>Matrix</title>")
    );

    // 5. Base à jour.
    let f = app.film(&matrix.id).unwrap().unwrap();
    assert_eq!(f.chemin, nouveau);
    assert!(f.fichiers.iter().all(|x| x.chemin.starts_with(&nouveau)));
    assert!(f.historique_renommage.is_some());
    assert_eq!(f.fichier_principal().unwrap().nom, "Matrix (1999).mkv");

    // Un nouveau scan retrouve le film renommé sans doublon.
    let r = app.mettre_a_jour_sources_films().attendre().unwrap();
    assert!(r.ajoutes.is_empty(), "{r:?}");
    assert_eq!(app.film(&matrix.id).unwrap().unwrap().titre, "Matrix");
    app.arreter().unwrap();
    drop(serveur);
}

#[test]
fn application_du_choix_selon_la_configuration() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let serveur = monter_tmdb(&rt);
    let d = tempfile::tempdir().unwrap();
    let (app, _) = demarrer_avec_source(d.path(), Some(format!("{}/3/", serveur.uri())));
    app.definir_cle_tmdb(CLE_TEST).unwrap();
    app.mettre_a_jour_sources_films().attendre().unwrap();
    let matrix = film_par_titre(&app, "The Matrix");
    let choix = app
        .rechercher_film(
            &matrix.id,
            Some(RequeteFilm {
                texte: "Matrix".into(),
                annee: Some(1999),
                ..RequeteFilm::default()
            }),
        )
        .attendre()
        .unwrap()
        .remove(0);
    let config = ConfigMetadonnees {
        titre: false,
        artwork: false,
        saga: false,
        ..ConfigMetadonnees::tout()
    };
    let f = app
        .appliquer_resultat(&matrix.id, choix, config)
        .attendre()
        .unwrap();
    assert_eq!(f.titre, "The Matrix", "titre non écrasé");
    assert_eq!(f.resume, "Un pirate découvre la vérité.");
    assert!(f.saga_id.is_none());
    assert!(f.fichiers_de_type(TypeFichier::Poster).next().is_none());
    assert!(app.sagas().unwrap().is_empty());

    // Film verrouillé : refusé.
    app.avec_stockage(|s| {
        let mut m = s.bibliotheque.obtenir::<Film>(&matrix.id).unwrap().unwrap();
        m.verrouille = true;
        s.bibliotheque.enregistrer(&mut m).unwrap();
    });
    let r = app
        .scraper_films(vec![matrix.id.clone()], ConfigMetadonnees::tout())
        .attendre()
        .unwrap();
    assert_eq!(r.verrouilles, vec![matrix.id.clone()]);
    app.arreter().unwrap();
}

#[test]
fn taches_annulation_file_principale_et_panique() {
    let d = tempfile::tempdir().unwrap();
    let app = Application::demarrer(options(d.path(), services(None))).unwrap();
    let taches = app.taches();

    // Tâche longue annulée.
    let longue = taches.lancer("longue", TypeTache::Principale, |ctx| {
        for i in 0..1000u64 {
            ctx.verifier_annulation()?;
            ctx.progression(i, 1000, "attente");
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    });
    // Tâche principale suivante : attend la fin de la première.
    let suivante = taches.lancer("suivante", TypeTache::Principale, |_| Ok(42));
    // Tâche de fond : s'exécute sans attendre.
    let fond = taches.lancer("fond", TypeTache::Fond, |_| Ok("fond"));
    assert_eq!(fond.attendre().unwrap(), "fond");
    assert!(suivante.essayer_resultat().is_none());
    assert_eq!(taches.taches_actives().len(), 2);

    std::thread::sleep(Duration::from_millis(30));
    longue.annuler();
    let evts: Vec<Evenement> = longue.evenements.iter().collect();
    assert!(matches!(
        evts.last(),
        Some(Evenement::TacheTerminee {
            issue: IssueTache::Annulee,
            ..
        })
    ));
    assert!(
        evts.iter()
            .any(|e| matches!(e, Evenement::Progression { .. }))
    );
    assert_eq!(suivante.attendre().unwrap(), 42);

    // Une panique ne fait pas tomber l'application.
    let p = taches.lancer::<(), _>("panique", TypeTache::Fond, |_| panic!("boum"));
    assert!(matches!(p.attendre(), Err(ErreurApp::Interne(m)) if m.contains("boum")));
    assert!(taches.attendre_inactivite(Duration::from_secs(2)));
    app.arreter().unwrap();
}

#[test]
fn arret_propre_libere_le_verrou_et_refuse_les_taches() {
    let d = tempfile::tempdir().unwrap();
    let app = Application::demarrer(options(d.path(), services(None))).unwrap();
    // Seconde instance refusée tant que la première tourne.
    assert!(Application::demarrer(options(d.path(), services(None))).is_err());
    let gestionnaire = app.taches().clone();
    app.arreter().unwrap();
    let t = gestionnaire.lancer("après arrêt", TypeTache::Fond, |_| Ok(()));
    assert!(matches!(t.attendre(), Err(ErreurApp::ArretEnCours)));
    let app2 = Application::demarrer(options(d.path(), services(None))).unwrap();
    app2.arreter().unwrap();
}
