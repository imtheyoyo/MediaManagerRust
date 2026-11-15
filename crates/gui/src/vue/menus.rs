//! Rendu des menus déclaratifs (barre d'outils, menu contextuel).

use crate::commandes::Entree;
use crate::etat::{Etat, Message};
use eframe::egui::{self, Button};

pub fn afficher_entrees(
    ui: &mut egui::Ui,
    etat: &Etat,
    entrees: &[Entree],
    sortie: &mut Vec<Message>,
) {
    for entree in entrees {
        match entree {
            Entree::Separateur => {
                ui.separator();
            }
            Entree::Cmd(c) => {
                let mut bouton = Button::new(etat.i18n.t(c.cle()));
                if let Some(t) = c.raccourci_texte() {
                    bouton = bouton.shortcut_text(t);
                }
                if ui.add(bouton).clicked() {
                    sortie.push(Message::Commande(*c));
                    ui.close();
                }
            }
        }
    }
}
