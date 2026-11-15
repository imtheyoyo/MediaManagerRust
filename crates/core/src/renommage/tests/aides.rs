//! Constructeurs de données de test.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::modele::*;

pub fn fichier(chemin: &str, t: TypeFichier) -> FichierMedia {
    FichierMedia::nouveau(Path::new(chemin), t)
}

pub fn video_hd(chemin: &str) -> FichierMedia {
    let mut f = fichier(chemin, TypeFichier::Video);
    f.video.largeur = 1920;
    f.video.hauteur = 1080;
    f.video.codec = "h264".into();
    f.taille = 4_370_000_000;
    f.flux_audio.push(FluxAudio {
        codec: "AC3".into(),
        canaux: 6,
        langue: "eng".into(),
        ..Default::default()
    });
    f
}

/// « The Matrix » (1999) dans `/films/Matrix.1999.1080p`.
pub fn matrix() -> Film {
    Film {
        id: "1".into(),
        titre: "The Matrix".into(),
        titre_original: "The Matrix".into(),
        annee: 1999,
        source_donnees: PathBuf::from("/films"),
        chemin: PathBuf::from("/films/Matrix.1999.1080p"),
        fichiers: vec![video_hd("/films/Matrix.1999.1080p/Matrix.1999.1080p.mkv")],
        ..Film::default()
    }
}

/// Base attendue avec le profil par défaut.
pub const BASE_MATRIX: &str = "The Matrix (1999) 1080p AC3";
pub const DOSSIER_MATRIX: &str = "/films/The Matrix (1999)";

pub fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

/// Série « Show » (2008) avec des épisodes.
pub fn serie(episodes: Vec<Episode>) -> Serie {
    Serie {
        id: "s".into(),
        titre: "Show".into(),
        annee: 2008,
        source_donnees: p("/series"),
        chemin: p("/series/show"),
        episodes,
        ..Serie::default()
    }
}

pub fn episode(
    id: &str,
    saison: i32,
    numero: i32,
    titre: &str,
    fichiers: Vec<FichierMedia>,
) -> Episode {
    Episode {
        id: id.into(),
        titre: titre.into(),
        numeros: vec![NumeroEpisode {
            groupe: GroupeEpisodes::diffusion(),
            saison,
            episode: numero,
        }],
        fichiers,
        ..Episode::default()
    }
}

/// Crée les fichiers (contenu = chemin relatif) sous `racine`.
pub fn creer(racine: &Path, relatifs: &[&str]) {
    for r in relatifs {
        let c = racine.join(r);
        fs::create_dir_all(c.parent().unwrap()).unwrap();
        fs::write(&c, r.as_bytes()).unwrap();
    }
}

/// Instantané : chemins relatifs (dossiers suffixés par `/`) → contenu.
pub fn instantane(racine: &Path) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for e in walkdir::WalkDir::new(racine).min_depth(1) {
        let e = e.unwrap();
        let rel = e
            .path()
            .strip_prefix(racine)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if e.file_type().is_dir() {
            m.insert(format!("{rel}/"), String::new());
        } else {
            m.insert(rel, fs::read_to_string(e.path()).unwrap_or_default());
        }
    }
    m
}

/// Film construit à partir de fichiers réels (types devinés par le test).
pub fn film_disque(source: &Path, dossier: &str, fichiers: &[(&str, TypeFichier)]) -> Film {
    let chemin = source.join(dossier);
    Film {
        id: dossier.into(),
        titre: "The Matrix".into(),
        annee: 1999,
        source_donnees: source.to_path_buf(),
        chemin: chemin.clone(),
        fichiers: fichiers
            .iter()
            .map(|(r, t)| {
                let mut f = FichierMedia::nouveau(&chemin.join(r), *t);
                if *t == TypeFichier::Video {
                    f.video.largeur = 1920;
                    f.video.hauteur = 1080;
                    f.flux_audio.push(FluxAudio {
                        codec: "AC3".into(),
                        canaux: 6,
                        ..Default::default()
                    });
                }
                f
            })
            .collect(),
        ..Film::default()
    }
}
