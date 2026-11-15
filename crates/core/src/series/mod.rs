//! Scan des sources de séries et détection saison/épisode.
//!
//! * [`detection`] : saison / épisode(s) / date depuis un nom de fichier ou un
//!   chemin relatif (fonction pure [`detecter_episodes`]) ;
//! * [`scan`] : parcours d'une source de séries et construction des
//!   [`Serie`](crate::modele::Serie) / [`Saison`](crate::modele::Saison) /
//!   [`Episode`](crate::modele::Episode) avec leurs fichiers associés ;
//! * [`correspondance`] : appariement épisodes locaux ↔ épisodes scrapés,
//!   choix de l'ordre (diffusion / DVD / absolu), épisodes manquants ;
//! * [`organisation`] : saisons dérivées, tri, changement d'ordre actif.

pub mod correspondance;
pub mod detection;
mod motifs;
pub mod organisation;
pub mod scan;

pub use correspondance::{
    Correspondance, EpisodeScrape, ReglagesFictifs, apparier_episodes, appliquer_scrape_episode,
    choisir_episode_manuel, choisir_meilleur_groupe, creer_episodes_fictifs, episodes_a_afficher,
    episodes_manquants, mettre_a_jour_depuis_scrape, similarite,
};
pub use detection::{
    ETENDUE_MAX_PLAGE, ResultatDetection, decoder_romain, detecter_episodes,
    detecter_saison_fichier_dossier, est_dossier_saison, nettoyer_titre_episode,
};
pub use organisation::{definir_groupe_actif, recalculer_saisons, trier_episodes};
pub use scan::{
    AvertissementSeries, ResultatScanSeries, scanner_dossier_serie, scanner_source_series,
};

#[cfg(test)]
mod tests;
