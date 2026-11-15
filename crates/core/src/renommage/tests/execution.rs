//! Exécution sur arborescences temporaires : succès, corbeille, conflits
//! apparus après la planification, retour arrière, multi-volumes simulé.

use std::fs;
use std::path::Path;

use super::aides::*;
use crate::modele::*;
use crate::renommage::execution::{copier_fichier, deplacer_dossier, deplacer_fichier};
use crate::renommage::*;

fn planifier(films: &[&Film], profil: &ProfilRenommageFilm) -> PlanRenommage {
    planifier_films(films, &[], profil, &SondeSystemeFichiers)
}

fn film_complet(source: &Path) -> Film {
    creer(
        source,
        &[
            "Matrix.1999/Matrix.1999.mkv",
            "Matrix.1999/Matrix.1999.fr.srt",
            "Matrix.1999/folder.jpg",
            "Matrix.1999/fanart.jpg",
            "Matrix.1999/vieux-fanart.jpg",
            "Matrix.1999/movie.nfo",
            "Matrix.1999/extrafanart/fanart1.jpg",
            "Matrix.1999/lisezmoi.txt",
        ],
    );
    let mut f = film_disque(
        source,
        "Matrix.1999",
        &[
            ("Matrix.1999.mkv", TypeFichier::Video),
            ("Matrix.1999.fr.srt", TypeFichier::SousTitres),
            ("folder.jpg", TypeFichier::Poster),
            ("fanart.jpg", TypeFichier::Fanart),
            ("vieux-fanart.jpg", TypeFichier::Fanart),
            ("movie.nfo", TypeFichier::Nfo),
            ("extrafanart/fanart1.jpg", TypeFichier::ExtraFanart),
            ("lisezmoi.txt", TypeFichier::Texte),
        ],
    );
    f.fichiers[3].date_fichier = 10;
    f
}

const B: &str = "The Matrix (1999) 1080p AC3";

#[test]
fn renommage_complet_puis_modele() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path();
    let mut f = film_complet(src);
    let mut profil = ProfilRenommageFilm::default();
    profil.noms.poster = vec!["<b>-poster".into(), "poster".into()];
    let plan = planifier(&[&f], &profil);
    assert!(!plan.a_des_conflits(), "{:?}", plan.conflits());
    let rapport = executer(&plan, &OptionsExecution::default());
    assert!(rapport.est_complet(), "{rapport:?}");
    let attendu: Vec<String> = [
        "The Matrix (1999)/".to_string(),
        format!("The Matrix (1999)/{B}-fanart.jpg"),
        format!("The Matrix (1999)/{B}-fanart1.jpg"),
        format!("The Matrix (1999)/{B}-poster.jpg"),
        format!("The Matrix (1999)/{B}.fra.srt"),
        format!("The Matrix (1999)/{B}.mkv"),
        format!("The Matrix (1999)/{B}.nfo"),
        "The Matrix (1999)/lisezmoi.txt".to_string(),
        "The Matrix (1999)/poster.jpg".to_string(),
    ]
    .into_iter()
    .collect();
    let obtenu: Vec<String> = instantane(src).into_keys().collect();
    assert_eq!(obtenu, attendu);
    // Contenus conservés, copie identique à l'original.
    let s = instantane(src);
    assert_eq!(
        s[&format!("The Matrix (1999)/{B}.mkv")],
        "Matrix.1999/Matrix.1999.mkv"
    );
    assert_eq!(s["The Matrix (1999)/poster.jpg"], "Matrix.1999/folder.jpg");
    assert_eq!(
        s[&format!("The Matrix (1999)/{B}-poster.jpg")],
        "Matrix.1999/folder.jpg"
    );
    assert_eq!(
        s[&format!("The Matrix (1999)/{B}-fanart.jpg")],
        "Matrix.1999/fanart.jpg"
    );
    // Historique et mise à jour du modèle.
    match &rapport.resultats[0].statut {
        StatutElement::Reussi { historique } => {
            assert_eq!(historique.nouveau_chemin, src.join("The Matrix (1999)"));
            assert!(
                historique
                    .fichiers
                    .contains(&("Matrix.1999.mkv".to_string(), format!("{B}.mkv")))
            );
        }
        s => panic!("{s:?}"),
    }
    appliquer_au_film(&mut f, &plan.elements[0]);
    for fm in &f.fichiers {
        assert!(
            fm.chemin_complet().exists(),
            "{}",
            fm.chemin_complet().display()
        );
    }
    // Un second passage ne change plus rien.
    let plan2 = planifier(&[&f], &profil);
    assert!(plan2.est_vide(), "{:?}", plan2.elements[0].operations);
}

#[test]
fn corbeille_conserve_les_supprimes() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path();
    let f = film_complet(src);
    let plan = planifier(&[&f], &ProfilRenommageFilm::default());
    let options = OptionsExecution {
        corbeille: true,
        ..Default::default()
    };
    let rapport = executer(&plan, &options);
    assert!(rapport.est_complet(), "{rapport:?}");
    let s = instantane(src);
    assert!(
        s.contains_key(&format!(
            "{DOSSIER_CORBEILLE}/The Matrix (1999)/vieux-fanart.jpg"
        )),
        "{s:?}"
    );
    // Aucun dossier de travail résiduel.
    assert!(!s.keys().any(|k| k.contains(".renommage-en-cours")));
}

#[test]
fn simulation_sans_effet() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path();
    let f = film_complet(src);
    let avant = instantane(src);
    let plan = planifier(&[&f], &ProfilRenommageFilm::default());
    let rapport = executer(
        &plan,
        &OptionsExecution {
            simulation: true,
            ..Default::default()
        },
    );
    assert!(matches!(
        rapport.resultats[0].statut,
        StatutElement::Reussi { .. }
    ));
    assert_eq!(instantane(src), avant);
}

#[test]
fn conflit_apparu_apres_planification_retour_arriere() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path();
    let f = film_complet(src);
    let plan = planifier(&[&f], &ProfilRenommageFilm::default());
    assert!(!plan.a_des_conflits());
    let avant = instantane(src);
    // Le dossier ne bloque pas (déplacé d'abord), mais un fichier occupe
    // désormais la destination du sous-titre dans le dossier source : après
    // le déplacement du dossier, il se retrouve à la place visée.
    fs::write(src.join(format!("Matrix.1999/{B}.fra.srt")), "intrus").unwrap();
    let avant_intrus = instantane(src);
    let rapport = executer(&plan, &OptionsExecution::default());
    match &rapport.resultats[0].statut {
        StatutElement::Echoue {
            erreur,
            retour_complet,
            erreurs_retour,
        } => {
            assert!(retour_complet, "{erreurs_retour:?}");
            assert!(erreur.message.contains("existe déjà"), "{erreur}");
        }
        s => panic!("{s:?}"),
    }
    assert_eq!(instantane(src), avant_intrus, "état initial restauré");
    assert_ne!(avant, avant_intrus);
    assert_eq!(rapport.nombre_echecs(), 1);
}

#[test]
fn dossier_cree_apres_planification() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path();
    let f = film_complet(src);
    let plan = planifier(&[&f], &ProfilRenommageFilm::default());
    creer(src, &["The Matrix (1999)/autre.mkv"]);
    let avant = instantane(src);
    let rapport = executer(&plan, &OptionsExecution::default());
    assert!(matches!(
        rapport.resultats[0].statut,
        StatutElement::Echoue {
            retour_complet: true,
            ..
        }
    ));
    assert_eq!(instantane(src), avant);
}

#[test]
fn echec_tardif_annule_copies_et_suppressions() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path();
    creer(src, &["F/a.mkv", "F/poster.jpg", "F/vieux.jpg"]);
    let f = film_disque(
        src,
        "F",
        &[
            ("a.mkv", TypeFichier::Video),
            ("poster.jpg", TypeFichier::Poster),
            ("vieux.jpg", TypeFichier::Poster),
        ],
    );
    let mut profil = ProfilRenommageFilm {
        dossier_actif: false,
        ..Default::default()
    };
    profil.noms.poster = vec!["<b>-poster".into(), "poster".into(), "folder".into()];
    let plan = planifier(&[&f], &profil);
    assert!(!plan.a_des_conflits(), "{:?}", plan.conflits());
    // La dernière opération de transfert échouera : on occupe sa cible.
    let derniere = plan.elements[0]
        .operations
        .iter()
        .rev()
        .find_map(|o| match o {
            Operation::Deplacer { vers, .. } => Some(vers.clone()),
            _ => None,
        })
        .unwrap();
    fs::write(&derniere, "intrus").unwrap();
    let avant = instantane(src);
    let rapport = executer(&plan, &OptionsExecution::default());
    assert!(
        matches!(
            rapport.resultats[0].statut,
            StatutElement::Echoue {
                retour_complet: true,
                ..
            }
        ),
        "{rapport:?}"
    );
    assert_eq!(instantane(src), avant);
}

#[test]
fn plan_en_conflit_ignore() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path();
    let f = film_complet(src);
    creer(src, &["The Matrix (1999)/x.mkv"]);
    let plan = planifier(&[&f], &ProfilRenommageFilm::default());
    assert!(plan.a_des_conflits());
    let avant = instantane(src);
    let rapport = executer(&plan, &OptionsExecution::default());
    assert!(matches!(
        rapport.resultats[0].statut,
        StatutElement::Ignore { .. }
    ));
    assert_eq!(instantane(src), avant);
}

#[test]
fn deplacement_inter_volumes_simule() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path();
    let f = film_complet(src);
    let plan = planifier(&[&f], &ProfilRenommageFilm::default());
    let rapport = executer(
        &plan,
        &OptionsExecution {
            forcer_copie: true,
            ..Default::default()
        },
    );
    assert!(rapport.est_complet(), "{rapport:?}");
    let s = instantane(src);
    assert_eq!(
        s[&format!("The Matrix (1999)/{B}.mkv")],
        "Matrix.1999/Matrix.1999.mkv"
    );
    assert!(!src.join("Matrix.1999").exists());
}

#[test]
fn primitives_sans_ecrasement() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    creer(d, &["a", "b", "rep/x", "rep2/y"]);
    assert!(deplacer_fichier(&d.join("a"), &d.join("b"), false).is_err());
    assert!(deplacer_fichier(&d.join("a"), &d.join("b"), true).is_err());
    assert!(copier_fichier(&d.join("a"), &d.join("b")).is_err());
    assert!(deplacer_dossier(&d.join("rep"), &d.join("rep2"), false).is_err());
    assert_eq!(fs::read_to_string(d.join("b")).unwrap(), "b");
    assert_eq!(fs::read_to_string(d.join("a")).unwrap(), "a");
    // Copie forcée d'un dossier (voie inter-volumes).
    deplacer_dossier(&d.join("rep"), &d.join("rep3"), true).unwrap();
    assert_eq!(fs::read_to_string(d.join("rep3/x")).unwrap(), "rep/x");
    assert!(!d.join("rep").exists());
    // Renommage de casse seule.
    deplacer_fichier(&d.join("a"), &d.join("A"), false).unwrap();
    assert_eq!(fs::read_to_string(d.join("A")).unwrap(), "a");
}

#[test]
fn multi_films_vers_dossiers_propres() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path();
    creer(
        src,
        &[
            "Matrix.1999.mkv",
            "Matrix.1999-poster.jpg",
            "Alien.1979.mkv",
        ],
    );
    let mut matrix = film_disque(
        src,
        "",
        &[
            ("Matrix.1999.mkv", TypeFichier::Video),
            ("Matrix.1999-poster.jpg", TypeFichier::Poster),
        ],
    );
    matrix.chemin = src.to_path_buf();
    matrix.dossier_multi_films = true;
    let mut alien = film_disque(src, "", &[("Alien.1979.mkv", TypeFichier::Video)]);
    alien.id = "alien".into();
    alien.titre = "Alien".into();
    alien.annee = 1979;
    alien.chemin = src.to_path_buf();
    alien.dossier_multi_films = true;
    let plan = planifier(&[&matrix, &alien], &ProfilRenommageFilm::default());
    assert!(!plan.a_des_conflits(), "{:?}", plan.conflits());
    let rapport = executer(&plan, &OptionsExecution::default());
    assert!(rapport.est_complet(), "{rapport:?}");
    let s: Vec<String> = instantane(src).into_keys().collect();
    assert_eq!(
        s,
        vec![
            "Alien (1979)/".to_string(),
            "Alien (1979)/Alien (1979) 1080p AC3.mkv".to_string(),
            "The Matrix (1999)/".to_string(),
            format!("The Matrix (1999)/{B}-poster.jpg"),
            format!("The Matrix (1999)/{B}.mkv"),
        ]
    );
}

#[test]
fn dossiers_vides_et_indesirables() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path();
    creer(
        src,
        &[
            "a/b/Matrix/m.mkv",
            "a/b/Matrix/vide/.gitkeep-non",
            "a/b/Matrix/info.url",
        ],
    );
    fs::remove_file(src.join("a/b/Matrix/vide/.gitkeep-non")).unwrap();
    let f = film_disque(src, "a/b/Matrix", &[("m.mkv", TypeFichier::Video)]);
    let profil = ProfilRenommageFilm {
        nettoyer_indesirables: true,
        ..Default::default()
    };
    let plan = planifier(&[&f], &profil);
    let rapport = executer(&plan, &OptionsExecution::default());
    assert!(rapport.est_complet(), "{rapport:?}");
    let s: Vec<String> = instantane(src).into_keys().collect();
    assert_eq!(
        s,
        vec![
            "The Matrix (1999)/".to_string(),
            format!("The Matrix (1999)/{B}.mkv")
        ]
    );
}

#[test]
fn serie_complete_avec_dependances() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path();
    creer(
        src,
        &[
            "show/s01e01.mkv",
            "show/s01e01.en.srt",
            "show/s01e02.mkv",
            "show/tvshow.nfo",
        ],
    );
    let dossier = src.join("show");
    let f = |n: &str, t| FichierMedia::nouveau(&dossier.join(n), t);
    let mut s = serie(vec![
        episode(
            "1",
            1,
            1,
            "Pilot",
            vec![
                f("s01e01.mkv", TypeFichier::Video),
                f("s01e01.en.srt", TypeFichier::SousTitres),
            ],
        ),
        episode(
            "2",
            1,
            2,
            "Second",
            vec![f("s01e02.mkv", TypeFichier::Video)],
        ),
    ]);
    s.source_donnees = src.to_path_buf();
    s.chemin = dossier.clone();
    s.fichiers.push(f("tvshow.nfo", TypeFichier::Nfo));
    let plan = planifier_serie(&s, &ProfilRenommageSerie::default(), &SondeSystemeFichiers);
    assert!(!plan.a_des_conflits(), "{:?}", plan.conflits());
    let rapport = executer(&plan, &OptionsExecution::default());
    assert!(rapport.est_complet(), "{rapport:?}");
    let k: Vec<String> = instantane(src).into_keys().collect();
    assert_eq!(
        k,
        vec![
            "Show (2008)/",
            "Show (2008)/Season 1/",
            "Show (2008)/Season 1/Show - S01E01 - Pilot.eng.srt",
            "Show (2008)/Season 1/Show - S01E01 - Pilot.mkv",
            "Show (2008)/Season 1/Show - S01E02 - Second.mkv",
            "Show (2008)/tvshow.nfo",
        ]
    );
    let reussis = elements_reussis(&rapport, plan.elements.len());
    appliquer_aux_episodes(&mut s, &plan, &reussis);
    assert_eq!(s.chemin, src.join("Show (2008)"));
    for e in &s.episodes {
        for fm in &e.fichiers {
            assert!(
                fm.chemin_complet().exists(),
                "{}",
                fm.chemin_complet().display()
            );
        }
    }
}

#[test]
fn echec_serie_bloque_les_episodes() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path();
    creer(src, &["show/s01e01.mkv"]);
    let dossier = src.join("show");
    let mut s = serie(vec![episode(
        "1",
        1,
        1,
        "Pilot",
        vec![FichierMedia::nouveau(
            &dossier.join("s01e01.mkv"),
            TypeFichier::Video,
        )],
    )]);
    s.source_donnees = src.to_path_buf();
    s.chemin = dossier;
    let plan = planifier_serie(&s, &ProfilRenommageSerie::default(), &SondeSystemeFichiers);
    creer(src, &["Show (2008)/occupe"]);
    let avant = instantane(src);
    let rapport = executer(&plan, &OptionsExecution::default());
    assert!(matches!(
        rapport.resultats[0].statut,
        StatutElement::Echoue { .. }
    ));
    assert!(matches!(
        rapport.resultats[1].statut,
        StatutElement::Ignore { .. }
    ));
    assert_eq!(instantane(src), avant);
}
