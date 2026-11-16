//! Récupération des évaluations (notes) seules : seules les notes des
//! sources renvoyées par les fournisseurs sont remplacées ; le reste des
//! métadonnées, la note de l'utilisateur (`user`) et les notes des autres
//! sources ne sont jamais modifiés.
//!
//! Fournisseurs : TMDB (note `tmdb`, clé TMDB), puis ceux dont la clé est
//! enregistrée : OMDb (`imdb`, `rottenTomatoes`, `metacritic`), Trakt
//! (`trakt`) et MDBList (mêmes sources que ci-dessus). Pour une source
//! donnée, le premier fournisseur qui la renseigne l'emporte (ordre : TMDB,
//! OMDb, Trakt, MDBList). Une erreur d'un fournisseur optionnel (clé refusée,
//! limitation de débit, titre inconnu) est un simple avertissement : elle ne
//! fait pas échouer le film ni les autres sources, et un fournisseur dont la
//! clé est refusée ou limité n'est plus interrogé pendant le traitement.

use std::collections::{BTreeMap, BTreeSet};

use mediamanager_core::modele::Film;
use mediamanager_scrapers::{ErreurScraper, Evaluation, FournisseurFilm};

use crate::application::Noyau;
use crate::conversion::{options_recherche, vers_note};
use crate::edition_lot::actualiser_nfo_film;
use crate::erreur::{ErreurApp, Resultat};
use crate::evenements::{Evenement, NiveauMessage};
use crate::fournisseurs::{erreur_bloquante_pour_fournisseur, fournisseurs_evaluations};
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
    /// Avertissements des fournisseurs optionnels (clé refusée, limite
    /// atteinte, titre inconnu) : ne sont pas des échecs.
    pub avertissements: Vec<String>,
    /// Identifiants des fournisseurs interrogés (`tmdb`, `omdb`, `trakt`,
    /// `mdblist`).
    pub fournisseurs: Vec<String>,
}

fn a_un_identifiant(film: &Film) -> bool {
    film.ids.tmdb() > 0 || !film.ids.imdb().is_empty()
}

/// Fusionne les notes d'un fournisseur : une source déjà fournie plus tôt
/// dans le traitement n'est pas remplacée ; `user` et les valeurs invalides
/// sont ignorés.
fn fusionner(obtenues: &mut BTreeMap<String, Evaluation>, nouvelles: Vec<Evaluation>) {
    for e in nouvelles {
        if e.source.is_empty()
            || e.source == "user"
            || !e.valeur.is_finite()
            || e.valeur <= 0.0
            || obtenues.contains_key(&e.source)
        {
            continue;
        }
        obtenues.insert(e.source.clone(), e);
    }
}

/// Tâche : rafraîchit les notes d'une sélection de films. Le NFO existant
/// d'un film modifié est réécrit.
pub(crate) fn recuperer(
    n: &Noyau,
    ctx: &ContexteTache,
    ids: &[String],
) -> Resultat<RapportEvaluations> {
    let reglages = n.reglages();
    let tmdb = match fournisseur(n) {
        Ok(f) => Some(f),
        Err(ErreurApp::CleTmdbAbsente) => None,
        Err(e) => return Err(e),
    };
    let autres = fournisseurs_evaluations(n, false)?;
    if tmdb.is_none() && autres.is_empty() {
        return Err(ErreurApp::CleTmdbAbsente);
    }
    let mut rapport = RapportEvaluations::default();
    if tmdb.is_some() {
        rapport.fournisseurs.push("tmdb".into());
    }
    rapport
        .fournisseurs
        .extend(autres.iter().map(|f| f.identifiant().to_string()));
    // Fournisseurs optionnels désactivés pour le reste du traitement.
    let mut desactives: BTreeSet<&'static str> = BTreeSet::new();
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
        let mut o = options_recherche(&film, &reglages.films);
        let mut obtenues: BTreeMap<String, Evaluation> = BTreeMap::new();
        let mut erreur_tmdb: Option<String> = None;
        if let Some(f) = &tmdb {
            match ctx.bloquer(f.metadonnees(&o)) {
                Ok(m) => {
                    // Les identifiants du fournisseur (IMDb…) servent aux autres.
                    for (k, v) in &m.ids {
                        o.ids.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                    fusionner(&mut obtenues, m.evaluations);
                }
                Err(e @ ErreurScraper::CleApiInvalide) if autres.is_empty() => {
                    return Err(e.into());
                }
                Err(e) => erreur_tmdb = Some(e.to_string()),
            }
        }
        for f in &autres {
            if desactives.contains(f.identifiant()) {
                continue;
            }
            match ctx.bloquer(f.evaluations(&o)) {
                Ok(v) => fusionner(&mut obtenues, v),
                // Pas d'identifiant utilisable ou titre inconnu : normal.
                Err(ErreurScraper::IdentifiantManquant(_) | ErreurScraper::AucunResultat) => {}
                Err(e) => {
                    let msg = format!("{} : « {} » : {e}", f.identifiant(), film.titre);
                    if erreur_bloquante_pour_fournisseur(&e) {
                        desactives.insert(f.identifiant());
                        ctx.message(
                            NiveauMessage::Avertissement,
                            format!("{msg} (fournisseur ignoré pour la suite)"),
                        );
                    } else {
                        ctx.message(NiveauMessage::Avertissement, msg.clone());
                    }
                    rapport.avertissements.push(msg);
                }
            }
        }
        if obtenues.is_empty()
            && let Some(e) = erreur_tmdb
        {
            rapport.erreurs.push((id.clone(), e));
            continue;
        }
        let avant = film.notes.clone();
        for e in obtenues.values() {
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
