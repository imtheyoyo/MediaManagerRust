//! Choix et téléchargement des images d'un film (affiche, fond, logo…).
//!
//! Les noms de fichiers suivent les motifs de [`NomsFichiersFilm`] (ceux du
//! renommeur : `<b>` = nom de la vidéo sans marqueur de pile, `<n>` = indice),
//! filtrés selon le contexte (dossier multi-films, disque) par
//! [`motifs_artwork`]. Un type sans motif n'est pas téléchargé.

use std::path::{Path, PathBuf};

use mediamanager_core::modele::{FichierMedia, Film, TypeFichier};
use mediamanager_core::renommage::{NomsFichiersFilm, motifs_artwork};
use mediamanager_scrapers::{Artwork, TypeArtwork};
use mediamanager_store::reglages::TailleImage;

use crate::conversion::vers_type_fichier;
use crate::erreur::{ErreurApp, Resultat};

/// Réglages du téléchargement des images.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigArtwork {
    /// Motifs des noms de fichiers par type.
    pub noms: NomsFichiersFilm,
    /// Types téléchargés.
    pub types: Vec<TypeFichier>,
}

impl Default for ConfigArtwork {
    fn default() -> Self {
        Self {
            noms: NomsFichiersFilm::default(),
            types: vec![
                TypeFichier::Poster,
                TypeFichier::Fanart,
                TypeFichier::Clearlogo,
                TypeFichier::Thumb,
            ],
        }
    }
}

/// Rang d'une langue d'image : langues préférées d'abord (dans l'ordre),
/// puis les images sans texte, puis les autres. Pour les fonds, les images
/// sans texte passent en tête.
fn rang_langue(a: &Artwork, langues: &[String]) -> usize {
    if a.langue.is_empty() {
        return if a.type_artwork == TypeArtwork::Fond {
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
pub fn url_pour_taille(a: &Artwork, taille: TailleImage) -> String {
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

/// Meilleure image de chaque type : langue préférée, puis votes.
pub fn choisir_images<'a>(images: &'a [Artwork], langues: &[String]) -> Vec<&'a Artwork> {
    let mut par_type: Vec<&Artwork> = Vec::new();
    for a in images {
        match par_type
            .iter_mut()
            .find(|x| x.type_artwork == a.type_artwork)
        {
            Some(x) => {
                let meilleur = (rang_langue(a, langues), std::cmp::Reverse(a.votes))
                    < (rang_langue(x, langues), std::cmp::Reverse(x.votes));
                if meilleur {
                    *x = a;
                }
            }
            None => par_type.push(a),
        }
    }
    par_type
}

/// Extension d'image d'après l'URL (`jpeg` ⇒ `jpg`, `jpg` par défaut).
pub fn extension_url(url: &str) -> String {
    let chemin = url.split(['?', '#']).next().unwrap_or("");
    let nom = chemin.rsplit('/').next().unwrap_or("");
    let ext = nom
        .rsplit_once('.')
        .map(|(_, e)| e.to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "jpeg" | "" => "jpg".into(),
        "png" | "jpg" | "webp" | "gif" | "bmp" => ext,
        _ => "jpg".into(),
    }
}

/// Chemins (absolus) des fichiers à écrire pour un type d'image.
pub fn destinations(
    film: &Film,
    t: TypeFichier,
    noms: &NomsFichiersFilm,
    ext: &str,
) -> Vec<PathBuf> {
    let base = film.nom_video_sans_pile();
    motifs_artwork(t, noms, film.dossier_multi_films, film.est_disque)
        .into_iter()
        .filter_map(|m| {
            if m.contains("<b>") && base.is_empty() {
                return None;
            }
            let rel = m.replace("<b>", &base).replace("<n>", "");
            let rel = rel.trim_matches('/');
            (!rel.is_empty()).then(|| film.chemin.join(format!("{rel}.{ext}")))
        })
        .collect()
}

/// Écrit un fichier de façon atomique (temporaire puis renommage).
pub fn ecrire_atomiquement(chemin: &Path, octets: &[u8]) -> Resultat<()> {
    if let Some(p) = chemin.parent() {
        std::fs::create_dir_all(p).map_err(|e| ErreurApp::io(p, e))?;
    }
    let mut tmp = chemin.as_os_str().to_os_string();
    tmp.push(".partiel");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, octets).map_err(|e| ErreurApp::io(&tmp, e))?;
    std::fs::rename(&tmp, chemin).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        ErreurApp::io(chemin, e)
    })
}

/// Télécharge une URL (statut HTTP vérifié, contenu non vide).
pub async fn telecharger(client: &reqwest::Client, url: &str) -> Resultat<Vec<u8>> {
    let erreur = |m: String| ErreurApp::Telechargement {
        url: url.to_string(),
        message: m,
    };
    let rep = client
        .get(url)
        .send()
        .await
        .map_err(|e| erreur(e.without_url().to_string()))?;
    if !rep.status().is_success() {
        return Err(erreur(format!("statut HTTP {}", rep.status().as_u16())));
    }
    let octets = rep
        .bytes()
        .await
        .map_err(|e| erreur(e.without_url().to_string()))?;
    if octets.is_empty() {
        return Err(erreur("contenu vide".into()));
    }
    Ok(octets.to_vec())
}

/// Ajoute (ou remplace) un fichier dans la liste du film.
pub fn ajouter_fichier(film: &mut Film, chemin: &Path, t: TypeFichier) {
    let mut f = FichierMedia::nouveau(chemin, t);
    if let Ok(m) = std::fs::metadata(chemin) {
        f.taille = m.len();
        f.date_fichier = m
            .modified()
            .ok()
            .and_then(|d| d.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
    }
    film.fichiers.retain(|x| x.chemin_complet() != chemin);
    film.fichiers.push(f);
    film.fichiers
        .sort_by(mediamanager_core::modele::comparer_fichiers_film);
}

/// Image à télécharger : type, URL, destinations.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageATelecharger {
    pub type_fichier: TypeFichier,
    pub url: String,
    pub destinations: Vec<PathBuf>,
}

/// Prépare la liste des téléchargements d'un film.
pub fn planifier_images(
    film: &Film,
    images: &[Artwork],
    config: &ConfigArtwork,
    langues: &[String],
    taille_affiche: TailleImage,
    taille_fond: TailleImage,
) -> Vec<ImageATelecharger> {
    choisir_images(images, langues)
        .into_iter()
        .filter_map(|a| {
            let t = vers_type_fichier(a.type_artwork);
            if !config.types.contains(&t) {
                return None;
            }
            let taille = if t == TypeFichier::Fanart {
                taille_fond
            } else {
                taille_affiche
            };
            let url = url_pour_taille(a, taille);
            if url.is_empty() {
                return None;
            }
            let dest = destinations(film, t, &config.noms, &extension_url(&url));
            (!dest.is_empty()).then_some(ImageATelecharger {
                type_fichier: t,
                url,
                destinations: dest,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(t: TypeArtwork, langue: &str, votes: u32, url: &str) -> Artwork {
        Artwork {
            fournisseur: "tmdb".into(),
            type_artwork: t,
            url_apercu: String::new(),
            url_originale: url.into(),
            langue: langue.into(),
            votes,
            tailles: Vec::new(),
        }
    }

    #[test]
    fn choix_par_langue_puis_votes() {
        let imgs = vec![
            image(TypeArtwork::Affiche, "en", 50, "en"),
            image(TypeArtwork::Affiche, "fr", 1, "fr"),
            image(TypeArtwork::Fond, "fr", 99, "fond-fr"),
            image(TypeArtwork::Fond, "", 2, "fond-neutre"),
        ];
        let l = vec!["fr".to_string(), "en".to_string()];
        let c = choisir_images(&imgs, &l);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].url_originale, "fr");
        assert_eq!(c[1].url_originale, "fond-neutre");
    }

    #[test]
    fn noms_de_fichiers() {
        let dossier = PathBuf::from("/films/Matrix (1999)");
        let film = Film {
            chemin: dossier.clone(),
            fichiers: vec![FichierMedia::nouveau(
                &dossier.join("Matrix.mkv"),
                TypeFichier::Video,
            )],
            ..Film::default()
        };
        let d = destinations(
            &film,
            TypeFichier::Poster,
            &NomsFichiersFilm::default(),
            "jpg",
        );
        assert_eq!(d, vec![dossier.join("Matrix-poster.jpg")]);
        assert_eq!(extension_url("https://x/y/a.JPEG?z=1"), "jpg");
        assert_eq!(extension_url("https://x/y/a.png"), "png");
    }
}
