//! Nettoyage (« Nettoyer ») : repère les fichiers inutiles des dossiers de
//! films (motifs des réglages, extraits, miniatures système, NFO orphelins)
//! puis les déplace dans la corbeille de la source (`.corbeille`). Jamais de
//! suppression définitive, jamais en dehors du dossier du film.
//!
//! Sont ignorés par l'analyse comme par l'exécution : films verrouillés,
//! films sur disque (DVD/Blu-ray), dossiers partagés par plusieurs films ou
//! confondus avec la racine d'une source, films sans source connue.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use mediamanager_core::modele::{Film, TypeFichier};
use mediamanager_core::nettoyage::{RaisonInutile, ReglesNettoyage, analyser_dossier};

use crate::application::Noyau;
use crate::erreur::Resultat;
use crate::evenements::{Evenement, NiveauMessage};
use crate::gestion_films::mettre_a_la_corbeille;
use crate::sources::source_de;
use crate::taches::ContexteTache;

/// Fichier ou dossier inutile d'un film.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FichierInutile {
    pub id_film: String,
    /// Chemin absolu, sous le dossier du film.
    pub chemin: PathBuf,
    pub est_dossier: bool,
    pub raison: RaisonInutile,
    /// Taille en octets (récursive pour un dossier).
    pub taille: u64,
}

/// Bilan d'un nettoyage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapportNettoyage {
    /// Destinations dans la corbeille de la source.
    pub mis_a_la_corbeille: Vec<PathBuf>,
    /// (chemin, raison) : éléments refusés (hors dossier du film, protégés,
    /// film verrouillé ou introuvable…).
    pub ignores: Vec<(PathBuf, String)>,
    /// (chemin, message) : déplacements échoués.
    pub echecs: Vec<(PathBuf, String)>,
    /// Octets déplacés.
    pub octets: u64,
}

/// Fichiers du film à ne jamais toucher : tout ce que le film référence,
/// sauf les textes, les inconnus et les extraits.
fn fichiers_proteges(film: &Film) -> BTreeSet<PathBuf> {
    film.fichiers
        .iter()
        .filter(|f| {
            !matches!(
                f.type_fichier,
                TypeFichier::Texte | TypeFichier::Inconnu | TypeFichier::Sample
            )
        })
        .map(|f| f.chemin_complet())
        .collect()
}

/// Source du film si son dossier peut être nettoyé.
fn source_nettoyable(film: &Film, tous: &[Film], sources: &[PathBuf]) -> Option<PathBuf> {
    if film.verrouille
        || film.est_disque
        || film.dossier_multi_films
        || film.chemin.as_os_str().is_empty()
    {
        return None;
    }
    let source = source_de(&film.chemin, sources).cloned().or_else(|| {
        (!film.source_donnees.as_os_str().is_empty()
            && film.chemin.starts_with(&film.source_donnees))
        .then(|| film.source_donnees.clone())
    })?;
    if film.chemin == source {
        return None;
    }
    let partage = tous
        .iter()
        .any(|f| f.id != film.id && f.chemin.starts_with(&film.chemin));
    (!partage).then_some(source)
}

/// Analyse les dossiers des films choisis (lecture seule).
pub(crate) fn analyser(n: &Noyau, ids: &[String]) -> Resultat<Vec<FichierInutile>> {
    let reglages = n.reglages();
    let regles = ReglesNettoyage::nouvelles(&reglages.general.motifs_nettoyage);
    for m in &regles.invalides {
        tracing::warn!("motif de nettoyage invalide ignoré : {m}");
    }
    let tous = n.verrou().bibliotheque.lister::<Film>()?;
    let mut sortie = Vec::new();
    let mut vus = BTreeSet::new();
    for id in ids {
        if !vus.insert(id.clone()) {
            continue;
        }
        let Some(film) = tous.iter().find(|f| &f.id == id) else {
            continue;
        };
        if source_nettoyable(film, &tous, &reglages.films.sources).is_none() {
            continue;
        }
        let candidats = match analyser_dossier(
            &film.chemin,
            &fichiers_proteges(film),
            &regles,
            &reglages.general.extensions_video,
        ) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("nettoyage : {:?} illisible : {e}", film.chemin);
                continue;
            }
        };
        sortie.extend(candidats.into_iter().map(|c| FichierInutile {
            id_film: film.id.clone(),
            chemin: c.chemin,
            est_dossier: c.est_dossier,
            raison: c.raison,
            taille: c.taille,
        }));
    }
    Ok(sortie)
}

/// Vérifie qu'un chemin peut être mis à la corbeille : strictement sous le
/// dossier du film (liens symboliques résolus), ni lien symbolique, ni
/// fichier protégé ni dossier en contenant.
fn verifier_chemin(chemin: &Path, film: &Film) -> Result<(), String> {
    if chemin
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err("chemin non normalisé".into());
    }
    if chemin == film.chemin || !chemin.starts_with(&film.chemin) {
        return Err("hors du dossier du film".into());
    }
    // Les photos d'acteurs (`.actors`) ne sont jamais des fichiers inutiles.
    if chemin.strip_prefix(&film.chemin).is_ok_and(|r| {
        r.components()
            .any(|c| c.as_os_str() == crate::acteurs::DOSSIER_ACTEURS)
    }) {
        return Err("dossier des photos d'acteurs".into());
    }
    let meta = std::fs::symlink_metadata(chemin).map_err(|e| format!("introuvable : {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("lien symbolique".into());
    }
    let racine = film
        .chemin
        .canonicalize()
        .map_err(|e| format!("dossier du film illisible : {e}"))?;
    let reel = chemin
        .canonicalize()
        .map_err(|e| format!("chemin illisible : {e}"))?;
    if reel == racine || !reel.starts_with(&racine) {
        return Err("hors du dossier du film".into());
    }
    if fichiers_proteges(film)
        .iter()
        .any(|p| p == chemin || p.starts_with(chemin))
    {
        return Err("fichier référencé par le film".into());
    }
    Ok(())
}

/// Tâche : déplace les éléments vers la corbeille de leur source.
pub(crate) fn executer(
    n: &Noyau,
    ctx: &ContexteTache,
    liste: &[FichierInutile],
) -> Resultat<RapportNettoyage> {
    let reglages = n.reglages();
    let tous = n.verrou().bibliotheque.lister::<Film>()?;
    let mut rapport = RapportNettoyage::default();
    let mut par_film: BTreeMap<&str, Vec<&FichierInutile>> = BTreeMap::new();
    for f in liste {
        par_film.entry(f.id_film.as_str()).or_default().push(f);
    }
    let total = liste.len() as u64;
    let mut fait = 0u64;
    for (id, elements) in par_film {
        let Some(film) = tous.iter().find(|f| f.id == id) else {
            for e in elements {
                rapport
                    .ignores
                    .push((e.chemin.clone(), "film introuvable".into()));
            }
            continue;
        };
        let Some(source) = source_nettoyable(film, &tous, &reglages.films.sources) else {
            for e in elements {
                rapport.ignores.push((
                    e.chemin.clone(),
                    "film verrouillé, partagé ou sans source".into(),
                ));
            }
            continue;
        };
        let mut film = film.clone();
        let mut change = false;
        for e in elements {
            ctx.verifier_annulation()?;
            ctx.progression(fait, total, e.chemin.display().to_string());
            fait += 1;
            if let Err(raison) = verifier_chemin(&e.chemin, &film) {
                rapport.ignores.push((e.chemin.clone(), raison));
                continue;
            }
            let taille = e.taille;
            match mettre_a_la_corbeille(&e.chemin, &source) {
                Ok(dest) => {
                    rapport.mis_a_la_corbeille.push(dest);
                    rapport.octets += taille;
                    film.fichiers
                        .retain(|f| !f.chemin_complet().starts_with(&e.chemin));
                    change = true;
                }
                Err(err) => {
                    ctx.message(
                        NiveauMessage::Erreur,
                        format!("{} : {err}", e.chemin.display()),
                    );
                    rapport.echecs.push((e.chemin.clone(), err.to_string()));
                }
            }
        }
        if change {
            n.verrou().bibliotheque.enregistrer(&mut film)?;
            ctx.emettre(Evenement::FilmModifie {
                id: film.id.clone(),
            });
        }
    }
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}
