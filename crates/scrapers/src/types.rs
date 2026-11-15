//! Types propres au crate : options, résultats de recherche, métadonnées, images.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Clés d'identifiants usuelles.
pub mod cles_id {
    pub const TMDB: &str = "tmdb";
    pub const TMDB_COLLECTION: &str = "tmdbSet";
    pub const IMDB: &str = "imdb";
    pub const TVDB: &str = "tvdb";
}

/// Options d'une recherche ou d'un scrap.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OptionsRecherche {
    /// Texte recherché.
    pub requete: String,
    /// Année recherchée (<= 1900 ou `None` = ignorée).
    pub annee: Option<i32>,
    /// Identifiants connus par fournisseur (`tmdb`, `imdb`, `tmdbSet`...).
    pub ids: BTreeMap<String, String>,
    /// Langue de scrap (`fr`, `pt_BR`...).
    pub langue: String,
    /// Pays de la classification d'âge (ISO 3166-1 alpha-2).
    pub pays_classification: String,
    /// Pays de la date de sortie locale ("" = date mondiale).
    pub pays_sortie: String,
}

impl Default for OptionsRecherche {
    fn default() -> Self {
        Self {
            requete: String::new(),
            annee: None,
            ids: BTreeMap::new(),
            langue: "en".into(),
            pays_classification: "US".into(),
            pays_sortie: String::new(),
        }
    }
}

impl OptionsRecherche {
    /// Options avec une requête texte.
    pub fn avec_requete(requete: impl Into<String>) -> Self {
        Self {
            requete: requete.into(),
            ..Self::default()
        }
    }

    /// Ajoute un identifiant (builder).
    pub fn avec_id(mut self, fournisseur: &str, id: impl Into<String>) -> Self {
        self.ids.insert(fournisseur.to_string(), id.into());
        self
    }

    /// Identifiant TMDB (0 si absent ou illisible).
    pub fn id_tmdb(&self) -> u64 {
        self.ids
            .get(cles_id::TMDB)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    }

    /// Identifiant de collection TMDB (0 si absent).
    pub fn id_collection(&self) -> u64 {
        self.ids
            .get(cles_id::TMDB_COLLECTION)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    }

    /// Identifiant IMDb valide ("" sinon).
    pub fn id_imdb(&self) -> String {
        match self.ids.get(cles_id::IMDB) {
            Some(v) if crate::correspondance::est_id_imdb(v.trim()) => v.trim().to_string(),
            _ => String::new(),
        }
    }
}

/// Résultat d'une recherche.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ResultatRecherche {
    pub ids: BTreeMap<String, String>,
    pub fournisseur: String,
    pub titre: String,
    pub titre_original: String,
    pub langue_originale: String,
    pub resume: String,
    /// Année de sortie (0 = inconnue).
    pub annee: i32,
    /// Score de correspondance (peut être négatif après pénalités).
    pub score: f32,
    pub url_affiche: String,
}

/// Rôle d'une personne au générique.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TypePersonne {
    Acteur,
    Realisateur,
    Scenariste,
    Producteur,
    Monteur,
    Compositeur,
    Photographie,
    Invite,
    Autre,
}

/// Personne du générique.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Personne {
    pub type_personne: TypePersonne,
    pub nom: String,
    /// Personnage (acteurs) ou fonction.
    pub role: String,
    pub ids: BTreeMap<String, String>,
    pub url_photo: String,
    pub url_profil: String,
}

/// Note attribuée par une source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Evaluation {
    pub source: String,
    pub valeur: f32,
    pub votes: u32,
    pub maximum: f32,
}

/// Classification d'âge brute (pays + libellé tel que fourni).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Classification {
    pub pays: String,
    pub valeur: String,
}

/// Métadonnées d'un film.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MetadonneesFilm {
    pub fournisseur: String,
    pub ids: BTreeMap<String, String>,
    pub titre: String,
    pub titre_original: String,
    pub titre_anglais: String,
    pub langue_originale: String,
    pub annee: i32,
    /// Date de sortie `AAAA-MM-JJ`.
    pub date_sortie: Option<String>,
    pub resume: String,
    pub accroche: String,
    /// Durée en minutes (0 = inconnue).
    pub duree: u32,
    pub nom_collection: String,
    pub evaluations: Vec<Evaluation>,
    pub personnes: Vec<Personne>,
    /// Genres : identifiant interne (`ADVENTURE`...) ou nom brut si inconnu.
    pub genres: Vec<String>,
    pub classifications: Vec<Classification>,
    pub societes_production: Vec<String>,
    pub langues_parlees: Vec<String>,
    pub pays: Vec<String>,
    pub mots_cles: Vec<String>,
    pub artwork: Vec<Artwork>,
}

/// Type d'image.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TypeArtwork {
    Affiche,
    Fond,
    Vignette,
    LogoTransparent,
}

/// Une déclinaison de taille d'une image.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TailleImage {
    pub largeur: u32,
    pub hauteur: u32,
    pub url: String,
    /// Ordre de taille (puissance de 2, 16 = très grande... 1 = petite).
    pub ordre: u32,
}

/// Image (affiche, fond...).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Artwork {
    pub fournisseur: String,
    pub type_artwork: TypeArtwork,
    pub url_apercu: String,
    pub url_originale: String,
    /// Code ISO-2 ("" = sans texte).
    pub langue: String,
    pub votes: u32,
    pub tailles: Vec<TailleImage>,
}

/// Bande-annonce.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BandeAnnonce {
    pub nom: String,
    pub url: String,
    /// `youtube`, `vimeo`...
    pub hebergeur: String,
    /// Ex. `1080p`.
    pub qualite: String,
    /// Ex. `fr-FR`.
    pub langue: String,
    pub date: String,
}

/// Un film d'une collection.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PartieCollection {
    pub ids: BTreeMap<String, String>,
    pub titre: String,
    pub titre_original: String,
    pub resume: String,
    pub date_sortie: Option<String>,
    pub annee: i32,
    pub url_affiche: String,
    /// Métadonnées complètes si elles ont été récupérées.
    pub metadonnees: Option<MetadonneesFilm>,
}

/// Collection (saga).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CollectionFilm {
    pub ids: BTreeMap<String, String>,
    pub titre: String,
    pub titre_anglais: String,
    pub resume: String,
    pub url_affiche: String,
    pub url_fond: String,
    /// Films déjà sortis, ordre du fournisseur.
    pub parties: Vec<PartieCollection>,
}

// ---------------------------------------------------------------- séries

/// Clés d'identifiants propres aux séries.
pub mod cles_id_serie {
    pub const TVMAZE: &str = "tvmaze";
    pub const TVRAGE: &str = "tvrage";
    pub const WIKIDATA: &str = "wikidata";
}

/// Statut de diffusion d'une série.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum StatutDiffusion {
    #[default]
    Inconnu,
    EnCours,
    Terminee,
}

/// Type d'ordre de numérotation des épisodes.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TypeGroupeEpisodes {
    /// Ordre de diffusion (ordre par défaut).
    #[default]
    Diffusion,
    Dvd,
    Absolu,
    Alternatif,
    /// Ordre d'affichage (épisodes spéciaux placés dans leur saison).
    Affichage,
}

/// Groupe (ordre) d'épisodes : type + nom libre.
///
/// Deux groupes sont égaux s'ils ont le même type et, sauf pour l'ordre de
/// diffusion, le même nom.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupeEpisodes {
    pub type_groupe: TypeGroupeEpisodes,
    pub nom: String,
}

impl GroupeEpisodes {
    /// Groupe de type donné, nom vide.
    pub fn nouveau(type_groupe: TypeGroupeEpisodes, nom: impl Into<String>) -> Self {
        Self {
            type_groupe,
            nom: nom.into(),
        }
    }
    /// Ordre de diffusion.
    pub fn diffusion() -> Self {
        Self::nouveau(TypeGroupeEpisodes::Diffusion, "")
    }
    /// Ordre DVD (nom vide).
    pub fn dvd() -> Self {
        Self::nouveau(TypeGroupeEpisodes::Dvd, "")
    }
    /// Ordre absolu (nom vide).
    pub fn absolu() -> Self {
        Self::nouveau(TypeGroupeEpisodes::Absolu, "")
    }
}

impl PartialEq for GroupeEpisodes {
    fn eq(&self, autre: &Self) -> bool {
        self.type_groupe == autre.type_groupe
            && (self.type_groupe == TypeGroupeEpisodes::Diffusion || self.nom == autre.nom)
    }
}
impl Eq for GroupeEpisodes {}

/// Numéro (saison, épisode) d'un épisode dans un groupe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NumeroEpisode {
    pub groupe: GroupeEpisodes,
    pub saison: u32,
    pub episode: u32,
}

/// Type d'image étendu (séries, saisons, artwork Fanart.tv).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TypeArtworkEtendu {
    Affiche,
    Fond,
    Banniere,
    Vignette,
    LogoTransparent,
    ClearArt,
    Disque,
    /// Affiche sans texte.
    ArtCle,
    ArtPersonnage,
    Carre,
    AfficheSaison,
    FondSaison,
    BanniereSaison,
    VignetteSaison,
}

impl TypeArtworkEtendu {
    /// Type de base correspondant, s'il existe.
    pub fn en_type_base(self) -> Option<TypeArtwork> {
        Some(match self {
            Self::Affiche => TypeArtwork::Affiche,
            Self::Fond => TypeArtwork::Fond,
            Self::Vignette => TypeArtwork::Vignette,
            Self::LogoTransparent => TypeArtwork::LogoTransparent,
            _ => return None,
        })
    }
}

impl From<TypeArtwork> for TypeArtworkEtendu {
    fn from(t: TypeArtwork) -> Self {
        match t {
            TypeArtwork::Affiche => Self::Affiche,
            TypeArtwork::Fond => Self::Fond,
            TypeArtwork::Vignette => Self::Vignette,
            TypeArtwork::LogoTransparent => Self::LogoTransparent,
        }
    }
}

/// Image avec type étendu et numéro de saison éventuel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtworkEtendu {
    pub fournisseur: String,
    pub type_artwork: TypeArtworkEtendu,
    pub url_apercu: String,
    pub url_originale: String,
    /// Code ISO-2 ("" = sans texte).
    pub langue: String,
    pub votes: u32,
    /// Saison concernée (`None` = série entière ; 0 = spéciaux).
    pub saison: Option<u32>,
    pub tailles: Vec<TailleImage>,
}

impl ArtworkEtendu {
    /// Conversion depuis une image de base, avec type et saison imposés.
    pub fn depuis_artwork(
        a: Artwork,
        type_artwork: TypeArtworkEtendu,
        saison: Option<u32>,
    ) -> Self {
        Self {
            fournisseur: a.fournisseur,
            type_artwork,
            url_apercu: a.url_apercu,
            url_originale: a.url_originale,
            langue: a.langue,
            votes: a.votes,
            saison,
            tailles: a.tailles,
        }
    }

    /// Image de base si le type s'y prête (affiche, fond, vignette, logo).
    pub fn en_artwork(&self) -> Option<Artwork> {
        Some(Artwork {
            fournisseur: self.fournisseur.clone(),
            type_artwork: self.type_artwork.en_type_base()?,
            url_apercu: self.url_apercu.clone(),
            url_originale: self.url_originale.clone(),
            langue: self.langue.clone(),
            votes: self.votes,
            tailles: self.tailles.clone(),
        })
    }
}

impl From<Artwork> for ArtworkEtendu {
    fn from(a: Artwork) -> Self {
        let t = a.type_artwork.into();
        Self::depuis_artwork(a, t, None)
    }
}

/// Options d'un scrap d'épisode.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OptionsEpisode {
    /// Options de la série (identifiants de la série, langue, pays...).
    pub serie: OptionsRecherche,
    /// Identifiants propres à l'épisode (`tmdb`, `imdb`, `tvdb`, `tvmaze`).
    pub ids_episode: BTreeMap<String, String>,
    /// Groupe dans lequel `saison`/`episode` sont exprimés.
    pub groupe: GroupeEpisodes,
    pub saison: Option<u32>,
    pub episode: Option<u32>,
    /// Date de diffusion `AAAA-MM-JJ`.
    pub date_diffusion: Option<String>,
    /// Titre connu de l'épisode.
    pub titre: String,
}

impl OptionsEpisode {
    /// Options pour l'épisode (saison, épisode) en ordre de diffusion.
    pub fn diffusion(serie: OptionsRecherche, saison: u32, episode: u32) -> Self {
        Self {
            serie,
            saison: Some(saison),
            episode: Some(episode),
            ..Default::default()
        }
    }
}

/// Métadonnées d'un épisode.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MetadonneesEpisode {
    pub fournisseur: String,
    pub ids: BTreeMap<String, String>,
    pub titre: String,
    pub titre_original: String,
    pub titre_anglais: String,
    pub resume: String,
    /// Numéros dans les différents ordres (diffusion, DVD, absolu...).
    pub numeros: Vec<NumeroEpisode>,
    /// Date de diffusion `AAAA-MM-JJ`.
    pub date_diffusion: Option<String>,
    /// Durée en minutes (0 = inconnue).
    pub duree: u32,
    pub evaluations: Vec<Evaluation>,
    pub personnes: Vec<Personne>,
    pub artwork: Vec<ArtworkEtendu>,
}

impl MetadonneesEpisode {
    /// Numéro (saison, épisode) dans un groupe.
    pub fn numero(&self, groupe: &GroupeEpisodes) -> Option<(u32, u32)> {
        self.numeros
            .iter()
            .find(|n| n.groupe == *groupe)
            .map(|n| (n.saison, n.episode))
    }
    /// Numéro en ordre de diffusion.
    pub fn numero_diffusion(&self) -> Option<(u32, u32)> {
        self.numero(&GroupeEpisodes::diffusion())
    }
    /// Premier numéro DVD trouvé (quel que soit le nom du groupe).
    pub fn numero_dvd(&self) -> Option<(u32, u32)> {
        self.numero_de_type(TypeGroupeEpisodes::Dvd)
    }
    /// Premier numéro absolu trouvé (le numéro d'épisode est le rang absolu).
    pub fn numero_absolu(&self) -> Option<u32> {
        self.numero_de_type(TypeGroupeEpisodes::Absolu)
            .map(|(_, e)| e)
    }
    fn numero_de_type(&self, t: TypeGroupeEpisodes) -> Option<(u32, u32)> {
        self.numeros
            .iter()
            .find(|n| n.groupe.type_groupe == t)
            .map(|n| (n.saison, n.episode))
    }
    /// Ajoute ou remplace le numéro d'un groupe.
    pub fn definir_numero(&mut self, groupe: GroupeEpisodes, saison: u32, episode: u32) {
        self.numeros.retain(|n| n.groupe != groupe);
        self.numeros.push(NumeroEpisode {
            groupe,
            saison,
            episode,
        });
    }
}

/// Métadonnées d'une saison (dans un groupe d'épisodes donné).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MetadonneesSaison {
    pub groupe: GroupeEpisodes,
    pub numero: u32,
    pub ids: BTreeMap<String, String>,
    pub nom: String,
    pub resume: String,
    pub date_diffusion: Option<String>,
    pub nombre_episodes: u32,
    pub url_affiche: String,
}

/// Métadonnées d'une série.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct MetadonneesSerie {
    pub fournisseur: String,
    pub ids: BTreeMap<String, String>,
    pub titre: String,
    pub titre_original: String,
    pub titre_anglais: String,
    pub langue_originale: String,
    /// Année de première diffusion (0 = inconnue).
    pub annee: i32,
    /// Année de fin (seulement si la série est terminée ; 0 sinon).
    pub annee_fin: i32,
    /// Date de première diffusion `AAAA-MM-JJ`.
    pub date_premiere_diffusion: Option<String>,
    pub resume: String,
    pub accroche: String,
    /// Durée typique d'un épisode en minutes (0 = inconnue).
    pub duree: u32,
    pub statut: StatutDiffusion,
    pub evaluations: Vec<Evaluation>,
    pub personnes: Vec<Personne>,
    pub genres: Vec<String>,
    pub classifications: Vec<Classification>,
    /// Chaînes puis sociétés de production.
    pub societes_production: Vec<String>,
    pub pays: Vec<String>,
    pub mots_cles: Vec<String>,
    pub artwork: Vec<ArtworkEtendu>,
    /// Saisons de tous les groupes (filtrer sur `groupe`).
    pub saisons: Vec<MetadonneesSaison>,
    /// Groupes d'épisodes disponibles (toujours au moins l'ordre de diffusion).
    pub groupes_episodes: Vec<GroupeEpisodes>,
}

impl MetadonneesSerie {
    /// Saisons d'un groupe donné.
    pub fn saisons_du_groupe<'a>(
        &'a self,
        groupe: &'a GroupeEpisodes,
    ) -> impl Iterator<Item = &'a MetadonneesSaison> + 'a {
        self.saisons.iter().filter(move |s| s.groupe == *groupe)
    }
}
