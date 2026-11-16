//! Récupération des métadonnées des séries (TMDB) : recherche, application du
//! résultat choisi, récupération automatique, liste d'épisodes et
//! appariement aux épisodes locaux, épisodes manquants (« fictifs »),
//! images de la série, des saisons et des épisodes, NFO.
//!
//! La clé d'API est lue dans le coffre à secrets ; sans clé,
//! [`ErreurApp::CleTmdbAbsente`] est renvoyée avant tout appel réseau.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use mediamanager_core::modele::{Episode, FichierMedia, Serie, TypeFichier};
use mediamanager_core::nfo::{
    DonneesConservees, NommageNfoEpisode, NommageNfoSaison, NommageNfoSerie, OptionsNfo,
    ecrire_nfo_episodes, ecrire_nfo_saison, ecrire_nfo_serie, ecrire_si_change, horodatage_courant,
    lire_fichier_nfo_episodes, lire_fichier_nfo_saison, lire_fichier_nfo_serie, nom_nfo_episode,
    nom_nfo_saison, nom_nfo_serie,
};
use mediamanager_core::renommage::plan::relatif;
use mediamanager_core::series::{
    EpisodeScrape, ReglagesFictifs, apparier_episodes, appliquer_scrape_episode,
    creer_episodes_fictifs, definir_groupe_actif, episodes_a_afficher, episodes_manquants,
    mettre_a_jour_depuis_scrape, recalculer_saisons,
};
use mediamanager_scrapers::correspondance_serie::choisir_meilleure_serie;
use mediamanager_scrapers::{
    ArtworkEtendu, ClientHttp, ConfigHttp, ConfigTmdb, ErreurScraper, FournisseurArtworkSerie,
    FournisseurSerie, MetadonneesEpisode, MetadonneesSerie, OptionsRecherche, ResultatRecherche,
    SEUIL_SERIE, TmdbSerie, TypeArtworkEtendu,
};
use mediamanager_store::Reglages;
use mediamanager_store::base::nouvel_id;
use mediamanager_store::reglages::TailleImage;

use crate::application::Noyau;
use crate::artwork::{
    self, ajouter_fichier_liste, extension_url, nombre_affiches_voulu, nombre_fanarts_voulu,
};
use crate::conversion_serie::{
    ConfigSerie, appliquer_metadonnees_serie, appliquer_saisons, options_recherche_serie,
    type_fichier_serie, vers_episode_scrape, vers_groupe,
};
use crate::ecriture_nfo::connecteur;
use crate::erreur::{ErreurApp, Resultat};
use crate::evenements::{Evenement, NiveauMessage};
use crate::metadonnees::RapportScrap;
use crate::renommage::RapportRenommage;
use crate::taches::ContexteTache;

/// Recherche de série saisie par l'utilisateur (dialogue de choix).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequeteSerie {
    /// Texte (titre, `tt…`, `tmdb:…`).
    pub texte: String,
    pub annee: Option<i32>,
    /// Identifiants imposés (`tmdb`, `imdb`, `tvdb`).
    pub ids: BTreeMap<String, String>,
    /// Langue de recherche ; `None` = langue des réglages.
    pub langue: Option<String>,
}

/// Scraper TMDB des séries configuré (clé du coffre, cache HTTP, URL de test).
pub(crate) fn fournisseur(n: &Noyau) -> Resultat<TmdbSerie> {
    let cle = n.cle_tmdb()?;
    let mut cfg = ConfigTmdb::nouvelle(cle);
    cfg.url_api = n.config.url_api_tmdb.clone();
    let http = ClientHttp::new(ConfigHttp {
        dossier_cache: n.dossier_cache_http(),
        ..ConfigHttp::default()
    })?;
    Ok(TmdbSerie::new(cfg, http)?)
}

pub(crate) fn charger_serie(n: &Noyau, id: &str) -> Resultat<Serie> {
    n.verrou()
        .bibliotheque
        .obtenir::<Serie>(id)?
        .ok_or_else(|| ErreurApp::Introuvable(format!("série {id}")))
}

/// Tâche : recherche de candidats pour une série.
pub(crate) fn rechercher(
    n: &Noyau,
    ctx: &ContexteTache,
    id: &str,
    requete: Option<RequeteSerie>,
) -> Resultat<Vec<ResultatRecherche>> {
    let serie = charger_serie(n, id)?;
    let reglages = n.reglages();
    let f = fournisseur(n)?;
    let mut o = options_recherche_serie(&serie, &reglages.series);
    if let Some(r) = requete {
        o.requete = r.texte;
        o.annee = r.annee;
        o.ids = r.ids;
        if let Some(l) = r.langue.filter(|l| !l.trim().is_empty()) {
            o.langue = l;
        }
    }
    ctx.progression(0, 1, format!("Recherche de « {} »", o.requete));
    let r = match ctx.bloquer(f.rechercher(&o)) {
        Ok(r) => r,
        Err(ErreurScraper::AucunResultat) => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    ctx.progression(1, 1, format!("{} résultat(s)", r.len()));
    Ok(r)
}

/// Tâche : applique le résultat choisi dans le dialogue.
pub(crate) fn appliquer_choix(
    n: &Noyau,
    ctx: &ContexteTache,
    id: &str,
    choix: &ResultatRecherche,
    config: &ConfigSerie,
) -> Resultat<Serie> {
    Ok(appliquer_choix_avec_renommage(n, ctx, id, choix, config)?.0)
}

/// Applique le résultat choisi puis, si le réglage « renommer après
/// scraping » des séries est actif, renomme la série. Un échec du renommage
/// n'annule pas le scrap (message d'avertissement).
pub(crate) fn appliquer_choix_avec_renommage(
    n: &Noyau,
    ctx: &ContexteTache,
    id: &str,
    choix: &ResultatRecherche,
    config: &ConfigSerie,
) -> Resultat<(Serie, Option<RapportRenommage>)> {
    let serie = charger_serie(n, id)?;
    if serie.verrouille {
        return Err(ErreurApp::Verrouille(serie.titre));
    }
    let reglages = n.reglages();
    let f = fournisseur(n)?;
    let mut o = options_recherche_serie(&serie, &reglages.series);
    o.ids = choix.ids.clone();
    ctx.progression(0, 1, format!("Métadonnées de « {} »", choix.titre));
    let mut photos = crate::acteurs::BilanPhotos::default();
    let mut serie = scraper_serie(n, ctx, &f, serie, o, config, &reglages, &mut photos)?;
    let mut rapport = None;
    if reglages.series.renommer_apres_scraping {
        match crate::renommage_serie::renommer_series(n, ctx, std::slice::from_ref(&serie.id)) {
            Ok(r) => {
                if !r.renommes.is_empty() {
                    serie = charger_serie(n, id)?;
                }
                rapport = Some(r);
            }
            Err(ErreurApp::Annulee) => return Err(ErreurApp::Annulee),
            Err(e) => ctx.message(
                NiveauMessage::Avertissement,
                format!("renommage de « {} » impossible : {e}", serie.titre),
            ),
        }
    }
    ctx.progression(1, 1, "Terminé");
    Ok((serie, rapport))
}

/// Tâche : récupération automatique pour une sélection de séries. Le seuil
/// est fixe ([`SEUIL_SERIE`]) ; un résultat unique est toujours accepté.
pub(crate) fn scraper_automatiquement(
    n: &Noyau,
    ctx: &ContexteTache,
    ids: &[String],
    config: &ConfigSerie,
) -> Resultat<RapportScrap> {
    let reglages = n.reglages();
    let f = fournisseur(n)?;
    let mut rapport = RapportScrap::default();
    let total = ids.len() as u64;
    for (i, id) in ids.iter().enumerate() {
        ctx.verifier_annulation()?;
        let serie = match charger_serie(n, id) {
            Ok(s) => s,
            Err(e) => {
                rapport.erreurs.push((id.clone(), e.to_string()));
                continue;
            }
        };
        ctx.progression(i as u64, total, serie.titre.clone());
        if serie.verrouille {
            rapport.verrouilles.push(id.clone());
            continue;
        }
        let o = options_recherche_serie(&serie, &reglages.series);
        let resultats = match ctx.bloquer(f.rechercher(&o)) {
            Ok(r) => r,
            Err(e @ ErreurScraper::CleApiInvalide) => return Err(e.into()),
            Err(ErreurScraper::AucunResultat) => Vec::new(),
            Err(e) => {
                ctx.message(
                    NiveauMessage::Avertissement,
                    format!("{} : {e}", serie.titre),
                );
                rapport.erreurs.push((id.clone(), e.to_string()));
                continue;
            }
        };
        let Some(meilleur) = choisir_meilleure_serie(&resultats, SEUIL_SERIE) else {
            ctx.message(
                NiveauMessage::Info,
                format!("« {} » : aucun résultat assez sûr", serie.titre),
            );
            rapport.sans_resultat.push(id.clone());
            continue;
        };
        let mut o2 = o.clone();
        o2.ids = meilleur.ids.clone();
        let mut photos = crate::acteurs::BilanPhotos::default();
        let r = scraper_serie(n, ctx, &f, serie, o2, config, &reglages, &mut photos);
        rapport.photos_acteurs += photos.telechargees;
        rapport.photos_acteurs_echecs += photos.echecs;
        match r {
            Ok(_) => rapport.reussis.push(id.clone()),
            Err(ErreurApp::Annulee) => return Err(ErreurApp::Annulee),
            Err(e) => {
                ctx.message(NiveauMessage::Avertissement, e.to_string());
                rapport.erreurs.push((id.clone(), e.to_string()));
            }
        }
    }
    ctx.progression(total, total, "Terminé");
    if reglages.series.renommer_apres_scraping && !rapport.reussis.is_empty() {
        rapport.renommage = Some(crate::renommage_serie::renommer_series(
            n,
            ctx,
            &rapport.reussis,
        )?);
    }
    Ok(rapport)
}

/// Apparie la liste scrapée aux épisodes locaux selon l'ordre demandé (ou
/// choisi automatiquement), crée les épisodes fictifs, recalcule les
/// saisons. Renvoie les paires (identifiant de l'épisode local, indice dans
/// la liste scrapée).
pub fn associer_episodes(
    serie: &mut Serie,
    liste: &[MetadonneesEpisode],
    metadonnees: &MetadonneesSerie,
    config: &ConfigSerie,
) -> Vec<(String, usize)> {
    for e in serie.episodes.iter_mut().filter(|e| e.id.is_empty()) {
        e.id = nouvel_id();
    }
    let scrapes: Vec<EpisodeScrape> = liste.iter().map(vers_episode_scrape).collect();
    let groupes: Vec<_> = metadonnees
        .groupes_episodes
        .iter()
        .map(vers_groupe)
        .collect();
    let ids_avant: Vec<String> = serie.episodes.iter().map(|e| e.id.clone()).collect();
    let ecraser = !config.ne_pas_ecraser;
    let c = match &config.ordre {
        None => mettre_a_jour_depuis_scrape(serie, &scrapes, &groupes, ecraser),
        Some(g) => {
            for x in &groupes {
                if !serie.groupes.contains(x) {
                    serie.groupes.push(x.clone());
                }
            }
            serie.groupe_actif = g.clone();
            serie.episodes_fictifs = creer_episodes_fictifs(&scrapes, &serie.id);
            let c = apparier_episodes(&serie.episodes, &scrapes, g);
            for &(i, j) in &c.paires {
                appliquer_scrape_episode(&mut serie.episodes[i], &scrapes[j], ecraser);
            }
            definir_groupe_actif(serie, g.clone());
            c
        }
    };
    if !config.episodes_fictifs {
        serie.episodes_fictifs.clear();
        recalculer_saisons(serie);
    }
    c.paires
        .iter()
        .map(|&(i, j)| (ids_avant[i].clone(), j))
        .collect()
}

/// Cœur commun : métadonnées de la série, épisodes, images, NFO, base.
#[allow(clippy::too_many_arguments)]
fn scraper_serie(
    n: &Noyau,
    ctx: &ContexteTache,
    f: &TmdbSerie,
    mut serie: Serie,
    options: OptionsRecherche,
    config: &ConfigSerie,
    reglages: &Reglages,
    photos: &mut crate::acteurs::BilanPhotos,
) -> Resultat<Serie> {
    if serie.verrouille {
        return Err(ErreurApp::Verrouille(serie.titre));
    }
    let m = ctx.bloquer(f.metadonnees_serie(&options))?;
    let mut o = options.clone();
    o.ids.extend(m.ids.clone());
    appliquer_metadonnees_serie(
        &mut serie,
        &m,
        config.ne_pas_ecraser,
        &reglages.series.pays_certification,
    );

    let mut liste: Vec<MetadonneesEpisode> = Vec::new();
    let mut paires: Vec<(String, usize)> = Vec::new();
    if config.episodes {
        ctx.verifier_annulation()?;
        match ctx.bloquer(f.liste_episodes(&o)) {
            Ok(l) => liste = l,
            Err(e @ ErreurScraper::CleApiInvalide) => return Err(e.into()),
            Err(e) => ctx.message(
                NiveauMessage::Avertissement,
                format!("épisodes de « {} » indisponibles : {e}", serie.titre),
            ),
        }
        if !liste.is_empty() {
            paires = associer_episodes(&mut serie, &liste, &m, config);
            appliquer_saisons(&mut serie, &m, config.ne_pas_ecraser);
        }
    }

    if !serie.chemin.as_os_str().is_empty() {
        let mut plan: Vec<ImageSerie> = Vec::new();
        if config.artwork {
            let mut images: Vec<ArtworkEtendu> = m.artwork.clone();
            match ctx.bloquer(f.artwork_serie(&o, None)) {
                Ok(v) => images.extend(v),
                Err(e @ ErreurScraper::CleApiInvalide) => return Err(e.into()),
                Err(e) => ctx.message(
                    NiveauMessage::Avertissement,
                    format!("images de « {} » indisponibles : {e}", serie.titre),
                ),
            }
            plan.extend(planifier_images_serie(&serie, &images, reglages));
            plan.extend(planifier_images_supplementaires(&serie, &images, reglages));
        }
        if config.artwork_episodes {
            plan.extend(planifier_vignettes(&serie, &liste, &paires, reglages));
        }
        if !plan.is_empty() {
            let client = artwork::client_telechargement()?;
            for img in plan {
                ctx.verifier_annulation()?;
                match ctx.bloquer(artwork::telecharger(&client, &img.url)) {
                    Ok(octets) => {
                        artwork::ecrire_atomiquement(&img.destination, &octets)?;
                        rattacher_image(&mut serie, &img);
                    }
                    Err(e) => ctx.message(NiveauMessage::Avertissement, e.to_string()),
                }
            }
        }
        // Option indépendante de « images » : `.actors` est dans le dossier
        // de la série (acteurs de la série et invités des épisodes).
        if reglages.series.images_acteurs {
            let mut personnes = serie.acteurs.clone();
            personnes.extend(
                serie
                    .episodes
                    .iter()
                    .flat_map(|e| e.invites.iter().cloned()),
            );
            let b = crate::acteurs::telecharger_photos_acteurs(ctx, &serie.chemin, &personnes)?;
            photos.ajouter(b);
            if b.telechargees > 0 || b.echecs > 0 {
                ctx.message(
                    NiveauMessage::Info,
                    format!(
                        "« {} » : {} photo(s) d'acteurs téléchargée(s), {} en échec",
                        serie.titre, b.telechargees, b.echecs
                    ),
                );
            }
        }
        ecrire_nfo_serie_complete(
            &mut serie,
            reglages,
            n.config.horodatage_nfo,
            n.config.nommage_nfo_saison,
        )?;
    }
    n.verrou().bibliotheque.enregistrer(&mut serie)?;
    ctx.emettre(Evenement::SerieModifiee {
        id: serie.id.clone(),
    });
    Ok(serie)
}

// ---------------------------------------------------------------------------
// Images
// ---------------------------------------------------------------------------

/// Image d'une série à télécharger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageSerie {
    pub type_fichier: TypeFichier,
    pub url: String,
    pub destination: PathBuf,
    /// Saison concernée (images de saison).
    pub saison: Option<i32>,
    /// Épisodes concernés (vignettes ; plusieurs pour un multi-épisodes).
    pub episodes: Vec<String>,
    /// 0 = image principale ; k ≥ 1 = k-ième image supplémentaire de la
    /// série (`poster<k>`, `extrafanart/fanart<k>`).
    pub indice: u32,
}

fn rang_langue(a: &ArtworkEtendu, langues: &[String]) -> usize {
    if a.langue.is_empty() {
        return if matches!(
            a.type_artwork,
            TypeArtworkEtendu::Fond | TypeArtworkEtendu::FondSaison
        ) {
            0
        } else {
            langues.len() + 1
        };
    }
    match langues
        .iter()
        .position(|l| l.eq_ignore_ascii_case(&a.langue))
    {
        Some(i) => i + 1,
        None => langues.len() + 2,
    }
}

/// URL retenue pour une taille demandée.
pub fn url_etendue(a: &ArtworkEtendu, taille: TailleImage) -> String {
    let mut tailles = a.tailles.clone();
    tailles.sort_by_key(|t| t.ordre);
    let choisie = match taille {
        TailleImage::Originale => None,
        TailleImage::Grande => tailles.last(),
        TailleImage::Moyenne => tailles.get(tailles.len() / 2),
        TailleImage::Petite => tailles.first(),
    };
    choisie
        .map(|t| t.url.clone())
        .filter(|u| !u.is_empty())
        .or_else(|| (!a.url_originale.is_empty()).then(|| a.url_originale.clone()))
        .unwrap_or_else(|| a.url_apercu.clone())
}

/// Meilleure image par (type, saison) : langue préférée puis votes.
pub fn choisir_images_serie<'a>(
    images: &'a [ArtworkEtendu],
    langues: &[String],
) -> Vec<&'a ArtworkEtendu> {
    let mut choix: Vec<&ArtworkEtendu> = Vec::new();
    for a in images {
        match choix
            .iter_mut()
            .find(|x| x.type_artwork == a.type_artwork && x.saison == a.saison)
        {
            Some(x) => {
                if (rang_langue(a, langues), std::cmp::Reverse(a.votes))
                    < (rang_langue(x, langues), std::cmp::Reverse(x.votes))
                {
                    *x = a;
                }
            }
            None => choix.push(a),
        }
    }
    choix
}

/// Nom (sans extension) d'une image de série ou de saison ; `None` pour un
/// type non téléchargé.
pub fn nom_image_serie(t: TypeFichier, saison: Option<i32>) -> Option<String> {
    let prefixe_saison = |s: i32| {
        if s == 0 {
            "season-specials".to_string()
        } else if s < 0 {
            "season-all".to_string()
        } else {
            format!("season{s:02}")
        }
    };
    Some(match (t, saison) {
        (TypeFichier::Poster, None) => "poster".into(),
        (TypeFichier::Fanart, None) => "fanart".into(),
        (TypeFichier::Banner, None) => "banner".into(),
        (TypeFichier::Thumb, None) => "landscape".into(),
        (TypeFichier::Clearlogo, None) => "clearlogo".into(),
        (TypeFichier::Clearart, None) => "clearart".into(),
        (TypeFichier::SaisonPoster, Some(s)) => format!("{}-poster", prefixe_saison(s)),
        (TypeFichier::SaisonFanart, Some(s)) => format!("{}-fanart", prefixe_saison(s)),
        (TypeFichier::SaisonBanner, Some(s)) => format!("{}-banner", prefixe_saison(s)),
        (TypeFichier::SaisonThumb, Some(s)) => format!("{}-thumb", prefixe_saison(s)),
        _ => return None,
    })
}

fn langues_images(reglages: &Reglages) -> Vec<String> {
    let l = reglages
        .series
        .langue_scraping
        .split(['_', '-'])
        .next()
        .unwrap_or("")
        .to_string();
    let mut v = Vec::new();
    if !l.is_empty() {
        v.push(l);
    }
    if !v.iter().any(|x| x == "en") {
        v.push("en".into());
    }
    v
}

/// Images de la série et de ses saisons (saisons connues seulement).
pub fn planifier_images_serie(
    serie: &Serie,
    images: &[ArtworkEtendu],
    reglages: &Reglages,
) -> Vec<ImageSerie> {
    let saisons: BTreeSet<i32> = serie.saisons.iter().map(|s| s.numero).collect();
    choisir_images_serie(images, &langues_images(reglages))
        .into_iter()
        .filter_map(|a| {
            let t = type_fichier_serie(a.type_artwork)?;
            let saison = a.saison.map(|s| s as i32);
            if let Some(s) = saison
                && !saisons.contains(&s)
            {
                return None;
            }
            // Une image de série typée « saison » sans numéro est ignorée.
            let nom = nom_image_serie(t, saison)?;
            let taille = match t {
                TypeFichier::Fanart | TypeFichier::SaisonFanart => reglages.series.taille_fanart,
                TypeFichier::Thumb | TypeFichier::SaisonThumb => reglages.series.taille_vignette,
                _ => reglages.series.taille_affiche,
            };
            let url = url_etendue(a, taille);
            if url.is_empty() {
                return None;
            }
            Some(ImageSerie {
                type_fichier: t,
                destination: serie.chemin.join(format!("{nom}.{}", extension_url(&url))),
                url,
                saison,
                episodes: Vec::new(),
                indice: 0,
            })
        })
        .collect()
}

/// Affiches et fonds supplémentaires de la série (selon `nombre_affiches` et
/// `nombre_fanarts`) : les meilleures images (langue, votes, taille) autres
/// que l'image principale, écrites `poster<k>` et `extrafanart/fanart<k>`.
pub fn planifier_images_supplementaires(
    serie: &Serie,
    images: &[ArtworkEtendu],
    reglages: &Reglages,
) -> Vec<ImageSerie> {
    let langues = langues_images(reglages);
    let principales: Vec<String> = choisir_images_serie(images, &langues)
        .into_iter()
        .filter(|a| a.saison.is_none())
        .map(|a| url_etendue(a, TailleImage::Originale))
        .collect();
    let voulus = [
        (
            TypeArtworkEtendu::Affiche,
            nombre_affiches_voulu(reglages.series.nombre_affiches),
            TypeFichier::Graphic,
            reglages.series.taille_affiche,
        ),
        (
            TypeArtworkEtendu::Fond,
            nombre_fanarts_voulu(reglages.series.nombre_fanarts, false, 0),
            TypeFichier::ExtraFanart,
            reglages.series.taille_fanart,
        ),
    ];
    let mut sortie = Vec::new();
    for (ta, n, type_fichier, taille) in voulus {
        if n < 2 {
            continue;
        }
        let mut v: Vec<&ArtworkEtendu> = images
            .iter()
            .filter(|a| a.type_artwork == ta && a.saison.is_none())
            .collect();
        v.sort_by_key(|a| {
            (
                rang_langue(a, &langues),
                std::cmp::Reverse(a.votes),
                std::cmp::Reverse(a.tailles.iter().map(|t| t.largeur).max().unwrap_or(0)),
            )
        });
        let mut vus: Vec<String> = principales.clone();
        let mut k = 0u32;
        for a in v {
            if k as usize >= n - 1 {
                break;
            }
            let id = url_etendue(a, TailleImage::Originale);
            let url = url_etendue(a, taille);
            if url.is_empty() || vus.contains(&id) {
                continue;
            }
            vus.push(id);
            k += 1;
            let ext = extension_url(&url);
            let nom = if type_fichier == TypeFichier::ExtraFanart {
                format!("extrafanart/fanart{k}.{ext}")
            } else {
                format!("poster{k}.{ext}")
            };
            sortie.push(ImageSerie {
                type_fichier,
                destination: serie.chemin.join(nom),
                url,
                saison: None,
                episodes: Vec::new(),
                indice: k,
            });
        }
    }
    sortie
}

fn video(e: &Episode) -> Option<&FichierMedia> {
    e.fichiers
        .iter()
        .find(|f| f.type_fichier == TypeFichier::Video)
}

/// Vignettes des épisodes appariés : `<nom de la vidéo>-thumb.<ext>` à côté
/// de la vidéo (une seule par fichier pour les multi-épisodes).
pub fn planifier_vignettes(
    serie: &Serie,
    liste: &[MetadonneesEpisode],
    paires: &[(String, usize)],
    reglages: &Reglages,
) -> Vec<ImageSerie> {
    let langues = langues_images(reglages);
    let mut sortie: Vec<ImageSerie> = Vec::new();
    for (id, j) in paires {
        let Some(ep) = serie.episodes.iter().find(|e| &e.id == id) else {
            continue;
        };
        let (Some(v), Some(nfo)) = (
            video(ep),
            nom_nfo_episode(ep, NommageNfoEpisode::NomFichier),
        ) else {
            continue;
        };
        let Some(m) = liste.get(*j) else { continue };
        let vignettes: Vec<ArtworkEtendu> = m
            .artwork
            .iter()
            .filter(|a| a.type_artwork == TypeArtworkEtendu::Vignette)
            .cloned()
            .collect();
        let Some(a) = choisir_images_serie(&vignettes, &langues)
            .into_iter()
            .next()
        else {
            continue;
        };
        let url = url_etendue(a, reglages.series.taille_vignette);
        if url.is_empty() {
            continue;
        }
        let base = nfo.trim_end_matches(".nfo");
        let destination = v
            .chemin
            .join(format!("{base}-thumb.{}", extension_url(&url)));
        match sortie.iter_mut().find(|x| x.destination == destination) {
            Some(x) => x.episodes.push(id.clone()),
            None => sortie.push(ImageSerie {
                type_fichier: TypeFichier::Thumb,
                url,
                destination,
                saison: None,
                episodes: vec![id.clone()],
                indice: 0,
            }),
        }
    }
    sortie
}

fn rattacher_image(serie: &mut Serie, img: &ImageSerie) {
    if !img.episodes.is_empty() {
        for e in serie
            .episodes
            .iter_mut()
            .filter(|e| img.episodes.contains(&e.id))
        {
            ajouter_fichier_liste(&mut e.fichiers, &img.destination, img.type_fichier);
        }
        return;
    }
    match img.saison {
        Some(s) => {
            if let Some(saison) = serie.saisons.iter_mut().find(|x| x.numero == s) {
                ajouter_fichier_liste(&mut saison.fichiers, &img.destination, img.type_fichier);
            }
        }
        None => {
            ajouter_fichier_liste(&mut serie.fichiers, &img.destination, img.type_fichier);
            if img.indice == 0 {
                serie.urls_artwork.insert(img.type_fichier, img.url.clone());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// NFO
// ---------------------------------------------------------------------------

/// Options d'écriture des NFO de séries d'après les réglages.
pub fn options_nfo_series(r: &Reglages, horodatage: bool) -> OptionsNfo {
    OptionsNfo {
        ecrire_guide_episodes: r.series.nfo_guide_episodes,
        ecrire_date_fin: r.series.nfo_date_fin,
        ecrire_tous_acteurs: r.series.nfo_tous_acteurs,
        horodatage: horodatage.then(horodatage_courant),
        ..OptionsNfo::series()
    }
}

fn ecrire(chemin: &std::path::Path, contenu: &str) -> Resultat<bool> {
    if let Some(p) = chemin.parent() {
        std::fs::create_dir_all(p).map_err(|e| ErreurApp::io(p, e))?;
    }
    ecrire_si_change(chemin, contenu).map_err(|e| ErreurApp::io(chemin, e))
}

/// Préfixe relatif (`../`, `../../`…) menant de `dossier` à `racine`
/// (vide si `dossier` est la racine ou n'est pas dessous).
fn prefixe_vers_racine(dossier: &std::path::Path, racine: &std::path::Path) -> String {
    match dossier.strip_prefix(racine) {
        Ok(r) => "../".repeat(r.components().count()),
        Err(_) => String::new(),
    }
}

/// Écrit `tvshow.nfo`, les NFO des épisodes (un par fichier vidéo,
/// multi-épisodes compris) et, si `nommage_saison` est donné, ceux des
/// saisons. Balises inconnues des anciens NFO conservées ; fichiers ajoutés
/// au modèle. Renvoie le nombre de fichiers réellement modifiés.
pub fn ecrire_nfo_serie_complete(
    serie: &mut Serie,
    reglages: &Reglages,
    horodatage: bool,
    nommage_saison: Option<NommageNfoSaison>,
) -> Resultat<usize> {
    let mut options = options_nfo_series(reglages, horodatage);
    let dossier_acteurs = serie.chemin.join(crate::acteurs::DOSSIER_ACTEURS);
    if dossier_acteurs.is_dir() {
        options.dossier_acteurs = Some(dossier_acteurs);
    }
    let conn = connecteur(reglages.series.format_nfo);
    let mut modifies = 0;

    // Série.
    let chemin = serie.chemin.join(nom_nfo_serie(NommageNfoSerie::TvShow));
    let conserves = lire_fichier_nfo_serie(&chemin)
        .map(|l| l.conserves)
        .unwrap_or_default();
    if ecrire(
        &chemin,
        &ecrire_nfo_serie(serie, &conserves, conn, &options),
    )? {
        modifies += 1;
    }
    ajouter_fichier_liste(&mut serie.fichiers, &chemin, TypeFichier::Nfo);

    // Épisodes, regroupés par fichier vidéo.
    let mut groupes: Vec<(PathBuf, Vec<usize>)> = Vec::new();
    for (i, e) in serie.episodes.iter().enumerate() {
        if e.est_fictif() {
            continue;
        }
        let (Some(v), Some(nom)) = (video(e), nom_nfo_episode(e, NommageNfoEpisode::NomFichier))
        else {
            continue;
        };
        let chemin = v.chemin.join(nom);
        match groupes.iter_mut().find(|g| g.0 == chemin) {
            Some(g) => g.1.push(i),
            None => groupes.push((chemin, vec![i])),
        }
    }
    for (chemin, indices) in groupes {
        let anciens: Vec<DonneesConservees> = lire_fichier_nfo_episodes(&chemin)
            .map(|l| l.episodes.into_iter().map(|e| e.conserves).collect())
            .unwrap_or_default();
        let contenu = {
            let refs: Vec<&Episode> = indices.iter().map(|&i| &serie.episodes[i]).collect();
            // Le NFO d'un épisode peut se trouver dans un dossier de saison :
            // la photo `.actors` de la série est référencée depuis ce dossier.
            let mut opt_ep = options.clone();
            if let Some(d) = chemin.parent() {
                opt_ep.prefixe_acteurs = prefixe_vers_racine(d, &serie.chemin);
            }
            ecrire_nfo_episodes(serie, &refs, &anciens, conn, &opt_ep)
        };
        if ecrire(&chemin, &contenu)? {
            modifies += 1;
        }
        for i in indices {
            ajouter_fichier_liste(&mut serie.episodes[i].fichiers, &chemin, TypeFichier::Nfo);
        }
    }

    // Saisons.
    if let Some(nommage) = nommage_saison {
        let groupe = serie.groupe_actif.clone();
        for k in 0..serie.saisons.len() {
            let numero = serie.saisons[k].numero;
            let dossier = serie
                .episodes
                .iter()
                .filter(|e| e.saison(&groupe) == numero)
                .find_map(video)
                .map(|v| relatif(&v.chemin, &serie.chemin))
                .unwrap_or_default();
            if nommage == NommageNfoSaison::DossierSaison && dossier.is_empty() {
                continue;
            }
            let chemin = serie.chemin.join(nom_nfo_saison(numero, nommage, &dossier));
            let conserves = lire_fichier_nfo_saison(&chemin)
                .map(|l| l.conserves)
                .unwrap_or_default();
            let contenu = ecrire_nfo_saison(serie, &serie.saisons[k], &conserves, conn, &options);
            if ecrire(&chemin, &contenu)? {
                modifies += 1;
            }
            ajouter_fichier_liste(&mut serie.saisons[k].fichiers, &chemin, TypeFichier::Nfo);
        }
    }
    Ok(modifies)
}

/// Tâche : réécrit les NFO d'une sélection de séries.
pub(crate) fn reecrire_nfo(n: &Noyau, ctx: &ContexteTache, ids: &[String]) -> Resultat<usize> {
    let reglages = n.reglages();
    let mut total = 0;
    for (i, id) in ids.iter().enumerate() {
        ctx.verifier_annulation()?;
        let Some(mut serie) = n.verrou().bibliotheque.obtenir::<Serie>(id)? else {
            continue;
        };
        ctx.progression(i as u64, ids.len() as u64, serie.titre.clone());
        total += ecrire_nfo_serie_complete(
            &mut serie,
            &reglages,
            n.config.horodatage_nfo,
            n.config.nommage_nfo_saison,
        )?;
        if n.verrou().bibliotheque.enregistrer(&mut serie)? {
            ctx.emettre(Evenement::SerieModifiee { id: id.clone() });
        }
    }
    Ok(total)
}

// ---------------------------------------------------------------------------
// Épisodes manquants
// ---------------------------------------------------------------------------

/// Réglages d'affichage des épisodes fictifs d'après les réglages.
pub fn reglages_fictifs(r: &Reglages) -> ReglagesFictifs {
    ReglagesFictifs {
        afficher_manquants: r.series.afficher_episodes_manquants,
        afficher_speciaux_manquants: r.series.afficher_speciaux_manquants,
        afficher_non_diffuses: r.series.afficher_non_diffuses,
    }
}

/// Épisodes à afficher : réels, plus les manquants autorisés par les réglages.
pub fn episodes_affiches(serie: &Serie, r: &Reglages) -> Vec<Episode> {
    let aujourdhui = mediamanager_scrapers::tmdb::date_du_jour();
    episodes_a_afficher(serie, reglages_fictifs(r), Some(&aujourdhui))
        .into_iter()
        .cloned()
        .collect()
}

/// Épisodes scrapés sans fichier local (tous, quels que soient les réglages).
pub fn episodes_absents(serie: &Serie) -> Vec<Episode> {
    episodes_manquants(serie).into_iter().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mediamanager_scrapers::TailleImage as Taille;

    fn image(t: TypeArtworkEtendu, langue: &str, saison: Option<u32>, url: &str) -> ArtworkEtendu {
        ArtworkEtendu {
            fournisseur: "tmdb".into(),
            type_artwork: t,
            url_apercu: String::new(),
            url_originale: url.into(),
            langue: langue.into(),
            votes: 0,
            saison,
            tailles: vec![Taille {
                largeur: 300,
                hauteur: 450,
                url: format!("{url}-petit"),
                ordre: 1,
            }],
        }
    }

    #[test]
    fn noms_et_choix_des_images() {
        assert_eq!(
            nom_image_serie(TypeFichier::SaisonPoster, Some(1)).unwrap(),
            "season01-poster"
        );
        assert_eq!(
            nom_image_serie(TypeFichier::SaisonFanart, Some(0)).unwrap(),
            "season-specials-fanart"
        );
        assert!(nom_image_serie(TypeFichier::Disc, None).is_none());
        let imgs = vec![
            image(TypeArtworkEtendu::Affiche, "en", None, "en"),
            image(TypeArtworkEtendu::Affiche, "fr", None, "fr"),
            image(TypeArtworkEtendu::AfficheSaison, "fr", Some(1), "s1"),
            image(TypeArtworkEtendu::AfficheSaison, "fr", Some(2), "s2"),
        ];
        let c = choisir_images_serie(&imgs, &["fr".to_string()]);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].url_originale, "fr");
        assert_eq!(url_etendue(c[0], TailleImage::Petite), "fr-petit");
        assert_eq!(url_etendue(c[0], TailleImage::Originale), "fr");
    }
}
