//! Récupération des évaluations (notes) seules : seules les notes des
//! sources renvoyées par le fournisseur sont remplacées ; le reste des
//! métadonnées, la note de l'utilisateur et les notes des autres sources ne
//! sont jamais modifiés.
//!
//! Fournisseur utilisé : TMDB. Aucun fournisseur de notes IMDb ou Trakt n'est
//! disponible dans le crate `scrapers` (il ne gère que leurs identifiants) :
//! leurs notes déjà présentes en base sont donc conservées telles quelles.

use mediamanager_core::modele::Film;
use mediamanager_scrapers::{ErreurScraper, FournisseurFilm};

use crate::application::Noyau;
use crate::conversion::{options_recherche, vers_note};
use crate::edition_lot::actualiser_nfo_film;
use crate::erreur::{ErreurApp, Resultat};
use crate::evenements::Evenement;
use crate::metadonnees::{charger_film, fournisseur};
use crate::taches::ContexteTache;

/// Bilan d'une récupération des évaluations.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapportEvaluations {
    /// Films dont une note a changé.
    pub mis_a_jour: Vec<String>,
    /// Films dont les notes sont déjà à jour.
    pub inchanges: Vec<String>,
    /// Films sans identifiant TMDB ni IMDb (aucune recherche tentée).
    pub sans_identifiant: Vec<String>,
    pub verrouilles: Vec<String>,
    /// (identifiant, message d'erreur).
    pub erreurs: Vec<(String, String)>,
}

fn a_un_identifiant(film: &Film) -> bool {
    film.ids.tmdb() > 0 || !film.ids.imdb().is_empty()
}

/// Tâche : rafraîchit les notes d'une sélection de films. Le NFO existant
/// d'un film modifié est réécrit.
pub(crate) fn recuperer(
    n: &Noyau,
    ctx: &ContexteTache,
    ids: &[String],
) -> Resultat<RapportEvaluations> {
    let reglages = n.reglages();
    let f = fournisseur(n)?;
    let mut rapport = RapportEvaluations::default();
    let total = ids.len() as u64;
    for (i, id) in ids.iter().enumerate() {
        ctx.verifier_annulation()?;
        let mut film = match charger_film(n, id) {
            Ok(f) => f,
            Err(e) => {
                rapport.erreurs.push((id.clone(), e.to_string()));
                continue;
            }
        };
        ctx.progression(i as u64, total, film.titre.clone());
        if film.verrouille {
            rapport.verrouilles.push(id.clone());
            continue;
        }
        if !a_un_identifiant(&film) {
            rapport.sans_identifiant.push(id.clone());
            continue;
        }
        let o = options_recherche(&film, &reglages.films);
        let m = match ctx.bloquer(f.metadonnees(&o)) {
            Ok(m) => m,
            Err(e @ ErreurScraper::CleApiInvalide) => return Err(e.into()),
            Err(e) => {
                rapport.erreurs.push((id.clone(), e.to_string()));
                continue;
            }
        };
        let avant = film.notes.clone();
        for e in &m.evaluations {
            film.notes.insert(e.source.clone(), vers_note(e));
        }
        if film.notes == avant {
            rapport.inchanges.push(id.clone());
            continue;
        }
        if let Err(e) = actualiser_nfo_film(n, &mut film, &reglages) {
            rapport.erreurs.push((id.clone(), format!("NFO : {e}")));
        }
        match n.verrou().bibliotheque.enregistrer(&mut film) {
            Ok(_) => {
                ctx.emettre(Evenement::FilmModifie { id: id.clone() });
                rapport.mis_a_jour.push(id.clone());
            }
            Err(e) => rapport
                .erreurs
                .push((id.clone(), ErreurApp::from(e).to_string())),
        }
    }
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}
