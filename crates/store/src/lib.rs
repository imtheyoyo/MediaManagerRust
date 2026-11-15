//! Stockage de la médiathèque et des réglages.
//!
//! - [`dossiers`] : dossiers de l'application, mode portable ;
//! - [`verrou`] : instance unique (`data/.lock`) ;
//! - [`base`] : base SQLite (films, sagas, séries), migrations ;
//! - [`reglages`] : réglages JSON atomiques ;
//! - [`secrets`] : clés d'API et mots de passe hors du fichier de réglages ;
//! - [`sauvegarde`] : archives quotidiennes, rotation, restauration.
//!
//! [`Stockage::demarrer`] enchaîne le tout dans l'ordre sûr.

mod atomique;
pub mod base;
pub mod dossiers;
pub mod erreur;
pub mod reglages;
pub mod sauvegarde;
pub mod secrets;
pub mod verrou;

#[cfg(test)]
mod tests;

use std::path::PathBuf;

pub use base::{Bibliotheque, Entite, Lot, ReferenceEpisode, VERSION_SCHEMA};
pub use dossiers::{DossiersApplication, ModeDossiers};
pub use erreur::{ErreurStockage, Resultat};
pub use reglages::{
    GestionnaireReglages, Reglages, ReglagesFilms, ReglagesGeneraux, ReglagesRenommeur,
    ReglagesSeries,
};
pub use sauvegarde::{EtatOuverture, EtatSauvegarde};
pub use secrets::{CoffreAuto, CoffreFichier, CoffreMemoire, CoffreSecrets};
pub use verrou::VerrouInstance;

/// Ce qui s'est passé au démarrage (à présenter à l'utilisateur au besoin).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RapportDemarrage {
    pub sauvegarde: EtatSauvegarde,
    pub ouverture: EtatOuverture,
    /// Premier lancement (ou réglages réinitialisés).
    pub reglages_nouveaux: bool,
    /// Fichier de réglages invalide mis de côté.
    pub reglages_corrompus: Option<PathBuf>,
}

/// Stockage ouvert : verrou détenu, base ouverte, réglages chargés.
#[derive(Debug)]
pub struct Stockage {
    pub dossiers: DossiersApplication,
    pub bibliotheque: Bibliotheque,
    pub gestionnaire_reglages: GestionnaireReglages,
    pub reglages: Reglages,
    // Déclaré en dernier : libéré après la fermeture de la base.
    verrou: VerrouInstance,
}

impl Stockage {
    /// Démarrage complet avec le coffre de secrets par défaut.
    pub fn demarrer(dossiers: DossiersApplication) -> Resultat<(Self, RapportDemarrage)> {
        let coffre = Box::new(CoffreAuto::pour_dossier(&dossiers.donnees));
        Self::demarrer_avec(dossiers, coffre)
    }

    /// Démarrage complet : dossiers, verrou (avant tout accès), sauvegarde du
    /// jour (base vérifiée avant copie), ouverture ou restauration de la base,
    /// réglages.
    pub fn demarrer_avec(
        dossiers: DossiersApplication,
        coffre: Box<dyn CoffreSecrets>,
    ) -> Resultat<(Self, RapportDemarrage)> {
        dossiers.creer()?;
        let verrou = VerrouInstance::acquerir(&dossiers.donnees)?;
        let sauvegarde = sauvegarde::creer_sauvegarde_quotidienne(&dossiers)?;
        let (bibliotheque, ouverture) = sauvegarde::ouvrir_ou_restaurer(&dossiers)?;
        let mut gestionnaire_reglages =
            GestionnaireReglages::new(dossiers.fichier_reglages(), coffre);
        let chargement = gestionnaire_reglages.charger()?;
        let rapport = RapportDemarrage {
            sauvegarde,
            ouverture,
            reglages_nouveaux: chargement.nouveau,
            reglages_corrompus: chargement.fichier_corrompu,
        };
        Ok((
            Self {
                dossiers,
                bibliotheque,
                gestionnaire_reglages,
                reglages: chargement.reglages,
                verrou,
            },
            rapport,
        ))
    }

    /// Chemin du verrou détenu.
    pub fn chemin_verrou(&self) -> &std::path::Path {
        self.verrou.chemin()
    }

    /// Enregistre les réglages s'ils ont changé puis ferme la base.
    pub fn fermer(mut self) -> Resultat<()> {
        self.gestionnaire_reglages
            .enregistrer_si_modifie(&self.reglages)?;
        self.bibliotheque.fermer()?;
        drop(self.verrou);
        Ok(())
    }
}
