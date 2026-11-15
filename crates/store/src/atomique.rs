//! Écriture atomique de fichiers : fichier temporaire dans le même dossier,
//! synchronisation sur disque, puis renommage par-dessus la cible.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::erreur::{ErreurStockage, Resultat};

/// Chemin du fichier temporaire associé à `cible`.
pub(crate) fn chemin_temporaire(cible: &Path) -> PathBuf {
    let mut nom = cible.file_name().unwrap_or_default().to_os_string();
    nom.push(format!(".{}.tmp", std::process::id()));
    cible.with_file_name(nom)
}

/// Écrit `contenu` dans `cible` de façon atomique. Avec `prive`, le fichier
/// est créé avec les permissions `0600` (Unix).
pub(crate) fn ecrire_atomiquement(cible: &Path, contenu: &[u8], prive: bool) -> Resultat<()> {
    if let Some(parent) = cible.parent() {
        fs::create_dir_all(parent).map_err(|e| ErreurStockage::io(parent, e))?;
    }
    let tmp = chemin_temporaire(cible);
    let resultat = (|| -> std::io::Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        if prive {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(not(unix))]
        let _ = prive;
        let mut f = options.open(&tmp)?;
        f.write_all(contenu)?;
        f.sync_all()?;
        drop(f);
        // `rename` remplace la cible existante (Unix et Windows).
        fs::rename(&tmp, cible)?;
        // Rend le renommage durable (Unix) ; sans effet ailleurs.
        #[cfg(unix)]
        if let Some(parent) = cible.parent()
            && let Ok(d) = fs::File::open(parent)
        {
            let _ = d.sync_all();
        }
        Ok(())
    })();
    resultat.map_err(|e| {
        let _ = fs::remove_file(&tmp);
        ErreurStockage::io(cible, e)
    })
}
