//! Lecture et écriture des fichiers NFO (films, séries, épisodes, saisons).
//!
//! - Lecture **tolérante** : XML valide ou bruité (texte autour de la racine,
//!   BOM, UTF-16, Windows-1252, balises mal fermées), et NFO réduit à une URL
//!   ou à un identifiant (extraction imdb / tmdb / tvdb). La lecture n'échoue
//!   jamais : le résultat porte un drapeau `valide`.
//! - Écriture au format Kodi (et variantes par [`Connecteur`]) : déclaration
//!   XML, indentation de 2 espaces, fins de ligne CRLF, balises non gérées
//!   d'un ancien NFO recopiées ([`DonneesConservees`]).
//! - Noms de fichiers NFO selon les réglages ([`noms`]), en fonctions pures.

pub mod arbre;
pub mod commun;
pub mod episode;
pub mod film;
pub mod noms;
pub mod outils;
pub mod saison;
pub mod serie;

pub use arbre::{Element, Noeud, decoder_octets};
pub use commun::{Connecteur, DonneesConservees, OptionsNfo, contenu_equivalent, ecrire_si_change};
pub use episode::{
    EpisodeLu, EpisodesLus, ecrire_nfo_episodes, lire_fichier_nfo_episodes, lire_nfo_episodes,
    lire_nfo_episodes_octets,
};
pub use film::{
    FilmLu, SagaLue, ecrire_nfo_film, lire_fichier_nfo_film, lire_nfo_film, lire_nfo_film_octets,
};
pub use noms::{
    NommageNfoEpisode, NommageNfoFilm, NommageNfoSaison, NommageNfoSerie, nom_nfo_episode,
    nom_nfo_saison, nom_nfo_serie, noms_nfo_film,
};
pub use outils::{horodatage_courant, ids_depuis_texte};
pub use saison::{
    SaisonLue, ecrire_nfo_saison, lire_fichier_nfo_saison, lire_nfo_saison, lire_nfo_saison_octets,
};
pub use serie::{
    SerieLue, ecrire_nfo_serie, lire_fichier_nfo_serie, lire_nfo_serie, lire_nfo_serie_octets,
};

#[cfg(test)]
mod tests;
