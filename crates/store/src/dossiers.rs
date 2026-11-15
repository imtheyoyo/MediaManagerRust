//! Résolution des dossiers de l'application et mode portable.
//!
//! Règles (par ordre de priorité) :
//! 1. variable d'environnement [`VARIABLE_DOSSIER`] non vide : ce dossier est la
//!    racine du contenu (mode « explicite ») ;
//! 2. **mode portable par défaut** : si le dossier de l'exécutable est
//!    inscriptible et ne contient pas le marqueur [`MARQUEUR_NON_PORTABLE`],
//!    tout est stocké à côté de l'exécutable (`data/`, `cache/`, `logs/`,
//!    `backup/`) ;
//! 3. sinon (dossier en lecture seule, par exemple `Program Files`, ou
//!    marqueur présent) : dossiers utilisateur standard (crate `directories`) ;
//!    sous Linux dans un conteneur (présence de `/.dockerenv`) : `/data`.
//!
//! Le marqueur [`MARQUEUR_PORTABLE`] reste reconnu (il force le mode portable
//! même en présence du marqueur inverse).

use std::fs;
use std::path::{Path, PathBuf};

use crate::erreur::{ErreurStockage, Resultat};

/// Nom du fichier marqueur du mode portable.
pub const MARQUEUR_PORTABLE: &str = "portable";
/// Nom du fichier marqueur désactivant le mode portable par défaut.
pub const MARQUEUR_NON_PORTABLE: &str = "non-portable";
/// Variable d'environnement forçant la racine du contenu.
pub const VARIABLE_DOSSIER: &str = "MEDIAMANAGER_DOSSIER";
/// Nom de l'application pour les dossiers utilisateur.
pub const NOM_APPLICATION: &str = "MediaManagerRust";

/// Mode de résolution retenu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeDossiers {
    /// Données à côté de l'exécutable.
    Portable,
    /// Dossiers utilisateur standard du système.
    Utilisateur,
    /// Racine imposée (variable d'environnement ou appel explicite).
    Explicite,
}

/// Ensemble des dossiers utilisés par l'application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DossiersApplication {
    pub mode: ModeDossiers,
    /// Racine du contenu.
    pub racine: PathBuf,
    /// Base, réglages, secrets, verrou.
    pub donnees: PathBuf,
    /// Caches (images, HTTP).
    pub cache: PathBuf,
    /// Journaux.
    pub journaux: PathBuf,
    /// Sauvegardes quotidiennes et fichiers corrompus mis de côté.
    pub sauvegardes: PathBuf,
}

impl DossiersApplication {
    /// Tous les sous-dossiers sous une même racine.
    pub fn depuis_racine(racine: impl Into<PathBuf>, mode: ModeDossiers) -> Self {
        let racine = racine.into();
        Self {
            mode,
            donnees: racine.join("data"),
            cache: racine.join("cache"),
            journaux: racine.join("logs"),
            sauvegardes: racine.join("backup"),
            racine,
        }
    }

    /// Résout les dossiers pour l'exécutable courant (voir la doc du module).
    pub fn resoudre() -> Resultat<Self> {
        let forcee = std::env::var_os(VARIABLE_DOSSIER)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from);
        let dossier_exe = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf));
        Self::resoudre_avec(forcee, dossier_exe.as_deref())
    }

    /// Variante testable de [`resoudre`](Self::resoudre).
    pub fn resoudre_avec(forcee: Option<PathBuf>, dossier_exe: Option<&Path>) -> Resultat<Self> {
        if let Some(racine) = forcee {
            return Ok(Self::depuis_racine(racine, ModeDossiers::Explicite));
        }
        if let Some(exe) = dossier_exe {
            let force = exe.join(MARQUEUR_PORTABLE).is_file();
            let refuse = exe.join(MARQUEUR_NON_PORTABLE).is_file();
            if force || !refuse {
                if est_inscriptible(exe) {
                    return Ok(Self::depuis_racine(exe, ModeDossiers::Portable));
                }
                tracing::warn!(
                    "mode portable demandé mais {exe:?} n'est pas inscriptible : \
                     utilisation des dossiers utilisateur"
                );
            }
        }
        Self::utilisateur()
    }

    /// Dossiers utilisateur standard.
    fn utilisateur() -> Resultat<Self> {
        if cfg!(target_os = "linux") && Path::new("/.dockerenv").exists() {
            return Ok(Self::depuis_racine("/data", ModeDossiers::Utilisateur));
        }
        let projet = directories::ProjectDirs::from("", "", NOM_APPLICATION)
            .ok_or(ErreurStockage::DossierUtilisateurIntrouvable)?;
        let mut d = Self::depuis_racine(projet.data_dir(), ModeDossiers::Utilisateur);
        d.cache = projet.cache_dir().to_path_buf();
        Ok(d)
    }

    /// Crée tous les dossiers manquants.
    pub fn creer(&self) -> Resultat<()> {
        for d in [
            &self.donnees,
            &self.cache,
            &self.journaux,
            &self.sauvegardes,
        ] {
            fs::create_dir_all(d).map_err(|e| ErreurStockage::io(d, e))?;
        }
        Ok(())
    }

    /// Chemin de la base de la médiathèque.
    pub fn base(&self) -> PathBuf {
        self.donnees.join(crate::base::NOM_BASE)
    }

    /// Chemin du fichier de réglages.
    pub fn fichier_reglages(&self) -> PathBuf {
        self.donnees.join(crate::reglages::NOM_FICHIER_REGLAGES)
    }
}

/// Teste l'écriture réelle (création puis suppression d'un fichier) : plus
/// fiable que les permissions affichées (ACL, montages en lecture seule).
pub fn est_inscriptible(dossier: &Path) -> bool {
    let test = dossier.join(format!(".test-ecriture-{}", std::process::id()));
    match fs::File::create(&test) {
        Ok(_) => {
            let _ = fs::remove_file(&test);
            true
        }
        Err(_) => false,
    }
}
