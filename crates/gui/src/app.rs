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
    self, Sortie, barre_etat, barre_menus, barre_modules, barre_outils, details, series, tableau,
};
use eframe::egui::{self, CentralPanel, Event, Frame, Id, Key, Margin, Panel, ViewportCommand};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

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
        let ctx = cc.egui_ctx.clone();
        let repaint: Repaint = Arc::new(move || ctx.request_repaint());
        etat.definir_repaint(repaint.clone());
        etat.restaurer_interface(&etat_ui);
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
        let relache = ctx.input(|i| !i.pointer.any_down());
        CentralPanel::default()
            .frame(Frame::new().fill(fond).inner_margin(Margin::same(6)))
            .show(ui, |ui| match self.etat.module {
                Module::Films => {
                    let largeur = ui.available_width();
                    let defaut = self.etat.largeur_details.unwrap_or(largeur * 0.5);
                    let reponse = Panel::right("panneau_details")
                        .resizable(true)
                        .default_size(defaut.clamp(300.0, (largeur * 0.8).max(300.0)))
                        .size_range(300.0..=(largeur * 0.8).max(300.0))
                        .frame(
                            Frame::new()
                                .fill(ui.visuals().panel_fill)
                                .inner_margin(Margin::same(8)),
                        )
                        .show(ui, |ui| {
                            details::afficher(ui, &self.etat, &mut self.images, &mut sortie)
                        });
                    let l = reponse.response.rect.width();
                    if relache
                        && self
                            .etat
                            .largeur_details
                            .is_none_or(|x| (x - l).abs() > 1.0)
                    {
                        sortie.push(Message::LargeurDetails(l));
                    }
                    CentralPanel::default()
                        .frame(Frame::new().inner_margin(Margin::symmetric(4, 0)))
                        .show(ui, |ui| tableau::afficher(ui, &self.etat, &mut sortie));
                }
                Module::Series => series::afficher(ui, &self.etat, &mut sortie),
                autre => {
                    let nom = self.etat.i18n.t(autre.cle());
                    ui.add_space(24.0);
                    ui.label(
                        self.etat
                            .i18n
                            .t_args("gui.module.placeholder", &[("module", nom.as_str())]),
                    );
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
            Some(Dialogue::Confirmation(d)) => {
                vue::dialogue_confirmation::afficher(&ctx, &self.etat, d, &mut sortie)
            }
            Some(Dialogue::Assistant(d)) => {
                vue::dialogue_assistant::afficher(&ctx, &self.etat, d, &mut sortie)
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
        let ctx = egui::Context::default();
        let repaint: Repaint = Arc::new(|| {});
        let mut app = Application {
            etat,
            theme_applique: None,
            images: GestionnaireImages::new(repaint),
            chemin_ui: dossier.join("ui.json"),
            fermeture_envoyee: false,
        };
        let image = |app: &mut Application| {
            let mut sortie = ctx.run_ui(Default::default(), |ui| app.dessiner(ui));
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
        ] {
            etapes.push(Message::OuvrirReglagesPage(page));
        }
        etapes.extend([
            Message::FermerDialogue,
            Message::OuvrirDialogue(TypeDialogue::Messages),
            Message::FermerDialogue,
            Message::ChoisirModule(crate::etat::Module::Series),
            Message::ChoisirModule(crate::etat::Module::Collections),
            Message::ChoisirModule(crate::etat::Module::Films),
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
