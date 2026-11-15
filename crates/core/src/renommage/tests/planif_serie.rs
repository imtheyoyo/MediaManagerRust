//! Planification pure du renommage des séries.

use super::aides::*;
use crate::modele::*;
use crate::renommage::*;

fn ep_video(id: &str, s: i32, e: i32, titre: &str, nom: &str) -> Episode {
    episode(
        id,
        s,
        e,
        titre,
        vec![video_hd(&format!("/series/show/{nom}"))],
    )
}

fn nouveaux(plan: &PlanRenommage, ancien: &str) -> Vec<String> {
    plan.elements
        .iter()
        .flat_map(|e| e.lignes.iter())
        .find(|l| l.ancien.as_path() == std::path::Path::new(ancien))
        .unwrap_or_else(|| panic!("ligne absente : {ancien}"))
        .nouveaux
        .iter()
        .map(|n| n.to_string_lossy().replace('\\', "/"))
        .collect()
}

#[test]
fn renommage_par_defaut() {
    let mut e = ep_video("1", 1, 2, "Pilot", "show.s01e02.mkv");
    e.fichiers.push(fichier(
        "/series/show/show.s01e02.fr.srt",
        TypeFichier::SousTitres,
    ));
    e.fichiers
        .push(fichier("/series/show/show.s01e02.nfo", TypeFichier::Nfo));
    e.fichiers.push(fichier(
        "/series/show/show.s01e02-thumb.jpg",
        TypeFichier::Thumb,
    ));
    let mut s = serie(vec![e]);
    s.fichiers
        .push(fichier("/series/show/poster.jpg", TypeFichier::Poster));
    let plan = planifier_serie(&s, &ProfilRenommageSerie::default(), &SondeVide);
    assert!(!plan.a_des_conflits(), "{:?}", plan.conflits());
    assert_eq!(plan.elements.len(), 2);
    let serie_el = &plan.elements[0];
    assert_eq!(serie_el.nouveau_dossier, p("/series/Show (2008)"));
    assert_eq!(
        serie_el.operations[0],
        Operation::DeplacerDossier {
            de: p("/series/show"),
            vers: p("/series/Show (2008)")
        }
    );
    assert_eq!(
        nouveaux(&plan, "/series/show/poster.jpg"),
        vec!["/series/Show (2008)/poster.jpg"]
    );
    let d = "/series/Show (2008)/Season 1";
    assert_eq!(plan.elements[1].depend_de, Some(0));
    assert_eq!(
        nouveaux(&plan, "/series/show/show.s01e02.mkv"),
        vec![format!("{d}/Show - S01E02 - Pilot.mkv")]
    );
    assert_eq!(
        nouveaux(&plan, "/series/show/show.s01e02.fr.srt"),
        vec![format!("{d}/Show - S01E02 - Pilot.fra.srt")]
    );
    assert_eq!(
        nouveaux(&plan, "/series/show/show.s01e02.nfo"),
        vec![format!("{d}/Show - S01E02 - Pilot.nfo")]
    );
    assert_eq!(
        nouveaux(&plan, "/series/show/show.s01e02-thumb.jpg"),
        vec![format!("{d}/Show - S01E02 - Pilot-thumb.jpg")]
    );
    // Les fichiers de l'épisode sont lus dans le dossier déjà renommé.
    assert!(plan.elements[1].operations.contains(&Operation::Deplacer {
        de: p("/series/Show (2008)/show.s01e02.mkv"),
        vers: p(&format!("{d}/Show - S01E02 - Pilot.mkv"))
    }));
}

#[test]
fn saison_speciale_et_dossier_saison() {
    let s = serie(vec![ep_video("1", 0, 1, "Bonus", "bonus.mkv")]);
    let plan = planifier_serie(&s, &ProfilRenommageSerie::default(), &SondeVide);
    assert_eq!(
        nouveaux(&plan, "/series/show/bonus.mkv"),
        vec!["/series/Show (2008)/Specials/Show - S00E01 - Bonus.mkv"]
    );
    let profil = ProfilRenommageSerie {
        saison_speciale: false,
        ..Default::default()
    };
    let plan = planifier_serie(&s, &profil, &SondeVide);
    assert_eq!(
        nouveaux(&plan, "/series/show/bonus.mkv"),
        vec!["/series/Show (2008)/Season 0/Show - S00E01 - Bonus.mkv"]
    );
    // Gabarit de saison qui donne un nom vide : repli sur « Season N ».
    let s = serie(vec![ep_video("1", 2, 1, "A", "a.mkv")]);
    let profil = ProfilRenommageSerie {
        gabarit_dossier_saison: "${seasonName}".into(),
        gabarit_fichier: "${showTitle} ${title}".into(),
        ..Default::default()
    };
    assert_eq!(
        nom_dossier_saison(&s, &s.episodes[0], &profil).unwrap(),
        "Season 2"
    );
}

#[test]
fn multi_episodes_repeat_et_range() {
    let v = video_hd("/series/show/show.s01e01e02.mkv");
    let a = episode("1", 1, 1, "A", vec![v.clone()]);
    let mut b = episode("2", 1, 2, "B", vec![v]);
    b.multi_episodes = true;
    let s = serie(vec![a, b]);
    let plan = planifier_serie(&s, &ProfilRenommageSerie::default(), &SondeVide);
    assert_eq!(
        plan.elements.len(),
        2,
        "un seul élément pour le fichier partagé"
    );
    assert_eq!(
        plan.elements[1].nature,
        NatureElement::Episodes {
            ids: vec!["1".into(), "2".into()]
        }
    );
    assert_eq!(
        nouveaux(&plan, "/series/show/show.s01e01e02.mkv"),
        vec!["/series/Show (2008)/Season 1/Show - S01E01 S01E02 - A - B.mkv"]
    );
    let profil = ProfilRenommageSerie {
        style_multi_episodes: StyleMulti::Plage,
        ..Default::default()
    };
    let plan = planifier_serie(&s, &profil, &SondeVide);
    assert_eq!(
        nouveaux(&plan, "/series/show/show.s01e01e02.mkv"),
        vec!["/series/Show (2008)/Season 1/Show - S01E01-E02 - A - B.mkv"]
    );
    let profil = ProfilRenommageSerie {
        style_multi_episodes: StyleMulti::Plage,
        gabarit_fichier: "${showTitle} ${seasonNr}x${episodeNr2} ${title}".into(),
        ..Default::default()
    };
    let plan = planifier_serie(&s, &profil, &SondeVide);
    assert_eq!(
        nouveaux(&plan, "/series/show/show.s01e01e02.mkv"),
        vec!["/series/Show (2008)/Season 1/Show 1x01-02 A - B.mkv"]
    );
}

#[test]
fn espaces_deux_points_et_barres() {
    let s = serie(vec![ep_video("1", 1, 2, "Doctor Who: Rose AC/DC", "x.mkv")]);
    let profil = ProfilRenommageSerie {
        remplacer_espaces_fichier: true,
        remplacement_espaces_fichier: "_".into(),
        remplacement_deux_points: "-".into(),
        ..Default::default()
    };
    let plan = planifier_serie(&s, &profil, &SondeVide);
    assert_eq!(
        nouveaux(&plan, "/series/show/x.mkv"),
        vec!["/series/Show (2008)/Season 1/Show_-_S01E02_-_Doctor_Who_-_Rose_AC_DC.mkv"]
    );
}

#[test]
fn dossiers_desactives() {
    let mut e = ep_video("1", 1, 2, "Pilot", "S1/x.mkv");
    e.fichiers[0].chemin = p("/series/show/S1");
    let s = serie(vec![e]);
    let profil = ProfilRenommageSerie {
        dossier_serie_actif: false,
        dossier_saison_actif: false,
        ..Default::default()
    };
    let plan = planifier_serie(&s, &profil, &SondeVide);
    assert!(plan.elements[0].est_inchange());
    assert_eq!(plan.elements[1].depend_de, None);
    assert_eq!(
        nouveaux(&plan, "/series/show/S1/x.mkv"),
        vec!["/series/show/S1/Show - S01E02 - Pilot.mkv"]
    );
}

#[test]
fn numeros_manquants_et_dependances() {
    let s = serie(vec![ep_video("1", -1, 2, "Pilot", "x.mkv")]);
    let plan = planifier_serie(&s, &ProfilRenommageSerie::default(), &SondeVide);
    assert!(matches!(
        plan.elements[1].conflits[0],
        Conflit::DonneesManquantes(_)
    ));
    // Dossier de série bloqué ⇒ épisodes bloqués par dépendance.
    let s = serie(vec![ep_video("1", 1, 2, "Pilot", "x.mkv")]);
    let sonde = SondeListe::nouvelle([p("/series/Show (2008)")]);
    let plan = planifier_serie(&s, &ProfilRenommageSerie::default(), &sonde);
    assert!(
        plan.elements[0]
            .conflits
            .contains(&Conflit::DossierExistant(p("/series/Show (2008)")))
    );
    assert!(plan.elements[1].conflits.contains(&Conflit::Dependance(0)));
}

#[test]
fn episode_disque() {
    let mut v = fichier(
        "/series/show/S01E01/VIDEO_TS/VTS_01_1.VOB",
        TypeFichier::Video,
    );
    v.taille = 10;
    let ifo = fichier(
        "/series/show/S01E01/VIDEO_TS/VIDEO_TS.IFO",
        TypeFichier::Video,
    );
    let mut e = episode("1", 1, 1, "Pilot", vec![v, ifo]);
    e.est_disque = true;
    let s = serie(vec![e]);
    let plan = planifier_serie(&s, &ProfilRenommageSerie::default(), &SondeVide);
    assert!(!plan.a_des_conflits(), "{:?}", plan.conflits());
    let el = &plan.elements[1];
    assert_eq!(
        el.operations[0],
        Operation::DeplacerDossier {
            de: p("/series/Show (2008)/S01E01"),
            vers: p("/series/Show (2008)/Season 1/Show - S01E01 - Pilot")
        }
    );
    assert_eq!(
        nouveaux(&plan, "/series/show/S01E01/VIDEO_TS/VTS_01_1.VOB"),
        vec!["/series/Show (2008)/Season 1/Show - S01E01 - Pilot/VIDEO_TS/VTS_01_1.VOB"]
    );
}

#[test]
fn episodes_fictifs_ignores_et_doublons() {
    let mut fictif = episode("f", 1, 3, "Futur", vec![]);
    fictif.fictif = true;
    let a = ep_video("1", 1, 1, "Même", "a.mkv");
    let b = ep_video("2", 1, 1, "Même", "b.mkv");
    let s = serie(vec![a, b, fictif]);
    let plan = planifier_serie(&s, &ProfilRenommageSerie::default(), &SondeVide);
    assert_eq!(plan.elements.len(), 3);
    assert!(
        plan.elements[1]
            .conflits
            .iter()
            .any(|c| matches!(c, Conflit::DestinationDupliquee { .. }))
    );
    assert!(plan.elements[2].lignes[0].doublon);
}

#[test]
fn recommandations() {
    assert!(gabarits_recommandes(
        "Season ${seasonNr}",
        "${showTitle} - S${seasonNr2}E${episodeNr2}"
    ));
    assert!(gabarits_recommandes("", "${originalFilename}"));
    assert!(!gabarits_recommandes("", "${showTitle} ${title}"));
    assert!(!gabarits_recommandes("", "E${episodeNr} S${seasonNr}"));
}
