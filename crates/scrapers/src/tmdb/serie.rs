//! Fournisseur TMDB pour les séries : recherche, détail, saisons, épisodes,
//! groupes d'épisodes (ordres alternatifs), images et bandes-annonces.

use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde::Deserialize;

use super::{
    Credits, Genre, IdsExternes, ImageTmdb, MembreDistribution, MembreEquipe, Mot, PAGES_MAX,
    PREFIXES_ARTICLES, ReponseImages, ReponseVideos, Societe, TmdbFilm, annee_de,
    artwork_depuis_images, bandes_depuis_videos, construire_artwork, date_iso,
    genre_depuis_id_tmdb, langue_requete, non_vide, personne, trier_images, type_equipe,
};
use crate::correspondance::{
    calculer_score, nettoyer_caracteres, retirer_nom_triable, trier_resultats,
};
use crate::correspondance_serie::{ReglesAppariement, apparier_episode, est_titre_generique};
use crate::erreur::{ErreurScraper, Resultat};
use crate::fournisseur::{FournisseurArtworkSerie, FournisseurBandesAnnonces, FournisseurSerie};
use crate::http::ClientHttp;
use crate::tmdb::ConfigTmdb;
use crate::types::*;

const ID_FOURNISSEUR: &str = "tmdb";
/// Durée de validité du cache mémoire des listes d'épisodes.
const TTL_LISTE_EPISODES: Duration = Duration::from_secs(600);
const AJOUTS_DETAIL: &str =
    "translations,credits,external_ids,content_ratings,keywords,episode_groups";

/// Scraper TMDB (séries).
#[derive(Debug)]
pub struct TmdbSerie {
    interne: TmdbFilm,
    cache_episodes: Mutex<HashMap<String, (Instant, Vec<MetadonneesEpisode>)>>,
}

// ---------------------------------------------------------------- JSON TMDB

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct ElementSerie {
    id: u64,
    name: String,
    original_name: String,
    original_language: String,
    overview: String,
    poster_path: Option<String>,
    first_air_date: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct PageSeries {
    total_pages: u32,
    results: Vec<ElementSerie>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ReponseTrouverSerie {
    tv_results: Vec<ElementSerie>,
    tv_episode_results: Vec<EpisodeTrouve>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct EpisodeTrouve {
    show_id: u64,
    season_number: u32,
    episode_number: u32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DetailSerie {
    id: u64,
    name: String,
    original_name: String,
    original_language: String,
    overview: String,
    tagline: String,
    first_air_date: Option<String>,
    last_air_date: Option<String>,
    episode_run_time: Vec<u32>,
    origin_country: Vec<String>,
    networks: Vec<Societe>,
    production_companies: Vec<Societe>,
    status: String,
    genres: Vec<Genre>,
    vote_average: f32,
    vote_count: u32,
    poster_path: Option<String>,
    number_of_seasons: u32,
    seasons: Vec<SaisonResume>,
    credits: Credits,
    external_ids: IdsExternes,
    content_ratings: ClassificationsTv,
    keywords: MotsSerie,
    translations: TraductionsSerie,
    episode_groups: GroupesRef,
    images: ReponseImages,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SaisonResume {
    id: u64,
    season_number: u32,
    name: String,
    overview: String,
    air_date: Option<String>,
    episode_count: u32,
    poster_path: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ClassificationsTv {
    results: Vec<ClassificationTv>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct ClassificationTv {
    iso_3166_1: String,
    rating: String,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct MotsSerie {
    results: Vec<Mot>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct TraductionsSerie {
    translations: Vec<TraductionSerie>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct TraductionSerie {
    iso_639_1: String,
    iso_3166_1: String,
    data: DonneesTraductionSerie,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct DonneesTraductionSerie {
    name: String,
    overview: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct GroupesRef {
    results: Vec<GroupeRef>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct GroupeRef {
    id: String,
    name: String,
    #[serde(rename = "type")]
    type_groupe: u32,
    episode_count: u32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DetailGroupe {
    groups: Vec<SousGroupe>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct SousGroupe {
    name: String,
    order: u32,
    episodes: Vec<EpisodeDeGroupe>,
}
#[derive(Deserialize, Default)]
#[serde(default)]
struct EpisodeDeGroupe {
    id: u64,
    order: u32,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct DetailSaison {
    episodes: Vec<EpisodeTmdb>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct EpisodeTmdb {
    id: u64,
    name: String,
    overview: String,
    air_date: Option<String>,
    season_number: u32,
    episode_number: u32,
    runtime: Option<u32>,
    vote_average: f32,
    vote_count: u32,
    still_path: Option<String>,
    guest_stars: Vec<MembreDistribution>,
    crew: Vec<MembreEquipe>,
    external_ids: IdsExternes,
    credits: Credits,
    images: ImagesEpisode,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ImagesEpisode {
    stills: Vec<ImageTmdb>,
}

// ---------------------------------------------------------------- utilitaires

/// Statut TMDB -> statut de diffusion.
pub fn statut_depuis_tmdb(statut: &str) -> StatutDiffusion {
    match statut.trim().to_lowercase().as_str() {
        "returning series" | "in production" | "planned" | "pilot" => StatutDiffusion::EnCours,
        "ended" | "canceled" | "cancelled" => StatutDiffusion::Terminee,
        _ => StatutDiffusion::Inconnu,
    }
}

/// Type de groupe d'épisodes TMDB -> type interne (`None` = ignoré).
pub fn type_groupe_depuis_tmdb(t: u32) -> Option<TypeGroupeEpisodes> {
    match t {
        1 | 4..=7 => Some(TypeGroupeEpisodes::Alternatif),
        2 => Some(TypeGroupeEpisodes::Absolu),
        3 => Some(TypeGroupeEpisodes::Dvd),
        _ => None,
    }
}

fn traduction<'a>(trad: &'a [TraductionSerie], langue: &str) -> Option<&'a TraductionSerie> {
    let (l, p) = match langue.split_once(['_', '-']) {
        Some((l, p)) => (l.to_lowercase(), p.to_uppercase()),
        None => (langue.to_lowercase(), String::new()),
    };
    trad.iter()
        .find(|t| t.iso_639_1.eq_ignore_ascii_case(&l) && (p.is_empty() || t.iso_3166_1 == p))
        .or_else(|| trad.iter().find(|t| t.iso_639_1.eq_ignore_ascii_case(&l)))
}

fn ajouter_ids_externes(ids: &mut std::collections::BTreeMap<String, String>, e: &IdsExternes) {
    if let Some(i) = non_vide(&e.imdb_id) {
        ids.insert(cles_id::IMDB.into(), i.to_string());
    }
    if let Some(w) = non_vide(&e.wikidata_id) {
        ids.insert(cles_id_serie::WIKIDATA.into(), w.to_string());
    }
    for (cle, v) in [
        (cles_id::TVDB, e.tvdb_id),
        (cles_id_serie::TVRAGE, e.tvrage_id),
    ] {
        if let Some(v) = v.filter(|v| *v > 0) {
            ids.insert(cle.into(), v.to_string());
        }
    }
}

fn erreur_404_en_aucun(e: ErreurScraper) -> ErreurScraper {
    match e {
        ErreurScraper::Http { statut: 404, .. } => ErreurScraper::AucunResultat,
        autre => autre,
    }
}

/// Numéros d'épisodes par identifiant TMDB d'épisode, pour chaque groupe chargé.
type NumerosParEpisode = HashMap<u64, Vec<(GroupeEpisodes, u32, u32)>>;

impl TmdbSerie {
    /// Crée le scraper. Échoue si la clé est vide (aucune clé par défaut n'existe).
    pub fn new(config: ConfigTmdb, http: ClientHttp) -> Resultat<Self> {
        Ok(Self {
            interne: TmdbFilm::new(config, http)?,
            cache_episodes: Mutex::new(HashMap::new()),
        })
    }

    fn config(&self) -> &ConfigTmdb {
        &self.interne.config
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        chemin: &str,
        params: &[(&str, Option<&str>)],
    ) -> Resultat<T> {
        self.interne.get(chemin, params).await
    }

    async fn base_images(&self) -> Resultat<String> {
        self.interne.base_images().await.map(str::to_string)
    }

    /// Identifiant TMDB de la série : `tmdb`, sinon `imdb` puis `tvdb` via `find`.
    async fn resoudre_id(&self, options: &OptionsRecherche) -> Resultat<u64> {
        let id = options.id_tmdb();
        if id > 0 {
            return Ok(id);
        }
        let langue = langue_requete(&options.langue);
        let imdb = options.id_imdb();
        let tvdb = options
            .ids
            .get(cles_id::TVDB)
            .map(|s| s.trim().to_string())
            .filter(|s| s.parse::<u64>().is_ok_and(|n| n > 0));
        let mut essais = Vec::new();
        if !imdb.is_empty() {
            essais.push((imdb, "imdb_id"));
        }
        if let Some(t) = tvdb {
            essais.push((t, "tvdb_id"));
        }
        if essais.is_empty() {
            return Err(ErreurScraper::IdentifiantManquant(
                "tmdb, imdb, tvdb".into(),
            ));
        }
        for (ext, source) in essais {
            let r: ReponseTrouverSerie = self
                .get(
                    &format!("find/{ext}"),
                    &[
                        ("external_source", Some(source)),
                        ("language", Some(&langue)),
                    ],
                )
                .await?;
            if let Some(e) = r.tv_results.first() {
                return Ok(e.id);
            }
        }
        Err(ErreurScraper::AucunResultat)
    }

    async fn detail(&self, id: u64, langue: &str, ajouts: &str) -> Resultat<DetailSerie> {
        self.get(
            &format!("tv/{id}"),
            &[
                ("language", Some(langue).filter(|l| !l.is_empty())),
                ("append_to_response", Some(ajouts)),
            ],
        )
        .await
        .map_err(erreur_404_en_aucun)
    }

    fn resultat_depuis(
        e: &ElementSerie,
        base_images: &str,
        options: &OptionsRecherche,
    ) -> ResultatRecherche {
        let mut r = ResultatRecherche {
            fournisseur: ID_FOURNISSEUR.into(),
            titre: e.name.clone(),
            titre_original: e.original_name.clone(),
            langue_originale: e.original_language.clone(),
            resume: e.overview.clone(),
            annee: annee_de(&e.first_air_date),
            url_affiche: non_vide(&e.poster_path)
                .map(|p| TmdbFilm::url_image(base_images, "w342", p))
                .unwrap_or_default(),
            ..Default::default()
        };
        r.ids.insert(cles_id::TMDB.into(), e.id.to_string());
        calculer_score(options, &mut r);
        r
    }

    /// Charge le contenu des groupes d'épisodes non vides.
    async fn charger_groupes(
        &self,
        refs: &GroupesRef,
        langue: &str,
    ) -> Vec<(GroupeEpisodes, DetailGroupe)> {
        let mut sortie = Vec::new();
        for g in refs
            .results
            .iter()
            .filter(|g| g.episode_count != 0 && !g.id.is_empty())
        {
            let Some(t) = type_groupe_depuis_tmdb(g.type_groupe) else {
                continue;
            };
            match self
                .get::<DetailGroupe>(
                    &format!("tv/episode_group/{}", g.id),
                    &[("language", Some(langue))],
                )
                .await
            {
                Ok(d) => sortie.push((GroupeEpisodes::nouveau(t, g.name.trim()), d)),
                Err(e) => tracing::warn!("groupe d'épisodes {} indisponible : {e}", g.id),
            }
        }
        sortie
    }

    fn numeros_des_groupes(groupes: &[(GroupeEpisodes, DetailGroupe)]) -> NumerosParEpisode {
        let mut m: NumerosParEpisode = HashMap::new();
        for (g, d) in groupes {
            for sg in &d.groups {
                for e in &sg.episodes {
                    m.entry(e.id)
                        .or_default()
                        .push((g.clone(), sg.order, e.order + 1));
                }
            }
        }
        m
    }

    fn convertir_serie(
        &self,
        d: &DetailSerie,
        options: &OptionsRecherche,
        base: &str,
        groupes: &[(GroupeEpisodes, DetailGroupe)],
    ) -> MetadonneesSerie {
        let cfg = self.config();
        let langue = options.langue.as_str();
        let statut = statut_depuis_tmdb(&d.status);
        let mut m = MetadonneesSerie {
            fournisseur: ID_FOURNISSEUR.into(),
            titre: d.name.trim().to_string(),
            titre_original: d.original_name.trim().to_string(),
            langue_originale: d.original_language.clone(),
            annee: annee_de(&d.first_air_date),
            annee_fin: if statut == StatutDiffusion::Terminee {
                annee_de(&d.last_air_date)
            } else {
                0
            },
            date_premiere_diffusion: non_vide(&d.first_air_date).and_then(date_iso),
            resume: d.overview.trim().to_string(),
            accroche: d.tagline.trim().to_string(),
            duree: d.episode_run_time.first().copied().unwrap_or(0),
            statut,
            pays: d
                .origin_country
                .iter()
                .filter(|p| !p.is_empty())
                .cloned()
                .collect(),
            ..Default::default()
        };
        m.ids.insert(cles_id::TMDB.into(), d.id.to_string());
        ajouter_ids_externes(&mut m.ids, &d.external_ids);
        let trad = &d.translations.translations;
        if cfg.repli_titre {
            if m.titre.is_empty()
                && let Some(t) = traduction(trad, &cfg.langue_repli)
            {
                m.titre = t.data.name.trim().to_string();
            }
            if m.titre.is_empty() {
                m.titre = m.titre_original.clone();
            }
            if m.resume.is_empty()
                && let Some(t) = traduction(trad, &cfg.langue_repli)
            {
                m.resume = t.data.overview.trim().to_string();
            }
        }
        m.titre_anglais = trad
            .iter()
            .find(|t| t.iso_639_1 == "en" && !t.data.name.trim().is_empty())
            .map(|t| t.data.name.trim().to_string())
            .or_else(|| (d.original_language == "en").then(|| m.titre_original.clone()))
            .unwrap_or_default();
        if d.vote_count > 0 || d.vote_average > 0.0 {
            m.evaluations.push(Evaluation {
                source: "tmdb".into(),
                valeur: d.vote_average,
                votes: d.vote_count,
                maximum: 10.0,
            });
        }
        for s in d.networks.iter().chain(d.production_companies.iter()) {
            let n = s.name.trim().to_string();
            if !n.is_empty() && !m.societes_production.contains(&n) {
                m.societes_production.push(n);
            }
        }
        for g in &d.genres {
            let nom = genre_depuis_id_tmdb(g.id)
                .map(str::to_string)
                .unwrap_or_else(|| g.name.clone());
            if !nom.is_empty() && !m.genres.contains(&nom) {
                m.genres.push(nom);
            }
        }
        for c in d.content_ratings.results.iter().filter(|c| {
            c.iso_3166_1
                .eq_ignore_ascii_case(&options.pays_classification)
                && !c.rating.trim().is_empty()
        }) {
            m.classifications.push(Classification {
                pays: c.iso_3166_1.clone(),
                valeur: c.rating.trim().to_string(),
            });
        }
        m.mots_cles = d
            .keywords
            .results
            .iter()
            .map(|k| k.name.clone())
            .filter(|k| !k.is_empty())
            .collect();
        for a in d.credits.cast.iter().filter(|a| !a.name.trim().is_empty()) {
            m.personnes.push(personne(
                TypePersonne::Acteur,
                a.id,
                &a.name,
                &a.character,
                &a.profile_path,
                base,
            ));
        }
        for c in &d.credits.crew {
            if let Some(t) = type_equipe(&c.job, &c.department)
                && !c.name.trim().is_empty()
            {
                m.personnes
                    .push(personne(t, c.id, &c.name, &c.job, &c.profile_path, base));
            }
        }
        if let Some(p) = non_vide(&d.poster_path) {
            m.artwork.push(ArtworkEtendu {
                fournisseur: ID_FOURNISSEUR.into(),
                type_artwork: TypeArtworkEtendu::Affiche,
                url_apercu: TmdbFilm::url_image(base, "w342", p),
                url_originale: TmdbFilm::url_image(base, "original", p),
                langue: langue.split(['_', '-']).next().unwrap_or("").to_string(),
                votes: 0,
                saison: None,
                tailles: Vec::new(),
            });
        }
        // Saisons en ordre de diffusion.
        m.groupes_episodes.push(GroupeEpisodes::diffusion());
        for s in &d.seasons {
            let mut saison = MetadonneesSaison {
                groupe: GroupeEpisodes::diffusion(),
                numero: s.season_number,
                nom: s.name.trim().to_string(),
                resume: s.overview.trim().to_string(),
                date_diffusion: non_vide(&s.air_date).and_then(date_iso),
                nombre_episodes: s.episode_count,
                url_affiche: non_vide(&s.poster_path)
                    .map(|p| TmdbFilm::url_image(base, "w342", p))
                    .unwrap_or_default(),
                ..Default::default()
            };
            if s.id > 0 {
                saison.ids.insert(cles_id::TMDB.into(), s.id.to_string());
            }
            m.saisons.push(saison);
        }
        // Saisons des groupes alternatifs (saisons vides retirées).
        for (g, dg) in groupes {
            if !m.groupes_episodes.contains(g) {
                m.groupes_episodes.push(g.clone());
            }
            for sg in dg.groups.iter().filter(|sg| !sg.episodes.is_empty()) {
                m.saisons.push(MetadonneesSaison {
                    groupe: g.clone(),
                    numero: sg.order,
                    nom: sg.name.trim().to_string(),
                    nombre_episodes: sg.episodes.len() as u32,
                    ..Default::default()
                });
            }
        }
        m
    }

    fn convertir_episode(e: &EpisodeTmdb, base: &str) -> MetadonneesEpisode {
        let mut m = MetadonneesEpisode {
            fournisseur: ID_FOURNISSEUR.into(),
            titre: e.name.trim().to_string(),
            resume: e.overview.trim().to_string(),
            date_diffusion: non_vide(&e.air_date).and_then(date_iso),
            duree: e.runtime.unwrap_or(0),
            ..Default::default()
        };
        if e.id > 0 {
            m.ids.insert(cles_id::TMDB.into(), e.id.to_string());
        }
        ajouter_ids_externes(&mut m.ids, &e.external_ids);
        m.definir_numero(
            GroupeEpisodes::diffusion(),
            e.season_number,
            e.episode_number,
        );
        if e.vote_count > 0 {
            m.evaluations.push(Evaluation {
                source: "tmdb".into(),
                valeur: e.vote_average,
                votes: e.vote_count,
                maximum: 10.0,
            });
        }
        for a in e.credits.cast.iter().filter(|a| !a.name.trim().is_empty()) {
            m.personnes.push(personne(
                TypePersonne::Acteur,
                a.id,
                &a.name,
                &a.character,
                &a.profile_path,
                base,
            ));
        }
        for a in e.guest_stars.iter().filter(|a| !a.name.trim().is_empty()) {
            let p = personne(
                TypePersonne::Invite,
                a.id,
                &a.name,
                &a.character,
                &a.profile_path,
                base,
            );
            if !m
                .personnes
                .iter()
                .any(|x| x.nom == p.nom && x.type_personne == p.type_personne)
            {
                m.personnes.push(p);
            }
        }
        for c in e.crew.iter().chain(e.credits.crew.iter()) {
            if let Some(t) = type_equipe(&c.job, &c.department)
                && !c.name.trim().is_empty()
            {
                let p = personne(t, c.id, &c.name, &c.job, &c.profile_path, base);
                if !m
                    .personnes
                    .iter()
                    .any(|x| x.nom == p.nom && x.type_personne == p.type_personne)
                {
                    m.personnes.push(p);
                }
            }
        }
        // Vignettes : images de l'épisode, sinon `still_path`.
        let mut stills = e.images.stills.clone();
        if stills.is_empty()
            && let Some(p) = non_vide(&e.still_path)
        {
            stills.push(ImageTmdb {
                file_path: p.to_string(),
                ..Default::default()
            });
        }
        m.artwork = construire_artwork(
            &stills,
            TypeArtwork::Vignette,
            "w300",
            &[1280, 300],
            base,
            false,
        )
        .into_iter()
        .map(ArtworkEtendu::from)
        .collect();
        m
    }

    /// Détail d'un épisode par numéro de diffusion.
    async fn episode_par_numero(
        &self,
        id_serie: u64,
        saison: u32,
        episode: u32,
        langue: &str,
        base: &str,
    ) -> Resultat<MetadonneesEpisode> {
        let e: EpisodeTmdb = self
            .get(
                &format!("tv/{id_serie}/season/{saison}/episode/{episode}"),
                &[
                    ("language", Some(langue)),
                    ("append_to_response", Some("external_ids,credits,images")),
                ],
            )
            .await
            .map_err(erreur_404_en_aucun)?;
        Ok(Self::convertir_episode(&e, base))
    }

    /// Complète titres génériques et résumés vides d'une saison par la langue de repli.
    async fn completer_par_repli(&self, id: u64, saison: u32, episodes: &mut [MetadonneesEpisode]) {
        let cfg = self.config();
        if !cfg.repli_titre || cfg.langue_repli.trim().is_empty() {
            return;
        }
        let a_completer = episodes
            .iter()
            .any(|e| est_titre_generique(&e.titre) || e.resume.is_empty());
        if !a_completer {
            return;
        }
        let langue = langue_requete(&cfg.langue_repli);
        let Ok(r) = self
            .get::<DetailSaison>(
                &format!("tv/{id}/season/{saison}"),
                &[("language", Some(&langue))],
            )
            .await
        else {
            return;
        };
        for e in episodes.iter_mut() {
            let Some(src) = r
                .episodes
                .iter()
                .find(|x| e.ids.get(cles_id::TMDB) == Some(&x.id.to_string()))
            else {
                continue;
            };
            if est_titre_generique(&e.titre)
                && !src.name.trim().is_empty()
                && !est_titre_generique(&src.name)
            {
                e.titre = src.name.trim().to_string();
            }
            if e.resume.is_empty() {
                e.resume = src.overview.trim().to_string();
            }
        }
    }

    fn lire_cache(&self, cle: &str) -> Option<Vec<MetadonneesEpisode>> {
        let cache = self.cache_episodes.lock().ok()?;
        cache
            .get(cle)
            .filter(|(t, _)| t.elapsed() < TTL_LISTE_EPISODES)
            .map(|(_, v)| v.clone())
    }

    fn ecrire_cache(&self, cle: String, v: &[MetadonneesEpisode]) {
        if let Ok(mut cache) = self.cache_episodes.lock() {
            cache.retain(|_, (t, _)| t.elapsed() < TTL_LISTE_EPISODES);
            cache.insert(cle, (Instant::now(), v.to_vec()));
        }
    }
}

// ---------------------------------------------------------------- traits

#[async_trait]
impl FournisseurSerie for TmdbSerie {
    fn identifiant(&self) -> &'static str {
        ID_FOURNISSEUR
    }

    async fn rechercher(&self, options: &OptionsRecherche) -> Resultat<Vec<ResultatRecherche>> {
        let base = self.base_images().await?;
        let langue = langue_requete(&options.langue);
        let requete =
            nettoyer_caracteres(&retirer_nom_triable(&options.requete, &PREFIXES_ARTICLES))
                .trim()
                .to_string();
        let mut elements: Vec<ElementSerie> = Vec::new();
        let mut derniere_erreur: Option<ErreurScraper> = None;
        let a_un_id = options.id_tmdb() > 0
            || !options.id_imdb().is_empty()
            || options.ids.contains_key(cles_id::TVDB);
        if a_un_id {
            match self.resoudre_id(options).await {
                Ok(id) => match self
                    .get::<ElementSerie>(&format!("tv/{id}"), &[("language", Some(&langue))])
                    .await
                {
                    Ok(e) => elements.push(e),
                    Err(ErreurScraper::CleApiInvalide) => {
                        return Err(ErreurScraper::CleApiInvalide);
                    }
                    Err(e) => derniere_erreur = Some(e),
                },
                Err(ErreurScraper::CleApiInvalide) => return Err(ErreurScraper::CleApiInvalide),
                Err(e) => derniere_erreur = Some(e),
            }
        }
        if elements.is_empty() && !requete.is_empty() {
            let adulte = if self.config().inclure_adulte {
                "true"
            } else {
                "false"
            };
            let mut page = 1u32;
            loop {
                let p = page.to_string();
                match self
                    .get::<PageSeries>(
                        "search/tv",
                        &[
                            ("query", Some(&requete)),
                            ("page", Some(&p)),
                            ("language", Some(&langue)),
                            ("include_adult", Some(adulte)),
                        ],
                    )
                    .await
                {
                    Ok(r) => {
                        elements.extend(r.results);
                        if page >= r.total_pages.min(PAGES_MAX) {
                            break;
                        }
                        page += 1;
                    }
                    Err(ErreurScraper::CleApiInvalide) => {
                        return Err(ErreurScraper::CleApiInvalide);
                    }
                    Err(e) => {
                        derniere_erreur = Some(e);
                        break;
                    }
                }
            }
        }
        if elements.is_empty()
            && let Some(e) = derniere_erreur
        {
            return Err(e);
        }
        let mut vus = BTreeSet::new();
        let mut opts = options.clone();
        if a_un_id && elements.len() == 1 {
            // Résultat obtenu par identifiant : score forcé à 1.0.
            opts.ids
                .insert(cles_id::TMDB.into(), elements[0].id.to_string());
        }
        let mut v: Vec<ResultatRecherche> = elements
            .iter()
            .filter(|e| e.id > 0 && vus.insert(e.id))
            .map(|e| Self::resultat_depuis(e, &base, &opts))
            .collect();
        trier_resultats(&mut v);
        Ok(v)
    }

    async fn metadonnees_serie(&self, options: &OptionsRecherche) -> Resultat<MetadonneesSerie> {
        let base = self.base_images().await?;
        let id = self.resoudre_id(options).await?;
        let langue = langue_requete(&options.langue);
        let d = self.detail(id, &langue, AJOUTS_DETAIL).await?;
        let groupes = self.charger_groupes(&d.episode_groups, &langue).await;
        Ok(self.convertir_serie(&d, options, &base, &groupes))
    }

    async fn liste_episodes(
        &self,
        options: &OptionsRecherche,
    ) -> Resultat<Vec<MetadonneesEpisode>> {
        let id = self.resoudre_id(options).await?;
        let langue = langue_requete(&options.langue);
        let cle = format!("{id}_{langue}");
        if let Some(v) = self.lire_cache(&cle) {
            return Ok(v);
        }
        let base = self.base_images().await?;
        let d = self.detail(id, &langue, AJOUTS_DETAIL).await?;
        let groupes = self.charger_groupes(&d.episode_groups, &langue).await;
        let numeros = Self::numeros_des_groupes(&groupes);
        let mut tout = Vec::new();
        for s in &d.seasons {
            let r: DetailSaison = match self
                .get(
                    &format!("tv/{id}/season/{}", s.season_number),
                    &[
                        ("language", Some(&langue)),
                        ("append_to_response", Some("credits")),
                    ],
                )
                .await
            {
                Ok(r) => r,
                Err(ErreurScraper::CleApiInvalide) => return Err(ErreurScraper::CleApiInvalide),
                Err(e) => {
                    tracing::warn!("saison {} indisponible : {e}", s.season_number);
                    continue;
                }
            };
            let mut episodes: Vec<MetadonneesEpisode> = r
                .episodes
                .iter()
                .map(|e| {
                    let mut m = Self::convertir_episode(e, &base);
                    for (g, sn, en) in numeros.get(&e.id).into_iter().flatten() {
                        m.definir_numero(g.clone(), *sn, *en);
                    }
                    m
                })
                .collect();
            self.completer_par_repli(id, s.season_number, &mut episodes)
                .await;
            tout.extend(episodes);
        }
        self.ecrire_cache(cle, &tout);
        Ok(tout)
    }

    async fn metadonnees_episode(&self, options: &OptionsEpisode) -> Resultat<MetadonneesEpisode> {
        let base = self.base_images().await?;
        let langue = langue_requete(&options.serie.langue);
        let id_episode = options
            .ids_episode
            .get(cles_id::TMDB)
            .is_some_and(|s| !s.trim().is_empty() && s.trim() != "0");
        // Identifiant IMDb ou TVDB de l'épisode -> find.
        if !id_episode {
            for (cle, source) in [(cles_id::IMDB, "imdb_id"), (cles_id::TVDB, "tvdb_id")] {
                let Some(ext) = options
                    .ids_episode
                    .get(cle)
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                else {
                    continue;
                };
                let r: ReponseTrouverSerie = self
                    .get(
                        &format!("find/{ext}"),
                        &[
                            ("external_source", Some(source)),
                            ("language", Some(&langue)),
                        ],
                    )
                    .await?;
                if let Some(t) = r.tv_episode_results.first() {
                    return self
                        .episode_par_numero(
                            t.show_id,
                            t.season_number,
                            t.episode_number,
                            &langue,
                            &base,
                        )
                        .await;
                }
            }
        }
        let id_serie = self.resoudre_id(&options.serie).await?;
        // Raccourci : numéro de diffusion connu, pas d'identifiant d'épisode.
        if !id_episode
            && options.groupe.type_groupe == TypeGroupeEpisodes::Diffusion
            && let (Some(s), Some(e)) = (options.saison, options.episode)
        {
            return self
                .episode_par_numero(id_serie, s, e, &langue, &base)
                .await;
        }
        let mut serie = options.serie.clone();
        serie.ids.insert(cles_id::TMDB.into(), id_serie.to_string());
        let liste = self.liste_episodes(&serie).await?;
        let trouve = apparier_episode(&liste, options, ReglesAppariement::TMDB)?;
        let (s, e) = trouve
            .numero_diffusion()
            .ok_or(ErreurScraper::AucunResultat)?;
        let mut m = self
            .episode_par_numero(id_serie, s, e, &langue, &base)
            .await?;
        for n in &trouve.numeros {
            m.definir_numero(n.groupe.clone(), n.saison, n.episode);
        }
        Ok(m)
    }
}

#[async_trait]
impl FournisseurArtworkSerie for TmdbSerie {
    async fn artwork_serie(
        &self,
        options: &OptionsRecherche,
        type_filtre: Option<TypeArtworkEtendu>,
    ) -> Resultat<Vec<ArtworkEtendu>> {
        let filtre_base = match type_filtre {
            None | Some(TypeArtworkEtendu::AfficheSaison) => None,
            Some(t) => match t.en_type_base() {
                Some(b) => Some(b),
                // Types non fournis par TMDB.
                None => return Ok(Vec::new()),
            },
        };
        let base = self.base_images().await?;
        let id = self.resoudre_id(options).await?;
        let avec_saisons = matches!(type_filtre, None | Some(TypeArtworkEtendu::AfficheSaison));
        let mut sortie = Vec::new();
        if avec_saisons {
            let d = self.detail(id, "", "images").await?;
            if type_filtre.is_none() {
                sortie.extend(
                    artwork_depuis_images(d.images, options, &base, None)
                        .into_iter()
                        .map(ArtworkEtendu::from),
                );
            }
            let langue = options.langue.split(['_', '-']).next().unwrap_or("en");
            for i in 0..=d.number_of_seasons {
                match self
                    .get::<ReponseImages>(&format!("tv/{id}/season/{i}/images"), &[])
                    .await
                {
                    Ok(mut r) => {
                        trier_images(&mut r.posters, langue);
                        sortie.extend(
                            construire_artwork(
                                &r.posters,
                                TypeArtwork::Affiche,
                                "w342",
                                &[780, 500, 342, 185],
                                &base,
                                false,
                            )
                            .into_iter()
                            .map(|a| {
                                ArtworkEtendu::depuis_artwork(
                                    a,
                                    TypeArtworkEtendu::AfficheSaison,
                                    Some(i),
                                )
                            }),
                        );
                    }
                    Err(ErreurScraper::CleApiInvalide) => {
                        return Err(ErreurScraper::CleApiInvalide);
                    }
                    Err(e) => tracing::debug!("images de la saison {i} indisponibles : {e}"),
                }
            }
        } else {
            let r: ReponseImages = self.get(&format!("tv/{id}/images"), &[]).await?;
            sortie.extend(
                artwork_depuis_images(r, options, &base, filtre_base)
                    .into_iter()
                    .map(ArtworkEtendu::from),
            );
        }
        Ok(sortie)
    }

    async fn artwork_episode(&self, options: &OptionsEpisode) -> Resultat<Vec<ArtworkEtendu>> {
        let base = self.base_images().await?;
        if options.groupe.type_groupe == TypeGroupeEpisodes::Diffusion
            && let (Some(s), Some(e)) = (options.saison, options.episode)
            && options.ids_episode.is_empty()
        {
            let id = self.resoudre_id(&options.serie).await?;
            let mut r: ImagesEpisode = self
                .get(&format!("tv/{id}/season/{s}/episode/{e}/images"), &[])
                .await
                .map_err(erreur_404_en_aucun)?;
            let langue = options
                .serie
                .langue
                .split(['_', '-'])
                .next()
                .unwrap_or("en");
            trier_images(&mut r.stills, langue);
            return Ok(construire_artwork(
                &r.stills,
                TypeArtwork::Vignette,
                "w300",
                &[1280, 300],
                &base,
                false,
            )
            .into_iter()
            .map(ArtworkEtendu::from)
            .collect());
        }
        Ok(self.metadonnees_episode(options).await?.artwork)
    }
}

#[async_trait]
impl FournisseurBandesAnnonces for TmdbSerie {
    async fn bandes_annonces(&self, options: &OptionsRecherche) -> Resultat<Vec<BandeAnnonce>> {
        let id = self.resoudre_id(options).await?;
        let langue = langue_requete(&options.langue);
        let mut videos = Vec::new();
        for l in [langue.as_str(), ""] {
            let r: ReponseVideos = self
                .get(&format!("tv/{id}/videos"), &[("language", Some(l))])
                .await?;
            videos.extend(r.results);
        }
        Ok(bandes_depuis_videos(videos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuts() {
        assert_eq!(
            statut_depuis_tmdb("Returning Series"),
            StatutDiffusion::EnCours
        );
        assert_eq!(statut_depuis_tmdb("Ended"), StatutDiffusion::Terminee);
        assert_eq!(statut_depuis_tmdb("Canceled"), StatutDiffusion::Terminee);
        assert_eq!(statut_depuis_tmdb("???"), StatutDiffusion::Inconnu);
    }

    #[test]
    fn types_de_groupes() {
        assert_eq!(
            type_groupe_depuis_tmdb(1),
            Some(TypeGroupeEpisodes::Alternatif)
        );
        assert_eq!(type_groupe_depuis_tmdb(2), Some(TypeGroupeEpisodes::Absolu));
        assert_eq!(type_groupe_depuis_tmdb(3), Some(TypeGroupeEpisodes::Dvd));
        assert_eq!(
            type_groupe_depuis_tmdb(6),
            Some(TypeGroupeEpisodes::Alternatif)
        );
        assert_eq!(type_groupe_depuis_tmdb(9), None);
    }
}
