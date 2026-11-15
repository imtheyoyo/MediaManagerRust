//! Frontière d'intégration : tout ce que l'interface attend des services de l'application
//! (bibliothèque, images, recherche d'extracteur, écriture, coffre à secrets).
//!
//! Aujourd'hui alimentée par [`SourceFactice`] (données de `donnees.rs`, images générées
//! localement, résultats de recherche simulés). L'intégration fournira une implémentation
//! adossée aux crates `app` / `store` / `scrapers` sans toucher aux vues.

use crate::donnees::films_factices;
use crate::modele::{Film, normaliser};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

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

/// Ce que l'intégration devra fournir. Les méthodes bloquantes (`rechercher`,
/// `enregistrer_film`) sont appelées hors du thread de l'interface.
pub trait SourceDonnees: Send + Sync {
    /// Instantané de la bibliothèque de films.
    fn films(&self) -> Vec<Film>;
    /// Chemin local d'une illustration du film, s'il existe.
    fn chemin_image(&self, film: &Film, genre: GenreImage) -> Option<PathBuf>;
    /// Recherche auprès d'un extracteur (bloquant).
    fn rechercher(&self, requete: &RequeteRecherche) -> Result<Vec<ResultatRecherche>, String>;
    /// Extracteurs proposés dans le dialogue de recherche.
    fn extracteurs(&self) -> Vec<String> {
        vec!["TMDB".into()]
    }
    /// Écrit un film modifié (base + NFO plus tard).
    fn enregistrer_film(&self, film: &Film) -> Result<(), String>;
    /// Transmet un secret (clé d'API...) au coffre à secrets. Jamais écrit dans `ui.json`.
    fn transmettre_secret(&self, nom: &str, valeur: &str);
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
/// recherche simulée sur un petit catalogue.
pub struct SourceFactice {
    dossier_images: PathBuf,
    secrets: Mutex<HashMap<String, String>>,
    /// Latence simulée de la recherche.
    pub latence: std::time::Duration,
}

impl Default for SourceFactice {
    fn default() -> Self {
        Self::new(std::env::temp_dir().join("mediamanagerrust-demo"))
    }
}

impl SourceFactice {
    pub fn new(dossier_images: PathBuf) -> Self {
        Self {
            dossier_images,
            secrets: Mutex::new(HashMap::new()),
            latence: std::time::Duration::from_millis(250),
        }
    }

    /// Secret reçu (tests uniquement : l'intégration le confiera au coffre).
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
        films_factices()
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

    fn enregistrer_film(&self, _film: &Film) -> Result<(), String> {
        Ok(())
    }

    fn transmettre_secret(&self, nom: &str, valeur: &str) {
        if let Ok(mut s) = self.secrets.lock() {
            s.insert(nom.to_string(), valeur.to_string());
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
        s.transmettre_secret("tmdb", "abc");
        assert_eq!(s.secret("tmdb").as_deref(), Some("abc"));
    }
}
