//! Tests d'intégration des séries (scan → scrap → NFO → renommage → base) et
//! de la gestion des films (sélection, saga, retrait, annulation). Faux
//! serveur TMDB local (wiremock), arborescences temporaires : aucun appel
//! réseau réel.

use std::path::{Path, PathBuf};

use mediamanager_app::*;
use mediamanager_core::modele::commun::cles_id;
use mediamanager_core::modele::{Film, Identifiants, Saga, TypeFichier};
use mediamanager_store::{CoffreMemoire, DossiersApplication, ModeDossiers};
use serde_json::json;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CLE_TEST: &str = "cle-de-test-factice";

fn options(racine: &Path, url: Option<String>) -> OptionsDemarrage {
    OptionsDemarrage {
        dossiers: Some(DossiersApplication::depuis_racine(
            racine.join("app"),
            ModeDossiers::Explicite,
        )),
        coffre: Some(Box::new(CoffreMemoire::default())),
        journal: OptionsJournal::default(),
        services: ConfigServices {
            url_api_tmdb: url,
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

fn image(file_path: &str, langue: Option<&str>) -> serde_json::Value {
    json!({"file_path": file_path, "iso_639_1": langue, "vote_count": 1, "width": 1000, "height": 1500})
}

fn monter_tmdb(rt: &tokio::runtime::Runtime) -> MockServer {
    rt.block_on(async {
        let s = MockServer::start().await;
        let base_images = format!("{}/img/", s.uri());
        let reponse = |v: serde_json::Value| ResponseTemplate::new(200).set_body_json(v);
        Mock::given(method("GET"))
            .and(path("/3/configuration"))
            .respond_with(reponse(json!({"images": {"secure_base_url": base_images}})))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/3/search/tv"))
            .respond_with(reponse(json!({
                "page": 1, "total_pages": 1,
                "results": [{"id": 1399, "name": "Ma Serie", "original_name": "Ma Serie",
                             "first_air_date": "2020-01-05", "poster_path": "/p.jpg",
                             "overview": "Une série de test."}]
            })))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/3/tv/1399"))
            .respond_with(reponse(json!({
                "id": 1399, "name": "Ma Serie", "original_name": "Ma Serie",
                "original_language": "fr", "overview": "Une série de test.",
                "first_air_date": "2020-01-05", "last_air_date": "2020-02-01",
                "status": "Ended", "episode_run_time": [42],
                "genres": [{"id": 18, "name": "Drame"}],
                "vote_average": 7.5, "vote_count": 10, "poster_path": "/p.jpg",
                "number_of_seasons": 1,
                "seasons": [{"id": 11, "season_number": 1, "name": "Saison 1",
                             "overview": "Première saison.", "episode_count": 3,
                             "air_date": "2020-01-05"}],
                "credits": {"cast": [{"id": 1, "name": "Actrice", "character": "Héroïne"}], "crew": []},
                "external_ids": {"imdb_id": "tt1234567", "tvdb_id": 777},
                "episode_groups": {"results": []},
                "images": {"posters": [image("/p.jpg", Some("fr"))],
                           "backdrops": [image("/f.jpg", None)], "logos": []}
            })))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/3/tv/1399/season/1"))
            .respond_with(reponse(json!({"episodes": [
                {"id": 101, "name": "Pilote", "overview": "Le début.", "air_date": "2020-01-05",
                 "season_number": 1, "episode_number": 1, "still_path": "/e1.jpg"},
                {"id": 102, "name": "Deuxième", "overview": "La suite.", "air_date": "2020-01-12",
                 "season_number": 1, "episode_number": 2, "still_path": "/e2.jpg"},
                {"id": 103, "name": "Troisième", "overview": "La fin.", "air_date": "2020-01-19",
                 "season_number": 1, "episode_number": 3}
            ]})))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path_regex(r"^/3/tv/1399/season/\d+/images$"))
            .respond_with(reponse(json!({"posters": [image("/s1.jpg", Some("fr"))]})))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/3/collection/2344"))
            .respond_with(reponse(json!({
                "id": 2344, "name": "Matrix - La saga",
                "parts": [
                    {"id": 603, "title": "Matrix", "release_date": "1999-03-30"},
                    {"id": 604, "title": "Matrix Reloaded", "release_date": "2003-05-15"},
                    {"id": 605, "title": "Matrix Revolutions", "release_date": "2003-11-05"}
                ]
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
fn series_scan_scrap_nfo_renommage_bout_en_bout() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let serveur = monter_tmdb(&rt);
    let d = tempfile::tempdir().unwrap();
    let series = d.path().join("series");
    let dossier = series.join("Ma Serie (2020)");
    ecrire(&dossier.join("Ma Serie - S01E01.mkv"), b"ep1");
    ecrire(&dossier.join("Ma Serie - S01E02.mkv"), b"ep2");
    let app =
        Application::demarrer(options(d.path(), Some(format!("{}/3/", serveur.uri())))).unwrap();
    app.definir_cle_tmdb(CLE_TEST).unwrap();
    let s = series.clone();
    app.modifier_reglages(move |r| {
        r.series.sources = vec![s];
        r.general.corbeille_activee = false;
    })
    .unwrap();
    let abonne = app.abonner();

    // 1. Scan.
    app.mettre_a_jour_sources_series().attendre().unwrap();
    let serie = app.series().unwrap().remove(0);
    assert_eq!(serie.episodes.len(), 2);

    // 2. Recherche scorée.
    let r = app.rechercher_serie(&serie.id, None).attendre().unwrap();
    assert_eq!(r[0].ids["tmdb"], "1399");

    // 3. Récupération automatique.
    let rapport = app
        .scraper_series(vec![serie.id.clone()], ConfigSerie::default())
        .attendre()
        .unwrap();
    assert_eq!(rapport.reussis, vec![serie.id.clone()], "{rapport:?}");
    let s = app.serie(&serie.id).unwrap().unwrap();
    assert_eq!(s.titre, "Ma Serie");
    assert_eq!(s.resume, "Une série de test.");
    assert_eq!(s.ids.imdb(), "tt1234567");
    assert_eq!(s.acteurs[0].nom, "Actrice");
    let titres: Vec<&str> = s.episodes.iter().map(|e| e.titre.as_str()).collect();
    assert_eq!(titres, vec!["Pilote", "Deuxième"]);
    assert_eq!(s.episodes_fictifs.len(), 3);
    assert_eq!(s.saisons[0].titre, "Saison 1");
    let manquants = app.episodes_manquants(&serie.id).unwrap();
    assert_eq!(manquants.len(), 1);
    assert_eq!(manquants[0].titre, "Troisième");
    // Affichage des manquants selon le réglage.
    assert_eq!(app.episodes_a_afficher(&serie.id).unwrap().len(), 2);
    app.modifier_reglages(|r| r.series.afficher_episodes_manquants = true)
        .unwrap();
    assert_eq!(app.episodes_a_afficher(&serie.id).unwrap().len(), 3);

    // Images et NFO.
    assert_eq!(
        std::fs::read(dossier.join("poster.jpg")).unwrap(),
        b"\xFF\xD8\xFFimage"
    );
    assert!(dossier.join("fanart.jpg").exists());
    assert!(dossier.join("season01-poster.jpg").exists());
    assert!(dossier.join("Ma Serie - S01E01-thumb.jpg").exists());
    let tvshow = std::fs::read_to_string(dossier.join("tvshow.nfo")).unwrap();
    assert!(tvshow.contains("<title>Ma Serie</title>"), "{tvshow}");
    let nfo1 = std::fs::read_to_string(dossier.join("Ma Serie - S01E01.nfo")).unwrap();
    assert!(nfo1.contains("<title>Pilote</title>"), "{nfo1}");
    assert!(
        s.saisons[0]
            .fichiers
            .iter()
            .any(|f| f.type_fichier == TypeFichier::SaisonPoster)
    );
    assert!(rx_contient(
        &abonne,
        |e| matches!(e, Evenement::SerieModifiee { id } if *id == serie.id)
    ));

    // 4. Aperçu puis renommage.
    let apercu = app
        .apercu_renommage_series(std::slice::from_ref(&serie.id))
        .unwrap();
    assert!(apercu.verrouilles.is_empty());
    assert!(
        !apercu.plan.a_des_conflits(),
        "{:?}",
        apercu.plan.conflits()
    );
    assert!(
        dossier.join("Ma Serie - S01E01.mkv").exists(),
        "aperçu sans écriture"
    );
    let r = app
        .renommer_series(vec![serie.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(r.renommes, vec![serie.id.clone()], "{r:?}");
    let saison = dossier.join("Season 1");
    assert!(saison.join("Ma Serie - S01E01 - Pilote.mkv").exists());
    assert!(saison.join("Ma Serie - S01E02 - Deuxième.mkv").exists());
    assert!(saison.join("Ma Serie - S01E01 - Pilote.nfo").exists());
    assert!(saison.join("Ma Serie - S01E01 - Pilote-thumb.jpg").exists());
    assert!(!dossier.join("Ma Serie - S01E01.mkv").exists());

    // 5. Base à jour, nouveau scan sans doublon.
    let s = app.serie(&serie.id).unwrap().unwrap();
    assert!(
        s.episodes
            .iter()
            .flat_map(|e| e.fichiers.iter())
            .all(|f| f.chemin == saison)
    );
    let r = app.mettre_a_jour_sources_series().attendre().unwrap();
    assert!(r.ajoutes.is_empty(), "{r:?}");
    let s = app.serie(&serie.id).unwrap().unwrap();
    assert_eq!(s.episodes.len(), 2);
    assert!(s.episodes.iter().any(|e| e.titre == "Pilote"));

    // Série verrouillée : exclue de l'aperçu et du scraping.
    let mut v = s.clone();
    v.verrouille = true;
    app.enregistrer_serie(v).unwrap();
    let apercu = app
        .apercu_renommage_series(std::slice::from_ref(&serie.id))
        .unwrap();
    assert_eq!(apercu.verrouilles, vec![serie.id.clone()]);
    assert!(apercu.plan.elements.is_empty());
    app.arreter().unwrap();
    drop(serveur);
}

fn rx_contient(rx: &std::sync::mpsc::Receiver<Evenement>, f: impl Fn(&Evenement) -> bool) -> bool {
    rx.try_iter().any(|e| f(&e))
}

/// Deux films dans une source.
fn films(racine: &Path) -> PathBuf {
    let films = racine.join("films");
    ecrire(
        &films.join("The Matrix (1999)/The Matrix (1999).mkv"),
        b"video matrix",
    );
    ecrire(
        &films.join("Inception (2010)/Inception (2010).mkv"),
        b"video inception",
    );
    films
}

fn demarrer_films(racine: &Path, url: Option<String>) -> (Application, PathBuf) {
    let f = films(racine);
    let app = Application::demarrer(options(racine, url)).unwrap();
    let s = f.clone();
    app.modifier_reglages(move |r| {
        r.films.sources = vec![s];
        r.general.corbeille_activee = false;
    })
    .unwrap();
    app.mettre_a_jour_sources_films().attendre().unwrap();
    (app, f)
}

fn film(app: &Application, debut: &str) -> Film {
    app.films()
        .unwrap()
        .into_iter()
        .find(|f| f.titre.starts_with(debut))
        .unwrap()
}

#[test]
fn mise_a_jour_limitee_a_la_selection() {
    let d = tempfile::tempdir().unwrap();
    let (app, films) = demarrer_films(d.path(), None);
    let matrix = film(&app, "The Matrix");
    let inception = film(&app, "Inception");
    ecrire(
        &films.join("The Matrix (1999)/The Matrix (1999).fr.srt"),
        b"1",
    );
    ecrire(
        &films.join("Inception (2010)/Inception (2010).fr.srt"),
        b"1",
    );
    ecrire(&films.join("Autre (2001)/Autre.mkv"), b"x");

    let r = app
        .mettre_a_jour_selection_films(vec![matrix.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(r.modifies, vec![matrix.id.clone()], "{r:?}");
    assert!(r.ajoutes.is_empty(), "les autres dossiers sont ignorés");
    let m = app.film(&matrix.id).unwrap().unwrap();
    assert!(
        m.fichiers
            .iter()
            .any(|f| f.type_fichier == TypeFichier::SousTitres)
    );
    let i = app.film(&inception.id).unwrap().unwrap();
    assert!(
        !i.fichiers
            .iter()
            .any(|f| f.type_fichier == TypeFichier::SousTitres)
    );
    assert_eq!(app.films().unwrap().len(), 2);

    // Dossier disparu : le film choisi est retiré.
    std::fs::remove_dir_all(films.join("The Matrix (1999)")).unwrap();
    let r = app
        .mettre_a_jour_selection_films(vec![matrix.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(r.supprimes, vec![matrix.id.clone()]);
    assert!(app.film(&matrix.id).unwrap().is_none());
    assert!(app.film(&inception.id).unwrap().is_some());
    app.arreter().unwrap();
}

#[test]
fn films_manquants_d_une_saga() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let serveur = monter_tmdb(&rt);
    let d = tempfile::tempdir().unwrap();
    let (app, _) = demarrer_films(d.path(), Some(format!("{}/3/", serveur.uri())));
    app.definir_cle_tmdb(CLE_TEST).unwrap();
    let mut matrix = film(&app, "The Matrix");
    matrix.ids.definir(cles_id::TMDB, "603");
    let matrix = app.enregistrer_film(matrix).unwrap();
    let id_saga = app.avec_stockage(|s| {
        let mut ids = Identifiants::new();
        ids.definir(cles_id::TMDB_SAGA, "2344");
        let mut saga = Saga {
            titre: "Matrix".into(),
            ids,
            films: vec![matrix.id.clone()],
            ..Saga::default()
        };
        s.bibliotheque.enregistrer(&mut saga).unwrap();
        saga.id
    });
    let manquants = app.films_manquants_saga(&id_saga).attendre().unwrap();
    let titres: Vec<&str> = manquants.iter().map(|p| p.titre.as_str()).collect();
    assert_eq!(titres, vec!["Matrix Reloaded", "Matrix Revolutions"]);
    app.arreter().unwrap();
    drop(serveur);
}

#[test]
fn retrait_avec_corbeille_et_evenements() {
    let d = tempfile::tempdir().unwrap();
    let (app, films) = demarrer_films(d.path(), None);
    let abonne = app.abonner();
    let matrix = film(&app, "The Matrix");
    let inception = film(&app, "Inception");

    let r = app
        .retirer_films(vec![matrix.id.clone()], true)
        .attendre()
        .unwrap();
    assert_eq!(r.retires, vec![matrix.id.clone()], "{r:?}");
    let corbeille = films
        .join(DOSSIER_CORBEILLE_SOURCE)
        .join("The Matrix (1999)");
    assert_eq!(r.mis_a_la_corbeille, vec![corbeille.clone()]);
    assert!(corbeille.join("The Matrix (1999).mkv").exists());
    assert!(!films.join("The Matrix (1999)").exists());
    assert!(app.film(&matrix.id).unwrap().is_none());
    assert!(rx_contient(
        &abonne,
        |e| matches!(e, Evenement::FilmSupprime { id } if *id == matrix.id)
    ));

    // Retrait sans les fichiers : la vidéo reste en place.
    let r = app
        .retirer_films(vec![inception.id.clone()], false)
        .attendre()
        .unwrap();
    assert_eq!(r.retires, vec![inception.id.clone()]);
    assert!(films.join("Inception (2010)/Inception (2010).mkv").exists());

    // Le dossier de corbeille est ignoré par le scan.
    let r = app.mettre_a_jour_sources_films().attendre().unwrap();
    assert_eq!(r.ajoutes.len(), 1, "{r:?}");
    assert!(film(&app, "Inception").chemin.ends_with("Inception (2010)"));

    // Film verrouillé : laissé intact.
    let mut i = film(&app, "Inception");
    i.verrouille = true;
    let i = app.enregistrer_film(i).unwrap();
    let r = app
        .retirer_films(vec![i.id.clone()], true)
        .attendre()
        .unwrap();
    assert_eq!(r.verrouilles, vec![i.id.clone()]);
    assert!(app.film(&i.id).unwrap().is_some());
    app.arreter().unwrap();
}

#[test]
fn annulation_du_renommage_et_apercu_detaille() {
    let d = tempfile::tempdir().unwrap();
    let (app, films) = demarrer_films(d.path(), None);
    app.modifier_reglages(|r| {
        r.renommeur.gabarit_dossier_film = "${title} [${year}]".into();
        r.renommeur.gabarit_fichier_film = "${title} [${year}]".into();
    })
    .unwrap();
    let matrix = film(&app, "The Matrix");
    let mut inception = film(&app, "Inception");
    inception.verrouille = true;
    let inception = app.enregistrer_film(inception).unwrap();

    let apercu = app
        .apercu_renommage_films_detaille(&[matrix.id.clone(), inception.id.clone()])
        .unwrap();
    assert_eq!(apercu.verrouilles, vec![inception.id.clone()]);
    assert_eq!(apercu.plan.elements.len(), 1);

    let r = app
        .renommer_films(vec![matrix.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(r.renommes, vec![matrix.id.clone()]);
    let nouveau = films.join("The Matrix [1999]");
    assert!(nouveau.join("The Matrix [1999].mkv").exists());

    let f = app.annuler_renommage_film(&matrix.id).attendre().unwrap();
    let ancien = films.join("The Matrix (1999)");
    assert_eq!(f.chemin, ancien);
    assert!(ancien.join("The Matrix (1999).mkv").exists());
    assert!(!nouveau.exists());
    assert!(f.historique_renommage.is_none());
    assert_eq!(f.fichier_principal().unwrap().nom, "The Matrix (1999).mkv");
    assert_eq!(app.film(&matrix.id).unwrap().unwrap().chemin, ancien);

    // Plus d'historique : nouvelle annulation refusée.
    let e = app
        .annuler_renommage_film(&matrix.id)
        .attendre()
        .unwrap_err();
    assert!(matches!(e, ErreurApp::Introuvable(_)), "{e:?}");
    // Un nouveau scan ne crée pas de doublon.
    let r = app.mettre_a_jour_sources_films().attendre().unwrap();
    assert!(r.ajoutes.is_empty(), "{r:?}");
    app.arreter().unwrap();
}
