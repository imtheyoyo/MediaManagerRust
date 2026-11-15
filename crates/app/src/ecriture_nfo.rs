//! Écriture des NFO de films selon les réglages.

use mediamanager_core::modele::{Film, Saga, TypeFichier};
use mediamanager_core::nfo::{
    Connecteur, DonneesConservees, NommageNfoFilm, OptionsNfo, ecrire_nfo_film, ecrire_si_change,
    horodatage_courant, lire_fichier_nfo_film, noms_nfo_film,
};
use mediamanager_store::reglages::FormatNfo;
use mediamanager_store::{Reglages, ReglagesFilms};

use crate::application::Noyau;
use crate::artwork::ajouter_fichier;
use crate::erreur::{ErreurApp, Resultat};
use crate::evenements::Evenement;
use crate::taches::ContexteTache;

/// Connecteur NFO correspondant au format des réglages.
pub fn connecteur(format: FormatNfo) -> Connecteur {
    match format {
        FormatNfo::Kodi => Connecteur::Kodi,
        FormatNfo::Jellyfin => Connecteur::Jellyfin,
        FormatNfo::Emby => Connecteur::Emby,
        FormatNfo::Plex => Connecteur::Plex,
    }
}

/// Options d'écriture des NFO de films d'après les réglages.
pub fn options_nfo_films(r: &ReglagesFilms, horodatage: bool) -> OptionsNfo {
    OptionsNfo {
        sources_notes: if r.sources_notes.is_empty() {
            OptionsNfo::films().sources_notes
        } else {
            r.sources_notes.clone()
        },
        creer_resume_court: r.creer_synopsis,
        ecrire_urls_artwork: r.nfo_urls_artwork,
        ecrire_fileinfo: r.nfo_infos_fichier,
        ecrire_date_ajout: r.nfo_date_ajout,
        horodatage: horodatage.then(horodatage_courant),
        ..OptionsNfo::films()
    }
}

/// Écrit les NFO d'un film (balises inconnues d'un NFO existant
/// conservées) et ajoute les fichiers au film. Renvoie le nombre de fichiers
/// réellement modifiés.
pub fn ecrire_nfo(
    film: &mut Film,
    saga: Option<&Saga>,
    reglages: &Reglages,
    nommages: &[NommageNfoFilm],
    horodatage: bool,
) -> Resultat<usize> {
    let options = options_nfo_films(&reglages.films, horodatage);
    let conn = connecteur(reglages.films.format_nfo);
    let mut modifies = 0;
    for nom in noms_nfo_film(film, nommages, false) {
        let chemin = film.chemin.join(&nom);
        let conserves: DonneesConservees = lire_fichier_nfo_film(&chemin)
            .map(|l| l.conserves)
            .unwrap_or_default();
        let contenu = ecrire_nfo_film(film, saga, &conserves, conn, &options);
        if let Some(p) = chemin.parent() {
            std::fs::create_dir_all(p).map_err(|e| ErreurApp::io(p, e))?;
        }
        if ecrire_si_change(&chemin, &contenu).map_err(|e| ErreurApp::io(&chemin, e))? {
            modifies += 1;
        }
        ajouter_fichier(film, &chemin, TypeFichier::Nfo);
    }
    Ok(modifies)
}

/// Saga d'un film (lecture en base).
pub(crate) fn saga_du_film(n: &Noyau, film: &Film) -> Resultat<Option<Saga>> {
    match &film.saga_id {
        Some(id) => Ok(n.verrou().bibliotheque.obtenir::<Saga>(id)?),
        None => Ok(None),
    }
}

/// Tâche : réécrit les NFO d'une sélection.
pub(crate) fn reecrire_films(n: &Noyau, ctx: &ContexteTache, ids: &[String]) -> Resultat<usize> {
    let reglages = n.reglages();
    let mut total = 0;
    for (i, id) in ids.iter().enumerate() {
        ctx.verifier_annulation()?;
        let Some(mut film) = n.verrou().bibliotheque.obtenir::<Film>(id)? else {
            continue;
        };
        ctx.progression(i as u64, ids.len() as u64, film.titre.clone());
        let saga = saga_du_film(n, &film)?;
        total += ecrire_nfo(
            &mut film,
            saga.as_ref(),
            &reglages,
            &n.config.nommage_nfo_film,
            n.config.horodatage_nfo,
        )?;
        if n.verrou().bibliotheque.enregistrer(&mut film)? {
            ctx.emettre(Evenement::FilmModifie { id: id.clone() });
        }
    }
    Ok(total)
}
