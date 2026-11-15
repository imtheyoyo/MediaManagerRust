//! Traits génériques des fournisseurs de métadonnées (films et séries).

use async_trait::async_trait;

use crate::erreur::Resultat;
use crate::types::{
    Artwork, ArtworkEtendu, BandeAnnonce, CollectionFilm, MetadonneesEpisode, MetadonneesFilm,
    MetadonneesSerie, OptionsEpisode, OptionsRecherche, ResultatRecherche, TypeArtwork,
    TypeArtworkEtendu,
};

/// Recherche et détail d'un film.
#[async_trait]
pub trait FournisseurFilm: Send + Sync {
    /// Identifiant court du fournisseur (`tmdb`...).
    fn identifiant(&self) -> &'static str;
    /// Recherche ; résultats triés par score décroissant.
    async fn rechercher(&self, options: &OptionsRecherche) -> Resultat<Vec<ResultatRecherche>>;
    /// Métadonnées complètes (identifiant `tmdb` ou `imdb` requis dans `options.ids`).
    async fn metadonnees(&self, options: &OptionsRecherche) -> Resultat<MetadonneesFilm>;
}

/// Fournisseur d'images.
#[async_trait]
pub trait FournisseurArtwork: Send + Sync {
    /// Images d'un film ; `type_filtre` = `None` pour tous les types.
    async fn artwork(
        &self,
        options: &OptionsRecherche,
        type_filtre: Option<TypeArtwork>,
    ) -> Resultat<Vec<Artwork>>;
}

/// Fournisseur de bandes-annonces.
#[async_trait]
pub trait FournisseurBandesAnnonces: Send + Sync {
    async fn bandes_annonces(&self, options: &OptionsRecherche) -> Resultat<Vec<BandeAnnonce>>;
}

/// Fournisseur de collections (sagas).
#[async_trait]
pub trait FournisseurCollection: Send + Sync {
    /// Recherche de collections par texte.
    async fn rechercher_collections(
        &self,
        options: &OptionsRecherche,
    ) -> Resultat<Vec<ResultatRecherche>>;
    /// Détail d'une collection (identifiant `tmdbSet` dans `options.ids`).
    async fn collection(&self, options: &OptionsRecherche) -> Resultat<CollectionFilm>;
}

/// Recherche et détail d'une série et de ses épisodes.
#[async_trait]
pub trait FournisseurSerie: Send + Sync {
    /// Identifiant court du fournisseur (`tmdb`, `tvmaze`...).
    fn identifiant(&self) -> &'static str;
    /// Recherche ; résultats triés par score décroissant.
    async fn rechercher(&self, options: &OptionsRecherche) -> Resultat<Vec<ResultatRecherche>>;
    /// Métadonnées de la série (avec saisons et groupes d'épisodes).
    async fn metadonnees_serie(&self, options: &OptionsRecherche) -> Resultat<MetadonneesSerie>;
    /// Liste complète des épisodes, numérotés dans tous les groupes connus.
    async fn liste_episodes(&self, options: &OptionsRecherche)
    -> Resultat<Vec<MetadonneesEpisode>>;
    /// Détail d'un épisode, apparié par identifiant, numéro, titre ou date.
    async fn metadonnees_episode(&self, options: &OptionsEpisode) -> Resultat<MetadonneesEpisode>;
}

/// Fournisseur d'images de séries (série, saisons, épisodes).
#[async_trait]
pub trait FournisseurArtworkSerie: Send + Sync {
    /// Images de la série et de ses saisons ; `type_filtre` = `None` pour tout.
    async fn artwork_serie(
        &self,
        options: &OptionsRecherche,
        type_filtre: Option<TypeArtworkEtendu>,
    ) -> Resultat<Vec<ArtworkEtendu>>;
    /// Images d'un épisode (aucune par défaut).
    async fn artwork_episode(&self, _options: &OptionsEpisode) -> Resultat<Vec<ArtworkEtendu>> {
        Ok(Vec::new())
    }
}
