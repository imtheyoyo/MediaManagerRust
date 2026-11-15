//! Frontière d'intégration : tout ce que l'interface attend des services de l'application
//! (bibliothèque, images, recherche, tâches, renommage, réglages, coffre à secrets).
//!
//! Deux implémentations :
//! - [`SourceApplication`](crate::source_application::SourceApplication) : services réels
//!   (`mediamanager_app::Application`) ;
//! - [`SourceFactice`] : démonstration et tests visuels (données de `donnees.rs`, images
//!   générées localement, recherche et tâches simulées).

use crate::dialogues::appliquer_resultat;
use crate::donnees::films_factices;
use crate::etat::Message;
use crate::modele::{Film, normaliser};
use crate::renommage::{self, ApercuFilm};
use crate::taches::{IssueTache, Repaint};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Nom du secret de la clé d'API TMDB transmis par l'interface.
pub const SECRET_CLE_TMDB: &str = "tmdb.cle_api";

/// Types d'illustration affichés par le panneau de détails.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GenreImage {
    Affiche,
    Fanart,
    Banniere,
    Miniature,
}

impl GenreImage {
    pub const TOUS: [GenreImage; 4] = [
        GenreImage::Affiche,
        GenreImage::Fanart,
        GenreImage::Banniere,
        GenreImage::Miniature,
    ];

    pub fn cle(self) -> &'static str {
        match self {
            GenreImage::Affiche => "gui.artwork.poster",
            GenreImage::Fanart => "gui.artwork.fanart",
            GenreImage::Banniere => "gui.artwork.banner",
            GenreImage::Miniature => "gui.artwork.thumb",
        }
    }

    /// Rang dans [`GenreImage::TOUS`].
    pub fn rang(self) -> usize {
        self as usize
    }

    fn suffixe(self) -> &'static str {
        match self {
            GenreImage::Affiche => "affiche",
            GenreImage::Fanart => "fanart",
            GenreImage::Banniere => "banniere",
            GenreImage::Miniature => "miniature",
        }
    }

    /// Dimensions des images de démonstration.
    fn dimensions_demo(self) -> (u32, u32) {
        match self {
            GenreImage::Affiche => (200, 300),
            GenreImage::Fanart => (480, 270),
            GenreImage::Banniere => (500, 92),
            GenreImage::Miniature => (320, 180),
        }
    }
}

/// Paramètres d'une recherche d'extracteur (dialogue de choix d'un résultat).
#[derive(Clone, Debug, PartialEq)]
pub struct RequeteRecherche {
    /// Film pour lequel on cherche.
    pub film: u64,
    pub extracteur: String,
    pub langue: String,
    pub texte: String,
    pub annee: Option<u32>,
}

/// Un résultat de recherche tel qu'affiché par le dialogue.
#[derive(Clone, Debug, PartialEq)]
pub struct ResultatRecherche {
    pub id: String,
    pub titre: String,
    pub titre_original: String,
    pub annee: u32,
    /// Score de similarité dans [0, 1].
    pub score: f32,
    pub resume: String,
    pub affiche: Option<PathBuf>,
}

/// Canal de retour vers l'interface : messages appliqués par l'état à la prochaine image.
#[derive(Clone)]
pub struct Retour {
    pub tx: Sender<Message>,
    pub repaint: Repaint,
}

impl Retour {
    /// Envoie un message et réveille l'interface ; `false` si l'interface a disparu.
    pub fn envoyer(&self, m: Message) -> bool {
        let ok = self.tx.send(m).is_ok();
        (self.repaint)();
        ok
    }
}

/// Traitements longs demandés par l'interface (exécutés en tâches par la source).
#[derive(Clone, Debug, PartialEq)]
pub enum Travail {
    /// Toutes les sources : films puis séries.
    MettreAJourSources,
    /// Sources de séries uniquement.
    MettreAJourSeries,
    /// Récupération automatique (meilleur résultat) ; `artwork` = images comprises.
    RecupererAuto {
        ids: Vec<u64>,
        artwork: bool,
    },
    /// Résultat choisi dans le dialogue de recherche.
    AppliquerResultat {
        film: u64,
        resultat: ResultatRecherche,
        ne_pas_remplacer: bool,
    },
    EcrireNfo {
        ids: Vec<u64>,
    },
    Renommer {
        ids: Vec<u64>,
    },
}

impl Travail {
    /// Clé de traduction du nom de la tâche.
    pub fn cle_nom(&self) -> &'static str {
        match self {
            Travail::MettreAJourSources => "gui.task.update",
            Travail::MettreAJourSeries => "gui.task.updatetv",
            Travail::RecupererAuto { .. } | Travail::AppliquerResultat { .. } => "gui.task.scrape",
            Travail::EcrireNfo { .. } => "gui.task.nfo",
            Travail::Renommer { .. } => "gui.task.rename",
        }
    }
}

/// Réglages des services modifiables depuis l'interface (miroir d'une partie du fichier de
/// réglages du stockage ; les secrets restent dans le coffre).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReglagesServices {
    pub sources_films: Vec<String>,
    pub sources_series: Vec<String>,
    /// Dossiers ignorés par le scan des films.
    pub dossiers_ignores: Vec<String>,
    pub langue_scraping: String,
    pub gabarit_dossier_film: String,
    pub gabarit_fichier_film: String,
    pub remplacer_espaces: bool,
    pub caractere_espace: String,
    pub ascii_seulement: bool,
    pub caractere_remplacement: String,
    pub supprimer_dossiers_vides: bool,
    pub renommer_apres_scraping: bool,
    pub proxy_hote: String,
    pub proxy_port: String,
    pub proxy_utilisateur: String,
    /// Secret : confié au coffre à l'enregistrement, jamais écrit dans `ui.json`.
    pub proxy_mot_de_passe: String,
    pub ignorer_erreurs_ssl: bool,
    pub threads_telechargement: u32,
    pub corbeille: bool,
    pub cache_images: bool,
}

/// Épisode affiché dans l'arbre des séries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EpisodeAffichage {
    pub episode: i32,
    pub titre: String,
    pub fichier: String,
    pub vu: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaisonAffichage {
    pub numero: i32,
    pub titre: String,
    pub episodes: Vec<EpisodeAffichage>,
}

/// Série affichée (arbre série → saisons → épisodes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SerieAffichage {
    pub id: String,
    pub titre: String,
    pub annee: u32,
    pub chemin: String,
    pub saisons: Vec<SaisonAffichage>,
}

impl SerieAffichage {
    pub fn nombre_episodes(&self) -> usize {
        self.saisons.iter().map(|s| s.episodes.len()).sum()
    }
}

/// Saga (collection de films) affichée par le module Collections.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SagaAffichage {
    pub id: String,
    pub titre: String,
    pub titre_original: String,
    pub resume: String,
    /// Identifiants affichés des films de la saga (ceux absents de la bibliothèque sont ignorés).
    pub films: Vec<u64>,
    /// Illustrations locales existantes de la saga.
    pub affiche: Option<PathBuf>,
    pub fanart: Option<PathBuf>,
    pub banniere: Option<PathBuf>,
}

/// Ce que l'interface attend des services. Les méthodes bloquantes (`rechercher`) sont
/// appelées hors du thread de l'interface ; les autres sont courtes.
pub trait SourceDonnees: Send + Sync {
    /// Instantané de la bibliothèque de films.
    fn films(&self) -> Vec<Film>;
    /// Instantané des séries.
    fn series(&self) -> Vec<SerieAffichage>;
    /// Instantané des sagas (par défaut : aucune).
    fn sagas(&self) -> Vec<SagaAffichage> {
        Vec::new()
    }
    /// Chemin local d'une illustration du film, s'il existe.
    fn chemin_image(&self, film: &Film, genre: GenreImage) -> Option<PathBuf>;
    /// Recherche auprès d'un extracteur (bloquant).
    fn rechercher(&self, requete: &RequeteRecherche) -> Result<Vec<ResultatRecherche>, String>;
    /// Extracteurs proposés dans le dialogue de recherche.
    fn extracteurs(&self) -> Vec<String> {
        vec!["TMDB".into()]
    }
    /// Écrit des films modifiés dans l'interface (base, puis NFO en tâche).
    fn enregistrer_films(&self, films: &[Film]) -> Result<(), String>;
    /// Retire des films de la base (et supprime leurs fichiers si demandé).
    fn retirer_films(&self, ids: &[u64], supprimer_fichiers: bool) -> Result<(), String>;
    /// Transmet un secret (clé d'API...) au coffre à secrets. Jamais écrit dans `ui.json`.
    fn transmettre_secret(&self, nom: &str, valeur: &str) -> Result<(), String>;
    /// Réglages courants des services.
    fn reglages_services(&self) -> ReglagesServices;
    /// Enregistre les réglages des services.
    fn enregistrer_reglages_services(&self, r: &ReglagesServices) -> Result<(), String>;
    /// Plan de renommage d'une sélection (rien n'est écrit).
    fn apercu_renommage(&self, ids: &[u64]) -> Result<Vec<ApercuFilm>, String>;
    /// Lance un traitement long ; son cycle de vie arrive par `retour` (ou par le canal
    /// installé par [`brancher`](Self::brancher)).
    fn lancer(&self, travail: Travail, retour: &Retour) -> Result<(), String>;
    /// Demande l'annulation d'une tâche.
    fn annuler_tache(&self, id: u64);
    /// Installe le canal de retour des événements spontanés des services.
    fn brancher(&self, _retour: Retour) {}
}

/// Similarité de deux titres : coefficient de Dice sur les bigrammes de caractères normalisés.
pub fn similarite(a: &str, b: &str) -> f32 {
    let nettoyer = |s: &str| -> Vec<char> {
        normaliser(s)
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect()
    };
    let (a, b) = (nettoyer(a), nettoyer(b));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    let bigrammes =
        |v: &[char]| -> Vec<(char, char)> { v.windows(2).map(|w| (w[0], w[1])).collect() };
    let (ba, mut bb) = (bigrammes(&a), bigrammes(&b));
    if ba.is_empty() || bb.is_empty() {
        return 0.0;
    }
    let total = (ba.len() + bb.len()) as f32;
    let mut communs = 0usize;
    for g in &ba {
        if let Some(p) = bb.iter().position(|x| x == g) {
            bb.swap_remove(p);
            communs += 1;
        }
    }
    2.0 * communs as f32 / total
}

/// Score d'un résultat : similarité du titre (meilleure entre titre et titre original), ajustée par l'année.
pub fn score_resultat(requete: &RequeteRecherche, titre: &str, original: &str, annee: u32) -> f32 {
    let base = similarite(&requete.texte, titre).max(similarite(&requete.texte, original));
    let ajuste = match requete.annee {
        Some(a) if a == annee => base + 0.1,
        Some(a) if a.abs_diff(annee) > 1 => base - 0.1,
        _ => base,
    };
    ajuste.clamp(0.0, 1.0)
}

/// Titre « nettoyé » d'un nom de dossier/fichier : séparateurs remplacés, année et balises retirées.
pub fn titre_nettoye(nom: &str) -> (String, Option<u32>) {
    let sans_ext = match nom.rsplit_once('.') {
        Some((base, ext)) if ext.len() <= 4 && ext.chars().all(|c| c.is_ascii_alphanumeric()) => {
            base
        }
        _ => nom,
    };
    let jetons: Vec<&str> = sans_ext
        .split(['.', '_', ' ', '(', ')', '[', ']'])
        .filter(|j| !j.is_empty())
        .collect();
    // Les balises techniques terminent le titre.
    let fin = jetons
        .iter()
        .position(|j| {
            matches!(
                j.to_lowercase().as_str(),
                "1080p" | "720p" | "2160p" | "480p" | "bluray" | "x264" | "x265" | "hevc" | "4k"
            )
        })
        .unwrap_or(jetons.len());
    let est_annee =
        |j: &str| j.len() == 4 && j.parse::<u32>().is_ok_and(|a| (1880..=2100).contains(&a));
    // La DERNIÈRE année (hors premier mot) est celle du film : « Blade Runner 2049 2017 ».
    match (1..fin).rev().find(|&n| est_annee(jetons[n])) {
        Some(n) => (jetons[..n].join(" "), jetons[n].parse().ok()),
        None => (jetons[..fin].join(" "), None),
    }
}

/// Source factice : films de démonstration, images générées dans un dossier temporaire,
/// recherche et tâches simulées (les effets sont appliqués à la fin de la tâche).
pub struct SourceFactice {
    dossier_images: PathBuf,
    films: Arc<Mutex<Vec<Film>>>,
    services: Arc<Mutex<ReglagesServices>>,
    secrets: Mutex<HashMap<String, String>>,
    annulations: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>,
    prochain_id: AtomicU64,
    /// Latence simulée de la recherche.
    pub latence: Duration,
    /// Durée d'une unité de travail des tâches simulées.
    pub pas: Duration,
}

impl Default for SourceFactice {
    fn default() -> Self {
        Self::new(std::env::temp_dir().join("mediamanagerrust-demo"))
    }
}

/// Réglages de démonstration.
pub fn reglages_demo() -> ReglagesServices {
    ReglagesServices {
        sources_films: vec!["D:\\Films".into(), "E:\\Films 4K".into()],
        sources_series: vec!["D:\\Séries".into()],
        dossiers_ignores: vec!["@eaDir".into(), "$RECYCLE.BIN".into()],
        langue_scraping: "fr".into(),
        gabarit_dossier_film: "${title} (${year})".into(),
        gabarit_fichier_film: "${title} (${year})".into(),
        caractere_espace: "_".into(),
        supprimer_dossiers_vides: true,
        threads_telechargement: 2,
        corbeille: true,
        cache_images: true,
        ..ReglagesServices::default()
    }
}

impl SourceFactice {
    pub fn new(dossier_images: PathBuf) -> Self {
        Self {
            dossier_images,
            films: Arc::new(Mutex::new(films_factices())),
            services: Arc::new(Mutex::new(reglages_demo())),
            secrets: Mutex::new(HashMap::new()),
            annulations: Arc::default(),
            prochain_id: AtomicU64::new(1),
            latence: Duration::from_millis(250),
            pas: Duration::from_millis(80),
        }
    }

    /// Secret reçu (tests uniquement).
    pub fn secret(&self, nom: &str) -> Option<String> {
        self.secrets.lock().ok()?.get(nom).cloned()
    }

    fn catalogue() -> Vec<(String, String, u32, String)> {
        let mut v: Vec<(String, String, u32, String)> = films_factices()
            .into_iter()
            .map(|f| (f.titre, f.titre_original, f.annee, f.resume))
            .collect();
        for (t, o, a) in [
            ("Blade Runner", "Blade Runner", 1982),
            ("Alien, le retour", "Aliens", 1986),
            ("Alien³", "Alien 3", 1992),
            ("Dune : Deuxième partie", "Dune: Part Two", 2024),
            ("Matrix Reloaded", "The Matrix Reloaded", 2003),
            ("Mad Max", "Mad Max", 1979),
        ] {
            v.push((
                t.into(),
                o.into(),
                a,
                format!("Résumé factice de « {t} », renvoyé par la recherche de démonstration."),
            ));
        }
        v
    }

    fn image_demo(&self, cle: &str, genre: GenreImage, graine: u64) -> Option<PathBuf> {
        let chemin = self
            .dossier_images
            .join(format!("{cle}-{}.png", genre.suffixe()));
        if !chemin.exists() {
            let (l, h) = genre.dimensions_demo();
            generer_image_demo(&chemin, l, h, graine).ok()?;
        }
        Some(chemin)
    }
}

/// Effet d'une tâche simulée sur les films de démonstration ; renvoie les films modifiés.
fn effet_simule(travail: &Travail, films: &mut [Film], services: &ReglagesServices) -> Vec<Film> {
    let mut modifies = Vec::new();
    match travail {
        Travail::AppliquerResultat {
            film,
            resultat,
            ne_pas_remplacer,
        } => {
            if let Some(f) = films.iter_mut().find(|f| f.id == *film) {
                appliquer_resultat(f, resultat, *ne_pas_remplacer);
                modifies.push(f.clone());
            }
        }
        Travail::RecupererAuto { ids, .. } => {
            for f in films.iter_mut().filter(|f| ids.contains(&f.id)) {
                f.metadonnees_ok = true;
                modifies.push(f.clone());
            }
        }
        Travail::Renommer { ids } => {
            let refs: Vec<&Film> = films.iter().filter(|f| ids.contains(&f.id)).collect();
            let apercus = renommage::calculer(
                &refs,
                &services.gabarit_dossier_film,
                &services.gabarit_fichier_film,
            );
            for a in apercus.into_iter().filter(ApercuFilm::executable) {
                if let Some(f) = films.iter_mut().find(|f| f.id == a.id) {
                    f.dossier = a.nouveau_dossier.clone();
                    for (fic, ligne) in f.fichiers.iter_mut().zip(&a.fichiers) {
                        fic.nom = ligne.nouveau.clone();
                    }
                    if let Some(v) = f
                        .fichiers
                        .iter()
                        .find(|x| x.type_fichier == crate::modele::TypeFichier::Video)
                    {
                        f.nom_fichier = v.nom.clone();
                    }
                    modifies.push(f.clone());
                }
            }
        }
        Travail::MettreAJourSources | Travail::MettreAJourSeries | Travail::EcrireNfo { .. } => {}
    }
    modifies
}

/// Séries de démonstration.
pub fn series_demo() -> Vec<SerieAffichage> {
    let ep = |episode: i32, titre: &str, vu: bool| EpisodeAffichage {
        episode,
        titre: titre.into(),
        fichier: format!("S01E{episode:02}.mkv"),
        vu,
    };
    vec![
        SerieAffichage {
            id: "demo-1".into(),
            titre: "Série Exemple".into(),
            annee: 2019,
            chemin: "D:\\Séries\\Série Exemple (2019)".into(),
            saisons: vec![
                SaisonAffichage {
                    numero: 1,
                    titre: String::new(),
                    episodes: vec![ep(1, "Pilote", true), ep(2, "La suite", false)],
                },
                SaisonAffichage {
                    numero: 2,
                    titre: String::new(),
                    episodes: vec![ep(1, "Le retour", false)],
                },
            ],
        },
        SerieAffichage {
            id: "demo-2".into(),
            titre: "Autre Série".into(),
            annee: 2023,
            chemin: "D:\\Séries\\Autre Série (2023)".into(),
            saisons: vec![SaisonAffichage {
                numero: 1,
                titre: String::new(),
                episodes: vec![ep(1, "Commencement", false)],
            }],
        },
    ]
}

/// Image de démonstration : dégradé dont la teinte dépend de la graine, bandeau sombre en bas.
pub fn generer_image_demo(
    chemin: &Path,
    largeur: u32,
    hauteur: u32,
    graine: u64,
) -> Result<(), String> {
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let base = [
        (graine.wrapping_mul(97) % 200) as u8 + 30,
        (graine.wrapping_mul(53) % 200) as u8 + 30,
        (graine.wrapping_mul(29) % 200) as u8 + 30,
    ];
    let img = image::RgbImage::from_fn(largeur, hauteur, |x, y| {
        let t = y as f32 / hauteur.max(1) as f32;
        let u = x as f32 / largeur.max(1) as f32;
        let sombre = if t > 0.8 { 0.35 } else { 1.0 };
        let c = |v: u8, k: f32| ((v as f32 * (1.0 - 0.5 * t) + 60.0 * k) * sombre).min(255.0) as u8;
        image::Rgb([c(base[0], u), c(base[1], 1.0 - u), c(base[2], t)])
    });
    // Écriture atomique : fichier temporaire puis renommage (deux threads peuvent générer la même image).
    let tmp = chemin.with_extension(format!("{}.tmp", std::process::id()));
    img.save_with_format(&tmp, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, chemin).map_err(|e| e.to_string())
}

impl SourceDonnees for SourceFactice {
    fn films(&self) -> Vec<Film> {
        self.films.lock().map(|f| f.clone()).unwrap_or_default()
    }

    fn series(&self) -> Vec<SerieAffichage> {
        series_demo()
    }

    fn sagas(&self) -> Vec<SagaAffichage> {
        let films = self.films();
        [
            ("demo-saga-dune", crate::donnees::SAGA_DUNE, 901u64),
            ("demo-saga-matrix", crate::donnees::SAGA_MATRIX, 902),
        ]
        .into_iter()
        .map(|(id, titre, graine)| SagaAffichage {
            id: id.into(),
            titre: titre.into(),
            titre_original: String::new(),
            resume: format!("Résumé factice de la collection « {titre} »."),
            films: films
                .iter()
                .filter(|f| f.saga.as_deref() == Some(titre))
                .map(|f| f.id)
                .collect(),
            affiche: self.image_demo(id, GenreImage::Affiche, graine),
            fanart: self.image_demo(id, GenreImage::Fanart, graine),
            banniere: None,
        })
        .collect()
    }

    fn chemin_image(&self, film: &Film, genre: GenreImage) -> Option<PathBuf> {
        // Comme une vraie bibliothèque, certains films n'ont pas d'images.
        if !film.images_ok && genre != GenreImage::Affiche {
            return None;
        }
        self.image_demo(&format!("film{}", film.id), genre, film.id)
    }

    fn rechercher(&self, requete: &RequeteRecherche) -> Result<Vec<ResultatRecherche>, String> {
        std::thread::sleep(self.latence);
        if requete.texte.trim().is_empty() {
            return Err("recherche vide".into());
        }
        let mut v: Vec<ResultatRecherche> = Self::catalogue()
            .into_iter()
            .enumerate()
            .map(|(n, (titre, original, annee, resume))| {
                let score = score_resultat(requete, &titre, &original, annee);
                ResultatRecherche {
                    id: format!("{}", 500_000 + n * 17),
                    affiche: self.image_demo(
                        &format!("resultat{n}"),
                        GenreImage::Affiche,
                        n as u64 + 100,
                    ),
                    titre,
                    titre_original: original,
                    annee,
                    score,
                    resume,
                }
            })
            .filter(|r| r.score >= 0.3)
            .collect();
        v.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.annee.cmp(&b.annee)));
        v.truncate(20);
        Ok(v)
    }

    fn enregistrer_films(&self, films: &[Film]) -> Result<(), String> {
        let mut tous = self.films.lock().map_err(|e| e.to_string())?;
        for f in films {
            if let Some(x) = tous.iter_mut().find(|x| x.id == f.id) {
                *x = f.clone();
            }
        }
        Ok(())
    }

    fn retirer_films(&self, ids: &[u64], _supprimer_fichiers: bool) -> Result<(), String> {
        let mut tous = self.films.lock().map_err(|e| e.to_string())?;
        tous.retain(|f| !ids.contains(&f.id));
        Ok(())
    }

    fn transmettre_secret(&self, nom: &str, valeur: &str) -> Result<(), String> {
        let mut s = self.secrets.lock().map_err(|e| e.to_string())?;
        s.insert(nom.to_string(), valeur.to_string());
        Ok(())
    }

    fn reglages_services(&self) -> ReglagesServices {
        self.services.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn enregistrer_reglages_services(&self, r: &ReglagesServices) -> Result<(), String> {
        *self.services.lock().map_err(|e| e.to_string())? = r.clone();
        Ok(())
    }

    fn apercu_renommage(&self, ids: &[u64]) -> Result<Vec<ApercuFilm>, String> {
        let services = self.reglages_services();
        let films = self.films.lock().map_err(|e| e.to_string())?;
        let refs: Vec<&Film> = films.iter().filter(|f| ids.contains(&f.id)).collect();
        Ok(renommage::calculer(
            &refs,
            &services.gabarit_dossier_film,
            &services.gabarit_fichier_film,
        ))
    }

    fn lancer(&self, travail: Travail, retour: &Retour) -> Result<(), String> {
        let id = self.prochain_id.fetch_add(1, Ordering::SeqCst);
        let annulation = Arc::new(AtomicBool::new(false));
        if let Ok(mut a) = self.annulations.lock() {
            a.insert(id, annulation.clone());
        }
        let total: u32 = match travail {
            Travail::MettreAJourSources => 40,
            Travail::RecupererAuto { .. } => 25,
            _ => 10,
        };
        let (pas, films, services, annulations, retour) = (
            self.pas,
            self.films.clone(),
            self.services.clone(),
            self.annulations.clone(),
            retour.clone(),
        );
        std::thread::spawn(move || {
            retour.envoyer(Message::TacheDemarree {
                id,
                nom: travail.cle_nom().to_string(),
                principale: true,
            });
            let mut issue = IssueTache::Reussie;
            for fait in 1..=total {
                std::thread::sleep(pas);
                if annulation.load(Ordering::Relaxed) {
                    issue = IssueTache::Annulee;
                    break;
                }
                if !retour.envoyer(Message::TacheProgression {
                    id,
                    fait,
                    total,
                    description: String::new(),
                }) {
                    return;
                }
            }
            if issue == IssueTache::Reussie {
                let services = services.lock().map(|s| s.clone()).unwrap_or_default();
                let modifies = match films.lock() {
                    Ok(mut f) => effet_simule(&travail, &mut f, &services),
                    Err(_) => Vec::new(),
                };
                for f in modifies {
                    retour.envoyer(Message::FilmActualise(Box::new(f)));
                }
            }
            if let Ok(mut a) = annulations.lock() {
                a.remove(&id);
            }
            retour.envoyer(Message::TacheTerminee { id, issue });
        });
        Ok(())
    }

    fn annuler_tache(&self, id: u64) {
        if let Some(a) = self
            .annulations
            .lock()
            .ok()
            .and_then(|a| a.get(&id).cloned())
        {
            a.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similarite_bornee_et_ordonnee() {
        assert_eq!(similarite("Matrix", "matrix"), 1.0);
        assert_eq!(similarite("", "x"), 0.0);
        let proche = similarite("blade runner 2049", "Blade Runner 2049");
        let moyen = similarite("blade runner 2049", "Blade Runner");
        let loin = similarite("blade runner 2049", "Parasite");
        assert!(proche > moyen && moyen > loin, "{proche} {moyen} {loin}");
    }

    #[test]
    fn titre_nettoye_depuis_un_dossier() {
        assert_eq!(
            titre_nettoye("Blade.Runner.2049.2017.1080p"),
            ("Blade Runner 2049".into(), Some(2017))
        );
        assert_eq!(titre_nettoye("Alien (1979)"), ("Alien".into(), Some(1979)));
        assert_eq!(
            titre_nettoye("mad.max.fury.road.mkv"),
            ("mad max fury road".into(), None)
        );
    }

    #[test]
    fn recherche_factice_triee_par_score() {
        let dossier = std::env::temp_dir().join(format!("mmr-test-rech-{}", std::process::id()));
        let mut s = SourceFactice::new(dossier.clone());
        s.latence = std::time::Duration::ZERO;
        let r = s
            .rechercher(&RequeteRecherche {
                film: 2,
                extracteur: "TMDB".into(),
                langue: "fr".into(),
                texte: "Blade Runner 2049".into(),
                annee: Some(2017),
            })
            .unwrap();
        assert_eq!(r[0].titre, "Blade Runner 2049");
        assert!(r.windows(2).all(|w| w[0].score >= w[1].score));
        assert!(r.iter().any(|x| x.titre == "Blade Runner"));
        assert!(r[0].affiche.as_ref().unwrap().exists());
        let _ = std::fs::remove_dir_all(dossier);
    }

    #[test]
    fn secrets_conserves_en_memoire() {
        let s = SourceFactice::default();
        s.transmettre_secret("tmdb", "abc").unwrap();
        assert_eq!(s.secret("tmdb").as_deref(), Some("abc"));
    }
}
