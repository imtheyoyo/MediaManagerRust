//! Analyse des fichiers médias par un faux `ffprobe` (script), sans réseau :
//! films et épisodes, enregistrement en base, diagnostic des outils, absence
//! d'outil.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use mediamanager_app::*;
use mediamanager_core::flux::ConfigOutils;
use mediamanager_core::modele::TypeFichier;
use mediamanager_store::{CoffreMemoire, DossiersApplication, ModeDossiers};

const SORTIE_FFPROBE: &str = r#"streams.stream.0.codec_name="h264"
streams.stream.0.codec_type="video"
streams.stream.0.width=1920
streams.stream.0.height=1080
streams.stream.0.avg_frame_rate="24/1"
streams.stream.1.codec_name="ac3"
streams.stream.1.codec_type="audio"
streams.stream.1.channels=6
streams.stream.1.channel_layout="5.1"
streams.stream.1.tags.language="fra"
format.format_name="matroska,webm"
format.duration="2640.000000"
format.bit_rate="4000000"
"#;

fn ecrire(p: &Path, contenu: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, contenu).unwrap();
}

/// Faux ffprobe : `-version` annonce une version, sinon la sortie fixe.
fn faux_ffprobe(dossier: &Path) -> PathBuf {
    let chemin = dossier.join("ffprobe");
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then echo 'ffprobe version 9.9-test'; exit 0; fi\ncat <<'FIN'\n{SORTIE_FFPROBE}FIN\n"
    );
    ecrire(&chemin, script.as_bytes());
    std::fs::set_permissions(&chemin, std::fs::Permissions::from_mode(0o755)).unwrap();
    chemin
}

fn demarrer(racine: &Path) -> Application {
    Application::demarrer(OptionsDemarrage {
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
            cache_http: false,
            horodatage_nfo: false,
            // Aucune recherche hors du réglage : ni PATH ni dossier de l'exécutable.
            outils: ConfigOutils {
                path: Some(std::ffi::OsString::new()),
                dossier_application: Some(racine.join("inexistant")),
                ..ConfigOutils::default()
            },
            ..ConfigServices::default()
        },
    })
    .unwrap()
}

#[test]
fn analyse_des_films_et_des_episodes() {
    let d = tempfile::tempdir().unwrap();
    let films = d.path().join("films");
    let series = d.path().join("series");
    ecrire(&films.join("Alien (1979)/Alien.1979.mkv"), b"film");
    ecrire(&series.join("Ma Serie (2020)/Ma.Serie.S01E01.mkv"), b"ep1");
    ecrire(&series.join("Ma Serie (2020)/Ma.Serie.S01E02.mkv"), b"ep2");
    let outil = faux_ffprobe(&d.path().join("bin"));
    let app = demarrer(d.path());
    let (f, s) = (films.clone(), series.clone());
    app.modifier_reglages(move |r| {
        r.films.sources = vec![f];
        r.series.sources = vec![s];
    })
    .unwrap();
    app.mettre_a_jour_sources_films().attendre().unwrap();
    app.mettre_a_jour_sources_series().attendre().unwrap();
    let film = app.films().unwrap().remove(0);
    let serie = app.series().unwrap().remove(0);

    // Sans outil : échec explicite, rien n'est modifié.
    assert!(!app.outils_analyse_disponibles());
    let e = app
        .analyser_medias(vec![film.id.clone()], vec![])
        .attendre()
        .unwrap_err();
    assert!(matches!(e, ErreurApp::OutilsAnalyseAbsents), "{e}");
    assert!(e.to_string().contains("outils/"), "{e}");
    assert!(!app.diagnostiquer_outils("").disponible());

    // Réglage du chemin : diagnostic avec version, puis analyse.
    let diag = app.diagnostiquer_outils(&outil.display().to_string());
    assert_eq!(diag.ffprobe.as_ref().unwrap().chemin, outil);
    assert_eq!(
        diag.ffprobe.unwrap().version.as_deref(),
        Some("ffprobe version 9.9-test")
    );
    let o = outil.clone();
    app.modifier_reglages(move |r| r.general.chemin_ffmpeg = o.display().to_string())
        .unwrap();
    assert!(app.outils_analyse_disponibles());
    let rapport = app
        .analyser_medias(vec![film.id.clone()], vec![serie.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(rapport.analyses, 3, "{rapport:?}");
    assert_eq!(rapport.films, vec![film.id.clone()]);
    assert_eq!(rapport.series, vec![serie.id.clone()]);
    assert!(rapport.echecs.is_empty(), "{rapport:?}");

    let f = app.film(&film.id).unwrap().unwrap();
    let v = f
        .fichiers
        .iter()
        .find(|x| x.type_fichier == TypeFichier::Video)
        .unwrap();
    assert_eq!(v.video.hauteur, 1080);
    assert_eq!(v.flux_audio[0].codec, "AC3");
    assert_eq!(v.flux_audio[0].canaux, 6);
    assert_eq!(v.duree_secondes, 2640);
    let s = app.serie(&serie.id).unwrap().unwrap();
    assert_eq!(s.episodes.len(), 2);
    for e in &s.episodes {
        let v = e
            .fichiers
            .iter()
            .find(|x| x.type_fichier == TypeFichier::Video)
            .unwrap();
        assert_eq!(v.video.largeur, 1920);
        assert_eq!(v.flux_audio.len(), 1);
    }

    // Un film verrouillé n'est pas touché.
    let mut verrouille = app.film(&film.id).unwrap().unwrap();
    verrouille.verrouille = true;
    app.enregistrer_film(verrouille).unwrap();
    let rapport = app
        .analyser_medias(vec![film.id.clone()], vec![])
        .attendre()
        .unwrap();
    assert_eq!(rapport.verrouilles, vec![film.id.clone()]);
    assert_eq!(rapport.analyses, 0);
    app.arreter().unwrap();
}
