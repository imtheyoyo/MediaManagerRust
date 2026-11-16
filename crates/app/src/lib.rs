//! Services de l'application, partagés par l'interface graphique et la ligne
//! de commande.
//!
//! Point d'entrée : [`Application::demarrer`]. Les traitements longs (mise à
//! jour des sources, récupération des métadonnées, renommage) s'exécutent en
//! tâches d'arrière-plan et renvoient une [`PoigneeTache`] : identifiant,
//! récepteur d'événements, annulation et résultat typé. L'API est entièrement
//! synchrone (utilisable telle quelle depuis une boucle egui ou une CLI).
//!
//! ```no_run
//! use mediamanager_app::{Application, OptionsDemarrage};
//!
//! let app = Application::demarrer(OptionsDemarrage::default())?;
//! let tache = app.mettre_a_jour_sources_films();
//! for evt in tache.evenements.iter() {
//!     println!("{evt:?}");
//!     if matches!(evt, mediamanager_app::Evenement::TacheTerminee { .. }) {
//!         break;
//!     }
//! }
//! let rapport = tache.attendre()?;
//! println!("{} film(s) ajouté(s)", rapport.ajoutes.len());
//! app.arreter()?;
//! # Ok::<(), mediamanager_app::ErreurApp>(())
//! ```
//!
//! Modules :
//! - [`application`] : démarrage, arrêt, accès aux données, lancement des tâches ;
//! - [`taches`] / [`evenements`] : gestionnaire de tâches et flux d'événements ;
//! - [`sources`] : mise à jour des sources (fusion avec la base) ;
//! - [`conversion`] : types des scrapers ↔ modèle partagé ;
//! - [`metadonnees`], [`artwork`], [`acteurs`], [`ecriture_nfo`] : scraping,
//!   images (dont photos des acteurs), NFO ;
//! - [`renommage`] : aperçu et exécution du renommage ;
//! - [`series`], [`conversion_serie`], [`renommage_serie`] : séries
//!   (scraping, épisodes, images, NFO, renommage) ;
//! - [`gestion_films`] : sélection, films manquants d'une saga, retrait,
//!   annulation du renommage ;
//! - [`analyse`] : analyse des flux des fichiers médias (ffprobe, MediaInfo) ;
//! - [`export`] : export de la bibliothèque par modèles (HTML, CSV, texte) ;
//! - [`nettoyage`] : fichiers inutiles des dossiers de films (corbeille) ;
//! - [`evaluations`] : rafraîchissement des notes ;
//! - [`edition_lot`] : édition globale de films et de séries ;
//! - [`gestion_series`] : état « vu » et retrait des séries ;
//! - [`journal`] : journalisation dans `logs/`.

pub mod acteurs;
pub mod analyse;
pub mod application;
pub mod artwork;
pub mod bandes_annonces;
pub mod conversion;
pub mod conversion_serie;
pub mod ecriture_nfo;
pub mod edition_lot;
pub mod erreur;
pub mod evaluations;
pub mod evenements;
pub mod export;
pub mod fournisseurs;
pub mod gestion_films;
pub mod gestion_series;
pub mod journal;
pub mod metadonnees;
pub mod nettoyage;
pub mod renommage;
pub mod renommage_serie;
pub mod series;
pub mod sources;
pub mod taches;

pub use analyse::{DiagnosticOutils, OutilTrouve, RapportAnalyse};
pub use application::{Application, ConfigServices, OptionsDemarrage};
pub use artwork::ConfigArtwork;
pub use bandes_annonces::{
    FichierBandeAnnonce, OptionsTelechargement, RapportBandesAnnonces, RapportTelechargement,
};
pub use conversion::ConfigMetadonnees;
pub use conversion_serie::ConfigSerie;
pub use edition_lot::RapportLot;
pub use erreur::{ErreurApp, Resultat};
pub use evaluations::RapportEvaluations;
pub use evenements::{Evenement, IdTache, IssueTache, NiveauMessage};
pub use export::{ModeleExport, OptionsExport, RapportExport, TypeExport};
pub use fournisseurs::EtatServiceCle;
pub use gestion_films::{DOSSIER_CORBEILLE_SOURCE, RapportRetrait};
pub use gestion_series::{CibleVu, RapportVu};
pub use journal::OptionsJournal;
pub use mediamanager_core::bandes_annonces::FormatVideoBandeAnnonce;
pub use mediamanager_core::edition_lot::{ModificationsLot, ModificationsLotSeries};
pub use mediamanager_core::nettoyage::RaisonInutile;
pub use mediamanager_store::reglages::QualiteBandeAnnonce;
pub use metadonnees::{RapportScrap, RequeteFilm};
pub use nettoyage::{FichierInutile, RapportNettoyage};
pub use renommage::{ApercuRenommage, RapportRenommage};
pub use series::RequeteSerie;
pub use sources::{OptionsMiseAJour, RapportMiseAJour, TraitementDisparus};
pub use taches::{GestionnaireTaches, InfoTache, PoigneeTache, TypeTache};

/// Types des fournisseurs de métadonnées utiles aux interfaces (résultats
/// de recherche, collections, séries).
pub use mediamanager_scrapers::{
    ArtworkEtendu, CollectionFilm, GroupeEpisodes as GroupeEpisodesFournisseur, MetadonneesEpisode,
    MetadonneesSaison, MetadonneesSerie, PartieCollection, ResultatRecherche,
    StatutDiffusion as StatutDiffusionFournisseur, TypeArtworkEtendu, TypeGroupeEpisodes,
};
