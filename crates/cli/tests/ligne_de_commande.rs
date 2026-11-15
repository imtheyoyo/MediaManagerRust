//! Tests d'intégration de l'outil `mediamanager` : dossier de données et
//! médiathèque factice (fichiers vidéo vides) temporaires, aucun accès réseau.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
use predicates::str::contains;
use tempfile::TempDir;

const CLE_FACTICE: &str = "cle-factice-0123456789";

struct Environnement {
    _tmp: TempDir,
    racine: PathBuf,
    films: PathBuf,
}

fn toucher(p: &Path) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, b"").unwrap();
}

impl Environnement {
    /// Dossier de données vide et médiathèque de trois films.
    fn nouveau() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let racine = tmp.path().join("app");
        let films = tmp.path().join("films");
        toucher(&films.join("Film A (2010)/Film.A.2010.mkv"));
        toucher(&films.join("Film.B.2011/Film.B.2011.mkv"));
        toucher(&films.join("La Grande Aventure (1999)/La Grande Aventure (1999).avi"));
        Self {
            _tmp: tmp,
            racine,
            films,
        }
    }

    fn cmd(&self) -> Command {
        let mut c = Command::cargo_bin("mediamanager").unwrap();
        c.env_remove("MEDIAMANAGER_DOSSIER")
            .env_remove("MEDIAMANAGER_CLE_TMDB")
            .env("HOME", self._tmp.path())
            .arg("--dossier")
            .arg(&self.racine);
        c
    }

    /// Ajoute la source de films et met la bibliothèque à jour.
    fn importer(&self) {
        self.cmd()
            .args(["sources", "ajouter"])
            .arg(&self.films)
            .assert()
            .success();
        self.cmd()
            .arg("maj")
            .assert()
            .success()
            .stdout(contains("3 ajouté(s)"));
    }

    fn json(&self, args: &[&str]) -> Vec<serde_json::Value> {
        let sortie = self
            .cmd()
            .arg("lister")
            .arg("--json")
            .args(args)
            .output()
            .unwrap();
        assert!(sortie.status.success());
        serde_json::from_slice::<serde_json::Value>(&sortie.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .clone()
    }

    fn id_de(&self, titre: &str) -> String {
        self.json(&[])
            .into_iter()
            .find(|f| f["titre"] == titre)
            .unwrap_or_else(|| panic!("film {titre} absent"))["id"]
            .as_str()
            .unwrap()
            .to_string()
    }
}

#[test]
fn aide_version_et_erreurs_d_arguments() {
    let e = Environnement::nouveau();
    e.cmd()
        .arg("--help")
        .assert()
        .code(0)
        .stdout(contains("Codes de sortie").and(contains("renommer")));
    e.cmd()
        .arg("--version")
        .assert()
        .code(0)
        .stdout(contains(env!("CARGO_PKG_VERSION")));
    // Erreur d'arguments : code 1 (2 est réservé aux conflits de renommage).
    e.cmd().arg("inconnue").assert().code(1);
    e.cmd()
        .args(["scraper", "--tous", "--nouveaux"])
        .assert()
        .code(1);
    // Rien n'a été créé pour une simple erreur d'arguments.
    assert!(!e.racine.exists());
}

#[test]
fn info_sur_dossier_neuf() {
    let e = Environnement::nouveau();
    e.cmd().arg("info").assert().success().stdout(
        contains("imposé")
            .and(contains(e.racine.join("data").display().to_string()))
            .and(contains("Films       : 0"))
            .and(contains("non configurée")),
    );
}

#[test]
fn gestion_des_sources() {
    let e = Environnement::nouveau();
    e.cmd()
        .args(["sources", "lister"])
        .assert()
        .success()
        .stdout(contains("Aucune source de films"));
    e.cmd()
        .args(["sources", "ajouter"])
        .arg(&e.films)
        .assert()
        .success()
        .stdout(contains("ajoutée"));
    e.cmd()
        .args(["sources", "ajouter"])
        .arg(&e.films)
        .assert()
        .success()
        .stdout(contains("déjà présente"));
    let attendu = std::fs::canonicalize(&e.films).unwrap();
    e.cmd()
        .args(["sources", "lister"])
        .assert()
        .success()
        .stdout(contains(attendu.display().to_string()));
    // Les sources de séries sont distinctes.
    e.cmd()
        .args(["sources", "lister", "--series"])
        .assert()
        .success()
        .stdout(contains("Aucune source de séries"));
    e.cmd()
        .args(["sources", "ajouter", "--series"])
        .arg(&e.films)
        .assert()
        .success();
    e.cmd()
        .args(["sources", "lister", "--series"])
        .assert()
        .success()
        .stdout(contains(attendu.display().to_string()));
    // Erreurs.
    e.cmd()
        .args(["sources", "ajouter"])
        .arg(e.films.join("inexistant"))
        .assert()
        .code(1)
        .stderr(contains("n'existe pas"));
    e.cmd()
        .args(["sources", "retirer", "/nulle/part"])
        .assert()
        .code(1);
    e.cmd()
        .args(["sources", "retirer"])
        .arg(&e.films)
        .assert()
        .success()
        .stdout(contains("retirée"));
    e.cmd()
        .args(["sources", "lister"])
        .assert()
        .success()
        .stdout(contains("Aucune source de films"));
}

#[test]
fn mise_a_jour_et_liste() {
    let e = Environnement::nouveau();
    e.cmd()
        .arg("maj")
        .assert()
        .success()
        .stdout(contains("Aucune source de films"));
    e.importer();

    e.cmd().arg("lister").assert().success().stdout(
        contains("Film A")
            .and(contains("Film B"))
            .and(contains("La Grande Aventure"))
            .and(contains("3 films")),
    );
    let tous = e.json(&[]);
    assert_eq!(tous.len(), 3);
    assert!(tous.iter().all(|f| f["nouveau"] == true));
    assert!(tous.iter().all(|f| f["metadonnees"] == false));
    let a = tous.iter().find(|f| f["titre"] == "Film A").unwrap();
    assert_eq!(a["annee"], 2010);

    assert_eq!(e.json(&["--recherche", "aVENT"]).len(), 1);
    assert_eq!(e.json(&["--sans-metadonnees"]).len(), 3);
    assert_eq!(e.json(&["--nouveaux"]).len(), 3);

    // Deuxième passage : rien de nouveau.
    e.cmd()
        .args(["maj", "--tout"])
        .assert()
        .success()
        .stdout(contains("Films : 0 ajouté(s)").and(contains("Aucune source de séries")));
    assert!(e.json(&["--nouveaux"]).is_empty());
    e.cmd()
        .args(["lister", "--nouveaux"])
        .assert()
        .success()
        .stdout(contains("Aucun film"));

    // Film disparu.
    std::fs::remove_dir_all(e.films.join("Film.B.2011")).unwrap();
    e.cmd()
        .args(["-q", "maj"])
        .assert()
        .success()
        .stdout(contains("1 disparu(s)"));
    assert_eq!(e.json(&[]).len(), 2);
    e.cmd()
        .arg("info")
        .assert()
        .success()
        .stdout(contains("Films       : 2 (sources : 1)"));
}

#[test]
fn scraper_sans_cle_echoue_proprement() {
    let e = Environnement::nouveau();
    e.importer();
    e.cmd()
        .arg("scraper")
        .assert()
        .code(1)
        .stderr(contains("aucune clé d'API TMDB").and(contains("mediamanager cle-tmdb")));
    e.cmd()
        .args(["scraper", "--tous"])
        .assert()
        .code(1)
        .stderr(contains("cle-tmdb"));
}

#[test]
fn cle_tmdb_jamais_affichee() {
    let e = Environnement::nouveau();
    // Depuis la variable d'environnement.
    e.cmd()
        .env("MEDIAMANAGER_CLE_TMDB", CLE_FACTICE)
        .args(["-v", "cle-tmdb"])
        .assert()
        .success()
        .stdout(contains("enregistrée").and(contains(CLE_FACTICE).not()))
        .stderr(contains(CLE_FACTICE).not());
    e.cmd()
        .arg("info")
        .assert()
        .success()
        .stdout(contains("Clé TMDB    : configurée").and(contains(CLE_FACTICE).not()));
    // Jamais en clair dans le fichier de réglages.
    let reglages = std::fs::read_to_string(e.racine.join("data/reglages.json")).unwrap();
    assert!(!reglages.contains(CLE_FACTICE));

    // Effacement.
    e.cmd().args(["cle-tmdb", "--supprimer"]).assert().success();
    e.cmd()
        .arg("info")
        .assert()
        .success()
        .stdout(contains("non configurée"));

    // Depuis l'entrée standard redirigée.
    e.cmd()
        .arg("cle-tmdb")
        .write_stdin(format!("{CLE_FACTICE}\n"))
        .assert()
        .success()
        .stdout(contains(CLE_FACTICE).not());
    e.cmd()
        .arg("info")
        .assert()
        .success()
        .stdout(contains("Clé TMDB    : configurée"));

    // Clé vide : refusée.
    e.cmd()
        .arg("cle-tmdb")
        .write_stdin("\n")
        .assert()
        .code(1)
        .stderr(contains("clé vide"));
}

#[test]
fn ecriture_des_nfo() {
    let e = Environnement::nouveau();
    e.importer();
    e.cmd()
        .arg("nfo")
        .assert()
        .success()
        .stdout(contains("pour 3 films"));
    assert!(e.films.join("Film A (2010)/Film.A.2010.nfo").is_file());
    assert!(e.films.join("Film.B.2011/Film.B.2011.nfo").is_file());

    // Sélection par début d'identifiant.
    let id = e.id_de("Film A");
    e.cmd()
        .args(["nfo", "--id", &id[..8]])
        .assert()
        .success()
        .stdout(contains("pour 1 film\n"));
    e.cmd()
        .args(["nfo", "--id", "zzzz-inconnu"])
        .assert()
        .code(1)
        .stderr(contains("introuvable"));
}

#[test]
fn renommage_apercu_puis_application() {
    let e = Environnement::nouveau();
    e.importer();
    let ancien = e.films.join("Film.B.2011/Film.B.2011.mkv");
    let nouveau = e.films.join("Film B (2011)/Film B (2011).mkv");

    // Aperçu : rien n'est modifié.
    e.cmd().arg("renommer").assert().code(0).stdout(
        contains("Film.B.2011 → Film B (2011)")
            .and(contains("--appliquer"))
            .and(contains("CONFLIT").not()),
    );
    assert!(ancien.is_file());

    // Confirmation refusée.
    e.cmd()
        .args(["renommer", "--appliquer"])
        .write_stdin("n\n")
        .assert()
        .code(0)
        .stdout(contains("abandonné"));
    assert!(ancien.is_file());

    // Confirmation acceptée sur l'entrée standard, pour un seul film.
    let id = e.id_de("Film A");
    e.cmd()
        .args(["renommer", "--appliquer", "--id", &id])
        .write_stdin("o\n")
        .assert()
        .code(0)
        .stdout(contains("1 renommé(s)"));
    assert!(e.films.join("Film A (2010)/Film A (2010).mkv").is_file());
    assert!(ancien.is_file());

    // Sans confirmation.
    e.cmd()
        .args(["renommer", "--appliquer", "--oui"])
        .assert()
        .code(0)
        .stdout(contains("1 renommé(s)"));
    assert!(!ancien.exists());
    assert!(nouveau.is_file());
    let b = e
        .json(&[])
        .into_iter()
        .find(|f| f["titre"] == "Film B")
        .unwrap();
    assert!(b["chemin"].as_str().unwrap().ends_with("Film B (2011)"));

    e.cmd()
        .arg("renommer")
        .assert()
        .code(0)
        .stdout(contains("Aucun renommage nécessaire"));
}

#[test]
fn renommage_en_conflit_code_2() {
    let e = Environnement::nouveau();
    e.importer();
    // Le dossier de destination de « Film B » existe déjà (non vide).
    std::fs::create_dir_all(e.films.join("Film B (2011)")).unwrap();
    std::fs::write(e.films.join("Film B (2011)/notes.txt"), b"x").unwrap();

    e.cmd()
        .arg("renommer")
        .assert()
        .code(2)
        .stdout(contains("CONFLIT").and(contains("existe déjà")));
    e.cmd()
        .args(["renommer", "--appliquer", "--oui"])
        .assert()
        .code(2)
        .stdout(contains("1 renommé(s)").and(contains("1 en conflit")));
    // L'élément en conflit n'a pas été touché.
    assert!(e.films.join("Film.B.2011/Film.B.2011.mkv").is_file());
    assert!(e.films.join("Film A (2010)/Film A (2010).mkv").is_file());
}

#[test]
fn instance_deja_lancee_code_3() {
    let e = Environnement::nouveau();
    let verrou = mediamanager_store::VerrouInstance::acquerir(&e.racine.join("data")).unwrap();
    e.cmd()
        .arg("info")
        .assert()
        .code(3)
        .stderr(contains("autre instance"));
    drop(verrou);
    e.cmd().arg("info").assert().code(0);
}
