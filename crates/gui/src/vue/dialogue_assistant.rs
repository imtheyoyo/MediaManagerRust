//! Assistant de premier lancement : accueil, interface (langue, thème), sources de films,
//! extracteur (clé d'API saisie dans un champ masqué, transmise au coffre à secrets).

use super::Sortie;
use crate::dialogues::{ActionAssistant, EtatAssistant, PageAssistant};
use crate::etat::{ActionDialogue, Etat, Message};
use crate::i18n::Langue;
use crate::polices;
use crate::reglages::Theme;
use crate::selecteur::CibleDossier;
use crate::theme::{ORANGE, accent};
use eframe::egui::{self, Align, Button, Key, Layout, Modal, RichText, TextEdit, Vec2};

fn envoyer(sortie: &mut Sortie, a: ActionAssistant) {
    sortie.push(Message::Dialogue(ActionDialogue::Assistant(a)));
}

pub fn afficher(ctx: &egui::Context, etat: &Etat, a: &EtatAssistant, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let reponse = Modal::new(egui::Id::unique("dlg_assistant")).show(ctx, |ui| {
        ui.set_min_size(Vec2::new(760.0, 480.0));
        ui.heading(i.t("app.welcome"));
        // Fil d'Ariane des pages
        ui.horizontal(|ui| {
            for (n, p) in PageAssistant::TOUTES.iter().enumerate() {
                let t = RichText::new(format!("{}. {}", n + 1, i.t(p.cle())));
                ui.label(if *p == a.page {
                    t.strong().color(accent(etat.reglages.theme))
                } else {
                    t.weak()
                });
            }
        });
        ui.separator();
        ui.allocate_ui(Vec2::new(ui.available_width(), 340.0), |ui| {
            ui.set_min_height(340.0);
            match a.page {
                PageAssistant::Accueil => {
                    ui.label(i.t("wizard.welcome.text"));
                }
                PageAssistant::Interface => {
                    egui::Grid::new("as_ui")
                        .num_columns(2)
                        .spacing([14.0, 10.0])
                        .show(ui, |ui| {
                            ui.label(i.t("Settings.language"));
                            ui.horizontal(|ui| {
                                for l in Langue::TOUTES {
                                    if ui.selectable_label(a.langue == l, l.nom_natif()).clicked() {
                                        envoyer(sortie, ActionAssistant::Langue(l));
                                    }
                                }
                            });
                            ui.end_row();
                            ui.label(i.t("Settings.uitheme"));
                            ui.horizontal(|ui| {
                                for (t, cle) in [
                                    (Theme::Clair, "Settings.theme.light"),
                                    (Theme::Sombre, "Settings.theme.dark"),
                                ] {
                                    if ui.selectable_label(a.theme == t, i.t(cle)).clicked() {
                                        envoyer(sortie, ActionAssistant::Theme(t));
                                    }
                                }
                            });
                            ui.end_row();
                        });
                }
                PageAssistant::Sources => {
                    ui.label(i.t("wizard.sources.text"));
                    ui.add_space(6.0);
                    for (n, s) in a.sources.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(s);
                            if ui
                                .small_button(polices::CROIX)
                                .on_hover_text(i.t("Button.remove"))
                                .clicked()
                            {
                                envoyer(sortie, ActionAssistant::RetirerSource(n));
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        let mut saisie = a.saisie_source.clone();
                        let r = ui.add(
                            TextEdit::singleline(&mut saisie)
                                .hint_text(i.t("wizard.sources.hint"))
                                .desired_width(360.0),
                        );
                        if r.changed() {
                            envoyer(sortie, ActionAssistant::SaisieSource(saisie));
                        }
                        let entree = r.lost_focus() && ui.input(|inp| inp.key_pressed(Key::Enter));
                        if ui.button(format!("+ {}", i.t("Button.add"))).clicked() || entree {
                            envoyer(sortie, ActionAssistant::AjouterSource);
                        }
                    });
                    // Sélecteur natif ; la saisie ci-dessus reste disponible en repli.
                    if ui
                        .button(format!("{}…", i.t("gui.folder.browse")))
                        .on_hover_text(i.t("gui.folder.choose"))
                        .clicked()
                    {
                        sortie.push(Message::ChoisirDossier(CibleDossier::Assistant));
                    }
                    if let Some(cle) = a.avertissement {
                        ui.label(
                            RichText::new(format!("{} {}", polices::AVERTISSEMENT, i.t(cle)))
                                .color(ORANGE),
                        );
                    }
                }
                PageAssistant::Extracteur => {
                    egui::Grid::new("as_extracteur")
                        .num_columns(2)
                        .spacing([14.0, 10.0])
                        .show(ui, |ui| {
                            ui.label(i.t("wizard.scraper.default"));
                            ui.label("TMDB");
                            ui.end_row();
                            ui.label(i.t("wizard.scraper.language"));
                            ui.horizontal(|ui| {
                                for l in Langue::TOUTES {
                                    if ui
                                        .selectable_label(
                                            a.langue_extracteur == l.code(),
                                            l.nom_natif(),
                                        )
                                        .clicked()
                                    {
                                        envoyer(
                                            sortie,
                                            ActionAssistant::LangueExtracteur(l.code().into()),
                                        );
                                    }
                                }
                            });
                            ui.end_row();
                            ui.label(i.t("wizard.scraper.apikey"));
                            let mut cle = a.cle_tmdb.clone();
                            if ui
                                .add(
                                    TextEdit::singleline(&mut cle)
                                        .password(true)
                                        .desired_width(320.0),
                                )
                                .changed()
                            {
                                envoyer(sortie, ActionAssistant::CleTmdb(cle));
                            }
                            ui.end_row();
                        });
                    ui.label(
                        RichText::new(i.t("wizard.scraper.apikey.note"))
                            .weak()
                            .small(),
                    );
                }
                PageAssistant::Fin => {
                    ui.label(i.t("wizard.finish.text"));
                    ui.add_space(6.0);
                    let n = a.sources.len().to_string();
                    ui.label(i.t_args("wizard.finish.sources", &[("count", n.as_str())]));
                }
            }
        });
        ui.separator();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if a.page == PageAssistant::Fin {
                if ui.button(i.t("wizard.finish")).clicked() {
                    envoyer(sortie, ActionAssistant::Terminer);
                }
            } else if ui.button(i.t("wizard.next")).clicked() {
                envoyer(sortie, ActionAssistant::Suivante);
            }
            if ui
                .add_enabled(
                    a.page != PageAssistant::Accueil,
                    Button::new(i.t("Button.back")),
                )
                .clicked()
            {
                envoyer(sortie, ActionAssistant::Precedente);
            }
            if ui.button(i.t("Button.cancel")).clicked() {
                sortie.push(Message::FermerDialogue);
            }
        });
    });
    if reponse.should_close() {
        sortie.push(Message::FermerDialogue);
    }
}
