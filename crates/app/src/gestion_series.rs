//! Gestion des séries : état « vu » (série, saison ou épisodes) et retrait
//! de la base avec mise des fichiers à la corbeille de la source.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use mediamanager_core::edition_lot::marquer_vu;
use mediamanager_core::modele::Serie;

use crate::application::Noyau;
use crate::edition_lot::actualiser_nfo_serie;
use crate::erreur::Resultat;
use crate::evenements::{Evenement, NiveauMessage};
use crate::gestion_films::{RapportRetrait, mettre_a_la_corbeille};
use crate::sources::{self, CLE_SERIES_DISPARUES, CLE_SERIES_NOUVELLES, lire_marques, source_de};
use crate::taches::ContexteTache;

/// Cible d'un changement d'état « vu ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CibleVu {
    /// Tous les épisodes (avec fichier) d'une série.
    Serie(String),
    /// Certains épisodes d'une série (identifiants d'épisodes).
    Episodes {
        id_serie: String,
        ids_episodes: Vec<String>,
    },
    /// Tous les épisodes d'une saison (numéro dans l'ordre actif).
    Saison { id_serie: String, saison: i32 },
}

/// Bilan d'un changement d'état « vu ».
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapportVu {
    /// État appliqué (`true` = vu).
    pub vu: bool,
    pub series_modifiees: Vec<String>,
    /// Épisodes dont l'état a changé.
    pub episodes_modifies: usize,
    pub introuvables: Vec<String>,
    /// (identifiant de série, message) : NFO non réécrits…
    pub erreurs: Vec<(String, String)>,
}

fn indices_cible(serie: &Serie, cible: &CibleVu) -> Vec<usize> {
    let groupe = &serie.groupe_actif;
    serie
        .episodes
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.est_fictif())
        .filter(|(_, e)| match cible {
            CibleVu::Serie(_) => true,
            CibleVu::Episodes { ids_episodes, .. } => ids_episodes.contains(&e.id),
            CibleVu::Saison { saison, .. } => e.saison(groupe) == *saison,
        })
        .map(|(i, _)| i)
        .collect()
}

fn id_serie_cible(c: &CibleVu) -> &str {
    match c {
        CibleVu::Serie(id) => id,
        CibleVu::Episodes { id_serie, .. } | CibleVu::Saison { id_serie, .. } => id_serie,
    }
}

/// Change l'état « vu ». `etat = None` : bascule — tout devient vu, sauf si
/// tous les épisodes visés l'étaient déjà (alors tout devient non vu). Les
/// séries verrouillées sont acceptées : l'état « vu » est une donnée de
/// l'utilisateur, pas une métadonnée du fournisseur. Renvoie le rapport et
/// les événements à publier.
pub(crate) fn basculer_vu(
    n: &Noyau,
    cibles: &[CibleVu],
    etat: Option<bool>,
) -> Resultat<(RapportVu, Vec<Evenement>)> {
    let reglages = n.reglages();
    let mut rapport = RapportVu::default();
    let mut evenements = Vec::new();
    // Série → indices d'épisodes visés (dédoublonnés).
    let mut series: BTreeMap<String, (Serie, BTreeSet<usize>)> = BTreeMap::new();
    for c in cibles {
        let id = id_serie_cible(c);
        if !series.contains_key(id) {
            match n.verrou().bibliotheque.obtenir::<Serie>(id)? {
                Some(s) => {
                    series.insert(id.to_string(), (s, BTreeSet::new()));
                }
                None => {
                    if !rapport.introuvables.iter().any(|x| x == id) {
                        rapport.introuvables.push(id.to_string());
                    }
                    continue;
                }
            }
        }
        if let Some((s, set)) = series.get_mut(id) {
            set.extend(indices_cible(s, c));
        }
    }
    let nouvel_etat = etat.unwrap_or_else(|| {
        let tous_vus = series
            .values()
            .flat_map(|(s, set)| set.iter().map(|&i| s.episodes[i].vu))
            .all(|v| v);
        let au_moins_un = series.values().any(|(_, set)| !set.is_empty());
        !(au_moins_un && tous_vus)
    });
    rapport.vu = nouvel_etat;
    for (id, (mut serie, indices)) in series {
        let mut modifies = 0;
        for i in indices {
            let e = &mut serie.episodes[i];
            if marquer_vu(
                nouvel_etat,
                &mut e.vu,
                &mut e.nombre_lectures,
                &mut e.derniere_lecture,
            ) {
                modifies += 1;
            }
        }
        if modifies == 0 {
            continue;
        }
        serie.derniere_lecture = serie
            .episodes
            .iter()
            .filter_map(|e| e.derniere_lecture)
            .max();
        if let Err(e) = actualiser_nfo_serie(n, &mut serie, &reglages) {
            rapport.erreurs.push((id.clone(), format!("NFO : {e}")));
        }
        n.verrou().bibliotheque.enregistrer(&mut serie)?;
        rapport.episodes_modifies += modifies;
        rapport.series_modifiees.push(id.clone());
        evenements.push(Evenement::SerieModifiee { id });
    }
    Ok((rapport, evenements))
}

/// Fichiers à mettre à la corbeille : le dossier entier s'il n'est partagé
/// avec aucune autre série, sinon les fichiers de la série un par un.
fn fichiers_a_la_corbeille(
    serie: &Serie,
    source: &std::path::Path,
    toutes: &[Serie],
) -> Resultat<Vec<PathBuf>> {
    let partage = serie.chemin == source
        || toutes.iter().any(|s| {
            s.id != serie.id
                && (s.chemin.starts_with(&serie.chemin) || serie.chemin.starts_with(&s.chemin))
        });
    if !partage && serie.chemin.is_dir() {
        return Ok(vec![mettre_a_la_corbeille(&serie.chemin, source)?]);
    }
    let mut chemins: BTreeSet<PathBuf> = BTreeSet::new();
    for f in serie
        .fichiers
        .iter()
        .chain(serie.saisons.iter().flat_map(|s| s.fichiers.iter()))
        .chain(serie.episodes.iter().flat_map(|e| e.fichiers.iter()))
    {
        chemins.insert(f.chemin_complet());
    }
    let mut v = Vec::new();
    for c in chemins {
        if c.exists() && c.starts_with(&serie.chemin) {
            v.push(mettre_a_la_corbeille(&c, source)?);
        }
    }
    Ok(v)
}

/// Tâche : retire des séries de la base ; avec `supprimer_fichiers`, leurs
/// fichiers vont dans `<source>/.corbeille`. Séries verrouillées refusées.
pub(crate) fn retirer_series(
    n: &Noyau,
    ctx: &ContexteTache,
    ids: &[String],
    supprimer_fichiers: bool,
) -> Resultat<RapportRetrait> {
    let reglages = n.reglages();
    let sources = reglages.series.sources.clone();
    let mut rapport = RapportRetrait::default();
    let toutes = n.verrou().bibliotheque.lister::<Serie>()?;
    let total = ids.len() as u64;
    for (i, id) in ids.iter().enumerate() {
        ctx.verifier_annulation()?;
        let Some(serie) = toutes.iter().find(|s| &s.id == id) else {
            rapport
                .echecs
                .push((id.clone(), format!("série {id} introuvable")));
            continue;
        };
        ctx.progression(i as u64, total, serie.titre.clone());
        if serie.verrouille {
            rapport.verrouilles.push(id.clone());
            continue;
        }
        if supprimer_fichiers && !serie.chemin.as_os_str().is_empty() {
            let source = source_de(&serie.chemin, &sources).cloned().or_else(|| {
                (!serie.source_donnees.as_os_str().is_empty()
                    && serie.chemin.starts_with(&serie.source_donnees))
                .then(|| serie.source_donnees.clone())
            });
            let Some(source) = source else {
                rapport
                    .echecs
                    .push((id.clone(), "source de données inconnue".into()));
                continue;
            };
            match fichiers_a_la_corbeille(serie, &source, &toutes) {
                Ok(v) => rapport.mis_a_la_corbeille.extend(v),
                Err(e) => {
                    ctx.message(NiveauMessage::Erreur, format!("{} : {e}", serie.titre));
                    rapport.echecs.push((id.clone(), e.to_string()));
                    continue;
                }
            }
        }
        n.verrou().bibliotheque.supprimer::<Serie>(id)?;
        ctx.emettre(Evenement::SerieSupprimee { id: id.clone() });
        rapport.retires.push(id.clone());
    }
    if !rapport.retires.is_empty() {
        let mut s = n.verrou();
        for cle in [CLE_SERIES_NOUVELLES, CLE_SERIES_DISPARUES] {
            let mut m: BTreeSet<String> = lire_marques(&s, cle)?.into_iter().collect();
            m.retain(|x| !rapport.retires.contains(x));
            sources::ecrire_marques(&mut s, cle, &m)?;
        }
    }
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}
