//! Journalisation : fichier `logs/mediamanager.log` avec rotation simple.
//!
//! Au démarrage, le journal précédent devient `mediamanager.1.log`, le
//! `.1` devient `.2`, etc. ; au-delà de [`OptionsJournal::archives`] les plus
//! anciens sont supprimés. Un crochet de panique journalise les paniques
//! (fil, message, emplacement) avant le comportement par défaut.
//!
//! L'abonné `tracing` est global au processus : un second appel (tests,
//! redémarrage interne) ne remplace pas le premier et renvoie `Ok(None)`.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, Once};

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use crate::erreur::{ErreurApp, Resultat};

/// Nom du journal courant.
pub const NOM_JOURNAL: &str = "mediamanager.log";

/// Réglages de la journalisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionsJournal {
    /// Active le journal (désactivé : aucun abonné installé).
    pub actif: bool,
    /// Filtre au format `tracing` (`info`, `debug`, `mediamanager_app=trace`…).
    pub filtre: String,
    /// Copie aussi les avertissements et erreurs sur la sortie d'erreur.
    pub console: bool,
    /// Nombre de journaux précédents conservés.
    pub archives: usize,
}

impl Default for OptionsJournal {
    fn default() -> Self {
        Self {
            actif: true,
            filtre: "info".into(),
            console: false,
            archives: 5,
        }
    }
}

fn nom_archive(n: usize) -> String {
    format!("mediamanager.{n}.log")
}

/// Fait tourner les journaux du dossier (sans rien installer).
pub fn faire_tourner(dossier: &Path, archives: usize) -> Resultat<()> {
    std::fs::create_dir_all(dossier).map_err(|e| ErreurApp::io(dossier, e))?;
    let courant = dossier.join(NOM_JOURNAL);
    if archives == 0 {
        if courant.exists() {
            std::fs::remove_file(&courant).map_err(|e| ErreurApp::io(&courant, e))?;
        }
        return Ok(());
    }
    let plus_ancien = dossier.join(nom_archive(archives));
    if plus_ancien.exists() {
        std::fs::remove_file(&plus_ancien).map_err(|e| ErreurApp::io(&plus_ancien, e))?;
    }
    for n in (1..archives).rev() {
        let de = dossier.join(nom_archive(n));
        if de.exists() {
            let vers = dossier.join(nom_archive(n + 1));
            std::fs::rename(&de, &vers).map_err(|e| ErreurApp::io(&de, e))?;
        }
    }
    if courant.exists() {
        let vers = dossier.join(nom_archive(1));
        std::fs::rename(&courant, &vers).map_err(|e| ErreurApp::io(&courant, e))?;
    }
    Ok(())
}

static CROCHET_PANIQUE: Once = Once::new();

fn installer_crochet_panique() {
    CROCHET_PANIQUE.call_once(|| {
        let precedent = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let fil = std::thread::current()
                .name()
                .unwrap_or("<anonyme>")
                .to_string();
            tracing::error!("panique non capturée dans le fil {fil} : {info}");
            precedent(info);
        }));
    });
}

/// Installe la journalisation dans `dossier`. Renvoie le chemin du journal
/// si l'abonné a été installé par cet appel.
pub fn initialiser(dossier: &Path, options: &OptionsJournal) -> Resultat<Option<PathBuf>> {
    if !options.actif {
        return Ok(None);
    }
    if tracing::dispatcher::has_been_set() {
        return Ok(None);
    }
    faire_tourner(dossier, options.archives)?;
    let chemin = dossier.join(NOM_JOURNAL);
    let fichier: File = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&chemin)
        .map_err(|e| ErreurApp::io(&chemin, e))?;
    let filtre = EnvFilter::try_new(&options.filtre).unwrap_or_else(|_| EnvFilter::new("info"));
    let couche_fichier = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_thread_names(true)
        .with_writer(Mutex::new(fichier));
    let couche_console = options.console.then(|| {
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_filter(tracing_subscriber::filter::LevelFilter::WARN)
    });
    let installe = tracing_subscriber::registry()
        .with(filtre)
        .with(couche_fichier)
        .with(couche_console)
        .try_init()
        .is_ok();
    installer_crochet_panique();
    if installe {
        tracing::info!(
            "{} {} — système {} / {}, journal {:?}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            chemin
        );
        Ok(Some(chemin))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_simple() {
        let d = tempfile::tempdir().unwrap();
        for i in 0..4 {
            std::fs::write(d.path().join(NOM_JOURNAL), format!("session {i}")).unwrap();
            faire_tourner(d.path(), 2).unwrap();
        }
        assert!(!d.path().join(NOM_JOURNAL).exists());
        assert_eq!(
            std::fs::read_to_string(d.path().join(nom_archive(1))).unwrap(),
            "session 3"
        );
        assert_eq!(
            std::fs::read_to_string(d.path().join(nom_archive(2))).unwrap(),
            "session 2"
        );
        assert!(!d.path().join(nom_archive(3)).exists());
    }
}
