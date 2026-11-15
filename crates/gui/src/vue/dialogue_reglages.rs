//! Dialogue de réglages : arbre filtrable des pages (gauche) + page courante (droite).
//! Pas de bouton « Appliquer » : chaque contrôle modifie immédiatement le réglage ; les
//! réglages des services sont enregistrés à la fermeture du dialogue.

use super::Sortie;
use crate::etat::{ActionDialogue, Etat, EtatDialogueReglages, Message};
use crate::i18n::Langue;
use crate::modele::normaliser;
use crate::reglages::{Page, Reglages, Theme};
use crate::selecteur::CibleDossier;
use crate::source_donnees::ReglagesServices;
use eframe::egui::{self, Align, Layout, Modal, RichText, ScrollArea, TextEdit, Vec2};

/// Taille initiale de la fenêtre des réglages (redimensionnable par le coin inférieur droit).
pub const TAILLE_INITIALE: Vec2 = Vec2::new(900.0, 600.0);
const TAILLE_MIN: Vec2 = Vec2::new(640.0, 420.0);
const LARGEUR_ARBRE: f32 = 220.0;
const HAUTEUR_PIED: f32 = 34.0;

/// Fenêtre modale des réglages. Toutes les zones ont une taille bornée par celle de la
/// fenêtre (aucune ne dépend de l'espace « disponible » de la couche modale, sans quoi le
/// contenu grandissait à chaque image jusqu'à sortir de l'écran).
pub fn afficher(ctx: &egui::Context, etat: &Etat, d: &EtatDialogueReglages, sortie: &mut Sortie) {
    let mut r: Reglages = etat.reglages.clone();
    let mut s: ReglagesServices = etat.services.clone();
    let ecran = ctx.content_rect().size();
    // Marge pour le cadre de la fenêtre modale.
    let max = (ecran - Vec2::splat(48.0)).max(TAILLE_MIN);
    let reponse = Modal::new(egui::Id::unique("dlg_reglages")).show(ctx, |ui| {
        egui::Resize::default()
            .id_salt("taille_reglages")
            .default_size(TAILLE_INITIALE.min(max))
            .min_size(TAILLE_MIN.min(max))
            .max_size(max)
            .show(ui, |ui| {
                contenu(ui, etat, d, &mut r, &mut s, sortie);
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

fn contenu(
    ui: &mut egui::Ui,
    etat: &Etat,
    d: &EtatDialogueReglages,
    r: &mut Reglages,
    s: &mut ReglagesServices,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    ui.heading(i.t("gui.settings.title"));
    ui.separator();
    let hauteur = (ui.available_height() - HAUTEUR_PIED).max(120.0);
    let largeur = ui.available_width();
    ui.allocate_ui_with_layout(
        Vec2::new(largeur, hauteur),
        Layout::left_to_right(Align::Min),
        |ui| {
            ui.set_min_size(Vec2::new(largeur, hauteur));
            // Arbre des pages (tout développé), filtrable.
            ui.allocate_ui_with_layout(
                Vec2::new(LARGEUR_ARBRE, hauteur),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(LARGEUR_ARBRE);
                    let mut filtre = d.filtre.clone();
                    if ui
                        .add(
                            TextEdit::singleline(&mut filtre)
                                .hint_text(i.t("Settings.filter"))
                                .desired_width(LARGEUR_ARBRE - 8.0),
                        )
                        .changed()
                    {
                        sortie.push(Message::Dialogue(ActionDialogue::ReglagesFiltre(
                            filtre.clone(),
                        )));
                    }
                    let motif = normaliser(&filtre);
                    ScrollArea::vertical()
                        .id_salt("arbre_reglages")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (page, niveau, cle) in Page::ARBRE {
                                let libelle = i.t(cle);
                                if !motif.is_empty() && !normaliser(&libelle).contains(&motif) {
                                    continue;
                                }
                                ui.horizontal(|ui| {
                                    ui.add_space(14.0 * *niveau as f32);
                                    let texte = if *niveau == 0 {
                                        RichText::new(libelle).strong()
                                    } else {
                                        RichText::new(libelle)
                                    };
                                    if ui.selectable_label(d.page == *page, texte).clicked() {
                                        sortie.push(Message::Dialogue(
                                            ActionDialogue::ReglagesPage(*page),
                                        ));
                                    }
                                });
                            }
                        });
                },
            );
            ui.separator();
            // Page sélectionnée.
            let reste = ui.available_size();
            ui.allocate_ui_with_layout(reste, Layout::top_down(Align::Min), |ui| {
                ui.set_min_size(reste);
                ui.label(RichText::new(i.t(d.page.cle())).heading());
                ui.add_space(6.0);
                ScrollArea::vertical()
                    .id_salt("page_reglages")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        page(ui, etat, d, r, s, sortie);
                    });
            });
        },
    );
    ui.separator();
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if ui.button(i.t("Button.close")).clicked() {
            sortie.push(Message::FermerDialogue);
        }
    });
}

/// Liste de dossiers éditable : retrait par ✖, ajout par le sélecteur natif (« Parcourir… »)
/// ou, en repli, par saisie manuelle.
fn liste_dossiers(
    ui: &mut egui::Ui,
    etat: &Etat,
    dossiers: &mut Vec<String>,
    saisie: &str,
    message: fn(String) -> ActionDialogue,
    cible: CibleDossier,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    let mut a_retirer = None;
    if dossiers.is_empty() {
        ui.label(RichText::new(i.t("gui.folder.none")).weak());
    }
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
    ui.add_space(4.0);
    if ui
        .button(format!("{}…", i.t("gui.folder.browse")))
        .on_hover_text(i.t("gui.folder.choose"))
        .clicked()
    {
        sortie.push(Message::ChoisirDossier(cible));
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
            ui.label(RichText::new(i.t("gui.update.section")).strong());
            ui.checkbox(&mut r.verifier_mises_a_jour, i.t("gui.update.atstartup"));
            ui.horizontal(|ui| {
                if ui.button(format!("{}…", i.t("gui.update.check"))).clicked() {
                    sortie.push(Message::RechercherMiseAJour { manuel: true });
                }
                ui.label(
                    RichText::new(etat.config_mise_a_jour.courant.libelle())
                        .weak()
                        .small(),
                );
            });
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
                CibleDossier::SourceFilms,
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
                CibleDossier::SourceSeries,
                sortie,
            );
        }
        _ => {
            ui.label(RichText::new(i.t("Settings.todo")).weak());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::etat::{Dialogue, TypeDialogue};
    use eframe::egui::{Pos2, RawInput, Rect, epaint::Shape};

    /// Rendu hors écran du dialogue : textes dessinés (avec leur position) et rectangle de la
    /// fenêtre, après `images` images.
    fn rendre(etat: &Etat, ecran: Vec2, images: usize) -> (Vec<(String, Pos2)>, Rect, Sortie) {
        let ctx = egui::Context::default();
        ctx.set_visuals(crate::theme::visuals(etat.reglages.theme));
        let entree = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, ecran)),
            ..Default::default()
        };
        let mut textes = Vec::new();
        let mut sortie_dialogue = Vec::new();
        for _ in 0..images {
            let mut sortie = ctx.run_ui(entree.clone(), |ui| {
                sortie_dialogue.clear();
                if let Some(Dialogue::Reglages(d)) = &etat.dialogue {
                    afficher(ui.ctx(), etat, d, &mut sortie_dialogue);
                }
            });
            sortie.textures_delta.clear();
            textes = sortie
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    Shape::Text(t) if c.clip_rect.contains(t.pos) => {
                        Some((t.galley.text().to_string(), t.pos))
                    }
                    _ => None,
                })
                .collect();
        }
        let rect = ctx
            .memory(|m| m.area_rect(egui::Id::unique("dlg_reglages")))
            .expect("fenêtre des réglages affichée");
        (textes, rect, sortie_dialogue)
    }

    #[test]
    fn contenu_de_la_page_dessine_et_taille_stable() {
        let mut etat = Etat::nouveau();
        etat.appliquer(Message::OuvrirDialogue(TypeDialogue::Reglages));
        let Some(Dialogue::Reglages(d)) = &etat.dialogue else {
            panic!("dialogue attendu");
        };
        assert_eq!(d.page, Page::General, "page Général ouverte par défaut");
        let i = &etat.i18n;
        for ecran in [Vec2::new(1920.0, 1080.0), Vec2::new(1050.0, 700.0)] {
            let (textes, rect, sortie) = rendre(&etat, ecran, 4);
            let present = |t: &str| textes.iter().any(|(x, _)| x == t);
            // Titre, arbre (toutes les pages), page Général et bouton de fermeture.
            assert!(present(&i.t("gui.settings.title")), "{ecran:?}");
            for (_, _, cle) in Page::ARBRE {
                assert!(present(&i.t(cle)), "page {cle} absente de l'arbre");
            }
            for cle in [
                "Settings.language",
                "Settings.uitheme",
                "Settings.rememberlastmodule",
                "gui.update.atstartup",
                "Button.close",
            ] {
                assert!(present(&i.t(cle)), "{cle} absent ({ecran:?})");
            }
            // Tout est dans l'écran.
            let ecran_rect = Rect::from_min_size(Pos2::ZERO, ecran);
            assert!(ecran_rect.contains_rect(rect), "{rect:?} hors de {ecran:?}");
            assert!(textes.iter().all(|(_, p)| ecran_rect.contains(*p)));
            // Taille correcte et stable d'une image à l'autre (plus de croissance continue).
            let (_, rect_plus_tard, _) = rendre(&etat, ecran, 12);
            assert_eq!(rect.size(), rect_plus_tard.size(), "{ecran:?}");
            if ecran.x >= 1920.0 {
                assert!(
                    (880.0..1000.0).contains(&rect.width())
                        && (580.0..700.0).contains(&rect.height()),
                    "taille {:?}",
                    rect.size()
                );
            }
            assert!(
                sortie.is_empty(),
                "aucun message sans interaction : {sortie:?}"
            );
        }
    }

    #[test]
    fn pages_des_sources_proposent_le_selecteur() {
        let mut etat = Etat::nouveau();
        etat.appliquer(Message::OuvrirReglagesPage(Page::FilmsSource));
        let (textes, _, _) = rendre(&etat, Vec2::new(1280.0, 800.0), 3);
        let parcourir = format!("{}…", etat.i18n.t("gui.folder.browse"));
        assert!(textes.iter().any(|(t, _)| *t == parcourir));
        etat.appliquer(Message::OuvrirReglagesPage(Page::Series));
        let (textes, _, _) = rendre(&etat, Vec2::new(1280.0, 800.0), 3);
        assert!(textes.iter().any(|(t, _)| *t == parcourir));
    }
}
