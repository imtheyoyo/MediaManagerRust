//! Verrou d'instance unique sur `data/.lock`.
//!
//! Verrou consultatif exclusif du système (`flock` / `LockFileEx`) via
//! `std::fs::File::try_lock` : il est libéré automatiquement par le système si
//! le processus meurt, donc pas de fichier « orphelin » à nettoyer. Le PID est
//! écrit dans le fichier à titre informatif uniquement.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::erreur::{ErreurStockage, Resultat};

/// Nom du fichier de verrou dans le dossier de données.
pub const NOM_VERROU: &str = ".lock";

/// Verrou détenu tant que la valeur existe.
#[derive(Debug)]
pub struct VerrouInstance {
    fichier: File,
    chemin: PathBuf,
}

impl VerrouInstance {
    /// Acquiert le verrou sans attendre ; échoue avec
    /// [`ErreurStockage::InstanceDejaLancee`] s'il est déjà détenu.
    pub fn acquerir(dossier_donnees: &Path) -> Resultat<Self> {
        fs::create_dir_all(dossier_donnees).map_err(|e| ErreurStockage::io(dossier_donnees, e))?;
        let chemin = dossier_donnees.join(NOM_VERROU);
        let mut fichier = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&chemin)
            .map_err(|e| ErreurStockage::io(&chemin, e))?;
        match fichier.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(ErreurStockage::InstanceDejaLancee(chemin));
            }
            Err(TryLockError::Error(e)) => return Err(ErreurStockage::io(&chemin, e)),
        }
        // Informatif : PID du détenteur.
        let _ = fichier.set_len(0);
        let _ = writeln!(fichier, "{}", std::process::id());
        Ok(Self { fichier, chemin })
    }

    /// Chemin du fichier de verrou.
    pub fn chemin(&self) -> &Path {
        &self.chemin
    }
}

impl Drop for VerrouInstance {
    fn drop(&mut self) {
        let _ = self.fichier.unlock();
    }
}
