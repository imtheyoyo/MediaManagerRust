//! Édition globale : applique les mêmes modifications à plusieurs films ou
//! séries, avec enregistrement, réécriture des NFO existants et événements.
//!
//! Les éléments verrouillés sont ignorés (comme pour le scraping).

use mediamanager_core::edition_lot::{ModificationsLot, ModificationsLotSeries};
use mediamanager_core::modele::{Film, Serie, TypeFichier};
use mediamanager_store::Reglages;

use crate::application::Noyau;
use crate::ecriture_nfo::{ecrire_nfo, saga_du_film};
use crate::erreur::Resultat;
use crate::evenements::{Evenement, NiveauMessage};
use crate::series::ecrire_nfo_serie_complete;
use crate::taches::ContexteTache;

/// Bilan d'une édition globale.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapportLot {
    pub modifies: Vec<String>,
    /// Déjà conformes aux modifications demandées.
    pub inchanges: Vec<String>,
    pub verrouilles: Vec<String>,
    pub introuvables: Vec<String>,
    /// (identifiant, message).
    pub erreurs: Vec<(String, String)>,
}

/// Réécrit les NFO d'un film s'il en a déjà.
pub(crate) fn actualiser_nfo_film(n: &Noyau, film: &mut Film, reglages: &Reglages) -> Resultat<()> {
    let a_nfo = !film.chemin.as_os_str().is_empty()
        && film
            .fichiers
            .iter()
            .any(|f| f.type_fichier == TypeFichier::Nfo);
    if a_nfo {
        let saga = saga_du_film(n, film)?;
        ecrire_nfo(
            film,
            saga.as_ref(),
            reglages,
            &n.config.nommage_nfo_film,
            n.config.horodatage_nfo,
        )?;
    }
    Ok(())
}

/// Réécrit les NFO d'une série s'il en a déjà.
pub(crate) fn actualiser_nfo_serie(
    n: &Noyau,
    serie: &mut Serie,
    reglages: &Reglages,
) -> Resultat<()> {
    let a_nfo = !serie.chemin.as_os_str().is_empty()
        && (serie
            .fichiers
            .iter()
            .any(|f| f.type_fichier == TypeFichier::Nfo)
            || serie
                .episodes
                .iter()
                .flat_map(|e| e.fichiers.iter())
                .any(|f| f.type_fichier == TypeFichier::Nfo));
    if a_nfo {
        ecrire_nfo_serie_complete(
            serie,
            reglages,
            n.config.horodatage_nfo,
            n.config.nommage_nfo_saison,
        )?;
    }
    Ok(())
}

/// Tâche : édition globale de films.
pub(crate) fn modifier_films(
    n: &Noyau,
    ctx: &ContexteTache,
    ids: &[String],
    modifs: &ModificationsLot,
) -> Resultat<RapportLot> {
    let reglages = n.reglages();
    let mut rapport = RapportLot::default();
    let total = ids.len() as u64;
    let mut vus = std::collections::BTreeSet::new();
    for (i, id) in ids.iter().enumerate() {
        ctx.verifier_annulation()?;
        if !vus.insert(id.as_str()) {
            continue;
        }
        let Some(mut film) = n.verrou().bibliotheque.obtenir::<Film>(id)? else {
            rapport.introuvables.push(id.clone());
            continue;
        };
        ctx.progression(i as u64, total, film.titre.clone());
        if film.verrouille {
            rapport.verrouilles.push(id.clone());
            continue;
        }
        if !modifs.appliquer(&mut film) {
            rapport.inchanges.push(id.clone());
            continue;
        }
        if let Err(e) = actualiser_nfo_film(n, &mut film, &reglages) {
            ctx.message(
                NiveauMessage::Avertissement,
                format!("NFO de « {} » : {e}", film.titre),
            );
            rapport.erreurs.push((id.clone(), format!("NFO : {e}")));
        }
        n.verrou().bibliotheque.enregistrer(&mut film)?;
        ctx.emettre(Evenement::FilmModifie { id: id.clone() });
        rapport.modifies.push(id.clone());
    }
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}

/// Tâche : édition globale de séries.
pub(crate) fn modifier_series(
    n: &Noyau,
    ctx: &ContexteTache,
    ids: &[String],
    modifs: &ModificationsLotSeries,
) -> Resultat<RapportLot> {
    let reglages = n.reglages();
    let mut rapport = RapportLot::default();
    let total = ids.len() as u64;
    let mut vus = std::collections::BTreeSet::new();
    for (i, id) in ids.iter().enumerate() {
        ctx.verifier_annulation()?;
        if !vus.insert(id.as_str()) {
            continue;
        }
        let Some(mut serie) = n.verrou().bibliotheque.obtenir::<Serie>(id)? else {
            rapport.introuvables.push(id.clone());
            continue;
        };
        ctx.progression(i as u64, total, serie.titre.clone());
        if serie.verrouille {
            rapport.verrouilles.push(id.clone());
            continue;
        }
        if !modifs.appliquer(&mut serie) {
            rapport.inchanges.push(id.clone());
            continue;
        }
        if let Err(e) = actualiser_nfo_serie(n, &mut serie, &reglages) {
            ctx.message(
                NiveauMessage::Avertissement,
                format!("NFO de « {} » : {e}", serie.titre),
            );
            rapport.erreurs.push((id.clone(), format!("NFO : {e}")));
        }
        n.verrou().bibliotheque.enregistrer(&mut serie)?;
        ctx.emettre(Evenement::SerieModifiee { id: id.clone() });
        rapport.modifies.push(id.clone());
    }
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}
