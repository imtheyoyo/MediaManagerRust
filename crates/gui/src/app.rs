//! Application eframe : coque de l'interface (barres, panneaux, dialogues, raccourcis, thèmes).

use crate::commandes::Commande;
use crate::etat::{Dialogue, Etat, Message, Module};
use crate::reglages::Theme;
use crate::theme;
use crate::vue::{self, Sortie, barre_etat, barre_modules, barre_outils, details, tableau};
use eframe::egui::{self, CentralPanel, Event, Frame, Id, Key, Margin, Panel};
use std::sync::Arc;
use std::time::Instant;

pub struct Application {
    etat: Etat,
    theme_applique: Option<Theme>,
}

impl Application {
    pub fn nouvelle(cc: &eframe::CreationContext<'_>) -> Self {
        let mut etat = Etat::nouveau();
        let ctx = cc.egui_ctx.clone();
        etat.definir_repaint(Arc::new(move || ctx.request_repaint()));
        Self {
            etat,
            theme_applique: None,
        }
    }

    fn appliquer_theme(&mut self, ctx: &egui::Context) {
        let theme_voulu = self.etat.reglages.theme;
        if self.theme_applique != Some(theme_voulu) {
            ctx.set_visuals(theme::visuals(theme_voulu));
            self.theme_applique = Some(theme_voulu);
        }
    }

    /// Raccourcis clavier globaux ; ignorés tant qu'un dialogue modal est affiché.
    fn raccourcis(&self, ctx: &egui::Context, sortie: &mut Sortie) {
        if self.etat.dialogue.is_some() {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::F)) {
            ctx.memory_mut(|m| m.request_focus(Id::unique(tableau::ID_CHAMP_RECHERCHE)));
        }
        // Les combinaisons les plus spécifiques (Ctrl+Maj) d'abord.
        for commande in Commande::TOUTES {
            if let Some(raccourci) = commande.raccourci_egui() {
                let libre = ctx.egui_wants_keyboard_input() && raccourci.modifiers.is_none();
                if !libre && ctx.input_mut(|i| i.consume_shortcut(&raccourci)) {
                    sortie.push(Message::Commande(commande));
                }
            }
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (haut, bas, page_haut, page_bas, debut, fin, entree, maj, tout, textes) =
            ctx.input(|i| {
                let textes: Vec<String> = i
                    .events
                    .iter()
                    .filter_map(|e| match e {
                        Event::Text(t) if !t.trim().is_empty() => Some(t.clone()),
                        _ => None,
                    })
                    .collect();
                (
                    i.key_pressed(Key::ArrowUp),
                    i.key_pressed(Key::ArrowDown),
                    i.key_pressed(Key::PageUp),
                    i.key_pressed(Key::PageDown),
                    i.key_pressed(Key::Home),
                    i.key_pressed(Key::End),
                    i.key_pressed(Key::Enter),
                    i.modifiers.shift,
                    i.modifiers.command && i.key_pressed(Key::A),
                    textes,
                )
            });
        let deplacements = [
            (haut, -1),
            (bas, 1),
            (page_haut, -15),
            (page_bas, 15),
            (debut, i32::MIN / 2),
            (fin, i32::MAX / 2),
        ];
        for (actif, delta) in deplacements {
            if actif {
                sortie.push(Message::Deplacer {
                    delta,
                    etendre: maj,
                });
            }
        }
        if tout {
            sortie.push(Message::SelectionnerTout);
        }
        if entree {
            sortie.push(Message::Commande(Commande::Editer));
        }
        for texte in textes {
            sortie.push(Message::Frappe {
                texte,
                instant: Instant::now(),
            });
        }
    }
}

impl eframe::App for Application {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.etat.drainer();
        self.appliquer_theme(&ctx);
        let mut sortie: Sortie = Vec::new();
        self.raccourcis(&ctx, &mut sortie);

        barre_outils::afficher(ui, &self.etat, &mut sortie);
        barre_etat::afficher(ui, &self.etat, &mut sortie);
        barre_modules::afficher(ui, &self.etat, &mut sortie);

        let fond = theme::fond_alterne(self.etat.reglages.theme);
        CentralPanel::default()
            .frame(Frame::new().fill(fond).inner_margin(Margin::same(6)))
            .show(ui, |ui| match self.etat.module {
                Module::Films => {
                    let largeur = ui.available_width();
                    Panel::right("panneau_details")
                        .resizable(true)
                        .default_size(largeur * 0.5)
                        .size_range(300.0..=largeur * 0.8)
                        .frame(
                            Frame::new()
                                .fill(ui.visuals().panel_fill)
                                .inner_margin(Margin::same(8)),
                        )
                        .show(ui, |ui| details::afficher(ui, &self.etat, &mut sortie));
                    CentralPanel::default()
                        .frame(Frame::new().inner_margin(Margin::symmetric(4, 0)))
                        .show(ui, |ui| tableau::afficher(ui, &self.etat, &mut sortie));
                }
                autre => {
                    let nom = self.etat.i18n.t(autre.cle());
                    ui.add_space(24.0);
                    ui.label(
                        self.etat
                            .i18n
                            .t_args("gui.module.placeholder", &[("module", nom.as_str())]),
                    );
                }
            });

        match &self.etat.dialogue {
            Some(Dialogue::ApercuRenommage(d)) => {
                vue::dialogue_renommage::afficher(&ctx, &self.etat, d, &mut sortie)
            }
            Some(Dialogue::Reglages(d)) => {
                vue::dialogue_reglages::afficher(&ctx, &self.etat, d, &mut sortie)
            }
            Some(Dialogue::Messages) => {
                vue::dialogue_messages::afficher(&ctx, &self.etat, &mut sortie)
            }
            None => {}
        }

        self.etat.scroll_vers = None;
        for message in sortie {
            self.etat.appliquer(message);
        }
    }
}
