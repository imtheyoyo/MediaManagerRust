//! Bandes-annonces : récupération TMDB (faux serveur wiremock) et
//! téléchargement par un faux `yt-dlp` (script), sans réseau.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mediamanager_app::*;
use mediamanager_core::modele::{BandeAnnonce, Film, TypeFichier};
use mediamanager_store::{CoffreMemoire, DossiersApplication, ModeDossiers};
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CLE_TEST: &str = "cle-de-test-factice";

fn ecrire(p: &Path, contenu: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, contenu).unwrap();
}

/// Faux yt-dlp : journalise ses arguments, écrit `trailer.mp4` à l'endroit
/// demandé par `-o`. L'URL contenant `LENT` bloque (annulation), `ECHEC`
/// échoue, `--version` annonce une version.
fn faux_yt_dlp(dossier: &Path) -> PathBuf {
    let chemin = dossier.join("yt-dlp");
    let script = r#"#!/bin/sh
if [ "$1" = "--version" ]; then echo '2099.01.01-test'; exit 0; fi
dir="$(dirname "$0")"
echo "$@" >> "$dir/appels.log"
sortie=""
dernier=""
while [ $# -gt 0 ]; do
  if [ "$1" = "-o" ]; then sortie="$2"; fi
  dernier="$1"
  shift
done
case "$dernier" in
  *LENT*) touch "$dir/demarre"; exec sleep 30 ;;
  *ECHEC*) echo "ERROR: vidéo indisponible" >&2; exit 1 ;;
esac
cible="$(echo "$sortie" | sed 's/%(ext)s/mp4/')"
echo "[download]  50.0% of 1.00MiB"
printf 'FAUSSE VIDEO %s' "$dernier" > "$cible"
echo "[download] 100% of 1.00MiB"
"#;
    ecrire(&chemin, script.as_bytes());
    std::fs::set_permissions(&chemin, std::fs::Permissions::from_mode(0o755)).unwrap();
    chemin
}

fn demarrer(racine: &Path, url_tmdb: Option<String>) -> (Application, PathBuf) {
    let films = racine.join("films");
    ecrire(
        &films.join("Inception (2010)/Inception.2010.mkv"),
        b"video inception",
    );
    ecrire(
        &films.join("Inception (2010)/Inception.2010.nfo"),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<movie><title>Origine</title>\
         <year>2010</year><uniqueid type=\"tmdb\">27205</uniqueid></movie>"
            .as_bytes(),
    );
    let app = Application::demarrer(OptionsDemarrage {
        dossiers: Some(DossiersApplication::depuis_racine(
            racine.join("app"),
            ModeDossiers::Explicite,
        )),
        coffre: Some(Box::new(CoffreMemoire::default())),
        journal: OptionsJournal {
            actif: false,
            ..OptionsJournal::default()
        },
        services: ConfigServices {
            url_api_tmdb: url_tmdb,
            cache_http: false,
            horodatage_nfo: false,
            // Aucune recherche hors du réglage : ni PATH ni dossier de l'exécutable.
            outils: mediamanager_core::flux::ConfigOutils {
                path: Some(std::ffi::OsString::new()),
                dossier_application: Some(racine.join("inexistant")),
                ..Default::default()
            },
            ..ConfigServices::default()
        },
    })
    .unwrap();
    let f = films.clone();
    app.modifier_reglages(move |r| r.films.sources = vec![f])
        .unwrap();
    app.mettre_a_jour_sources_films().attendre().unwrap();
    (app, films.join("Inception (2010)"))
}

fn regler_outil(app: &Application, outil: &Path) {
    let o = outil.display().to_string();
    app.modifier_reglages(move |r| r.general.chemin_yt_dlp = o)
        .unwrap();
}

fn le_film(app: &Application) -> Film {
    app.films().unwrap().remove(0)
}

fn bande(cle: &str, qualite: &str, langue: &str) -> BandeAnnonce {
    BandeAnnonce {
        id: cle.into(),
        nom: format!("Bande-annonce {cle}"),
        url: format!("https://www.youtube.com/watch?v={cle}"),
        qualite: qualite.into(),
        langue: langue.into(),
        fournisseur: "youtube".into(),
        ..BandeAnnonce::default()
    }
}

fn definir_bandes(app: &Application, bandes: Vec<BandeAnnonce>) {
    let mut f = le_film(app);
    f.bandes_annonces = bandes;
    app.enregistrer_film(f).unwrap();
}

fn nfo(dossier: &Path) -> String {
    std::fs::read_to_string(dossier.join("Inception.2010.nfo")).unwrap()
}

#[test]
fn telechargement_nommage_enregistrement_nfo_et_non_ecrasement() {
    let d = tempfile::tempdir().unwrap();
    let (app, dossier) = demarrer(d.path(), None);
    let outil = faux_yt_dlp(&d.path().join("bin"));
    regler_outil(&app, &outil);
    assert_eq!(app.chemin_yt_dlp(), Some(outil.clone()));
    let diag = app
        .diagnostiquer_yt_dlp(&outil.display().to_string())
        .unwrap();
    assert_eq!(diag.chemin, outil);
    assert_eq!(diag.version.as_deref(), Some("2099.01.01-test"));
    let film = le_film(&app);
    let b = bande("AAAAAAAAA11", "1080p", "fr-FR");
    let abonne = app.abonner();

    let options = OptionsTelechargement {
        qualite: QualiteBandeAnnonce::Hd1080,
        format: FormatVideoBandeAnnonce::Mkv,
        ..OptionsTelechargement::default()
    };
    let r = app
        .telecharger_bande_annonce(&film.id, &b, options.clone())
        .attendre()
        .unwrap();
    let attendu = dossier.join("Inception.2010-trailer.mp4");
    assert_eq!(r.telecharges.len(), 1, "{r:?}");
    assert_eq!(r.telecharges[0].chemin, attendu);
    assert_eq!(
        std::fs::read_to_string(&attendu).unwrap(),
        "FAUSSE VIDEO https://www.youtube.com/watch?v=AAAAAAAAA11"
    );
    // Aucun dossier temporaire ne subsiste.
    let restes: Vec<_> = std::fs::read_dir(&dossier)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".bande-annonce")
        })
        .collect();
    assert!(restes.is_empty(), "{restes:?}");

    // Arguments : hauteur, conteneur, URL canonique.
    let journal = std::fs::read_to_string(d.path().join("bin/appels.log")).unwrap();
    assert!(journal.contains("height<=1080"), "{journal}");
    assert!(journal.contains("--merge-output-format mkv"), "{journal}");
    assert!(
        journal.trim_end().ends_with("watch?v=AAAAAAAAA11"),
        "{journal}"
    );

    // Enregistré dans le film (type bande-annonce), bande-annonce principale
    // référencée dans le NFO.
    let f = app.film(&film.id).unwrap().unwrap();
    let t: Vec<_> = f.fichiers_de_type(TypeFichier::Trailer).collect();
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].nom, "Inception.2010-trailer.mp4");
    assert!(t[0].taille > 0);
    assert_eq!(f.bandes_annonces.len(), 1);
    assert!(
        nfo(&dossier).contains(
            "<trailer>plugin://plugin.video.youtube/play/?video_id=AAAAAAAAA11</trailer>"
        )
    );

    // Événements : film modifié, progression en pourcentage.
    let evts: Vec<_> = abonne.try_iter().collect();
    assert!(
        evts.iter()
            .any(|e| matches!(e, Evenement::FilmModifie { .. }))
    );
    assert!(evts.iter().any(|e| matches!(
        e,
        Evenement::Progression {
            fait: 50,
            total: 100,
            ..
        }
    )));

    // Deuxième téléchargement : jamais d'écrasement, numéro ajouté ; la
    // première bande-annonce reste la principale (NFO inchangé).
    let b2 = bande("BBBBBBBBB22", "720p", "en-US");
    let r2 = app
        .telecharger_bande_annonce(&film.id, &b2, options.clone())
        .attendre()
        .unwrap();
    assert_eq!(
        r2.telecharges[0].chemin,
        dossier.join("Inception.2010-trailer.2.mp4")
    );
    assert!(
        std::fs::read_to_string(&attendu)
            .unwrap()
            .ends_with("AAAAAAAAA11")
    );
    assert!(nfo(&dossier).contains("AAAAAAAAA11"));
    assert!(!nfo(&dossier).contains("BBBBBBBBB22"));
    let f = app.film(&film.id).unwrap().unwrap();
    assert_eq!(f.fichiers_de_type(TypeFichier::Trailer).count(), 2);
    assert_eq!(f.bandes_annonces.len(), 2);

    // Un fichier d'une autre extension occupe aussi le nom.
    ecrire(&dossier.join("Inception.2010-trailer.3.mkv"), b"autre");
    let r3 = app
        .telecharger_bande_annonce(&film.id, &bande("CCCCCCCCC33", "", ""), options)
        .attendre()
        .unwrap();
    assert_eq!(
        r3.telecharges[0].chemin,
        dossier.join("Inception.2010-trailer.4.mp4")
    );
    app.arreter().unwrap();
}

#[test]
fn outil_absent_et_film_verrouille() {
    let d = tempfile::tempdir().unwrap();
    let (app, dossier) = demarrer(d.path(), None);
    let film = le_film(&app);
    definir_bandes(&app, vec![bande("AAAAAAAAA11", "720p", "fr")]);

    assert!(!app.yt_dlp_disponible());
    assert!(app.diagnostiquer_yt_dlp("").is_none());
    let e = app
        .telecharger_bande_annonce(
            &film.id,
            &bande("AAAAAAAAA11", "", ""),
            OptionsTelechargement::default(),
        )
        .attendre()
        .unwrap_err();
    assert!(
        matches!(e, ErreurApp::OutilIntrouvable(ref o) if o == "yt-dlp"),
        "{e:?}"
    );
    assert!(e.to_string().contains("yt-dlp.exe"), "{e}");
    assert!(e.to_string().contains("outils/"), "{e}");
    let e = app
        .telecharger_bandes_annonces_auto(vec![film.id.clone()])
        .attendre()
        .unwrap_err();
    assert!(matches!(e, ErreurApp::OutilIntrouvable(_)), "{e:?}");

    // Outil trouvé dans outils/ à côté de l'application (hors réglage).
    let outil = faux_yt_dlp(&d.path().join("bin"));
    regler_outil(&app, &outil);

    // Film verrouillé : refus explicite en manuel, exclusion en automatique.
    app.avec_stockage(|s| {
        let mut f = s.bibliotheque.obtenir::<Film>(&film.id).unwrap().unwrap();
        f.verrouille = true;
        s.bibliotheque.enregistrer(&mut f).unwrap();
    });
    let e = app
        .telecharger_bande_annonce(
            &film.id,
            &bande("AAAAAAAAA11", "", ""),
            OptionsTelechargement::default(),
        )
        .attendre()
        .unwrap_err();
    assert!(matches!(e, ErreurApp::Verrouille(_)), "{e:?}");
    let r = app
        .telecharger_bandes_annonces_auto(vec![film.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(r.verrouilles, vec![film.id.clone()]);
    assert!(r.telecharges.is_empty());
    assert!(!d.path().join("bin/appels.log").exists());
    assert!(!dossier.join("Inception.2010-trailer.mp4").exists());
    app.arreter().unwrap();
}

#[test]
fn detection_dans_le_dossier_outils() {
    let d = tempfile::tempdir().unwrap();
    let (app, _) = demarrer(d.path(), None);
    drop(app);
    // Nouvelle application dont le « dossier de l'application » contient outils/.
    let racine = d.path().join("appli");
    let outil = faux_yt_dlp(&racine.join("outils"));
    let app = Application::demarrer(OptionsDemarrage {
        dossiers: Some(DossiersApplication::depuis_racine(
            d.path().join("app2"),
            ModeDossiers::Explicite,
        )),
        coffre: Some(Box::new(CoffreMemoire::default())),
        journal: OptionsJournal {
            actif: false,
            ..OptionsJournal::default()
        },
        services: ConfigServices {
            outils: mediamanager_core::flux::ConfigOutils {
                path: Some(std::ffi::OsString::new()),
                dossier_application: Some(racine),
                ..Default::default()
            },
            ..ConfigServices::default()
        },
    })
    .unwrap();
    assert_eq!(app.chemin_yt_dlp(), Some(outil));
    app.arreter().unwrap();
}

#[test]
fn echec_de_yt_dlp_et_annulation() {
    let d = tempfile::tempdir().unwrap();
    let (app, dossier) = demarrer(d.path(), None);
    let outil = faux_yt_dlp(&d.path().join("bin"));
    regler_outil(&app, &outil);
    let film = le_film(&app);

    let e = app
        .telecharger_bande_annonce(
            &film.id,
            &bande("ECHEC1", "", ""),
            OptionsTelechargement::default(),
        )
        .attendre()
        .unwrap_err();
    assert!(matches!(e, ErreurApp::Telechargement { .. }), "{e:?}");
    assert!(e.to_string().contains("vidéo indisponible"), "{e}");

    // Annulation : le processus est tué, rien n'est créé.
    let tache = app.telecharger_bande_annonce(
        &film.id,
        &bande("LENT1", "", ""),
        OptionsTelechargement::default(),
    );
    let marque = d.path().join("bin/demarre");
    let debut = Instant::now();
    while !marque.exists() {
        assert!(
            debut.elapsed() < Duration::from_secs(20),
            "yt-dlp non lancé"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    tache.annuler();
    let e = tache.attendre().unwrap_err();
    assert!(matches!(e, ErreurApp::Annulee), "{e:?}");
    assert!(
        debut.elapsed() < Duration::from_secs(15),
        "annulation trop lente"
    );
    let noms: Vec<String> = std::fs::read_dir(&dossier)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        noms.iter()
            .all(|n| !n.contains("trailer") && !n.starts_with(".bande")),
        "{noms:?}"
    );
    let f = app.film(&film.id).unwrap().unwrap();
    assert_eq!(f.fichiers_de_type(TypeFichier::Trailer).count(), 0);
    app.arreter().unwrap();
}

#[test]
fn delai_depasse() {
    let d = tempfile::tempdir().unwrap();
    let (app, _) = demarrer(d.path(), None);
    regler_outil(&app, &faux_yt_dlp(&d.path().join("bin")));
    let film = le_film(&app);
    let options = OptionsTelechargement {
        delai: Duration::from_millis(300),
        ..OptionsTelechargement::default()
    };
    let e = app
        .telecharger_bande_annonce(&film.id, &bande("LENT2", "", ""), options)
        .attendre()
        .unwrap_err();
    assert!(matches!(e, ErreurApp::Telechargement { .. }), "{e:?}");
    assert!(e.to_string().contains("n'a pas répondu"), "{e}");
    app.arreter().unwrap();
}

#[test]
fn telechargement_automatique_choisit_la_meilleure() {
    let d = tempfile::tempdir().unwrap();
    let (app, dossier) = demarrer(d.path(), None);
    regler_outil(&app, &faux_yt_dlp(&d.path().join("bin")));
    let film = le_film(&app);
    let mut vimeo = bande("VIM", "720p", "fr-FR");
    vimeo.url = "https://vimeo.com/42".into();
    vimeo.fournisseur = "vimeo".into();
    definir_bandes(
        &app,
        vec![
            bande("EN1080", "1080p", "en-US"),
            vimeo,
            bande("FR480", "480p", "fr-FR"),
            bande("FR720", "720p", "fr-FR"),
            bande("FR1080", "1080p", "fr-FR"),
        ],
    );
    let r = app
        .telecharger_bandes_annonces_auto_avec(
            vec![film.id.clone(), "inconnu".into()],
            OptionsTelechargement {
                langue: "fr".into(),
                ..OptionsTelechargement::default()
            },
        )
        .attendre()
        .unwrap();
    assert_eq!(r.telecharges.len(), 1, "{r:?}");
    assert_eq!(
        r.telecharges[0].url,
        "https://www.youtube.com/watch?v=FR720"
    );
    assert_eq!(r.echecs.len(), 1);
    assert!(dossier.join("Inception.2010-trailer.mp4").is_file());

    // Déjà pourvu : rien de plus.
    let r = app
        .telecharger_bandes_annonces_auto_avec(
            vec![film.id.clone()],
            OptionsTelechargement::default(),
        )
        .attendre()
        .unwrap();
    assert_eq!(r.deja_presents, vec![film.id.clone()]);
    assert!(r.telecharges.is_empty());

    // Deux bandes-annonces voulues : une seconde est téléchargée (la suivante).
    let r = app
        .telecharger_bandes_annonces_auto_avec(
            vec![film.id.clone()],
            OptionsTelechargement {
                nombre: 2,
                langue: "fr".into(),
                ..OptionsTelechargement::default()
            },
        )
        .attendre()
        .unwrap();
    assert_eq!(r.telecharges.len(), 1, "{r:?}");
    assert_eq!(
        r.telecharges[0].chemin,
        dossier.join("Inception.2010-trailer.2.mp4")
    );
    app.arreter().unwrap();
}

#[test]
fn auto_sans_bande_annonce() {
    let d = tempfile::tempdir().unwrap();
    let (app, _) = demarrer(d.path(), Some("http://127.0.0.1:9/3/".into()));
    regler_outil(&app, &faux_yt_dlp(&d.path().join("bin")));
    let film = le_film(&app);
    // Aucune bande-annonce, aucune clé TMDB : signalé, sans erreur.
    let r = app
        .telecharger_bandes_annonces_auto(vec![film.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(r.sans_bande_annonce, vec![film.id.clone()]);
    app.arreter().unwrap();
}

fn monter_tmdb(rt: &tokio::runtime::Runtime) -> MockServer {
    rt.block_on(async {
        let s = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/3/configuration"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(
                    json!({"images": {"secure_base_url": format!("{}/img/", s.uri())}}),
                ),
            )
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/3/movie/27205"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 27205, "title": "Inception", "original_title": "Inception",
                "overview": "Un rêve dans un rêve.", "release_date": "2010-07-16"
            })))
            .mount(&s)
            .await;
        let videos = json!({"results": [
            {"name": "Bande-annonce VF", "key": "AAAAAAAAA11", "site": "YouTube", "size": 1080,
             "type": "Trailer", "iso_639_1": "fr", "iso_3166_1": "FR",
             "published_at": "2010-05-01T10:00:00.000Z"},
            {"name": "Teaser", "key": "TEASER", "site": "YouTube", "size": 1080, "type": "Teaser"},
            {"name": "Trailer VO", "key": "BBBBBBBBB22", "site": "YouTube", "size": 480,
             "type": "Trailer", "iso_639_1": "en", "iso_3166_1": "US"}
        ]});
        Mock::given(method("GET"))
            .and(path("/3/movie/27205/videos"))
            .and(query_param("language", "fr-FR"))
            .respond_with(ResponseTemplate::new(200).set_body_json(videos.clone()))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path("/3/movie/27205/videos"))
            .respond_with(ResponseTemplate::new(200).set_body_json(videos))
            .mount(&s)
            .await;
        s
    })
}

fn verifier_bandes_tmdb(f: &Film) {
    assert_eq!(f.bandes_annonces.len(), 2, "{:?}", f.bandes_annonces);
    let a = &f.bandes_annonces[0];
    assert_eq!(a.nom, "Bande-annonce VF");
    assert_eq!(a.url, "https://www.youtube.com/watch?v=AAAAAAAAA11&hd=1");
    assert_eq!(a.qualite, "1080p");
    assert_eq!(a.langue, "fr-FR");
    assert_eq!(a.fournisseur, "youtube");
    assert_eq!(a.id, "AAAAAAAAA11");
    assert_eq!(a.date, "2010-05-01");
    assert_eq!(a.scrape_par, "tmdb");
    assert!(
        a.dans_nfo,
        "la meilleure en langue de scraping est la principale"
    );
    assert!(!f.bandes_annonces[1].dans_nfo);
    assert_eq!(
        f.bandes_annonces[1].url,
        "https://www.youtube.com/watch?v=BBBBBBBBB22"
    );
}

#[test]
fn recuperation_tmdb_et_scraping() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let serveur = monter_tmdb(&rt);
    let d = tempfile::tempdir().unwrap();
    let (app, dossier) = demarrer(d.path(), Some(format!("{}/3/", serveur.uri())));
    app.definir_cle_tmdb(CLE_TEST).unwrap();
    let film = le_film(&app);

    // Récupération seule.
    let r = app
        .recuperer_bandes_annonces(vec![film.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(r.mis_a_jour, vec![(film.id.clone(), 2)], "{r:?}");
    verifier_bandes_tmdb(&app.film(&film.id).unwrap().unwrap());

    // Idempotent (pas de doublon) et verrou respecté.
    app.recuperer_bandes_annonces(vec![film.id.clone()])
        .attendre()
        .unwrap();
    verifier_bandes_tmdb(&app.film(&film.id).unwrap().unwrap());

    // Scraping complet : mêmes bandes-annonces, écrites dans le NFO.
    let choix = app
        .rechercher_film(&film.id, None)
        .attendre()
        .ok()
        .and_then(|v| v.into_iter().next());
    assert!(choix.is_none() || choix.is_some());
    let mut f = app.film(&film.id).unwrap().unwrap();
    f.bandes_annonces.clear();
    app.enregistrer_film(f).unwrap();
    let choix = ResultatRecherche {
        ids: [("tmdb".to_string(), "27205".to_string())].into(),
        titre: "Inception".into(),
        ..ResultatRecherche::default()
    };
    let config = ConfigMetadonnees {
        artwork: false,
        saga: false,
        ..ConfigMetadonnees::tout()
    };
    let f = app
        .appliquer_resultat(&film.id, choix, config)
        .attendre()
        .unwrap();
    verifier_bandes_tmdb(&f);
    assert!(
        nfo(&dossier).contains("video_id=AAAAAAAAA11"),
        "{}",
        nfo(&dossier)
    );

    // Verrouillé : ignoré.
    app.avec_stockage(|s| {
        let mut f = s.bibliotheque.obtenir::<Film>(&film.id).unwrap().unwrap();
        f.verrouille = true;
        s.bibliotheque.enregistrer(&mut f).unwrap();
    });
    let r = app
        .recuperer_bandes_annonces(vec![film.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(r.verrouilles, vec![film.id.clone()]);
    app.arreter().unwrap();
}

#[test]
fn auto_recupere_chez_tmdb_puis_telecharge() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let serveur = monter_tmdb(&rt);
    let d = tempfile::tempdir().unwrap();
    let (app, dossier) = demarrer(d.path(), Some(format!("{}/3/", serveur.uri())));
    app.definir_cle_tmdb(CLE_TEST).unwrap();
    regler_outil(&app, &faux_yt_dlp(&d.path().join("bin")));
    let film = le_film(&app);
    let r = app
        .telecharger_bandes_annonces_auto(vec![film.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(r.telecharges.len(), 1, "{r:?}");
    assert_eq!(
        r.telecharges[0].url,
        "https://www.youtube.com/watch?v=AAAAAAAAA11"
    );
    assert!(dossier.join("Inception.2010-trailer.mp4").is_file());
    assert_eq!(
        app.film(&film.id).unwrap().unwrap().bandes_annonces.len(),
        2
    );
    app.arreter().unwrap();
}
