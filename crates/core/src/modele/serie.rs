//! Types de séries partagés avec le cœur « séries » (structure seulement).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::commun::{BandeAnnonce, Edition, Identifiants, Note, Personne, SourceMedia};
use super::fichier::{FichierMedia, TypeFichier};

/// Type d'ordre d'épisodes.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum TypeGroupe {
    /// Ordre de diffusion (défaut).
    #[default]
    Diffusion,
    Dvd,
    Absolu,
    Alternatif,
    /// Numéros d'affichage (`displayseason` / `displayepisode`).
    Affichage,
}

/// Ordre d'épisodes : type + nom (le nom est ignoré pour l'ordre de diffusion).
#[derive(Debug, Clone, Default, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GroupeEpisodes {
    pub type_groupe: TypeGroupe,
    pub nom: String,
}

impl PartialEq for GroupeEpisodes {
    fn eq(&self, autre: &Self) -> bool {
        if self.type_groupe != autre.type_groupe {
            return false;
        }
        self.type_groupe == TypeGroupe::Diffusion || self.nom == autre.nom
    }
}

impl std::hash::Hash for GroupeEpisodes {
    // Cohérent avec `PartialEq` : le nom n'entre pas en compte pour la diffusion.
    fn hash<H: std::hash::Hasher>(&self, etat: &mut H) {
        self.type_groupe.hash(etat);
        if self.type_groupe != TypeGroupe::Diffusion {
            self.nom.hash(etat);
        }
    }
}

impl GroupeEpisodes {
    pub fn diffusion() -> Self {
        Self {
            type_groupe: TypeGroupe::Diffusion,
            nom: String::new(),
        }
    }
    pub fn dvd() -> Self {
        Self {
            type_groupe: TypeGroupe::Dvd,
            nom: String::new(),
        }
    }
    pub fn absolu() -> Self {
        Self {
            type_groupe: TypeGroupe::Absolu,
            nom: String::new(),
        }
    }
    pub fn affichage() -> Self {
        Self {
            type_groupe: TypeGroupe::Affichage,
            nom: String::new(),
        }
    }
}

/// Numéro d'épisode dans un groupe donné. Saison 0 = « Specials »,
/// saison -1 = « toutes saisons », épisode -1 = « non catégorisé ».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumeroEpisode {
    pub groupe: GroupeEpisodes,
    pub saison: i32,
    pub episode: i32,
}

impl NumeroEpisode {
    /// Valide si saison et épisode sont positifs ou nuls.
    pub fn est_valide(&self) -> bool {
        self.saison > -1 && self.episode > -1
    }
}

/// État de diffusion d'une série.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatutDiffusion {
    #[default]
    Inconnu,
    EnCours,
    Terminee,
    Annulee,
}

/// Catégorie d'un épisode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeEpisode {
    NonCategorise,
    Special,
    Normal,
    FictifSpecial,
    FictifNormal,
    FictifNonDiffuse,
}

/// Style d'écriture des épisodes multiples.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleMultiEpisodes {
    #[default]
    Repeter,
    Plage,
}

/// Série TV.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Serie {
    pub id: String,
    pub verrouille: bool,
    pub source_donnees: PathBuf,
    /// Dossier de la série.
    pub chemin: PathBuf,
    pub ids: Identifiants,
    pub titre: String,
    pub titre_original: String,
    pub titre_anglais: String,
    pub titre_tri: String,
    pub slogan: String,
    pub resume: String,
    pub annee: u32,
    pub annee_fin: u32,
    /// Première diffusion `aaaa-mm-jj`.
    pub premiere_diffusion: Option<String>,
    pub statut: StatutDiffusion,
    pub duree_minutes: u32,
    pub certification: String,
    pub pays: String,
    pub societe_production: String,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub acteurs: Vec<Personne>,
    pub equipe: Vec<Personne>,
    pub notes: BTreeMap<String, Note>,
    pub top250: u32,
    /// Ordre d'épisodes actif.
    pub groupe_actif: GroupeEpisodes,
    /// Ordres proposés par le scraper.
    pub groupes: Vec<GroupeEpisodes>,
    pub saisons: Vec<Saison>,
    pub episodes: Vec<Episode>,
    /// Épisodes scrapés sans fichier.
    pub episodes_fictifs: Vec<Episode>,
    pub fichiers: Vec<FichierMedia>,
    pub urls_artwork: BTreeMap<TypeFichier, String>,
    pub note_utilisateur: String,
    pub date_ajout: i64,
    pub derniere_lecture: Option<i64>,
    pub bandes_annonces: Vec<BandeAnnonce>,
}

/// Saison d'une série (dérivée des épisodes du groupe actif).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Saison {
    pub numero: i32,
    pub titre: String,
    pub resume: String,
    pub premiere_diffusion: Option<String>,
    /// Artworks de la saison (poster, fanart, bannière, vignette).
    pub fichiers: Vec<FichierMedia>,
}

/// Épisode d'une série. Un fichier multi-épisodes donne plusieurs épisodes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Episode {
    pub id: String,
    pub id_serie: String,
    /// Un numéro par groupe connu.
    pub numeros: Vec<NumeroEpisode>,
    pub titre: String,
    pub titre_original: String,
    pub titre_anglais: String,
    pub resume: String,
    pub premiere_diffusion: Option<String>,
    pub duree_minutes: u32,
    pub notes: BTreeMap<String, Note>,
    pub ids: Identifiants,
    pub tags: Vec<String>,
    pub invites: Vec<Personne>,
    pub equipe: Vec<Personne>,
    pub vu: bool,
    pub nombre_lectures: u32,
    pub derniere_lecture: Option<i64>,
    pub date_ajout: i64,
    pub source_media: SourceMedia,
    pub edition: Edition,
    pub est_disque: bool,
    /// Plusieurs épisodes dans le même fichier.
    pub multi_episodes: bool,
    pub empile: bool,
    pub nom_fichier_origine: String,
    pub note_utilisateur: String,
    pub fichiers: Vec<FichierMedia>,
    /// Épisode fictif (scrapé, sans fichier).
    pub fictif: bool,
}

impl Episode {
    /// Fictif ou sans aucun fichier média.
    pub fn est_fictif(&self) -> bool {
        self.fictif || self.fichiers.is_empty()
    }

    /// Numéro dans un groupe donné. Pour l'ordre de diffusion, tout numéro de
    /// diffusion convient si le groupe exact est absent.
    pub fn numero_dans(&self, groupe: &GroupeEpisodes) -> Option<&NumeroEpisode> {
        self.numeros.iter().find(|n| &n.groupe == groupe)
    }

    /// Saison dans le groupe actif de la série (`-1` si inconnue).
    pub fn saison(&self, groupe_actif: &GroupeEpisodes) -> i32 {
        self.numero_dans(groupe_actif).map_or(-1, |n| n.saison)
    }

    /// Épisode dans le groupe actif de la série (`-1` si inconnu).
    pub fn episode(&self, groupe_actif: &GroupeEpisodes) -> i32 {
        self.numero_dans(groupe_actif).map_or(-1, |n| n.episode)
    }

    /// Catégorie de l'épisode dans le groupe actif.
    pub fn type_episode(
        &self,
        groupe_actif: &GroupeEpisodes,
        date_courante: Option<&str>,
    ) -> TypeEpisode {
        let (s, e) = (self.saison(groupe_actif), self.episode(groupe_actif));
        if s < 0 || e < 0 {
            return TypeEpisode::NonCategorise;
        }
        if !self.est_fictif() {
            return if s == 0 {
                TypeEpisode::Special
            } else {
                TypeEpisode::Normal
            };
        }
        if s == 0 {
            return TypeEpisode::FictifSpecial;
        }
        match (&self.premiere_diffusion, date_courante) {
            (Some(d), Some(now)) if d.as_str() <= now => TypeEpisode::FictifNormal,
            _ => TypeEpisode::FictifNonDiffuse,
        }
    }
}
