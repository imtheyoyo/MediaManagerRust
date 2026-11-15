//! Tâches d'arrière-plan : une tâche = un thread qui envoie des `Message` par un canal `mpsc`
//! et demande un rafraîchissement de l'interface. Annulation coopérative.

use crate::etat::Message;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Fonction de rafraîchissement de l'interface (clone de `egui::Context::request_repaint`).
pub type Repaint = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Debug)]
pub struct Tache {
    pub id: u64,
    pub cle_nom: &'static str,
    pub fait: u32,
    pub total: u32,
    /// Une tâche « principale » est annulable depuis la barre d'état (bouton Stop).
    pub principale: bool,
    pub annulation: Arc<AtomicBool>,
}

impl Tache {
    pub fn indeterminee(&self) -> bool {
        self.total <= 1
    }

    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.fait as f32 / self.total as f32).clamp(0.0, 1.0)
        }
    }

    pub fn annuler(&self) {
        self.annulation.store(true, Ordering::Relaxed);
    }
}

/// Tâche simulée (remplacée plus tard par les vraies tâches : scan, scrape, renommage...).
pub fn lancer_simulation(
    id: u64,
    total: u32,
    pas: Duration,
    tx: Sender<Message>,
    repaint: Repaint,
    annulation: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        for fait in 1..=total {
            thread::sleep(pas);
            if annulation.load(Ordering::Relaxed) {
                let _ = tx.send(Message::TacheTerminee { id, annulee: true });
                repaint();
                return;
            }
            if tx
                .send(Message::TacheProgression { id, fait, total })
                .is_err()
            {
                return;
            }
            repaint();
        }
        let _ = tx.send(Message::TacheTerminee { id, annulee: false });
        repaint();
    })
}
