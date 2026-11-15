//! Dialogue de réglages : arbre filtrable des pages (gauche) + page courante (droite).
//! Pas de bouton « Appliquer » : chaque contrôle modifie immédiatement le réglage ; les
//! réglages des services sont enregistrés à la fermeture du dialogue.

use super::Sortie;
use crate::etat::{ActionDialogue, Etat, EtatDialogueReglages, Message};
use crate::i18n::Langue;
use crate::modele::normaliser;
use crate::reglages::{Page, Reglages, Theme};
use crate::source_donnees::ReglagesServices;
use eframe::egui::{self, Align, Layout, Modal, RichText, ScrollArea, TextEdit, Vec2};

pub fn afficher(ctx: &egui::Context, etat: &Etat, d: &EtatDialogueReglages, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let mut r: Reglages = etat.reglages.clone();
    let mut s: ReglagesServices = etat.services.clone();
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
                        page(ui, etat, d, &mut r, &mut s, sortie);
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
    if s != etat.services {
        sortie.push(Message::AppliquerServices(Box::new(s)));
    }
    if reponse.should_close() {
        sortie.push(Message::FermerDialogue);
    }
}

/// Liste de dossiers éditable : retrait par ✖, ajout par saisie.
fn liste_dossiers(
    ui: &mut egui::Ui,
    etat: &Etat,
    dossiers: &mut Vec<String>,
    saisie: &str,
    message: fn(String) -> ActionDialogue,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    let mut a_retirer = None;
    for (n, dossier) in dossiers.iter().enumerate() {
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
        dossiers.remove(n);
    }
    ui.horizontal(|ui| {
        let mut texte = saisie.to_string();
        let champ = ui.add(
            TextEdit::singleline(&mut texte)
                .hint_text(i.t("wizard.sources.hint"))
                .desired_width(320.0),
        );
        if champ.changed() {
            sortie.push(Message::Dialogue(message(texte.clone())));
        }
        let valide = champ.lost_focus() && ui.input(|e| e.key_pressed(egui::Key::Enter));
        let nouveau = texte.trim().to_string();
        if (ui.button(format!("+ {}", i.t("Button.add"))).clicked() || valide)
            && !nouveau.is_empty()
            && !dossiers.contains(&nouveau)
        {
            dossiers.push(nouveau);
            sortie.push(Message::Dialogue(message(String::new())));
        }
    });
}

fn page(
    ui: &mut egui::Ui,
    etat: &Etat,
    d: &EtatDialogueReglages,
    r: &mut Reglages,
    s: &mut ReglagesServices,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    match d.page {
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
                    ui.text_edit_singleline(&mut s.proxy_hote);
                    ui.end_row();
                    ui.label(i.t("Settings.proxy.port"));
                    ui.text_edit_singleline(&mut s.proxy_port);
                    ui.end_row();
                    ui.label(i.t("Settings.proxy.user"));
                    ui.text_edit_singleline(&mut s.proxy_utilisateur);
                    ui.end_row();
                    ui.label(i.t("Settings.proxy.password"));
                    ui.add(TextEdit::singleline(&mut s.proxy_mot_de_passe).password(true));
                    ui.end_row();
                });
            ui.add_space(8.0);
            ui.checkbox(&mut s.ignorer_erreurs_ssl, i.t("Settings.ignoresslerrors"));
            ui.horizontal(|ui| {
                ui.label(i.t("Settings.parallel"));
                ui.add(egui::DragValue::new(&mut s.threads_telechargement).range(1..=16));
            });
        }
        Page::ServicesExternes => {
            ui.label(RichText::new(i.t("wizard.scraper.apikey")).strong());
            ui.horizontal(|ui| {
                let mut cle = d.saisie_cle.clone();
                if ui
                    .add(
                        TextEdit::singleline(&mut cle)
                            .password(true)
                            .desired_width(320.0),
                    )
                    .changed()
                {
                    sortie.push(Message::Dialogue(ActionDialogue::ReglagesSaisieCle(cle)));
                }
                if ui
                    .add_enabled(
                        !d.saisie_cle.trim().is_empty(),
                        egui::Button::new(i.t("Button.save")),
                    )
                    .clicked()
                {
                    sortie.push(Message::Dialogue(ActionDialogue::ReglagesEnregistrerCle));
                }
            });
            ui.label(
                RichText::new(i.t("wizard.scraper.apikey.note"))
                    .weak()
                    .small(),
            );
        }
        Page::Divers => {
            ui.checkbox(&mut s.cache_images, i.t("Settings.imagecache"));
            ui.checkbox(&mut s.corbeille, i.t("Settings.trash"));
        }
        Page::FilmsSource => {
            ui.label(RichText::new(i.t("Settings.folders")).strong());
            liste_dossiers(
                ui,
                etat,
                &mut s.sources_films,
                &d.saisie_source,
                ActionDialogue::ReglagesSaisieSource,
                sortie,
            );
            ui.add_space(10.0);
            ui.label(RichText::new(i.t("Settings.exclusions")).strong());
            let mut texte = s.dossiers_ignores.join(", ");
            if ui
                .add(TextEdit::singleline(&mut texte).desired_width(420.0))
                .changed()
            {
                s.dossiers_ignores = texte
                    .split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect();
            }
        }
        Page::FilmsExtracteur => {
            ui.horizontal(|ui| {
                ui.label(i.t("wizard.scraper.language"));
                ui.add(TextEdit::singleline(&mut s.langue_scraping).desired_width(60.0));
            });
            ui.checkbox(
                &mut s.renommer_apres_scraping,
                i.t("Settings.renamer.afterscrape"),
            );
        }
        Page::FilmsRenommeur => {
            egui::Grid::new("g_renamer")
                .num_columns(2)
                .spacing([14.0, 6.0])
                .show(ui, |ui| {
                    ui.label(i.t("Settings.renamer.folder"));
                    ui.add(TextEdit::singleline(&mut s.gabarit_dossier_film).desired_width(380.0));
                    ui.end_row();
                    ui.label(i.t("Settings.renamer.file"));
                    ui.add(TextEdit::singleline(&mut s.gabarit_fichier_film).desired_width(380.0));
                    ui.end_row();
                    ui.label(i.t("Settings.renamer.replacement"));
                    ui.add(TextEdit::singleline(&mut s.caractere_remplacement).desired_width(40.0));
                    ui.end_row();
                });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.checkbox(&mut s.remplacer_espaces, i.t("Settings.renamer.spaces"));
                ui.add_enabled(
                    s.remplacer_espaces,
                    TextEdit::singleline(&mut s.caractere_espace).desired_width(30.0),
                );
            });
            ui.checkbox(&mut s.ascii_seulement, i.t("Settings.renamer.ascii"));
            ui.checkbox(
                &mut s.supprimer_dossiers_vides,
                i.t("Settings.renamer.cleanup"),
            );
            ui.add_space(8.0);
            ui.label(RichText::new(i.t("Settings.renamer.hint")).weak().small());
        }
        Page::Series => {
            ui.label(RichText::new(i.t("Settings.folders")).strong());
            liste_dossiers(
                ui,
                etat,
                &mut s.sources_series,
                &d.saisie_source_serie,
                ActionDialogue::ReglagesSaisieSourceSerie,
                sortie,
            );
        }
        _ => {
            ui.label(RichText::new(i.t("Settings.todo")).weak());
        }
    }
}
