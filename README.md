# MediaManagerRust

Gestionnaire de médiathèque films et séries, écrit en Rust, avec interface graphique Windows.

## État du projet

| Crate | Rôle | État |
|---|---|---|
| `crates/core` | modèle de données, scan des dossiers de films | fait (73 tests) — NFO et renommage à venir |
| `crates/gabarit` | moteur de gabarits du renommeur (`${titre}`, conditions, boucles, nettoyage des noms) | fait (74 tests) |
| `crates/scrapers` | client HTTP avec cache, correspondance, fournisseur TMDB films | fait (32 tests) |
| `crates/store` | persistance (SQLite prévu) | à faire |
| `crates/gui` | interface egui : tableau, détails, thèmes, réglages, aperçu du renommage | coque faite (41 tests), données factices |
| `crates/cli` | outil en ligne de commande | à faire |

Prochaines étapes : lecture/écriture NFO, renommeur films, stockage SQLite, branchement de l'interface sur le cœur.

## Clé TMDB

La récupération des métadonnées nécessite votre propre clé API TMDB, à saisir dans les réglages. Aucune clé n'est fournie.

## Compilation

```
cargo build --release
cargo test --workspace
```
