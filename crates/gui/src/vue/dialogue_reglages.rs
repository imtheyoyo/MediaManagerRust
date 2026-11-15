//! Dialogue de réglages : arbre filtrable des pages (gauche) + page courante (droite).
//! Pas de bouton « Appliquer » : chaque contrôle modifie immédiatement le réglage ; les
//! réglages des services sont enregistrés à la fermeture du dialogue.

use super::{Sortie, reglages_pages};
use crate::etat::{ActionDialogue, Etat, EtatDialogueReglages, Message};
use crate::modele::normaliser;
use crate::reglages::{Page, Reglages};
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
                ui.label(RichText::new(d.page.titre(i)).heading());
                ui.add_space(6.0);
                ScrollArea::vertical()
                    .id_salt("page_reglages")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        reglages_pages::page(ui, etat, d, r, s, sortie);
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
        etat.appliquer(Message::OuvrirReglagesPage(Page::SeriesSource));
        let (textes, _, _) = rendre(&etat, Vec2::new(1280.0, 800.0), 3);
        assert!(textes.iter().any(|(t, _)| *t == parcourir));
    }

    /// Premiers libellés (clés de traduction) visibles en haut de chaque page.
    fn libelles_attendus(page: Page) -> &'static [&'static str] {
        use Page::*;
        match page {
            General => &["Settings.language", "Settings.uitheme"],
            TypesFichiers => &["gui.settings.ext.video"],
            TriTitre => &["gui.settings.sort.prefixes"],
            OutilsExternes => &["gui.tools.path"],
            MaterielsExternes => &["gui.settings.soon", "gui.settings.soon.devices"],
            ServicesExternes => &["gui.settings.keys.tmdb"],
            Systeme => &["Settings.proxy", "Settings.proxy.host"],
            Ard => &["gui.settings.soon", "gui.settings.soon.ard"],
            Divers => &["Settings.imagecache"],
            Films => &["gui.settings.section.tasks", "Settings.renamer.afterscrape"],
            Series => &["gui.settings.section.tasks"],
            FilmsInterface | SeriesInterface => &["gui.settings.soon", "gui.settings.soon.columns"],
            FilmsSource | SeriesSource => &["Settings.folders"],
            FilmsNfo | SeriesNfo => &["gui.settings.nfo.format"],
            FilmsExtracteur | SeriesExtracteur => &["gui.settings.scraper.language"],
            FilmsIllustrations | SeriesIllustrations => &["gui.settings.art.best"],
            FilmsBandesAnnonces | SeriesBandesAnnonces => &["gui.settings.trailer.auto"],
            FilmsSousTitres | SeriesSousTitres => &["gui.settings.subtitle.language"],
            FilmsRenommeur => &["Settings.renamer.folder", "Settings.renamer.file"],
            SeriesRenommeur => &["Settings.renamer.folder"],
            FilmsPostTraitement | SeriesPostTraitement => {
                &["gui.settings.soon", "gui.settings.soon.scripts"]
            }
            Collections => &["gui.settings.sets.datafolder"],
        }
    }

    /// Toutes les pages de l'arbre, à deux tailles d'écran : titre et libellés présents, aucun
    /// texte de traduction manquant, fenêtre dans l'écran et de taille identique entre
    /// l'image 4 et l'image 20.
    #[test]
    fn toutes_les_pages_dessinees_et_taille_stable() {
        let mut etat = Etat::nouveau();
        for ecran in [Vec2::new(1914.0, 1026.0), Vec2::new(1050.0, 700.0)] {
            let ecran_rect = Rect::from_min_size(Pos2::ZERO, ecran);
            for (page, _, cle) in Page::ARBRE {
                etat.appliquer(Message::OuvrirReglagesPage(*page));
                let i = &etat.i18n;
                let (textes, rect, sortie) = rendre(&etat, ecran, 4);
                let present = |t: &str| textes.iter().any(|(x, _)| x == t);
                let contexte = format!("page {page:?} ({cle}) à {ecran:?}");
                assert!(present(&page.titre(i)), "titre absent : {contexte}");
                for libelle in libelles_attendus(*page) {
                    assert!(present(&i.t(libelle)), "{libelle} absent : {contexte}");
                }
                assert!(
                    textes.iter().all(|(t, _)| !t.contains("???")),
                    "traduction manquante : {contexte}"
                );
                assert!(
                    ecran_rect.contains_rect(rect),
                    "{rect:?} hors écran : {contexte}"
                );
                let (_, plus_tard, _) = rendre(&etat, ecran, 20);
                assert_eq!(
                    rect.size(),
                    plus_tard.size(),
                    "taille instable : {contexte}"
                );
                assert!(
                    sortie.is_empty(),
                    "message émis sans interaction : {sortie:?} ({contexte})"
                );
            }
        }
    }

    /// Les réglages ne sont jamais modifiés par le seul affichage d'une page (aucune
    /// normalisation silencieuse qui déclencherait un enregistrement).
    #[test]
    fn afficher_une_page_ne_modifie_rien() {
        let mut etat = Etat::nouveau();
        for (page, _, _) in Page::ARBRE {
            etat.appliquer(Message::OuvrirReglagesPage(*page));
            let (_, _, sortie) = rendre(&etat, Vec2::new(1280.0, 800.0), 6);
            assert!(
                !sortie.iter().any(|m| matches!(
                    m,
                    Message::AppliquerReglages(_) | Message::AppliquerServices(_)
                )),
                "{page:?}"
            );
        }
    }

    /// Chaque clé de traduction citée par les pages existe en français et en anglais, et chaque
    /// case ou libellé de champ a son infobulle (`clé.tip`).
    #[test]
    fn cles_des_pages_traduites_avec_infobulles() {
        use crate::i18n::analyser;
        let fr = analyser(include_str!("../../i18n/fr.properties"));
        let en = analyser(include_str!("../../i18n/en.properties"));
        let source = include_str!("reglages_pages.rs");
        let cles =
            regex::Regex::new(r#""((?:gui|Settings|wizard|Button)\.[A-Za-z0-9_.]+)""#).unwrap();
        for c in cles.captures_iter(source) {
            let cle = &c[1];
            assert!(fr.contains_key(cle), "absente du français : {cle}");
            assert!(en.contains_key(cle), "absente de l'anglais : {cle}");
        }
        let champs = regex::Regex::new(
            r#"(?s)\b(?:case|etiquette)\(\s*ui,\s*i,\s*(?:[^,"()]+,\s*)?"([^"]+)""#,
        )
        .unwrap();
        let mut n = 0;
        for c in champs.captures_iter(source) {
            let tip = format!("{}.tip", &c[1]);
            assert!(
                fr.contains_key(&tip),
                "infobulle absente du français : {tip}"
            );
            assert!(
                en.contains_key(&tip),
                "infobulle absente de l'anglais : {tip}"
            );
            n += 1;
        }
        assert!(n > 40, "motif de contrôle inopérant ({n} champs trouvés)");
    }
}
