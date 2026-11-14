# MediaManagerRust

Gestionnaire de médiathèque films et séries, écrit en Rust, avec interface graphique Windows.

## État du projet

Squelette en cours de construction. Fonctions visées :

- scan des dossiers de films et de séries ;
- récupération des métadonnées en ligne (TMDB, IMDb, TheTVDB…) ;
- lecture et écriture des fichiers NFO ;
- renommage et rangement par gabarits, avec aperçu ;
- interface graphique en français, thèmes clair et sombre.

## Organisation

| Crate | Rôle |
|---|---|
| `crates/core` | modèle de données, scan, NFO, renommage |
| `crates/scrapers` | métadonnées en ligne |
| `crates/store` | persistance de la bibliothèque et des réglages |
| `crates/cli` | outil en ligne de commande (`mediamanager`) |
| `crates/gui` | application graphique (`mediamanager-gui`) |

## Compilation

```
cargo build --release
```
