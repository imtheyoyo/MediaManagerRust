//! Traits génériques des fournisseurs de métadonnées de films.

use async_trait::async_trait;

use crate::erreur::Resultat;
use crate::types::{
    Artwork, BandeAnnonce, CollectionFilm, MetadonneesFilm, OptionsRecherche, ResultatRecherche,
    TypeArtwork,
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
