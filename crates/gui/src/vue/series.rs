//! Module Séries (version minimale) : arbre série, saisons, épisodes, mise à jour des
//! sources de séries. Le reste du module arrivera plus tard.

use super::Sortie;
use crate::etat::{Etat, Message};
use crate::source_donnees::SaisonAffichage;
use eframe::egui::{self, CollapsingHeader, RichText, ScrollArea};

fn titre_saison(etat: &Etat, s: &SaisonAffichage) -> String {
    let base = match s.numero {
        0 => etat.i18n.t("gui.tvshows.specials"),
        n if n < 0 => etat.i18n.t("gui.tvshows.unknown"),
        n => etat
            .i18n
            .t_args("gui.tvshows.season", &[("number", n.to_string().as_str())]),
    };
    if s.titre.is_empty() {
        base
    } else {
        format!("{base} - {}", s.titre)
    }
}

pub fn afficher(ui: &mut egui::Ui, etat: &Etat, sortie: &mut Sortie) {
    let i = &etat.i18n;
    ui.horizontal(|ui| {
        if ui
            .button(format!("⟳  {}", i.t("gui.tvshows.update")))
            .clicked()
        {
            sortie.push(Message::MettreAJourSeries);
        }
        let n = etat.series.len().to_string();
        ui.label(i.t_args("gui.tvshows.count", &[("count", n.as_str())]));
    });
    ui.label(RichText::new(i.t("gui.tvshows.soon")).weak().small());
    ui.separator();
    if etat.series.is_empty() {
        ui.add_space(12.0);
        ui.label(i.t("gui.tvshows.empty"));
        return;
    }
    ScrollArea::vertical()
        .id_salt("arbre_series")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for serie in &etat.series {
                let titre = if serie.annee > 0 {
                    format!("{} ({})", serie.titre, serie.annee)
                } else {
                    serie.titre.clone()
                };
                let n = serie.nombre_episodes().to_string();
                CollapsingHeader::new(RichText::new(titre).strong())
                    .id_salt(("serie", &serie.id))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} - {}",
                                serie.chemin,
                                i.t_args("gui.tvshows.episodes", &[("count", n.as_str())])
                            ))
                            .weak()
                            .small(),
                        );
                        for saison in &serie.saisons {
                            CollapsingHeader::new(titre_saison(etat, saison))
                                .id_salt(("saison", &serie.id, saison.numero))
                                .show(ui, |ui| {
                                    for e in &saison.episodes {
                                        ui.horizontal(|ui| {
                                            ui.label(if e.vu { "✔" } else { " " });
                                            let numero = if e.episode >= 0 {
                                                format!("{:02}", e.episode)
                                            } else {
                                                "?".into()
                                            };
                                            ui.label(RichText::new(numero).monospace());
                                            ui.label(&e.titre);
                                            ui.label(RichText::new(&e.fichier).weak());
                                        });
                                    }
                                });
                        }
                    });
            }
        });
}
