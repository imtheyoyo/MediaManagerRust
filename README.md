# MediaManagerRust

Gestionnaire de médiathèque films et séries, écrit en Rust, avec interface graphique Windows.

## État du projet

| Crate | Rôle |
|---|---|
| `crates/core` | modèle, scan films/séries, NFO, renommeur (plan + exécution sûre) |
| `crates/gabarit` | moteur de gabarits du renommeur |
| `crates/scrapers` | client HTTP avec cache ; TMDB films et séries, Fanart.tv, TVmaze |
| `crates/store` | SQLite, réglages atomiques, secrets dans le trousseau, mode portable, sauvegardes |
| `crates/app` | services : mise à jour des sources, récupération des métadonnées, images, NFO, renommage, tâches |
| `crates/gui` | application graphique `mediamanager-gui` (egui) |
| `crates/cli` | outil en ligne de commande `mediamanager` |

Pas encore fait : récupération et renommage des séries, collections, analyse des flux (MediaInfo/ffprobe), export.

## Utilisation

- Interface : `mediamanager-gui` (ou `mediamanager-gui --demo` pour des données de démonstration).
- Ligne de commande : `mediamanager --help` (`maj`, `lister`, `scraper`, `nfo`, `renommer --appliquer`, `cle-tmdb`, `info`).
- Mode portable par défaut : les données sont créées à côté de l'exécutable. Un fichier `non-portable` à côté de l'exécutable utilise les dossiers utilisateur.

## Clé TMDB

La récupération des métadonnées nécessite votre propre clé API TMDB, à saisir dans les réglages. Aucune clé n'est fournie.

## Compilation

```
cargo build --release
cargo test --workspace
```
