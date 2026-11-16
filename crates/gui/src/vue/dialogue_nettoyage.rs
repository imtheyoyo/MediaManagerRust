//! Dialogue « Nettoyer » : fichiers inutiles des dossiers des films choisis (raison et taille),
//! cases à cocher, total libérable ; l'exécution déplace les éléments cochés dans la corbeille
//! de leur source après une dernière confirmation.

use super::Sortie;
use super::details::taille_lisible;
use crate::dialogues::{ActionNettoyage, EtatNettoyage, PhaseNettoyage};
use crate::etat::{ActionDialogue, Etat, Message};
use crate::polices;
use crate::source_donnees::RaisonNettoyage;
use crate::theme::{ORANGE, ROUGE};
use eframe::egui::{self, Align, Layout, Modal, RichText, Vec2};
use egui_extras::{Column, TableBuilder};

const LARGEUR: f32 = 780.0;
const HAUTEUR: f32 = 440.0;
/// Hauteur maximale de la liste (défilement au-delà).
const HAUTEUR_LISTE: f32 = 280.0;

fn envoyer(sortie: &mut Sortie, a: ActionNettoyage) {
    sortie.push(Message::Dialogue(ActionDialogue::Nettoyage(a)));
}

/// Libellé de la raison pour laquelle un élément est jugé inutile.
pub fn libelle_raison(etat: &Etat, r: &RaisonNettoyage) -> String {
    let i = &etat.i18n;
    match r {
        RaisonNettoyage::Motif(m) => i.t_args("gui.cleanup.reason.pattern", &[("pattern", m)]),
        RaisonNettoyage::Parasite => i.t("gui.cleanup.reason.junk"),
        RaisonNettoyage::Extrait => i.t("gui.cleanup.reason.sample"),
        RaisonNettoyage::NfoOrphelin => i.t("gui.cleanup.reason.orphannfo"),
    }
}

fn taille(etat: &Etat, octets: u64) -> String {
    let t = taille_lisible(etat, octets);
    if t.is_empty() { "0 Ko".into() } else { t }
}

pub fn afficher(ctx: &egui::Context, etat: &Etat, n: &EtatNettoyage, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let reponse = Modal::new(egui::Id::unique("dlg_nettoyage")).show(ctx, |ui| {
        ui.set_min_size(Vec2::new(LARGEUR, HAUTEUR));
        ui.heading(i.t("movie.cleanup"));
        ui.separator();
        match &n.phase {
            PhaseNettoyage::Analyse => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(i.t("gui.cleanup.analyzing"));
                });
            }
            PhaseNettoyage::Echec(e) => {
                ui.add(
                    egui::Label::new(
                        RichText::new(format!(
                            "{} {} {e}",
                            polices::AVERTISSEMENT,
                            i.t("gui.cleanup.failed")
                        ))
                        .color(ROUGE),
                    )
                    .wrap(),
                );
            }
            PhaseNettoyage::Liste | PhaseNettoyage::Confirmation => liste(ui, etat, n, sortie),
        }
        if n.phase == PhaseNettoyage::Confirmation {
            ui.add_space(6.0);
            ui.add(
                egui::Label::new(
                    RichText::new(format!(
                        "{} {}",
                        polices::AVERTISSEMENT,
                        i.t_args(
                            "gui.cleanup.confirm",
                            &[
                                ("count", n.nb_coches().to_string().as_str()),
                                ("size", taille(etat, n.octets_coches()).as_str()),
                            ],
                        )
                    ))
                    .color(ORANGE),
                )
                .wrap(),
            );
        }
        ui.add_space(8.0);
        ui.separator();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            match n.phase {
                PhaseNettoyage::Confirmation => {
                    if ui.button(i.t("Button.ok")).clicked() {
                        envoyer(sortie, ActionNettoyage::ConfirmerExecution);
                    }
                    if ui.button(i.t("gui.cleanup.back")).clicked() {
                        envoyer(sortie, ActionNettoyage::AnnulerConfirmation);
                    }
                }
                _ => {
                    if ui.button(i.t("Button.close")).clicked() {
                        sortie.push(Message::FermerDialogue);
                    }
                    if n.phase == PhaseNettoyage::Liste
                        && ui
                            .add_enabled(
                                n.nb_coches() > 0,
                                egui::Button::new(format!(
                                    "{} {}",
                                    polices::CORBEILLE,
                                    i.t("gui.cleanup.run")
                                )),
                            )
                            .clicked()
                    {
                        envoyer(sortie, ActionNettoyage::Executer);
                    }
                    if n.phase == PhaseNettoyage::Liste && !n.lignes.is_empty() {
                        ui.label(
                            RichText::new(i.t_args(
                                "gui.cleanup.total",
                                &[
                                    ("size", taille(etat, n.octets_coches()).as_str()),
                                    ("count", n.nb_coches().to_string().as_str()),
                                ],
                            ))
                            .strong(),
                        );
                    }
                }
            };
        });
    });
    if reponse.should_close() {
        sortie.push(Message::FermerDialogue);
    }
}

fn liste(ui: &mut egui::Ui, etat: &Etat, n: &EtatNettoyage, sortie: &mut Sortie) {
    let i = &etat.i18n;
    ui.label(
        RichText::new(i.t_args(
            "gui.cleanup.summary",
            &[
                ("films", n.nb_films.to_string().as_str()),
                ("count", n.lignes.len().to_string().as_str()),
            ],
        ))
        .weak(),
    );
    if n.lignes.is_empty() {
        ui.add_space(6.0);
        ui.label(i.t("gui.cleanup.none"));
        return;
    }
    let actif = n.phase == PhaseNettoyage::Liste;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(actif, egui::Button::new(i.t("gui.cleanup.selectall")))
            .clicked()
        {
            envoyer(sortie, ActionNettoyage::ToutCocher(true));
        }
        if ui
            .add_enabled(actif, egui::Button::new(i.t("gui.cleanup.selectnone")))
            .clicked()
        {
            envoyer(sortie, ActionNettoyage::ToutCocher(false));
        }
    });
    ui.add_space(4.0);
    TableBuilder::new(ui)
        .id_salt("table_nettoyage")
        .striped(true)
        .max_scroll_height(HAUTEUR_LISTE)
        .column(Column::exact(28.0))
        .column(Column::exact(170.0).clip(true))
        .column(Column::remainder().clip(true))
        .column(Column::exact(150.0).clip(true))
        .column(Column::exact(80.0))
        .header(22.0, |mut h| {
            h.col(|_| {});
            for cle in [
                "gui.cleanup.col.movie",
                "gui.cleanup.col.item",
                "gui.cleanup.col.reason",
                "gui.cleanup.col.size",
            ] {
                h.col(|ui| {
                    ui.strong(i.t(cle));
                });
            }
        })
        .body(|body| {
            body.rows(22.0, n.lignes.len(), |mut ligne| {
                let rang = ligne.index();
                let l = &n.lignes[rang];
                ligne.col(|ui| {
                    let mut coche = l.coche;
                    if ui
                        .add_enabled(actif, egui::Checkbox::without_text(&mut coche))
                        .changed()
                    {
                        envoyer(sortie, ActionNettoyage::Cocher(rang, coche));
                    }
                });
                ligne.col(|ui| {
                    ui.add(egui::Label::new(&l.fichier.titre_film).truncate());
                });
                ligne.col(|ui| {
                    let nom = if l.fichier.est_dossier {
                        format!("{}{}", l.fichier.chemin, std::path::MAIN_SEPARATOR)
                    } else {
                        l.fichier.chemin.clone()
                    };
                    ui.add(egui::Label::new(nom).truncate())
                        .on_hover_text(&l.fichier.chemin);
                });
                ligne.col(|ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(libelle_raison(etat, &l.fichier.raison)).weak(),
                        )
                        .truncate(),
                    );
                });
                ligne.col(|ui| {
                    ui.label(taille(etat, l.fichier.taille));
                });
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogues::LigneNettoyage;
    use crate::etat::Dialogue;
    use crate::source_donnees::FichierInutileAffiche;
    use crate::vue::essais::verifier_stable;

    fn lignes(n: usize) -> Vec<LigneNettoyage> {
        (0..n)
            .map(|k| LigneNettoyage {
                fichier: FichierInutileAffiche {
                    id_film: k as u64,
                    titre_film: format!("Un film au titre très très long numéro {k}"),
                    chemin: format!("D:\\Films\\{}\\Sample", "dossier-long".repeat(8)),
                    est_dossier: k % 2 == 0,
                    raison: match k % 4 {
                        0 => RaisonNettoyage::Extrait,
                        1 => RaisonNettoyage::Parasite,
                        2 => RaisonNettoyage::NfoOrphelin,
                        _ => RaisonNettoyage::Motif("(?i)^release\\.nfo$".into()),
                    },
                    taille: 1_234_567 * (k as u64 + 1),
                },
                coche: k % 3 != 0,
            })
            .collect()
    }

    fn etat_avec(phase: PhaseNettoyage, n: usize) -> Etat {
        let mut etat = Etat::nouveau();
        etat.dialogue = Some(Dialogue::Nettoyage(Box::new(EtatNettoyage {
            id_requete: 1,
            nb_films: 3,
            phase,
            lignes: lignes(n),
        })));
        etat
    }

    fn verifier(etat: &Etat, cles: &[&str]) {
        let attendus: Vec<String> = cles.iter().map(|c| etat.i18n.t(c)).collect();
        verifier_stable(etat, "dlg_nettoyage", &attendus, |ctx, sortie| {
            if let Some(Dialogue::Nettoyage(d)) = &etat.dialogue {
                afficher(ctx, etat, d, sortie);
            }
        });
    }

    #[test]
    fn liste_courte_et_longue_taille_stable() {
        for n in [4, 60] {
            let etat = etat_avec(PhaseNettoyage::Liste, n);
            verifier(
                &etat,
                &[
                    "movie.cleanup",
                    "gui.cleanup.col.movie",
                    "gui.cleanup.col.reason",
                    "gui.cleanup.run",
                    "gui.cleanup.selectall",
                ],
            );
        }
    }

    #[test]
    fn analyse_confirmation_echec_et_liste_vide_taille_stable() {
        verifier(
            &etat_avec(PhaseNettoyage::Analyse, 0),
            &["movie.cleanup", "gui.cleanup.analyzing"],
        );
        verifier(
            &etat_avec(PhaseNettoyage::Confirmation, 6),
            &["gui.cleanup.back", "Button.ok"],
        );
        verifier(
            &etat_avec(PhaseNettoyage::Echec("accès refusé ".repeat(30)), 0),
            &["gui.cleanup.failed"],
        );
        verifier(&etat_avec(PhaseNettoyage::Liste, 0), &["gui.cleanup.none"]);
    }

    #[test]
    fn raisons_traduites() {
        let etat = Etat::nouveau();
        assert_eq!(
            libelle_raison(&etat, &RaisonNettoyage::Parasite),
            etat.i18n.t("gui.cleanup.reason.junk")
        );
        assert!(libelle_raison(&etat, &RaisonNettoyage::Motif("abc".into())).contains("abc"));
    }
}
