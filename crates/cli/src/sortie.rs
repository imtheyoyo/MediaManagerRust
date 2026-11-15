//! Affichage : messages, barre de progression, tableaux.

use std::time::Duration;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use mediamanager_app::{ErreurApp, Evenement, IssueTache, NiveauMessage, PoigneeTache};

/// Niveau de détail de l'affichage.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sortie {
    pub verbeux: u8,
    pub silencieux: bool,
}

impl Sortie {
    /// Information (masquée en mode silencieux).
    pub fn info(&self, texte: impl AsRef<str>) {
        if !self.silencieux {
            println!("{}", texte.as_ref());
        }
    }

    /// Détail (affiché seulement avec -v).
    pub fn detail(&self, texte: impl AsRef<str>) {
        if self.verbeux > 0 {
            println!("{}", texte.as_ref());
        }
    }

    /// Avertissement sur la sortie d'erreur (masqué en mode silencieux).
    pub fn avertissement(&self, texte: impl AsRef<str>) {
        if !self.silencieux {
            eprintln!("avertissement : {}", texte.as_ref());
        }
    }

    fn nouvelle_barre(&self) -> ProgressBar {
        if self.silencieux {
            return ProgressBar::hidden();
        }
        // Cible stderr : masquée automatiquement hors terminal.
        let barre = ProgressBar::with_draw_target(None, ProgressDrawTarget::stderr());
        barre.set_style(style_indetermine());
        barre.enable_steady_tick(Duration::from_millis(120));
        barre
    }

    fn message_tache(&self, barre: &ProgressBar, niveau: NiveauMessage, texte: &str) {
        let afficher = match niveau {
            NiveauMessage::Erreur => true,
            NiveauMessage::Avertissement => !self.silencieux,
            NiveauMessage::Info => self.verbeux > 0,
        };
        if !afficher {
            return;
        }
        let prefixe = match niveau {
            NiveauMessage::Erreur => "erreur : ",
            NiveauMessage::Avertissement => "avertissement : ",
            NiveauMessage::Info => "",
        };
        barre.suspend(|| eprintln!("{prefixe}{texte}"));
    }

    /// Suit une tâche jusqu'à sa fin (progression, messages) et renvoie son
    /// résultat.
    pub fn suivre<T>(&self, tache: PoigneeTache<T>) -> Result<T, ErreurApp> {
        let barre = self.nouvelle_barre();
        let mut avec_longueur = false;
        // Dernier message d'erreur retenu : il double souvent l'erreur finale
        // de la tâche, affichée par ailleurs.
        let mut erreur_en_attente: Option<String> = None;
        for evt in tache.evenements.iter() {
            if let Evenement::TacheTerminee { issue, .. } = &evt {
                if let Some(m) = erreur_en_attente.take() {
                    let doublon =
                        matches!(issue, IssueTache::Echouee(e) if m.ends_with(e.as_str()));
                    if !doublon {
                        self.message_tache(&barre, NiveauMessage::Erreur, &m);
                    }
                }
                break;
            }
            if let Some(m) = erreur_en_attente.take() {
                self.message_tache(&barre, NiveauMessage::Erreur, &m);
            }
            match evt {
                Evenement::Progression {
                    fait,
                    total,
                    description,
                    ..
                } => {
                    if total > 0 {
                        if !avec_longueur {
                            barre.set_style(style_determine());
                            avec_longueur = true;
                        }
                        barre.set_length(total);
                        barre.set_position(fait.min(total));
                    }
                    barre.set_message(description);
                }
                Evenement::Message {
                    niveau: NiveauMessage::Erreur,
                    texte,
                    ..
                } => erreur_en_attente = Some(texte),
                Evenement::Message { niveau, texte, .. } => {
                    self.message_tache(&barre, niveau, &texte)
                }
                _ => {}
            }
        }
        if let Some(m) = erreur_en_attente.take() {
            self.message_tache(&barre, NiveauMessage::Erreur, &m);
        }
        barre.finish_and_clear();
        tache.attendre()
    }
}

fn style_indetermine() -> ProgressStyle {
    ProgressStyle::with_template("{spinner} {wide_msg}")
        .unwrap_or_else(|_| ProgressStyle::default_spinner())
}

fn style_determine() -> ProgressStyle {
    ProgressStyle::with_template("{spinner} [{bar:30}] {pos}/{len} {wide_msg}")
        .map(|s| s.progress_chars("=> "))
        .unwrap_or_else(|_| ProgressStyle::default_bar())
}

/// Largeur affichée approximative (en caractères).
fn largeur(s: &str) -> usize {
    s.chars().count()
}

/// Tronque à `max` caractères (avec « … »).
pub fn tronquer(s: &str, max: usize) -> String {
    if largeur(s) <= max {
        s.to_string()
    } else {
        let mut r: String = s.chars().take(max.saturating_sub(1)).collect();
        r.push('…');
        r
    }
}

/// Met en forme un tableau aligné (la dernière colonne n'est pas complétée).
pub fn tableau(entetes: &[&str], lignes: &[Vec<String>]) -> String {
    let n = entetes.len();
    let mut largeurs: Vec<usize> = entetes.iter().map(|e| largeur(e)).collect();
    for l in lignes {
        for (i, c) in l.iter().enumerate().take(n) {
            largeurs[i] = largeurs[i].max(largeur(c));
        }
    }
    let mut sortie = String::new();
    let mut ecrire = |cellules: &mut dyn Iterator<Item = &str>| {
        let mut ligne = String::new();
        for (i, c) in cellules.enumerate() {
            if i > 0 {
                ligne.push_str("  ");
            }
            ligne.push_str(c);
            if i + 1 < n {
                ligne.push_str(&" ".repeat(largeurs[i].saturating_sub(largeur(c))));
            }
        }
        sortie.push_str(ligne.trim_end());
        sortie.push('\n');
    };
    ecrire(&mut entetes.iter().copied());
    let soulignement: Vec<String> = largeurs.iter().map(|l| "-".repeat(*l)).collect();
    ecrire(&mut soulignement.iter().map(String::as_str));
    for l in lignes {
        ecrire(&mut l.iter().map(String::as_str));
    }
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tableau_aligne() {
        let t = tableau(
            &["Titre", "Année"],
            &[
                vec!["Élan".into(), "2010".into()],
                vec!["Un long titre".into(), "".into()],
            ],
        );
        let lignes: Vec<&str> = t.lines().collect();
        assert_eq!(lignes[0], "Titre          Année");
        assert_eq!(lignes[1], "-------------  -----");
        assert_eq!(lignes[2], "Élan           2010");
        assert_eq!(lignes[3], "Un long titre");
    }

    #[test]
    fn troncature() {
        assert_eq!(tronquer("abcdef", 4), "abc…");
        assert_eq!(tronquer("abc", 4), "abc");
    }
}
