//! Fournisseur TVmaze (séries uniquement, sans clé d'API).

use std::collections::{BTreeMap, HashMap};

use async_trait::async_trait;
use serde::Deserialize;

use crate::correspondance::{date_iso_valide, trier_resultats};
use crate::correspondance_serie::{ReglesAppariement, apparier_episode};
use crate::erreur::{ErreurScraper, Resultat};
use crate::fournisseur::{FournisseurArtworkSerie, FournisseurSerie};
use crate::http::ClientHttp;
use crate::types::*;

const URL_API: &str = "https://api.tvmaze.com/";
const ID_FOURNISSEUR: &str = "tvmaze";

/// Configuration du fournisseur TVmaze.
#[derive(Debug, Clone, Default)]
pub struct ConfigTvmaze {
    /// Surcharge de l'URL de base (tests, miroir). Se termine par `/`.
    pub url_api: Option<String>,
}

/// Scraper TVmaze.
#[derive(Debug)]
pub struct TvMaze {
    config: ConfigTvmaze,
    http: ClientHttp,
}

// ---------------------------------------------------------------- JSON

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct ImageTvmaze {
    medium: Option<String>,
    original: Option<String>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct NoteTvmaze {
    average: Option<f32>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct Externes {
    tvrage: Option<u64>,
    thetvdb: Option<u64>,
    imdb: Option<String>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct Pays {
    name: String,
    code: String,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct ChaineAvecPays {
    name: String,
    country: Option<Pays>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct Emission {
    id: u64,
    url: String,
    name: String,
    language: Option<String>,
    genres: Vec<String>,
    status: String,
    runtime: Option<u32>,
    #[serde(rename = "averageRuntime")]
    duree_moyenne: Option<u32>,
    premiered: Option<String>,
    ended: Option<String>,
    rating: NoteTvmaze,
    externals: Externes,
    image: Option<ImageTvmaze>,
    summary: Option<String>,
    network: Option<ChaineAvecPays>,
    #[serde(rename = "webChannel")]
    chaine_web: Option<ChaineAvecPays>,
    #[serde(rename = "_embedded")]
    inclus: InclusEmission,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct InclusEmission {
    seasons: Vec<SaisonTvmaze>,
    cast: Vec<RoleTvmaze>,
    crew: Vec<EquipeTvmaze>,
    images: Vec<ImageEmission>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct SaisonTvmaze {
    id: u64,
    number: Option<u32>,
    name: String,
    #[serde(rename = "episodeOrder")]
    nombre_episodes: Option<u32>,
    #[serde(rename = "premiereDate")]
    date_premiere: Option<String>,
    image: Option<ImageTvmaze>,
    summary: Option<String>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct PersonneTvmaze {
    id: u64,
    name: String,
    url: String,
    image: Option<ImageTvmaze>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct PersonnageTvmaze {
    name: String,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct RoleTvmaze {
    person: PersonneTvmaze,
    character: PersonnageTvmaze,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct EquipeTvmaze {
    #[serde(rename = "type", alias = "guestCrewType")]
    fonction: String,
    person: PersonneTvmaze,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct Resolution {
    url: String,
    width: u32,
    height: u32,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct Resolutions {
    original: Option<Resolution>,
    medium: Option<Resolution>,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct ImageEmission {
    #[serde(rename = "type")]
    type_image: Option<String>,
    resolutions: Resolutions,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ResultatTvmaze {
    score: f32,
    show: Emission,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct EpisodeTvmaze {
    id: u64,
    name: String,
    season: u32,
    number: Option<u32>,
    #[serde(rename = "type")]
    type_episode: String,
    airdate: Option<String>,
    runtime: Option<u32>,
    rating: NoteTvmaze,
    image: Option<ImageTvmaze>,
    summary: Option<String>,
    #[serde(rename = "_embedded")]
    inclus: InclusEpisode,
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct InclusEpisode {
    guestcast: Vec<RoleTvmaze>,
    guestcrew: Vec<EquipeTvmaze>,
    episodes: Vec<EpisodeTvmaze>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ListeAlternative {
    id: u64,
    dvd_release: bool,
    verbatim_order: bool,
    country_premiere: bool,
    streaming_premiere: bool,
    broadcast_premiere: bool,
    language_premiere: bool,
    language: Option<String>,
    network: Option<ChaineAvecPays>,
    #[serde(rename = "webChannel")]
    chaine_web: Option<ChaineAvecPays>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct EpisodeAlternatif {
    season: u32,
    number: u32,
    #[serde(rename = "_embedded")]
    inclus: InclusEpisode,
}

// ---------------------------------------------------------------- utilitaires

/// Texte brut à partir d'un fragment HTML simple (balises retirées, entités courantes décodées).
pub fn html_en_texte(html: &str) -> String {
    let mut sortie = String::with_capacity(html.len());
    let mut dans_balise = false;
    let mut balise = String::new();
    for c in html.chars() {
        match c {
            '<' => {
                dans_balise = true;
                balise.clear();
            }
            '>' if dans_balise => {
                dans_balise = false;
                let b = balise.trim_start_matches('/').to_lowercase();
                let nom = b.split([' ', '/']).next().unwrap_or("");
                if matches!(nom, "br" | "p" | "div" | "li")
                    && !sortie.ends_with('\n')
                    && !sortie.is_empty()
                {
                    sortie.push('\n');
                }
            }
            _ if dans_balise => balise.push(c),
            _ => sortie.push(c),
        }
    }
    let s = sortie
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    s.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Statut TVmaze -> statut de diffusion.
pub fn statut_depuis_tvmaze(statut: &str) -> StatutDiffusion {
    match statut.trim().to_lowercase().as_str() {
        "running" => StatutDiffusion::EnCours,
        "ended" => StatutDiffusion::Terminee,
        _ => StatutDiffusion::Inconnu,
    }
}

fn type_equipe_tvmaze(fonction: &str) -> Option<TypePersonne> {
    Some(match fonction.trim() {
        "Executive Producer" | "Producer" | "Co-Executive Producer" => TypePersonne::Producteur,
        "Director Of Photography" | "Director of Photography" => TypePersonne::Photographie,
        "Creator" | "Writer" | "Developer" => TypePersonne::Scenariste,
        "Editor" => TypePersonne::Monteur,
        "Music" | "Composer" => TypePersonne::Compositeur,
        "Director" => TypePersonne::Realisateur,
        _ => return None,
    })
}

fn annee(date: &Option<String>) -> i32 {
    date.as_deref()
        .and_then(|d| d.get(..4))
        .and_then(|a| a.parse().ok())
        .unwrap_or(0)
}

fn date(d: &Option<String>) -> Option<String> {
    d.as_deref().and_then(date_iso_valide)
}

fn personne(t: TypePersonne, p: &PersonneTvmaze, role: &str) -> Personne {
    let mut ids = BTreeMap::new();
    if p.id > 0 {
        ids.insert(ID_FOURNISSEUR.to_string(), p.id.to_string());
    }
    Personne {
        type_personne: t,
        nom: p.name.trim().to_string(),
        role: role.trim().to_string(),
        ids,
        url_photo: p
            .image
            .as_ref()
            .and_then(|i| i.original.clone().or_else(|| i.medium.clone()))
            .unwrap_or_default(),
        url_profil: p.url.clone(),
    }
}

fn artwork_simple(
    image: &ImageTvmaze,
    t: TypeArtworkEtendu,
    saison: Option<u32>,
) -> Option<ArtworkEtendu> {
    let originale = image.original.clone().or_else(|| image.medium.clone())?;
    Some(ArtworkEtendu {
        fournisseur: ID_FOURNISSEUR.into(),
        type_artwork: t,
        url_apercu: image.medium.clone().unwrap_or_else(|| originale.clone()),
        url_originale: originale,
        langue: String::new(),
        votes: 0,
        saison,
        tailles: Vec::new(),
    })
}

fn ids_emission(e: &Emission) -> BTreeMap<String, String> {
    let mut ids = BTreeMap::new();
    ids.insert(ID_FOURNISSEUR.to_string(), e.id.to_string());
    if let Some(i) = e
        .externals
        .imdb
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        ids.insert(cles_id::IMDB.into(), i.to_string());
    }
    if let Some(t) = e.externals.thetvdb.filter(|n| *n > 0) {
        ids.insert(cles_id::TVDB.into(), t.to_string());
    }
    if let Some(t) = e.externals.tvrage.filter(|n| *n > 0) {
        ids.insert(cles_id_serie::TVRAGE.into(), t.to_string());
    }
    ids
}

/// Groupe d'épisodes correspondant à une liste alternative TVmaze.
fn groupe_alternatif(l: &ListeAlternative) -> Option<GroupeEpisodes> {
    let chaine = l
        .network
        .as_ref()
        .or(l.chaine_web.as_ref())
        .cloned()
        .unwrap_or_default();
    let pays = chaine
        .country
        .as_ref()
        .map(|p| {
            if p.code.is_empty() {
                p.name.clone()
            } else {
                p.code.clone()
            }
        })
        .unwrap_or_default();
    let suffixe = if pays.is_empty() {
        chaine.name.clone()
    } else {
        format!("{} ({pays})", chaine.name)
    };
    let alt = |prefixe: &str| {
        let nom = if suffixe.trim().is_empty() {
            prefixe.to_string()
        } else {
            format!("{prefixe} - {suffixe}")
        };
        GroupeEpisodes::nouveau(TypeGroupeEpisodes::Alternatif, nom)
    };
    Some(if l.dvd_release {
        GroupeEpisodes::dvd()
    } else if l.verbatim_order {
        GroupeEpisodes::nouveau(TypeGroupeEpisodes::Alternatif, "Verbatim")
    } else if l.country_premiere {
        alt("Country Premiere")
    } else if l.streaming_premiere {
        alt("Streaming Premiere")
    } else if l.broadcast_premiere {
        alt("Broadcast Premiere")
    } else if l.language_premiere {
        GroupeEpisodes::nouveau(
            TypeGroupeEpisodes::Alternatif,
            format!(
                "Language Premiere ({})",
                l.language.clone().unwrap_or_default()
            ),
        )
    } else {
        return None;
    })
}

impl TvMaze {
    /// Crée le scraper (aucune clé requise).
    pub fn new(config: ConfigTvmaze, http: ClientHttp) -> Self {
        Self { config, http }
    }

    fn base(&self) -> &str {
        self.config.url_api.as_deref().unwrap_or(URL_API)
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        chemin: &str,
        params: &[(&str, Option<&str>)],
    ) -> Resultat<T> {
        let url = format!("{}{}", self.base(), chemin);
        self.http.obtenir_json(&url, params, &[]).await
    }

    /// Recherche par identifiant (`tvmaze`, `imdb`, `tvdb`, `tvrage`) ; 404 = pas de résultat.
    async fn par_identifiant(&self, options: &OptionsRecherche) -> Resultat<Option<Emission>> {
        let id = |cle: &str| {
            options
                .ids
                .get(cle)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && s != "0")
        };
        let essais: Vec<(String, Vec<(&str, String)>)> = [
            id(cles_id_serie::TVMAZE).map(|v| (format!("shows/{v}"), vec![])),
            Some(options.id_imdb())
                .filter(|s| !s.is_empty())
                .map(|v| ("lookup/shows".to_string(), vec![("imdb", v)])),
            id(cles_id::TVDB).map(|v| ("lookup/shows".to_string(), vec![("thetvdb", v)])),
            id(cles_id_serie::TVRAGE).map(|v| ("lookup/shows".to_string(), vec![("tvrage", v)])),
        ]
        .into_iter()
        .flatten()
        .collect();
        for (chemin, params) in essais {
            let p: Vec<(&str, Option<&str>)> =
                params.iter().map(|(k, v)| (*k, Some(v.as_str()))).collect();
            match self.get::<Emission>(&chemin, &p).await {
                Ok(e) if e.id > 0 => return Ok(Some(e)),
                Ok(_) | Err(ErreurScraper::Http { statut: 404, .. }) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(None)
    }

    async fn resoudre_id(&self, options: &OptionsRecherche) -> Resultat<u64> {
        if let Some(id) = options
            .ids
            .get(cles_id_serie::TVMAZE)
            .and_then(|s| s.trim().parse::<u64>().ok())
            .filter(|n| *n > 0)
        {
            return Ok(id);
        }
        let a_un_id = !options.id_imdb().is_empty()
            || options.ids.contains_key(cles_id::TVDB)
            || options.ids.contains_key(cles_id_serie::TVRAGE);
        if !a_un_id {
            return Err(ErreurScraper::IdentifiantManquant(
                "tvmaze, imdb, tvdb, tvrage".into(),
            ));
        }
        self.par_identifiant(options)
            .await?
            .map(|e| e.id)
            .ok_or(ErreurScraper::AucunResultat)
    }

    async fn emission_complete(&self, id: u64) -> Resultat<Emission> {
        self.get(
            &format!("shows/{id}"),
            &[
                ("embed[]", Some("seasons")),
                ("embed[]", Some("crew")),
                ("embed[]", Some("cast")),
                ("embed[]", Some("images")),
            ],
        )
        .await
        .map_err(|e| match e {
            ErreurScraper::Http { statut: 404, .. } => ErreurScraper::AucunResultat,
            autre => autre,
        })
    }

    fn resultat_depuis(e: &Emission, score: f32) -> ResultatRecherche {
        ResultatRecherche {
            ids: ids_emission(e),
            fournisseur: ID_FOURNISSEUR.into(),
            titre: e.name.trim().to_string(),
            langue_originale: e.language.clone().unwrap_or_default(),
            resume: e.summary.as_deref().map(html_en_texte).unwrap_or_default(),
            annee: annee(&e.premiered),
            score,
            url_affiche: e
                .image
                .as_ref()
                .and_then(|i| i.medium.clone().or_else(|| i.original.clone()))
                .unwrap_or_default(),
            ..Default::default()
        }
    }

    fn artwork_emission(e: &Emission) -> Vec<ArtworkEtendu> {
        let mut v = Vec::new();
        if let Some(i) = &e.image
            && let Some(a) = artwork_simple(i, TypeArtworkEtendu::Affiche, None)
        {
            v.push(a);
        }
        for img in &e.inclus.images {
            let t = match img.type_image.as_deref().map(str::to_uppercase).as_deref() {
                Some("POSTER") => TypeArtworkEtendu::Affiche,
                Some("BANNER") => TypeArtworkEtendu::Banniere,
                Some("BACKGROUND") => TypeArtworkEtendu::Fond,
                _ => continue,
            };
            let Some(orig) = img
                .resolutions
                .original
                .as_ref()
                .filter(|r| !r.url.is_empty())
            else {
                continue;
            };
            if v.iter().any(|a| a.url_originale == orig.url) {
                continue;
            }
            let apercu = img
                .resolutions
                .medium
                .as_ref()
                .map(|r| r.url.clone())
                .unwrap_or_else(|| orig.url.clone());
            let mut tailles = vec![TailleImage {
                largeur: orig.width,
                hauteur: orig.height,
                url: orig.url.clone(),
                ordre: crate::fanart::ordre_taille_etendu(t, orig.width),
            }];
            if let Some(m) = &img.resolutions.medium {
                tailles.push(TailleImage {
                    largeur: m.width,
                    hauteur: m.height,
                    url: m.url.clone(),
                    ordre: crate::fanart::ordre_taille_etendu(t, m.width),
                });
            }
            v.push(ArtworkEtendu {
                fournisseur: ID_FOURNISSEUR.into(),
                type_artwork: t,
                url_apercu: apercu,
                url_originale: orig.url.clone(),
                langue: String::new(),
                votes: 0,
                saison: None,
                tailles,
            });
        }
        for s in &e.inclus.seasons {
            if let (Some(n), Some(i)) = (s.number, &s.image)
                && let Some(a) = artwork_simple(i, TypeArtworkEtendu::AfficheSaison, Some(n))
            {
                v.push(a);
            }
        }
        v
    }

    fn convertir_emission(e: &Emission, groupes: &[GroupeEpisodes]) -> MetadonneesSerie {
        let statut = statut_depuis_tvmaze(&e.status);
        let mut m = MetadonneesSerie {
            fournisseur: ID_FOURNISSEUR.into(),
            ids: ids_emission(e),
            titre: e.name.trim().to_string(),
            titre_original: e.name.trim().to_string(),
            langue_originale: e.language.clone().unwrap_or_default(),
            annee: annee(&e.premiered),
            annee_fin: if statut == StatutDiffusion::Terminee {
                annee(&e.ended)
            } else {
                0
            },
            date_premiere_diffusion: date(&e.premiered),
            resume: e.summary.as_deref().map(html_en_texte).unwrap_or_default(),
            duree: e.runtime.or(e.duree_moyenne).unwrap_or(0),
            statut,
            genres: e.genres.clone(),
            artwork: Self::artwork_emission(e),
            ..Default::default()
        };
        if let Some(n) = e.rating.average {
            m.evaluations.push(Evaluation {
                source: ID_FOURNISSEUR.into(),
                valeur: n,
                votes: 0,
                maximum: 10.0,
            });
        }
        for c in [&e.network, &e.chaine_web].into_iter().flatten() {
            if !c.name.trim().is_empty() {
                m.societes_production.push(c.name.trim().to_string());
            }
            if let Some(p) = &c.country
                && !p.code.is_empty()
                && !m.pays.contains(&p.code)
            {
                m.pays.push(p.code.clone());
            }
        }
        for r in &e.inclus.cast {
            if !r.person.name.trim().is_empty() {
                m.personnes
                    .push(personne(TypePersonne::Acteur, &r.person, &r.character.name));
            }
        }
        for c in &e.inclus.crew {
            if let Some(t) = type_equipe_tvmaze(&c.fonction)
                && !c.person.name.trim().is_empty()
            {
                m.personnes.push(personne(t, &c.person, &c.fonction));
            }
        }
        m.groupes_episodes.push(GroupeEpisodes::diffusion());
        for g in groupes {
            if !m.groupes_episodes.contains(g) {
                m.groupes_episodes.push(g.clone());
            }
        }
        for s in &e.inclus.seasons {
            let Some(n) = s.number else { continue };
            let mut saison = MetadonneesSaison {
                groupe: GroupeEpisodes::diffusion(),
                numero: n,
                nom: s.name.trim().to_string(),
                resume: s.summary.as_deref().map(html_en_texte).unwrap_or_default(),
                date_diffusion: date(&s.date_premiere),
                nombre_episodes: s.nombre_episodes.unwrap_or(0),
                url_affiche: s
                    .image
                    .as_ref()
                    .and_then(|i| i.original.clone().or_else(|| i.medium.clone()))
                    .unwrap_or_default(),
                ..Default::default()
            };
            if s.id > 0 {
                saison.ids.insert(ID_FOURNISSEUR.into(), s.id.to_string());
            }
            m.saisons.push(saison);
        }
        m
    }

    /// Listes alternatives et numérotation par identifiant d'épisode.
    async fn groupes_alternatifs(
        &self,
        id: u64,
    ) -> (
        Vec<GroupeEpisodes>,
        HashMap<u64, Vec<(GroupeEpisodes, u32, u32)>>,
    ) {
        let mut groupes = Vec::new();
        let mut numeros: HashMap<u64, Vec<(GroupeEpisodes, u32, u32)>> = HashMap::new();
        let listes: Vec<ListeAlternative> =
            match self.get(&format!("shows/{id}/alternatelists"), &[]).await {
                Ok(l) => l,
                Err(e) => {
                    tracing::debug!("listes alternatives indisponibles : {e}");
                    return (groupes, numeros);
                }
            };
        for l in listes {
            let Some(g) = groupe_alternatif(&l) else {
                continue;
            };
            let eps: Vec<EpisodeAlternatif> = match self
                .get(
                    &format!("alternatelists/{}/alternateepisodes", l.id),
                    &[("embed", Some("episodes"))],
                )
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::debug!("liste alternative {} indisponible : {e}", l.id);
                    continue;
                }
            };
            for a in eps {
                for e in &a.inclus.episodes {
                    numeros
                        .entry(e.id)
                        .or_default()
                        .push((g.clone(), a.season, a.number));
                }
            }
            if !groupes.contains(&g) {
                groupes.push(g);
            }
        }
        (groupes, numeros)
    }

    fn convertir_episode(e: &EpisodeTvmaze, rang_special: &mut u32) -> MetadonneesEpisode {
        let mut m = MetadonneesEpisode {
            fournisseur: ID_FOURNISSEUR.into(),
            titre: e.name.trim().to_string(),
            resume: e.summary.as_deref().map(html_en_texte).unwrap_or_default(),
            date_diffusion: date(&e.airdate),
            duree: e.runtime.unwrap_or(0),
            ..Default::default()
        };
        m.ids.insert(ID_FOURNISSEUR.into(), e.id.to_string());
        if e.type_episode.is_empty() || e.type_episode == "regular" {
            m.definir_numero(GroupeEpisodes::diffusion(), e.season, e.number.unwrap_or(0));
        } else {
            // Épisode spécial : saison 0 en diffusion, sa place réelle en affichage.
            *rang_special += 1;
            let n = e.number.unwrap_or(*rang_special);
            m.definir_numero(GroupeEpisodes::diffusion(), 0, n);
            m.definir_numero(
                GroupeEpisodes::nouveau(TypeGroupeEpisodes::Affichage, ""),
                e.season,
                n,
            );
        }
        if let Some(n) = e.rating.average {
            m.evaluations.push(Evaluation {
                source: ID_FOURNISSEUR.into(),
                valeur: n,
                votes: 0,
                maximum: 10.0,
            });
        }
        if let Some(i) = &e.image
            && let Some(a) = artwork_simple(i, TypeArtworkEtendu::Vignette, None)
        {
            m.artwork.push(a);
        }
        for r in &e.inclus.guestcast {
            if !r.person.name.trim().is_empty() {
                m.personnes
                    .push(personne(TypePersonne::Invite, &r.person, &r.character.name));
            }
        }
        for c in &e.inclus.guestcrew {
            if let Some(t) = type_equipe_tvmaze(&c.fonction)
                && !c.person.name.trim().is_empty()
            {
                m.personnes.push(personne(t, &c.person, &c.fonction));
            }
        }
        m
    }
}

// ---------------------------------------------------------------- traits

#[async_trait]
impl FournisseurSerie for TvMaze {
    fn identifiant(&self) -> &'static str {
        ID_FOURNISSEUR
    }

    /// Recherche ; le score est celui renvoyé par TVmaze (1.0 si trouvé par identifiant).
    async fn rechercher(&self, options: &OptionsRecherche) -> Resultat<Vec<ResultatRecherche>> {
        if let Some(e) = self.par_identifiant(options).await? {
            return Ok(vec![Self::resultat_depuis(&e, 1.0)]);
        }
        let q = options.requete.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let r: Vec<ResultatTvmaze> = self.get("search/shows", &[("q", Some(q))]).await?;
        let imdb = options.id_imdb();
        let mut v: Vec<ResultatRecherche> = r
            .iter()
            .filter(|x| x.show.id > 0)
            .map(|x| {
                let mut res = Self::resultat_depuis(&x.show, x.score);
                if !imdb.is_empty() && res.ids.get(cles_id::IMDB) == Some(&imdb) {
                    res.score = 1.0;
                }
                res
            })
            .collect();
        trier_resultats(&mut v);
        Ok(v)
    }

    async fn metadonnees_serie(&self, options: &OptionsRecherche) -> Resultat<MetadonneesSerie> {
        let id = self.resoudre_id(options).await?;
        let e = self.emission_complete(id).await?;
        let (groupes, _) = self.groupes_alternatifs(id).await;
        Ok(Self::convertir_emission(&e, &groupes))
    }

    async fn liste_episodes(
        &self,
        options: &OptionsRecherche,
    ) -> Resultat<Vec<MetadonneesEpisode>> {
        let id = self.resoudre_id(options).await?;
        let e = self.emission_complete(id).await?;
        let (_, numeros) = self.groupes_alternatifs(id).await;
        let mut tout = Vec::new();
        let mut rang_special = 0u32;
        for s in &e.inclus.seasons {
            if s.id == 0 {
                continue;
            }
            let eps: Vec<EpisodeTvmaze> = match self
                .get(
                    &format!("seasons/{}/episodes", s.id),
                    &[
                        ("embed[]", Some("guestcast")),
                        ("embed[]", Some("guestcrew")),
                    ],
                )
                .await
            {
                Ok(v) => v,
                Err(err) => {
                    tracing::warn!("saison {} indisponible : {err}", s.id);
                    continue;
                }
            };
            for ep in &eps {
                let mut m = Self::convertir_episode(ep, &mut rang_special);
                for (g, sn, en) in numeros.get(&ep.id).into_iter().flatten() {
                    m.definir_numero(g.clone(), *sn, *en);
                }
                tout.push(m);
            }
        }
        Ok(tout)
    }

    async fn metadonnees_episode(&self, options: &OptionsEpisode) -> Resultat<MetadonneesEpisode> {
        let liste = self.liste_episodes(&options.serie).await?;
        apparier_episode(&liste, options, ReglesAppariement::TVMAZE).cloned()
    }
}

#[async_trait]
impl FournisseurArtworkSerie for TvMaze {
    async fn artwork_serie(
        &self,
        options: &OptionsRecherche,
        type_filtre: Option<TypeArtworkEtendu>,
    ) -> Resultat<Vec<ArtworkEtendu>> {
        let id = self.resoudre_id(options).await?;
        let e = self.emission_complete(id).await?;
        Ok(Self::artwork_emission(&e)
            .into_iter()
            .filter(|a| type_filtre.is_none_or(|t| t == a.type_artwork))
            .collect())
    }

    async fn artwork_episode(&self, options: &OptionsEpisode) -> Resultat<Vec<ArtworkEtendu>> {
        Ok(self.metadonnees_episode(options).await?.artwork)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_simple() {
        assert_eq!(
            html_en_texte("<p>Un <b>homme</b> &amp; son chien.</p><p>Suite&nbsp;!</p>"),
            "Un homme & son chien.\nSuite !"
        );
        assert_eq!(html_en_texte(""), "");
    }

    #[test]
    fn statuts() {
        assert_eq!(statut_depuis_tvmaze("Running"), StatutDiffusion::EnCours);
        assert_eq!(statut_depuis_tvmaze("Ended"), StatutDiffusion::Terminee);
        assert_eq!(
            statut_depuis_tvmaze("To Be Determined"),
            StatutDiffusion::Inconnu
        );
    }

    #[test]
    fn noms_des_listes_alternatives() {
        let l = ListeAlternative {
            dvd_release: true,
            ..Default::default()
        };
        assert_eq!(groupe_alternatif(&l), Some(GroupeEpisodes::dvd()));
        let l = ListeAlternative {
            country_premiere: true,
            network: Some(ChaineAvecPays {
                name: "BBC One".into(),
                country: Some(Pays {
                    name: "United Kingdom".into(),
                    code: "GB".into(),
                }),
            }),
            ..Default::default()
        };
        assert_eq!(
            groupe_alternatif(&l).unwrap().nom,
            "Country Premiere - BBC One (GB)"
        );
        assert_eq!(groupe_alternatif(&ListeAlternative::default()), None);
    }

    #[test]
    fn episode_special_numerote_en_saison_zero() {
        let e = EpisodeTvmaze {
            id: 5,
            name: "Spécial".into(),
            season: 2,
            number: None,
            type_episode: "significant_special".into(),
            ..Default::default()
        };
        let mut rang = 0;
        let m = TvMaze::convertir_episode(&e, &mut rang);
        assert_eq!(m.numero_diffusion(), Some((0, 1)));
        assert_eq!(
            m.numero(&GroupeEpisodes::nouveau(TypeGroupeEpisodes::Affichage, "")),
            Some((2, 1))
        );
    }
}
