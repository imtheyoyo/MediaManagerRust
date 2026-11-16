//! Récupération des métadonnées en ligne : TMDB (films et séries), Fanart.tv
//! (images) et TVmaze (séries).
//!
//! Le crate est volontairement autonome : il définit ses propres types
//! ([`types`]) ; une couche de conversion vers le modèle partagé viendra plus
//! tard. Aucune clé d'API n'est embarquée : elle est fournie par l'utilisateur
//! via [`tmdb::ConfigTmdb`].

pub mod correspondance;
pub mod correspondance_serie;
pub mod erreur;
pub mod evaluations;
pub mod fanart;
pub mod fournisseur;
pub mod http;
pub mod tmdb;
pub mod tvmaze;
pub mod types;

pub use correspondance_serie::{ReglesAppariement, SEUIL_SERIE};
pub use erreur::{ErreurScraper, Resultat};
pub use evaluations::{
    ConfigMdbList, ConfigOmdb, ConfigTrakt, MdbList, Omdb, SERVICES_TESTABLES, Trakt,
    tester_cle_service,
};
pub use fanart::{ConfigFanart, FanartTv};
pub use fournisseur::{
    FournisseurArtwork, FournisseurArtworkSerie, FournisseurBandesAnnonces, FournisseurCollection,
    FournisseurEvaluations, FournisseurFilm, FournisseurSerie,
};
pub use http::{ClientHttp, ConfigHttp};
pub use tmdb::serie::TmdbSerie;
pub use tmdb::{ConfigTmdb, TmdbFilm};
pub use tvmaze::{ConfigTvmaze, TvMaze};
pub use types::*;
