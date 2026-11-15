//! Récupération des métadonnées en ligne (TMDB pour l'instant).
//!
//! Le crate est volontairement autonome : il définit ses propres types
//! ([`types`]) ; une couche de conversion vers le modèle partagé viendra plus
//! tard. Aucune clé d'API n'est embarquée : elle est fournie par l'utilisateur
//! via [`tmdb::ConfigTmdb`].

pub mod correspondance;
pub mod erreur;
pub mod fournisseur;
pub mod http;
pub mod tmdb;
pub mod types;

pub use erreur::{ErreurScraper, Resultat};
pub use fournisseur::{
    FournisseurArtwork, FournisseurBandesAnnonces, FournisseurCollection, FournisseurFilm,
};
pub use http::{ClientHttp, ConfigHttp};
pub use tmdb::{ConfigTmdb, TmdbFilm};
pub use types::*;
