//! Réglages de l'application : un fichier JSON unique `data/reglages.json`.
//!
//! Corrections apportées par rapport aux pièges connus :
//! - écriture **atomique** (fichier temporaire + `rename`) ;
//! - fichier illisible ou invalide **mis de côté** en
//!   `reglages.json.<horodatage>.corrompu` avant de repartir des valeurs par
//!   défaut (au lieu d'être écrasé silencieusement) ;
//! - l'état « déjà enregistré » n'est mis à jour **qu'après** une écriture
//!   réussie : un échec d'écriture sera retenté au prochain
//!   [`GestionnaireReglages::enregistrer_si_modifie`] ;
//! - validation TLS active par défaut (`ignorer_erreurs_ssl = false`) ;
//! - secrets (clés d'API, mot de passe du proxy) **hors du fichier** : stockés
//!   dans un [`CoffreSecrets`]. Un secret trouvé en clair dans le fichier
//!   (saisie manuelle) est migré vers le coffre puis effacé du fichier ;
//! - champs inconnus conservés et réécrits (aller-retour sans perte) ; valeurs
//!   d'énumération inconnues ramenées à une valeur par défaut.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::atomique::ecrire_atomiquement;
use crate::erreur::{ErreurStockage, Resultat};
use crate::secrets::CoffreSecrets;

/// Nom du fichier de réglages dans le dossier de données.
pub const NOM_FICHIER_REGLAGES: &str = "reglages.json";
/// Version du format des réglages produite par ce code.
pub const VERSION_REGLAGES: u32 = 1;

/// Noms des secrets dans le coffre.
pub mod noms_secrets {
    pub const CLE_API_TMDB: &str = "cle_api_tmdb";
    pub const CLE_API_HTTP: &str = "cle_api_http";
    pub const MOT_DE_PASSE_PROXY: &str = "mot_de_passe_proxy";
}

/// Désérialise une valeur ; en cas de valeur inconnue ou de type inattendu,
/// prend la valeur par défaut au lieu d'invalider tout le fichier.
fn tolerant<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let v = serde_json::Value::deserialize(d)?;
    Ok(T::deserialize(v).unwrap_or_else(|e| {
        tracing::warn!("valeur de réglage inconnue, défaut utilisé : {e}");
        T::default()
    }))
}

fn chaines(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

// ---------------------------------------------------------------------------
// Énumérations
// ---------------------------------------------------------------------------

/// Taille des images mises en cache.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TailleCacheImages {
    Petite,
    #[default]
    Grande,
    Originale,
}

/// Qualité de compression des images du cache.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualiteCacheImages {
    Equilibree,
    Haute,
    #[default]
    Maximale,
}

/// Taille d'image demandée aux fournisseurs d'artwork.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TailleImage {
    Petite,
    Moyenne,
    #[default]
    Grande,
    Originale,
}

/// Format des fichiers NFO.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatNfo {
    #[default]
    Kodi,
    Jellyfin,
    Emby,
    Plex,
}

/// Qualité préférée des bandes-annonces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualiteBandeAnnonce {
    Sd,
    #[default]
    Hd720,
    Hd1080,
    Uhd,
}

// ---------------------------------------------------------------------------
// Groupes
// ---------------------------------------------------------------------------

/// Réglages généraux.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReglagesGeneraux {
    /// Langue de l'interface (code ISO 639-1).
    pub langue: String,
    pub theme: String,
    pub taille_police: u32,
    // Réseau
    pub threads_telechargement: u32,
    /// Désactive la validation TLS. **Faux par défaut** (sécurité).
    pub ignorer_erreurs_ssl: bool,
    pub proxy_hote: String,
    pub proxy_port: u16,
    pub proxy_utilisateur: String,
    /// Secret : jamais écrit dans le fichier (voir le module).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub mot_de_passe_proxy: String,
    // Fournisseurs
    /// Clé d'API TMDB fournie par l'utilisateur (vide par défaut). Secret.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cle_api_tmdb: String,
    // Corbeille
    pub corbeille_activee: bool,
    pub vider_corbeille_a_la_sortie: bool,
    // Types de fichiers
    pub extensions_video: Vec<String>,
    pub extensions_audio: Vec<String>,
    pub extensions_sous_titres: Vec<String>,
    /// Expressions régulières des fichiers à nettoyer.
    pub motifs_nettoyage: Vec<String>,
    /// Articles déplacés en fin de titre pour le tri.
    pub prefixes_titre: Vec<String>,
    // Cache d'images
    pub cache_images: bool,
    #[serde(deserialize_with = "tolerant")]
    pub taille_cache_images: TailleCacheImages,
    #[serde(deserialize_with = "tolerant")]
    pub qualite_cache_images: QualiteCacheImages,
    // Outils
    /// Chemin de ffmpeg/ffprobe ; vide = recherche automatique.
    pub chemin_ffmpeg: String,
    pub chemin_yt_dlp: String,
    pub lecteur_media: String,
    // API HTTP locale
    pub serveur_http_actif: bool,
    pub port_http: u16,
    /// Clé de l'API HTTP locale, générée au premier lancement. Secret.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cle_api_http: String,
    // Mises à jour
    pub mise_a_jour_auto: bool,
    pub intervalle_mise_a_jour_jours: u32,
}

impl Default for ReglagesGeneraux {
    fn default() -> Self {
        Self {
            langue: "fr".into(),
            theme: "clair".into(),
            taille_police: 12,
            threads_telechargement: 2,
            ignorer_erreurs_ssl: false,
            proxy_hote: String::new(),
            proxy_port: 0,
            proxy_utilisateur: String::new(),
            mot_de_passe_proxy: String::new(),
            cle_api_tmdb: String::new(),
            corbeille_activee: true,
            vider_corbeille_a_la_sortie: false,
            extensions_video: chaines(&[
                ".3gp", ".asf", ".avi", ".bdmv", ".divx", ".dvr-ms", ".flv", ".ifo", ".img",
                ".iso", ".m2ts", ".m4v", ".mk3d", ".mkv", ".mov", ".mp4", ".mpeg", ".mpg", ".mts",
                ".ogm", ".ogv", ".rm", ".rmvb", ".strm", ".ts", ".vob", ".webm", ".wmv",
            ]),
            extensions_audio: chaines(&[
                ".aac", ".ac3", ".dts", ".flac", ".m4a", ".mka", ".mp3", ".ogg", ".opus", ".wav",
                ".wma",
            ]),
            extensions_sous_titres: chaines(&[
                ".ass", ".idx", ".smi", ".srt", ".ssa", ".sub", ".sup", ".vtt",
            ]),
            motifs_nettoyage: chaines(&[r"\.txt$", r"\.url$", r"\.html$", r"\.sfv$"]),
            prefixes_titre: chaines(&[
                "A", "An", "The", "Der", "Die", "Das", "Ein", "Eine", "Le", "La", "Les", "L'",
                "L´", "L`", "Un", "Une", "Des", "Du", "D'", "D´", "D`",
            ]),
            cache_images: true,
            taille_cache_images: TailleCacheImages::Grande,
            qualite_cache_images: QualiteCacheImages::Maximale,
            chemin_ffmpeg: String::new(),
            chemin_yt_dlp: String::new(),
            lecteur_media: String::new(),
            serveur_http_actif: false,
            port_http: 7878,
            cle_api_http: String::new(),
            mise_a_jour_auto: true,
            intervalle_mise_a_jour_jours: 1,
        }
    }
}

/// Réglages du module Films.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReglagesFilms {
    pub sources: Vec<PathBuf>,
    // Scan
    pub ignorer_dossiers_nomedia: bool,
    pub dossiers_ignores: Vec<String>,
    pub mots_exclus: Vec<String>,
    pub mise_a_jour_au_demarrage: bool,
    pub cache_images_a_l_import: bool,
    // Scraper
    pub scraper: String,
    pub langue_scraping: String,
    pub pays_certification: String,
    pub seuil_scraper: f64,
    pub repli_scraper: bool,
    pub ne_pas_ecraser: bool,
    pub recuperer_toutes_notes: bool,
    pub sources_notes: Vec<String>,
    // Artwork
    pub meilleure_image_auto: bool,
    #[serde(deserialize_with = "tolerant")]
    pub taille_affiche: TailleImage,
    #[serde(deserialize_with = "tolerant")]
    pub taille_fanart: TailleImage,
    pub langues_images: Vec<String>,
    pub fanarts_extra: bool,
    pub nombre_fanarts_extra: u32,
    pub images_acteurs: bool,
    // NFO
    #[serde(deserialize_with = "tolerant")]
    pub format_nfo: FormatNfo,
    pub nfo_propre: bool,
    pub nfo_date_ajout: bool,
    pub creer_synopsis: bool,
    pub nfo_infos_fichier: bool,
    pub nfo_urls_artwork: bool,
    // Bandes-annonces, sous-titres, analyse
    pub bandes_annonces_auto: bool,
    #[serde(deserialize_with = "tolerant")]
    pub qualite_bande_annonce: QualiteBandeAnnonce,
    pub langue_sous_titres: String,
    pub duree_depuis_analyse: bool,
    // Renommage
    pub renommer_apres_scraping: bool,
    // Sagas
    pub dossier_donnees_sagas: String,
    pub afficher_films_manquants_sagas: bool,
}

impl Default for ReglagesFilms {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            ignorer_dossiers_nomedia: true,
            dossiers_ignores: chaines(&["MAKEMKV"]),
            mots_exclus: Vec::new(),
            mise_a_jour_au_demarrage: false,
            cache_images_a_l_import: true,
            scraper: "tmdb".into(),
            langue_scraping: "fr".into(),
            pays_certification: "FR".into(),
            seuil_scraper: 0.75,
            repli_scraper: false,
            ne_pas_ecraser: false,
            recuperer_toutes_notes: true,
            sources_notes: chaines(&["imdb"]),
            meilleure_image_auto: true,
            taille_affiche: TailleImage::Grande,
            taille_fanart: TailleImage::Grande,
            langues_images: chaines(&["fr", "en"]),
            fanarts_extra: false,
            nombre_fanarts_extra: 5,
            images_acteurs: false,
            format_nfo: FormatNfo::Kodi,
            nfo_propre: false,
            nfo_date_ajout: true,
            creer_synopsis: true,
            nfo_infos_fichier: true,
            nfo_urls_artwork: true,
            bandes_annonces_auto: false,
            qualite_bande_annonce: QualiteBandeAnnonce::Hd720,
            langue_sous_titres: "fr".into(),
            duree_depuis_analyse: false,
            renommer_apres_scraping: false,
            dossier_donnees_sagas: String::new(),
            afficher_films_manquants_sagas: false,
        }
    }
}

/// Réglages du module Séries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReglagesSeries {
    pub sources: Vec<PathBuf>,
    pub ignorer_dossiers_nomedia: bool,
    pub dossiers_ignores: Vec<String>,
    pub mots_exclus: Vec<String>,
    pub mise_a_jour_au_demarrage: bool,
    pub cache_images_a_l_import: bool,
    pub afficher_episodes_manquants: bool,
    pub afficher_speciaux_manquants: bool,
    pub afficher_non_diffuses: bool,
    pub scraper: String,
    pub langue_scraping: String,
    pub pays_certification: String,
    pub recuperer_toutes_notes: bool,
    pub meilleure_image_auto: bool,
    #[serde(deserialize_with = "tolerant")]
    pub taille_affiche: TailleImage,
    #[serde(deserialize_with = "tolerant")]
    pub taille_fanart: TailleImage,
    #[serde(deserialize_with = "tolerant")]
    pub taille_vignette: TailleImage,
    pub repli_artwork_saison: bool,
    #[serde(deserialize_with = "tolerant")]
    pub format_nfo: FormatNfo,
    pub nfo_guide_episodes: bool,
    pub nfo_date_fin: bool,
    pub nfo_tous_acteurs: bool,
    pub bandes_annonces_auto: bool,
    #[serde(deserialize_with = "tolerant")]
    pub qualite_bande_annonce: QualiteBandeAnnonce,
    pub langue_sous_titres: String,
    /// Durée prise dans l'analyse des fichiers (vrai par défaut pour les séries).
    pub duree_depuis_analyse: bool,
    pub renommer_apres_scraping: bool,
}

impl Default for ReglagesSeries {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            ignorer_dossiers_nomedia: true,
            dossiers_ignores: chaines(&["MAKEMKV"]),
            mots_exclus: Vec::new(),
            mise_a_jour_au_demarrage: false,
            cache_images_a_l_import: true,
            afficher_episodes_manquants: false,
            afficher_speciaux_manquants: false,
            afficher_non_diffuses: false,
            scraper: "tvdb".into(),
            langue_scraping: "fr".into(),
            pays_certification: "FR".into(),
            recuperer_toutes_notes: false,
            meilleure_image_auto: true,
            taille_affiche: TailleImage::Grande,
            taille_fanart: TailleImage::Grande,
            taille_vignette: TailleImage::Moyenne,
            repli_artwork_saison: false,
            format_nfo: FormatNfo::Kodi,
            nfo_guide_episodes: true,
            nfo_date_fin: false,
            nfo_tous_acteurs: false,
            bandes_annonces_auto: false,
            qualite_bande_annonce: QualiteBandeAnnonce::Hd720,
            langue_sous_titres: "fr".into(),
            duree_depuis_analyse: true,
            renommer_apres_scraping: false,
        }
    }
}

/// Réglages du renommeur (gabarits).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReglagesRenommeur {
    pub gabarit_dossier_film: String,
    pub gabarit_fichier_film: String,
    pub gabarit_dossier_serie: String,
    pub gabarit_dossier_saison: String,
    pub gabarit_fichier_episode: String,
    /// Remplace les espaces par [`caractere_espace`](Self::caractere_espace).
    pub remplacer_espaces: bool,
    pub caractere_espace: String,
    /// Translittère en ASCII.
    pub ascii_seulement: bool,
    /// Caractère de remplacement des caractères interdits (« : », « / »…).
    pub caractere_remplacement: String,
    /// Supprime les dossiers vides laissés après un déplacement.
    pub supprimer_dossiers_vides: bool,
}

impl Default for ReglagesRenommeur {
    fn default() -> Self {
        Self {
            gabarit_dossier_film: "${title}${ - ,edition,} (${year})".into(),
            gabarit_fichier_film: "${title}${ - ,edition,} (${year}) ${videoFormat} ${audioCodec}"
                .into(),
            gabarit_dossier_serie: "${showTitle} (${showYear})".into(),
            gabarit_dossier_saison: "Season ${seasonNr}".into(),
            gabarit_fichier_episode: "${showTitle} - S${seasonNr2}E${episodeNr2} - ${title}".into(),
            remplacer_espaces: false,
            caractere_espace: "_".into(),
            ascii_seulement: false,
            caractere_remplacement: String::new(),
            supprimer_dossiers_vides: true,
        }
    }
}

/// Ensemble des réglages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reglages {
    /// Version du format (migrations des réglages).
    pub version: u32,
    pub general: ReglagesGeneraux,
    pub films: ReglagesFilms,
    pub series: ReglagesSeries,
    pub renommeur: ReglagesRenommeur,
    /// Champs inconnus (version plus récente, saisie manuelle) : conservés et
    /// réécrits tels quels.
    #[serde(flatten)]
    pub autres: BTreeMap<String, serde_json::Value>,
}

impl Default for Reglages {
    fn default() -> Self {
        Self {
            version: VERSION_REGLAGES,
            general: ReglagesGeneraux::default(),
            films: ReglagesFilms::default(),
            series: ReglagesSeries::default(),
            renommeur: ReglagesRenommeur::default(),
            autres: BTreeMap::new(),
        }
    }
}

impl Reglages {
    /// Références mutables vers les champs secrets, avec leur nom de coffre.
    fn secrets_mut(&mut self) -> [(&'static str, &mut String); 3] {
        let g = &mut self.general;
        [
            (noms_secrets::CLE_API_TMDB, &mut g.cle_api_tmdb),
            (noms_secrets::CLE_API_HTTP, &mut g.cle_api_http),
            (noms_secrets::MOT_DE_PASSE_PROXY, &mut g.mot_de_passe_proxy),
        ]
    }

    /// Copie sans secrets (forme écrite sur disque, diagnostics).
    pub fn sans_secrets(&self) -> Reglages {
        let mut r = self.clone();
        for (_, v) in r.secrets_mut() {
            v.clear();
        }
        r
    }

    /// Migrations des réglages entre versions du format.
    fn migrer(&mut self) {
        // Aucun palier pour l'instant (version 1). Ajouter ici les paliers
        // `if self.version < N { … }` puis mettre à jour la version.
        self.version = self.version.max(VERSION_REGLAGES);
    }
}

// ---------------------------------------------------------------------------
// Gestionnaire
// ---------------------------------------------------------------------------

/// Issue du chargement des réglages.
#[derive(Debug)]
pub struct ChargementReglages {
    pub reglages: Reglages,
    /// Vrai si aucun fichier valide n'existait (premier lancement, assistant).
    pub nouveau: bool,
    /// Fichier invalide mis de côté, le cas échéant (à signaler à l'utilisateur).
    pub fichier_corrompu: Option<PathBuf>,
}

/// Lecture et écriture des réglages.
pub struct GestionnaireReglages {
    chemin: PathBuf,
    coffre: Box<dyn CoffreSecrets>,
    /// Dernière version écrite avec succès (forme sur disque, sans secrets).
    dernier_ecrit: Option<Reglages>,
    /// Derniers secrets écrits avec succès.
    derniers_secrets: Option<Vec<String>>,
}

impl std::fmt::Debug for GestionnaireReglages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GestionnaireReglages")
            .field("chemin", &self.chemin)
            .finish_non_exhaustive()
    }
}

impl GestionnaireReglages {
    pub fn new(chemin: impl Into<PathBuf>, coffre: Box<dyn CoffreSecrets>) -> Self {
        Self {
            chemin: chemin.into(),
            coffre,
            dernier_ecrit: None,
            derniers_secrets: None,
        }
    }

    pub fn chemin(&self) -> &Path {
        &self.chemin
    }

    /// Accès au coffre (autres secrets : jetons de services…).
    pub fn coffre(&self) -> &dyn CoffreSecrets {
        self.coffre.as_ref()
    }

    /// Charge les réglages. Fichier absent : valeurs par défaut écrites.
    /// Fichier invalide : renommé en `.corrompu`, valeurs par défaut écrites.
    /// Les secrets sont lus dans le coffre ; la clé de l'API locale est
    /// générée si absente.
    pub fn charger(&mut self) -> Resultat<ChargementReglages> {
        let mut nouveau = false;
        let mut fichier_corrompu = None;
        let mut a_reecrire = false;

        let mut reglages = match std::fs::read(&self.chemin) {
            Ok(octets) => match serde_json::from_slice::<Reglages>(&octets) {
                Ok(r) => r,
                Err(e) => {
                    let mis_de_cote = self.mettre_de_cote()?;
                    tracing::warn!(
                        "réglages invalides ({e}) : copie conservée dans {mis_de_cote:?}, \
                         valeurs par défaut utilisées"
                    );
                    fichier_corrompu = Some(mis_de_cote);
                    nouveau = true;
                    Reglages::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                nouveau = true;
                Reglages::default()
            }
            Err(e) => return Err(ErreurStockage::io(&self.chemin, e)),
        };
        if nouveau {
            a_reecrire = true;
        }

        let version_lue = reglages.version;
        reglages.migrer();
        if reglages.version != version_lue {
            a_reecrire = true;
        }

        // Secrets : valeur en clair dans le fichier => migration vers le coffre.
        for (nom, valeur) in reglages.secrets_mut() {
            if !valeur.is_empty() {
                self.coffre.ecrire(nom, valeur)?;
                a_reecrire = true;
                tracing::info!("secret « {nom} » déplacé du fichier de réglages vers le coffre");
            } else if let Some(v) = self.coffre.lire(nom)? {
                *valeur = v;
            }
        }
        let secrets_lus = secrets_de(&mut reglages.clone());
        if reglages.general.cle_api_http.is_empty() {
            reglages.general.cle_api_http = uuid::Uuid::new_v4().to_string();
        }

        if a_reecrire {
            self.enregistrer(&reglages)?;
        } else {
            self.dernier_ecrit = Some(reglages.sans_secrets());
            self.derniers_secrets = Some(secrets_lus);
            // Une clé d'API tout juste générée doit être conservée.
            self.enregistrer_secrets(&reglages)?;
        }

        Ok(ChargementReglages {
            reglages,
            nouveau,
            fichier_corrompu,
        })
    }

    /// Écrit les réglages (atomique) et les secrets (coffre).
    pub fn enregistrer(&mut self, reglages: &Reglages) -> Resultat<()> {
        self.enregistrer_secrets(reglages)?;
        let sur_disque = reglages.sans_secrets();
        let json = serde_json::to_vec_pretty(&sur_disque)?;
        ecrire_atomiquement(&self.chemin, &json, false)?;
        // Mis à jour uniquement après succès : un échec sera retenté.
        self.dernier_ecrit = Some(sur_disque);
        Ok(())
    }

    /// N'écrit que si les réglages diffèrent de la dernière écriture réussie.
    /// Renvoie `true` si une écriture a eu lieu.
    pub fn enregistrer_si_modifie(&mut self, reglages: &Reglages) -> Resultat<bool> {
        let fichier_identique = self.dernier_ecrit.as_ref() == Some(&reglages.sans_secrets());
        let secrets_identiques =
            self.derniers_secrets.as_ref() == Some(&secrets_de(&mut reglages.clone()));
        if fichier_identique && secrets_identiques {
            return Ok(false);
        }
        self.enregistrer(reglages)?;
        Ok(true)
    }

    fn enregistrer_secrets(&mut self, reglages: &Reglages) -> Resultat<()> {
        let mut copie = reglages.clone();
        let valeurs = secrets_de(&mut copie);
        if self.derniers_secrets.as_ref() == Some(&valeurs) {
            return Ok(());
        }
        for (nom, valeur) in copie.secrets_mut() {
            if valeur.is_empty() {
                self.coffre.supprimer(nom)?;
            } else {
                self.coffre.ecrire(nom, valeur)?;
            }
        }
        self.derniers_secrets = Some(valeurs);
        Ok(())
    }

    /// Renomme le fichier courant en `<nom>.<horodatage>.corrompu`.
    fn mettre_de_cote(&self) -> Resultat<PathBuf> {
        let mut nom = self.chemin.file_name().unwrap_or_default().to_os_string();
        nom.push(format!(
            ".{}.corrompu",
            chrono::Local::now().format("%Y%m%d-%H%M%S%.3f")
        ));
        let cible = self.chemin.with_file_name(nom);
        std::fs::rename(&self.chemin, &cible).map_err(|e| ErreurStockage::io(&self.chemin, e))?;
        Ok(cible)
    }
}

fn secrets_de(r: &mut Reglages) -> Vec<String> {
    r.secrets_mut()
        .into_iter()
        .map(|(_, v)| v.clone())
        .collect()
}
