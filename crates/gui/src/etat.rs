//! Modèle d'état (`Etat`), messages et logique de mise à jour (sans aucune dépendance egui :
//! entièrement testable). La vue lit un `&Etat` et émet des `Message`.

use crate::commandes::Commande;
use crate::donnees::films_factices;
use crate::i18n::I18n;
use crate::modele::{Colonne, ColonneEtat, Film, Ordre, colonnes_par_defaut, normaliser};
use crate::reglages::{Page, Reglages};
use crate::renommage::{self, Profil};
use crate::taches::{Repaint, Tache, lancer_simulation};
use regex::Regex;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

/// Fenêtre de la recherche incrémentale au clavier (500 ms comme l'original).
pub const DELAI_FRAPPE: Duration = Duration::from_millis(500);
const MAX_MESSAGES_PAR_IMAGE: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Module {
    Films,
    Collections,
    Series,
}

impl Module {
    pub const TOUS: [Module; 3] = [Module::Films, Module::Collections, Module::Series];

    pub fn cle(self) -> &'static str {
        match self {
            Module::Films => "app.movies",
            Module::Collections => "app.moviesets",
            Module::Series => "app.tvshows",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OngletDetails {
    Details,
    Distribution,
    FichiersMedias,
    Illustration,
    BandeAnnonce,
}

impl OngletDetails {
    pub const TOUS: [OngletDetails; 5] = [
        OngletDetails::Details,
        OngletDetails::Distribution,
        OngletDetails::FichiersMedias,
        OngletDetails::Illustration,
        OngletDetails::BandeAnnonce,
    ];

    pub fn cle(self) -> &'static str {
        match self {
            OngletDetails::Details => "metatag.details",
            OngletDetails::Distribution => "metatag.cast",
            OngletDetails::FichiersMedias => "metatag.mediafiles",
            OngletDetails::Illustration => "metatag.artwork",
            OngletDetails::BandeAnnonce => "metatag.trailer",
        }
    }
}

/// Filtre rapide (le dialogue de filtres avancés viendra plus tard).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FiltreRapide {
    #[default]
    Aucun,
    Vus,
    NonVus,
    Nouveaux,
}

impl FiltreRapide {
    fn accepte(self, f: &Film) -> bool {
        match self {
            FiltreRapide::Aucun => true,
            FiltreRapide::Vus => f.vu,
            FiltreRapide::NonVus => !f.vu,
            FiltreRapide::Nouveaux => f.nouveau,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Niveau {
    Info,
    Avertissement,
    Erreur,
}

#[derive(Clone, Debug)]
pub struct Notification {
    pub niveau: Niveau,
    pub texte: String,
}

#[derive(Clone, Debug)]
pub struct EtatApercu {
    pub ids: Vec<u64>,
    pub profil: usize,
    pub masquer_inchanges: bool,
    pub film_actif: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct EtatDialogueReglages {
    pub page: Page,
    pub filtre: String,
}

#[derive(Clone, Debug)]
pub enum Dialogue {
    ApercuRenommage(EtatApercu),
    Reglages(EtatDialogueReglages),
    Messages,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeDialogue {
    ApercuRenommage,
    Reglages,
    Messages,
}

#[derive(Clone, Debug)]
pub enum ActionDialogue {
    ApercuProfil(usize),
    ApercuMasquer(bool),
    ApercuFilm(u64),
    ReglagesPage(Page),
    ReglagesFiltre(String),
}

/// Messages : seul `Etat::appliquer` modifie l'état.
#[derive(Clone, Debug)]
pub enum Message {
    ChoisirModule(Module),
    ChoisirOnglet(OngletDetails),
    Recherche(String),
    FiltreRapide(FiltreRapide),
    EffacerFiltres,
    Trier { colonne: Colonne, additif: bool },
    Selectionner { id: u64, ctrl: bool, maj: bool },
    SelectionnerTout,
    Deplacer { delta: i32, etendre: bool },
    Frappe { texte: String, instant: Instant },
    ColonneVisible(Colonne, bool),
    Commande(Commande),
    OuvrirDialogue(TypeDialogue),
    FermerDialogue,
    Dialogue(ActionDialogue),
    ApercuAppliquer,
    AppliquerReglages(Box<Reglages>),
    Notifier(Niveau, String),
    MessagesLus,
    ArreterTache(u64),
    TacheProgression { id: u64, fait: u32, total: u32 },
    TacheTerminee { id: u64, annulee: bool },
}

pub struct Etat {
    pub i18n: I18n,
    pub reglages: Reglages,
    pub films: Vec<Film>,
    pub module: Module,
    pub onglet: OngletDetails,
    pub recherche: String,
    pub recherche_invalide: bool,
    regex: Option<Regex>,
    pub filtre_rapide: FiltreRapide,
    pub colonnes: Vec<ColonneEtat>,
    pub tri: Vec<(Colonne, Ordre)>,
    /// Indices (dans `films`) triés ET filtrés.
    pub visibles: Vec<usize>,
    pub selection: BTreeSet<u64>,
    pub ancre: Option<u64>,
    pub principal: Option<u64>,
    /// Ligne vers laquelle faire défiler à la prochaine image (consommé par l'application).
    pub scroll_vers: Option<usize>,
    pub taches: Vec<Tache>,
    pub notifications: Vec<Notification>,
    pub non_lus: usize,
    pub dialogue: Option<Dialogue>,
    tampon_frappe: String,
    derniere_frappe: Option<Instant>,
    tx: Sender<Message>,
    rx: Receiver<Message>,
    repaint: Repaint,
    prochain_id_tache: u64,
    /// Pas de la tâche simulée (réduit dans les tests).
    pas_simulation: Duration,
}

impl Etat {
    pub fn nouveau() -> Self {
        Self::avec_films(films_factices())
    }

    pub fn avec_films(films: Vec<Film>) -> Self {
        let (tx, rx) = channel();
        let reglages = Reglages::default();
        let mut etat = Self {
            i18n: I18n::new(reglages.langue),
            reglages,
            films,
            module: Module::Films,
            onglet: OngletDetails::Details,
            recherche: String::new(),
            recherche_invalide: false,
            regex: None,
            filtre_rapide: FiltreRapide::Aucun,
            colonnes: colonnes_par_defaut(),
            tri: vec![(Colonne::Titre, Ordre::Croissant)],
            visibles: Vec::new(),
            selection: BTreeSet::new(),
            ancre: None,
            principal: None,
            scroll_vers: None,
            taches: Vec::new(),
            notifications: Vec::new(),
            non_lus: 0,
            dialogue: None,
            tampon_frappe: String::new(),
            derniere_frappe: None,
            tx,
            rx,
            repaint: Arc::new(|| {}),
            prochain_id_tache: 1,
            pas_simulation: Duration::from_millis(80),
        };
        etat.recalculer();
        etat
    }

    pub fn definir_repaint(&mut self, repaint: Repaint) {
        self.repaint = repaint;
    }

    // ---------------------------------------------------------------- lecture

    pub fn colonnes_visibles(&self) -> Vec<Colonne> {
        self.colonnes
            .iter()
            .filter(|c| c.visible)
            .map(|c| c.colonne)
            .collect()
    }

    pub fn film(&self, id: u64) -> Option<&Film> {
        self.films.iter().find(|f| f.id == id)
    }

    /// Film affiché dans le panneau de détails (le premier de la sélection).
    pub fn film_principal(&self) -> Option<&Film> {
        self.principal.and_then(|id| self.film(id))
    }

    pub fn films_selectionnes(&self) -> Vec<&Film> {
        self.films
            .iter()
            .filter(|f| self.selection.contains(&f.id))
            .collect()
    }

    pub fn taille_selection(&self) -> u64 {
        self.films_selectionnes()
            .iter()
            .map(|f| f.taille_octets)
            .sum()
    }

    pub fn position(&self, id: u64) -> Option<usize> {
        self.visibles.iter().position(|&i| self.films[i].id == id)
    }

    pub fn filtre_actif(&self) -> bool {
        !self.recherche.is_empty() || self.filtre_rapide != FiltreRapide::Aucun
    }

    /// Tâche affichée dans la barre d'état : une principale d'abord, sinon n'importe laquelle.
    pub fn tache_active(&self) -> Option<&Tache> {
        self.taches
            .iter()
            .find(|t| t.principale)
            .or_else(|| self.taches.first())
    }

    pub fn profil_actif(&self) -> Option<&Profil> {
        self.reglages.profils.get(self.reglages.profil_actif)
    }

    pub fn tri_de(&self, colonne: Colonne) -> Option<(usize, Ordre)> {
        self.tri
            .iter()
            .position(|(c, _)| *c == colonne)
            .map(|p| (p, self.tri[p].1))
    }

    // ---------------------------------------------------------------- canal de tâches

    /// Applique les messages reçus des tâches d'arrière-plan (non bloquant, borné).
    pub fn drainer(&mut self) {
        for _ in 0..MAX_MESSAGES_PAR_IMAGE {
            match self.rx.try_recv() {
                Ok(m) => self.appliquer(m),
                Err(_) => break,
            }
        }
    }

    fn lancer_tache(&mut self, cle_nom: &'static str, total: u32, principale: bool) {
        if principale && self.taches.iter().any(|t| t.principale) {
            let texte = self.i18n.t("gui.task.running");
            self.notifier(Niveau::Avertissement, texte);
            return;
        }
        let id = self.prochain_id_tache;
        self.prochain_id_tache += 1;
        let annulation = Arc::new(AtomicBool::new(false));
        self.taches.push(Tache {
            id,
            cle_nom,
            fait: 0,
            total,
            principale,
            annulation: annulation.clone(),
        });
        lancer_simulation(
            id,
            total,
            self.pas_simulation,
            self.tx.clone(),
            self.repaint.clone(),
            annulation,
        );
    }

    fn notifier(&mut self, niveau: Niveau, texte: String) {
        self.notifications.push(Notification { niveau, texte });
        self.non_lus += 1;
    }

    // ---------------------------------------------------------------- mise à jour

    pub fn appliquer(&mut self, message: Message) {
        match message {
            Message::ChoisirModule(m) => self.module = m,
            Message::ChoisirOnglet(o) => self.onglet = o,
            Message::Recherche(texte) => {
                self.recherche = texte;
                self.compiler_recherche();
                self.recalculer();
            }
            Message::FiltreRapide(f) => {
                self.filtre_rapide = f;
                self.recalculer();
            }
            Message::EffacerFiltres => {
                self.filtre_rapide = FiltreRapide::Aucun;
                self.recherche.clear();
                self.compiler_recherche();
                self.recalculer();
            }
            Message::Trier { colonne, additif } => self.trier(colonne, additif),
            Message::Selectionner { id, ctrl, maj } => self.selectionner(id, ctrl, maj),
            Message::SelectionnerTout => {
                self.selection = self.visibles.iter().map(|&i| self.films[i].id).collect();
            }
            Message::Deplacer { delta, etendre } => self.deplacer(delta, etendre),
            Message::Frappe { texte, instant } => self.frappe(&texte, instant),
            Message::ColonneVisible(col, visible) => {
                // La colonne titre ne peut pas être masquée.
                if let Some(c) = self.colonnes.iter_mut().find(|c| c.colonne == col)
                    && col != Colonne::Titre
                {
                    c.visible = visible;
                }
            }
            Message::Commande(c) => self.executer(c),
            Message::OuvrirDialogue(t) => self.ouvrir_dialogue(t),
            Message::FermerDialogue => self.dialogue = None,
            Message::Dialogue(a) => self.action_dialogue(a),
            Message::ApercuAppliquer => self.appliquer_renommage(),
            Message::AppliquerReglages(r) => {
                if r.langue != self.reglages.langue {
                    self.i18n = I18n::new(r.langue);
                }
                self.reglages = *r;
            }
            Message::Notifier(niveau, texte) => self.notifier(niveau, texte),
            Message::MessagesLus => self.non_lus = 0,
            Message::ArreterTache(id) => {
                if let Some(t) = self.taches.iter().find(|t| t.id == id) {
                    t.annuler();
                }
            }
            Message::TacheProgression { id, fait, total } => {
                if let Some(t) = self.taches.iter_mut().find(|t| t.id == id) {
                    t.fait = fait;
                    t.total = total;
                }
            }
            Message::TacheTerminee { id, annulee } => {
                if let Some(pos) = self.taches.iter().position(|t| t.id == id) {
                    let tache = self.taches.remove(pos);
                    let nom = self.i18n.t(tache.cle_nom);
                    let cle = if annulee {
                        "gui.task.cancelled"
                    } else {
                        "gui.task.done"
                    };
                    let texte = self.i18n.t_args(cle, &[("task", nom.as_str())]);
                    self.notifier(
                        if annulee {
                            Niveau::Avertissement
                        } else {
                            Niveau::Info
                        },
                        texte,
                    );
                }
            }
        }
    }

    fn compiler_recherche(&mut self) {
        if self.recherche.is_empty() {
            self.regex = None;
            self.recherche_invalide = false;
            return;
        }
        // Regex insensible à la casse (multiligne : un champ par ligne) sur le texte normalisé ; invalide : on garde l'ancienne.
        match Regex::new(&format!("(?im){}", normaliser(&self.recherche))) {
            Ok(re) => {
                self.regex = Some(re);
                self.recherche_invalide = false;
            }
            Err(_) => self.recherche_invalide = true,
        }
    }

    fn accepte(&self, f: &Film) -> bool {
        self.filtre_rapide.accepte(f)
            && self
                .regex
                .as_ref()
                .is_none_or(|re| re.is_match(&f.texte_recherche()))
    }

    /// Recalcule la vue dérivée (filtre + tri), conserve la sélection valide, sélectionne la 1re ligne si vide.
    pub fn recalculer(&mut self) {
        let mut v: Vec<usize> = (0..self.films.len())
            .filter(|&i| self.accepte(&self.films[i]))
            .collect();
        let tri = self.tri.clone();
        v.sort_by(|&a, &b| {
            let (fa, fb) = (&self.films[a], &self.films[b]);
            for (col, ordre) in &tri {
                let o = col.comparer(fa, fb);
                let o = if *ordre == Ordre::Decroissant {
                    o.reverse()
                } else {
                    o
                };
                if o.is_ne() {
                    return o;
                }
            }
            // Le titre est toujours le tri secondaire, puis l'année et l'id pour la stabilité.
            Colonne::Titre
                .comparer(fa, fb)
                .then(fa.annee.cmp(&fb.annee))
                .then(fa.id.cmp(&fb.id))
        });
        self.visibles = v;
        let ids: BTreeSet<u64> = self.visibles.iter().map(|&i| self.films[i].id).collect();
        self.selection.retain(|id| ids.contains(id));
        if self
            .principal
            .is_some_and(|id| !self.selection.contains(&id))
        {
            self.principal = self.selection.iter().next().copied();
        }
        if self.selection.is_empty()
            && let Some(&premier) = self.visibles.first()
        {
            let id = self.films[premier].id;
            self.selection.insert(id);
            self.principal = Some(id);
            self.ancre = Some(id);
        }
        if self.visibles.is_empty() {
            self.principal = None;
        }
    }

    fn trier(&mut self, colonne: Colonne, additif: bool) {
        if additif {
            match self.tri.iter_mut().find(|(c, _)| *c == colonne) {
                Some((_, o)) => *o = o.inverse(),
                None => self.tri.push((colonne, Ordre::Croissant)),
            }
        } else if self.tri.len() == 1 && self.tri[0].0 == colonne {
            self.tri[0].1 = self.tri[0].1.inverse();
        } else {
            self.tri = vec![(colonne, Ordre::Croissant)];
        }
        self.recalculer();
    }

    fn id_en_ligne(&self, ligne: usize) -> u64 {
        self.films[self.visibles[ligne]].id
    }

    fn selectionner(&mut self, id: u64, ctrl: bool, maj: bool) {
        let Some(pos) = self.position(id) else { return };
        if maj {
            let depart = self.ancre.and_then(|a| self.position(a)).unwrap_or(pos);
            let (a, b) = (depart.min(pos), depart.max(pos));
            let plage: Vec<u64> = (a..=b).map(|l| self.id_en_ligne(l)).collect();
            if !ctrl {
                self.selection.clear();
            }
            self.selection.extend(plage);
        } else if ctrl {
            if !self.selection.remove(&id) {
                self.selection.insert(id);
            }
            self.ancre = Some(id);
        } else {
            self.selection = BTreeSet::from([id]);
            self.ancre = Some(id);
        }
        self.principal = if self.selection.contains(&id) {
            Some(id)
        } else {
            self.selection.iter().next().copied()
        };
    }

    fn deplacer(&mut self, delta: i32, etendre: bool) {
        if self.visibles.is_empty() {
            return;
        }
        let courant = self.principal.and_then(|id| self.position(id)).unwrap_or(0) as i64;
        let max = self.visibles.len() as i64 - 1;
        let cible = (courant + delta as i64).clamp(0, max) as usize;
        let id = self.id_en_ligne(cible);
        self.selectionner(id, false, etendre);
        self.scroll_vers = Some(cible);
    }

    fn frappe(&mut self, texte: &str, instant: Instant) {
        if self
            .derniere_frappe
            .is_none_or(|t| instant.saturating_duration_since(t) > DELAI_FRAPPE)
        {
            self.tampon_frappe.clear();
        }
        self.derniere_frappe = Some(instant);
        self.tampon_frappe.push_str(&normaliser(texte));
        let tampon = self.tampon_frappe.clone();
        let trouve = self
            .visibles
            .iter()
            .position(|&i| normaliser(&self.films[i].titre_triable).starts_with(&tampon));
        if let Some(ligne) = trouve {
            let id = self.id_en_ligne(ligne);
            self.selectionner(id, false, false);
            self.scroll_vers = Some(ligne);
        }
    }

    // ---------------------------------------------------------------- commandes

    fn executer(&mut self, commande: Commande) {
        if commande.requiert_selection() && self.selection.is_empty() {
            let texte = self.i18n.t("app.nothingselected");
            self.notifier(Niveau::Avertissement, texte);
            return;
        }
        match commande {
            Commande::MettreAJourToutesSources => self.lancer_tache("gui.task.update", 40, true),
            Commande::ScrapeMeilleur | Commande::ScrapeNonTraites | Commande::ScrapeMetadonnees => {
                self.lancer_tache("gui.task.scrape", 25, true)
            }
            Commande::BasculerVu => {
                let ids = self.selection.clone();
                for f in self.films.iter_mut().filter(|f| ids.contains(&f.id)) {
                    f.vu = !f.vu;
                }
                self.recalculer();
            }
            Commande::BasculerVerrou => {
                let ids = self.selection.clone();
                for f in self.films.iter_mut().filter(|f| ids.contains(&f.id)) {
                    f.verrouille = !f.verrouille;
                }
            }
            Commande::RetirerBase => {
                let n = self.selection.len();
                let ids = std::mem::take(&mut self.selection);
                self.films.retain(|f| !ids.contains(&f.id));
                self.principal = None;
                self.ancre = None;
                self.recalculer();
                let compte = n.to_string();
                let texte = self
                    .i18n
                    .t_args("gui.removed", &[("count", compte.as_str())]);
                self.notifier(Niveau::Info, texte);
            }
            Commande::Renommer | Commande::ApercuRenommage => {
                self.ouvrir_dialogue(TypeDialogue::ApercuRenommage)
            }
            autre => {
                let action = self.i18n.t(autre.cle());
                let texte = self
                    .i18n
                    .t_args("gui.notimplemented", &[("action", action.as_str())]);
                self.notifier(Niveau::Info, texte);
            }
        }
    }

    // ---------------------------------------------------------------- dialogues

    fn ouvrir_dialogue(&mut self, type_dialogue: TypeDialogue) {
        match type_dialogue {
            TypeDialogue::ApercuRenommage => {
                if self.selection.is_empty() {
                    let texte = self.i18n.t("app.nothingselected");
                    self.notifier(Niveau::Avertissement, texte);
                    return;
                }
                let ids: Vec<u64> = self.films_selectionnes().iter().map(|f| f.id).collect();
                let profil = self.reglages.profil_actif;
                self.dialogue = Some(Dialogue::ApercuRenommage(EtatApercu {
                    ids,
                    profil,
                    masquer_inchanges: false,
                    film_actif: None,
                }));
                // Le premier film de l'aperçu (trié par titre) est affiché par défaut.
                let premier = self.apercu_courant().first().map(|x| x.id);
                if let Some(Dialogue::ApercuRenommage(a)) = &mut self.dialogue {
                    a.film_actif = premier;
                }
            }
            TypeDialogue::Reglages => {
                self.dialogue = Some(Dialogue::Reglages(EtatDialogueReglages {
                    page: Page::General,
                    filtre: String::new(),
                }));
            }
            TypeDialogue::Messages => {
                self.dialogue = Some(Dialogue::Messages);
                self.non_lus = 0;
            }
        }
    }

    fn action_dialogue(&mut self, action: ActionDialogue) {
        match (&mut self.dialogue, action) {
            (Some(Dialogue::ApercuRenommage(a)), ActionDialogue::ApercuProfil(p)) => a.profil = p,
            (Some(Dialogue::ApercuRenommage(a)), ActionDialogue::ApercuMasquer(m)) => {
                a.masquer_inchanges = m
            }
            (Some(Dialogue::ApercuRenommage(a)), ActionDialogue::ApercuFilm(id)) => {
                a.film_actif = Some(id)
            }
            (Some(Dialogue::Reglages(r)), ActionDialogue::ReglagesPage(p)) => r.page = p,
            (Some(Dialogue::Reglages(r)), ActionDialogue::ReglagesFiltre(f)) => r.filtre = f,
            _ => {}
        }
    }

    /// Aperçu du renommage pour l'état courant du dialogue (calculé à la volée, tri par titre).
    pub fn apercu_courant(&self) -> Vec<renommage::ApercuFilm> {
        let Some(Dialogue::ApercuRenommage(a)) = &self.dialogue else {
            return Vec::new();
        };
        let Some(profil) = self.reglages.profils.get(a.profil) else {
            return Vec::new();
        };
        let films: Vec<&Film> = a.ids.iter().filter_map(|&id| self.film(id)).collect();
        renommage::calculer(&films, profil)
    }

    /// « Renommer » du dialogue : applique les nouveaux noms (factices) aux films sans problème.
    fn appliquer_renommage(&mut self) {
        let apercus = self.apercu_courant();
        let faits: Vec<_> = apercus
            .into_iter()
            .filter(|a| !a.probleme && !a.rien_a_renommer())
            .collect();
        for a in &faits {
            if let Some(f) = self.films.iter_mut().find(|f| f.id == a.id) {
                f.dossier = a.nouveau_dossier.clone();
                for (fic, ligne) in f.fichiers.iter_mut().zip(&a.fichiers) {
                    fic.nom = ligne.nouveau.clone();
                    if fic.type_fichier == crate::modele::TypeFichier::Video {
                        f.nom_fichier = ligne.nouveau.clone();
                    }
                }
            }
        }
        let ids: BTreeSet<u64> = faits.iter().map(|a| a.id).collect();
        if let Some(Dialogue::ApercuRenommage(d)) = &mut self.dialogue {
            d.ids.retain(|id| !ids.contains(id));
            if d.film_actif.is_some_and(|id| ids.contains(&id)) {
                d.film_actif = d.ids.first().copied();
            }
            if d.ids.is_empty() {
                self.dialogue = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titres(e: &Etat) -> Vec<String> {
        e.visibles
            .iter()
            .map(|&i| e.films[i].titre.clone())
            .collect()
    }

    fn id_de(e: &Etat, titre: &str, annee: u32) -> u64 {
        e.films
            .iter()
            .find(|f| f.titre == titre && f.annee == annee)
            .unwrap()
            .id
    }

    #[test]
    fn etat_initial_tri_par_titre_et_premiere_ligne_selectionnee() {
        let e = Etat::nouveau();
        assert_eq!(e.visibles.len(), e.films.len());
        let t = titres(&e);
        assert_eq!(t[0], "Alien");
        // « Dune » (1984) avant « Dune » (2021) : titre égal, année en repli
        let dunes: Vec<u32> = e
            .visibles
            .iter()
            .map(|&i| &e.films[i])
            .filter(|f| f.titre == "Dune")
            .map(|f| f.annee)
            .collect();
        assert_eq!(dunes, [1984, 2021]);
        assert_eq!(e.selection.len(), 1);
        assert_eq!(e.principal, Some(e.films[e.visibles[0]].id));
    }

    #[test]
    fn titre_triable_deplace_l_article() {
        let e = Etat::nouveau();
        let t = titres(&e);
        let pos_evades = t.iter().position(|x| x == "Les Évadés").unwrap();
        let pos_dune = t.iter().position(|x| x == "Dune").unwrap();
        let pos_fabuleux = t.iter().position(|x| x.starts_with("Le Fabuleux")).unwrap();
        let pos_gladiator = t.iter().position(|x| x == "Gladiator").unwrap();
        // « Évadés, Les » (E) et « Fabuleux Destin..., Le » (F) : triés sans leur article
        assert!(pos_dune < pos_evades && pos_evades < pos_fabuleux && pos_fabuleux < pos_gladiator);
    }

    #[test]
    fn recherche_insensible_a_la_casse_et_aux_accents() {
        let mut e = Etat::nouveau();
        e.appliquer(Message::Recherche("HEREDITE".into()));
        assert_eq!(titres(&e), ["Hérédité"]);
        e.appliquer(Message::Recherche("amélie".into()));
        assert_eq!(titres(&e).len(), 1);
        e.appliquer(Message::Recherche("^dune$".into()));
        assert_eq!(titres(&e), ["Dune", "Dune"]);
    }

    #[test]
    fn regex_invalide_conserve_le_dernier_filtre() {
        let mut e = Etat::nouveau();
        e.appliquer(Message::Recherche("alien".into()));
        assert_eq!(titres(&e), ["Alien"]);
        e.appliquer(Message::Recherche("alien(".into()));
        assert!(e.recherche_invalide);
        assert_eq!(titres(&e), ["Alien"]);
        e.appliquer(Message::EffacerFiltres);
        assert!(!e.recherche_invalide);
        assert_eq!(e.visibles.len(), e.films.len());
    }

    #[test]
    fn filtre_rapide_et_indicateur() {
        let mut e = Etat::nouveau();
        assert!(!e.filtre_actif());
        e.appliquer(Message::FiltreRapide(FiltreRapide::Nouveaux));
        assert!(e.filtre_actif());
        assert!(e.visibles.iter().all(|&i| e.films[i].nouveau));
        e.appliquer(Message::FiltreRapide(FiltreRapide::Vus));
        assert!(e.visibles.iter().all(|&i| e.films[i].vu));
    }

    #[test]
    fn tri_simple_inverse_et_additif() {
        let mut e = Etat::nouveau();
        e.appliquer(Message::Trier {
            colonne: Colonne::Note,
            additif: false,
        });
        assert_eq!(e.tri, [(Colonne::Note, Ordre::Croissant)]);
        e.appliquer(Message::Trier {
            colonne: Colonne::Note,
            additif: false,
        });
        assert_eq!(e.tri, [(Colonne::Note, Ordre::Decroissant)]);
        let premier = &e.films[e.visibles[0]];
        assert_eq!(premier.titre, "Les Évadés"); // note 9.3
        e.appliquer(Message::Trier {
            colonne: Colonne::Annee,
            additif: true,
        });
        assert_eq!(e.tri.len(), 2);
        e.appliquer(Message::Trier {
            colonne: Colonne::Annee,
            additif: true,
        });
        assert_eq!(e.tri[1], (Colonne::Annee, Ordre::Decroissant));
    }

    #[test]
    fn selection_simple_ctrl_et_plage() {
        let mut e = Etat::nouveau();
        let ids: Vec<u64> = e.visibles.iter().map(|&i| e.films[i].id).collect();
        e.appliquer(Message::Selectionner {
            id: ids[2],
            ctrl: false,
            maj: false,
        });
        assert_eq!(e.selection.len(), 1);
        e.appliquer(Message::Selectionner {
            id: ids[5],
            ctrl: false,
            maj: true,
        });
        assert_eq!(e.selection.len(), 4);
        e.appliquer(Message::Selectionner {
            id: ids[3],
            ctrl: true,
            maj: false,
        });
        assert_eq!(e.selection.len(), 3);
        assert!(!e.selection.contains(&ids[3]));
        e.appliquer(Message::SelectionnerTout);
        assert_eq!(e.selection.len(), e.films.len());
    }

    #[test]
    fn la_selection_est_conservee_ou_reprise_apres_filtre() {
        let mut e = Etat::nouveau();
        let id = id_de(&e, "Matrix", 1999);
        e.appliquer(Message::Selectionner {
            id,
            ctrl: false,
            maj: false,
        });
        e.appliquer(Message::Recherche("matrix".into()));
        assert!(e.selection.contains(&id));
        e.appliquer(Message::Recherche("interstellar".into()));
        assert!(!e.selection.contains(&id));
        assert_eq!(e.film_principal().unwrap().titre, "Interstellar");
        e.appliquer(Message::Recherche("zzzz".into()));
        assert!(e.visibles.is_empty() && e.film_principal().is_none());
    }

    #[test]
    fn deplacement_clavier() {
        let mut e = Etat::nouveau();
        e.appliquer(Message::Deplacer {
            delta: 1,
            etendre: false,
        });
        assert_eq!(e.principal, Some(e.films[e.visibles[1]].id));
        assert_eq!(e.scroll_vers, Some(1));
        e.appliquer(Message::Deplacer {
            delta: -10,
            etendre: false,
        });
        assert_eq!(e.position(e.principal.unwrap()), Some(0));
        e.appliquer(Message::Deplacer {
            delta: 2,
            etendre: true,
        });
        assert_eq!(e.selection.len(), 3); // plage ancre (ligne 0) -> ligne 2
    }

    #[test]
    fn frappe_incrementale_avec_fenetre_de_500_ms() {
        let mut e = Etat::nouveau();
        let t0 = Instant::now();
        e.appliquer(Message::Frappe {
            texte: "m".into(),
            instant: t0,
        });
        assert_eq!(e.film_principal().unwrap().titre, "Mad Max: Fury Road");
        e.appliquer(Message::Frappe {
            texte: "a".into(),
            instant: t0 + Duration::from_millis(200),
        });
        e.appliquer(Message::Frappe {
            texte: "t".into(),
            instant: t0 + Duration::from_millis(400),
        });
        assert_eq!(e.film_principal().unwrap().titre, "Matrix");
        // au-delà de 500 ms le tampon repart de zéro : « p » -> Parasite
        e.appliquer(Message::Frappe {
            texte: "p".into(),
            instant: t0 + Duration::from_millis(1500),
        });
        assert_eq!(e.film_principal().unwrap().titre, "Parasite");
    }

    #[test]
    fn colonne_titre_non_masquable() {
        let mut e = Etat::nouveau();
        e.appliquer(Message::ColonneVisible(Colonne::Titre, false));
        assert!(e.colonnes_visibles().contains(&Colonne::Titre));
        e.appliquer(Message::ColonneVisible(Colonne::Duree, true));
        assert!(e.colonnes_visibles().contains(&Colonne::Duree));
    }

    #[test]
    fn totaux_de_selection() {
        let mut e = Etat::nouveau();
        let a = id_de(&e, "Alien", 1979);
        let b = id_de(&e, "Tenet", 2020);
        e.appliquer(Message::Selectionner {
            id: a,
            ctrl: false,
            maj: false,
        });
        e.appliquer(Message::Selectionner {
            id: b,
            ctrl: true,
            maj: false,
        });
        let go = crate::modele::gigaoctets(e.taille_selection());
        assert!((go - 39.7).abs() < 0.01, "{go}");
    }

    #[test]
    fn commandes_sans_selection_notifient() {
        let mut e = Etat::nouveau();
        e.appliquer(Message::Recherche("zzzz".into()));
        e.appliquer(Message::Commande(Commande::BasculerVu));
        assert_eq!(
            e.notifications.last().unwrap().texte,
            "Aucune entrée sélectionnée"
        );
        assert_eq!(e.non_lus, 1);
        e.appliquer(Message::MessagesLus);
        assert_eq!(e.non_lus, 0);
    }

    #[test]
    fn basculer_vu_et_retirer() {
        let mut e = Etat::nouveau();
        let id = e.principal.unwrap();
        let avant = e.film(id).unwrap().vu;
        e.appliquer(Message::Commande(Commande::BasculerVu));
        assert_ne!(e.film(id).unwrap().vu, avant);
        let total = e.films.len();
        e.appliquer(Message::Commande(Commande::RetirerBase));
        assert_eq!(e.films.len(), total - 1);
        assert!(e.film(id).is_none());
        assert_eq!(e.selection.len(), 1); // la 1re ligne est resélectionnée
    }

    #[test]
    fn tache_en_arriere_plan_via_le_canal() {
        let mut e = Etat::nouveau();
        e.pas_simulation = Duration::from_millis(1);
        e.appliquer(Message::Commande(Commande::MettreAJourToutesSources));
        assert_eq!(e.taches.len(), 1);
        assert!(e.tache_active().unwrap().principale);
        // une seconde tâche principale est refusée
        e.appliquer(Message::Commande(Commande::ScrapeNonTraites));
        assert_eq!(e.taches.len(), 1);
        let fin = Instant::now() + Duration::from_secs(10);
        while !e.taches.is_empty() && Instant::now() < fin {
            if let Ok(m) = e.rx.recv_timeout(Duration::from_millis(100)) {
                e.appliquer(m);
            }
        }
        assert!(e.taches.is_empty());
        assert_eq!(
            e.notifications.last().unwrap().texte,
            "Tâche terminée : Mise à jour des sources"
        );
    }

    #[test]
    fn annulation_d_une_tache() {
        let mut e = Etat::nouveau();
        e.pas_simulation = Duration::from_millis(5);
        e.appliquer(Message::Commande(Commande::MettreAJourToutesSources));
        let id = e.taches[0].id;
        e.appliquer(Message::ArreterTache(id));
        let fin = Instant::now() + Duration::from_secs(10);
        while !e.taches.is_empty() && Instant::now() < fin {
            e.drainer();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(
            e.notifications
                .last()
                .unwrap()
                .texte
                .starts_with("Tâche annulée")
        );
    }

    #[test]
    fn dialogue_apercu_renommage_et_application() {
        let mut e = Etat::nouveau();
        let id = id_de(&e, "Blade Runner 2049", 2017);
        e.appliquer(Message::Selectionner {
            id,
            ctrl: false,
            maj: false,
        });
        e.appliquer(Message::Commande(Commande::ApercuRenommage));
        assert!(matches!(e.dialogue, Some(Dialogue::ApercuRenommage(_))));
        let apercus = e.apercu_courant();
        assert_eq!(apercus.len(), 1);
        assert_eq!(apercus[0].nouveau_dossier, "Blade Runner 2049 (2017)");
        e.appliquer(Message::ApercuAppliquer);
        assert!(e.dialogue.is_none());
        assert_eq!(e.film(id).unwrap().dossier, "Blade Runner 2049 (2017)");
        assert_eq!(
            e.film(id).unwrap().nom_fichier,
            "Blade Runner 2049 (2017).mkv"
        );
    }

    #[test]
    fn changement_de_langue_recharge_les_traductions() {
        let mut e = Etat::nouveau();
        assert_eq!(e.i18n.t("app.movies"), "Films");
        let mut r = e.reglages.clone();
        r.langue = crate::i18n::Langue::Anglais;
        e.appliquer(Message::AppliquerReglages(Box::new(r)));
        assert_eq!(e.i18n.t("app.movies"), "Movies");
    }
}
