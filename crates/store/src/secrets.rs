//! Magasin de secrets (clés d'API, mots de passe).
//!
//! - Windows et macOS : trousseau du système (crate `keyring`, magasins natifs
//!   « Credential Manager » et « Keychain »), avec repli sur le fichier si le
//!   trousseau refuse l'opération.
//! - Autres systèmes (Linux…) : fichier `data/secrets.json` créé en `0600`
//!   (lisible par l'utilisateur seul), écrit atomiquement. Choix documenté :
//!   le trousseau Linux (Secret Service) impose D-Bus et un démon de session
//!   souvent absent (serveur, conteneur, NAS) ; le magasin du noyau
//!   (`keyutils`) n'est pas persistant au redémarrage. Le fichier `0600` est
//!   la protection usuelle des outils en ligne de commande ; il n'est **pas**
//!   chiffré. Il est exclu des sauvegardes zip (dont les permissions ne sont
//!   pas restreintes).
//!
//! Les secrets ne sont jamais journalisés.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::atomique::ecrire_atomiquement;
use crate::erreur::{ErreurStockage, Resultat};

/// Nom du fichier de repli dans le dossier de données.
pub const NOM_FICHIER_SECRETS: &str = "secrets.json";

/// Magasin de secrets nommés.
pub trait CoffreSecrets: Send + Sync {
    /// Valeur du secret, `None` s'il est absent.
    fn lire(&self, nom: &str) -> Resultat<Option<String>>;
    /// Écrit (ou remplace) un secret. Une valeur vide le supprime.
    fn ecrire(&self, nom: &str, valeur: &str) -> Resultat<()>;
    /// Supprime un secret (sans erreur s'il est absent).
    fn supprimer(&self, nom: &str) -> Resultat<()>;
}

// ---------------------------------------------------------------------------
// Mémoire (tests)
// ---------------------------------------------------------------------------

/// Magasin en mémoire, non persistant (tests, mode « sans secrets »).
#[derive(Debug, Default)]
pub struct CoffreMemoire(Mutex<BTreeMap<String, String>>);

impl CoffreSecrets for CoffreMemoire {
    fn lire(&self, nom: &str) -> Resultat<Option<String>> {
        Ok(self.0.lock().expect("coffre empoisonné").get(nom).cloned())
    }
    fn ecrire(&self, nom: &str, valeur: &str) -> Resultat<()> {
        if valeur.is_empty() {
            return self.supprimer(nom);
        }
        self.0
            .lock()
            .expect("coffre empoisonné")
            .insert(nom.to_string(), valeur.to_string());
        Ok(())
    }
    fn supprimer(&self, nom: &str) -> Resultat<()> {
        self.0.lock().expect("coffre empoisonné").remove(nom);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Fichier 0600
// ---------------------------------------------------------------------------

/// Magasin dans un fichier JSON à permissions restreintes.
#[derive(Debug)]
pub struct CoffreFichier {
    chemin: PathBuf,
    verrou: Mutex<()>,
}

impl CoffreFichier {
    pub fn new(chemin: impl Into<PathBuf>) -> Self {
        Self {
            chemin: chemin.into(),
            verrou: Mutex::new(()),
        }
    }

    pub fn chemin(&self) -> &Path {
        &self.chemin
    }

    fn charger(&self) -> Resultat<BTreeMap<String, String>> {
        match std::fs::read(&self.chemin) {
            Ok(octets) => Ok(serde_json::from_slice(&octets).unwrap_or_else(|e| {
                tracing::warn!(
                    "fichier de secrets {:?} illisible, ignoré : {e}",
                    self.chemin
                );
                BTreeMap::new()
            })),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(ErreurStockage::io(&self.chemin, e)),
        }
    }

    fn sauver(&self, table: &BTreeMap<String, String>) -> Resultat<()> {
        ecrire_atomiquement(&self.chemin, &serde_json::to_vec_pretty(table)?, true)
    }
}

impl CoffreSecrets for CoffreFichier {
    fn lire(&self, nom: &str) -> Resultat<Option<String>> {
        let _g = self.verrou.lock().expect("coffre empoisonné");
        Ok(self.charger()?.remove(nom))
    }
    fn ecrire(&self, nom: &str, valeur: &str) -> Resultat<()> {
        if valeur.is_empty() {
            return self.supprimer(nom);
        }
        let _g = self.verrou.lock().expect("coffre empoisonné");
        let mut t = self.charger()?;
        if t.get(nom).map(String::as_str) == Some(valeur) {
            return Ok(());
        }
        t.insert(nom.to_string(), valeur.to_string());
        self.sauver(&t)
    }
    fn supprimer(&self, nom: &str) -> Resultat<()> {
        let _g = self.verrou.lock().expect("coffre empoisonné");
        let mut t = self.charger()?;
        if t.remove(nom).is_some() {
            self.sauver(&t)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Trousseau du système (Windows, macOS)
// ---------------------------------------------------------------------------

#[cfg(any(windows, target_os = "macos"))]
mod systeme {
    use super::*;

    /// Trousseau natif du système.
    #[derive(Debug, Default)]
    pub struct CoffreSysteme;

    fn entree(nom: &str) -> Resultat<keyring::Entry> {
        keyring::Entry::new(crate::dossiers::NOM_APPLICATION, nom)
            .map_err(|e| ErreurStockage::Secrets(e.to_string()))
    }

    impl CoffreSecrets for CoffreSysteme {
        fn lire(&self, nom: &str) -> Resultat<Option<String>> {
            match entree(nom)?.get_password() {
                Ok(v) => Ok(Some(v)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(ErreurStockage::Secrets(e.to_string())),
            }
        }
        fn ecrire(&self, nom: &str, valeur: &str) -> Resultat<()> {
            if valeur.is_empty() {
                return self.supprimer(nom);
            }
            entree(nom)?
                .set_password(valeur)
                .map_err(|e| ErreurStockage::Secrets(e.to_string()))
        }
        fn supprimer(&self, nom: &str) -> Resultat<()> {
            match entree(nom)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(ErreurStockage::Secrets(e.to_string())),
            }
        }
    }
}

#[cfg(any(windows, target_os = "macos"))]
pub use systeme::CoffreSysteme;

// ---------------------------------------------------------------------------
// Coffre par défaut : trousseau puis repli fichier
// ---------------------------------------------------------------------------

/// Trousseau du système si disponible, sinon fichier `0600`.
pub struct CoffreAuto {
    systeme: Option<Box<dyn CoffreSecrets>>,
    fichier: CoffreFichier,
}

impl std::fmt::Debug for CoffreAuto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoffreAuto")
            .field("systeme", &self.systeme.is_some())
            .field("fichier", &self.fichier.chemin)
            .finish()
    }
}

impl CoffreAuto {
    /// Coffre par défaut pour un dossier de données.
    pub fn pour_dossier(dossier_donnees: &Path) -> Self {
        let fichier = CoffreFichier::new(dossier_donnees.join(NOM_FICHIER_SECRETS));
        #[cfg(any(windows, target_os = "macos"))]
        let systeme: Option<Box<dyn CoffreSecrets>> = Some(Box::new(CoffreSysteme));
        #[cfg(not(any(windows, target_os = "macos")))]
        let systeme: Option<Box<dyn CoffreSecrets>> = None;
        Self { systeme, fichier }
    }

    /// Indique si le trousseau du système est utilisé en premier.
    pub fn utilise_trousseau(&self) -> bool {
        self.systeme.is_some()
    }
}

impl CoffreSecrets for CoffreAuto {
    fn lire(&self, nom: &str) -> Resultat<Option<String>> {
        if let Some(s) = &self.systeme {
            match s.lire(nom) {
                Ok(Some(v)) => return Ok(Some(v)),
                Ok(None) => {}
                Err(e) => tracing::warn!("trousseau indisponible ({e}) : repli sur le fichier"),
            }
        }
        self.fichier.lire(nom)
    }
    fn ecrire(&self, nom: &str, valeur: &str) -> Resultat<()> {
        if let Some(s) = &self.systeme {
            match s.ecrire(nom, valeur) {
                // Écrit dans le trousseau : on retire une éventuelle copie de repli.
                Ok(()) => return self.fichier.supprimer(nom),
                Err(e) => tracing::warn!("trousseau indisponible ({e}) : repli sur le fichier"),
            }
        }
        self.fichier.ecrire(nom, valeur)
    }
    fn supprimer(&self, nom: &str) -> Resultat<()> {
        if let Some(s) = &self.systeme
            && let Err(e) = s.supprimer(nom)
        {
            tracing::warn!("trousseau indisponible ({e})");
        }
        self.fichier.supprimer(nom)
    }
}
