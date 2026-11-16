//! Localisation des outils externes d'analyse (`ffprobe`, `mediainfo`).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{DELAI_PAR_DEFAUT, execution};

/// Sous-dossiers du dossier de l'application où chercher les outils.
const SOUS_DOSSIERS: &[&str] = &["outils", "addons", ""];

/// Réglages de recherche des outils.
#[derive(Debug, Clone, Default)]
pub struct ConfigOutils {
    /// Chemin réglé par l'utilisateur pour `ffprobe` : exécutable, dossier le
    /// contenant, ou exécutable `ffmpeg` voisin (le nom est alors transposé).
    pub chemin_ffprobe: Option<PathBuf>,
    /// Chemin réglé par l'utilisateur pour `mediainfo` (exécutable ou dossier).
    pub chemin_mediainfo: Option<PathBuf>,
    /// Dossier de l'application ; par défaut celui de l'exécutable courant.
    pub dossier_application: Option<PathBuf>,
    /// Valeur de `PATH` à utiliser ; par défaut celle de l'environnement.
    pub path: Option<OsString>,
    /// Délai maximal d'une analyse ; par défaut [`DELAI_PAR_DEFAUT`].
    pub delai: Option<Duration>,
}

/// Outils trouvés et délai d'exécution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outils {
    pub ffprobe: Option<PathBuf>,
    pub mediainfo: Option<PathBuf>,
    /// Délai maximal accordé à chaque lancement.
    pub delai: Duration,
}

impl Default for Outils {
    fn default() -> Self {
        Self {
            ffprobe: None,
            mediainfo: None,
            delai: DELAI_PAR_DEFAUT,
        }
    }
}

impl Outils {
    /// Vrai si au moins un outil d'analyse est utilisable.
    pub fn disponible(&self) -> bool {
        self.ffprobe.is_some() || self.mediainfo.is_some()
    }
}

/// Cherche `ffprobe` puis `mediainfo`. Pour chacun, dans l'ordre : chemin
/// réglé par l'utilisateur, dossier de l'application (`outils/`, `addons/`
/// puis le dossier lui-même), enfin `PATH`.
pub fn trouver_outils(config: &ConfigOutils) -> Outils {
    let dossier = config.dossier_application.clone().or_else(|| {
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf))
    });
    let path = config.path.clone().or_else(|| std::env::var_os("PATH"));
    let chercher = |nom: &str, regle: Option<&Path>| {
        trouver_outil(nom, regle, dossier.as_deref(), path.as_deref())
    };
    Outils {
        ffprobe: chercher("ffprobe", config.chemin_ffprobe.as_deref()),
        mediainfo: chercher("mediainfo", config.chemin_mediainfo.as_deref()),
        delai: config.delai.unwrap_or(DELAI_PAR_DEFAUT),
    }
}

/// Cherche un outil par son nom (sans extension).
pub(super) fn trouver_outil(
    nom: &str,
    regle: Option<&Path>,
    dossier_application: Option<&Path>,
    path: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    if let Some(regle) = regle.filter(|r| !r.as_os_str().is_empty())
        && let Some(trouve) = depuis_reglage(nom, regle)
    {
        return Some(trouve);
    }
    if let Some(dossier) = dossier_application {
        for sous in SOUS_DOSSIERS {
            let base = if sous.is_empty() {
                dossier.to_path_buf()
            } else {
                dossier.join(sous)
            };
            if let Some(trouve) = dans_dossier(nom, &base) {
                return Some(trouve);
            }
        }
    }
    path.and_then(|p| std::env::split_paths(p).find_map(|d| dans_dossier(nom, &d)))
}

/// Interprète le chemin réglé : dossier, exécutable attendu, ou exécutable
/// voisin d'un autre nom (`ffmpeg` pour `ffprobe`).
fn depuis_reglage(nom: &str, regle: &Path) -> Option<PathBuf> {
    if regle.is_dir() {
        return dans_dossier(nom, regle);
    }
    let radical = regle
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if radical == nom {
        return est_executable(regle).then(|| regle.to_path_buf());
    }
    // Réglage pointant vers un outil voisin : même dossier, nom transposé.
    regle.parent().and_then(|parent| dans_dossier(nom, parent))
}

fn dans_dossier(nom: &str, dossier: &Path) -> Option<PathBuf> {
    noms_executables(nom)
        .into_iter()
        .map(|n| dossier.join(n))
        .find(|c| est_executable(c))
}

fn noms_executables(nom: &str) -> Vec<String> {
    if cfg!(windows) {
        vec![format!("{nom}.exe"), nom.to_string()]
    } else {
        vec![nom.to_string()]
    }
}

fn est_executable(chemin: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(chemin) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Première ligne de la sortie de version de l'outil (essai d'exécution,
/// délai de 5 s) ; `None` si l'outil ne se lance pas.
pub fn version_outil(chemin: &Path) -> Option<String> {
    let est_ffprobe = chemin
        .file_stem()
        .is_some_and(|s| s.to_string_lossy().to_lowercase().starts_with("ff"));
    let argument = if est_ffprobe { "-version" } else { "--Version" };
    let sortie =
        execution::executer(chemin, &[OsString::from(argument)], Duration::from_secs(5)).ok()?;
    // ffprobe annonce sa version en première ligne ; mediainfo en dernière.
    let mut lignes = sortie.lines().map(str::trim).filter(|l| !l.is_empty());
    if est_ffprobe {
        lignes.next()
    } else {
        lignes.next_back()
    }
    .map(str::to_string)
}

/// Cherche `yt-dlp` : chemin réglé par l'utilisateur (exécutable quel que
/// soit son nom, ou dossier le contenant), puis dossier de l'application
/// (`outils/`, `addons/`, le dossier lui-même), enfin `PATH`. Le délai de
/// la configuration n'intervient pas ici.
pub fn trouver_yt_dlp(config: &ConfigOutils, chemin_reglage: &str) -> Option<PathBuf> {
    let regle = chemin_reglage.trim();
    if !regle.is_empty() {
        let regle = Path::new(regle);
        if regle.is_file() {
            if est_executable(regle) {
                return Some(regle.to_path_buf());
            }
        } else if let Some(trouve) = dans_dossier("yt-dlp", regle) {
            return Some(trouve);
        }
    }
    let dossier = config.dossier_application.clone().or_else(|| {
        std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf))
    });
    let path = config.path.clone().or_else(|| std::env::var_os("PATH"));
    trouver_outil("yt-dlp", None, dossier.as_deref(), path.as_deref())
}

/// Version annoncée par `yt-dlp --version` (délai de 10 s) ; `None` si
/// l'outil ne se lance pas.
pub fn version_yt_dlp(chemin: &Path) -> Option<String> {
    let sortie = execution::executer(
        chemin,
        &[OsString::from("--version")],
        Duration::from_secs(10),
    )
    .ok()?;
    sortie
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_string)
}
