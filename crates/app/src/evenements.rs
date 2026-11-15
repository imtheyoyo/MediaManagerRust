//! Flux d'événements diffusés aux interfaces (GUI, CLI).
//!
//! Les événements passent par des canaux [`std::sync::mpsc`] : un code
//! synchrone (boucle egui) les lit avec `try_recv` à chaque image, une CLI
//! avec `recv`. Chaque abonné reçoit sa propre copie ; un abonné dont le
//! récepteur a été détruit est oublié à la diffusion suivante.

use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};

/// Identifiant d'une tâche (croissant, unique pour la session).
pub type IdTache = u64;

/// Gravité d'un message destiné à l'utilisateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NiveauMessage {
    Info,
    Avertissement,
    Erreur,
}

/// Issue d'une tâche terminée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueTache {
    Reussie,
    Annulee,
    /// Message d'erreur lisible.
    Echouee(String),
}

/// Événement publié par les services.
#[derive(Debug, Clone, PartialEq)]
pub enum Evenement {
    /// La tâche a commencé (après son attente dans la file principale).
    TacheDemarree {
        tache: IdTache,
        nom: String,
    },
    /// Avancement : `fait` sur `total` unités (`total` = 0 : indéterminé).
    Progression {
        tache: IdTache,
        fait: u64,
        total: u64,
        description: String,
    },
    /// Message à présenter à l'utilisateur (notification, journal visible).
    Message {
        tache: Option<IdTache>,
        niveau: NiveauMessage,
        texte: String,
    },
    FilmAjoute {
        id: String,
    },
    FilmModifie {
        id: String,
    },
    FilmSupprime {
        id: String,
    },
    SerieAjoutee {
        id: String,
    },
    SerieModifiee {
        id: String,
    },
    SerieSupprimee {
        id: String,
    },
    /// Fin de la tâche, quelle qu'en soit l'issue.
    TacheTerminee {
        tache: IdTache,
        nom: String,
        issue: IssueTache,
    },
}

/// Diffuseur d'événements à plusieurs abonnés.
#[derive(Debug, Default)]
pub struct Diffuseur {
    abonnes: Mutex<Vec<Sender<Evenement>>>,
}

impl Diffuseur {
    /// Nouvel abonné : reçoit tous les événements publiés à partir de maintenant.
    pub fn abonner(&self) -> Receiver<Evenement> {
        let (tx, rx) = channel();
        self.abonnes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(tx);
        rx
    }

    /// Publie un événement à tous les abonnés vivants.
    pub fn publier(&self, e: &Evenement) {
        let mut abonnes = self.abonnes.lock().unwrap_or_else(|e| e.into_inner());
        abonnes.retain(|tx| tx.send(e.clone()).is_ok());
    }
}
