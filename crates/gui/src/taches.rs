//! Tâches d'arrière-plan vues par l'interface : la source de données les exécute et publie
//! leur cycle de vie (`TacheDemarree`, `TacheProgression`, `TacheTerminee`) par le canal de
//! l'état ; l'interface n'en garde qu'un instantané pour la barre d'état.

use std::sync::Arc;

/// Fonction de rafraîchissement de l'interface (clone de `egui::Context::request_repaint`).
pub type Repaint = Arc<dyn Fn() + Send + Sync>;

/// Issue d'une tâche terminée.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IssueTache {
    Reussie,
    Annulee,
    /// Message d'erreur lisible.
    Echouee(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tache {
    pub id: u64,
    /// Nom affiché (déjà traduit ou fourni par les services).
    pub nom: String,
    /// Étape en cours (« Analyse de … »).
    pub description: String,
    pub fait: u32,
    pub total: u32,
    /// Une tâche « principale » est annulable depuis la barre d'état (bouton Stop).
    pub principale: bool,
}

impl Tache {
    pub fn nouvelle(id: u64, nom: impl Into<String>, principale: bool) -> Self {
        Self {
            id,
            nom: nom.into(),
            description: String::new(),
            fait: 0,
            total: 0,
            principale,
        }
    }

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
}

/// Conversion saturée d'un compteur 64 bits vers l'affichage.
pub fn compteur(v: u64) -> u32 {
    u32::try_from(v).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_bornee_et_indeterminee() {
        let mut t = Tache::nouvelle(1, "x", true);
        assert!(t.indeterminee());
        assert_eq!(t.fraction(), 0.0);
        t.fait = 5;
        t.total = 4;
        assert_eq!(t.fraction(), 1.0);
        assert_eq!(compteur(u64::MAX), u32::MAX);
    }
}
