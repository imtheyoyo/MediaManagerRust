//! Tests du scan des sources de séries sur des arborescences temporaires.

use std::fs;
use std::path::Path;

use crate::modele::{Edition, Episode, GroupeEpisodes, Serie, SourceMedia, TypeFichier};
use crate::scan::{ErreurScan, OptionsScan};
use crate::series::scan::*;

fn creer(racine: &Path, fichiers: &[&str]) {
    for f in fichiers {
        let p = racine.join(f);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, b"x").unwrap();
    }
}

fn scanner(racine: &Path) -> ResultatScanSeries {
    scanner_source_series(racine, &OptionsScan::default()).expect("scan")
}

fn serie<'a>(r: &'a ResultatScanSeries, titre: &str) -> &'a Serie {
    r.series
        .iter()
        .find(|s| s.titre == titre)
        .unwrap_or_else(|| panic!("série {titre} absente : {:?}", titres(r)))
}

fn titres(r: &ResultatScanSeries) -> Vec<String> {
    r.series.iter().map(|s| s.titre.clone()).collect()
}

fn se(e: &Episode) -> (i32, i32) {
    let g = GroupeEpisodes::diffusion();
    (e.saison(&g), e.episode(&g))
}

fn episode(s: &Serie, saison: i32, ep: i32) -> &Episode {
    s.episodes
        .iter()
        .find(|e| se(e) == (saison, ep))
        .unwrap_or_else(|| {
            panic!(
                "S{saison}E{ep} absent : {:?}",
                s.episodes.iter().map(se).collect::<Vec<_>>()
            )
        })
}

fn noms_types(e: &Episode) -> Vec<(String, TypeFichier)> {
    e.fichiers
        .iter()
        .map(|f| (f.nom.clone(), f.type_fichier))
        .collect()
}

#[test]
fn structure_complete() {
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path();
    creer(
        r,
        &[
            "Show (2019)/tvshow.nfo",
            "Show (2019)/poster.jpg",
            "Show (2019)/fanart.jpg",
            "Show (2019)/banner.jpg",
            "Show (2019)/season01-poster.jpg",
            "Show (2019)/season-specials-poster.jpg",
            "Show (2019)/season-all-poster.jpg",
            "Show (2019)/Season 1/Show - S01E01 - Pilot.mkv",
            "Show (2019)/Season 1/Show - S01E01 - Pilot.nfo",
            "Show (2019)/Season 1/Show - S01E01 - Pilot-thumb.jpg",
            "Show (2019)/Season 1/Show - S01E01 - Pilot.fr.srt",
            "Show (2019)/Season 1/Show - S01E01 - Pilot.en.forced.srt",
            "Show (2019)/Season 1/Show - S01E02E03 - Double.mkv",
            "Show (2019)/Season 1/Show - S01E02E03 - Double.jpg",
            "Show (2019)/Season 1/folder.jpg",
            "Show (2019)/Season 2/Show - 2x01.mkv",
            "Show (2019)/Season 2/Show.S02E02.CD1.avi",
            "Show (2019)/Season 2/Show.S02E02.CD2.avi",
            "Show (2019)/Season 2/S02E01.de.srt",
            "Show (2019)/Specials/Show S00E01.mkv",
            "Show (2019)/Extras/Making of.mkv",
            "Show (2019)/Show - S03E01.Directors.Cut.BluRay.mkv",
            "Show (2019)/S03E02/VIDEO_TS/VIDEO_TS.IFO",
            "Show (2019)/S03E02/VIDEO_TS/VTS_01_1.VOB",
            "Show (2019)/S03E02/S03E02.nfo",
            "Show (2019)/.cache/Show S09E09.mkv",
            "Show (2019)/Ignore/.mmignore",
            "Show (2019)/Ignore/Show S08E08.mkv",
            "Show (2019)/@eaDir/Show S07E07.mkv",
        ],
    );
    let res = scanner(r);
    assert_eq!(res.series.len(), 1, "{:?}", titres(&res));
    let s = serie(&res, "Show");
    assert_eq!(s.annee, 2019);
    assert_eq!(s.chemin, r.join("Show (2019)"));
    assert_eq!(s.source_donnees, r);

    let numeros: Vec<(i32, i32)> = s.episodes.iter().map(se).collect();
    assert_eq!(
        numeros,
        vec![
            (0, 1),
            (1, 1),
            (1, 2),
            (1, 3),
            (2, 1),
            (2, 2),
            (3, 1),
            (3, 2)
        ]
    );

    // Épisode avec ses annexes.
    let e = episode(s, 1, 1);
    assert_eq!(e.titre, "Pilot");
    assert!(!e.multi_episodes);
    let types = noms_types(e);
    assert!(types.contains(&("Show - S01E01 - Pilot.mkv".into(), TypeFichier::Video)));
    assert!(types.contains(&("Show - S01E01 - Pilot.nfo".into(), TypeFichier::Nfo)));
    assert!(types.contains(&("Show - S01E01 - Pilot-thumb.jpg".into(), TypeFichier::Thumb)));
    assert!(types.contains(&(
        "Show - S01E01 - Pilot.fr.srt".into(),
        TypeFichier::SousTitres
    )));
    assert!(types.contains(&(
        "Show - S01E01 - Pilot.en.forced.srt".into(),
        TypeFichier::SousTitres
    )));
    assert_eq!(e.fichiers.len(), 5);
    assert_eq!(e.nom_fichier_origine, "Show - S01E01 - Pilot.mkv");

    // Multi-épisodes : deux épisodes partageant les mêmes fichiers ; l'image
    // de même nom devient la vignette.
    let (e2, e3) = (episode(s, 1, 2), episode(s, 1, 3));
    assert!(e2.multi_episodes && e3.multi_episodes);
    assert_eq!(e2.fichiers, e3.fichiers);
    assert!(noms_types(e2).contains(&("Show - S01E02E03 - Double.jpg".into(), TypeFichier::Thumb)));
    assert_eq!(e2.titre, "Double");

    // Sous-titres nommés autrement, rattachés par détection.
    assert!(
        noms_types(episode(s, 2, 1)).contains(&("S02E01.de.srt".into(), TypeFichier::SousTitres))
    );

    // Empilement.
    let e = episode(s, 2, 2);
    assert!(e.empile);
    let videos: Vec<(String, u32)> = e
        .fichiers
        .iter()
        .filter(|f| f.type_fichier == TypeFichier::Video)
        .map(|f| (f.nom.clone(), f.empilement))
        .collect();
    assert_eq!(
        videos,
        vec![
            ("Show.S02E02.CD1.avi".into(), 1),
            ("Show.S02E02.CD2.avi".into(), 2)
        ]
    );

    // Épisode à plat : édition et source.
    let e = episode(s, 3, 1);
    assert_eq!(e.edition, Edition::DirectorsCut);
    assert_eq!(e.source_media, SourceMedia::Bluray);

    // Disque.
    let e = episode(s, 3, 2);
    assert!(e.est_disque);
    let types = noms_types(e);
    assert!(types.contains(&("VIDEO_TS.IFO".into(), TypeFichier::Video)));
    assert!(types.contains(&("S03E02.nfo".into(), TypeFichier::Nfo)));
    assert_eq!(e.source_media, SourceMedia::Dvd);

    // Spécial.
    assert_eq!(episode(s, 0, 1).nom_fichier_origine, "Show S00E01.mkv");

    // Saisons et artworks de saison.
    let saisons: Vec<i32> = s.saisons.iter().map(|x| x.numero).collect();
    assert_eq!(saisons, vec![0, 1, 2, 3]);
    let art = |n: i32| -> Vec<(String, TypeFichier)> {
        s.saisons
            .iter()
            .find(|x| x.numero == n)
            .unwrap()
            .fichiers
            .iter()
            .map(|f| (f.nom.clone(), f.type_fichier))
            .collect()
    };
    assert_eq!(
        art(0),
        vec![(
            "season-specials-poster.jpg".into(),
            TypeFichier::SaisonPoster
        )]
    );
    let a1 = art(1);
    assert!(a1.contains(&("season01-poster.jpg".into(), TypeFichier::SaisonPoster)));
    assert!(a1.contains(&("folder.jpg".into(), TypeFichier::SaisonPoster)));
    assert!(art(2).is_empty());

    // Fichiers de la série.
    let fs_serie: Vec<(String, TypeFichier)> = s
        .fichiers
        .iter()
        .map(|f| (f.nom.clone(), f.type_fichier))
        .collect();
    for attendu in [
        ("tvshow.nfo", TypeFichier::Nfo),
        ("poster.jpg", TypeFichier::Poster),
        ("fanart.jpg", TypeFichier::Fanart),
        ("banner.jpg", TypeFichier::Banner),
        ("season-all-poster.jpg", TypeFichier::SaisonPoster),
        ("Making of.mkv", TypeFichier::Extra),
    ] {
        assert!(
            fs_serie.contains(&(attendu.0.to_string(), attendu.1)),
            "{attendu:?} absent de {fs_serie:?}"
        );
    }

    // Rien des dossiers cachés / marqués / système.
    for e in &s.episodes {
        assert!(!matches!(se(e).0, 7..=9), "{:?}", e.nom_fichier_origine);
    }
    assert!(res.avertissements.is_empty(), "{:?}", res.avertissements);
}

#[test]
fn regroupements_racine_et_series_vides() {
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path();
    creer(
        r,
        &[
            "A/Another Show/101.mkv",
            "A/Autre Série/Season 1/02.mkv",
            "B/Better Show 2010/Better.Show.2010.S01E01.mkv",
            "Vide/readme.txt",
            "Seulement des images/poster.jpg",
            "episode.S01E01.mkv",
            "notes.txt",
            "MAKEMKV/Show/Show S01E01.mkv",
            "$RECYCLE.BIN/Show/Show S01E01.mkv",
        ],
    );
    let res = scanner(r);
    assert_eq!(
        titres(&res),
        vec!["Another Show", "Autre Série", "Better Show"],
        "les séries sont triées par chemin"
    );
    assert_eq!(serie(&res, "Better Show").annee, 2010);
    assert_eq!(se(&serie(&res, "Another Show").episodes[0]), (1, 1));
    assert_eq!(se(&serie(&res, "Autre Série").episodes[0]), (1, 2));
    assert_eq!(
        se(&serie(&res, "Better Show").episodes[0]),
        (1, 1),
        "année du nom de série ignorée"
    );
    assert_eq!(
        res.avertissements,
        vec![AvertissementSeries::EpisodeALaRacine(
            r.join("episode.S01E01.mkv")
        )]
    );
}

#[test]
fn episodes_non_detectes_et_dates() {
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path();
    creer(
        r,
        &[
            "Talk Show/Talk Show 2021-03-14 Invité.mkv",
            "Talk Show/Talk Show 2021-03-15 Autre.mkv",
            "Talk Show/Bonus HDTV.mkv",
        ],
    );
    let res = scanner(r);
    let s = serie(&res, "Talk Show");
    assert_eq!(s.episodes.len(), 3);
    let dates: Vec<Option<String>> = s
        .episodes
        .iter()
        .map(|e| e.premiere_diffusion.clone())
        .collect();
    assert!(dates.contains(&Some("2021-03-14".into())));
    assert!(dates.contains(&Some("2021-03-15".into())));
    let non_cat: Vec<&Episode> = s.episodes.iter().filter(|e| se(e) == (-1, -1)).collect();
    assert_eq!(non_cat.len(), 1);
    assert_eq!(
        res.avertissements,
        vec![AvertissementSeries::EpisodeNonDetecte(
            r.join("Talk Show/Bonus HDTV.mkv")
        )]
    );
    // Saison = année pour les épisodes datés.
    assert!(s.episodes.iter().filter(|e| se(e) == (2021, -1)).count() == 2);
    assert_eq!(
        s.saisons.iter().map(|x| x.numero).collect::<Vec<_>>(),
        vec![2021]
    );
    assert_eq!(
        s.saisons[0].premiere_diffusion.as_deref(),
        Some("2021-03-14")
    );
}

#[test]
fn identifiants_et_options() {
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path();
    creer(
        r,
        &[
            "Show tt0944947 tmdb-1399/S01E01.mkv",
            "Autre/Perso/S01E01.mkv",
            "Autre/S01E02.mkv",
            "Media/.nomedia",
            "Media/Show/S01E01.mkv",
        ],
    );
    let mut options = OptionsScan {
        dossiers_ignores: vec!["Perso".into()],
        ..OptionsScan::default()
    };
    let res = scanner_source_series(r, &options).unwrap();
    let s = res
        .series
        .iter()
        .find(|s| s.chemin.ends_with("Show tt0944947 tmdb-1399"))
        .unwrap();
    assert_eq!(s.ids.imdb(), "tt0944947");
    assert_eq!(s.ids.tmdb(), 1399);
    let autre = serie(&res, "Autre");
    assert_eq!(autre.episodes.len(), 1, "sous-dossier exclu par motif");
    assert!(!res.series.iter().any(|s| s.chemin.ends_with("Media")));

    // `.nomedia` désactivé : le dossier est scanné.
    options.ignorer_nomedia = false;
    let res = scanner_source_series(r, &options).unwrap();
    assert!(res.series.iter().any(|s| s.chemin.ends_with("Media")));
}

#[test]
fn erreurs_de_source() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(matches!(
        scanner_source_series(&tmp.path().join("absent"), &OptionsScan::default()),
        Err(ErreurScan::SourceIntrouvable(_))
    ));
    if !cfg!(windows) {
        assert!(matches!(
            scanner_source_series(tmp.path(), &OptionsScan::default()),
            Err(ErreurScan::SourceVide(_))
        ));
    }
    creer(tmp.path(), &["f.txt"]);
    assert!(matches!(
        scanner_source_series(&tmp.path().join("f.txt"), &OptionsScan::default()),
        Err(ErreurScan::PasUnDossier(_))
    ));
}

#[test]
fn dossier_serie_direct_et_bluray() {
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path();
    creer(
        r,
        &[
            "Show/Season 1/Episode 4/BDMV/index.bdmv",
            "Show/Season 1/Episode 4/BDMV/STREAM/00001.m2ts",
            "Show/Season 1/Episode 4/poster.jpg",
            "Show/Season 1/Show - 1x05 - Titre.mkv",
            "Show/Season 1/Show - 1x05 - Titre.mediainfo.xml",
        ],
    );
    let (s, avert) = scanner_dossier_serie(&r.join("Show"), r, &OptionsScan::default());
    assert!(avert.is_empty(), "{avert:?}");
    let s = s.expect("série");
    assert_eq!(s.episodes.len(), 2);
    let e = episode(&s, 1, 4);
    assert!(e.est_disque);
    let types = noms_types(e);
    assert!(types.contains(&("index.bdmv".into(), TypeFichier::Video)));
    assert!(types.contains(&("poster.jpg".into(), TypeFichier::Thumb)));
    assert_eq!(
        e.fichiers.len(),
        2,
        "les flux internes ne sont pas des épisodes"
    );
    let e = episode(&s, 1, 5);
    assert!(noms_types(e).contains(&(
        "Show - 1x05 - Titre.mediainfo.xml".into(),
        TypeFichier::MediaInfo
    )));
    assert_eq!(e.titre, "Titre");

    // Dossier sans vidéo : pas de série.
    creer(r, &["Rien/poster.jpg"]);
    assert!(
        scanner_dossier_serie(&r.join("Rien"), r, &OptionsScan::default())
            .0
            .is_none()
    );
}

#[test]
fn edition_depuis_nom() {
    assert_eq!(
        edition_episode("Show.S01E01.Directors.Cut.mkv"),
        Edition::DirectorsCut
    );
    assert_eq!(edition_episode("Show.S01E01.Uncut.mkv"), Edition::NonCoupee);
    assert_eq!(
        edition_episode("Show.S01E01.Remastered.mkv"),
        Edition::Remasterisee
    );
    assert_eq!(edition_episode("Show.S01E01.mkv"), Edition::Aucune);
}
