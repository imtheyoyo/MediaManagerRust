//! Vues egui : elles lisent un `&Etat` et émettent des `Message` (aucune logique métier).

pub mod barre_etat;
pub mod barre_modules;
pub mod barre_outils;
pub mod details;
pub mod dialogue_messages;
pub mod dialogue_reglages;
pub mod dialogue_renommage;
pub mod menus;
pub mod tableau;

use crate::etat::Message;

/// File des messages émis pendant le dessin, appliqués après la frame.
pub type Sortie = Vec<Message>;
