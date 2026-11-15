//! Dialogue de mise à jour du programme : recherche, « Nouvelle version disponible » avec les
//! notes de version, installation (progression) ou erreur. La logique est dans
//! `mise_a_jour` (réseau, archive, fichiers) et `etat` (enchaînement).

use super::Sortie;
use crate::etat::{Etat, EtatDialogueMiseAJour, Message, PhaseMiseAJour};
use crate::mise_a_jour::EtapeInstallation;
use crate::theme::{ORANGE, ROUGE};
use eframe::egui::{self, Align, Button, Layout, Modal, ProgressBar, RichText, ScrollArea};

pub fn afficher(ctx: &egui::Context, etat: &Etat, d: &EtatDialogueMiseAJour, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let installation = matches!(d.phase, PhaseMiseAJour::Installation(_));
    let reponse = Modal::new(egui::Id::unique("dlg_mise_a_jour")).show(ctx, |ui| {
        ui.set_width(560.0);
        let titre = match d.phase {
            PhaseMiseAJour::Disponible => i.t("gui.update.available"),
            _ => i.t("gui.update.title"),
        };
        ui.heading(titre);
        ui.separator();
        egui::Grid::new("g_versions")
            .num_columns(2)
            .spacing([14.0, 4.0])
            .show(ui, |ui| {
                ui.label(i.t("gui.update.current"));
                ui.label(etat.config_mise_a_jour.courant.libelle());
                ui.end_row();
                if let Some(p) = &d.publication {
                    ui.label(i.t("gui.update.latest"));
                    ui.label(p.libelle());
                    ui.end_row();
                }
            });
        ui.add_space(6.0);
        match &d.phase {
            PhaseMiseAJour::Recherche => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(i.t("gui.update.checking"));
                });
            }
            PhaseMiseAJour::AJour => {
                ui.label(i.t("gui.update.uptodate"));
            }
            PhaseMiseAJour::Developpement => {
                ui.label(RichText::new(i.t("gui.update.development")).color(ORANGE));
            }
            PhaseMiseAJour::Disponible => {}
            PhaseMiseAJour::Installation(etape) => {
                let (texte, part) = match etape {
                    EtapeInstallation::Telechargement { fait, total } => {
                        let mo = |o: u64| i.decimal(o as f64 / 1_048_576.0, 1);
                        let t = match total {
                            Some(t) => format!("{} / {} Mo", mo(*fait), mo(*t)),
                            None => format!("{} Mo", mo(*fait)),
                        };
                        let part = total
                            .filter(|t| *t > 0)
                            .map(|t| *fait as f32 / t as f32)
                            .unwrap_or(0.0);
                        (format!("{} {t}", i.t("gui.update.downloading")), part)
                    }
                    EtapeInstallation::Extraction => (i.t("gui.update.extracting"), 1.0),
                    EtapeInstallation::Remplacement => (i.t("gui.update.replacing"), 1.0),
                };
                ui.add(ProgressBar::new(part).text(texte));
            }
            PhaseMiseAJour::Erreur(e) => {
                ui.label(RichText::new(e).color(ROUGE));
            }
        }
        if let Some(p) = &d.publication
            && matches!(
                d.phase,
                PhaseMiseAJour::Disponible | PhaseMiseAJour::Developpement
            )
            && !p.notes.trim().is_empty()
        {
            ui.add_space(6.0);
            ui.label(RichText::new(i.t("gui.update.notes")).strong());
            egui::Frame::new()
                .fill(ui.visuals().extreme_bg_color)
                .inner_margin(6)
                .show(ui, |ui| {
                    ScrollArea::vertical()
                        .id_salt("notes_version")
                        .max_height(260.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.label(p.notes.trim());
                        });
                });
        }
        ui.separator();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add_enabled(!installation, Button::new(i.t("Button.close")))
                .clicked()
            {
                sortie.push(Message::FermerDialogue);
            }
            if d.phase == PhaseMiseAJour::Disponible {
                let possible = d
                    .publication
                    .as_ref()
                    .is_some_and(|p| p.url_archive.is_some());
                let bouton = ui
                    .add_enabled(possible, Button::new(i.t("gui.update.install")))
                    .on_disabled_hover_text(i.t("gui.update.noarchive"));
                if bouton.clicked() {
                    sortie.push(Message::InstallerMiseAJour);
                }
            }
        });
    });
    if reponse.should_close() && !installation {
        sortie.push(Message::FermerDialogue);
    }
}
