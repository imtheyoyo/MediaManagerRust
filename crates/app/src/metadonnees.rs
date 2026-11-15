//! Récupération des métadonnées en ligne (TMDB) : recherche, application du
//! résultat choisi, récupération automatique, images et NFO.
//!
//! La clé d'API est lue dans le coffre à secrets du stockage ; sans clé,
//! [`ErreurApp::CleTmdbAbsente`] est renvoyée avant tout appel réseau.

use std::collections::BTreeMap;
use std::time::Duration;

use mediamanager_core::modele::commun::cles_id;
use mediamanager_core::modele::{Film, Saga};
use mediamanager_scrapers::correspondance::choisir_meilleur;
use mediamanager_scrapers::{
    Artwork, ClientHttp, ConfigHttp, ConfigTmdb, FournisseurArtwork, FournisseurFilm,
    OptionsRecherche, ResultatRecherche, TmdbFilm,
};
use mediamanager_store::Reglages;

use crate::application::Noyau;
use crate::artwork::{self, ajouter_fichier, planifier_images};
use crate::conversion::{
    ConfigMetadonnees, appliquer_metadonnees, options_recherche, vers_identifiants,
};
use crate::ecriture_nfo::ecrire_nfo;
use crate::erreur::{ErreurApp, Resultat};
use crate::evenements::{Evenement, NiveauMessage};
use crate::renommage::{self, RapportRenommage};
use crate::taches::ContexteTache;

/// Recherche saisie par l'utilisateur (dialogue de choix).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequeteFilm {
    /// Texte (titre, `tt…`, `tmdb:…`).
    pub texte: String,
    pub annee: Option<i32>,
    /// Identifiants imposés (`tmdb`, `imdb`).
    pub ids: BTreeMap<String, String>,
}

/// Bilan d'une récupération automatique.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapportScrap {
    pub reussis: Vec<String>,
    /// Aucun résultat au-dessus du seuil (ou ex æquo).
    pub sans_resultat: Vec<String>,
    pub verrouilles: Vec<String>,
    /// (identifiant, message d'erreur).
    pub erreurs: Vec<(String, String)>,
    /// Renommage enchaîné si le réglage « renommer après scraping » est actif.
    pub renommage: Option<RapportRenommage>,
}

/// Scraper TMDB configuré (clé du coffre, cache HTTP, URL de test).
fn fournisseur(n: &Noyau) -> Resultat<TmdbFilm> {
    let cle = n.cle_tmdb()?;
    let mut cfg = ConfigTmdb::nouvelle(cle);
    cfg.url_api = n.config.url_api_tmdb.clone();
    let http = ClientHttp::new(ConfigHttp {
        dossier_cache: n.dossier_cache_http(),
        ..ConfigHttp::default()
    })?;
    Ok(TmdbFilm::new(cfg, http)?)
}

fn charger_film(n: &Noyau, id: &str) -> Resultat<Film> {
    n.verrou()
        .bibliotheque
        .obtenir::<Film>(id)?
        .ok_or_else(|| ErreurApp::Introuvable(format!("film {id}")))
}

/// Tâche : recherche de candidats pour un film.
pub(crate) fn rechercher(
    n: &Noyau,
    ctx: &ContexteTache,
    id: &str,
    requete: Option<RequeteFilm>,
) -> Resultat<Vec<ResultatRecherche>> {
    let film = charger_film(n, id)?;
    let reglages = n.reglages();
    let f = fournisseur(n)?;
    let mut o = options_recherche(&film, &reglages.films);
    if let Some(r) = requete {
        o.requete = r.texte;
        o.annee = r.annee;
        o.ids = r.ids;
    }
    ctx.progression(0, 1, format!("Recherche de « {} »", o.requete));
    let r = ctx.bloquer(f.rechercher(&o))?;
    ctx.progression(1, 1, format!("{} résultat(s)", r.len()));
    Ok(r)
}

/// Rattache le film à la saga du fournisseur (créée au besoin).
fn lier_saga(
    n: &Noyau,
    film: &mut Film,
    ids: &BTreeMap<String, String>,
    nom: &str,
) -> Resultat<Option<Saga>> {
    let Some(id_collection) = ids.get(cles_id::TMDB_SAGA).filter(|v| !v.is_empty()) else {
        return Ok(film
            .saga_id
            .clone()
            .and_then(|id| n.verrou().bibliotheque.obtenir::<Saga>(&id).ok().flatten()));
    };
    let mut s = n.verrou();
    let mut saga = s
        .bibliotheque
        .chercher_par_id_externe::<Saga>(cles_id::TMDB_SAGA, id_collection)?
        .into_iter()
        .next()
        .unwrap_or_else(|| Saga {
            titre: nom.to_string(),
            ids: vers_identifiants(&BTreeMap::from([(
                cles_id::TMDB_SAGA.to_string(),
                id_collection.clone(),
            )])),
            ..Saga::default()
        });
    if saga.titre.is_empty() {
        saga.titre = nom.to_string();
    }
    if !saga.films.contains(&film.id) {
        saga.films.push(film.id.clone());
    }
    s.bibliotheque.enregistrer(&mut saga)?;
    film.saga_id = Some(saga.id.clone());
    Ok(Some(saga))
}

/// Cœur commun : métadonnées complètes, application, saga, images, NFO, base.
fn scraper_film(
    n: &Noyau,
    ctx: &ContexteTache,
    f: &TmdbFilm,
    mut film: Film,
    options: OptionsRecherche,
    config: &ConfigMetadonnees,
    reglages: &Reglages,
) -> Resultat<Film> {
    if film.verrouille {
        return Err(ErreurApp::Verrouille(film.titre));
    }
    let m = ctx.bloquer(f.metadonnees(&options))?;
    let mut images: Vec<Artwork> = m.artwork.clone();
    if config.artwork {
        let mut o = options.clone();
        o.ids.extend(m.ids.clone());
        match ctx.bloquer(f.artwork(&o, None)) {
            Ok(v) => images.extend(v),
            Err(e) => ctx.message(
                NiveauMessage::Avertissement,
                format!("images de « {} » indisponibles : {e}", film.titre),
            ),
        }
    }
    appliquer_metadonnees(&mut film, &m, config, &reglages.films.pays_certification);
    film.derniere_langue_scraping = reglages.films.langue_scraping.clone();
    let saga = if config.saga {
        lier_saga(n, &mut film, &m.ids, &m.nom_collection)?
    } else {
        None
    };

    if config.artwork && !film.chemin.as_os_str().is_empty() {
        let plan = planifier_images(
            &film,
            &images,
            &n.config.artwork,
            &reglages.films.langues_images,
            reglages.films.taille_affiche,
            reglages.films.taille_fanart,
        );
        if !plan.is_empty() {
            let client = reqwest::Client::builder()
                .user_agent(ConfigHttp::default().user_agent)
                .connect_timeout(Duration::from_secs(30))
                .timeout(Duration::from_secs(120))
                .build()
                .map_err(|e| ErreurApp::Interne(format!("client HTTP : {e}")))?;
            for img in plan {
                ctx.verifier_annulation()?;
                match ctx.bloquer(artwork::telecharger(&client, &img.url)) {
                    Ok(octets) => {
                        for d in &img.destinations {
                            artwork::ecrire_atomiquement(d, &octets)?;
                            ajouter_fichier(&mut film, d, img.type_fichier);
                        }
                        film.urls_artwork.insert(img.type_fichier, img.url.clone());
                    }
                    Err(e) => ctx.message(NiveauMessage::Avertissement, e.to_string()),
                }
            }
        }
    }

    if !film.chemin.as_os_str().is_empty() {
        ecrire_nfo(
            &mut film,
            saga.as_ref(),
            reglages,
            &n.config.nommage_nfo_film,
            n.config.horodatage_nfo,
        )?;
    }
    n.verrou().bibliotheque.enregistrer(&mut film)?;
    ctx.emettre(Evenement::FilmModifie {
        id: film.id.clone(),
    });
    Ok(film)
}

/// Tâche : applique le résultat choisi dans le dialogue.
pub(crate) fn appliquer_choix(
    n: &Noyau,
    ctx: &ContexteTache,
    id: &str,
    choix: &ResultatRecherche,
    config: &ConfigMetadonnees,
) -> Resultat<Film> {
    let film = charger_film(n, id)?;
    if film.verrouille {
        return Err(ErreurApp::Verrouille(film.titre));
    }
    let reglages = n.reglages();
    let f = fournisseur(n)?;
    let mut o = options_recherche(&film, &reglages.films);
    o.ids = choix.ids.clone();
    ctx.progression(0, 1, format!("Métadonnées de « {} »", choix.titre));
    let film = scraper_film(n, ctx, &f, film, o, config, &reglages)?;
    ctx.progression(1, 1, "Terminé");
    Ok(film)
}

/// Tâche : récupération automatique pour une sélection.
pub(crate) fn scraper_automatiquement(
    n: &Noyau,
    ctx: &ContexteTache,
    ids: &[String],
    config: &ConfigMetadonnees,
) -> Resultat<RapportScrap> {
    let reglages = n.reglages();
    let f = fournisseur(n)?;
    let seuil = reglages.films.seuil_scraper as f32;
    let mut rapport = RapportScrap::default();
    let total = ids.len() as u64;
    for (i, id) in ids.iter().enumerate() {
        ctx.verifier_annulation()?;
        let film = match charger_film(n, id) {
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
        let o = options_recherche(&film, &reglages.films);
        let resultats = match ctx.bloquer(f.rechercher(&o)) {
            Ok(r) => r,
            Err(e @ mediamanager_scrapers::ErreurScraper::CleApiInvalide) => {
                return Err(e.into());
            }
            Err(mediamanager_scrapers::ErreurScraper::AucunResultat) => Vec::new(),
            Err(e) => {
                ctx.message(
                    NiveauMessage::Avertissement,
                    format!("{} : {e}", film.titre),
                );
                rapport.erreurs.push((id.clone(), e.to_string()));
                continue;
            }
        };
        let Some(meilleur) = choisir_meilleur(&resultats, seuil) else {
            ctx.message(
                NiveauMessage::Info,
                format!("« {} » : aucun résultat assez sûr", film.titre),
            );
            rapport.sans_resultat.push(id.clone());
            continue;
        };
        let mut o2 = o.clone();
        o2.ids = meilleur.ids.clone();
        match scraper_film(n, ctx, &f, film, o2, config, &reglages) {
            Ok(_) => rapport.reussis.push(id.clone()),
            Err(ErreurApp::Annulee) => return Err(ErreurApp::Annulee),
            Err(e) => {
                ctx.message(NiveauMessage::Avertissement, e.to_string());
                rapport.erreurs.push((id.clone(), e.to_string()));
            }
        }
    }
    ctx.progression(total, total, "Terminé");
    if reglages.films.renommer_apres_scraping && !rapport.reussis.is_empty() {
        rapport.renommage = Some(renommage::renommer_films(n, ctx, &rapport.reussis)?);
    }
    Ok(rapport)
}
