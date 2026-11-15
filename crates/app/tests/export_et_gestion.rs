//! Tests d'intégration : export par modèles, nettoyage, évaluations, édition
//! globale, état « vu » et retrait des séries. Arborescences temporaires et
//! faux serveur TMDB (wiremock) : aucun appel réseau réel.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use mediamanager_app::*;
use mediamanager_core::modele::{Edition, Film, SourceMedia, TypeFichier};
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

fn png(largeur: u32, hauteur: u32) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(largeur, hauteur, image::Rgb([200, 30, 30]));
    let mut v = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut std::io::Cursor::new(&mut v), image::ImageFormat::Png)
        .unwrap();
    v
}

fn vider(rx: &Receiver<Evenement>) -> Vec<Evenement> {
    rx.try_iter().collect()
}

/// Application avec deux films scannés (« The Matrix » avec affiche PNG).
fn demarrer_films(racine: &Path, url: Option<String>) -> (Application, PathBuf) {
    let films = racine.join("films");
    ecrire(
        &films.join("The Matrix (1999)/The Matrix (1999).mkv"),
        b"video matrix",
    );
    ecrire(&films.join("The Matrix (1999)/poster.png"), &png(400, 600));
    ecrire(
        &films.join("Inception (2010)/Inception (2010).mkv"),
        b"video inception",
    );
    let app = Application::demarrer(options(racine, url)).unwrap();
    let f = films.clone();
    app.modifier_reglages(move |r| {
        r.films.sources = vec![f];
        r.general.corbeille_activee = false;
    })
    .unwrap();
    app.mettre_a_jour_sources_films().attendre().unwrap();
    (app, films)
}

fn film(app: &Application, debut: &str) -> Film {
    app.films()
        .unwrap()
        .into_iter()
        .find(|f| f.titre.starts_with(debut))
        .unwrap_or_else(|| panic!("film « {debut} » absent"))
}

fn modele(app: &Application, t: TypeExport, id: &str) -> ModeleExport {
    app.modeles_export(t)
        .into_iter()
        .find(|m| m.id == id)
        .unwrap_or_else(|| panic!("modèle {id} absent"))
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

#[test]
fn export_films_modeles_integres_et_echappement() {
    let d = tempfile::tempdir().unwrap();
    let (app, _) = demarrer_films(d.path(), None);
    let mut m = film(&app, "The Matrix");
    m.titre = "Matrix <script>alert(1)</script> & \"Cie\"".into();
    m.resume = "Il dit \"oui\", puis\nnon ; fin".into();
    m.genres = vec!["Action".into(), "Science-Fiction".into()];
    m.vu = true;
    let m = app.enregistrer_film(m).unwrap();
    let sortie = d.path().join("sortie");

    let ids = app.modeles_export(TypeExport::Films);
    for attendu in [
        "integre:films-html",
        "integre:films-csv",
        "integre:films-texte",
    ] {
        assert!(ids.iter().any(|x| x.id == attendu), "{attendu} manquant");
    }

    // HTML : échappé, vignette réduite et référencée.
    let r = app
        .exporter_films(
            None,
            &modele(&app, TypeExport::Films, "integre:films-html"),
            &sortie,
            OptionsExport {
                vignettes: Some(50),
                titre: Some("Ma <liste>".into()),
            },
        )
        .attendre()
        .unwrap();
    assert_eq!(r.exportes, 2, "{r:?}");
    let html = std::fs::read_to_string(&r.fichier_liste).unwrap();
    assert!(
        html.contains("Matrix &lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;Cie&quot;"),
        "{html}"
    );
    assert!(!html.contains("<script>"));
    assert!(html.contains("<title>Ma &lt;liste&gt;</title>"));
    assert!(html.contains("Action, Science-Fiction"));
    assert!(html.contains("class=\"vu\""));
    let vignette = sortie.join("images").join(format!("{}.jpg", m.id));
    assert!(
        html.contains(&format!("src=\"images/{}.jpg\"", m.id)),
        "{html}"
    );
    let img = image::open(&vignette).unwrap();
    assert_eq!(img.width(), 50);
    assert_eq!(r.vignettes, vec![vignette]);

    // CSV : champs entre guillemets, guillemets doublés, retour ligne conservé.
    let r = app
        .exporter_films(
            Some(vec![m.id.clone(), "inconnu".into()]),
            &modele(&app, TypeExport::Films, "integre:films-csv"),
            &sortie,
            OptionsExport::default(),
        )
        .attendre()
        .unwrap();
    assert_eq!(r.exportes, 1);
    assert_eq!(r.introuvables, vec!["inconnu".to_string()]);
    assert!(r.vignettes.is_empty());
    let csv = std::fs::read_to_string(&r.fichier_liste).unwrap();
    assert!(csv.starts_with("\u{FEFF}Titre;"), "{csv}");
    assert!(
        csv.contains("\"Matrix <script>alert(1)</script> & \"\"Cie\"\"\""),
        "{csv}"
    );
    assert!(
        csv.contains("\"Il dit \"\"oui\"\", puis\nnon ; fin\""),
        "{csv}"
    );
    assert!(csv.contains("\"Action, Science-Fiction\""), "{csv}");
    assert!(csv.contains(";oui;"));

    // Texte.
    let r = app
        .exporter_films(
            None,
            &modele(&app, TypeExport::Films, "integre:films-texte"),
            &sortie,
            OptionsExport::default(),
        )
        .attendre()
        .unwrap();
    let txt = std::fs::read_to_string(&r.fichier_liste).unwrap();
    assert!(txt.starts_with("Films (2)\n"), "{txt}");
    assert!(txt.contains("Inception (2010)\n"), "{txt}");
    assert!(txt.contains("[vu]"));

    // Mauvais type de modèle : refusé.
    let e = app
        .exporter_films(
            None,
            &modele(&app, TypeExport::Series, "integre:series-csv"),
            &sortie,
            OptionsExport::default(),
        )
        .attendre();
    assert!(e.is_err());
    app.arreter().unwrap();
}

#[test]
fn export_modele_utilisateur_avec_pages_de_detail() {
    let d = tempfile::tempdir().unwrap();
    let (app, _) = demarrer_films(d.path(), None);
    let racine = app.dossier_modeles_export();
    let mon = racine.join("mon-modele");
    ecrire(
        &mon.join("template.conf"),
        b"# modele\nname=Mon modele\ntype=MOVIE\nlist=liste.tpl\ndetail=detail.tpl\nfilename=catalogue\nextension=xml\n",
    );
    ecrire(
        &mon.join("liste.tpl"),
        "<liste n=\"${count}\">${foreach movies movie}<f t=\"${movie.title}\" d=\"${movie.detailFile}\"/>${end}</liste>".as_bytes(),
    );
    ecrire(&mon.join("detail.tpl"), b"<d>${movie.title}</d>");
    ecrire(&mon.join("css/style.css"), b"body{}");
    // Modèles invalides : traversée de dossier, type inconnu.
    let mauvais = racine.join("mauvais");
    ecrire(
        &mauvais.join("template.conf"),
        b"type=MOVIE\nlist=../secret.tpl\n",
    );
    ecrire(&racine.join("secret.tpl"), b"secret");
    ecrire(
        &racine.join("saga/template.conf"),
        b"type=MOVIE_SET\nlist=l\n",
    );
    ecrire(&racine.join("saga/l"), b"x");

    let modeles = app.modeles_export(TypeExport::Films);
    assert!(
        modeles
            .iter()
            .all(|m| m.id != "utilisateur:mauvais" && m.id != "utilisateur:saga")
    );
    let m = modeles
        .into_iter()
        .find(|m| m.id == "utilisateur:mon-modele")
        .unwrap();
    assert!(!m.integre);
    assert_eq!(m.nom, "Mon modele");
    assert!(
        app.modeles_export(TypeExport::Series)
            .iter()
            .all(|x| x.integre)
    );

    let mut f = film(&app, "Inception");
    f.titre = "L'Origine <&>".into();
    app.enregistrer_film(f.clone()).unwrap();
    let sortie = d.path().join("export");
    let r = app
        .exporter_films(
            Some(vec![f.id.clone()]),
            &m,
            &sortie,
            OptionsExport::default(),
        )
        .attendre()
        .unwrap();
    assert_eq!(r.fichier_liste, sortie.join("catalogue.xml"));
    let xml = std::fs::read_to_string(&r.fichier_liste).unwrap();
    assert_eq!(
        xml,
        format!(
            "<liste n=\"1\"><f t=\"L&apos;Origine &lt;&amp;&gt;\" d=\"films/{}.xml\"/></liste>",
            f.id
        )
    );
    assert_eq!(r.pages_detail.len(), 1);
    assert_eq!(
        std::fs::read_to_string(&r.pages_detail[0]).unwrap(),
        "<d>L&apos;Origine &lt;&amp;&gt;</d>"
    );
    assert!(sortie.join("css/style.css").exists());
    assert!(!sortie.join("template.conf").exists());
    assert!(!sortie.join("liste.tpl").exists());
    // Sortie dans le dossier du modèle : refusée.
    let e = app
        .exporter_films(None, &m, &mon.join("sortie"), OptionsExport::default())
        .attendre();
    assert!(e.is_err());
    app.arreter().unwrap();
}

fn demarrer_series(racine: &Path) -> (Application, PathBuf) {
    let series = racine.join("series");
    let dossier = series.join("Ma Serie (2020)");
    ecrire(&dossier.join("Ma Serie - S01E01.mkv"), b"e1");
    ecrire(&dossier.join("Ma Serie - S01E02.mkv"), b"e2");
    ecrire(&dossier.join("Ma Serie - S02E01.mkv"), b"e3");
    let app = Application::demarrer(options(racine, None)).unwrap();
    let s = series.clone();
    app.modifier_reglages(move |r| {
        r.series.sources = vec![s];
        r.general.corbeille_activee = false;
    })
    .unwrap();
    app.mettre_a_jour_sources_series().attendre().unwrap();
    (app, dossier)
}

#[test]
fn series_export_vu_et_retrait() {
    let d = tempfile::tempdir().unwrap();
    let (app, dossier) = demarrer_series(d.path());
    let abonne = app.abonner();
    let mut serie = app.series().unwrap().remove(0);
    assert_eq!(serie.episodes.len(), 3);
    serie.titre = "Ma <Série>".into();
    let serie = app.enregistrer_serie(serie).unwrap();
    vider(&abonne);

    // Vu : une saison, puis bascule d'un épisode, puis bascule globale.
    let r = app
        .basculer_vu_series(
            &[CibleVu::Saison {
                id_serie: serie.id.clone(),
                saison: 1,
            }],
            Some(true),
        )
        .unwrap();
    assert!(r.vu);
    assert_eq!(r.episodes_modifies, 2, "{r:?}");
    assert_eq!(r.series_modifiees, vec![serie.id.clone()]);
    assert!(
        vider(&abonne)
            .iter()
            .any(|e| matches!(e, Evenement::SerieModifiee { id } if *id == serie.id))
    );
    let s = app.serie(&serie.id).unwrap().unwrap();
    assert_eq!(s.episodes.iter().filter(|e| e.vu).count(), 2);
    assert!(
        s.episodes
            .iter()
            .filter(|e| e.vu)
            .all(|e| e.nombre_lectures == 1 && e.derniere_lecture.is_some())
    );
    assert!(s.derniere_lecture.is_some());

    // Idempotent : rien ne change, aucun événement.
    let r = app
        .basculer_vu_series(&[CibleVu::Serie(serie.id.clone())], Some(false))
        .unwrap();
    assert_eq!(r.episodes_modifies, 2);
    let r = app
        .basculer_vu_series(&[CibleVu::Serie(serie.id.clone())], Some(false))
        .unwrap();
    assert_eq!(r.episodes_modifies, 0);
    assert!(r.series_modifiees.is_empty());
    // Bascule sans état : pas tous vus ⇒ tous vus ; puis tous vus ⇒ aucun.
    let r = app
        .basculer_vu_series(&[CibleVu::Serie(serie.id.clone())], None)
        .unwrap();
    assert!(r.vu && r.episodes_modifies == 3);
    let un = s.episodes[0].id.clone();
    let r = app
        .basculer_vu_series(
            &[CibleVu::Episodes {
                id_serie: serie.id.clone(),
                ids_episodes: vec![un],
            }],
            None,
        )
        .unwrap();
    assert!(!r.vu && r.episodes_modifies == 1);
    let r = app
        .basculer_vu_series(&[CibleVu::Serie("absente".into())], None)
        .unwrap();
    assert_eq!(r.introuvables, vec!["absente".to_string()]);

    // Export HTML, CSV, texte (saisons, épisodes, échappement).
    let sortie = d.path().join("export");
    let html = app
        .exporter_series(
            None,
            &modele(&app, TypeExport::Series, "integre:series-html"),
            &sortie,
            OptionsExport::default(),
        )
        .attendre()
        .unwrap();
    let h = std::fs::read_to_string(&html.fichier_liste).unwrap();
    assert!(h.contains("Ma &lt;Série&gt; (2020)"), "{h}");
    assert!(h.contains("2/3 épisode(s) vu(s)"), "{h}");
    assert!(h.contains("Saison 1") && h.contains("Saison 2"), "{h}");
    let csv = app
        .exporter_series(
            None,
            &modele(&app, TypeExport::Series, "integre:series-csv"),
            &sortie,
            OptionsExport::default(),
        )
        .attendre()
        .unwrap();
    let c = std::fs::read_to_string(&csv.fichier_liste).unwrap();
    assert!(c.contains("\"Ma <Série>\""), "{c}");
    assert!(c.contains(";2;3;2;"), "{c}");
    let txt = app
        .exporter_series(
            Some(vec![serie.id.clone()]),
            &modele(&app, TypeExport::Series, "integre:series-texte"),
            &sortie,
            OptionsExport::default(),
        )
        .attendre()
        .unwrap();
    let t = std::fs::read_to_string(&txt.fichier_liste).unwrap();
    assert!(
        t.contains("Ma <Série> (2020) — 2/3 épisode(s) vu(s)"),
        "{t}"
    );
    assert!(t.contains("    1. ") && t.contains("[vu]"), "{t}");

    // Retrait : série verrouillée refusée ; sinon dossier à la corbeille.
    let mut v = app.serie(&serie.id).unwrap().unwrap();
    v.verrouille = true;
    app.enregistrer_serie(v).unwrap();
    let r = app
        .retirer_series(vec![serie.id.clone()], true)
        .attendre()
        .unwrap();
    assert_eq!(r.verrouilles, vec![serie.id.clone()]);
    assert!(r.retires.is_empty() && dossier.exists());
    let mut v = app.serie(&serie.id).unwrap().unwrap();
    v.verrouille = false;
    app.enregistrer_serie(v).unwrap();
    vider(&abonne);
    let r = app
        .retirer_series(vec![serie.id.clone(), "absente".into()], true)
        .attendre()
        .unwrap();
    assert_eq!(r.retires, vec![serie.id.clone()], "{r:?}");
    assert_eq!(r.echecs.len(), 1);
    assert!(!dossier.exists());
    let corbeille = d
        .path()
        .join("series")
        .join(DOSSIER_CORBEILLE_SOURCE)
        .join("Ma Serie (2020)");
    assert!(corbeille.join("Ma Serie - S01E01.mkv").exists());
    assert!(app.series().unwrap().is_empty());
    assert!(
        vider(&abonne)
            .iter()
            .any(|e| matches!(e, Evenement::SerieSupprimee { .. }))
    );
    app.arreter().unwrap();
}

#[test]
fn retrait_serie_sans_fichiers_garde_le_disque() {
    let d = tempfile::tempdir().unwrap();
    let (app, dossier) = demarrer_series(d.path());
    let s = app.series().unwrap().remove(0);
    let r = app
        .retirer_series(vec![s.id.clone()], false)
        .attendre()
        .unwrap();
    assert_eq!(r.retires.len(), 1);
    assert!(r.mis_a_la_corbeille.is_empty());
    assert!(dossier.join("Ma Serie - S01E01.mkv").exists());
    app.arreter().unwrap();
}

// ---------------------------------------------------------------------------
// Nettoyage
// ---------------------------------------------------------------------------

#[test]
fn nettoyage_analyse_et_corbeille() {
    let d = tempfile::tempdir().unwrap();
    let (app, films) = demarrer_films(d.path(), None);
    let m = film(&app, "The Matrix");
    let i = film(&app, "Inception");
    let dm = films.join("The Matrix (1999)");
    let di = films.join("Inception (2010)");
    ecrire(&dm.join("Visitez-nous.url"), b"[InternetShortcut]");
    ecrire(&dm.join("lisez-moi.txt"), b"abc");
    ecrire(&dm.join("Thumbs.db"), b"t");
    ecrire(&dm.join("Sample/extrait.mkv"), b"123456");
    ecrire(&dm.join("sous-dossier/notes.html"), b"<html/>");
    ecrire(&di.join("garde.srt"), b"1");
    app.mettre_a_jour_selection_films(vec![m.id.clone(), i.id.clone()])
        .attendre()
        .unwrap();
    // NFO apparu depuis la dernière mise à jour : non rattaché au film.
    ecrire(&dm.join("autre.nfo"), b"<movie><title>x</title></movie>");

    let liste = app
        .analyser_nettoyage(&[m.id.clone(), i.id.clone(), m.id.clone()])
        .unwrap();
    let noms: Vec<String> = liste
        .iter()
        .map(|f| {
            f.chemin
                .strip_prefix(&films)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    assert_eq!(
        noms,
        vec![
            "The Matrix (1999)/Sample",
            "The Matrix (1999)/Thumbs.db",
            "The Matrix (1999)/Visitez-nous.url",
            "The Matrix (1999)/autre.nfo",
            "The Matrix (1999)/lisez-moi.txt",
            "The Matrix (1999)/sous-dossier/notes.html",
        ],
        "{liste:?}"
    );
    assert!(liste.iter().all(|f| f.id_film == m.id));
    assert!(
        liste
            .iter()
            .find(|f| f.est_dossier)
            .is_some_and(|f| f.taille == 6)
    );
    // Rien n'a bougé pendant l'analyse.
    assert!(dm.join("lisez-moi.txt").exists());

    // Éléments forgés : vidéo du film, hors dossier, `..`, autre film.
    let mut forges = vec![
        FichierInutile {
            id_film: m.id.clone(),
            chemin: dm.join("The Matrix (1999).mkv"),
            est_dossier: false,
            raison: RaisonInutile::Parasite,
            taille: 0,
        },
        FichierInutile {
            id_film: m.id.clone(),
            chemin: di.join("garde.srt"),
            est_dossier: false,
            raison: RaisonInutile::Parasite,
            taille: 0,
        },
        FichierInutile {
            id_film: m.id.clone(),
            chemin: dm.join("../Inception (2010)/garde.srt"),
            est_dossier: false,
            raison: RaisonInutile::Parasite,
            taille: 0,
        },
        FichierInutile {
            id_film: m.id.clone(),
            chemin: dm.clone(),
            est_dossier: true,
            raison: RaisonInutile::Extrait,
            taille: 0,
        },
        FichierInutile {
            id_film: "inconnu".into(),
            chemin: dm.join("lisez-moi.txt"),
            est_dossier: false,
            raison: RaisonInutile::Parasite,
            taille: 0,
        },
    ];
    let r = app.executer_nettoyage(forges.clone()).attendre().unwrap();
    assert!(r.mis_a_la_corbeille.is_empty(), "{r:?}");
    assert_eq!(r.ignores.len(), 5, "{r:?}");
    assert!(dm.join("The Matrix (1999).mkv").exists());
    assert!(di.join("garde.srt").exists());
    assert!(dm.join("lisez-moi.txt").exists());

    // Exécution réelle.
    let abonne = app.abonner();
    forges.clear();
    let r = app.executer_nettoyage(liste.clone()).attendre().unwrap();
    assert_eq!(r.mis_a_la_corbeille.len(), 6, "{r:?}");
    assert!(r.echecs.is_empty() && r.ignores.is_empty());
    assert_eq!(r.octets, liste.iter().map(|f| f.taille).sum::<u64>());
    let corb = films
        .join(DOSSIER_CORBEILLE_SOURCE)
        .join("The Matrix (1999)");
    assert!(corb.join("lisez-moi.txt").exists());
    assert!(corb.join("Sample/extrait.mkv").exists());
    assert!(corb.join("sous-dossier/notes.html").exists());
    assert!(!dm.join("lisez-moi.txt").exists() && !dm.join("Sample").exists());
    assert!(dm.join("The Matrix (1999).mkv").exists() && dm.join("poster.png").exists());
    assert!(
        vider(&abonne)
            .iter()
            .any(|e| matches!(e, Evenement::FilmModifie { id } if *id == m.id))
    );
    // Plus rien à nettoyer ; la corbeille n'est pas analysée.
    assert!(
        app.analyser_nettoyage(std::slice::from_ref(&m.id))
            .unwrap()
            .is_empty()
    );
    app.arreter().unwrap();
}

#[test]
fn nettoyage_motifs_reglages_et_films_exclus() {
    let d = tempfile::tempdir().unwrap();
    let (app, films) = demarrer_films(d.path(), None);
    let m = film(&app, "The Matrix");
    let i = film(&app, "Inception");
    ecrire(&films.join("The Matrix (1999)/info.diz"), b"x");
    ecrire(&films.join("The Matrix (1999)/notes.txt"), b"x");
    ecrire(&films.join("Inception (2010)/notes.txt"), b"x");
    // Motifs des réglages : `.diz` seulement.
    app.modifier_reglages(|r| r.general.motifs_nettoyage = vec![r"\.diz$".into(), "(".into()])
        .unwrap();
    let l = app.analyser_nettoyage(std::slice::from_ref(&m.id)).unwrap();
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].raison, RaisonInutile::Motif(r"\.diz$".into()));
    // Verrouillé : ignoré.
    let mut v = i.clone();
    v.verrouille = true;
    app.enregistrer_film(v).unwrap();
    app.modifier_reglages(|r| r.general.motifs_nettoyage = vec![r"\.txt$".into()])
        .unwrap();
    assert!(
        app.analyser_nettoyage(std::slice::from_ref(&i.id))
            .unwrap()
            .is_empty()
    );
    let forge = vec![FichierInutile {
        id_film: i.id.clone(),
        chemin: films.join("Inception (2010)/notes.txt"),
        est_dossier: false,
        raison: RaisonInutile::Motif(r"\.txt$".into()),
        taille: 1,
    }];
    let r = app.executer_nettoyage(forge).attendre().unwrap();
    assert_eq!(r.ignores.len(), 1);
    assert!(films.join("Inception (2010)/notes.txt").exists());
    // Film dont le dossier est la racine de la source : ignoré.
    let mut racine = m.clone();
    racine.chemin = films.clone();
    app.enregistrer_film(racine).unwrap();
    assert!(
        app.analyser_nettoyage(std::slice::from_ref(&m.id))
            .unwrap()
            .is_empty()
    );
    app.arreter().unwrap();
}

// ---------------------------------------------------------------------------
// Évaluations
// ---------------------------------------------------------------------------

fn monter_tmdb(rt: &tokio::runtime::Runtime, note: f64) -> MockServer {
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
            .and(path("/3/movie/603"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 603, "title": "Titre du fournisseur", "original_title": "The Matrix",
                "overview": "Résumé du fournisseur", "runtime": 136,
                "release_date": "1999-03-30", "vote_average": note, "vote_count": 21000,
                "genres": [{"id": 28, "name": "Action"}],
                "credits": {"cast": [], "crew": []},
                "external_ids": {"imdb_id": "tt0133093"}
            })))
            .mount(&s)
            .await;
        Mock::given(method("GET"))
            .and(path_regex("^/3/movie/999.*"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({"status_code": 34})))
            .mount(&s)
            .await;
        s
    })
}

#[test]
fn evaluations_mises_a_jour_sans_toucher_au_reste() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let serveur = monter_tmdb(&rt, 9.1);
    let d = tempfile::tempdir().unwrap();
    let (app, films) = demarrer_films(d.path(), Some(format!("{}/3/", serveur.uri())));
    // Matrix : NFO existant, id TMDB, notes de plusieurs sources.
    let mut m = film(&app, "The Matrix");
    ecrire(
        &films.join("The Matrix (1999)/The Matrix (1999).nfo"),
        b"<movie><title>The Matrix</title></movie>",
    );
    app.mettre_a_jour_selection_films(vec![m.id.clone()])
        .attendre()
        .unwrap();
    m = app.film(&m.id).unwrap().unwrap();
    assert!(
        m.fichiers
            .iter()
            .any(|f| f.type_fichier == TypeFichier::Nfo)
    );
    m.ids.definir("tmdb", "603");
    m.titre = "Titre saisi".into();
    m.resume = "Résumé saisi".into();
    m.notes.insert(
        "tmdb".into(),
        mediamanager_core::modele::Note::nouvelle("tmdb", 5.0, 10, 10),
    );
    m.notes.insert(
        "imdb".into(),
        mediamanager_core::modele::Note::nouvelle("imdb", 8.7, 2_000_000, 10),
    );
    m.notes.insert(
        "user".into(),
        mediamanager_core::modele::Note::nouvelle("user", 10.0, 1, 10),
    );
    app.enregistrer_film(m.clone()).unwrap();
    // Inception : sans identifiant ; un film verrouillé ; un film en erreur.
    let i = film(&app, "Inception");
    let mut verrou = i.clone();
    verrou.ids.definir("tmdb", "603");
    verrou.verrouille = true;
    app.enregistrer_film(verrou).unwrap();

    // Sans clé : erreur avant tout appel réseau.
    assert!(matches!(
        app.recuperer_evaluations(vec![m.id.clone()]).attendre(),
        Err(ErreurApp::CleTmdbAbsente)
    ));
    app.definir_cle_tmdb(CLE_TEST).unwrap();
    let abonne = app.abonner();
    let r = app
        .recuperer_evaluations(vec![m.id.clone(), i.id.clone(), "absent".into()])
        .attendre()
        .unwrap();
    assert_eq!(r.mis_a_jour, vec![m.id.clone()], "{r:?}");
    assert_eq!(r.verrouilles, vec![i.id.clone()]);
    assert_eq!(r.erreurs.len(), 1);
    let apres = app.film(&m.id).unwrap().unwrap();
    assert!(
        (apres.notes["tmdb"].valeur - 9.1).abs() < 1e-4,
        "{:?}",
        apres.notes
    );
    assert_eq!(apres.notes["tmdb"].votes, 21000);
    assert_eq!(apres.notes["imdb"], m.notes["imdb"]);
    assert_eq!(apres.notes["user"], m.notes["user"]);
    // Le reste est intact.
    assert_eq!(apres.titre, "Titre saisi");
    assert_eq!(apres.resume, "Résumé saisi");
    assert_eq!(apres.genres, m.genres);
    assert_eq!(apres.duree_minutes, m.duree_minutes);
    assert!(
        vider(&abonne)
            .iter()
            .any(|e| matches!(e, Evenement::FilmModifie { id } if *id == m.id))
    );
    // Le NFO existant reflète la nouvelle note.
    let nfo =
        std::fs::read_to_string(films.join("The Matrix (1999)/The Matrix (1999).nfo")).unwrap();
    assert!(nfo.contains("9.1"), "{nfo}");

    // Deuxième passage : rien à changer, pas d'événement.
    let r = app
        .recuperer_evaluations(vec![m.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(r.inchanges, vec![m.id.clone()]);
    assert!(
        vider(&abonne).is_empty()
            || !vider(&abonne)
                .iter()
                .any(|e| matches!(e, Evenement::FilmModifie { .. }))
    );

    // Sans identifiant.
    let mut libre = film(&app, "Inception");
    libre.verrouille = false;
    libre.ids = Default::default();
    let libre = app.enregistrer_film(libre).unwrap();
    let r = app
        .recuperer_evaluations(vec![libre.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(r.sans_identifiant, vec![libre.id.clone()]);
    // Identifiant inconnu du fournisseur : erreur rapportée, base intacte.
    let mut faux = libre.clone();
    faux.ids.definir("tmdb", "999");
    app.enregistrer_film(faux.clone()).unwrap();
    let r = app
        .recuperer_evaluations(vec![faux.id.clone()])
        .attendre()
        .unwrap();
    assert_eq!(r.erreurs.len(), 1, "{r:?}");
    assert!(app.film(&faux.id).unwrap().unwrap().notes.is_empty());
    app.arreter().unwrap();
    drop(serveur);
}

// ---------------------------------------------------------------------------
// Édition globale
// ---------------------------------------------------------------------------

#[test]
fn edition_globale_films_et_series() {
    let d = tempfile::tempdir().unwrap();
    let (app, films) = demarrer_films(d.path(), None);
    let m = film(&app, "The Matrix");
    let i = film(&app, "Inception");
    ecrire(
        &films.join("The Matrix (1999)/The Matrix (1999).nfo"),
        b"<movie><title>The Matrix</title></movie>",
    );
    app.mettre_a_jour_selection_films(vec![m.id.clone()])
        .attendre()
        .unwrap();
    let mut v = app.film(&i.id).unwrap().unwrap();
    v.verrouille = true;
    app.enregistrer_film(v).unwrap();
    let mut a = app.film(&m.id).unwrap().unwrap();
    a.genres = vec!["Drame".into()];
    app.enregistrer_film(a).unwrap();
    let abonne = app.abonner();

    let modifs = ModificationsLot {
        genres_ajouter: vec!["Action".into(), "drame".into()],
        tags_ajouter: vec!["à revoir".into()],
        certification: Some("FR:12".into()),
        societe_production: Some("Studio Test".into()),
        pays: Some("US".into()),
        vu: Some(true),
        edition: Some(Edition::DirectorsCut),
        source_media: Some(SourceMedia::Bluray),
        langues_parlees: Some("en".into()),
        ..ModificationsLot::default()
    };
    let r = app
        .modifier_en_lot_films(
            vec![m.id.clone(), i.id.clone(), "absent".into()],
            modifs.clone(),
        )
        .attendre()
        .unwrap();
    assert_eq!(r.modifies, vec![m.id.clone()], "{r:?}");
    assert_eq!(r.verrouilles, vec![i.id.clone()]);
    assert_eq!(r.introuvables, vec!["absent".to_string()]);
    let f = app.film(&m.id).unwrap().unwrap();
    assert_eq!(f.genres, vec!["Drame".to_string(), "Action".to_string()]);
    assert_eq!(f.tags, vec!["à revoir".to_string()]);
    assert_eq!(f.certification, "FR:12");
    assert_eq!(f.societe_production, "Studio Test");
    assert_eq!(f.pays, "US");
    assert!(f.vu && f.nombre_lectures == 1);
    assert_eq!(f.edition, Edition::DirectorsCut);
    assert_eq!(f.source_media, SourceMedia::Bluray);
    assert_eq!(f.langues_parlees, "en");
    assert!(
        vider(&abonne)
            .iter()
            .any(|e| matches!(e, Evenement::FilmModifie { id } if *id == m.id))
    );
    let nfo =
        std::fs::read_to_string(films.join("The Matrix (1999)/The Matrix (1999).nfo")).unwrap();
    assert!(
        nfo.contains("<genre>Action</genre>") && nfo.contains("FR:12"),
        "{nfo}"
    );
    // Le film verrouillé n'a pas bougé.
    assert!(app.film(&i.id).unwrap().unwrap().genres.is_empty());
    // Idempotent + retrait.
    let r = app
        .modifier_en_lot_films(vec![m.id.clone()], modifs)
        .attendre()
        .unwrap();
    assert_eq!(r.inchanges, vec![m.id.clone()]);
    let r = app
        .modifier_en_lot_films(
            vec![m.id.clone()],
            ModificationsLot {
                genres_retirer: vec!["ACTION".into()],
                vu: Some(false),
                ..ModificationsLot::default()
            },
        )
        .attendre()
        .unwrap();
    assert_eq!(r.modifies.len(), 1);
    let f = app.film(&m.id).unwrap().unwrap();
    assert_eq!(f.genres, vec!["Drame".to_string()]);
    assert!(!f.vu && f.nombre_lectures == 0);
    app.arreter().unwrap();

    // Séries.
    let d2 = tempfile::tempdir().unwrap();
    let (app, _) = demarrer_series(d2.path());
    let s = app.series().unwrap().remove(0);
    let r = app
        .modifier_en_lot_series(
            vec![s.id.clone()],
            ModificationsLotSeries {
                genres_ajouter: vec!["Drame".into()],
                certification: Some("TV-14".into()),
                ..ModificationsLotSeries::default()
            },
        )
        .attendre()
        .unwrap();
    assert_eq!(r.modifies, vec![s.id.clone()]);
    let s = app.serie(&s.id).unwrap().unwrap();
    assert_eq!(s.genres, vec!["Drame".to_string()]);
    assert_eq!(s.certification, "TV-14");
    app.arreter().unwrap();
}
