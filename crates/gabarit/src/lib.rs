//! Moteur de gabarits utilisé par le renommeur.
//!
//! Le crate évalue un gabarit texte contre un contexte clé → valeur, sans
//! dépendre du modèle de données du reste du projet. Il reprend la syntaxe du
//! moteur du programme d'origine (petit moteur de gabarits à base de `${…}`, avec extensions).
//!
//! # Syntaxe
//!
//! * Texte hors `${…}` : littéral.
//! * Variable : `${titre}`, propriété imbriquée `${film.titre}`.
//! * Accès : `${liste[0]}`, `${liste[last]}`, `${liste[1,2]}` (début, compte),
//!   `${texte[0]}` (1er caractère), `${texte[0,3]}` (sous-chaîne). Hors bornes
//!   ⇒ vide ; indice invalide ⇒ erreur de syntaxe.
//! * Préfixe/suffixe conditionnels : `${ - ,edition,}` (émis seulement si la
//!   valeur est non vide). Le préfixe s'arrête à la 1re virgule et le suffixe
//!   commence après la dernière ; les virgules des arguments de rendu restent
//!   donc possibles au milieu : `${(,titre;replace(a,b),)}`.
//! * Rendus nommés chaînables : `${expr;upper;replace(a,b)}` — `lower`,
//!   `upper`, `title`, `first`, `array(sep)`, `uniqueArray(sep)`, `split(i)` /
//!   `split(a,b)`, `replace(a,b)` / `replace(table.csv)`, `number(fmt)`,
//!   `date(fmt)`, `bitrate(unité)`, `filesize(unité)`, `framerate(round)`,
//!   `nfd`, `chain(r1;r2)` (alias : `lowerCase`, `upperCase`, `titleCase`,
//!   `firstCharacter`, `arrayUnique`, `replacement`).
//! * Conditions : `${if expr}…${else}…${end}`, `${if !expr}`,
//!   `${if expr=valeur}`, `${if expr!=valeur}`, `&&`, `||` (`&&` prioritaire).
//! * Boucles : `${foreach liste item[ séparateur]}…${end}` ; variables
//!   auxiliaires `index_item` (base 0), `first_item`, `last_item`.
//! * Annotation : `${@regexp "motif" source "gabarit"}`.
//!
//! # Utilisation
//!
//! ```
//! use mediamanager_gabarit::{Contexte, rendre};
//!
//! let ctx = Contexte::nouveau().avec("titre", "Matrix").avec("annee", 1999);
//! assert_eq!(rendre("${titre} (${annee})", &ctx).unwrap(), "Matrix (1999)");
//! ```

pub mod dates;
pub mod destination;
pub mod erreur;
pub mod evaluation;
pub mod format_nombre;
pub mod multi;
pub mod nom_fichier;
pub mod options;
pub mod rendus;
pub mod texte;
pub mod valeur;

mod syntaxe;

pub use dates::DateHeure;
pub use destination::{
    FormeDestination, OptionsDestination, contient_jeton, creer_destination,
    creer_destination_ou_brut, gabarit_dossier_unique, gabarit_fichier_valide,
    nettoyer_destination,
};
pub use erreur::{ErreurGabarit, Resultat};
pub use evaluation::{Gabarit, Jetons, rendre, rendre_ou_brut};
pub use multi::{
    EpisodeContexte, ListesJetonsEpisode, StyleMultiEpisodes, creer_destination_multi,
    rendre_multi_episodes,
};
pub use nom_fichier::{
    Plateforme, assainir_chemin_relatif, assainir_nom_fichier, assainir_nom_fichier_unix,
    assainir_nom_fichier_windows, en_ascii, equivalents_unicode, est_nom_reserve_windows,
    remplacer_caracteres_invalides, remplacer_espaces,
};
pub use options::{OptionsRendu, ResolveurTable, SubstitutionSeparateurs};
pub use rendus::parser_table_csv;
pub use valeur::{Contexte, Valeur};
