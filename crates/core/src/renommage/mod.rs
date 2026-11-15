//! Renommage et rangement des films et des épisodes par gabarits.
//!
//! Le renommage se fait en **deux temps** :
//!
//! 1. [`planifier_film`] / [`planifier_films`] / [`planifier_serie`]
//!    construisent un [`PlanRenommage`] sans rien écrire : opérations
//!    ordonnées (dossiers, déplacements, copies, suppressions), aperçu
//!    ancien → nouveau chemin de chaque fichier, conflits (gabarit invalide,
//!    destination existante, doublons) et fichiers du modèle après
//!    renommage. Seule une [`SondeDisque`] facultative lit le disque.
//! 2. [`executer`] applique le plan élément par élément, sans jamais
//!    écraser, avec repli copie + suppression entre volumes et retour
//!    arrière au mieux en cas d'échec. L'historique de chaque élément réussi
//!    ([`HistoriqueRenommage`](crate::modele::HistoriqueRenommage)) est
//!    renvoyé ; [`appliquer_au_film`] et [`appliquer_aux_episodes`] mettent
//!    ensuite le modèle à jour.
//!
//! ```no_run
//! use mediamanager_core::modele::Film;
//! use mediamanager_core::renommage::*;
//!
//! # let mut film = Film::default();
//! let profil = ProfilRenommageFilm::default();
//! let plan = planifier_films(&[&film], &[], &profil, &SondeSystemeFichiers);
//! if !plan.a_des_conflits() {
//!     let rapport = executer(&plan, &OptionsExecution::default());
//!     if rapport.est_complet() {
//!         appliquer_au_film(&mut film, &plan.elements[0]);
//!     }
//! }
//! ```

pub mod contexte;
pub mod execution;
pub mod film;
pub mod langues;
pub mod plan;
pub mod profil;
pub mod serie;
pub mod technique;

mod noms;

pub use contexte::{
    JETONS_FILM, JETONS_SERIE, contexte_episode, contexte_film, contexte_serie, jetons_film,
    jetons_serie, titre_triable,
};
pub use execution::{
    DOSSIER_CORBEILLE, EchecElement, EntreeJournal, ErreurExecution, OptionsExecution,
    RapportExecution, ResultatElement, StatutElement, executer, executer_element,
};
pub use film::{
    FilmARenommer, motifs_artwork, nom_dossier_film, nom_fichier_film, planifier_film,
    planifier_films,
};
pub use langues::{StyleLangue, ecrire_langue};
pub use plan::{
    ActionFichier, Conflit, ElementPlan, LigneFichier, NatureElement, Operation, PlanRenommage,
    SondeDisque, SondeListe, SondeSystemeFichiers, SondeVide,
};
pub use profil::{
    NomsFichiersEpisode, NomsFichiersFilm, ProfilNomme, ProfilRenommageFilm, ProfilRenommageSerie,
    ProfilsRenommage, StyleMulti,
};
pub use serie::{
    gabarits_recommandes, nom_dossier_saison, nom_dossier_serie, nom_fichier_episodes,
    planifier_episodes, planifier_serie,
};

use crate::modele::{Episode, Film, Serie};

/// Met à jour un film après l'exécution réussie de son élément de plan :
/// dossier, fichiers, mode multi-films et historique.
pub fn appliquer_au_film(film: &mut Film, element: &ElementPlan) {
    film.chemin = element.nouveau_dossier.clone();
    film.fichiers = element.fichiers_apres.clone();
    film.dossier_multi_films = element.dossier_multi_films;
    let h = element.historique();
    if h.ancien_chemin != h.nouveau_chemin || !h.fichiers.is_empty() {
        film.historique_renommage = Some(h);
    }
}

/// Met à jour une série après exécution : dossier de la série (élément de
/// nature `Serie`) et fichiers des épisodes (éléments `Episodes`).
/// `reussis` indique, par indice d'élément, ceux qui ont été appliqués.
pub fn appliquer_aux_episodes(serie: &mut Serie, plan: &PlanRenommage, reussis: &[bool]) {
    for (i, el) in plan.elements.iter().enumerate() {
        if !reussis.get(i).copied().unwrap_or(false) {
            continue;
        }
        match &el.nature {
            NatureElement::Serie { .. } => {
                let ancien = serie.chemin.clone();
                serie.chemin = el.nouveau_dossier.clone();
                serie.fichiers = el.fichiers_apres.clone();
                // Les fichiers des épisodes non renommés suivent le dossier.
                for e in serie.episodes.iter_mut() {
                    for f in e.fichiers.iter_mut() {
                        f.chemin = plan::reloger(&f.chemin, &ancien, &serie.chemin);
                    }
                }
            }
            NatureElement::Episodes { ids } => {
                for e in serie.episodes.iter_mut().filter(|e| ids.contains(&e.id)) {
                    mettre_a_jour_episode(e, el);
                }
            }
            NatureElement::Film { .. } => {}
        }
    }
}

fn mettre_a_jour_episode(e: &mut Episode, el: &ElementPlan) {
    e.fichiers = el.fichiers_apres.clone();
}

/// Résultats d'un rapport sous forme de drapeaux « réussi » par élément.
pub fn elements_reussis(rapport: &RapportExecution, nombre: usize) -> Vec<bool> {
    let mut v = vec![false; nombre];
    for r in &rapport.resultats {
        if let Some(x) = v.get_mut(r.indice) {
            *x = matches!(
                r.statut,
                StatutElement::Reussi { .. } | StatutElement::Inchange
            );
        }
    }
    v
}

#[cfg(test)]
mod tests;
