//! Dialogue de réglages : arbre filtrable des pages (gauche) + page courante (droite).
//! Pas de bouton « Appliquer » : chaque contrôle modifie immédiatement le réglage.

use super::Sortie;
use crate::etat::{ActionDialogue, Etat, EtatDialogueReglages, Message};
use crate::i18n::Langue;
use crate::modele::normaliser;
use crate::reglages::{Page, Reglages, Theme};
use crate::renommage::appliquer_modele;
use eframe::egui::{self, Align, Layout, Modal, RichText, ScrollArea, TextEdit, Vec2};

pub fn afficher(ctx: &egui::Context, etat: &Etat, d: &EtatDialogueReglages, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let mut r: Reglages = etat.reglages.clone();
    let reponse = Modal::new(egui::Id::unique("dlg_reglages")).show(ctx, |ui| {
        ui.set_min_size(Vec2::new(800.0, 560.0));
        ui.heading(i.t("gui.settings.title"));
        ui.separator();
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(200.0);
                let mut filtre = d.filtre.clone();
                if ui
                    .add(TextEdit::singleline(&mut filtre).hint_text(i.t("Settings.filter")))
                    .changed()
                {
                    sortie.push(Message::Dialogue(ActionDialogue::ReglagesFiltre(
                        filtre.clone(),
                    )));
                }
                let motif = normaliser(&filtre);
                ScrollArea::vertical()
                    .id_salt("arbre_reglages")
                    .max_height(470.0)
                    .show(ui, |ui| {
                        for (page, niveau, cle) in Page::ARBRE {
                            let libelle = i.t(cle);
                            if !motif.is_empty() && !normaliser(&libelle).contains(&motif) {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                ui.add_space(14.0 * *niveau as f32);
                                if ui.selectable_label(d.page == *page, libelle).clicked() {
                                    sortie.push(Message::Dialogue(ActionDialogue::ReglagesPage(
                                        *page,
                                    )));
                                }
                            });
                        }
                    });
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.set_min_width(560.0);
                ui.label(RichText::new(i.t(d.page.cle())).heading());
                ui.add_space(6.0);
                ScrollArea::vertical()
                    .id_salt("page_reglages")
                    .max_height(440.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        page(ui, etat, d.page, &mut r);
                    });
            });
        });
        ui.separator();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button(i.t("Button.close")).clicked() {
                sortie.push(Message::FermerDialogue);
            }
        });
    });
    if r != etat.reglages {
        sortie.push(Message::AppliquerReglages(Box::new(r)));
    }
    if reponse.should_close() {
        sortie.push(Message::FermerDialogue);
    }
}

fn page(ui: &mut egui::Ui, etat: &Etat, page: Page, r: &mut Reglages) {
    let i = &etat.i18n;
    match page {
        Page::General => {
            egui::Grid::new("g_general")
                .num_columns(2)
                .spacing([14.0, 8.0])
                .show(ui, |ui| {
                    ui.label(i.t("Settings.language"));
                    egui::ComboBox::from_id_salt("langue")
                        .selected_text(r.langue.nom_natif())
                        .show_ui(ui, |ui| {
                            for l in Langue::TOUTES {
                                ui.selectable_value(&mut r.langue, l, l.nom_natif());
                            }
                        });
                    ui.end_row();
                    ui.label(i.t("Settings.uitheme"));
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut r.theme,
                            Theme::Clair,
                            i.t("Settings.theme.light"),
                        );
                        ui.selectable_value(
                            &mut r.theme,
                            Theme::Sombre,
                            i.t("Settings.theme.dark"),
                        );
                    });
                    ui.end_row();
                });
            ui.add_space(8.0);
            ui.checkbox(&mut r.memoriser_module, i.t("Settings.rememberlastmodule"));
            ui.checkbox(&mut r.afficher_memoire, i.t("Settings.showmemory"));
            ui.add_space(8.0);
            ui.label(
                RichText::new(i.t("Settings.restartrequired"))
                    .weak()
                    .small(),
            );
        }
        Page::Systeme => {
            ui.label(RichText::new(i.t("Settings.proxy")).strong());
            egui::Grid::new("g_proxy")
                .num_columns(2)
                .spacing([14.0, 6.0])
                .show(ui, |ui| {
                    ui.label(i.t("Settings.proxy.host"));
                    ui.text_edit_singleline(&mut r.proxy_hote);
                    ui.end_row();
                    ui.label(i.t("Settings.proxy.port"));
                    ui.text_edit_singleline(&mut r.proxy_port);
                    ui.end_row();
                    ui.label(i.t("Settings.proxy.user"));
                    ui.text_edit_singleline(&mut r.proxy_utilisateur);
                    ui.end_row();
                    ui.label(i.t("Settings.proxy.password"));
                    ui.add(TextEdit::singleline(&mut r.proxy_mot_de_passe).password(true));
                    ui.end_row();
                });
            ui.add_space(8.0);
            ui.checkbox(&mut r.ignorer_erreurs_ssl, i.t("Settings.ignoresslerrors"));
            ui.horizontal(|ui| {
                ui.label(i.t("Settings.parallel"));
                ui.add(egui::DragValue::new(&mut r.telechargements_paralleles).range(1..=16));
            });
        }
        Page::Divers => {
            ui.checkbox(&mut r.cache_image, i.t("Settings.imagecache"));
            ui.checkbox(&mut r.corbeille, i.t("Settings.trash"));
        }
        Page::FilmsSource => {
            ui.label(RichText::new(i.t("Settings.folders")).strong());
            let mut a_retirer = None;
            for (n, dossier) in r.sources.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(dossier);
                    if ui
                        .small_button("✖")
                        .on_hover_text(i.t("Button.remove"))
                        .clicked()
                    {
                        a_retirer = Some(n);
                    }
                });
            }
            if let Some(n) = a_retirer {
                r.sources.remove(n);
            }
            if ui.button(format!("+ {}", i.t("Button.add"))).clicked() {
                let nouveau = format!(
                    "D:\\{} {}",
                    i.t("Settings.folders.new"),
                    r.sources.len() + 1
                );
                r.sources.push(nouveau);
            }
            ui.add_space(10.0);
            ui.label(RichText::new(i.t("Settings.exclusions")).strong());
            for e in &r.exclusions {
                ui.label(e);
            }
        }
        Page::FilmsRenommeur => {
            ui.horizontal(|ui| {
                ui.label(i.t("Settings.renamer.profile"));
                let nom = r
                    .profils
                    .get(r.profil_actif)
                    .map(|p| p.nom.clone())
                    .unwrap_or_default();
                egui::ComboBox::from_id_salt("profil_actif")
                    .selected_text(nom)
                    .show_ui(ui, |ui| {
                        for (n, p) in r.profils.clone().iter().enumerate() {
                            ui.selectable_value(&mut r.profil_actif, n, &p.nom);
                        }
                    });
            });
            ui.add_space(6.0);
            let actif = r.profil_actif;
            if let Some(p) = r.profils.get_mut(actif) {
                egui::Grid::new("g_renamer")
                    .num_columns(2)
                    .spacing([14.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(i.t("Settings.renamer.folder"));
                        ui.add(TextEdit::singleline(&mut p.modele_dossier).desired_width(300.0));
                        ui.end_row();
                        ui.label(i.t("Settings.renamer.file"));
                        ui.add(TextEdit::singleline(&mut p.modele_fichier).desired_width(300.0));
                        ui.end_row();
                    });
                if let Some(exemple) = etat.films.first() {
                    ui.add_space(8.0);
                    ui.label(RichText::new(i.t("Settings.renamer.example")).strong());
                    ui.label(format!(
                        "{} \\ {}",
                        appliquer_modele(&p.modele_dossier, exemple),
                        appliquer_modele(&p.modele_fichier, exemple)
                    ));
                }
            }
            ui.add_space(8.0);
            ui.checkbox(&mut r.renommer_dossiers, i.t("Settings.renamer.dofolders"));
            ui.checkbox(&mut r.renommer_fichiers, i.t("Settings.renamer.dofiles"));
        }
        _ => {
            ui.label(RichText::new(i.t("Settings.todo")).weak());
        }
    }
}
