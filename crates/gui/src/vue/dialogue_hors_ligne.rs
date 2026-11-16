//! Dialogue « Créer un film hors ligne » : titre, année et source de données du film sans
//! fichier vidéo (un dossier et un marqueur `.disc` sont créés dans la source choisie).

use super::Sortie;
use crate::dialogues::{ActionHorsLigne, EtatHorsLigne};
use crate::etat::{ActionDialogue, Etat, Message};
use crate::polices;
use crate::theme::ROUGE;
use eframe::egui::{self, Align, ComboBox, Grid, Key, Layout, Modal, RichText, TextEdit};

const LARGEUR: f32 = 460.0;
const LARGEUR_CHAMP: f32 = 300.0;

fn envoyer(sortie: &mut Sortie, a: ActionHorsLigne) {
    sortie.push(Message::Dialogue(ActionDialogue::HorsLigne(a)));
}

pub fn afficher(ctx: &egui::Context, etat: &Etat, e: &EtatHorsLigne, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let mut titre = e.titre.clone();
    let mut annee = e.annee.clone();
    let mut source = e.source;
    let mut valider = false;
    let mut annuler = false;
    let reponse = Modal::new(egui::Id::unique("dlg_hors_ligne")).show(ctx, |ui| {
        ui.set_width(LARGEUR);
        ui.heading(i.t("movie.createoffline"));
        ui.add_space(4.0);
        ui.label(RichText::new(i.t("gui.offline.note")).weak().small());
        ui.add_space(8.0);
        Grid::new("hors_ligne_champs")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(i.t("metatag.title"));
                let r = ui.add(TextEdit::singleline(&mut titre).desired_width(LARGEUR_CHAMP));
                // Focus dans le titre à l'ouverture (une seule fois).
                let id_focus = egui::Id::unique("hors_ligne_focus");
                if !ui.data(|d| d.get_temp::<bool>(id_focus)).unwrap_or(false) {
                    r.request_focus();
                    ui.data_mut(|d| d.insert_temp(id_focus, true));
                }
                ui.end_row();
                ui.label(i.t("metatag.year"));
                ui.add(TextEdit::singleline(&mut annee).desired_width(70.0));
                ui.end_row();
                ui.label(i.t("Settings.source"));
                if e.sources.len() > 1 {
                    let courant = e.sources.get(source).cloned().unwrap_or_default();
                    ComboBox::from_id_salt("hors_ligne_source")
                        .width(LARGEUR_CHAMP - 18.0)
                        .selected_text(courant)
                        .show_ui(ui, |ui| {
                            for (n, s) in e.sources.iter().enumerate() {
                                ui.selectable_value(&mut source, n, s);
                            }
                        });
                } else {
                    ui.label(e.sources.first().cloned().unwrap_or_default());
                }
                ui.end_row();
            });
        if let Some(cle) = e.erreur {
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("{} {}", polices::AVERTISSEMENT, i.t(cle))).color(ROUGE),
            );
        }
        ui.add_space(10.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button(i.t("Button.ok")).clicked() {
                valider = true;
            }
            if ui.button(i.t("Button.cancel")).clicked() {
                annuler = true;
            }
        });
    });
    if titre != e.titre {
        envoyer(sortie, ActionHorsLigne::Titre(titre));
    }
    if annee != e.annee {
        envoyer(sortie, ActionHorsLigne::Annee(annee));
    }
    if source != e.source {
        envoyer(sortie, ActionHorsLigne::Source(source));
    }
    let entree = ctx.input_mut(|inp| inp.consume_key(egui::Modifiers::NONE, Key::Enter));
    if valider || entree {
        envoyer(sortie, ActionHorsLigne::Valider);
    } else if annuler || reponse.should_close() {
        envoyer(sortie, ActionHorsLigne::Abandonner);
    }
    if annuler || reponse.should_close() {
        ctx.data_mut(|d| d.remove::<bool>(egui::Id::unique("hors_ligne_focus")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::etat::Dialogue;
    use crate::vue::essais::verifier_stable;

    #[test]
    fn taille_stable() {
        let mut etat = Etat::nouveau();
        etat.appliquer(Message::Commande(
            crate::commandes::Commande::CreerHorsLigne,
        ));
        let Some(Dialogue::HorsLigne(e)) = &etat.dialogue else {
            panic!("dialogue non ouvert");
        };
        let e = (**e).clone();
        let attendus = vec![
            etat.i18n.t("movie.createoffline"),
            etat.i18n.t("metatag.title"),
            etat.i18n.t("metatag.year"),
            etat.i18n.t("Button.ok"),
            etat.i18n.t("Button.cancel"),
        ];
        verifier_stable(&etat, "dlg_hors_ligne", &attendus, |ctx, sortie| {
            afficher(ctx, &etat, &e, sortie)
        });
    }
}
