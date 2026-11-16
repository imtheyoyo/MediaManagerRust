//! Dialogue de réglages : arbre filtrable des pages (gauche) + page courante (droite).
//! Pas de bouton « Appliquer » : chaque contrôle modifie immédiatement le réglage ; les
//! réglages des services sont enregistrés à la fermeture du dialogue.

use super::{Sortie, reglages_pages};
use crate::etat::{ActionDialogue, Etat, EtatDialogueReglages, Message};
use crate::modele::normaliser;
use crate::reglages::{Page, Reglages, ToucheArbre, naviguer, pages_visibles};
use crate::source_donnees::ReglagesServices;
use eframe::egui::{self, Align, Key, Layout, Modal, RichText, ScrollArea, Sense, TextEdit, Vec2};
use std::collections::HashSet;

/// Taille initiale de la fenêtre des réglages (redimensionnable par le coin inférieur droit).
pub const TAILLE_INITIALE: Vec2 = Vec2::new(900.0, 600.0);
const TAILLE_MIN: Vec2 = Vec2::new(640.0, 420.0);
const LARGEUR_ARBRE: f32 = 220.0;
const HAUTEUR_PIED: f32 = 34.0;

/// Fenêtre modale des réglages. Toutes les zones ont une taille bornée par celle de la
/// fenêtre (aucune ne dépend de l'espace « disponible » de la couche modale, sans quoi le
/// contenu grandissait à chaque image jusqu'à sortir de l'écran).
/// Touches de navigation de l'arbre et action associée.
const TOUCHES_ARBRE: [(Key, ToucheArbre); 6] = [
    (Key::ArrowUp, ToucheArbre::Haut),
    (Key::ArrowDown, ToucheArbre::Bas),
    (Key::ArrowLeft, ToucheArbre::Gauche),
    (Key::ArrowRight, ToucheArbre::Droite),
    (Key::Home, ToucheArbre::Debut),
    (Key::End, ToucheArbre::Fin),
];

fn id_repliees() -> egui::Id {
    egui::Id::unique("reglages_noeuds_repliees")
}

fn id_derniere_page() -> egui::Id {
    egui::Id::unique("reglages_derniere_page_visible")
}

/// Nœuds réduits de l'arbre (état purement visuel, conservé par egui tant que la fenêtre vit).
fn lire_repliees(ctx: &egui::Context) -> HashSet<Page> {
    ctx.data(|d| d.get_temp::<HashSet<Page>>(id_repliees()))
        .unwrap_or_default()
}

fn ecrire_repliees(ctx: &egui::Context, r: &HashSet<Page>) {
    ctx.data_mut(|d| d.insert_temp(id_repliees(), r.clone()));
}

/// Pages affichées dans l'arbre pour le filtre `d.filtre` et les nœuds réduits `repliees`.
fn visibles(
    i: &crate::i18n::I18n,
    d: &EtatDialogueReglages,
    repliees: &HashSet<Page>,
) -> Vec<Page> {
    let motif = normaliser(&d.filtre);
    pages_visibles(
        |p| motif.is_empty() || normaliser(&i.t(p.cle())).contains(&motif),
        !motif.is_empty(),
        repliees,
    )
}

/// Navigation au clavier dans l'arbre : valable dès l'ouverture (aucun clic préalable), sauf si
/// un champ de texte a le focus (les flèches lui appartiennent). Renvoie la page à ouvrir.
fn navigation_clavier(
    ctx: &egui::Context,
    i: &crate::i18n::I18n,
    d: &EtatDialogueReglages,
) -> Option<Page> {
    if ctx.egui_wants_keyboard_input() {
        return None;
    }
    let mut repliees = lire_repliees(ctx);
    let mut page = d.page;
    for (touche, action) in TOUCHES_ARBRE {
        let appuyee = ctx.input_mut(|inp| inp.consume_key(egui::Modifiers::NONE, touche));
        if appuyee {
            let vis = visibles(i, d, &repliees);
            page = naviguer(action, page, &vis, &mut repliees);
        }
    }
    ecrire_repliees(ctx, &repliees);
    (page != d.page).then_some(page)
}

pub fn afficher(ctx: &egui::Context, etat: &Etat, d: &EtatDialogueReglages, sortie: &mut Sortie) {
    if let Some(page) = navigation_clavier(ctx, &etat.i18n, d) {
        sortie.push(Message::Dialogue(ActionDialogue::ReglagesPage(page)));
        ctx.request_repaint();
    }
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
            // Arbre des pages (nœuds réductibles), filtrable, pilotable au clavier.
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
                    let mut repliees = lire_repliees(ui.ctx());
                    let visibles = visibles(i, d, &repliees);
                    // La page sélectionnée est ramenée dans la zone visible quand elle change
                    // (clavier, clic, ouverture).
                    let a_defiler =
                        ui.ctx().data(|m| m.get_temp::<Page>(id_derniere_page())) != Some(d.page);
                    ScrollArea::vertical()
                        .id_salt("arbre_reglages")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for page in &visibles {
                                let page = *page;
                                let libelle = i.t(page.cle());
                                let niveau = page.niveau();
                                ui.horizontal(|ui| {
                                    ui.add_space(14.0 * f32::from(niveau));
                                    // Flèche de réduction (nœuds de premier niveau ayant des sous-pages).
                                    let (_, rep_fleche) =
                                        ui.allocate_exact_size(Vec2::splat(14.0), Sense::click());
                                    if page.a_des_enfants() && filtre.is_empty() {
                                        let reduit = repliees.contains(&page);
                                        egui::collapsing_header::paint_default_icon(
                                            ui,
                                            if reduit { 0.0 } else { 1.0 },
                                            &rep_fleche,
                                        );
                                        if rep_fleche.clicked() {
                                            if reduit {
                                                repliees.remove(&page);
                                            } else {
                                                repliees.insert(page);
                                                if d.page != page && d.page.racine() == page {
                                                    sortie.push(Message::Dialogue(
                                                        ActionDialogue::ReglagesPage(page),
                                                    ));
                                                }
                                            }
                                            ecrire_repliees(ui.ctx(), &repliees);
                                        }
                                    }
                                    let texte = if niveau == 0 {
                                        RichText::new(libelle).strong()
                                    } else {
                                        RichText::new(libelle)
                                    };
                                    let rep = ui.selectable_label(d.page == page, texte);
                                    if rep.clicked() {
                                        sortie.push(Message::Dialogue(
                                            ActionDialogue::ReglagesPage(page),
                                        ));
                                    }
                                    // Les flèches du clavier pilotent l'arbre, pas le focus egui.
                                    if rep.has_focus() {
                                        rep.surrender_focus();
                                    }
                                    if d.page == page && a_defiler {
                                        rep.scroll_to_me(None);
                                    }
                                });
                            }
                        });
                    ui.ctx()
                        .data_mut(|m| m.insert_temp(id_derniere_page(), d.page));
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

    /// Rend une image sans événement puis une image où `touche` est pressée ; renvoie les
    /// messages émis par cette dernière (le dialogue est ouvert sans aucun clic préalable).
    fn avec_touche(etat: &Etat, touche: egui::Key) -> Sortie {
        let ctx = egui::Context::default();
        ctx.set_visuals(crate::theme::visuals(etat.reglages.theme));
        let ecran = Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0));
        let Some(Dialogue::Reglages(d)) = &etat.dialogue else {
            panic!("dialogue attendu");
        };
        let mut sortie = Vec::new();
        for n in 0..3 {
            let mut entree = RawInput {
                screen_rect: Some(ecran),
                ..Default::default()
            };
            if n == 2 {
                entree.events.push(egui::Event::Key {
                    key: touche,
                    physical_key: Some(touche),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            sortie.clear();
            let mut out = ctx.run_ui(entree, |ui| afficher(ui.ctx(), etat, d, &mut sortie));
            out.textures_delta.clear();
        }
        sortie
    }

    fn pages_demandees(sortie: &Sortie) -> Vec<Page> {
        sortie
            .iter()
            .filter_map(|m| match m {
                Message::Dialogue(ActionDialogue::ReglagesPage(p)) => Some(*p),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn fleches_du_clavier_changent_de_page_des_l_ouverture() {
        let mut etat = Etat::nouveau();
        etat.appliquer(Message::OuvrirDialogue(TypeDialogue::Reglages));
        assert_eq!(
            pages_demandees(&avec_touche(&etat, egui::Key::ArrowDown)),
            [Page::TypesFichiers]
        );
        assert_eq!(
            pages_demandees(&avec_touche(&etat, egui::Key::End)),
            [Page::SeriesPostTraitement]
        );
        etat.appliquer(Message::OuvrirReglagesPage(Page::FilmsSource));
        assert_eq!(
            pages_demandees(&avec_touche(&etat, egui::Key::ArrowLeft)),
            [Page::Films]
        );
        assert_eq!(
            pages_demandees(&avec_touche(&etat, egui::Key::ArrowUp)),
            [Page::FilmsInterface]
        );
        assert_eq!(
            pages_demandees(&avec_touche(&etat, egui::Key::Home)),
            [Page::General]
        );
        // Une touche sans rapport ne change rien.
        assert!(pages_demandees(&avec_touche(&etat, egui::Key::A)).is_empty());
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

    /// Ouvre la page « Services externes » avec quelques clés enregistrées et une saisie en
    /// cours (jamais affichée en clair).
    fn etat_services() -> Etat {
        use crate::etat::Secret;
        let mut etat = Etat::nouveau();
        etat.appliquer(Message::OuvrirReglagesPage(Page::ServicesExternes));
        for (id, valeur) in [
            ("tmdb", "cle-tmdb-secrete"),
            ("trakt", "client-trakt-secret"),
        ] {
            etat.appliquer(Message::Dialogue(ActionDialogue::CleSaisie(
                id.into(),
                Secret::new(valeur),
            )));
            etat.appliquer(Message::Dialogue(ActionDialogue::CleEnregistrer(id.into())));
        }
        etat.appliquer(Message::Dialogue(ActionDialogue::CleSaisie(
            "omdb".into(),
            Secret::new("saisie-en-cours-secrete"),
        )));
        etat
    }

    /// Une ligne par service, boutons et liens présents, aucun secret dessiné, aucun
    /// « bientôt disponible » hors TheTVDB et Simkl, taille stable à 1914×1026 et 1366×768.
    #[test]
    fn page_services_externes_sept_lignes_et_taille_stable() {
        let etat = etat_services();
        let i = &etat.i18n;
        for ecran in [Vec2::new(1914.0, 1026.0), Vec2::new(1366.0, 768.0)] {
            let (textes, r4, sortie) = rendre(&etat, ecran, 4);
            let (_, r20, _) = rendre(&etat, ecran, 20);
            assert_eq!(r4.size(), r20.size(), "taille instable à {ecran:?}");
            assert!(Rect::from_min_size(Pos2::ZERO, ecran).contains_rect(r20));
            assert!(sortie.is_empty(), "{sortie:?}");
            let nombre = |t: &str| textes.iter().filter(|(x, _)| x == t).count();
            // La page défile dans la fenêtre : seules les premières lignes sont visibles.
            for nom in ["TMDB", "Fanart.tv", "TheTVDB"] {
                assert_eq!(nombre(nom), 1, "{nom} à {ecran:?}");
            }
            assert!(nombre(&i.t("gui.settings.keys.title")) == 1);
            // Contenu de la page visible : au moins les boutons du début (la page défile).
            assert!(nombre(&i.t("Button.save")) >= 2, "{ecran:?}");
            assert!(nombre(&i.t("gui.tools.test")) >= 2);
            assert!(nombre(&i.t("Button.remove")) >= 2);
            assert!(
                textes
                    .iter()
                    .any(|(t, _)| t.contains("https://www.themoviedb.org/settings/api"))
            );
            // Les secrets (enregistrés ou en cours de saisie) ne sont jamais dessinés.
            for (t, _) in &textes {
                assert!(
                    !t.contains("secrete") && !t.contains("secret-"),
                    "secret affiché : {t}"
                );
                assert!(!t.contains("???"), "traduction manquante : {t}");
            }
        }
    }

    /// Les sept services sont dessinés (page entière, sans défilement) avec leurs trois boutons,
    /// leur lien, et seuls TheTVDB et Simkl portent la mention « Bientôt disponible ».
    #[test]
    fn page_services_dessine_les_sept_services() {
        let etat = etat_services();
        let Some(Dialogue::Reglages(d)) = &etat.dialogue else {
            panic!("dialogue attendu");
        };
        let ctx = egui::Context::default();
        ctx.set_visuals(crate::theme::visuals(etat.reglages.theme));
        let entree = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 4000.0))),
            ..Default::default()
        };
        let mut sortie = Vec::new();
        let mut hauteurs = Vec::new();
        let mut textes = Vec::new();
        for _ in 0..4 {
            sortie.clear();
            let mut out = ctx.run_ui(entree.clone(), |ui| {
                let r = egui::CentralPanel::default().show(ui, |ui| {
                    reglages_pages::page_services(ui, &etat, d, &etat.services, &mut sortie);
                });
                hauteurs.push(r.response.rect.height());
            });
            out.textures_delta.clear();
            textes = out
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
                    _ => None,
                })
                .collect();
        }
        assert!(sortie.is_empty(), "{sortie:?}");
        assert_eq!(hauteurs[2], hauteurs[3], "hauteur instable");
        let i = &etat.i18n;
        let nombre = |t: &str| textes.iter().filter(|x| x.as_str() == t).count();
        for nom in [
            "TMDB",
            "Fanart.tv",
            "TheTVDB",
            "OMDb",
            "Trakt",
            "MDBList",
            "Simkl",
        ] {
            assert_eq!(nombre(nom), 1, "{nom}");
        }
        assert_eq!(nombre(&i.t("Button.save")), 7);
        assert_eq!(nombre(&i.t("Button.remove")), 7);
        assert_eq!(nombre(&i.t("gui.tools.test")), 7);
        assert_eq!(nombre(&i.t("gui.settings.keys.getkey")), 7);
        // Clés enregistrées : TMDB et Trakt ; les autres : « aucune clé ».
        assert_eq!(
            nombre(&format!(
                "{} {}",
                crate::polices::COCHE,
                i.t("gui.settings.keys.stored")
            )),
            2
        );
        assert_eq!(nombre(&i.t("gui.settings.keys.missing")), 5);
        assert_eq!(nombre(&i.t("gui.settings.soon")), 2);
        assert_eq!(nombre(&i.t("gui.settings.keys.test.unavailable")), 1);
        // Indications des champs vides : Trakt et Simkl (identifiant client), 4 autres clés
        // (OMDb a une saisie en cours, donc pas d'indication).
        assert_eq!(nombre(&i.t("gui.settings.keys.field.id")), 2);
        assert_eq!(nombre(&i.t("gui.settings.keys.field.key")), 4);
        assert!(
            textes
                .iter()
                .all(|t| !t.contains("secrete") && !t.contains("???"))
        );
    }

    /// Chaque service a un texte d'usage, un lien de clé et ses traductions fr / en.
    #[test]
    fn services_textes_liens_et_traductions() {
        use crate::i18n::analyser;
        use crate::source_donnees::{
            cle_testable, est_identifiant_client, lien_obtention_cle, services_cles_connus,
        };
        let fr = analyser(include_str!("../../i18n/fr.properties"));
        let en = analyser(include_str!("../../i18n/en.properties"));
        for c in services_cles_connus() {
            let cle = format!("gui.settings.service.{}.usage", c.id);
            assert!(fr.contains_key(&cle) && en.contains_key(&cle), "{cle}");
            assert!(
                lien_obtention_cle(&c.id).starts_with("https://"),
                "{}",
                c.id
            );
        }
        assert!(!cle_testable("tvdb") && cle_testable("omdb"));
        assert!(est_identifiant_client("trakt") && est_identifiant_client("Simkl"));
        assert!(!est_identifiant_client("omdb"));
        assert_eq!(lien_obtention_cle("inconnu"), "");
        for cle in ["gui.cmd.fetchtrailers", "gui.cmd.downloadtrailers"] {
            assert!(fr.contains_key(cle) && en.contains_key(cle), "{cle}");
        }
    }

    /// Page Films / Bandes-annonces : état de yt-dlp (détecté ou non), bouton Tester, rappel de la
    /// qualité ; taille stable.
    #[test]
    fn page_bandes_annonces_etat_de_yt_dlp() {
        let mut etat = Etat::nouveau();
        etat.appliquer(Message::OuvrirReglagesPage(Page::FilmsBandesAnnonces));
        let i = &etat.i18n;
        for ecran in [Vec2::new(1914.0, 1026.0), Vec2::new(1366.0, 768.0)] {
            let (textes, r4, sortie) = rendre(&etat, ecran, 4);
            let (_, r20, _) = rendre(&etat, ecran, 20);
            assert_eq!(r4.size(), r20.size(), "{ecran:?}");
            assert!(sortie.is_empty());
            let present = |t: &str| textes.iter().any(|(x, _)| x == t);
            assert!(present(&i.t("gui.settings.trailer.ytdlp")));
            assert!(present(&i.t("gui.settings.trailer.ytdlp.found")));
            assert!(present(&i.t("gui.tools.test")));
            let qualite = i.t("gui.settings.trailer.hd720");
            assert!(
                textes
                    .iter()
                    .any(|(t, _)| t.contains(&format!("{qualite} au plus")))
            );
        }
        // yt-dlp introuvable : avertissement et explication.
        if let Some(Dialogue::Reglages(d)) = &mut etat.dialogue {
            d.yt_dlp_detecte = false;
        }
        let (textes, _, _) = rendre(&etat, Vec2::new(1366.0, 768.0), 4);
        assert!(
            textes
                .iter()
                .any(|(t, _)| t.contains("yt-dlp est introuvable"))
        );
        assert!(
            textes
                .iter()
                .any(|(t, _)| t.contains("Paramètres › Outils externes"))
        );
        // La page des séries ne propose pas le téléchargement.
        etat.appliquer(Message::OuvrirReglagesPage(Page::SeriesBandesAnnonces));
        let (textes, _, _) = rendre(&etat, Vec2::new(1366.0, 768.0), 4);
        assert!(
            !textes
                .iter()
                .any(|(t, _)| t == &etat.i18n.t("gui.settings.trailer.ytdlp"))
        );
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
            ServicesExternes => &["gui.settings.keys.title"],
            Systeme => &["Settings.proxy", "Settings.proxy.host"],
            Ard => &["gui.settings.soon", "gui.settings.soon.ard"],
            Divers => &["Settings.imagecache"],
            Films => &["gui.settings.section.tasks", "Settings.renamer.afterscrape"],
            Series => &["gui.settings.section.tasks"],
            FilmsInterface | SeriesInterface => &[
                "gui.settings.table.columns",
                "gui.settings.table.display",
                "gui.settings.soon",
                "gui.settings.soon.missing",
            ],
            FilmsSource | SeriesSource => &["Settings.folders"],
            FilmsNfo | SeriesNfo => &["gui.settings.nfo.format"],
            FilmsExtracteur | SeriesExtracteur => &["gui.settings.scraper.language"],
            FilmsIllustrations | SeriesIllustrations => &[
                "gui.settings.art.best",
                "gui.settings.art.postercount",
                "gui.settings.art.fanartcount",
                "gui.settings.art.actors",
            ],
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
