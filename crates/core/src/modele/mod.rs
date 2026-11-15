//! Modèle de données : films, sagas, séries, fichiers média, identifiants,
//! notes et personnes. Tous les types sont (dé)sérialisables avec serde.

pub mod commun;
pub mod fichier;
pub mod film;
pub mod serie;

pub use commun::{
    BandeAnnonce, Edition, Identifiants, Note, Personne, SourceMedia, TypePersonne, ValeurId,
    est_id_imdb_valide,
};
pub use fichier::{
    DrapeauFlux, FichierMedia, FluxAudio, FluxSousTitres, FluxVideo, TypeFichier,
    comparer_fichiers_film,
};
pub use film::{Film, HistoriqueRenommage, Saga};
pub use serie::{
    Episode, GroupeEpisodes, NumeroEpisode, Saison, Serie, StatutDiffusion, StyleMultiEpisodes,
    TypeEpisode, TypeGroupe,
};

#[cfg(test)]
mod tests;
