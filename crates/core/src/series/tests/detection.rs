//! Table d'exemples de noms de fichiers pour la détection saison / épisode.

use crate::series::detection::*;

/// `(nom ou chemin relatif, nom de série, saison attendue, épisodes attendus)`.
/// Saison -1 et épisodes vides : rien de détecté.
const CAS: &[(&str, Option<&str>, i32, &[i32])] = &[
    // --- Formes SxxEyy -------------------------------------------------
    ("Show.S01E02.1080p.mkv", None, 1, &[2]),
    ("Show.s01e02.mkv", None, 1, &[2]),
    ("Show - S1E2 - Titre.avi", None, 1, &[2]),
    ("Show.S10E15.720p.HDTV.x264-GRP.mkv", None, 10, &[15]),
    ("Show S01 E02.mkv", None, 1, &[2]),
    ("Show.S01.E02.mkv", None, 1, &[2]),
    ("Show_S01_E02.mkv", None, 1, &[2]),
    ("Show.S2014E05.mkv", None, 2014, &[5]),
    ("Show.S01E100.mkv", None, 1, &[100]),
    ("Show.S01E00.mkv", None, 1, &[0]),
    ("Show.S00E05.mkv", None, 0, &[5]),
    ("Specials/Show S00E05.mkv", None, 0, &[5]),
    ("Season 3/Show.S01E02.mkv", None, 1, &[2]),
    // --- Multi-épisodes ------------------------------------------------
    ("Show.S02E03E04.mkv", None, 2, &[3, 4]),
    ("Show.S01E01E02E03.mkv", None, 1, &[1, 2, 3]),
    ("Show.S01E01-E03.mkv", None, 1, &[1, 2, 3]),
    ("Show.S01E01-03.mkv", None, 1, &[1, 2, 3]),
    ("Show.S01E05-E06.mkv", None, 1, &[5, 6]),
    ("Show.S01E01-E06.mkv", None, 1, &[1, 2, 3, 4, 5, 6]),
    ("Show.S01E01.E02.mkv", None, 1, &[1, 2]),
    ("Show.S01E01-2009.mkv", None, 1, &[1]),
    ("Show.S01E02E01.mkv", None, 1, &[2]),
    ("Show.1x01x02.mkv", None, 1, &[1, 2]),
    // --- Formes NxMM -----------------------------------------------------
    ("Show - 1x02 - Titre.avi", None, 1, &[2]),
    ("Show.2x01.mkv", None, 2, &[1]),
    ("Show 12x05.mkv", None, 12, &[5]),
    ("Show.1x02.1280x720.mkv", None, 1, &[2]),
    ("Show.1920x1080.S03E04.mkv", None, 3, &[4]),
    // --- Nombres seuls ---------------------------------------------------
    ("Show 102.mkv", None, 1, &[2]),
    ("Show.201.mkv", None, 2, &[1]),
    ("Show - 05.mkv", None, 1, &[5]),
    ("Show - 7.mkv", None, 1, &[7]),
    ("Show 2009 105.mkv", None, 1, &[5]),
    ("Season 2/05 - Titre.mkv", None, 2, &[5]),
    ("Season 2/Show 05.mkv", None, 2, &[5]),
    ("Season 12/1205 - Titre.mkv", None, 12, &[5]),
    ("Saison 3/Show - 04.mkv", None, 3, &[4]),
    ("Staffel 2/Show - 03.mkv", None, 2, &[3]),
    ("S02/Show 03.mkv", None, 2, &[3]),
    ("Season 1/01.mkv", None, 1, &[1]),
    ("Show S2 05.mkv", None, 2, &[5]),
    // --- Pièges : mots techniques, années, nom de série ----------------
    ("Show.1080p.mkv", None, -1, &[]),
    ("Show.720p.HDTV.x264-LOL.mkv", None, -1, &[]),
    ("Show.HDTV.XviD-LOL.avi", None, -1, &[]),
    ("Show.H.265.mkv", None, -1, &[]),
    ("Show.x265.HEVC.10bit.mkv", None, -1, &[]),
    ("Show.DD5.1.AAC2.0.mkv", None, -1, &[]),
    ("Show (2019).mkv", None, -1, &[]),
    ("Show.S01E02.H.264.mkv", None, 1, &[2]),
    ("Show.E05.1080p.WEB-DL.DD5.1.H.264-GRP.mkv", None, 1, &[5]),
    ("Show.105.1080p.x264.mkv", None, 1, &[5]),
    ("Show 2160p 4K 105.mkv", None, 1, &[5]),
    ("Doctor Who 2005 - S01E01.mkv", Some("Doctor Who"), 1, &[1]),
    (
        "Doctor.Who.2005.S02E03.mkv",
        Some("Doctor Who (2005)"),
        2,
        &[3],
    ),
    ("Doctor.Who.2005.101.mkv", Some("Doctor Who"), 1, &[1]),
    ("Doctor Who (2005) - 1x05.mkv", Some("Doctor Who"), 1, &[5]),
    ("Doctor Who 2005 - 05.mkv", Some("Doctor Who 2005"), 1, &[5]),
    ("The.100.S01E01.mkv", Some("The 100"), 1, &[1]),
    ("The.100.105.mkv", Some("The 100"), 1, &[5]),
    ("24 - 3x05.mkv", Some("24"), 3, &[5]),
    ("1883.S01E01.mkv", Some("1883"), 1, &[1]),
    ("1883 - 101.mkv", Some("1883"), 1, &[1]),
    ("Show.S01E02.Title.With.2009.mkv", None, 1, &[2]),
    ("Show - 3x07 - The 4x4 Car.mkv", None, 3, &[7]),
    ("Show.S01E01.DTS-HD.MA.5.1.mkv", None, 1, &[1]),
    ("Show.Name.S03E09.HDR.2160p.WEB.H265-GRP.mkv", None, 3, &[9]),
    ("Show/Season 01/Show - 01 - 1080p.mkv", None, 1, &[1]),
    ("Show - 101 - 102.mkv", None, 1, &[2]),
    // --- Formes « Season / Episode » et assimilées ---------------------
    ("Show.Season 3.Episode 7.mkv", None, 3, &[7]),
    ("Show Season 2 Ep 4.mkv", None, 2, &[4]),
    ("Season 1/Episode 3.mkv", None, 1, &[3]),
    ("Show - Episode 12.mkv", None, 1, &[12]),
    ("Show.Ep.03.mkv", None, 1, &[3]),
    ("Show.E07.mkv", None, 1, &[7]),
    ("Show - Ep 4.mkv", None, 1, &[4]),
    ("Staffel 4/Episode 2.mkv", None, 4, &[2]),
    ("Temporada 2/Show 03.mkv", None, 2, &[3]),
    // --- Chiffres romains, « n sur total » -------------------------------
    ("Show Part IV.mkv", None, 1, &[4]),
    ("Show.Pt.II.mkv", None, 1, &[2]),
    ("Show Part Deux.mkv", None, -1, &[]),
    ("Show 1⧸2.mkv", None, 1, &[1]),
    ("Show 3⧸2.mkv", None, -1, &[]),
    // --- Animés -----------------------------------------------------------
    ("[Group] Show - 05 [ABCD1234].mkv", None, 1, &[5]),
    ("[Group] Show - 05 [1080p][ABCD1234].mkv", None, 1, &[5]),
    ("[Group] Show - 05v2 [ABCD1234].mkv", None, 1, &[5]),
    ("[Group] Show - 01-02 [ABCD1234].mkv", None, 1, &[1, 2]),
    (
        "[Group] Show - 01-02-03 [ABCD1234].mkv",
        None,
        1,
        &[1, 2, 3],
    ),
    ("Show OVA 02 [ABCD1234].mkv", None, 0, &[2]),
    ("[Grp] Show Special 1 [DEADBEEF].mkv", None, 0, &[1]),
    ("[Grp] Show S2 - 05 [ABCD1234].mkv", None, 2, &[5]),
    ("[Grp] Show S2 - 05-07 [ABCD1234].mkv", None, 2, &[5, 6, 7]),
    ("Season 2/[Grp] Show - 05 [ABCD1234].mkv", None, 2, &[5]),
    ("Specials/[Grp] Show - 03 [ABCD1234].mkv", None, 0, &[3]),
    ("Specials/Show - 03.mkv", None, 0, &[3]),
    ("Show.S01E02.720p [ABCD1234].mkv", None, 1, &[2]),
    ("Crisp 05.mkv", None, 1, &[5]),
    // --- Disques -----------------------------------------------------------
    ("Season 1/S01E03/VIDEO_TS/VIDEO_TS.IFO", None, 1, &[3]),
    ("S02E04/BDMV/STREAM/00001.m2ts", None, 2, &[4]),
    ("Show 1x05/VIDEO_TS/VTS_01_1.VOB", None, 1, &[5]),
    // --- Séparateurs Windows ---------------------------------------------
    ("Season 4\\Show - 06.mkv", None, 4, &[6]),
];

#[test]
fn table_de_detection() {
    let mut echecs = Vec::new();
    for (nom, serie, saison, episodes) in CAS {
        let r = detecter_episodes(nom, *serie);
        if r.saison != *saison || r.episodes != *episodes {
            echecs.push(format!(
                "{nom:?} : attendu ({saison}, {episodes:?}), obtenu ({}, {:?})",
                r.saison, r.episodes
            ));
        }
    }
    assert!(CAS.len() >= 60, "table trop courte : {}", CAS.len());
    assert!(
        echecs.is_empty(),
        "{} échec(s) :\n{}",
        echecs.len(),
        echecs.join("\n")
    );
}

#[test]
fn dates() {
    let r = detecter_episodes("Show.2021-03-14.mkv", None);
    assert_eq!(r.date.as_deref(), Some("2021-03-14"));
    assert_eq!((r.saison, r.episodes.clone()), (2021, vec![]));

    let r = detecter_episodes("The Daily Show 2021.03.14 Invité.mkv", None);
    assert_eq!(r.date.as_deref(), Some("2021-03-14"));
    assert_eq!(r.saison, 2021);

    let r = detecter_episodes("Show 14-03-2021.mkv", None);
    assert_eq!(r.date.as_deref(), Some("2021-03-14"));

    // Date + numéro explicite : le numéro l'emporte, la date est conservée.
    let r = detecter_episodes("Show.S03E10.2021-03-14.mkv", None);
    assert_eq!((r.saison, r.episodes.clone()), (3, vec![10]));
    assert_eq!(r.date.as_deref(), Some("2021-03-14"));

    // Date invalide (mois 13) : pas une date.
    let r = detecter_episodes("Show.2021-13-14.mkv", None);
    assert_eq!(r.date, None);
}

#[test]
fn marqueur_de_pile() {
    assert!(detecter_episodes("Show.S01E01.CD1.mkv", None).marqueur_pile_trouve);
    assert!(detecter_episodes("Season 1/Show - 1x01 - part2.avi", None).marqueur_pile_trouve);
    assert!(!detecter_episodes("Show.S01E01.mkv", None).marqueur_pile_trouve);
    let r = detecter_episodes("Show.S01E01.CD2.mkv", None);
    assert_eq!((r.saison, r.episodes), (1, vec![1]));
}

#[test]
fn titres_nettoyes() {
    let cas: &[(&str, Option<&str>, &str)] = &[
        ("Show - S01E02 - Le Titre.mkv", Some("Show"), "Le Titre"),
        (
            "Show.S01E02.Le.Titre.720p.HDTV.x264-GRP.mkv",
            Some("Show"),
            "Le Titre",
        ),
        ("Season 1/Show - 1x02 - Pilot.avi", Some("Show"), "Pilot"),
        ("Show 102 - Pilot.mkv", Some("Show"), "Pilot"),
        (
            "[Grp] Show - 05 [ABCD1234].mkv",
            Some("Show"),
            "[Grp] Show - 05 [ABCD1234].mkv",
        ),
        ("Show.S01E02.mkv", Some("Show"), "Show.S01E02.mkv"),
        (
            "Doctor Who (2005) - S01E01 - Rose.mkv",
            Some("Doctor Who"),
            "Rose",
        ),
        ("Show - 3x07 - The 4x4 Car.mkv", Some("Show"), "The 4x4 Car"),
        (
            "Show.Name.S03E09.Le.Retour.HDR.2160p.WEB.H265-GRP.mkv",
            Some("Show Name"),
            "Le Retour",
        ),
        ("Show.S01E01.Titre.CD1.avi", Some("Show"), "Titre"),
        ("Show.S01E01.Titre.DTS-HD.MA.5.1.mkv", Some("Show"), "Titre"),
    ];
    for (nom, serie, attendu) in cas {
        assert_eq!(nettoyer_titre_episode(nom, *serie), *attendu, "{nom}");
    }
    assert_eq!(
        detecter_episodes("Show - S01E02 - Le Titre.mkv", Some("Show")).nom_nettoye,
        "Le Titre"
    );
}

#[test]
fn chiffres_romains() {
    assert_eq!(decoder_romain("IV"), Some(4));
    assert_eq!(decoder_romain("ix"), Some(9));
    assert_eq!(decoder_romain("MCMXCIX"), Some(1999));
    assert_eq!(decoder_romain("XLII"), Some(42));
    assert_eq!(decoder_romain(""), None);
    assert_eq!(decoder_romain("ABC"), None);
}

#[test]
fn saison_des_artworks() {
    assert_eq!(
        detecter_saison_fichier_dossier("season01-poster.jpg", ""),
        Some(1)
    );
    assert_eq!(
        detecter_saison_fichier_dossier("season12-fanart.jpg", ""),
        Some(12)
    );
    assert_eq!(
        detecter_saison_fichier_dossier("season-specials-poster.jpg", ""),
        Some(0)
    );
    assert_eq!(
        detecter_saison_fichier_dossier("poster.jpg", "Specials"),
        Some(0)
    );
    assert_eq!(
        detecter_saison_fichier_dossier("season-all-poster.jpg", ""),
        Some(-1)
    );
    assert_eq!(
        detecter_saison_fichier_dossier("folder.jpg", "Season 3"),
        Some(3)
    );
    assert_eq!(
        detecter_saison_fichier_dossier("folder.jpg", "Staffel 4"),
        Some(4)
    );
    assert_eq!(detecter_saison_fichier_dossier("poster.jpg", ""), None);
}

#[test]
fn dossiers_de_saison() {
    for n in [
        "Season 1",
        "season01",
        "S02",
        "Staffel 3",
        "Series 4",
        "Specials",
        "Saison 2",
    ] {
        assert!(est_dossier_saison(n), "{n}");
    }
    for n in ["Extras", "Show", "Season", "S01E01"] {
        assert!(!est_dossier_saison(n), "{n}");
    }
}
