//! Exécution des sous-commandes.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use mediamanager_app::{Application, ConfigMetadonnees, RapportMiseAJour};
use mediamanager_core::modele::Film;
use mediamanager_core::renommage::{ActionFichier, ElementPlan, PlanRenommage};
use mediamanager_store::ModeDossiers;
use serde::Serialize;

use crate::arguments::{ActionSources, Cible, Selection};
use crate::erreur::{CodeSortie, ErreurCli, ResultatCli};
use crate::sortie::{Sortie, tableau, tronquer};

/// Variable d'environnement lue par `cle-tmdb` (évite la saisie interactive).
pub const VARIABLE_CLE_TMDB: &str = "MEDIAMANAGER_CLE_TMDB";

/// Longueur des identifiants affichés dans le tableau.
const LONGUEUR_ID_COURT: usize = 8;

fn pluriel(n: usize, singulier: &str, pluriel: &str) -> String {
    format!("{n} {}", if n > 1 { pluriel } else { singulier })
}

// -- Sources ------------------------------------------------------------------

fn nature(series: bool) -> &'static str {
    if series { "séries" } else { "films" }
}

/// Chemin absolu et normalisé quand c'est possible.
fn chemin_absolu(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| {
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|d| d.join(p))
                .unwrap_or_else(|_| p.to_path_buf())
        }
    })
}

pub fn sources(app: &Application, s: &Sortie, action: ActionSources) -> ResultatCli {
    match action {
        ActionSources::Ajouter {
            chemin: dossier,
            series,
        } => {
            if !dossier.is_dir() {
                return Err(ErreurCli::nouvelle(format!(
                    "le dossier {} n'existe pas ou n'est pas un dossier",
                    dossier.display()
                )));
            }
            let chemin = chemin_absolu(&dossier);
            let mut deja = false;
            let c = chemin.clone();
            app.modifier_reglages(|r| {
                let liste = if series {
                    &mut r.series.sources
                } else {
                    &mut r.films.sources
                };
                if liste.iter().any(|x| x == &c) {
                    deja = true;
                } else {
                    liste.push(c);
                }
            })?;
            if deja {
                s.info(format!(
                    "La source de {} {} est déjà présente.",
                    nature(series),
                    chemin.display()
                ));
            } else {
                s.info(format!(
                    "Source de {} ajoutée : {}",
                    nature(series),
                    chemin.display()
                ));
            }
            Ok(CodeSortie::Succes)
        }
        ActionSources::Retirer {
            chemin: dossier,
            series,
        } => {
            let absolu = chemin_absolu(&dossier);
            let mut retire = None;
            app.modifier_reglages(|r| {
                let liste = if series {
                    &mut r.series.sources
                } else {
                    &mut r.films.sources
                };
                if let Some(i) = liste.iter().position(|x| x == &absolu || x == &dossier) {
                    retire = Some(liste.remove(i));
                }
            })?;
            match retire {
                Some(p) => {
                    s.info(format!(
                        "Source de {} retirée : {} (les fichiers ne sont pas modifiés ; \
                         lancez « mediamanager maj » pour mettre la bibliothèque à jour)",
                        nature(series),
                        p.display()
                    ));
                    Ok(CodeSortie::Succes)
                }
                None => Err(ErreurCli::nouvelle(format!(
                    "{} ne fait pas partie des sources de {}",
                    dossier.display(),
                    nature(series)
                ))),
            }
        }
        ActionSources::Lister { series } => {
            let r = app.reglages();
            let liste = if series {
                &r.series.sources
            } else {
                &r.films.sources
            };
            if liste.is_empty() {
                s.info(format!(
                    "Aucune source de {} (ajoutez-en avec « mediamanager sources ajouter{} <dossier> »).",
                    nature(series),
                    if series { " --series" } else { "" }
                ));
            }
            for p in liste {
                if p.is_dir() {
                    println!("{}", p.display());
                } else {
                    println!("{}  (indisponible)", p.display());
                }
            }
            Ok(CodeSortie::Succes)
        }
    }
}

// -- Mise à jour ----------------------------------------------------------------

fn resume_maj(app: &Application, s: &Sortie, series: bool, r: &RapportMiseAJour) {
    let titre = if series { "Séries" } else { "Films" };
    let disparus = r.marques_disparus.len() + r.supprimes.len();
    let mut ligne = format!(
        "{titre} : {} ajouté(s), {} modifié(s), {} disparu(s)",
        r.ajoutes.len(),
        r.modifies.len(),
        disparus
    );
    if !r.supprimes.is_empty() {
        ligne.push_str(&format!(
            " (dont {} supprimé(s) de la base)",
            r.supprimes.len()
        ));
    }
    if r.verrouilles > 0 {
        ligne.push_str(&format!(", {} verrouillé(s) ignoré(s)", r.verrouilles));
    }
    println!("{ligne}");
    for (source, raison) in &r.sources_indisponibles {
        s.avertissement(format!(
            "source indisponible {} : {raison}",
            source.display()
        ));
    }
    for a in &r.avertissements {
        s.avertissement(a);
    }
    if s.verbeux > 0 && !r.ajoutes.is_empty() {
        let titres: BTreeMap<String, String> = if series {
            app.series()
                .unwrap_or_default()
                .into_iter()
                .map(|x| (x.id, format!("{} ({})", x.titre, x.annee)))
                .collect()
        } else {
            app.films()
                .unwrap_or_default()
                .into_iter()
                .map(|x| (x.id.clone(), libelle_film(&x)))
                .collect()
        };
        for id in &r.ajoutes {
            s.detail(format!(
                "  + {}",
                titres.get(id).map(String::as_str).unwrap_or(id)
            ));
        }
    }
}

fn maj_un(app: &Application, s: &Sortie, series: bool) -> Result<(), ErreurCli> {
    let r = app.reglages();
    let vide = if series {
        r.series.sources.is_empty()
    } else {
        r.films.sources.is_empty()
    };
    if vide {
        s.info(format!(
            "Aucune source de {} configurée : rien à mettre à jour.",
            nature(series)
        ));
        return Ok(());
    }
    let tache = if series {
        app.mettre_a_jour_sources_series()
    } else {
        app.mettre_a_jour_sources_films()
    };
    let rapport = s.suivre(tache)?;
    resume_maj(app, s, series, &rapport);
    Ok(())
}

pub fn maj(app: &Application, s: &Sortie, series: bool, tout: bool) -> ResultatCli {
    if tout || !series {
        maj_un(app, s, false)?;
    }
    if tout || series {
        maj_un(app, s, true)?;
    }
    Ok(CodeSortie::Succes)
}

// -- Liste ----------------------------------------------------------------------

fn libelle_film(f: &Film) -> String {
    if f.annee > 0 {
        format!("{} ({})", f.titre, f.annee)
    } else {
        f.titre.clone()
    }
}

fn sans_metadonnees(f: &Film) -> bool {
    f.ids.est_vide() && f.resume.trim().is_empty()
}

/// Film tel qu'exporté par `lister --json`.
#[derive(Debug, Serialize)]
struct FilmListe<'a> {
    id: &'a str,
    titre: &'a str,
    titre_original: &'a str,
    annee: Option<u32>,
    imdb: Option<String>,
    tmdb: Option<i64>,
    chemin: &'a Path,
    nouveau: bool,
    verrouille: bool,
    metadonnees: bool,
}

pub fn lister(
    app: &Application,
    s: &Sortie,
    nouveaux: bool,
    sans_meta: bool,
    recherche: Option<&str>,
    json: bool,
) -> ResultatCli {
    let marques: BTreeSet<String> = app.films_nouveaux()?.into_iter().collect();
    let motif = recherche.map(str::to_lowercase);
    let films: Vec<Film> = app
        .films()?
        .into_iter()
        .filter(|f| !nouveaux || marques.contains(&f.id))
        .filter(|f| !sans_meta || sans_metadonnees(f))
        .filter(|f| {
            motif.as_ref().is_none_or(|m| {
                f.titre.to_lowercase().contains(m) || f.titre_original.to_lowercase().contains(m)
            })
        })
        .collect();

    if json {
        let liste: Vec<FilmListe> = films
            .iter()
            .map(|f| {
                let imdb = f.ids.imdb();
                let tmdb = f.ids.tmdb();
                FilmListe {
                    id: &f.id,
                    titre: &f.titre,
                    titre_original: &f.titre_original,
                    annee: (f.annee > 0).then_some(f.annee),
                    imdb: (!imdb.is_empty()).then_some(imdb),
                    tmdb: (tmdb > 0).then_some(tmdb),
                    chemin: &f.chemin,
                    nouveau: marques.contains(&f.id),
                    verrouille: f.verrouille,
                    metadonnees: !sans_metadonnees(f),
                }
            })
            .collect();
        let texte = serde_json::to_string_pretty(&liste)
            .map_err(|e| ErreurCli::nouvelle(format!("sérialisation JSON : {e}")))?;
        println!("{texte}");
        return Ok(CodeSortie::Succes);
    }

    if films.is_empty() {
        s.info("Aucun film.");
        return Ok(CodeSortie::Succes);
    }
    let lignes: Vec<Vec<String>> = films
        .iter()
        .map(|f| {
            let mut ids = Vec::new();
            let imdb = f.ids.imdb();
            if !imdb.is_empty() {
                ids.push(imdb);
            }
            if f.ids.tmdb() > 0 {
                ids.push(format!("tmdb:{}", f.ids.tmdb()));
            }
            let mut etat = String::new();
            if marques.contains(&f.id) {
                etat.push('N');
            }
            if f.verrouille {
                etat.push('V');
            }
            vec![
                f.id.chars().take(LONGUEUR_ID_COURT).collect(),
                etat,
                tronquer(&f.titre, 50),
                if f.annee > 0 {
                    f.annee.to_string()
                } else {
                    String::new()
                },
                ids.join(" "),
                f.chemin.display().to_string(),
            ]
        })
        .collect();
    print!(
        "{}",
        tableau(
            &["ID", "État", "Titre", "Année", "Identifiants", "Dossier"],
            &lignes
        )
    );
    s.info(format!(
        "{} (N : nouveau, V : verrouillé)",
        pluriel(films.len(), "film", "films")
    ));
    Ok(CodeSortie::Succes)
}

// -- Sélection ------------------------------------------------------------------

/// Résout une sélection en identifiants de films (préfixes acceptés).
fn resoudre(app: &Application, cible: Cible) -> Result<Vec<String>, ErreurCli> {
    match cible {
        Cible::Tous => Ok(app.films()?.into_iter().map(|f| f.id).collect()),
        Cible::Nouveaux => {
            let existants: BTreeSet<String> = app.films()?.into_iter().map(|f| f.id).collect();
            Ok(app
                .films_nouveaux()?
                .into_iter()
                .filter(|id| existants.contains(id))
                .collect())
        }
        Cible::Ids(demandes) => {
            let tous: Vec<String> = app.films()?.into_iter().map(|f| f.id).collect();
            let mut retenus = Vec::new();
            for d in demandes {
                let correspondances: Vec<&String> = tous
                    .iter()
                    .filter(|id| id.starts_with(d.as_str()))
                    .collect();
                let exact = tous.iter().find(|id| **id == d);
                let unique = match correspondances.as_slice() {
                    [seul] => Some(*seul),
                    _ => None,
                };
                match exact.or(unique) {
                    Some(id) => {
                        if !retenus.contains(id) {
                            retenus.push(id.clone());
                        }
                    }
                    None if correspondances.is_empty() => {
                        return Err(ErreurCli::nouvelle(format!("film « {d} » introuvable")));
                    }
                    None => {
                        return Err(ErreurCli::nouvelle(format!(
                            "identifiant « {d} » ambigu ({} films correspondent)",
                            correspondances.len()
                        )));
                    }
                }
            }
            Ok(retenus)
        }
    }
}

fn titres(app: &Application) -> BTreeMap<String, String> {
    app.films()
        .unwrap_or_default()
        .into_iter()
        .map(|f| (f.id.clone(), libelle_film(&f)))
        .collect()
}

fn titre<'a>(t: &'a BTreeMap<String, String>, id: &'a str) -> &'a str {
    t.get(id).map(String::as_str).unwrap_or(id)
}

// -- Récupération des métadonnées ----------------------------------------------------

pub fn scraper(app: &Application, s: &Sortie, selection: &Selection) -> ResultatCli {
    // Vérification préalable : message clair, aucun accès réseau sans clé.
    if app.reglages().general.cle_api_tmdb.trim().is_empty() {
        return Err(mediamanager_app::ErreurApp::CleTmdbAbsente.into());
    }
    let ids = resoudre(app, selection.cible(Cible::Nouveaux))?;
    if ids.is_empty() {
        s.info("Aucun film à traiter.");
        return Ok(CodeSortie::Succes);
    }
    let config = ConfigMetadonnees::depuis_reglages(&app.reglages().films);
    let rapport = s.suivre(app.scraper_films(ids, config))?;
    let t = titres(app);
    println!(
        "Métadonnées : {} réussi(s), {} non trouvé(s), {} verrouillé(s), {} erreur(s)",
        rapport.reussis.len(),
        rapport.sans_resultat.len(),
        rapport.verrouilles.len(),
        rapport.erreurs.len()
    );
    for id in &rapport.reussis {
        s.detail(format!("  ok  {}", titre(&t, id)));
    }
    if !rapport.sans_resultat.is_empty() {
        println!("Films non trouvés :");
        for id in &rapport.sans_resultat {
            println!("  - {} [{}]", titre(&t, id), id);
        }
    }
    for (id, e) in &rapport.erreurs {
        eprintln!("erreur : {} : {e}", titre(&t, id));
    }
    if let Some(r) = &rapport.renommage {
        s.info(format!(
            "Renommage après récupération : {} renommé(s), {} échec(s)",
            r.renommes.len(),
            r.echecs.len()
        ));
    }
    Ok(CodeSortie::Succes)
}

// -- NFO --------------------------------------------------------------------------

pub fn nfo(app: &Application, s: &Sortie, selection: &Selection) -> ResultatCli {
    let ids = resoudre(app, selection.cible(Cible::Tous))?;
    if ids.is_empty() {
        s.info("Aucun film à traiter.");
        return Ok(CodeSortie::Succes);
    }
    let n = ids.len();
    let ecrits = s.suivre(app.ecrire_nfo_films(ids))?;
    println!(
        "NFO : {ecrits} fichier(s) écrit(s) ou mis à jour pour {}",
        pluriel(n, "film", "films")
    );
    Ok(CodeSortie::Succes)
}

// -- Renommage ------------------------------------------------------------------------

fn relatif(p: &Path, base: &Path) -> String {
    p.strip_prefix(base)
        .ok()
        .filter(|r| !r.as_os_str().is_empty())
        .unwrap_or(p)
        .display()
        .to_string()
}

fn afficher_element(e: &ElementPlan) {
    println!("« {} »", e.libelle);
    let base = &e.source_donnees;
    if e.ancien_dossier != e.nouveau_dossier {
        println!(
            "  dossier : {} → {}",
            relatif(&e.ancien_dossier, base),
            relatif(&e.nouveau_dossier, base)
        );
    }
    for l in &e.lignes {
        let ancien = relatif(&l.ancien, base);
        match l.action {
            ActionFichier::Inchange | ActionFichier::SuitDossier => {}
            ActionFichier::Supprime => println!("  supprimé : {ancien}"),
            ActionFichier::Deplace | ActionFichier::Copie => {
                let nouveaux: Vec<String> = l.nouveaux.iter().map(|n| relatif(n, base)).collect();
                let verbe = if l.action == ActionFichier::Copie {
                    "  copie   : "
                } else {
                    "  fichier : "
                };
                let doublon = if l.doublon { "  (doublon)" } else { "" };
                println!("{verbe}{ancien} → {}{doublon}", nouveaux.join(", "));
            }
        }
    }
    for c in &e.conflits {
        println!("  CONFLIT : {c}");
    }
    for a in &e.avertissements {
        println!("  remarque : {a}");
    }
}

fn afficher_apercu(plan: &PlanRenommage, s: &Sortie) -> (usize, usize) {
    let mut a_renommer = 0;
    let mut en_conflit = 0;
    for e in &plan.elements {
        let conflit = !e.est_executable();
        if e.est_inchange() && !conflit {
            continue;
        }
        if conflit {
            en_conflit += 1;
        } else {
            a_renommer += 1;
        }
        afficher_element(e);
    }
    if a_renommer + en_conflit > 0 {
        s.info(format!(
            "{} à renommer, {} en conflit.",
            pluriel(a_renommer, "film", "films"),
            en_conflit
        ));
    }
    (a_renommer, en_conflit)
}

fn confirmer(question: &str) -> Result<bool, ErreurCli> {
    print!("{question} [o/N] ");
    std::io::stdout()
        .flush()
        .map_err(|e| ErreurCli::nouvelle(e.to_string()))?;
    let mut reponse = String::new();
    let lus = std::io::stdin()
        .lock()
        .read_line(&mut reponse)
        .map_err(|e| ErreurCli::nouvelle(format!("lecture de la réponse : {e}")))?;
    if lus == 0 {
        println!();
    }
    Ok(matches!(
        reponse.trim().to_lowercase().as_str(),
        "o" | "oui" | "y" | "yes"
    ))
}

pub fn renommer(
    app: &Application,
    s: &Sortie,
    selection: &Selection,
    appliquer: bool,
    oui: bool,
) -> ResultatCli {
    let ids = resoudre(app, selection.cible(Cible::Tous))?;
    if ids.is_empty() {
        s.info("Aucun film à traiter.");
        return Ok(CodeSortie::Succes);
    }
    let plan = app.apercu_renommage_films(&ids)?;
    let (a_renommer, en_conflit) = afficher_apercu(&plan, s);
    let code_conflits = if en_conflit > 0 {
        CodeSortie::ConflitsRenommage
    } else {
        CodeSortie::Succes
    };
    if a_renommer == 0 {
        if en_conflit == 0 {
            s.info("Aucun renommage nécessaire.");
        }
        return Ok(code_conflits);
    }
    if !appliquer {
        s.info("Aperçu seulement : relancez avec --appliquer pour renommer.");
        return Ok(code_conflits);
    }
    if !oui {
        if !std::io::stdin().is_terminal() {
            s.info("(confirmation lue sur l'entrée standard ; --oui pour s'en passer)");
        }
        let question = format!("Renommer {} ?", pluriel(a_renommer, "film", "films"));
        if !confirmer(&question)? {
            s.info("Renommage abandonné.");
            return Ok(code_conflits);
        }
    }
    let rapport = s.suivre(app.renommer_films(ids))?;
    let t = titres(app);
    // Les éléments en conflit sont « ignorés » par le service : déjà signalés.
    let echecs_reels: Vec<&(String, String)> = rapport
        .echecs
        .iter()
        .filter(|(id, _)| {
            !plan.elements.iter().any(|e| {
                !e.est_executable()
                    && matches!(&e.nature, mediamanager_core::renommage::NatureElement::Film { id: x } if x == id)
            })
        })
        .collect();
    println!(
        "Renommage : {} renommé(s), {} inchangé(s), {} échec(s), {} en conflit non traité(s)",
        rapport.renommes.len(),
        rapport.inchanges.len(),
        echecs_reels.len(),
        en_conflit
    );
    for (id, e) in &echecs_reels {
        eprintln!("erreur : {} : {e}", titre(&t, id));
    }
    if !echecs_reels.is_empty() {
        return Ok(CodeSortie::Erreur);
    }
    Ok(code_conflits)
}

// -- Clé TMDB -----------------------------------------------------------------------

fn lire_cle(s: &Sortie) -> Result<String, ErreurCli> {
    if let Ok(v) = std::env::var(VARIABLE_CLE_TMDB)
        && !v.trim().is_empty()
    {
        s.detail(format!("Clé lue dans la variable {VARIABLE_CLE_TMDB}."));
        return Ok(v);
    }
    if std::io::stdin().is_terminal() {
        rpassword::prompt_password("Clé d'API TMDB (saisie masquée) : ")
            .map_err(|e| ErreurCli::nouvelle(format!("saisie de la clé : {e}")))
    } else {
        // Entrée redirigée (script) : première ligne de l'entrée standard.
        let mut ligne = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut ligne)
            .map_err(|e| ErreurCli::nouvelle(format!("lecture de la clé : {e}")))?;
        Ok(ligne)
    }
}

pub fn cle_tmdb(app: &Application, s: &Sortie, supprimer: bool) -> ResultatCli {
    if supprimer {
        app.definir_cle_tmdb("")?;
        s.info("Clé d'API TMDB effacée.");
        return Ok(CodeSortie::Succes);
    }
    let cle = lire_cle(s)?;
    let cle = cle.trim();
    if cle.is_empty() {
        return Err(ErreurCli::nouvelle("clé vide : rien n'a été enregistré"));
    }
    if cle.chars().any(char::is_whitespace) {
        return Err(ErreurCli::nouvelle(
            "la clé ne doit pas contenir d'espace : rien n'a été enregistré",
        ));
    }
    app.definir_cle_tmdb(cle)?;
    s.info("Clé d'API TMDB enregistrée.");
    Ok(CodeSortie::Succes)
}

// -- Informations -----------------------------------------------------------------------

pub fn info(app: &Application) -> ResultatCli {
    let d = app.dossiers();
    let r = app.reglages();
    let mode = match d.mode {
        ModeDossiers::Portable => "portable (à côté de l'exécutable)",
        ModeDossiers::Utilisateur => "dossiers utilisateur du système",
        ModeDossiers::Explicite => "imposé (--dossier ou MEDIAMANAGER_DOSSIER)",
    };
    let films = app.films()?.len();
    let series = app.series()?.len();
    let lignes = [
        ("Version", env!("CARGO_PKG_VERSION").to_string()),
        ("Mode", mode.to_string()),
        ("Racine", d.racine.display().to_string()),
        ("Données", d.donnees.display().to_string()),
        ("Cache", d.cache.display().to_string()),
        ("Journaux", d.journaux.display().to_string()),
        ("Sauvegardes", d.sauvegardes.display().to_string()),
        (
            "Films",
            format!("{films} (sources : {})", r.films.sources.len()),
        ),
        (
            "Séries",
            format!("{series} (sources : {})", r.series.sources.len()),
        ),
        (
            "Clé TMDB",
            if r.general.cle_api_tmdb.trim().is_empty() {
                "non configurée".to_string()
            } else {
                "configurée".to_string()
            },
        ),
    ];
    println!("MediaManagerRust {}", env!("CARGO_PKG_VERSION"));
    let largeur = lignes
        .iter()
        .map(|(e, _)| e.chars().count())
        .max()
        .unwrap_or(0);
    for (etiquette, valeur) in lignes.iter().skip(1) {
        let pad = " ".repeat(largeur - etiquette.chars().count());
        println!("  {etiquette}{pad} : {valeur}");
    }
    Ok(CodeSortie::Succes)
}
