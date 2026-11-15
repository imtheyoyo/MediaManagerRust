//! Tests de la couche de stockage (dossiers temporaires).

use std::fs;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use mediamanager_core::modele::{Episode, Film, Saga, Serie};
use tempfile::TempDir;

use crate::base::{Bibliotheque, NOM_BASE, VERSION_SCHEMA};
use crate::dossiers::{
    DossiersApplication, MARQUEUR_NON_PORTABLE, MARQUEUR_PORTABLE, ModeDossiers,
};
use crate::erreur::ErreurStockage;
use crate::reglages::{GestionnaireReglages, Reglages, TailleImage, noms_secrets};
use crate::sauvegarde::{self, EtatOuverture, EtatSauvegarde};
use crate::secrets::{CoffreFichier, CoffreMemoire, CoffreSecrets};
use crate::verrou::VerrouInstance;
use crate::{RapportDemarrage, Stockage};

fn film(titre: &str, annee: u32, chemin: &str, tmdb: &str) -> Film {
    let mut f = Film {
        titre: titre.into(),
        annee,
        chemin: PathBuf::from(chemin),
        ..Film::default()
    };
    f.ids.definir("tmdb", tmdb);
    f
}

fn dossiers_temp() -> (TempDir, DossiersApplication) {
    let t = tempfile::tempdir().unwrap();
    let d = DossiersApplication::depuis_racine(t.path(), ModeDossiers::Explicite);
    d.creer().unwrap();
    (t, d)
}

/// Écrase un fichier par des octets qui ne sont pas une base SQLite.
fn corrompre(chemin: &Path) {
    fs::write(chemin, vec![0x42u8; 8192]).unwrap();
    for s in ["-wal", "-shm"] {
        let _ = fs::remove_file(format!("{}{s}", chemin.display()));
    }
}

// ---------------------------------------------------------------------------
// Base : CRUD
// ---------------------------------------------------------------------------

#[test]
fn crud_films() {
    let mut b = Bibliotheque::ouvrir_en_memoire().unwrap();
    let mut matrix = film("Matrix", 1999, "/films/Matrix (1999)", "603");
    matrix.ids.definir("imdb", "tt0133093");
    assert!(b.enregistrer(&mut matrix).unwrap());
    assert_eq!(matrix.id.len(), 36, "UUID attribué");

    let lu: Film = b.obtenir(&matrix.id).unwrap().unwrap();
    assert_eq!(lu, matrix);

    // Document identique : aucune écriture.
    assert!(!b.enregistrer(&mut matrix).unwrap());
    matrix.titre_original = "The Matrix".into();
    assert!(b.enregistrer(&mut matrix).unwrap());

    let mut alien = film("alien", 1979, "/films/Alien (1979)", "348");
    b.enregistrer(&mut alien).unwrap();
    let titres: Vec<String> = b
        .lister::<Film>()
        .unwrap()
        .into_iter()
        .map(|f| f.titre)
        .collect();
    assert_eq!(
        titres,
        ["alien", "Matrix"],
        "tri par titre insensible à la casse"
    );
    assert_eq!(b.compter::<Film>().unwrap(), 2);

    // Recherches.
    let r = b
        .chercher_par_chemin::<Film>(Path::new("/films/Matrix (1999)"))
        .unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].id, matrix.id);
    assert_eq!(
        b.chercher_par_id_externe::<Film>("tmdb", "603")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        b.chercher_par_id_externe::<Film>("imdb", "tt0133093")
            .unwrap()
            .len(),
        1
    );
    assert!(
        b.chercher_par_id_externe::<Film>("tmdb", "999")
            .unwrap()
            .is_empty()
    );
    assert_eq!(b.chercher_par_titre::<Film>("atri").unwrap().len(), 1);
    assert!(
        b.chercher_par_titre::<Film>("%").unwrap().is_empty(),
        "joker échappé"
    );

    // L'index suit les modifications d'identifiants.
    matrix.ids.definir("tmdb", "604");
    b.enregistrer(&mut matrix).unwrap();
    assert!(
        b.chercher_par_id_externe::<Film>("tmdb", "603")
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        b.chercher_par_id_externe::<Film>("tmdb", "604")
            .unwrap()
            .len(),
        1
    );

    // Suppression.
    assert!(b.supprimer::<Film>(&matrix.id).unwrap());
    assert!(!b.supprimer::<Film>(&matrix.id).unwrap());
    assert!(b.obtenir::<Film>(&matrix.id).unwrap().is_none());
    assert!(
        b.chercher_par_id_externe::<Film>("tmdb", "604")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn lots_et_sagas() {
    let mut b = Bibliotheque::ouvrir_en_memoire().unwrap();
    let mut films: Vec<Film> = (1..=3)
        .map(|i| {
            film(
                &format!("Film {i}"),
                2000 + i,
                &format!("/f/{i}"),
                &i.to_string(),
            )
        })
        .collect();
    assert_eq!(b.enregistrer_lot(&mut films).unwrap(), 3);
    assert_eq!(b.enregistrer_lot(&mut films).unwrap(), 0, "rien n'a changé");

    // Lot mixte validé.
    let mut saga = Saga {
        titre: "Trilogie".into(),
        films: films.iter().map(|f| f.id.clone()).collect(),
        ..Saga::default()
    };
    saga.ids.definir("tmdbSet", "2344");
    {
        let mut lot = b.lot().unwrap();
        lot.enregistrer(&mut saga).unwrap();
        for f in &mut films {
            f.saga_id = Some(saga.id.clone());
            lot.enregistrer(f).unwrap();
        }
        lot.valider().unwrap();
    }
    let s: Saga = b.obtenir(&saga.id).unwrap().unwrap();
    assert_eq!(s.films.len(), 3);
    assert_eq!(
        b.chercher_par_id_externe::<Saga>("tmdbSet", "2344")
            .unwrap()
            .len(),
        1
    );
    let f: Film = b.obtenir(&films[0].id).unwrap().unwrap();
    assert_eq!(f.saga_id.as_deref(), Some(saga.id.as_str()));

    // Lot abandonné (détruit sans validation) : aucun effet.
    {
        let mut lot = b.lot().unwrap();
        lot.supprimer::<Saga>(&saga.id).unwrap();
        let mut f4 = film("Film 4", 2004, "/f/4", "4");
        lot.enregistrer(&mut f4).unwrap();
    }
    assert!(b.obtenir::<Saga>(&saga.id).unwrap().is_some());
    assert_eq!(b.compter::<Film>().unwrap(), 3);
}

#[test]
fn series_avec_episodes() {
    let mut b = Bibliotheque::ouvrir_en_memoire().unwrap();
    let mut serie = Serie {
        titre: "Dark".into(),
        annee: 2017,
        chemin: "/series/Dark".into(),
        ..Serie::default()
    };
    serie.ids.definir("tvdb", "334824");
    for n in 1..=2 {
        let mut ep = Episode {
            titre: format!("Épisode {n}"),
            ..Episode::default()
        };
        ep.ids.definir("tvdb", &format!("60000{n}"));
        serie.episodes.push(ep);
    }
    b.enregistrer(&mut serie).unwrap();
    assert!(
        serie
            .episodes
            .iter()
            .all(|e| !e.id.is_empty() && e.id_serie == serie.id)
    );

    let lue: Serie = b.obtenir(&serie.id).unwrap().unwrap();
    assert_eq!(lue.episodes.len(), 2);
    assert_eq!(
        b.chercher_par_id_externe::<Serie>("tvdb", "334824")
            .unwrap()
            .len(),
        1
    );
    // Un identifiant d'épisode ne fait pas remonter la série par cette voie.
    assert!(
        b.chercher_par_id_externe::<Serie>("tvdb", "600001")
            .unwrap()
            .is_empty()
    );

    let refs = b
        .chercher_episodes_par_id_externe("tvdb", "600002")
        .unwrap();
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].serie_id, serie.id);
    assert_eq!(refs[0].episode_id, serie.episodes[1].id);

    b.supprimer::<Serie>(&serie.id).unwrap();
    assert!(
        b.chercher_episodes_par_id_externe("tvdb", "600002")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn persistance_et_valeurs() {
    let t = tempfile::tempdir().unwrap();
    let chemin = t.path().join(NOM_BASE);
    let id = {
        let mut b = Bibliotheque::ouvrir(&chemin).unwrap();
        let mut f = film("Persistant", 2001, "/p", "1");
        b.enregistrer(&mut f).unwrap();
        b.ecrire_valeur("maj.derniere", "123").unwrap();
        b.fermer().unwrap();
        f.id
    };
    let mut b = Bibliotheque::ouvrir(&chemin).unwrap();
    assert_eq!(b.obtenir::<Film>(&id).unwrap().unwrap().titre, "Persistant");
    assert_eq!(
        b.lire_valeur("maj.derniere").unwrap().as_deref(),
        Some("123")
    );
    assert!(b.supprimer_valeur("maj.derniere").unwrap());
    assert!(b.lire_valeur("maj.derniere").unwrap().is_none());
}

// ---------------------------------------------------------------------------
// Base : migrations et corruption
// ---------------------------------------------------------------------------

#[test]
fn migration_depuis_version_1() {
    let t = tempfile::tempdir().unwrap();
    let chemin = t.path().join(NOM_BASE);
    {
        let mut b = Bibliotheque::ouvrir_brut(&chemin).unwrap();
        b.migrer_jusqua(1).unwrap();
        assert_eq!(b.version_schema().unwrap(), 1);
        // Document écrit par la version 1 (pas d'index d'identifiants).
        let f = film("Ancien", 1990, "/a", "42");
        b.connexion()
            .execute(
                "INSERT INTO films (id, titre, annee, chemin, json) VALUES ('id-1', 'Ancien', 1990, '/a', ?1)",
                [serde_json::to_string(&f).unwrap()],
            )
            .unwrap();
    }
    let b = Bibliotheque::ouvrir(&chemin).unwrap();
    assert_eq!(b.version_schema().unwrap(), VERSION_SCHEMA);
    let r = b.chercher_par_id_externe::<Film>("tmdb", "42").unwrap();
    assert_eq!(r.len(), 1, "la migration 2 indexe les documents existants");
    assert_eq!(r[0].id, "id-1");
    drop(b);
    // Réouverture : aucune migration rejouée.
    let b = Bibliotheque::ouvrir(&chemin).unwrap();
    assert_eq!(b.compter::<Film>().unwrap(), 1);
}

#[test]
fn schema_trop_recent_refuse() {
    let t = tempfile::tempdir().unwrap();
    let chemin = t.path().join(NOM_BASE);
    {
        let b = Bibliotheque::ouvrir(&chemin).unwrap();
        b.connexion()
            .pragma_update(None, "user_version", 999)
            .unwrap();
    }
    match Bibliotheque::ouvrir(&chemin) {
        Err(ErreurStockage::SchemaTropRecent { trouvee: 999, .. }) => {}
        autre => panic!("attendu SchemaTropRecent, obtenu {autre:?}"),
    }
}

#[test]
fn base_corrompue_detectee() {
    let t = tempfile::tempdir().unwrap();
    let chemin = t.path().join(NOM_BASE);
    corrompre(&chemin);
    assert!(matches!(
        Bibliotheque::ouvrir(&chemin),
        Err(ErreurStockage::BaseCorrompue(_))
    ));
}

// ---------------------------------------------------------------------------
// Réglages
// ---------------------------------------------------------------------------

fn gestionnaire(chemin: &Path) -> GestionnaireReglages {
    GestionnaireReglages::new(chemin, Box::new(CoffreMemoire::default()))
}

fn fichiers_du_dossier(d: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(d)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn reglages_premier_lancement_et_aller_retour() {
    let t = tempfile::tempdir().unwrap();
    let chemin = t.path().join("reglages.json");
    let mut g = gestionnaire(&chemin);
    let c = g.charger().unwrap();
    assert!(c.nouveau);
    assert!(c.fichier_corrompu.is_none());
    assert!(chemin.is_file(), "défauts écrits");
    let mut r = c.reglages;
    assert!(!r.general.ignorer_erreurs_ssl, "TLS validé par défaut");
    assert!(
        r.general.cle_api_tmdb.is_empty(),
        "aucune clé fournie par défaut"
    );
    assert_eq!(r.films.scraper, "tmdb");
    assert_eq!(r.series.scraper, "tvdb");
    assert!(r.series.duree_depuis_analyse);
    assert_eq!(r.films.seuil_scraper, 0.75);
    assert_eq!(
        r.general.cle_api_http.len(),
        36,
        "clé de l'API locale générée"
    );
    assert_eq!(
        g.coffre()
            .lire(noms_secrets::CLE_API_HTTP)
            .unwrap()
            .as_deref(),
        Some(r.general.cle_api_http.as_str())
    );

    // Modification, secret compris.
    r.general.cle_api_tmdb = "cle-utilisateur".into();
    r.films.sources.push("/mnt/films".into());
    r.films.taille_affiche = TailleImage::Originale;
    assert!(g.enregistrer_si_modifie(&r).unwrap());
    assert!(!g.enregistrer_si_modifie(&r).unwrap(), "rien à écrire");

    let texte = fs::read_to_string(&chemin).unwrap();
    assert!(
        !texte.contains("cle-utilisateur"),
        "secret absent du fichier"
    );
    assert!(!texte.contains(&r.general.cle_api_http));
    assert_eq!(
        fichiers_du_dossier(t.path()),
        ["reglages.json"],
        "aucun temporaire"
    );

    // Relecture avec le même coffre.
    let c2 = g.charger().unwrap();
    assert!(!c2.nouveau);
    assert_eq!(c2.reglages, r);
}

#[test]
fn reglages_corrompus_mis_de_cote() {
    let t = tempfile::tempdir().unwrap();
    let chemin = t.path().join("reglages.json");
    fs::write(&chemin, "{ ceci n'est pas du JSON").unwrap();
    let mut g = gestionnaire(&chemin);
    let c = g.charger().unwrap();
    assert!(c.nouveau);
    let copie = c.fichier_corrompu.expect("copie conservée");
    assert!(copie.to_string_lossy().ends_with(".corrompu"));
    assert_eq!(
        fs::read_to_string(&copie).unwrap(),
        "{ ceci n'est pas du JSON"
    );
    // Le nouveau fichier est valide et contient les défauts.
    let relu: Reglages = serde_json::from_str(&fs::read_to_string(&chemin).unwrap()).unwrap();
    assert_eq!(relu.films.scraper, "tmdb");
}

#[test]
fn reglages_tolerants() {
    let t = tempfile::tempdir().unwrap();
    let chemin = t.path().join("reglages.json");
    fs::write(
        &chemin,
        r#"{
            "version": 1,
            "general": { "langue": "de", "cle_api_tmdb": "en-clair" },
            "films": { "taille_affiche": "gigantesque" },
            "champ_futur": { "x": 1 }
        }"#,
    )
    .unwrap();
    let mut g = gestionnaire(&chemin);
    let c = g.charger().unwrap();
    assert!(!c.nouveau && c.fichier_corrompu.is_none());
    let r = c.reglages;
    assert_eq!(r.general.langue, "de");
    assert_eq!(r.general.taille_police, 12, "champ manquant => défaut");
    assert_eq!(
        r.films.taille_affiche,
        TailleImage::Grande,
        "valeur inconnue => défaut"
    );
    assert_eq!(r.general.cle_api_tmdb, "en-clair");
    // Secret en clair migré vers le coffre et retiré du fichier.
    assert_eq!(
        g.coffre()
            .lire(noms_secrets::CLE_API_TMDB)
            .unwrap()
            .as_deref(),
        Some("en-clair")
    );
    let texte = fs::read_to_string(&chemin).unwrap();
    assert!(!texte.contains("en-clair"));
    // Champ inconnu conservé.
    assert!(texte.contains("champ_futur"));
}

#[test]
fn reglages_echec_ecriture_retente() {
    let t = tempfile::tempdir().unwrap();
    // Le parent du fichier est un fichier ordinaire : l'écriture échoue.
    let obstacle = t.path().join("sous");
    fs::write(&obstacle, "x").unwrap();
    let chemin = obstacle.join("reglages.json");
    let mut g = gestionnaire(&chemin);
    let r = Reglages::default();
    assert!(g.enregistrer_si_modifie(&r).is_err());
    fs::remove_file(&obstacle).unwrap();
    assert!(
        g.enregistrer_si_modifie(&r).unwrap(),
        "l'échec précédent n'a pas marqué les réglages comme enregistrés"
    );
    assert!(chemin.is_file());
}

#[test]
fn coffre_fichier() {
    let t = tempfile::tempdir().unwrap();
    let chemin = t.path().join("secrets.json");
    let c = CoffreFichier::new(&chemin);
    assert!(c.lire("a").unwrap().is_none());
    c.ecrire("a", "valeur").unwrap();
    assert_eq!(
        CoffreFichier::new(&chemin).lire("a").unwrap().as_deref(),
        Some("valeur")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&chemin).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
    c.ecrire("a", "").unwrap();
    assert!(c.lire("a").unwrap().is_none(), "valeur vide => suppression");
}

// ---------------------------------------------------------------------------
// Sauvegardes
// ---------------------------------------------------------------------------

fn entrees_zip(archive: &Path) -> Vec<String> {
    let mut z = zip::ZipArchive::new(fs::File::open(archive).unwrap()).unwrap();
    let mut v: Vec<String> = (0..z.len())
        .map(|i| z.by_index(i).unwrap().name().to_string())
        .collect();
    v.sort();
    v
}

fn jour(j: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, j).unwrap()
}

#[test]
fn sauvegarde_contenu_et_rotation() {
    let (_t, d) = dossiers_temp();
    // Base ouverte pendant la sauvegarde (mode WAL).
    let mut b = Bibliotheque::ouvrir(&d.base()).unwrap();
    b.enregistrer(&mut film("Sauvé", 2000, "/s", "7")).unwrap();
    fs::write(d.fichier_reglages(), "{}").unwrap();
    fs::write(d.donnees.join("secrets.json"), "{}").unwrap();
    let _verrou = VerrouInstance::acquerir(&d.donnees).unwrap();

    for j in 1..=7 {
        let etat = sauvegarde::creer_sauvegarde(&d, jour(j)).unwrap();
        assert!(matches!(etat, EtatSauvegarde::Creee(_)));
    }
    let toutes = sauvegarde::lister_sauvegardes(&d.sauvegardes).unwrap();
    assert_eq!(toutes.len(), 7);
    let supprimees = sauvegarde::rotation(&d.sauvegardes, 5).unwrap();
    assert_eq!(supprimees.len(), 2);
    let restantes: Vec<String> = sauvegarde::lister_sauvegardes(&d.sauvegardes)
        .unwrap()
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        restantes,
        [
            "data.2026-11-07.zip",
            "data.2026-11-06.zip",
            "data.2026-11-05.zip",
            "data.2026-11-04.zip",
            "data.2026-11-03.zip"
        ]
    );
    // Seuls les fichiers attendus (pas de copie ni de temporaire résiduels).
    assert_eq!(fichiers_du_dossier(&d.sauvegardes).len(), 5);

    let entrees = entrees_zip(&d.sauvegardes.join("data.2026-11-07.zip"));
    assert_eq!(entrees, ["data/bibliotheque.db", "data/reglages.json"]);

    // La copie de la base dans l'archive est lisible et complète.
    let cible = d.racine.join("extrait.db");
    let mut z = zip::ZipArchive::new(fs::File::open(restantes_chemin(&d, 0)).unwrap()).unwrap();
    std::io::copy(
        &mut z.by_name("data/bibliotheque.db").unwrap(),
        &mut fs::File::create(&cible).unwrap(),
    )
    .unwrap();
    let copie = Bibliotheque::ouvrir(&cible).unwrap();
    assert_eq!(copie.compter::<Film>().unwrap(), 1);
}

fn restantes_chemin(d: &DossiersApplication, i: usize) -> PathBuf {
    sauvegarde::lister_sauvegardes(&d.sauvegardes).unwrap()[i].clone()
}

#[test]
fn sauvegarde_refusee_si_base_corrompue() {
    let (_t, d) = dossiers_temp();
    Bibliotheque::ouvrir(&d.base()).unwrap().fermer().unwrap();
    let EtatSauvegarde::Creee(archive) = sauvegarde::creer_sauvegarde(&d, jour(15)).unwrap() else {
        panic!("sauvegarde attendue");
    };
    let avant = fs::read(&archive).unwrap();
    corrompre(&d.base());
    let etat = sauvegarde::creer_sauvegarde(&d, jour(15)).unwrap();
    assert!(matches!(etat, EtatSauvegarde::BaseCorrompue(_)));
    assert_eq!(
        fs::read(&archive).unwrap(),
        avant,
        "archive du jour préservée"
    );
}

#[test]
fn sauvegarde_sans_dossier_donnees() {
    let t = tempfile::tempdir().unwrap();
    let d = DossiersApplication::depuis_racine(t.path(), ModeDossiers::Explicite);
    assert_eq!(
        sauvegarde::creer_sauvegarde(&d, jour(1)).unwrap(),
        EtatSauvegarde::RienASauvegarder
    );
}

#[test]
fn restauration_automatique() {
    let (_t, d) = dossiers_temp();
    let id = {
        let mut b = Bibliotheque::ouvrir(&d.base()).unwrap();
        let mut f = film("Précieux", 1980, "/p", "9");
        b.enregistrer(&mut f).unwrap();
        b.fermer().unwrap();
        f.id
    };
    sauvegarde::creer_sauvegarde(&d, jour(10)).unwrap();
    corrompre(&d.base());

    let (b, etat) = sauvegarde::ouvrir_ou_restaurer(&d).unwrap();
    let EtatOuverture::Restauree {
        archive,
        base_corrompue,
    } = etat
    else {
        panic!("restauration attendue, obtenu {etat:?}");
    };
    assert!(archive.ends_with("data.2026-11-10.zip"));
    assert!(base_corrompue.is_file());
    assert!(base_corrompue.to_string_lossy().ends_with(".corrompu"));
    assert_eq!(b.obtenir::<Film>(&id).unwrap().unwrap().titre, "Précieux");
}

#[test]
fn restauration_impossible_base_vide() {
    let (_t, d) = dossiers_temp();
    corrompre(&d.base());
    let (b, etat) = sauvegarde::ouvrir_ou_restaurer(&d).unwrap();
    assert!(matches!(etat, EtatOuverture::Reinitialisee { .. }));
    assert_eq!(b.compter::<Film>().unwrap(), 0);
}

#[test]
fn restauration_manuelle() {
    let (_t, d) = dossiers_temp();
    {
        let mut b = Bibliotheque::ouvrir(&d.base()).unwrap();
        b.enregistrer(&mut film("Avant", 1, "/a", "1")).unwrap();
        b.fermer().unwrap();
    }
    fs::write(d.fichier_reglages(), r#"{"version":1}"#).unwrap();
    let EtatSauvegarde::Creee(archive) = sauvegarde::creer_sauvegarde(&d, jour(3)).unwrap() else {
        panic!()
    };
    {
        let mut b = Bibliotheque::ouvrir(&d.base()).unwrap();
        b.enregistrer(&mut film("Après", 2, "/b", "2")).unwrap();
        b.fermer().unwrap();
    }
    fs::write(d.fichier_reglages(), "modifié").unwrap();

    assert_eq!(sauvegarde::restaurer_archive(&archive, &d).unwrap(), 2);
    assert_eq!(
        fs::read_to_string(d.fichier_reglages()).unwrap(),
        r#"{"version":1}"#
    );
    let b = Bibliotheque::ouvrir(&d.base()).unwrap();
    let titres: Vec<String> = b
        .lister::<Film>()
        .unwrap()
        .into_iter()
        .map(|f| f.titre)
        .collect();
    assert_eq!(titres, ["Avant"]);
}

// ---------------------------------------------------------------------------
// Verrou et dossiers
// ---------------------------------------------------------------------------

#[test]
fn verrou_instance_unique() {
    let t = tempfile::tempdir().unwrap();
    let v1 = VerrouInstance::acquerir(t.path()).unwrap();
    assert!(matches!(
        VerrouInstance::acquerir(t.path()),
        Err(ErreurStockage::InstanceDejaLancee(_))
    ));
    drop(v1);
    let _v2 = VerrouInstance::acquerir(t.path()).expect("verrou libéré");
}

#[test]
fn resolution_des_dossiers() {
    let t = tempfile::tempdir().unwrap();
    let forcee = DossiersApplication::resoudre_avec(Some(t.path().join("x")), None).unwrap();
    assert_eq!(forcee.mode, ModeDossiers::Explicite);
    assert_eq!(forcee.donnees, t.path().join("x").join("data"));

    // Marqueur portable dans un dossier inscriptible.
    fs::write(t.path().join(MARQUEUR_PORTABLE), "").unwrap();
    let p = DossiersApplication::resoudre_avec(None, Some(t.path())).unwrap();
    assert_eq!(p.mode, ModeDossiers::Portable);
    assert_eq!(p.sauvegardes, t.path().join("backup"));
    assert_eq!(p.journaux, t.path().join("logs"));
    assert_eq!(p.cache, t.path().join("cache"));

    // Sans marqueur : mode portable par défaut (dossier inscriptible).
    let vide = tempfile::tempdir().unwrap();
    let p2 = DossiersApplication::resoudre_avec(None, Some(vide.path())).unwrap();
    assert_eq!(p2.mode, ModeDossiers::Portable);

    // Marqueur « non-portable » : dossiers utilisateur.
    fs::write(vide.path().join(MARQUEUR_NON_PORTABLE), "").unwrap();
    let u = DossiersApplication::resoudre_avec(None, Some(vide.path())).unwrap();
    assert_eq!(u.mode, ModeDossiers::Utilisateur);
    assert!(!u.racine.starts_with(vide.path()));
}

// ---------------------------------------------------------------------------
// Démarrage complet
// ---------------------------------------------------------------------------

#[test]
fn demarrage_complet() {
    let t = tempfile::tempdir().unwrap();
    let d = DossiersApplication::depuis_racine(t.path(), ModeDossiers::Explicite);
    let coffre = || Box::new(CoffreMemoire::default());

    let (mut s, rapport) = Stockage::demarrer_avec(d.clone(), coffre()).unwrap();
    assert_eq!(
        rapport,
        RapportDemarrage {
            sauvegarde: EtatSauvegarde::Creee(
                d.sauvegardes
                    .join(sauvegarde::nom_archive(chrono::Local::now().date_naive()))
            ),
            ouverture: EtatOuverture::Normale,
            reglages_nouveaux: true,
            reglages_corrompus: None,
        }
    );
    assert!(matches!(
        Stockage::demarrer_avec(d.clone(), coffre()),
        Err(ErreurStockage::InstanceDejaLancee(_))
    ));
    s.bibliotheque
        .enregistrer(&mut film("A", 1, "/a", "1"))
        .unwrap();
    s.reglages.general.langue = "en".into();
    s.fermer().unwrap();

    let (s2, r2) = Stockage::demarrer_avec(d, coffre()).unwrap();
    assert!(!r2.reglages_nouveaux);
    assert_eq!(s2.reglages.general.langue, "en");
    assert_eq!(s2.bibliotheque.compter::<Film>().unwrap(), 1);
}
