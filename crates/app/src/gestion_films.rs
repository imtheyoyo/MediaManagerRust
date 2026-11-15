//! Gestion des films de la bibliothèque : mise à jour d'une sélection,
//! films manquants d'une saga, retrait (avec corbeille), annulation du
//! dernier renommage.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mediamanager_core::modele::commun::cles_id;
use mediamanager_core::modele::{Film, Saga, TypeFichier};
use mediamanager_core::renommage::execution::{deplacer_dossier, deplacer_fichier};
use mediamanager_core::renommage::plan::relatif;
use mediamanager_core::scan::scanner_source;
use mediamanager_scrapers::{FournisseurCollection, OptionsRecherche, PartieCollection};

use crate::application::Noyau;
use crate::ecriture_nfo::{ecrire_nfo, saga_du_film};
use crate::erreur::{ErreurApp, Resultat};
use crate::evenements::{Evenement, NiveauMessage};
use crate::metadonnees::{charger_film, fournisseur};
use crate::sources::{
    self, CLE_FILMS_DISPARUS, CLE_FILMS_NOUVEAUX, RapportMiseAJour, TraitementDisparus,
    enregistrer_par_lots, fusionner_films, lire_marques, options_scan, retirer_des_sagas,
    source_de,
};
use crate::taches::ContexteTache;

/// Dossier de corbeille créé à la racine d'une source lors du retrait de
/// films avec leurs fichiers (dossier caché : ignoré par le scan).
pub const DOSSIER_CORBEILLE_SOURCE: &str = ".corbeille";

// ---------------------------------------------------------------------------
// Mise à jour d'une sélection
// ---------------------------------------------------------------------------

/// Tâche : mise à jour limitée aux dossiers des films choisis. La source est
/// analysée, mais seuls les films situés dans ces dossiers sont fusionnés,
/// ajoutés ou retirés ; le reste de la base n'est pas touché.
pub(crate) fn mettre_a_jour_selection(
    n: &Noyau,
    ctx: &ContexteTache,
    ids: &[String],
) -> Resultat<RapportMiseAJour> {
    let reglages = n.reglages();
    let sources = reglages.films.sources.clone();
    let opts = n.config.mise_a_jour.clone();
    let mut rapport = RapportMiseAJour::default();

    // Dossiers à analyser, regroupés par source.
    let mut par_source: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();
    for id in ids {
        let Some(f) = n.verrou().bibliotheque.obtenir::<Film>(id)? else {
            continue;
        };
        if f.verrouille {
            rapport.verrouilles += 1;
            continue;
        }
        match source_de(&f.chemin, &sources) {
            Some(s) => {
                par_source
                    .entry(s.clone())
                    .or_default()
                    .insert(f.chemin.clone());
            }
            None => rapport.avertissements.push(format!(
                "« {} » n'appartient à aucune source de données",
                f.titre
            )),
        }
    }

    let mut disparus_marques: BTreeSet<String> = lire_marques(&n.verrou(), CLE_FILMS_DISPARUS)?
        .into_iter()
        .collect();
    let mut nouveaux_marques: BTreeSet<String> = lire_marques(&n.verrou(), CLE_FILMS_NOUVEAUX)?
        .into_iter()
        .collect();
    let total = par_source.len() as u64;
    for (i, (source, dossiers)) in par_source.iter().enumerate() {
        ctx.verifier_annulation()?;
        ctx.progression(i as u64, total, format!("Analyse de {}", source.display()));
        let options = options_scan(
            &reglages.films.dossiers_ignores,
            reglages.films.ignorer_dossiers_nomedia,
            &reglages.films.mots_exclus,
            &reglages,
            source,
            &sources,
        );
        let scan = match scanner_source(source, &options) {
            Ok(r) => r,
            Err(e) => {
                ctx.message(NiveauMessage::Avertissement, e.to_string());
                rapport
                    .sources_indisponibles
                    .push((source.clone(), e.to_string()));
                continue;
            }
        };
        ctx.verifier_annulation()?;
        let scannes: Vec<Film> = scan
            .films
            .into_iter()
            .filter(|f| dossiers.contains(&f.chemin))
            .collect();

        let mut s = n.verrou();
        let existants: Vec<Film> = s
            .bibliotheque
            .lister::<Film>()?
            .into_iter()
            .filter(|f| dossiers.contains(&f.chemin))
            .collect();
        let ids_concernes: BTreeSet<String> = existants.iter().map(|f| f.id.clone()).collect();
        let mut fusion = fusionner_films(&existants, scannes);
        let supprimes = match opts.disparus {
            TraitementDisparus::Supprimer => fusion.disparus.clone(),
            TraitementDisparus::Marquer => Vec::new(),
        };
        enregistrer_par_lots(
            &mut s,
            &mut fusion.a_enregistrer,
            &supprimes,
            opts.taille_lot,
        )?;
        if !supprimes.is_empty() {
            retirer_des_sagas(&mut s, &supprimes)?;
        }
        disparus_marques.retain(|id| !ids_concernes.contains(id));
        nouveaux_marques.retain(|id| !supprimes.contains(id));
        nouveaux_marques.extend(fusion.nouveaux.iter().cloned());
        if opts.disparus == TraitementDisparus::Marquer {
            disparus_marques.extend(fusion.disparus.iter().cloned());
            rapport
                .marques_disparus
                .extend(fusion.disparus.iter().cloned());
        }
        drop(s);

        for id in &fusion.nouveaux {
            ctx.emettre(Evenement::FilmAjoute { id: id.clone() });
        }
        for id in &fusion.modifies {
            ctx.emettre(Evenement::FilmModifie { id: id.clone() });
        }
        for id in &supprimes {
            ctx.emettre(Evenement::FilmSupprime { id: id.clone() });
        }
        rapport.sources_traitees.push(source.clone());
        rapport.ajoutes.extend(fusion.nouveaux);
        rapport.modifies.extend(fusion.modifies);
        rapport.supprimes.extend(supprimes);
        rapport.verrouilles += fusion.verrouilles;
    }
    let mut s = n.verrou();
    sources::ecrire_marques(&mut s, CLE_FILMS_DISPARUS, &disparus_marques)?;
    sources::ecrire_marques(&mut s, CLE_FILMS_NOUVEAUX, &nouveaux_marques)?;
    drop(s);
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}

// ---------------------------------------------------------------------------
// Films manquants d'une saga
// ---------------------------------------------------------------------------

/// Identifiant de collection TMDB d'une saga (ou, à défaut, de ses films).
fn id_collection(n: &Noyau, saga: &Saga) -> Resultat<Option<String>> {
    if let Some(v) = saga.ids.obtenir(cles_id::TMDB_SAGA) {
        return Ok(Some(v.en_texte()));
    }
    let s = n.verrou();
    for id in &saga.films {
        if let Some(f) = s.bibliotheque.obtenir::<Film>(id)?
            && let Some(v) = f.ids.obtenir(cles_id::TMDB_SAGA)
        {
            return Ok(Some(v.en_texte()));
        }
    }
    Ok(None)
}

/// Tâche : parties de la collection du fournisseur absentes de la
/// bibliothèque (comparaison par identifiant TMDB, ordre du fournisseur).
pub(crate) fn films_manquants_saga(
    n: &Noyau,
    ctx: &ContexteTache,
    id_saga: &str,
) -> Resultat<Vec<PartieCollection>> {
    let saga = n
        .verrou()
        .bibliotheque
        .obtenir::<Saga>(id_saga)?
        .ok_or_else(|| ErreurApp::Introuvable(format!("saga {id_saga}")))?;
    let id = id_collection(n, &saga)?
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            ErreurApp::Introuvable(format!(
                "identifiant de collection TMDB de la saga « {} »",
                saga.titre
            ))
        })?;
    let reglages = n.reglages();
    let f = fournisseur(n)?;
    let o = OptionsRecherche {
        langue: reglages.films.langue_scraping.clone(),
        ..OptionsRecherche::default()
    }
    .avec_id(cles_id::TMDB_SAGA, id);
    ctx.progression(0, 1, format!("Collection « {} »", saga.titre));
    let collection = ctx.bloquer(f.collection(&o))?;
    let presents: BTreeSet<i64> = n
        .verrou()
        .bibliotheque
        .lister::<Film>()?
        .iter()
        .map(|f| f.ids.tmdb())
        .filter(|&t| t > 0)
        .collect();
    let manquants: Vec<PartieCollection> = collection
        .parties
        .into_iter()
        .filter(|p| {
            p.ids
                .get(cles_id::TMDB)
                .and_then(|v| v.trim().parse::<i64>().ok())
                .is_some_and(|t| !presents.contains(&t))
        })
        .collect();
    ctx.progression(1, 1, format!("{} film(s) manquant(s)", manquants.len()));
    Ok(manquants)
}

// ---------------------------------------------------------------------------
// Retrait
// ---------------------------------------------------------------------------

/// Bilan d'un retrait de films.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapportRetrait {
    /// Films retirés de la base.
    pub retires: Vec<String>,
    /// Chemins créés dans la corbeille de la source.
    pub mis_a_la_corbeille: Vec<PathBuf>,
    /// Films verrouillés, laissés intacts.
    pub verrouilles: Vec<String>,
    /// (identifiant, raison) : films non retirés.
    pub echecs: Vec<(String, String)>,
}

/// Premier chemin libre : `chemin`, sinon `chemin.1`, `chemin.2`…
fn chemin_libre(chemin: PathBuf) -> PathBuf {
    if !chemin.exists() {
        return chemin;
    }
    let nom = chemin
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    (1..)
        .map(|i| chemin.with_file_name(format!("{nom}.{i}")))
        .find(|c| !c.exists())
        .unwrap_or(chemin)
}

/// Déplace un fichier ou un dossier dans `<source>/.corbeille/<chemin
/// relatif>` (jamais de suppression définitive, jamais d'écrasement).
pub fn mettre_a_la_corbeille(chemin: &Path, source: &Path) -> Resultat<PathBuf> {
    let rel = chemin
        .strip_prefix(source)
        .map_err(|_| ErreurApp::io(chemin, format!("hors de la source {}", source.display())))?;
    let destination = chemin_libre(source.join(DOSSIER_CORBEILLE_SOURCE).join(rel));
    if let Some(p) = destination.parent() {
        std::fs::create_dir_all(p).map_err(|e| ErreurApp::io(p, e))?;
    }
    let r = if chemin.is_dir() {
        deplacer_dossier(chemin, &destination, false)
    } else {
        deplacer_fichier(chemin, &destination, false)
    };
    r.map_err(|e| ErreurApp::io(chemin, e))?;
    Ok(destination)
}

/// Met les fichiers d'un film à la corbeille : le dossier entier s'il n'est
/// partagé avec aucun autre film, sinon les fichiers du film un par un.
fn fichiers_a_la_corbeille(film: &Film, source: &Path, tous: &[Film]) -> Resultat<Vec<PathBuf>> {
    let partage = film.dossier_multi_films
        || film.chemin == source
        || tous
            .iter()
            .any(|f| f.id != film.id && f.chemin.starts_with(&film.chemin));
    if !partage && film.chemin.is_dir() {
        return Ok(vec![mettre_a_la_corbeille(&film.chemin, source)?]);
    }
    let mut v = Vec::new();
    for f in &film.fichiers {
        let c = f.chemin_complet();
        if c.exists() {
            v.push(mettre_a_la_corbeille(&c, source)?);
        }
    }
    Ok(v)
}

/// Tâche : retire des films de la base ; avec `supprimer_fichiers`, leurs
/// fichiers sont déplacés dans la corbeille de leur source.
pub(crate) fn retirer_films(
    n: &Noyau,
    ctx: &ContexteTache,
    ids: &[String],
    supprimer_fichiers: bool,
) -> Resultat<RapportRetrait> {
    let reglages = n.reglages();
    let sources = reglages.films.sources.clone();
    let mut rapport = RapportRetrait::default();
    let tous = n.verrou().bibliotheque.lister::<Film>()?;
    let total = ids.len() as u64;
    for (i, id) in ids.iter().enumerate() {
        ctx.verifier_annulation()?;
        let Some(film) = tous.iter().find(|f| &f.id == id) else {
            rapport
                .echecs
                .push((id.clone(), format!("film {id} introuvable")));
            continue;
        };
        ctx.progression(i as u64, total, film.titre.clone());
        if film.verrouille {
            rapport.verrouilles.push(id.clone());
            continue;
        }
        if supprimer_fichiers && !film.chemin.as_os_str().is_empty() {
            let source = source_de(&film.chemin, &sources).cloned().or_else(|| {
                (!film.source_donnees.as_os_str().is_empty()
                    && film.chemin.starts_with(&film.source_donnees))
                .then(|| film.source_donnees.clone())
            });
            let Some(source) = source else {
                rapport
                    .echecs
                    .push((id.clone(), "source de données inconnue".into()));
                continue;
            };
            match fichiers_a_la_corbeille(film, &source, &tous) {
                Ok(v) => rapport.mis_a_la_corbeille.extend(v),
                Err(e) => {
                    ctx.message(NiveauMessage::Erreur, format!("{} : {e}", film.titre));
                    rapport.echecs.push((id.clone(), e.to_string()));
                    continue;
                }
            }
        }
        let mut s = n.verrou();
        s.bibliotheque.supprimer::<Film>(id)?;
        retirer_des_sagas(&mut s, std::slice::from_ref(id))?;
        drop(s);
        ctx.emettre(Evenement::FilmSupprime { id: id.clone() });
        rapport.retires.push(id.clone());
    }
    if !rapport.retires.is_empty() {
        let mut s = n.verrou();
        for cle in [CLE_FILMS_NOUVEAUX, CLE_FILMS_DISPARUS] {
            let mut m: BTreeSet<String> = lire_marques(&s, cle)?.into_iter().collect();
            m.retain(|x| !rapport.retires.contains(x));
            sources::ecrire_marques(&mut s, cle, &m)?;
        }
    }
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}

// ---------------------------------------------------------------------------
// Annulation du dernier renommage
// ---------------------------------------------------------------------------

fn conflit(chemin: &Path, message: &str) -> ErreurApp {
    ErreurApp::io(chemin, message)
}

/// Tâche : annule le dernier renommage d'un film d'après son historique.
///
/// Les fichiers renommés reprennent leur ancien nom, puis le dossier revient
/// à son ancien emplacement (fichiers déplacés un à un si le dossier est
/// partagé). Toutes les destinations sont vérifiées avant la première
/// opération ; rien n'est jamais écrasé. Les copies supplémentaires créées
/// par le renommage restent en place et les fichiers supprimés alors (mis à
/// la corbeille du renommage) ne sont pas restaurés.
pub(crate) fn annuler_renommage(n: &Noyau, ctx: &ContexteTache, id: &str) -> Resultat<Film> {
    let mut film = charger_film(n, id)?;
    if film.verrouille {
        return Err(ErreurApp::Verrouille(film.titre));
    }
    let h = film.historique_renommage.clone().ok_or_else(|| {
        ErreurApp::Introuvable(format!("historique de renommage de « {} »", film.titre))
    })?;
    let dossier = h.nouveau_chemin.clone();
    let ancien = h.ancien_chemin.clone();
    if film.chemin != dossier {
        return Err(conflit(
            &film.chemin,
            "le film a été déplacé depuis son dernier renommage",
        ));
    }
    ctx.progression(
        0,
        2,
        format!("Annulation du renommage de « {} »", film.titre),
    );

    // Renommages à défaire (dans le dossier actuel).
    let renommages: Vec<(PathBuf, PathBuf)> = h
        .fichiers
        .iter()
        .map(|(a, nv)| (dossier.join(nv), dossier.join(a)))
        .filter(|(de, _)| de.exists())
        .collect();
    let sources_renommees: BTreeSet<&PathBuf> = renommages.iter().map(|(de, _)| de).collect();
    for (_, vers) in &renommages {
        if vers.exists() && !sources_renommees.contains(vers) {
            return Err(conflit(vers, "la destination existe déjà"));
        }
    }
    let table: BTreeMap<String, String> = h
        .fichiers
        .iter()
        .map(|(a, nv)| (nv.clone(), a.clone()))
        .collect();

    // Déplacement du dossier.
    let deplacer = ancien != dossier;
    let partage = deplacer
        && n.verrou()
            .bibliotheque
            .lister::<Film>()?
            .iter()
            .any(|f| f.id != film.id && f.chemin == dossier);
    // Chemins finaux (relatifs au dossier, après dérenommage).
    let finaux: Vec<String> = film
        .fichiers
        .iter()
        .map(|f| {
            let rel = relatif(&f.chemin_complet(), &dossier);
            table.get(&rel).cloned().unwrap_or(rel)
        })
        .collect();
    if deplacer {
        if partage {
            for rel in &finaux {
                let cible = ancien.join(rel);
                if cible.exists() {
                    return Err(conflit(&cible, "la destination existe déjà"));
                }
            }
        } else if ancien.exists() {
            return Err(conflit(&ancien, "l'ancien dossier existe déjà"));
        }
    }

    // 1. Noms d'origine (en deux temps : permutations possibles).
    let mut temporaires = Vec::new();
    for (k, (de, vers)) in renommages.iter().enumerate() {
        let tmp = de.with_file_name(format!(".annulation-{k}-{}", std::process::id()));
        deplacer_fichier(de, &tmp, false).map_err(|e| ErreurApp::io(de, e))?;
        temporaires.push((tmp, vers.clone()));
    }
    for (tmp, vers) in &temporaires {
        if let Some(p) = vers.parent() {
            std::fs::create_dir_all(p).map_err(|e| ErreurApp::io(p, e))?;
        }
        deplacer_fichier(tmp, vers, false).map_err(|e| ErreurApp::io(vers, e))?;
    }
    ctx.progression(1, 2, "Dossier");

    // 2. Dossier d'origine.
    if deplacer {
        if partage {
            for rel in &finaux {
                let de = dossier.join(rel);
                if !de.exists() {
                    continue;
                }
                let vers = ancien.join(rel);
                if let Some(p) = vers.parent() {
                    std::fs::create_dir_all(p).map_err(|e| ErreurApp::io(p, e))?;
                }
                deplacer_fichier(&de, &vers, false).map_err(|e| ErreurApp::io(&de, e))?;
            }
        } else {
            if let Some(p) = ancien.parent() {
                std::fs::create_dir_all(p).map_err(|e| ErreurApp::io(p, e))?;
            }
            deplacer_dossier(&dossier, &ancien, false).map_err(|e| ErreurApp::io(&dossier, e))?;
        }
    }

    // 3. Modèle.
    for (f, rel) in film.fichiers.iter_mut().zip(&finaux) {
        let complet = ancien.join(rel);
        f.chemin = complet.parent().map(Path::to_path_buf).unwrap_or_default();
        f.nom = complet
            .file_name()
            .map(|x| x.to_string_lossy().into_owned())
            .unwrap_or_default();
    }
    film.chemin = ancien;
    film.historique_renommage = None;
    if film.fichiers_de_type(TypeFichier::Nfo).next().is_some() {
        let reglages = n.reglages();
        let saga = saga_du_film(n, &film)?;
        if let Err(e) = ecrire_nfo(
            &mut film,
            saga.as_ref(),
            &reglages,
            &n.config.nommage_nfo_film,
            n.config.horodatage_nfo,
        ) {
            ctx.message(NiveauMessage::Avertissement, e.to_string());
        }
    }
    n.verrou().bibliotheque.enregistrer(&mut film)?;
    ctx.emettre(Evenement::FilmModifie {
        id: film.id.clone(),
    });
    ctx.progression(2, 2, "Terminé");
    Ok(film)
}
