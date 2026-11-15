//! Dialogue NON modal des filtres avancés (§2.3) : six onglets (pastille sur un onglet actif),
//! lignes à case trois états, composants de valeur, préréglages nommés.

use super::Sortie;
use crate::etat::{Etat, Message};
use crate::filtres::{
    ActionFiltre, EtatFiltre, LigneFiltre, OngletFiltre, OperateurEnsemble, OperateurNombre,
    TypeValeur, options,
};
use crate::i18n::I18n;
use crate::polices;
use crate::theme::accent;
use eframe::egui::{self, Button, ComboBox, DragValue, Id, RichText, ScrollArea, TextEdit, Vec2};

const ID_NOM_PRERE: &str = "filtres_nom_prereglage";

fn case_trois_etats(ui: &mut egui::Ui, i: &I18n, etat: EtatFiltre) -> bool {
    let infobulle = match etat {
        EtatFiltre::Inactif => i.t("filter.state.off"),
        EtatFiltre::Inclure => i.t("filter.state.include"),
        EtatFiltre::Exclure => i.t("filter.state.exclude"),
    };
    ui.add(
        Button::new(RichText::new(etat.glyphe()).strong())
            .min_size(Vec2::splat(20.0))
            .selected(etat != EtatFiltre::Inactif),
    )
    .on_hover_text(infobulle)
    .clicked()
}

fn operateur(ui: &mut egui::Ui, id: Id, op: &mut OperateurNombre) {
    ComboBox::from_id_salt(id)
        .width(44.0)
        .selected_text(op.symbole())
        .show_ui(ui, |ui| {
            for o in OperateurNombre::TOUS {
                ui.selectable_value(op, o, o.symbole());
            }
        });
}

/// Ligne de filtre : renvoie la ligne modifiée si l'utilisateur a changé quelque chose.
fn ligne(ui: &mut egui::Ui, etat: &Etat, l: &LigneFiltre) -> Option<LigneFiltre> {
    let i = &etat.i18n;
    let mut m = l.clone();
    let id = Id::unique(("filtre", l.id));
    if case_trois_etats(ui, i, l.etat) {
        m.etat = l.etat.suivant();
    }
    let libelle = RichText::new(i.t(l.id.cle()));
    ui.label(if l.effective() {
        libelle.color(accent(etat.reglages.theme))
    } else {
        libelle
    });
    ui.horizontal(|ui| match l.id.type_valeur() {
        TypeValeur::Aucune => {}
        TypeValeur::Texte => {
            ui.add(TextEdit::singleline(&mut m.texte).desired_width(220.0));
        }
        TypeValeur::Nombre => {
            operateur(ui, id.with("op"), &mut m.op_nombre);
            ui.add(DragValue::new(&mut m.a).speed(0.1));
            if m.op_nombre == OperateurNombre::Entre {
                ui.add(DragValue::new(&mut m.b).speed(0.1));
            }
        }
        TypeValeur::Date => {
            operateur(ui, id.with("op"), &mut m.op_nombre);
            ui.add(
                TextEdit::singleline(&mut m.texte)
                    .hint_text(i.t("filter.date.hint"))
                    .desired_width(96.0),
            );
            if m.op_nombre == OperateurNombre::Entre {
                ui.add(
                    TextEdit::singleline(&mut m.texte2)
                        .hint_text(i.t("filter.date.hint"))
                        .desired_width(96.0),
                );
            }
        }
        TypeValeur::Choix { ensemble } => {
            if ensemble {
                ComboBox::from_id_salt(id.with("ens"))
                    .width(90.0)
                    .selected_text(match m.op_ensemble {
                        OperateurEnsemble::AuMoinsUn => i.t("filter.any"),
                        OperateurEnsemble::Tous => i.t("filter.all"),
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut m.op_ensemble,
                            OperateurEnsemble::AuMoinsUn,
                            i.t("filter.any"),
                        );
                        ui.selectable_value(
                            &mut m.op_ensemble,
                            OperateurEnsemble::Tous,
                            i.t("filter.all"),
                        );
                    });
            }
            let resume = if m.choix.is_empty() {
                i.t("filter.choose")
            } else {
                m.choix.iter().cloned().collect::<Vec<_>>().join(", ")
            };
            let resume: String = resume.chars().take(40).collect();
            ui.menu_button(format!("{resume} {}", polices::DEROULANT), |ui| {
                ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    for o in options(l.id, &etat.films) {
                        let mut coche = m.choix.contains(&o);
                        if ui.checkbox(&mut coche, &o).changed() {
                            if coche {
                                m.choix.insert(o);
                            } else {
                                m.choix.remove(&o);
                            }
                        }
                    }
                });
            });
        }
    });
    ui.end_row();
    (m != *l).then_some(m)
}

pub fn afficher(ctx: &egui::Context, etat: &Etat, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let mut ouvert = true;
    egui::Window::new(i.t("filter.dialog.title"))
        .id(Id::unique("dlg_filtres"))
        .open(&mut ouvert)
        .min_size(Vec2::new(500.0, 500.0))
        .default_size(Vec2::new(620.0, 520.0))
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                for onglet in OngletFiltre::TOUS {
                    let mut texte = i.t(onglet.cle());
                    if etat.filtres.onglet_actif(onglet) {
                        texte = format!("{} {texte}", polices::FILTRE_ACTIF);
                    }
                    if ui
                        .selectable_label(etat.onglet_filtres == onglet, texte)
                        .clicked()
                    {
                        sortie.push(Message::OngletFiltres(onglet));
                    }
                }
            });
            ui.separator();
            let hauteur = (ui.available_height() - 90.0).max(120.0);
            ScrollArea::vertical()
                .id_salt("lignes_filtres")
                .max_height(hauteur)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("grille_filtres")
                        .num_columns(3)
                        .spacing([10.0, 6.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for l in etat.filtres.lignes_onglet(etat.onglet_filtres) {
                                if let Some(m) = ligne(ui, etat, l) {
                                    sortie.push(Message::Filtre(ActionFiltre::Ligne(Box::new(m))));
                                }
                            }
                        });
                });
            if ui.button(i.t("filter.reset.tab")).clicked() {
                sortie.push(Message::Filtre(ActionFiltre::ReinitialiserOnglet(
                    etat.onglet_filtres,
                )));
            }
            ui.separator();
            ui.horizontal(|ui| {
                let mut actif = etat.filtres.actif;
                if ui.checkbox(&mut actif, i.t("filter.enableall")).changed() {
                    sortie.push(Message::Filtre(ActionFiltre::Activer(actif)));
                }
                ui.separator();
                ui.label(i.t("filter.presets"));
                let id_nom = Id::unique(ID_NOM_PRERE);
                let mut nom: String = ui.data_mut(|d| d.get_temp(id_nom).unwrap_or_default());
                ComboBox::from_id_salt("prereglages")
                    .width(140.0)
                    .selected_text(nom.clone())
                    .show_ui(ui, |ui| {
                        for p in etat.filtres.prereglages.keys() {
                            if ui.selectable_label(&nom == p, p).clicked() {
                                nom = p.clone();
                                sortie.push(Message::Filtre(ActionFiltre::AppliquerPrereglage(
                                    p.clone(),
                                )));
                            }
                        }
                    });
                ui.add(
                    TextEdit::singleline(&mut nom)
                        .hint_text(i.t("filter.preset.name"))
                        .desired_width(120.0),
                );
                if ui
                    .add_enabled(!nom.trim().is_empty(), Button::new(i.t("Button.save")))
                    .on_hover_text(i.t("filter.savepreset"))
                    .clicked()
                {
                    sortie.push(Message::Filtre(ActionFiltre::EnregistrerPrereglage(
                        nom.clone(),
                    )));
                }
                let existe = etat.filtres.prereglages.contains_key(nom.trim());
                if ui
                    .add_enabled(existe, Button::new(polices::CORBEILLE))
                    .on_hover_text(i.t("Button.remove"))
                    .clicked()
                {
                    sortie.push(Message::Filtre(ActionFiltre::SupprimerPrereglage(
                        nom.trim().to_string(),
                    )));
                    nom.clear();
                }
                ui.data_mut(|d| d.insert_temp(id_nom, nom));
            });
        });
    if !ouvert {
        sortie.push(Message::OuvrirFiltres(false));
    }
}
