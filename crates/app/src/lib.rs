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
//! - [`metadonnees`], [`artwork`], [`ecriture_nfo`] : scraping, images, NFO ;
//! - [`renommage`] : aperçu et exécution du renommage ;
//! - [`series`], [`conversion_serie`], [`renommage_serie`] : séries
//!   (scraping, épisodes, images, NFO, renommage) ;
//! - [`gestion_films`] : sélection, films manquants d'une saga, retrait,
//!   annulation du renommage ;
//! - [`journal`] : journalisation dans `logs/`.

pub mod application;
pub mod artwork;
pub mod conversion;
pub mod conversion_serie;
pub mod ecriture_nfo;
pub mod erreur;
pub mod evenements;
pub mod gestion_films;
pub mod journal;
pub mod metadonnees;
pub mod renommage;
pub mod renommage_serie;
pub mod series;
pub mod sources;
pub mod taches;

pub use application::{Application, ConfigServices, OptionsDemarrage};
pub use artwork::ConfigArtwork;
pub use conversion::ConfigMetadonnees;
pub use conversion_serie::ConfigSerie;
pub use erreur::{ErreurApp, Resultat};
pub use evenements::{Evenement, IdTache, IssueTache, NiveauMessage};
pub use gestion_films::{DOSSIER_CORBEILLE_SOURCE, RapportRetrait};
pub use journal::OptionsJournal;
pub use metadonnees::{RapportScrap, RequeteFilm};
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
