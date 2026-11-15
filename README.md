# MediaManagerRust

Gestionnaire de médiathèque films et séries, écrit en Rust, avec interface graphique Windows.

## État du projet

| Crate | Rôle | État |
|---|---|---|
| `crates/core` | modèle, scan films/séries, NFO, renommeur | fait : scan films et séries, NFO, renommeur (plan + exécution sûre) |
| `crates/gabarit` | moteur de gabarits du renommeur (`${titre}`, conditions, boucles, nettoyage des noms) | fait (74 tests) |
| `crates/scrapers` | client HTTP avec cache, correspondance, fournisseur TMDB films | fait (32 tests) |
| `crates/store` | SQLite, réglages atomiques, mode portable, instance unique, sauvegardes | fait |
| `crates/gui` | interface egui : tableau, détails, thèmes, réglages, aperçu du renommage | coque faite (41 tests), données factices |
| `crates/cli` | outil en ligne de commande | à faire |

Prochaine étape : brancher l'interface et l'outil en ligne de commande sur le cœur, le stockage et TMDB.

## Clé TMDB

La récupération des métadonnées nécessite votre propre clé API TMDB, à saisir dans les réglages. Aucune clé n'est fournie.

## Compilation

```
cargo build --release
cargo test --workspace
```
