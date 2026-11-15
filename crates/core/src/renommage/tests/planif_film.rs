//! Planification pure du renommage des films.

use std::path::PathBuf;

use super::aides::*;
use crate::modele::*;
use crate::renommage::plan::ordonner_transferts;
use crate::renommage::*;

fn planifier(f: &Film, profil: &ProfilRenommageFilm) -> ElementPlan {
    planifier_film(
        FilmARenommer {
            film: f,
            saga: None,
        },
        profil,
        &SondeVide,
    )
}

fn defaut(f: &Film) -> ElementPlan {
    planifier(f, &ProfilRenommageFilm::default())
}

/// Nouveaux chemins (nom de fichier seul) d'un fichier existant.
fn nouveaux(el: &ElementPlan, ancien: &str) -> Vec<String> {
    el.lignes
        .iter()
        .find(|l| l.ancien.as_path() == std::path::Path::new(ancien))
        .unwrap_or_else(|| panic!("ligne absente : {ancien}"))
        .nouveaux
        .iter()
        .map(|n| n.to_string_lossy().into_owned())
        .collect()
}

fn ligne(el: &ElementPlan, ancien: &str) -> LigneFichier {
    el.lignes
        .iter()
        .find(|l| l.ancien.as_path() == std::path::Path::new(ancien))
        .cloned()
        .unwrap()
}

#[test]
fn renommage_simple_dossier_et_video() {
    let f = matrix();
    let el = defaut(&f);
    assert!(el.est_executable(), "{:?}", el.conflits);
    assert_eq!(el.nouveau_dossier, p(DOSSIER_MATRIX));
    assert_eq!(
        el.operations[0],
        Operation::DeplacerDossier {
            de: p("/films/Matrix.1999.1080p"),
            vers: p(DOSSIER_MATRIX)
        }
    );
    // Après le déplacement du dossier, le fichier est renommé en place.
    assert_eq!(
        el.operations[1],
        Operation::Deplacer {
            de: p("/films/The Matrix (1999)/Matrix.1999.1080p.mkv"),
            vers: p(&format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.mkv"))
        }
    );
    assert!(
        el.operations
            .contains(&Operation::NettoyerSousDossiersVides {
                racine: p(DOSSIER_MATRIX)
            })
    );
    let l = ligne(&el, "/films/Matrix.1999.1080p/Matrix.1999.1080p.mkv");
    assert_eq!(l.action, ActionFichier::Deplace);
    assert_eq!(el.fichiers_apres.len(), 1);
    assert_eq!(el.fichiers_apres[0].nom, format!("{BASE_MATRIX}.mkv"));
    assert_eq!(el.fichiers_apres[0].chemin, p(DOSSIER_MATRIX));
    let h = el.historique();
    assert_eq!(h.ancien_chemin, p("/films/Matrix.1999.1080p"));
    assert_eq!(h.nouveau_chemin, p(DOSSIER_MATRIX));
    assert_eq!(
        h.fichiers,
        vec![(
            "Matrix.1999.1080p.mkv".to_string(),
            format!("{BASE_MATRIX}.mkv")
        )]
    );
}

#[test]
fn deja_bien_nomme_inchange() {
    let mut f = matrix();
    f.chemin = p(DOSSIER_MATRIX);
    f.fichiers = vec![video_hd(&format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.mkv"))];
    let el = defaut(&f);
    assert!(el.est_inchange(), "{:?}", el.operations);
    assert!(el.lignes.iter().all(LigneFichier::est_inchange));
    assert!(el.historique().fichiers.is_empty());
}

#[test]
fn dossier_et_fichier_desactives() {
    let f = matrix();
    let profil = ProfilRenommageFilm {
        dossier_actif: false,
        fichier_actif: false,
        ..Default::default()
    };
    let el = planifier(&f, &profil);
    assert!(el.est_inchange());
    assert_eq!(el.nouveau_dossier, f.chemin);
    // Seul le fichier est renommé.
    let profil = ProfilRenommageFilm {
        dossier_actif: false,
        ..Default::default()
    };
    let el = planifier(&f, &profil);
    assert_eq!(
        nouveaux(&el, "/films/Matrix.1999.1080p/Matrix.1999.1080p.mkv"),
        vec![format!("/films/Matrix.1999.1080p/{BASE_MATRIX}.mkv")]
    );
}

#[test]
fn piles_cd1_cd2() {
    let mut f = matrix();
    f.empile = true;
    let mut a = video_hd("/films/Matrix.1999.1080p/Matrix.CD1.avi");
    a.empilement = 1;
    a.marqueur_empilement = "CD1".into();
    let mut b = video_hd("/films/Matrix.1999.1080p/Matrix.CD2.avi");
    b.empilement = 2;
    b.marqueur_empilement = "CD2".into();
    let mut s = fichier(
        "/films/Matrix.1999.1080p/Matrix.CD2.fr.srt",
        TypeFichier::SousTitres,
    );
    s.empilement = 2;
    s.marqueur_empilement = "CD2".into();
    f.fichiers = vec![a, b, s];
    let el = defaut(&f);
    assert!(el.est_executable(), "{:?}", el.conflits);
    assert_eq!(
        nouveaux(&el, "/films/Matrix.1999.1080p/Matrix.CD1.avi"),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX} CD1.avi")]
    );
    assert_eq!(
        nouveaux(&el, "/films/Matrix.1999.1080p/Matrix.CD2.avi"),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX} CD2.avi")]
    );
    assert_eq!(
        nouveaux(&el, "/films/Matrix.1999.1080p/Matrix.CD2.fr.srt"),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX} CD2.fra.srt")]
    );
    // Remplacement des espaces : le séparateur de pile suit.
    let profil = ProfilRenommageFilm {
        remplacer_espaces_fichier: true,
        remplacement_espaces_fichier: ".".into(),
        ..Default::default()
    };
    let el = planifier(&f, &profil);
    assert_eq!(
        nouveaux(&el, "/films/Matrix.1999.1080p/Matrix.CD1.avi"),
        vec![format!(
            "{DOSSIER_MATRIX}/The.Matrix.(1999).1080p.AC3.CD1.avi"
        )]
    );
}

#[test]
fn pile_sans_marqueur() {
    let mut f = matrix();
    let mut a = video_hd("/films/Matrix.1999.1080p/a.mkv");
    a.empilement = 1;
    let mut b = video_hd("/films/Matrix.1999.1080p/b.mkv");
    b.empilement = 2;
    f.fichiers = vec![a, b];
    let el = defaut(&f);
    assert_eq!(
        nouveaux(&el, "/films/Matrix.1999.1080p/b.mkv"),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX} CD2.mkv")]
    );
}

#[test]
fn sous_titres_langues() {
    let d = "/films/Matrix.1999.1080p";
    let mut f = matrix();
    let mut flux = fichier(&format!("{d}/x.srt"), TypeFichier::SousTitres);
    flux.sous_titres.push(FluxSousTitres {
        langue: "en".into(),
        drapeaux: vec![DrapeauFlux::Force, DrapeauFlux::Malentendants],
        ..Default::default()
    });
    f.fichiers.push(fichier(
        &format!("{d}/Matrix.1999.1080p.fr.srt"),
        TypeFichier::SousTitres,
    ));
    f.fichiers.push(fichier(
        &format!("{d}/Matrix.1999.1080p.de.forced.ass"),
        TypeFichier::SousTitres,
    ));
    f.fichiers.push(fichier(
        &format!("{d}/Matrix.1999.1080p.srt"),
        TypeFichier::SousTitres,
    ));
    f.fichiers.push(fichier(
        &format!("{d}/spanish.srt"),
        TypeFichier::SousTitres,
    ));
    f.fichiers.push(fichier(
        &format!("{d}/commentaire.srt"),
        TypeFichier::SousTitres,
    ));
    f.fichiers.push(flux);
    let el = defaut(&f);
    assert!(el.est_executable(), "{:?}", el.conflits);
    let n = |a: &str| nouveaux(&el, &format!("{d}/{a}"));
    assert_eq!(
        n("Matrix.1999.1080p.fr.srt"),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.fra.srt")]
    );
    assert_eq!(
        n("Matrix.1999.1080p.de.forced.ass"),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.deu.forced.ass")]
    );
    assert_eq!(
        n("Matrix.1999.1080p.srt"),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.srt")]
    );
    assert_eq!(
        n("spanish.srt"),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.spa.srt")]
    );
    assert_eq!(
        n("commentaire.srt"),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.commentaire.srt")]
    );
    assert_eq!(
        n("x.srt"),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.eng.forced.sdh.srt")]
    );
    // Style ISO 639-1 et original.
    let profil = ProfilRenommageFilm {
        style_langue_sous_titres: StyleLangue::Iso2,
        ..Default::default()
    };
    let el = planifier(&f, &profil);
    assert_eq!(
        nouveaux(&el, &format!("{d}/x.srt")),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.en.forced.sdh.srt")]
    );
    let profil = ProfilRenommageFilm {
        style_langue_sous_titres: StyleLangue::Original,
        ..Default::default()
    };
    let el = planifier(&f, &profil);
    assert_eq!(
        nouveaux(&el, &format!("{d}/Matrix.1999.1080p.fr.srt")),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.fr.srt")]
    );
}

#[test]
fn sub_entraine_idx() {
    let d = "/films/Matrix.1999.1080p";
    let mut f = matrix();
    let mut sub = fichier(&format!("{d}/piste.sub"), TypeFichier::SousTitres);
    sub.sous_titres.push(FluxSousTitres {
        langue: "fr".into(),
        ..Default::default()
    });
    f.fichiers.push(sub);
    f.fichiers
        .push(fichier(&format!("{d}/piste.idx"), TypeFichier::SousTitres));
    let el = defaut(&f);
    assert_eq!(
        nouveaux(&el, &format!("{d}/piste.sub")),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.fra.sub")]
    );
    assert_eq!(
        nouveaux(&el, &format!("{d}/piste.idx")),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.fra.idx")]
    );
}

#[test]
fn artwork_nfo_et_copies() {
    let d = "/films/Matrix.1999.1080p";
    let mut f = matrix();
    f.fichiers
        .push(fichier(&format!("{d}/folder.jpeg"), TypeFichier::Poster));
    let mut ancien = fichier(&format!("{d}/fanart.jpg"), TypeFichier::Fanart);
    ancien.date_fichier = 1;
    let mut recent = fichier(&format!("{d}/backdrop.png"), TypeFichier::Fanart);
    recent.date_fichier = 2;
    f.fichiers.push(ancien);
    f.fichiers.push(recent);
    f.fichiers
        .push(fichier(&format!("{d}/movie.nfo"), TypeFichier::Nfo));
    f.fichiers
        .push(fichier(&format!("{d}/logo.png"), TypeFichier::Logo));
    let mut profil = ProfilRenommageFilm::default();
    profil.noms.poster = vec!["<b>-poster".into(), "poster".into()];
    let el = planifier(&f, &profil);
    assert!(el.est_executable(), "{:?}", el.conflits);
    let n = |a: &str| nouveaux(&el, &format!("{d}/{a}"));
    assert_eq!(
        n("folder.jpeg"),
        vec![
            format!("{DOSSIER_MATRIX}/{BASE_MATRIX}-poster.jpg"),
            format!("{DOSSIER_MATRIX}/poster.jpg")
        ]
    );
    assert_eq!(
        n("backdrop.png"),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX}-fanart.png")]
    );
    assert_eq!(
        ligne(&el, &format!("{d}/fanart.jpg")).action,
        ActionFichier::Supprime
    );
    assert_eq!(
        n("movie.nfo"),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.nfo")]
    );
    assert_eq!(
        n("logo.png"),
        vec![format!("{DOSSIER_MATRIX}/{BASE_MATRIX}-clearlogo.png")]
    );
    // Copie puis déplacement de la même source, dans cet ordre.
    let src = p(&format!("{DOSSIER_MATRIX}/folder.jpeg"));
    let i_copie = el
        .operations
        .iter()
        .position(|o| matches!(o, Operation::Copier { de, .. } if de == &src))
        .unwrap();
    let i_dep = el
        .operations
        .iter()
        .position(|o| matches!(o, Operation::Deplacer { de, .. } if de == &src))
        .unwrap();
    assert!(i_copie < i_dep);
    assert!(el.operations.contains(&Operation::Supprimer {
        chemin: p(&format!("{DOSSIER_MATRIX}/fanart.jpg"))
    }));
    // Fichiers du modèle après renommage : poster en deux exemplaires.
    assert_eq!(
        el.fichiers_apres
            .iter()
            .filter(|x| x.type_fichier == TypeFichier::Poster)
            .count(),
        2
    );
    assert!(!el.fichiers_apres.iter().any(|x| x.nom == "fanart.jpg"));
}

#[test]
fn nfo_multiples_et_nettoyage() {
    let d = "/films/Matrix.1999.1080p";
    let mut f = matrix();
    let mut a = fichier(&format!("{d}/movie.nfo"), TypeFichier::Nfo);
    a.date_fichier = 5;
    let mut b = fichier(&format!("{d}/vieux.nfo"), TypeFichier::Nfo);
    b.date_fichier = 1;
    f.fichiers.push(a);
    f.fichiers.push(b);
    let mut profil = ProfilRenommageFilm::default();
    profil.noms.nfo = vec!["<b>".into(), "movie".into()];
    let el = planifier(&f, &profil);
    assert_eq!(
        nouveaux(&el, &format!("{d}/movie.nfo")),
        vec![
            format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.nfo"),
            format!("{DOSSIER_MATRIX}/movie.nfo")
        ]
    );
    assert_eq!(
        ligne(&el, &format!("{d}/movie.nfo")).action,
        ActionFichier::Copie
    );
    assert_eq!(
        ligne(&el, &format!("{d}/vieux.nfo")).action,
        ActionFichier::SuitDossier
    );
    profil.nettoyage_nfo = true;
    let el = planifier(&f, &profil);
    assert_eq!(
        ligne(&el, &format!("{d}/vieux.nfo")).action,
        ActionFichier::Supprime
    );
}

#[test]
fn trailers_extras_et_divers() {
    let d = "/films/Matrix.1999.1080p";
    let mut f = matrix();
    for (n, t) in [
        ("Matrix.1999.1080p-trailer.mp4", TypeFichier::Trailer),
        ("Matrix-trailer2.mp4", TypeFichier::Trailer),
        ("trailers/vo.mp4", TypeFichier::Trailer),
        ("Matrix.1999.1080p-behindthescenes.mkv", TypeFichier::Extra),
        ("extras/making-of.mkv", TypeFichier::Extra),
        ("Matrix.1999.1080p-sample.mkv", TypeFichier::Sample),
        ("Matrix.1999.1080p-320-10.bif", TypeFichier::Bif),
        ("Matrix.1999.1080p-mediainfo.xml", TypeFichier::MediaInfo),
        ("Matrix.1999.1080p.mkv.vsmeta", TypeFichier::Vsmeta),
        ("Matrix.1999.1080p.skiptro.json", TypeFichier::Skiptro),
        ("Matrix.1999.1080p.sp.hash.json", TypeFichier::Scenepass),
        ("extrafanart/fanart3.jpg", TypeFichier::ExtraFanart),
        ("notes.txt", TypeFichier::Texte),
        ("theme.mp3", TypeFichier::Theme),
    ] {
        f.fichiers.push(fichier(&format!("{d}/{n}"), t));
    }
    let el = defaut(&f);
    assert!(el.est_executable(), "{:?}", el.conflits);
    let n = |a: &str| nouveaux(&el, &format!("{d}/{a}"));
    let m = |x: &str| vec![format!("{DOSSIER_MATRIX}/{x}")];
    assert_eq!(
        n("Matrix.1999.1080p-trailer.mp4"),
        m(&format!("{BASE_MATRIX}-trailer.mp4"))
    );
    assert_eq!(
        n("Matrix-trailer2.mp4"),
        m(&format!("{BASE_MATRIX}-trailer.2.mp4"))
    );
    assert_eq!(n("trailers/vo.mp4"), m("trailers/vo.mp4"));
    assert_eq!(
        n("Matrix.1999.1080p-behindthescenes.mkv"),
        m(&format!("{BASE_MATRIX}-behindthescenes.mkv"))
    );
    assert_eq!(n("extras/making-of.mkv"), m("extras/making-of.mkv"));
    assert_eq!(
        n("Matrix.1999.1080p-sample.mkv"),
        m(&format!("{BASE_MATRIX}-sample.mkv"))
    );
    assert_eq!(
        n("Matrix.1999.1080p-320-10.bif"),
        m(&format!("{BASE_MATRIX}-320-10.bif"))
    );
    assert_eq!(
        n("Matrix.1999.1080p-mediainfo.xml"),
        m(&format!("{BASE_MATRIX}-mediainfo.xml"))
    );
    assert_eq!(
        n("Matrix.1999.1080p.mkv.vsmeta"),
        m(&format!("{BASE_MATRIX}.mkv.vsmeta"))
    );
    assert_eq!(
        n("Matrix.1999.1080p.skiptro.json"),
        m(&format!("{BASE_MATRIX}.skiptro.json"))
    );
    assert_eq!(
        n("Matrix.1999.1080p.sp.hash.json"),
        m(&format!("{BASE_MATRIX}.sp.hash.json"))
    );
    assert_eq!(
        n("extrafanart/fanart3.jpg"),
        m(&format!("{BASE_MATRIX}-fanart3.jpg"))
    );
    assert_eq!(n("notes.txt"), m("notes.txt"));
    assert_eq!(
        ligne(&el, &format!("{d}/theme.mp3")).action,
        ActionFichier::SuitDossier
    );
    // Un sous-dossier quitté (extrafanart/) sera nettoyé par la passe finale.
    assert!(
        el.operations
            .iter()
            .any(|o| matches!(o, Operation::NettoyerSousDossiersVides { .. }))
    );
}

#[test]
fn extrafanart_dans_un_dossier() {
    let d = "/films/Matrix.1999.1080p";
    let mut f = matrix();
    f.fichiers.push(fichier(
        &format!("{d}/Matrix.1999.1080p-fanart2.jpg"),
        TypeFichier::ExtraFanart,
    ));
    let mut profil = ProfilRenommageFilm::default();
    profil.noms.extrafanart = vec!["extrafanart/fanart<n>".into()];
    let el = planifier(&f, &profil);
    assert_eq!(
        nouveaux(&el, &format!("{d}/Matrix.1999.1080p-fanart2.jpg")),
        vec![format!("{DOSSIER_MATRIX}/extrafanart/fanart2.jpg")]
    );
}

#[test]
fn gabarit_invalide_et_donnees_manquantes() {
    let f = matrix();
    let profil = ProfilRenommageFilm {
        gabarit_dossier: "${title".into(),
        ..Default::default()
    };
    let el = planifier(&f, &profil);
    assert!(matches!(el.conflits[0], Conflit::GabaritInvalide { .. }));
    assert!(el.operations.is_empty());
    let profil = ProfilRenommageFilm {
        gabarit_fichier: "${title} ${end}".into(),
        ..Default::default()
    };
    let el = planifier(&f, &profil);
    assert!(
        matches!(el.conflits[0], Conflit::GabaritInvalide { .. }),
        "{:?}",
        el.conflits
    );
    let mut f2 = matrix();
    f2.titre.clear();
    assert!(matches!(
        defaut(&f2).conflits[0],
        Conflit::DonneesManquantes(_)
    ));
    let mut f3 = matrix();
    f3.source_donnees = PathBuf::new();
    assert!(!defaut(&f3).est_executable());
}

#[test]
fn gabarit_fichier_sans_titre_non_renomme() {
    let f = matrix();
    let profil = ProfilRenommageFilm {
        gabarit_fichier: "${year}".into(),
        ..Default::default()
    };
    let el = planifier(&f, &profil);
    assert!(!el.avertissements.is_empty());
    assert_eq!(
        nouveaux(&el, "/films/Matrix.1999.1080p/Matrix.1999.1080p.mkv"),
        vec![format!("{DOSSIER_MATRIX}/Matrix.1999.1080p.mkv")]
    );
}

#[test]
fn dossier_existant_avec_sonde() {
    let f = matrix();
    let sonde = SondeListe::nouvelle([p(DOSSIER_MATRIX)]);
    let el = planifier_film(
        FilmARenommer {
            film: &f,
            saga: None,
        },
        &ProfilRenommageFilm::default(),
        &sonde,
    );
    assert!(
        el.conflits
            .contains(&Conflit::DossierExistant(p(DOSSIER_MATRIX)))
    );
    // Fusion autorisée : dossier non vide ⇒ multi-films, pas de conflit.
    let sonde = SondeListe::nouvelle([p(&format!("{DOSSIER_MATRIX}/autre.mkv"))]);
    let profil = ProfilRenommageFilm {
        autoriser_multi_films_meme_dossier: true,
        ..Default::default()
    };
    let el = planifier_film(
        FilmARenommer {
            film: &f,
            saga: None,
        },
        &profil,
        &sonde,
    );
    assert!(el.est_executable(), "{:?}", el.conflits);
    assert!(el.dossier_multi_films);
    assert_eq!(
        el.operations[0],
        Operation::CreerDossier {
            chemin: p(DOSSIER_MATRIX)
        }
    );
}

#[test]
fn fichier_existant_avec_sonde() {
    let mut f = matrix();
    f.chemin = p(DOSSIER_MATRIX);
    f.fichiers = vec![video_hd(&format!("{DOSSIER_MATRIX}/x.mkv"))];
    let cible = p(&format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.mkv"));
    let sonde = SondeListe::nouvelle([cible.clone()]);
    let el = planifier_film(
        FilmARenommer {
            film: &f,
            saga: None,
        },
        &ProfilRenommageFilm::default(),
        &sonde,
    );
    assert!(el.conflits.contains(&Conflit::FichierExistant(cible)));
}

#[test]
fn doublons_dans_un_film() {
    let d = "/films/Matrix.1999.1080p";
    let mut f = matrix();
    let mut a = fichier(&format!("{d}/a.srt"), TypeFichier::SousTitres);
    a.sous_titres.push(FluxSousTitres {
        langue: "fr".into(),
        ..Default::default()
    });
    let mut b = fichier(&format!("{d}/b.srt"), TypeFichier::SousTitres);
    b.sous_titres.push(FluxSousTitres {
        langue: "fre".into(),
        ..Default::default()
    });
    f.fichiers.push(a);
    f.fichiers.push(b);
    let el = defaut(&f);
    assert!(
        matches!(&el.conflits[0], Conflit::DestinationDupliquee { sources, .. } if sources.len() == 2)
    );
    assert!(ligne(&el, &format!("{d}/a.srt")).doublon);
    assert!(ligne(&el, &format!("{d}/b.srt")).doublon);
}

#[test]
fn doublons_entre_films() {
    let a = matrix();
    let mut b = matrix();
    b.id = "2".into();
    b.chemin = p("/films/autre");
    b.fichiers = vec![video_hd("/films/autre/autre.mkv")];
    let plan = planifier_films(&[&a, &b], &[], &ProfilRenommageFilm::default(), &SondeVide);
    assert!(plan.a_des_conflits());
    for e in &plan.elements {
        assert!(
            e.conflits
                .contains(&Conflit::DossierDuplique(p(DOSSIER_MATRIX))),
            "{:?}",
            e.conflits
        );
        assert!(
            e.conflits
                .iter()
                .any(|c| matches!(c, Conflit::DestinationDupliquee { .. }))
        );
    }
    // Deux films distincts : aucun conflit.
    let mut c = matrix();
    c.id = "3".into();
    c.titre = "Alien".into();
    c.annee = 1979;
    c.chemin = p("/films/alien");
    c.fichiers = vec![video_hd("/films/alien/alien.mkv")];
    let plan = planifier_films(&[&a, &c], &[], &ProfilRenommageFilm::default(), &SondeVide);
    assert!(!plan.a_des_conflits());
    assert_eq!(plan.conflits().len(), 0);
}

#[test]
fn source_multi_films_deplacement_par_fichier() {
    let mut f = matrix();
    f.chemin = p("/films");
    f.dossier_multi_films = true;
    f.fichiers = vec![
        video_hd("/films/Matrix.1999.mkv"),
        fichier("/films/Matrix.1999-poster.jpg", TypeFichier::Poster),
    ];
    let el = defaut(&f);
    assert!(el.est_executable(), "{:?}", el.conflits);
    assert!(!el.dossier_multi_films);
    assert_eq!(
        el.operations[0],
        Operation::CreerDossier {
            chemin: p(DOSSIER_MATRIX)
        }
    );
    assert!(el.operations.contains(&Operation::Deplacer {
        de: p("/films/Matrix.1999.mkv"),
        vers: p(&format!("{DOSSIER_MATRIX}/{BASE_MATRIX}.mkv"))
    }));
    assert!(
        !el.operations
            .iter()
            .any(|o| matches!(o, Operation::DeplacerDossier { .. }))
    );
    // La source n'est jamais supprimée.
    assert!(!el.operations.contains(&Operation::SupprimerDossierVide {
        chemin: p("/films")
    }));
}

#[test]
fn destination_multi_films_noms_bases_sur_le_fichier() {
    let d = "/films/Matrix.1999.1080p";
    let mut f = matrix();
    f.fichiers
        .push(fichier(&format!("{d}/poster.jpg"), TypeFichier::Poster));
    f.fichiers
        .push(fichier(&format!("{d}/movie.nfo"), TypeFichier::Nfo));
    let mut profil = ProfilRenommageFilm {
        gabarit_dossier: "${title;first}".into(),
        ..Default::default()
    };
    profil.noms.poster = vec!["poster".into()];
    profil.noms.nfo = vec!["movie".into()];
    let el = planifier(&f, &profil);
    assert!(el.dossier_multi_films);
    assert!(el.avertissements.iter().any(|a| a.contains("non unique")));
    assert_eq!(
        nouveaux(&el, &format!("{d}/poster.jpg")),
        vec![format!("/films/T/{BASE_MATRIX}-poster.jpg")]
    );
    assert_eq!(
        nouveaux(&el, &format!("{d}/movie.nfo")),
        vec![format!("/films/T/{BASE_MATRIX}.nfo")]
    );
    assert!(
        el.operations
            .contains(&Operation::SupprimerDossierVide { chemin: p(d) })
    );
    assert!(
        !el.operations
            .iter()
            .any(|o| matches!(o, Operation::NettoyerSousDossiersVides { .. }))
    );
}

#[test]
fn film_disque() {
    let d = "/films/Matrix DVD";
    let mut f = matrix();
    f.chemin = p(d);
    f.est_disque = true;
    let mut v = video_hd(&format!("{d}/VIDEO_TS/VTS_01_1.VOB"));
    v.nom = "VTS_01_1.VOB".into();
    f.fichiers = vec![
        v,
        fichier(&format!("{d}/VIDEO_TS/VIDEO_TS.IFO"), TypeFichier::Video),
        fichier(&format!("{d}/Matrix-poster.jpg"), TypeFichier::Poster),
        fichier(&format!("{d}/VIDEO_TS/VIDEO_TS.nfo"), TypeFichier::Nfo),
    ];
    let mut profil = ProfilRenommageFilm::default();
    profil.noms.poster = vec!["<b>-poster".into()];
    let el = planifier(&f, &profil);
    assert!(el.est_executable(), "{:?}", el.conflits);
    assert_eq!(
        nouveaux(&el, &format!("{d}/VIDEO_TS/VTS_01_1.VOB")),
        vec![format!("{DOSSIER_MATRIX}/VIDEO_TS/VTS_01_1.VOB")]
    );
    assert_eq!(
        nouveaux(&el, &format!("{d}/Matrix-poster.jpg")),
        vec![format!("{DOSSIER_MATRIX}/poster.jpg")]
    );
    assert_eq!(
        ligne(&el, &format!("{d}/VIDEO_TS/VIDEO_TS.nfo")).action,
        ActionFichier::SuitDossier
    );
}

#[test]
fn changement_de_casse_seulement() {
    let mut f = matrix();
    f.chemin = p("/films/the matrix (1999)");
    f.fichiers = vec![video_hd(&format!(
        "/films/the matrix (1999)/{BASE_MATRIX}.mkv"
    ))];
    let sonde = SondeListe::nouvelle([p("/films/the matrix (1999)"), p(DOSSIER_MATRIX)]);
    let el = planifier_film(
        FilmARenommer {
            film: &f,
            saga: None,
        },
        &ProfilRenommageFilm::default(),
        &sonde,
    );
    assert!(el.est_executable(), "{:?}", el.conflits);
    assert!(matches!(
        el.operations[0],
        Operation::DeplacerDossier { .. }
    ));
}

#[test]
fn nouveau_dossier_dans_l_ancien() {
    let mut f = matrix();
    f.chemin = p("/films/The Matrix (1999)");
    f.fichiers = vec![video_hd("/films/The Matrix (1999)/m.mkv")];
    let profil = ProfilRenommageFilm {
        gabarit_dossier: "${title} (${year})/${imdb}tt".into(),
        ..Default::default()
    };
    let el = planifier(&f, &profil);
    assert!(el.est_executable(), "{:?}", el.conflits);
    assert_eq!(el.nouveau_dossier, p("/films/The Matrix (1999)/tt"));
    assert!(
        !el.operations
            .iter()
            .any(|o| matches!(o, Operation::DeplacerDossier { .. }))
    );
    assert!(el.operations.contains(&Operation::Deplacer {
        de: p("/films/The Matrix (1999)/m.mkv"),
        vers: p(&format!("/films/The Matrix (1999)/tt/{BASE_MATRIX}.mkv"))
    }));
}

#[test]
fn ancetres_vides_nettoyes() {
    let mut f = matrix();
    f.chemin = p("/films/a/b/Matrix");
    f.fichiers = vec![video_hd("/films/a/b/Matrix/m.mkv")];
    let el = defaut(&f);
    assert!(el.operations.contains(&Operation::SupprimerDossierVide {
        chemin: p("/films/a/b")
    }));
    assert!(el.operations.contains(&Operation::SupprimerDossierVide {
        chemin: p("/films/a")
    }));
    assert!(!el.operations.contains(&Operation::SupprimerDossierVide {
        chemin: p("/films")
    }));
}

#[test]
fn ordre_des_transferts_et_cycles() {
    let a = p("/d/a");
    let b = p("/d/b");
    let c = p("/d/c");
    // a→b doit attendre que b→c ait libéré b.
    let ops = ordonner_transferts(
        vec![
            Operation::Deplacer {
                de: a.clone(),
                vers: b.clone(),
            },
            Operation::Deplacer {
                de: b.clone(),
                vers: c.clone(),
            },
        ],
        &[],
    );
    assert_eq!(
        ops[0],
        Operation::Deplacer {
            de: b.clone(),
            vers: c.clone()
        }
    );
    assert_eq!(
        ops[1],
        Operation::Deplacer {
            de: a.clone(),
            vers: b.clone()
        }
    );
    // Échange a↔b : passage par un nom temporaire.
    let ops = ordonner_transferts(
        vec![
            Operation::Deplacer {
                de: a.clone(),
                vers: b.clone(),
            },
            Operation::Deplacer {
                de: b.clone(),
                vers: a.clone(),
            },
        ],
        &[],
    );
    assert_eq!(ops.len(), 3);
    let tmp = match &ops[0] {
        Operation::Deplacer { de, vers } => {
            assert_eq!(de, &a);
            vers.clone()
        }
        o => panic!("{o:?}"),
    };
    assert_eq!(
        ops[1],
        Operation::Deplacer {
            de: b.clone(),
            vers: a.clone()
        }
    );
    assert_eq!(
        ops[2],
        Operation::Deplacer {
            de: tmp,
            vers: b.clone()
        }
    );
    // Une suppression qui libère une destination passe en premier.
    let ops = ordonner_transferts(
        vec![Operation::Deplacer {
            de: a.clone(),
            vers: c.clone(),
        }],
        &[
            Operation::Supprimer { chemin: b.clone() },
            Operation::Supprimer { chemin: c.clone() },
        ],
    );
    assert_eq!(ops[0], Operation::Supprimer { chemin: c.clone() });
    assert_eq!(ops[2], Operation::Supprimer { chemin: b });
}

#[test]
fn profils_nommes() {
    let mut profils: ProfilsRenommage<ProfilRenommageFilm> = ProfilsRenommage::nouveau();
    assert_eq!(profils.noms(), vec!["Default".to_string()]);
    profils.definir(ProfilRenommageFilm {
        nom: "Plex".into(),
        gabarit_dossier: "${title} (${year}) {imdb-${imdb}}".into(),
        ..Default::default()
    });
    assert_eq!(profils.obtenir("Plex").nom, "Plex");
    assert_eq!(profils.obtenir("absent").nom, "Default");
    assert!(!profils.supprimer("Default"));
    assert!(profils.supprimer("Plex"));
    let json = serde_json::to_string(&profils).unwrap();
    let relu: ProfilsRenommage<ProfilRenommageFilm> = serde_json::from_str(&json).unwrap();
    assert_eq!(relu, profils);
}

#[test]
fn appliquer_au_modele() {
    let mut f = matrix();
    let plan = planifier_films(&[&f], &[], &ProfilRenommageFilm::default(), &SondeVide);
    appliquer_au_film(&mut f, &plan.elements[0]);
    assert_eq!(f.chemin, p(DOSSIER_MATRIX));
    assert_eq!(f.fichiers[0].nom, format!("{BASE_MATRIX}.mkv"));
    assert!(f.historique_renommage.is_some());
}
