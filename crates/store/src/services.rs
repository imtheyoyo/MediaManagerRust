//! Services externes à clé d'API : liste connue et accès à leurs clés dans le
//! coffre de secrets.
//!
//! Les clés vivent dans le [`CoffreSecrets`] (trousseau du système ou fichier
//! `0600`), jamais dans le fichier de réglages, et ne sont jamais journalisées :
//! aucune fonction de ce module n'écrit de valeur de secret dans un message
//! d'erreur ou de journal.
//!
//! La clé TMDB est aussi un champ de [`crate::Reglages`] (`general.cle_api_tmdb`,
//! recopié dans le coffre à l'enregistrement des réglages) : pour ce service,
//! passer par les réglages (ou `Application::enregistrer_cle`, qui le fait)
//! plutôt que par [`enregistrer_cle`], sinon la copie en mémoire des réglages
//! écraserait la nouvelle valeur au prochain enregistrement.

use crate::erreur::{ErreurStockage, Resultat};
use crate::reglages::noms_secrets;
use crate::secrets::CoffreSecrets;

/// Définition d'un service à clé d'API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefinitionService {
    /// Identifiant stable (`tmdb`, `fanart`, `tvdb`, `omdb`, `trakt`,
    /// `mdblist`, `simkl`).
    pub id: &'static str,
    /// Nom affichable.
    pub nom: &'static str,
    /// Nom du secret dans le coffre.
    pub secret: &'static str,
    /// Nature de la clé (par exemple « identifiant client »).
    pub type_cle: &'static str,
    /// Usage dans l'application.
    pub usage: &'static str,
}

/// Services connus, dans l'ordre d'affichage.
pub const SERVICES: [DefinitionService; 7] = [
    DefinitionService {
        id: "tmdb",
        nom: "TMDB",
        secret: noms_secrets::CLE_API_TMDB,
        type_cle: "clé d'API",
        usage: "métadonnées des films et séries, notes TMDB, images",
    },
    DefinitionService {
        id: "fanart",
        nom: "Fanart.tv",
        secret: noms_secrets::CLE_API_FANART,
        type_cle: "clé d'API de projet",
        usage: "images : logo, clearart, bannière, disque, vignette",
    },
    DefinitionService {
        id: "tvdb",
        nom: "TheTVDB",
        secret: noms_secrets::CLE_API_TVDB,
        type_cle: "clé d'API v4",
        usage: "séries (clé enregistrée ; fournisseur non disponible)",
    },
    DefinitionService {
        id: "omdb",
        nom: "OMDb",
        secret: noms_secrets::CLE_API_OMDB,
        type_cle: "clé d'API",
        usage: "notes IMDb, Rotten Tomatoes, Metacritic",
    },
    DefinitionService {
        id: "trakt",
        nom: "Trakt",
        secret: noms_secrets::CLE_API_TRAKT,
        type_cle: "identifiant client (client id)",
        usage: "notes et votes Trakt",
    },
    DefinitionService {
        id: "mdblist",
        nom: "MDBList",
        secret: noms_secrets::CLE_API_MDBLIST,
        type_cle: "clé d'API",
        usage: "notes agrégées (IMDb, Rotten Tomatoes, Metacritic, Trakt)",
    },
    DefinitionService {
        id: "simkl",
        nom: "Simkl",
        secret: noms_secrets::CLE_API_SIMKL,
        type_cle: "identifiant client (client id)",
        usage: "clé enregistrée (aucun usage pour l'instant)",
    },
];

/// Définition d'un service d'après son identifiant (insensible à la casse).
pub fn definition_service(id: &str) -> Option<&'static DefinitionService> {
    let id = id.trim();
    SERVICES.iter().find(|s| s.id.eq_ignore_ascii_case(id))
}

fn definition(id: &str) -> Resultat<&'static DefinitionService> {
    definition_service(id).ok_or_else(|| ErreurStockage::Secrets(format!("service inconnu : {id}")))
}

/// Clé enregistrée pour un service (`None` si absente ou vide).
pub fn lire_cle_api(coffre: &dyn CoffreSecrets, service: &str) -> Resultat<Option<String>> {
    let d = definition(service)?;
    Ok(coffre
        .lire(d.secret)?
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty()))
}

/// Vrai si une clé non vide est enregistrée pour ce service.
pub fn cle_api_enregistree(coffre: &dyn CoffreSecrets, service: &str) -> Resultat<bool> {
    Ok(lire_cle_api(coffre, service)?.is_some())
}

/// Enregistre la clé d'un service (espaces retirés) ; une valeur vide la
/// supprime. Voir la remarque du module pour TMDB.
pub fn enregistrer_cle(coffre: &dyn CoffreSecrets, service: &str, secret: &str) -> Resultat<()> {
    let d = definition(service)?;
    coffre.ecrire(d.secret, secret.trim())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::CoffreMemoire;

    #[test]
    fn enregistrement_lecture_suppression() {
        let c = CoffreMemoire::default();
        for s in SERVICES {
            assert!(!cle_api_enregistree(&c, s.id).unwrap());
        }
        enregistrer_cle(&c, "OMDb", "  abc123  ").unwrap();
        assert!(cle_api_enregistree(&c, "omdb").unwrap());
        assert_eq!(lire_cle_api(&c, "omdb").unwrap().as_deref(), Some("abc123"));
        // Les autres services ne sont pas touchés.
        assert!(!cle_api_enregistree(&c, "trakt").unwrap());
        enregistrer_cle(&c, "omdb", "").unwrap();
        assert!(!cle_api_enregistree(&c, "omdb").unwrap());
    }

    #[test]
    fn service_inconnu_sans_fuite() {
        let c = CoffreMemoire::default();
        let e = enregistrer_cle(&c, "inconnu", "valeur-secrete").unwrap_err();
        assert!(!e.to_string().contains("valeur-secrete"));
        assert!(cle_api_enregistree(&c, "inconnu").is_err());
    }

    #[test]
    fn secrets_distincts() {
        let mut noms: Vec<_> = SERVICES.iter().map(|s| s.secret).collect();
        noms.sort_unstable();
        noms.dedup();
        assert_eq!(noms.len(), SERVICES.len());
    }
}
