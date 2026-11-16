//! Dialogue « Exporter » : modèle, dossier de sortie, éléments exportés, vignettes et titre ;
//! progression pendant l'export, puis bilan avec accès au dossier et au fichier de liste.

use super::Sortie;
use crate::dialogues::{ActionExport, EtatExport, PhaseExport};
use crate::etat::{ActionDialogue, Etat, Message};
use crate::ouverture::CibleOuverture;
use crate::polices;
use crate::selecteur::CibleDossier;
use crate::theme::{ORANGE, ROUGE, VERT};
use eframe::egui::{
    self, Align, ComboBox, DragValue, Grid, Layout, Modal, ProgressBar, RichText, ScrollArea,
    TextEdit, Vec2,
};

const LARGEUR: f32 = 560.0;
const HAUTEUR: f32 = 300.0;

fn envoyer(sortie: &mut Sortie, a: ActionExport) {
    sortie.push(Message::Dialogue(ActionDialogue::Export(a)));
}

pub fn afficher(ctx: &egui::Context, etat: &Etat, e: &EtatExport, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let reponse = Modal::new(egui::Id::unique("dlg_export")).show(ctx, |ui| {
        ui.set_min_size(Vec2::new(LARGEUR, HAUTEUR));
        ui.heading(i.t("movie.export"));
        let n = e.nb_exportes().to_string();
        ui.label(
            RichText::new(i.t_args(
                if e.series {
                    "gui.export.summary.tv"
                } else {
                    "gui.export.summary.movies"
                },
                &[("count", n.as_str())],
            ))
            .weak(),
        );
        ui.separator();
        match &e.phase {
            PhaseExport::Saisie => saisie(ui, etat, e, sortie),
            PhaseExport::EnCours => progression(ui, etat),
            PhaseExport::Termine(r) => {
                ui.label(
                    RichText::new(format!(
                        "{} {}",
                        polices::COCHE,
                        i.t_args(
                            "gui.export.result",
                            &[("count", r.exportes.to_string().as_str())]
                        )
                    ))
                    .color(VERT)
                    .strong(),
                );
                ui.add_space(4.0);
                ui.label(RichText::new(i.t("gui.export.file")).strong());
                ui.add(egui::Label::new(r.fichier_liste.display().to_string()).wrap());
                if r.vignettes > 0 {
                    ui.label(i.t_args(
                        "gui.export.thumbscount",
                        &[("count", r.vignettes.to_string().as_str())],
                    ));
                }
                if !r.avertissements.is_empty() {
                    ScrollArea::vertical()
                        .id_salt("export_avertissements")
                        .max_height(90.0)
                        .show(ui, |ui| {
                            for a in &r.avertissements {
                                ui.label(
                                    RichText::new(format!("{} {a}", polices::AVERTISSEMENT))
                                        .color(ORANGE),
                                );
                            }
                        });
                }
            }
            PhaseExport::Echec(m) => {
                ui.label(
                    RichText::new(i.t("gui.export.failed"))
                        .color(ROUGE)
                        .strong(),
                );
                ui.add(egui::Label::new(RichText::new(m).color(ROUGE)).wrap());
            }
        }
        ui.add_space(8.0);
        ui.separator();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button(i.t("Button.close")).clicked() {
                sortie.push(Message::FermerDialogue);
            }
            match &e.phase {
                PhaseExport::Saisie => {
                    let actif = e.modele_choisi().is_some() && e.nb_exportes() > 0;
                    if ui
                        .add_enabled(actif, egui::Button::new(i.t("gui.export.start")))
                        .clicked()
                    {
                        envoyer(sortie, ActionExport::Lancer);
                    }
                }
                PhaseExport::Termine(r) => {
                    if ui.button(i.t("gui.export.openfile")).clicked() {
                        sortie.push(Message::Ouvrir(CibleOuverture::Fichier(
                            r.fichier_liste.clone(),
                        )));
                    }
                    if ui.button(i.t("gui.export.openfolder")).clicked() {
                        sortie.push(Message::Ouvrir(CibleOuverture::Dossier {
                            dossier: r.dossier(),
                            selection: Some(r.fichier_liste.clone()),
                        }));
                    }
                }
                PhaseExport::Echec(_) => {
                    if ui.button(i.t("gui.export.retry")).clicked() {
                        envoyer(sortie, ActionExport::Recommencer);
                    }
                }
                PhaseExport::EnCours => {}
            }
        });
    });
    if reponse.should_close() {
        sortie.push(Message::FermerDialogue);
    }
}

fn saisie(ui: &mut egui::Ui, etat: &Etat, e: &EtatExport, sortie: &mut Sortie) {
    let i = &etat.i18n;
    Grid::new("export_champs")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label(i.t("gui.export.model"));
            ui.vertical(|ui| {
                let nom = e.modele_choisi().map(|m| m.nom.clone()).unwrap_or_default();
                ComboBox::from_id_salt("export_modele")
                    .selected_text(nom)
                    .width(260.0)
                    .show_ui(ui, |ui| {
                        for (n, m) in e.modeles.iter().enumerate() {
                            if ui.selectable_label(n == e.modele, &m.nom).clicked() {
                                envoyer(sortie, ActionExport::Modele(n));
                            }
                        }
                    });
                if let Some(m) = e.modele_choisi()
                    && !m.description.is_empty()
                {
                    ui.add(egui::Label::new(RichText::new(&m.description).weak()).truncate());
                }
            });
            ui.end_row();

            ui.label(i.t("gui.export.folder"));
            ui.horizontal(|ui| {
                let mut dossier = e.dossier.clone();
                let mut edit = TextEdit::singleline(&mut dossier).desired_width(300.0);
                if e.erreur.is_some() {
                    edit = edit.text_color(ROUGE);
                }
                if ui.add(edit).changed() {
                    envoyer(sortie, ActionExport::Dossier(dossier));
                }
                if ui.button(i.t("gui.export.browse")).clicked() {
                    sortie.push(Message::ChoisirDossier(CibleDossier::Export));
                }
            });
            ui.end_row();

            ui.label(i.t("gui.export.scope"));
            ui.horizontal(|ui| {
                let mut seule = e.selection_seule;
                let sel = i.t_args(
                    "gui.export.scope.selection",
                    &[("count", e.nb_selection.to_string().as_str())],
                );
                let tout = i.t_args(
                    "gui.export.scope.all",
                    &[("count", e.nb_total.to_string().as_str())],
                );
                ui.add_enabled_ui(e.nb_selection > 0, |ui| {
                    ui.radio_value(&mut seule, true, sel);
                });
                ui.radio_value(&mut seule, false, tout);
                if seule != e.selection_seule {
                    envoyer(sortie, ActionExport::SelectionSeule(seule));
                }
            });
            ui.end_row();

            ui.label(i.t("gui.export.listtitle"));
            let mut titre = e.titre.clone();
            if ui
                .add(TextEdit::singleline(&mut titre).desired_width(300.0))
                .changed()
            {
                envoyer(sortie, ActionExport::Titre(titre));
            }
            ui.end_row();

            ui.label(i.t("gui.export.thumbnails"));
            ui.horizontal(|ui| {
                let mut v = e.vignettes;
                if ui.checkbox(&mut v, "").changed() {
                    envoyer(sortie, ActionExport::Vignettes(v));
                }
                ui.add_enabled_ui(e.vignettes, |ui| {
                    ui.label(i.t("gui.export.width"));
                    let mut l = e.largeur;
                    if ui
                        .add(DragValue::new(&mut l).range(0..=4000).suffix(" px"))
                        .changed()
                    {
                        envoyer(sortie, ActionExport::Largeur(l));
                    }
                });
            });
            ui.end_row();
        });
    if let Some(cle) = e.erreur {
        ui.add_space(4.0);
        ui.label(RichText::new(format!("{} {}", polices::AVERTISSEMENT, i.t(cle))).color(ROUGE));
    }
}

/// Barre de progression de la tâche en cours (la tâche d'export est la principale active).
fn progression(ui: &mut egui::Ui, etat: &Etat) {
    let i = &etat.i18n;
    ui.horizontal(|ui| {
        ui.spinner();
        ui.label(i.t("gui.export.running"));
    });
    if let Some(t) = etat.tache_active() {
        if t.indeterminee() {
            ui.add(
                ProgressBar::new(0.0)
                    .animate(true)
                    .desired_width(LARGEUR - 24.0),
            );
        } else {
            ui.add(
                ProgressBar::new(t.fraction())
                    .desired_width(LARGEUR - 24.0)
                    .text(format!("{} / {}", t.fait, t.total)),
            );
        }
        if !t.description.is_empty() {
            ui.add(egui::Label::new(RichText::new(&t.description).weak()).truncate());
        }
    } else {
        ui.add(
            ProgressBar::new(0.0)
                .animate(true)
                .desired_width(LARGEUR - 24.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commandes::Commande;
    use crate::etat::Dialogue;
    use crate::source_donnees::RapportExportAffiche;
    use crate::vue::essais::verifier_stable;
    use std::path::PathBuf;

    fn etat_export() -> Etat {
        let mut etat = Etat::nouveau();
        etat.appliquer(Message::Commande(Commande::Exporter));
        etat
    }

    fn verifier(etat: &Etat, cles: &[&str]) {
        let attendus: Vec<String> = cles.iter().map(|c| etat.i18n.t(c)).collect();
        verifier_stable(etat, "dlg_export", &attendus, |ctx, sortie| {
            if let Some(Dialogue::Export(d)) = &etat.dialogue {
                afficher(ctx, etat, d, sortie);
            }
        });
    }

    fn phase(etat: &mut Etat, p: PhaseExport) {
        if let Some(Dialogue::Export(d)) = &mut etat.dialogue {
            d.phase = p;
        }
    }

    #[test]
    fn export_saisie_taille_stable() {
        let mut etat = etat_export();
        verifier(
            &etat,
            &[
                "movie.export",
                "gui.export.model",
                "gui.export.folder",
                "gui.export.start",
            ],
        );
        // Avec vignettes, erreur de saisie et dossier très long.
        if let Some(Dialogue::Export(d)) = &mut etat.dialogue {
            d.vignettes = true;
            d.erreur = Some("gui.export.nofolder");
            d.dossier = "D:\\".to_string() + &"très long dossier ".repeat(20);
            d.titre = "Ma liste ".repeat(30);
        }
        verifier(&etat, &["gui.export.thumbnails", "gui.export.nofolder"]);
    }

    #[test]
    fn export_en_cours_termine_et_echec_taille_stable() {
        let mut etat = etat_export();
        phase(&mut etat, PhaseExport::EnCours);
        verifier(&etat, &["gui.export.running"]);
        phase(
            &mut etat,
            PhaseExport::Termine(RapportExportAffiche {
                fichier_liste: PathBuf::from("/tmp/export/très/long/chemin/vers/films.html"),
                exportes: 12,
                vignettes: 12,
                avertissements: (0..30).map(|n| format!("avertissement {n}")).collect(),
            }),
        );
        verifier(
            &etat,
            &[
                "gui.export.file",
                "gui.export.openfolder",
                "gui.export.openfile",
            ],
        );
        phase(&mut etat, PhaseExport::Echec("disque plein ".repeat(40)));
        verifier(&etat, &["gui.export.failed", "gui.export.retry"]);
    }

    #[test]
    fn export_des_series_taille_stable() {
        let mut etat = Etat::nouveau();
        etat.appliquer(Message::ChoisirModule(crate::etat::Module::Series));
        etat.appliquer(Message::Commande(Commande::Exporter));
        verifier(&etat, &["movie.export", "gui.export.scope"]);
    }
}
