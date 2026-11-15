//! Application eframe : coque de l'interface (barres, panneaux, dialogues, raccourcis, thèmes,
//! images, persistance de l'interface à la fermeture).

use crate::commandes::Commande;
use crate::etat::{Dialogue, Etat, Message, Module, Niveau};
use crate::images::GestionnaireImages;
use crate::persistance::{self, EtatInterface, GeometrieFenetre};
use crate::reglages::Theme;
use crate::source_donnees::SourceDonnees;
use crate::taches::Repaint;
use crate::theme;
use crate::vue::{
    self, Sortie, barre_etat, barre_menus, barre_modules, barre_outils, collections, details,
    series, tableau,
};
use eframe::egui::{
    self, CentralPanel, CursorIcon, Event, Frame, Id, Key, Margin, Rect, Sense, UiBuilder,
    ViewportCommand, pos2, vec2,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// Largeur de la poignée du séparateur liste / détails.
pub const LARGEUR_POIGNEE: f32 = 9.0;
/// Largeur minimale du panneau de détails.
pub const MIN_DETAILS: f32 = 320.0;
/// Largeur minimale de la liste.
pub const MIN_LISTE: f32 = 360.0;
const ID_SEPARATEUR: &str = "separateur_liste_details";

/// Largeur effective du panneau de détails : valeur mémorisée (moitié de la zone sinon),
/// bornée pour laisser au moins `MIN_LISTE` à la liste.
pub fn borner_largeur_details(largeur_zone: f32, voulue: Option<f32>) -> f32 {
    let max = (largeur_zone - MIN_LISTE - LARGEUR_POIGNEE).max(MIN_DETAILS);
    voulue
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(largeur_zone * 0.5)
        .clamp(MIN_DETAILS, max)
}

/// Découpe `zone` en (liste, poignée, détails) pour une largeur de détails donnée.
pub fn decouper(zone: Rect, largeur_details: f32) -> (Rect, Rect, Rect) {
    let x_poignee = zone.max.x - largeur_details - LARGEUR_POIGNEE;
    let liste = Rect::from_min_max(zone.min, pos2(x_poignee, zone.max.y));
    let poignee = Rect::from_min_max(
        pos2(x_poignee, zone.min.y),
        pos2(x_poignee + LARGEUR_POIGNEE, zone.max.y),
    );
    let details = Rect::from_min_max(pos2(poignee.max.x, zone.min.y), zone.max);
    (liste, poignee, details)
}

/// Poignée verticale déplaçable : renvoie la largeur des détails pour cette image (et émet
/// `LargeurDetails` pendant le glissement, mémorisée dans `ui.json`).
fn separateur(ui: &mut egui::Ui, etat: &Etat, zone: Rect, sortie: &mut Sortie) -> f32 {
    let mut largeur = borner_largeur_details(zone.width(), etat.largeur_details);
    let (_, poignee, _) = decouper(zone, largeur);
    let reponse = ui
        .interact(poignee, Id::unique(ID_SEPARATEUR), Sense::drag())
        .on_hover_cursor(CursorIcon::ResizeHorizontal);
    if reponse.dragged()
        && let Some(p) = reponse.interact_pointer_pos()
    {
        largeur =
            borner_largeur_details(zone.width(), Some(zone.max.x - p.x - LARGEUR_POIGNEE * 0.5));
    }
    if (reponse.dragged() || reponse.drag_stopped())
        && etat
            .largeur_details
            .is_none_or(|x| (x - largeur).abs() > 0.5)
    {
        sortie.push(Message::LargeurDetails(largeur));
    }
    let (_, poignee, _) = decouper(zone, largeur);
    let peintre = ui.painter();
    peintre.rect_filled(poignee, 0.0, theme::FOND_BARRES);
    let couleur = if reponse.hovered() || reponse.dragged() {
        egui::Color32::WHITE
    } else {
        theme::POIGNEE
    };
    // Poignée « ⋮ » : trois points centrés.
    for k in [-1.0, 0.0, 1.0] {
        peintre.circle_filled(poignee.center() + vec2(0.0, 5.0 * k), 1.3, couleur);
    }
    largeur
}

pub struct Application {
    etat: Etat,
    theme_applique: Option<Theme>,
    images: GestionnaireImages,
    chemin_ui: PathBuf,
    fermeture_envoyee: bool,
}

impl Application {
    pub fn nouvelle(
        cc: &eframe::CreationContext<'_>,
        source: Arc<dyn SourceDonnees>,
        chemin_ui: PathBuf,
        etat_ui: EtatInterface,
        erreur_ui: Option<String>,
    ) -> Self {
        let mut etat = Etat::avec_source(source);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let ctx = cc.egui_ctx.clone();
        let repaint: Repaint = Arc::new(move || ctx.request_repaint());
        etat.definir_repaint(repaint.clone());
        etat.restaurer_interface(&etat_ui);
        if etat.reglages.verifier_mises_a_jour {
            etat.appliquer(Message::RechercherMiseAJour { manuel: false });
        }
        if let Some(e) = erreur_ui {
            let texte = etat
                .i18n
                .t_args("gui.ui.loaderror", &[("error", e.as_str())]);
            etat.appliquer(Message::Notifier(Niveau::Avertissement, texte));
        }
        Self {
            etat,
            theme_applique: None,
            images: GestionnaireImages::new(repaint),
            chemin_ui,
            fermeture_envoyee: false,
        }
    }

    /// Écrit `ui.json` (atomique). Appelé à la fermeture.
    fn enregistrer_interface(&mut self) {
        let instantane = self.etat.instantane_interface();
        if let Err(e) = persistance::enregistrer(&self.chemin_ui, &instantane) {
            eprintln!(
                "ui.json non enregistré ({}) : {e}",
                self.chemin_ui.display()
            );
        }
    }

    /// Relève la géométrie de la fenêtre (taille hors maximisation) et gère la demande de fermeture.
    fn suivre_fenetre(&self, ctx: &egui::Context, sortie: &mut Sortie) {
        let (rect, maximisee, fermeture) = ctx.input(|i| {
            let v = i.viewport();
            (
                v.inner_rect,
                v.maximized.unwrap_or(false),
                v.close_requested(),
            )
        });
        let mut g = self.etat.fenetre;
        g.maximisee = maximisee;
        if let Some(r) = rect
            && !maximisee
        {
            g.largeur = r.width();
            g.hauteur = r.height();
        }
        if g != self.etat.fenetre && !ctx.input(|i| i.pointer.any_down()) {
            sortie.push(Message::Fenetre(GeometrieFenetre { ..g }));
        }
        if fermeture && !self.etat.quitter_autorise() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            sortie.push(Message::DemandeQuitter);
        }
    }

    fn appliquer_theme(&mut self, ctx: &egui::Context) {
        let theme_voulu = self.etat.reglages.theme;
        if self.theme_applique != Some(theme_voulu) {
            ctx.set_visuals(theme::visuals(theme_voulu));
            self.theme_applique = Some(theme_voulu);
        }
    }

    /// Raccourcis clavier globaux ; ignorés tant qu'un dialogue modal est affiché.
    fn raccourcis(&self, ctx: &egui::Context, sortie: &mut Sortie) {
        if self.etat.dialogue.is_some() || self.etat.visionneuse.is_some() {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, Key::F)) {
            sortie.push(Message::FocusRecherche);
        }
        // Les combinaisons les plus spécifiques (Ctrl+Maj) d'abord.
        for commande in Commande::TOUTES {
            if let Some(raccourci) = commande.raccourci_egui() {
                let libre = ctx.egui_wants_keyboard_input() && raccourci.modifiers.is_none();
                if !libre && ctx.input_mut(|i| i.consume_shortcut(&raccourci)) {
                    sortie.push(Message::Commande(commande));
                }
            }
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (haut, bas, page_haut, page_bas, debut, fin, entree, maj, tout, textes) =
            ctx.input(|i| {
                let textes: Vec<String> = i
                    .events
                    .iter()
                    .filter_map(|e| match e {
                        Event::Text(t) if !t.trim().is_empty() => Some(t.clone()),
                        _ => None,
                    })
                    .collect();
                (
                    i.key_pressed(Key::ArrowUp),
                    i.key_pressed(Key::ArrowDown),
                    i.key_pressed(Key::PageUp),
                    i.key_pressed(Key::PageDown),
                    i.key_pressed(Key::Home),
                    i.key_pressed(Key::End),
                    i.key_pressed(Key::Enter),
                    i.modifiers.shift,
                    i.modifiers.command && i.key_pressed(Key::A),
                    textes,
                )
            });
        let deplacements = [
            (haut, -1),
            (bas, 1),
            (page_haut, -15),
            (page_bas, 15),
            (debut, i32::MIN / 2),
            (fin, i32::MAX / 2),
        ];
        for (actif, delta) in deplacements {
            if actif {
                sortie.push(Message::Deplacer {
                    delta,
                    etendre: maj,
                });
            }
        }
        if tout {
            sortie.push(Message::SelectionnerTout);
        }
        if entree {
            sortie.push(Message::Commande(Commande::Editer));
        }
        for texte in textes {
            sortie.push(Message::Frappe {
                texte,
                instant: Instant::now(),
            });
        }
    }
}

impl eframe::App for Application {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.dessiner(ui);
    }

    fn on_exit(&mut self) {
        self.enregistrer_interface();
    }
}

impl Application {
    /// Une image complète : messages des tâches, dessin, application des messages émis.
    fn dessiner(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.etat.drainer();
        self.images.recevoir(&ctx);
        if std::mem::take(&mut self.etat.vider_cache_images) {
            self.images.vider();
        }
        if std::mem::take(&mut self.etat.focus_recherche) {
            ctx.memory_mut(|m| m.request_focus(Id::unique(tableau::ID_CHAMP_RECHERCHE)));
        }
        self.appliquer_theme(&ctx);
        let mut sortie: Sortie = Vec::new();
        self.suivre_fenetre(&ctx, &mut sortie);
        self.raccourcis(&ctx, &mut sortie);

        barre_menus::afficher(ui, &self.etat, &mut sortie);
        barre_outils::afficher(ui, &self.etat, &mut sortie);
        barre_etat::afficher(ui, &self.etat, &mut sortie);
        barre_modules::afficher(ui, &self.etat, &mut sortie);

        let fond = theme::fond_alterne(self.etat.reglages.theme);
        CentralPanel::default()
            .frame(Frame::new().fill(theme::FOND_BARRES).inner_margin(Margin {
                left: 18,
                right: 8,
                top: 0,
                bottom: 6,
            }))
            .show(ui, |ui| {
                let zone = ui.available_rect_before_wrap();
                ui.allocate_rect(zone, Sense::hover());
                let largeur = separateur(ui, &self.etat, zone, &mut sortie);
                let (liste, _, details) = decouper(zone, largeur);
                for (rect, est_liste) in [(liste, true), (details, false)] {
                    ui.painter().rect_filled(rect, 0.0, fond);
                    let mut enfant =
                        ui.new_child(UiBuilder::new().max_rect(rect).id_salt(if est_liste {
                            "zone_liste"
                        } else {
                            "zone_details"
                        }));
                    enfant.set_clip_rect(rect.intersect(ui.clip_rect()));
                    match (self.etat.module, est_liste) {
                        (Module::Series, true) => {
                            series::afficher(&mut enfant, &self.etat, &mut sortie)
                        }
                        (Module::Series, false) => series::afficher_details(
                            &mut enfant,
                            &self.etat,
                            &mut self.images,
                            &mut sortie,
                        ),
                        (Module::Collections, true) => {
                            collections::afficher(&mut enfant, &self.etat, &mut sortie)
                        }
                        (Module::Collections, false) => collections::afficher_details(
                            &mut enfant,
                            &self.etat,
                            &mut self.images,
                            &mut sortie,
                        ),
                        (_, true) => tableau::afficher(&mut enfant, &self.etat, &mut sortie),
                        (_, false) => details::afficher(
                            &mut enfant,
                            &self.etat,
                            &mut self.images,
                            &mut sortie,
                        ),
                    }
                }
            });

        if self.etat.filtres_ouverts {
            vue::dialogue_filtres::afficher(&ctx, &self.etat, &mut sortie);
        }

        match &self.etat.dialogue {
            Some(Dialogue::ApercuRenommage(d)) => {
                vue::dialogue_renommage::afficher(&ctx, &self.etat, d, &mut sortie)
            }
            Some(Dialogue::Reglages(d)) => {
                vue::dialogue_reglages::afficher(&ctx, &self.etat, d, &mut sortie)
            }
            Some(Dialogue::Messages) => {
                vue::dialogue_messages::afficher(&ctx, &self.etat, &mut sortie)
            }
            Some(Dialogue::Editeur(d)) => {
                vue::dialogue_editeur::afficher(&ctx, &self.etat, &mut self.images, d, &mut sortie)
            }
            Some(Dialogue::Recherche(d)) => vue::dialogue_recherche::afficher(
                &ctx,
                &self.etat,
                &mut self.images,
                d,
                &mut sortie,
            ),
            Some(Dialogue::EditeurSerie(d)) => {
                vue::dialogue_editeur_serie::afficher(&ctx, &self.etat, d, &mut sortie)
            }
            Some(Dialogue::Manquants(d)) => {
                vue::dialogue_manquants::afficher(&ctx, &self.etat, d, &mut sortie)
            }
            Some(Dialogue::Confirmation(d)) => {
                vue::dialogue_confirmation::afficher(&ctx, &self.etat, d, &mut sortie)
            }
            Some(Dialogue::Assistant(d)) => {
                vue::dialogue_assistant::afficher(&ctx, &self.etat, d, &mut sortie)
            }
            Some(Dialogue::MiseAJour(d)) => {
                vue::dialogue_mise_a_jour::afficher(&ctx, &self.etat, d, &mut sortie)
            }
            None => {}
        }

        if let Some(chemin) = self.etat.visionneuse.clone() {
            vue::visionneuse::afficher(&ctx, &self.etat, &mut self.images, &chemin, &mut sortie);
        }

        self.etat.scroll_vers = None;
        for message in sortie {
            self.etat.appliquer(message);
        }
        if self.etat.quitter && !self.fermeture_envoyee {
            self.fermeture_envoyee = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commandes::Commande;
    use crate::etat::TypeDialogue;
    use crate::reglages::Page;

    /// Rendu hors écran de chaque écran / dialogue : aucune panique, messages appliqués.
    fn rendre_tous_les_ecrans(etat: Etat, dossier: &std::path::Path) {
        for largeur in [1050.0, 1920.0] {
            rendre_tous_les_ecrans_en(etat_copie(&etat), dossier, largeur);
        }
        rendre_tous_les_ecrans_en(etat, dossier, 1280.0);
    }

    /// Nouvel état sur la même source (pour rejouer les écrans à une autre largeur).
    fn etat_copie(etat: &Etat) -> Etat {
        let mut e = Etat::avec_source(etat.source.clone());
        e.films = etat.films.clone();
        e.series = etat.series.clone();
        e.appliquer(Message::Recherche(String::new()));
        e
    }

    fn entree(largeur: f32) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(largeur, 760.0))),
            ..Default::default()
        }
    }

    fn rendre_tous_les_ecrans_en(etat: Etat, dossier: &std::path::Path, largeur: f32) {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let repaint: Repaint = Arc::new(|| {});
        let mut app = Application {
            etat,
            theme_applique: None,
            images: GestionnaireImages::new(repaint),
            chemin_ui: dossier.join("ui.json"),
            fermeture_envoyee: false,
        };
        let image = |app: &mut Application| {
            let mut sortie = ctx.run_ui(entree(largeur), |ui| app.dessiner(ui));
            sortie.textures_delta.clear();
        };
        let mut etapes: Vec<Message> = vec![
            Message::OuvrirDialogue(TypeDialogue::Assistant),
            Message::FermerDialogue,
            Message::OuvrirFiltres(true),
            Message::Commande(Commande::Editer),
            Message::FermerDialogue,
            Message::Commande(Commande::ScrapeInteractif),
            Message::FermerDialogue,
            Message::Commande(Commande::RetirerBase),
            Message::FermerDialogue,
            Message::Commande(Commande::ApercuRenommage),
            Message::FermerDialogue,
            Message::Commande(Commande::TrouverManquants),
            Message::FermerDialogue,
        ];
        for page in [
            Page::General,
            Page::Systeme,
            Page::ServicesExternes,
            Page::Divers,
            Page::FilmsSource,
            Page::FilmsExtracteur,
            Page::FilmsRenommeur,
            Page::Series,
            Page::OutilsExternes,
        ] {
            etapes.push(Message::OuvrirReglagesPage(page));
        }
        etapes.extend([
            Message::FermerDialogue,
            Message::OuvrirDialogue(TypeDialogue::Messages),
            Message::FermerDialogue,
            Message::ChoisirModule(crate::etat::Module::Series),
            Message::BasculerNoeudSerie("inexistante".into(), None),
            Message::SelectionnerNoeudSerie(crate::etat::NoeudSerie::Serie("inexistante".into())),
            Message::ChoisirOnglet(crate::etat::OngletDetails::FichiersMedias),
            Message::Commande(Commande::Editer),
            Message::Commande(Commande::ScrapeInteractif),
            Message::FermerDialogue,
            Message::Commande(Commande::TrouverManquants),
            Message::FermerDialogue,
            Message::Commande(Commande::ApercuRenommage),
            Message::FermerDialogue,
            Message::Commande(Commande::RetirerBase),
            Message::FermerDialogue,
            Message::ChoisirModule(crate::etat::Module::Collections),
            Message::SelectionnerNoeudSaga(crate::collections::NoeudSaga::Saga(
                "demo-saga-dune".into(),
            )),
            Message::BasculerSaga("demo-saga-dune".into()),
            Message::SelectionnerNoeudSaga(crate::collections::NoeudSaga::Film(
                "demo-saga-dune".into(),
                3,
            )),
            Message::ChoisirOnglet(crate::etat::OngletDetails::Distribution),
            Message::SelectionnerNoeudSaga(crate::collections::NoeudSaga::Saga(
                "inexistante".into(),
            )),
            Message::ChoisirModule(crate::etat::Module::Films),
            Message::ChoisirOnglet(crate::etat::OngletDetails::Distribution),
            Message::ChoisirOnglet(crate::etat::OngletDetails::FichiersMedias),
            Message::ChoisirOnglet(crate::etat::OngletDetails::BandeAnnonce),
            Message::ColonneVisible(crate::modele::Colonne::TailleFichier, true),
            Message::ColonneVisible(crate::modele::Colonne::Chemin, true),
            Message::ColonneVisible(crate::modele::Colonne::DateAjout, true),
            Message::ColonneVisible(crate::modele::Colonne::FormatVideo, true),
            Message::Trier {
                colonne: crate::modele::Colonne::TailleFichier,
                additif: false,
            },
            Message::ChoisirOnglet(crate::etat::OngletDetails::Illustration),
            Message::OuvrirVisionneuse(dossier.join("absente.png")),
        ]);
        image(&mut app);
        for m in etapes {
            app.etat.appliquer(m);
            image(&mut app);
            image(&mut app);
        }
        app.enregistrer_interface();
        let relu = persistance::charger(&dossier.join("ui.json")).unwrap();
        assert_eq!(relu, app.etat.instantane_interface());
    }

    #[test]
    fn largeur_des_details_bornee() {
        assert_eq!(borner_largeur_details(1000.0, None), 500.0);
        assert_eq!(borner_largeur_details(1000.0, Some(100.0)), MIN_DETAILS);
        assert_eq!(
            borner_largeur_details(1000.0, Some(900.0)),
            1000.0 - MIN_LISTE - LARGEUR_POIGNEE
        );
        assert_eq!(borner_largeur_details(1000.0, Some(f32::NAN)), 500.0);
        // Zone trop étroite : les détails gardent leur minimum.
        assert_eq!(borner_largeur_details(500.0, Some(450.0)), MIN_DETAILS);
        let zone = Rect::from_min_size(pos2(10.0, 0.0), vec2(1000.0, 500.0));
        let (liste, poignee, details) = decouper(zone, 400.0);
        assert_eq!(details.width(), 400.0);
        assert_eq!(poignee.width(), LARGEUR_POIGNEE);
        assert_eq!(liste.min.x, 10.0);
        assert_eq!(liste.max.x, poignee.min.x);
        assert_eq!(poignee.max.x, details.min.x);
    }

    /// Glisser la poignée à la souris change la largeur des détails, mémorisée dans ui.json.
    #[test]
    fn separateur_deplacable_et_largeur_persistee() {
        let dossier = tempfile::tempdir().unwrap();
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let mut app = Application {
            etat: Etat::nouveau(),
            theme_applique: None,
            images: GestionnaireImages::new(Arc::new(|| {})),
            chemin_ui: dossier.path().join("ui.json"),
            fermeture_envoyee: false,
        };
        let largeur = 1400.0;
        let image = |app: &mut Application, evenements: Vec<Event>| {
            let mut e = entree(largeur);
            e.events = evenements;
            let mut s = ctx.run_ui(e, |ui| app.dessiner(ui));
            s.textures_delta.clear();
        };
        image(&mut app, vec![]);
        image(&mut app, vec![]);
        assert_eq!(app.etat.largeur_details, None);
        // Position de la poignée : relevée sur la zone d'interaction enregistrée.
        let rect = ctx
            .read_response(Id::unique(ID_SEPARATEUR))
            .expect("poignée enregistrée")
            .rect;
        let depart = rect.center();
        let arrivee = depart - vec2(150.0, 0.0);
        let bouton = |pos, pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        image(&mut app, vec![Event::PointerMoved(depart)]);
        image(&mut app, vec![bouton(depart, true)]);
        for k in 1..=5 {
            let p = depart - vec2(30.0 * k as f32, 0.0);
            image(&mut app, vec![Event::PointerMoved(p)]);
        }
        image(&mut app, vec![bouton(arrivee, false)]);
        image(&mut app, vec![]);
        let l = app.etat.largeur_details.expect("largeur mémorisée");
        let avant = largeur - rect.max.x - 8.0;
        assert!(
            (l - (avant + 150.0)).abs() < 12.0,
            "largeur {l}, attendu environ {}",
            avant + 150.0
        );
        app.enregistrer_interface();
        let relu = persistance::charger(&dossier.path().join("ui.json")).unwrap();
        assert_eq!(relu.separateur, Some(l));
        // Restauration : la poignée revient à la même place.
        let mut restaure = Etat::nouveau();
        restaure.restaurer_interface(&relu);
        assert_eq!(restaure.largeur_details, Some(l));
    }

    #[test]
    fn rendu_de_tous_les_ecrans_sans_panique() {
        let dossier = tempfile::tempdir().unwrap();
        rendre_tous_les_ecrans(Etat::nouveau(), dossier.path());
    }

    #[test]
    fn rendu_de_tous_les_ecrans_avec_la_source_reelle() {
        let d = tempfile::tempdir().unwrap();
        let source = crate::source_application::tests::source_reelle(d.path());
        let mut etat = Etat::avec_source(source.clone());
        etat.appliquer(Message::Commande(Commande::MettreAJourToutesSources));
        crate::source_application::tests::attendre(&mut etat, |e| {
            e.films.len() == 2 && e.taches.is_empty()
        });
        rendre_tous_les_ecrans(etat, d.path());
        source.arreter().unwrap();
    }
}
