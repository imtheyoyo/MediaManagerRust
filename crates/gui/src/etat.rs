//! Modèle d'état (`Etat`), messages et logique de mise à jour (sans aucune dépendance egui :
//! entièrement testable). La vue lit un `&Etat` et émet des `Message`.

use crate::collections::NoeudSaga;
use crate::commandes::Commande;
use crate::dialogues::{
    ActionAssistant, ActionConfirmee, ActionEditeur, ActionEditeurSerie, ActionRecherche,
    CaseConfirmation, EtatAssistant, EtatConfirmation, EtatEditeur, EtatEditeurSerie,
    EtatManquants, EtatRecherche, FilmEdite, OngletEditeur, SerieEdite, valider, valider_serie,
};
use crate::filtres::{ActionFiltre, ContexteFiltre, FiltresAvances, OngletFiltre};
use crate::i18n::{I18n, Langue};
use crate::mise_a_jour::{self, ConfigMiseAJour, EtapeInstallation, Publication, Verdict};
use crate::modele::{Colonne, ColonneEtat, Film, Ordre, colonnes_par_defaut, normaliser};
use crate::ouverture::{self, CibleOuverture, Lanceur, Systeme};
use crate::persistance::{ColonnePersistee, EtatInterface, GeometrieFenetre, TriPersiste};
use crate::reglages::{Page, Reglages, Theme};
use crate::renommage::ApercuFilm;
use crate::selecteur::{self, CibleDossier, CibleFichier, SelecteurDossier, SelecteurFichier};
use crate::source_donnees::{
    DiagnosticOutils, GroupeManquants, ReglagesServices, RequeteRecherche, RequeteRechercheSerie,
    ResultatRecherche, Retour, SECRET_CLE_TMDB, SagaAffichage, SerieAffichage, SourceDonnees,
    SourceFactice, Travail, titre_nettoye,
};
use crate::taches::{IssueTache, Repaint, Tache};
use regex::Regex;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
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

    /// Identifiant de persistance (`ui.json`).
    pub fn id(self) -> &'static str {
        match self {
            Module::Films => "movies",
            Module::Collections => "moviesets",
            Module::Series => "tvshows",
        }
    }

    pub fn depuis_id(id: &str) -> Option<Module> {
        Module::TOUS.into_iter().find(|m| m.id() == id)
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

/// Nœud sélectionné dans l'arbre des séries (identifiant de série, numéro de saison,
/// rang de l'épisode dans la saison).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoeudSerie {
    Serie(String),
    Saison(String, i32),
    Episode(String, i32, usize),
}

impl NoeudSerie {
    pub fn id_serie(&self) -> &str {
        match self {
            NoeudSerie::Serie(id) | NoeudSerie::Saison(id, _) | NoeudSerie::Episode(id, _, _) => id,
        }
    }
}

/// Filtre rapide du menu « Filtre » (les filtres avancés sont dans `filtres`).
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
    pub masquer_inchanges: bool,
    pub film_actif: Option<u64>,
    /// Plan de renommage calculé par la source à l'ouverture du dialogue.
    pub apercus: Vec<ApercuFilm>,
    pub erreur: Option<String>,
    /// Séries concernées (renommage d'une série et de ses épisodes) ; `None` pour des films.
    pub series: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default)]
pub struct EtatDialogueReglages {
    pub page: Page,
    pub filtre: String,
    /// Saisies en cours (ajout d'un dossier source films / séries, clé d'API).
    pub saisie_source: String,
    pub saisie_source_serie: String,
    pub saisie_cle: String,
    /// Test des outils d'analyse en cours / dernier résultat.
    pub test_outils_en_cours: bool,
    pub test_outils: Option<Result<DiagnosticOutils, String>>,
}

/// Dialogue modal affiché (un seul à la fois ; le dialogue de filtres, non modal, est à part).
#[derive(Clone, Debug)]
pub enum Dialogue {
    ApercuRenommage(EtatApercu),
    Reglages(EtatDialogueReglages),
    Messages,
    Editeur(Box<EtatEditeur>),
    Recherche(Box<EtatRecherche>),
    EditeurSerie(Box<EtatEditeurSerie>),
    Manquants(EtatManquants),
    Confirmation(EtatConfirmation),
    Assistant(Box<EtatAssistant>),
    MiseAJour(EtatDialogueMiseAJour),
}

/// Étape affichée par le dialogue de mise à jour du programme.
#[derive(Clone, Debug, PartialEq)]
pub enum PhaseMiseAJour {
    Recherche,
    AJour,
    /// Compilation locale : aucune installation proposée.
    Developpement,
    Disponible,
    Installation(EtapeInstallation),
    Erreur(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct EtatDialogueMiseAJour {
    pub phase: PhaseMiseAJour,
    pub publication: Option<Publication>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeDialogue {
    ApercuRenommage,
    Reglages,
    Messages,
    Assistant,
}

#[derive(Clone, Debug)]
pub enum ActionDialogue {
    ApercuMasquer(bool),
    ApercuFilm(u64),
    ReglagesPage(Page),
    ReglagesFiltre(String),
    ReglagesSaisieSource(String),
    ReglagesSaisieSourceSerie(String),
    ReglagesSaisieCle(String),
    /// Transmet la clé saisie au coffre à secrets.
    ReglagesEnregistrerCle,
    Editeur(ActionEditeur),
    Recherche(ActionRecherche),
    EditeurSerie(ActionEditeurSerie),
    Assistant(ActionAssistant),
    ConfirmationNePlusDemander(bool),
    /// Case propre à l'action confirmée (« déplacer les fichiers vers la corbeille »).
    ConfirmationCase(bool),
    /// Choisit un élément de la liste des manquants (groupe, rang).
    ManquantsChoisir(usize, usize),
}

/// Messages : seul `Etat::appliquer` modifie l'état.
#[derive(Clone, Debug)]
pub enum Message {
    ChoisirModule(Module),
    ChoisirOnglet(OngletDetails),
    Recherche(String),
    FiltreRapide(FiltreRapide),
    EffacerFiltres,
    Trier {
        colonne: Colonne,
        additif: bool,
    },
    Selectionner {
        id: u64,
        ctrl: bool,
        maj: bool,
    },
    SelectionnerTout,
    Deplacer {
        delta: i32,
        etendre: bool,
    },
    Frappe {
        texte: String,
        instant: Instant,
    },
    ColonneVisible(Colonne, bool),
    /// Déplace une colonne d'un cran dans l'ordre d'affichage (-1 : à gauche).
    DeplacerColonne(Colonne, i32),
    /// Largeurs relevées après redimensionnement par l'utilisateur.
    LargeursColonnes(Vec<(Colonne, f32)>),
    /// Largeur du panneau de détails (position du séparateur).
    LargeurDetails(f32),
    Fenetre(GeometrieFenetre),
    Commande(Commande),
    OuvrirDialogue(TypeDialogue),
    FermerDialogue,
    Dialogue(ActionDialogue),
    /// « Valider » de l'incrustation de confirmation.
    Confirmer,
    ApercuAppliquer,
    AppliquerReglages(Box<Reglages>),
    /// Réglages des services modifiés dans le dialogue (enregistrés à sa fermeture).
    AppliquerServices(Box<ReglagesServices>),
    /// Ouvre les réglages sur une page donnée.
    OuvrirReglagesPage(Page),
    Filtre(ActionFiltre),
    OuvrirFiltres(bool),
    OngletFiltres(OngletFiltre),
    ResultatsRecherche {
        id_requete: u64,
        resultat: Result<Vec<ResultatRecherche>, String>,
    },
    OuvrirVisionneuse(PathBuf),
    FermerVisionneuse,
    ViderCacheImages,
    FocusRecherche,
    /// Demande de fermeture de la fenêtre (confirmation si des tâches tournent).
    DemandeQuitter,
    Notifier(Niveau, String),
    MessagesLus,
    ArreterTache(u64),
    /// Mise à jour des sources de séries (module Séries).
    MettreAJourSeries,
    /// Ouvre le sélecteur de dossiers natif (thread dédié).
    ChoisirDossier(CibleDossier),
    /// Résultat du sélecteur (`None` : annulé).
    DossierChoisi(CibleDossier, Option<PathBuf>),
    /// Ouvre le sélecteur de fichiers natif (thread dédié).
    ChoisirFichier(CibleFichier),
    /// Résultat du sélecteur de fichiers (`None` : annulé).
    FichierChoisi(CibleFichier, Option<PathBuf>),
    /// Teste les outils d'analyse avec le chemin saisi dans les réglages.
    TesterOutils,
    ResultatTestOutils(Result<DiagnosticOutils, String>),
    /// Fin de la recherche des éléments manquants (sagas ou série).
    ManquantsTrouves {
        id_requete: u64,
        groupes: Vec<GroupeManquants>,
        erreurs: Vec<String>,
    },
    /// Recherche d'une nouvelle version du programme (`manuel` : demandée par l'utilisateur,
    /// le dialogue s'ouvre aussitôt ; sinon vérification silencieuse au démarrage).
    RechercherMiseAJour {
        manuel: bool,
    },
    ResultatMiseAJour {
        manuel: bool,
        resultat: Result<(Verdict, Publication), String>,
    },
    /// « Installer » du dialogue de mise à jour.
    InstallerMiseAJour,
    ProgressionMiseAJour(EtapeInstallation),
    /// Fin de l'installation : relance du programme ou erreur.
    MiseAJourInstallee(Result<PathBuf, String>),
    /// Déploie / replie une série (`None`) ou une saison dans l'arbre des séries.
    BasculerNoeudSerie(String, Option<i32>),
    /// Sélectionne un nœud de l'arbre des séries.
    SelectionnerNoeudSerie(NoeudSerie),
    /// Déploie / replie une saga dans l'arbre des collections.
    BasculerSaga(String),
    /// Sélectionne un nœud de l'arbre des collections.
    SelectionnerNoeudSaga(NoeudSaga),
    /// Ouvre un dossier, un fichier ou une adresse avec le système (non bloquant).
    Ouvrir(CibleOuverture),
    // ---- messages publiés par la source de données
    TacheDemarree {
        id: u64,
        /// Clé de traduction ou nom déjà lisible.
        nom: String,
        principale: bool,
    },
    TacheProgression {
        id: u64,
        fait: u32,
        total: u32,
        description: String,
    },
    TacheTerminee {
        id: u64,
        issue: IssueTache,
    },
    /// Film ajouté ou modifié par les services.
    FilmActualise(Box<Film>),
    /// Film retiré de la bibliothèque.
    FilmRetire(u64),
    /// La liste des séries a changé.
    SeriesModifiees,
}

/// Ce dont on cherche les éléments manquants : (identifiant, titre) d'une saga ou d'une série.
#[derive(Clone, Debug, PartialEq, Eq)]
enum CibleManquants {
    Saga(String, String),
    Serie(String, String),
}

pub struct Etat {
    pub i18n: I18n,
    pub reglages: Reglages,
    /// Réglages des services (lus et écrits par la source).
    pub services: ReglagesServices,
    services_modifies: bool,
    pub source: Arc<dyn SourceDonnees>,
    pub films: Vec<Film>,
    pub series: Vec<SerieAffichage>,
    /// Nœuds déployés de l'arbre des séries : (série, saison) ; `None` = la série elle-même.
    pub series_deployees: BTreeSet<(String, Option<i32>)>,
    /// Nœud sélectionné dans l'arbre des séries.
    pub noeud_serie: Option<NoeudSerie>,
    /// Sagas (module Collections).
    pub sagas: Vec<SagaAffichage>,
    /// Sagas déployées dans l'arbre.
    pub sagas_deployees: BTreeSet<String>,
    /// Nœud sélectionné dans l'arbre des sagas.
    pub noeud_saga: Option<NoeudSaga>,
    /// Lancement des commandes du système (remplaçable en test) et système cible.
    pub lanceur: Lanceur,
    pub systeme: Systeme,
    pub module: Module,
    pub onglet: OngletDetails,
    pub recherche: String,
    pub recherche_invalide: bool,
    regex: Option<Regex>,
    pub filtre_rapide: FiltreRapide,
    pub filtres: FiltresAvances,
    pub filtres_ouverts: bool,
    pub onglet_filtres: OngletFiltre,
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
    /// Image affichée en plein écran (visionneuse).
    pub visionneuse: Option<PathBuf>,
    pub largeur_details: Option<f32>,
    pub fenetre: GeometrieFenetre,
    pub confirmations_masquees: BTreeSet<String>,
    pub assistant_termine: bool,
    /// Adresse de l'API des publications et identité du programme (remplaçables en test).
    pub config_mise_a_jour: ConfigMiseAJour,
    selecteur: SelecteurDossier,
    selecteur_fichier: SelecteurFichier,
    /// Demandes ponctuelles consommées par l'application (effets hors état).
    pub vider_cache_images: bool,
    pub focus_recherche: bool,
    pub quitter: bool,
    tampon_frappe: String,
    derniere_frappe: Option<Instant>,
    /// Canal de retour (tâches, recherche, événements des services) vers `rx`.
    retour: Retour,
    rx: Receiver<Message>,
    /// Rafraîchissement réel, remplaçable après la création (voir `definir_repaint`).
    repaint_partage: Arc<Mutex<Repaint>>,
    prochain_id_requete: u64,
    /// Vue dérivée ou séries à recalculer après un lot de messages.
    a_recalculer: bool,
    series_a_recharger: bool,
    sagas_a_recharger: bool,
}

impl Etat {
    pub fn nouveau() -> Self {
        Self::avec_source(Arc::new(SourceFactice::default()))
    }

    /// Point d'entrée de l'intégration : la bibliothèque vient de la source.
    pub fn avec_source(source: Arc<dyn SourceDonnees>) -> Self {
        let films = source.films();
        Self::construire(source, films)
    }

    pub fn avec_films(films: Vec<Film>) -> Self {
        Self::construire(Arc::new(SourceFactice::default()), films)
    }

    fn construire(source: Arc<dyn SourceDonnees>, films: Vec<Film>) -> Self {
        let (tx, rx) = channel();
        let reglages = Reglages::default();
        let repaint_partage: Arc<Mutex<Repaint>> = Arc::new(Mutex::new(Arc::new(|| {})));
        let partage = repaint_partage.clone();
        let retour = Retour {
            tx,
            repaint: Arc::new(move || {
                let r = partage.lock().map(|r| r.clone());
                if let Ok(r) = r {
                    r();
                }
            }),
        };
        source.brancher(retour.clone());
        let services = source.reglages_services();
        let series = source.series();
        let sagas = source.sagas();
        let mut etat = Self {
            i18n: I18n::new(reglages.langue),
            reglages,
            services,
            services_modifies: false,
            source,
            films,
            series,
            series_deployees: BTreeSet::new(),
            noeud_serie: None,
            sagas,
            sagas_deployees: BTreeSet::new(),
            noeud_saga: None,
            lanceur: ouverture::lanceur_systeme(),
            systeme: Systeme::courant(),
            module: Module::Films,
            onglet: OngletDetails::Details,
            recherche: String::new(),
            recherche_invalide: false,
            regex: None,
            filtre_rapide: FiltreRapide::Aucun,
            filtres: FiltresAvances::default(),
            filtres_ouverts: false,
            onglet_filtres: OngletFiltre::Details,
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
            visionneuse: None,
            largeur_details: None,
            fenetre: GeometrieFenetre::default(),
            confirmations_masquees: BTreeSet::new(),
            assistant_termine: false,
            config_mise_a_jour: ConfigMiseAJour::default(),
            selecteur: selecteur::par_defaut(),
            selecteur_fichier: selecteur::fichier_par_defaut(),
            vider_cache_images: false,
            focus_recherche: false,
            quitter: false,
            tampon_frappe: String::new(),
            derniere_frappe: None,
            retour,
            rx,
            repaint_partage,
            prochain_id_requete: 1,
            a_recalculer: false,
            series_a_recharger: false,
            sagas_a_recharger: false,
        };
        etat.recalculer();
        etat
    }

    pub fn definir_repaint(&mut self, repaint: Repaint) {
        if let Ok(mut r) = self.repaint_partage.lock() {
            *r = repaint;
        }
    }

    // ---------------------------------------------------------------- persistance de l'interface

    /// Instantané de ce qui est mémorisé dans `ui.json` (jamais de secret).
    pub fn instantane_interface(&self) -> EtatInterface {
        EtatInterface {
            fenetre: self.fenetre,
            dernier_module: self.module.id().into(),
            theme: self.reglages.theme.id().into(),
            langue: self.reglages.langue.code().into(),
            separateur: self.largeur_details,
            colonnes: self
                .colonnes
                .iter()
                .map(|c| ColonnePersistee {
                    id: c.colonne.id().into(),
                    visible: c.visible,
                    largeur: c.largeur,
                })
                .collect(),
            tri: self
                .tri
                .iter()
                .map(|(c, o)| TriPersiste {
                    colonne: c.id().into(),
                    croissant: *o == Ordre::Croissant,
                })
                .collect(),
            filtrage_actif: self.filtres.actif,
            filtres: self.filtres.lignes_actives(),
            prereglages_filtres: self.filtres.prereglages.clone(),
            confirmations_masquees: self.confirmations_masquees.clone(),
            assistant_termine: self.assistant_termine,
            verifier_mises_a_jour: self.reglages.verifier_mises_a_jour,
            ..EtatInterface::default()
        }
    }

    /// Restaure l'état mémorisé (identifiants inconnus ignorés, colonnes manquantes ajoutées).
    pub fn restaurer_interface(&mut self, ui: &EtatInterface) {
        self.fenetre = ui.fenetre;
        if self.reglages.memoriser_module {
            self.module = Module::depuis_id(&ui.dernier_module).unwrap_or(Module::Films);
        }
        if let Some(t) = Theme::depuis_id(&ui.theme) {
            self.reglages.theme = t;
        }
        if let Some(l) = Langue::depuis_code(&ui.langue)
            && l != self.reglages.langue
        {
            self.reglages.langue = l;
            self.i18n = I18n::new(l);
        }
        self.largeur_details = ui.separateur.filter(|v| v.is_finite() && *v > 0.0);
        if !ui.colonnes.is_empty() {
            let mut colonnes: Vec<ColonneEtat> = Vec::new();
            for p in &ui.colonnes {
                if let Some(c) = Colonne::depuis_id(&p.id)
                    && !colonnes.iter().any(|x| x.colonne == c)
                {
                    colonnes.push(ColonneEtat {
                        colonne: c,
                        visible: p.visible || c == Colonne::Titre,
                        largeur: p.largeur.filter(|l| l.is_finite() && *l > 0.0),
                    });
                }
            }
            for d in colonnes_par_defaut() {
                if !colonnes.iter().any(|x| x.colonne == d.colonne) {
                    colonnes.push(d);
                }
            }
            self.colonnes = colonnes;
        }
        let tri: Vec<(Colonne, Ordre)> = ui
            .tri
            .iter()
            .filter_map(|t| {
                Colonne::depuis_id(&t.colonne).map(|c| {
                    let o = if t.croissant {
                        Ordre::Croissant
                    } else {
                        Ordre::Decroissant
                    };
                    (c, o)
                })
            })
            .collect();
        if !tri.is_empty() {
            self.tri = tri;
        }
        self.filtres.charger_lignes(&ui.filtres);
        self.filtres.actif = ui.filtrage_actif;
        self.filtres.prereglages = ui.prereglages_filtres.clone();
        self.confirmations_masquees = ui.confirmations_masquees.clone();
        self.assistant_termine = ui.assistant_termine;
        self.reglages.verifier_mises_a_jour = ui.verifier_mises_a_jour;
        if !self.assistant_termine {
            self.ouvrir_dialogue(TypeDialogue::Assistant);
        }
        self.recalculer();
    }

    // ---------------------------------------------------------------- lecture

    pub fn colonnes_visibles(&self) -> Vec<Colonne> {
        self.colonnes
            .iter()
            .filter(|c| c.visible)
            .map(|c| c.colonne)
            .collect()
    }

    pub fn largeur_colonne(&self, colonne: Colonne) -> f32 {
        self.colonnes
            .iter()
            .find(|c| c.colonne == colonne)
            .and_then(|c| c.largeur)
            .unwrap_or_else(|| colonne.largeur_initiale())
    }

    pub fn film(&self, id: u64) -> Option<&Film> {
        self.films.iter().find(|f| f.id == id)
    }

    pub fn saga(&self, id: &str) -> Option<&SagaAffichage> {
        self.sagas.iter().find(|s| s.id == id)
    }

    /// Film du nœud sélectionné dans l'arbre des sagas.
    pub fn film_collection(&self) -> Option<&Film> {
        match &self.noeud_saga {
            Some(NoeudSaga::Film(_, id)) => self.film(*id),
            _ => None,
        }
    }

    /// Film visé par les actions sur les fichiers selon le module affiché.
    pub fn film_cible(&self) -> Option<&Film> {
        match self.module {
            Module::Collections => self.film_collection(),
            _ => self.film_principal(),
        }
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

    /// Identifiants sélectionnés dans l'ordre d'affichage.
    pub fn selection_ordonnee(&self) -> Vec<u64> {
        self.visibles
            .iter()
            .map(|&i| self.films[i].id)
            .filter(|id| self.selection.contains(id))
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
        !self.recherche.is_empty()
            || self.filtre_rapide != FiltreRapide::Aucun
            || self.filtres.est_actif()
    }

    /// Tâche affichée dans la barre d'état : une principale d'abord, sinon n'importe laquelle.
    pub fn tache_active(&self) -> Option<&Tache> {
        self.taches
            .iter()
            .find(|t| t.principale)
            .or_else(|| self.taches.first())
    }

    pub fn tri_de(&self, colonne: Colonne) -> Option<(usize, Ordre)> {
        self.tri
            .iter()
            .position(|(c, _)| *c == colonne)
            .map(|p| (p, self.tri[p].1))
    }

    /// Fermeture autorisée sans confirmation ?
    pub fn quitter_autorise(&self) -> bool {
        self.quitter || self.taches.is_empty()
    }

    // ---------------------------------------------------------------- canal de tâches

    /// Applique les messages reçus des tâches et des services (non bloquant, borné), puis
    /// recalcule une seule fois la vue dérivée.
    pub fn drainer(&mut self) {
        for _ in 0..MAX_MESSAGES_PAR_IMAGE {
            match self.rx.try_recv() {
                Ok(m) => self.appliquer_sans_recalcul(m),
                Err(_) => break,
            }
        }
        self.recalculs_differes();
    }

    fn recalculs_differes(&mut self) {
        if std::mem::take(&mut self.a_recalculer) {
            self.recalculer();
        }
        if std::mem::take(&mut self.series_a_recharger) {
            self.series = self.source.series();
        }
        if std::mem::take(&mut self.sagas_a_recharger) {
            self.sagas = self.source.sagas();
        }
    }

    /// Lance un traitement long ; il apparaîtra dans la barre d'état à son démarrage.
    fn lancer_travail(&mut self, travail: Travail) {
        if let Err(e) = self.source.lancer(travail, &self.retour) {
            self.notifier(Niveau::Erreur, e);
        }
    }

    /// Nom affiché d'une tâche : traduction si c'est une clé connue.
    fn nom_tache(&self, nom: &str) -> String {
        if self.i18n.brut(nom).is_some() {
            self.i18n.t(nom)
        } else {
            nom.to_string()
        }
    }

    /// Enregistre les réglages des services s'ils ont changé.
    fn enregistrer_services(&mut self) {
        if !std::mem::take(&mut self.services_modifies) {
            return;
        }
        if let Err(e) = self.source.enregistrer_reglages_services(&self.services) {
            self.notifier(Niveau::Erreur, e);
        }
    }

    fn fermer_dialogue(&mut self) {
        if matches!(self.dialogue, Some(Dialogue::Reglages(_))) {
            self.enregistrer_services();
        }
        self.dialogue = None;
    }

    fn notifier(&mut self, niveau: Niveau, texte: String) {
        self.notifications.push(Notification { niveau, texte });
        self.non_lus += 1;
    }

    // ---------------------------------------------------------------- mise à jour

    pub fn appliquer(&mut self, message: Message) {
        self.appliquer_sans_recalcul(message);
        self.recalculs_differes();
    }

    fn appliquer_sans_recalcul(&mut self, message: Message) {
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
                self.filtres.appliquer(ActionFiltre::Reinitialiser);
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
            Message::DeplacerColonne(col, delta) => self.deplacer_colonne(col, delta),
            Message::LargeursColonnes(largeurs) => {
                for (col, l) in largeurs {
                    if let Some(c) = self.colonnes.iter_mut().find(|c| c.colonne == col)
                        && l.is_finite()
                        && l > 0.0
                    {
                        c.largeur = Some(l);
                    }
                }
            }
            Message::LargeurDetails(l) => {
                if l.is_finite() && l > 0.0 {
                    self.largeur_details = Some(l);
                }
            }
            Message::Fenetre(g) => self.fenetre = g,
            Message::Commande(c) => self.executer(c),
            Message::OuvrirDialogue(t) => self.ouvrir_dialogue(t),
            Message::FermerDialogue => self.fermer_dialogue(),
            Message::Dialogue(a) => self.action_dialogue(a),
            Message::Confirmer => self.confirmer(),
            Message::ApercuAppliquer => self.appliquer_renommage(),
            Message::AppliquerReglages(r) => {
                if r.langue != self.reglages.langue {
                    self.i18n = I18n::new(r.langue);
                }
                self.reglages = *r;
            }
            Message::AppliquerServices(r) => {
                if *r != self.services {
                    self.services = *r;
                    self.services_modifies = true;
                }
            }
            Message::OuvrirReglagesPage(page) => {
                self.ouvrir_dialogue(TypeDialogue::Reglages);
                if let Some(Dialogue::Reglages(d)) = &mut self.dialogue {
                    d.page = page;
                }
            }
            Message::Filtre(a) => {
                self.filtres.appliquer(a);
                self.recalculer();
            }
            Message::OuvrirFiltres(o) => self.filtres_ouverts = o,
            Message::OngletFiltres(o) => self.onglet_filtres = o,
            Message::ResultatsRecherche {
                id_requete,
                resultat,
            } => self.recevoir_resultats(id_requete, resultat),
            Message::OuvrirVisionneuse(p) => self.visionneuse = Some(p),
            Message::FermerVisionneuse => self.visionneuse = None,
            Message::ViderCacheImages => {
                self.vider_cache_images = true;
                let texte = self.i18n.t("gui.images.cleared");
                self.notifier(Niveau::Info, texte);
            }
            Message::FocusRecherche => self.focus_recherche = true,
            Message::DemandeQuitter => {
                if self.taches.is_empty() {
                    self.quitter = true;
                } else {
                    self.demander_confirmation(ActionConfirmee::Quitter);
                }
            }
            Message::Notifier(niveau, texte) => self.notifier(niveau, texte),
            Message::MessagesLus => self.non_lus = 0,
            Message::ArreterTache(id) => self.source.annuler_tache(id),
            Message::MettreAJourSeries => self.lancer_travail(Travail::MettreAJourSeries),
            Message::ChoisirDossier(cible) => self.choisir_dossier(cible),
            Message::DossierChoisi(cible, chemin) => {
                if let Some(c) = chemin {
                    self.dossier_choisi(cible, c);
                }
            }
            Message::ChoisirFichier(cible) => self.choisir_fichier(cible),
            Message::FichierChoisi(cible, chemin) => {
                if let Some(c) = chemin {
                    self.fichier_choisi(cible, c);
                }
            }
            Message::TesterOutils => self.tester_outils(),
            Message::ResultatTestOutils(r) => {
                if let Some(Dialogue::Reglages(d)) = &mut self.dialogue {
                    d.test_outils_en_cours = false;
                    d.test_outils = Some(r);
                }
            }
            Message::ManquantsTrouves {
                id_requete,
                groupes,
                erreurs,
            } => {
                if let Some(Dialogue::Manquants(m)) = &mut self.dialogue
                    && m.id_requete == id_requete
                {
                    m.en_cours = false;
                    m.selection = groupes
                        .iter()
                        .position(|g| !g.elements.is_empty())
                        .map(|g| (g, 0));
                    m.groupes = groupes;
                    m.erreur = (!erreurs.is_empty()).then(|| erreurs.join("\n"));
                }
            }
            Message::RechercherMiseAJour { manuel } => self.rechercher_mise_a_jour(manuel),
            Message::ResultatMiseAJour { manuel, resultat } => {
                self.resultat_mise_a_jour(manuel, resultat)
            }
            Message::InstallerMiseAJour => self.installer_mise_a_jour(),
            Message::ProgressionMiseAJour(etape) => {
                if let Some(Dialogue::MiseAJour(d)) = &mut self.dialogue {
                    d.phase = PhaseMiseAJour::Installation(etape);
                }
            }
            Message::MiseAJourInstallee(r) => self.mise_a_jour_installee(r),
            Message::BasculerNoeudSerie(id, saison) => {
                let cle = (id, saison);
                if !self.series_deployees.remove(&cle) {
                    self.series_deployees.insert(cle);
                }
            }
            Message::SelectionnerNoeudSerie(noeud) => self.noeud_serie = Some(noeud),
            Message::BasculerSaga(id) => {
                if !self.sagas_deployees.remove(&id) {
                    self.sagas_deployees.insert(id);
                }
            }
            Message::SelectionnerNoeudSaga(noeud) => {
                // Un film choisi dans une saga déploie celle-ci (sélection depuis les détails).
                if let NoeudSaga::Film(id, _) = &noeud {
                    self.sagas_deployees.insert(id.clone());
                }
                self.noeud_saga = Some(noeud);
            }
            Message::Ouvrir(cible) => self.ouvrir(cible),
            Message::TacheDemarree {
                id,
                nom,
                principale,
            } => {
                if !self.taches.iter().any(|t| t.id == id) {
                    let nom = self.nom_tache(&nom);
                    self.taches.push(Tache::nouvelle(id, nom, principale));
                }
            }
            Message::TacheProgression {
                id,
                fait,
                total,
                description,
            } => {
                if let Some(t) = self.taches.iter_mut().find(|t| t.id == id) {
                    t.fait = fait;
                    t.total = total;
                    t.description = description;
                }
            }
            Message::TacheTerminee { id, issue } => {
                if let Some(pos) = self.taches.iter().position(|t| t.id == id) {
                    let tache = self.taches.remove(pos);
                    // Les recherches (tâches de fond) n'encombrent pas l'historique.
                    if !tache.principale && issue == IssueTache::Reussie {
                        return;
                    }
                    let nom = tache.nom.as_str();
                    let (cle, niveau) = match &issue {
                        IssueTache::Reussie => ("gui.task.done", Niveau::Info),
                        IssueTache::Annulee => ("gui.task.cancelled", Niveau::Avertissement),
                        IssueTache::Echouee(_) => ("gui.task.failed", Niveau::Erreur),
                    };
                    let texte = self.i18n.t_args(cle, &[("task", nom)]);
                    self.notifier(niveau, texte);
                }
            }
            Message::FilmActualise(f) => {
                match self.films.iter_mut().find(|x| x.id == f.id) {
                    Some(x) => *x = *f,
                    None => self.films.push(*f),
                }
                self.a_recalculer = true;
                self.sagas_a_recharger = true;
            }
            Message::FilmRetire(id) => {
                self.films.retain(|f| f.id != id);
                self.a_recalculer = true;
                self.sagas_a_recharger = true;
            }
            Message::SeriesModifiees => self.series_a_recharger = true,
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

    fn accepte(&self, f: &Film, ctx: &ContexteFiltre) -> bool {
        self.filtre_rapide.accepte(f)
            && self
                .regex
                .as_ref()
                .is_none_or(|re| re.is_match(&f.texte_recherche()))
            && self.filtres.accepte(f, ctx)
    }

    /// Recalcule la vue dérivée (filtre + tri), conserve la sélection valide, sélectionne la 1re ligne si vide.
    pub fn recalculer(&mut self) {
        let ctx = if self.filtres.actif && self.filtres.utilise_doublons() {
            ContexteFiltre::calculer(&self.films)
        } else {
            ContexteFiltre::default()
        };
        let mut v: Vec<usize> = (0..self.films.len())
            .filter(|&i| self.accepte(&self.films[i], &ctx))
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

    /// Déplace une colonne d'un cran parmi les colonnes visibles.
    fn deplacer_colonne(&mut self, col: Colonne, delta: i32) {
        let visibles: Vec<usize> = (0..self.colonnes.len())
            .filter(|&i| self.colonnes[i].visible)
            .collect();
        let Some(rang) = visibles
            .iter()
            .position(|&i| self.colonnes[i].colonne == col)
        else {
            return;
        };
        let cible = rang as i64 + delta.signum() as i64;
        if cible < 0 || cible as usize >= visibles.len() {
            return;
        }
        self.colonnes.swap(visibles[rang], visibles[cible as usize]);
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

    /// Lance l'ouverture d'une cible dans un fil dédié ; une erreur revient en notification.
    fn ouvrir(&mut self, cible: CibleOuverture) {
        if let CibleOuverture::Adresse(u) = &cible
            && !ouverture::adresse_valide(u)
        {
            let texte = self.i18n.t_args("gui.open.badurl", &[("url", u.as_str())]);
            self.notifier(Niveau::Avertissement, texte);
            return;
        }
        let commande = ouverture::commande(&cible, self.systeme);
        let (lanceur, retour) = (self.lanceur.clone(), self.retour.clone());
        let prefixe = self.i18n.t("gui.open.error");
        let lancement = std::thread::Builder::new()
            .name("mm-gui-ouverture".into())
            .spawn(move || {
                if let Err(e) = lanceur(commande) {
                    retour.envoyer(Message::Notifier(Niveau::Erreur, format!("{prefixe} {e}")));
                }
            });
        if let Err(e) = lancement {
            self.notifier(Niveau::Erreur, e.to_string());
        }
    }

    /// « Ouvrir le dossier » et « Lire » pour le film visé.
    fn action_fichier(&mut self, commande: Commande) {
        let Some(film) = self.film_cible() else {
            let texte = self.i18n.t("app.nothingselected");
            self.notifier(Niveau::Avertissement, texte);
            return;
        };
        let cible = if commande == Commande::Lire {
            match film.chemin_video() {
                Some(v) => CibleOuverture::Fichier(v),
                None => {
                    let texte = self.i18n.t("gui.play.novideo");
                    self.notifier(Niveau::Avertissement, texte);
                    return;
                }
            }
        } else {
            ouverture::cible_dossier(&film.chemin(), Some(&film.nom_fichier))
        };
        self.ouvrir(cible);
    }

    fn executer(&mut self, commande: Commande) {
        if self.module == Module::Series {
            return self.executer_serie(commande);
        }
        if matches!(commande, Commande::OuvrirDossier | Commande::Lire) {
            return self.action_fichier(commande);
        }
        // « Trouver les manquants » cherche la saga du nœud choisi dans Collections ou, à
        // défaut, celles des films sélectionnés : il gère lui-même l'absence de sélection.
        if commande == Commande::TrouverManquants {
            return self.trouver_manquants_films();
        }
        if commande.requiert_selection() && self.selection.is_empty() {
            self.avertir_aucune_selection();
            return;
        }
        match commande {
            Commande::MettreAJourToutesSources => self.lancer_travail(Travail::MettreAJourSources),
            Commande::MettreAJourSelection => {
                let ids = self.selection_ordonnee();
                self.lancer_travail(Travail::MettreAJourSelection { ids });
            }
            Commande::ScrapeMeilleur | Commande::ScrapeMetadonnees => {
                let ids = self.selection_ordonnee();
                self.lancer_travail(Travail::RecupererAuto {
                    ids,
                    artwork: commande == Commande::ScrapeMeilleur,
                });
            }
            Commande::ScrapeNonTraites => {
                let ids: Vec<u64> = self
                    .films
                    .iter()
                    .filter(|f| !f.metadonnees_ok && !f.verrouille)
                    .map(|f| f.id)
                    .collect();
                if ids.is_empty() {
                    let texte = self.i18n.t("gui.scrape.nothing");
                    self.notifier(Niveau::Info, texte);
                } else {
                    self.lancer_travail(Travail::RecupererAuto { ids, artwork: true });
                }
            }
            Commande::ReecrireNfo => {
                let ids = self.selection_ordonnee();
                self.lancer_travail(Travail::EcrireNfo { ids });
            }
            Commande::InfosMedia => {
                let films = self.selection_ordonnee();
                self.lancer_travail(Travail::AnalyserMedias {
                    films,
                    series: Vec::new(),
                });
            }
            Commande::AnnulerRenommage => {
                let ids = self.selection_ordonnee();
                self.lancer_travail(Travail::AnnulerRenommage { ids });
            }
            Commande::AjouterSource => {
                // Page des sources (saisie manuelle en repli) + sélecteur natif.
                self.appliquer_sans_recalcul(Message::OuvrirReglagesPage(Page::FilmsSource));
                self.choisir_dossier(CibleDossier::SourceFilms);
            }
            Commande::ScrapeInteractif => {
                let ids = self.selection_ordonnee();
                self.ouvrir_recherche(ids);
            }
            Commande::Editer => {
                let ids = self.selection_ordonnee();
                self.ouvrir_editeur(ids);
            }
            Commande::BasculerVu | Commande::BasculerVerrou => {
                let ids = self.selection.clone();
                let mut modifies = Vec::new();
                for f in self.films.iter_mut().filter(|f| ids.contains(&f.id)) {
                    if commande == Commande::BasculerVu {
                        f.vu = !f.vu;
                    } else {
                        f.verrouille = !f.verrouille;
                    }
                    modifies.push(f.clone());
                }
                if let Err(e) = self.source.enregistrer_films(&modifies) {
                    self.notifier(Niveau::Erreur, e);
                }
                self.recalculer();
            }
            Commande::RetirerBase => self.demander_confirmation(ActionConfirmee::RetirerBase),
            Commande::SupprimerFichiers => {
                self.demander_confirmation(ActionConfirmee::SupprimerFichiers)
            }
            Commande::Renommer | Commande::ApercuRenommage => {
                self.ouvrir_dialogue(TypeDialogue::ApercuRenommage)
            }
            autre => self.notifier_bientot(autre),
        }
    }

    fn avertir_aucune_selection(&mut self) {
        let texte = self.i18n.t("app.nothingselected");
        self.notifier(Niveau::Avertissement, texte);
    }

    /// Fonction pas encore réalisée : « bientôt disponible ».
    fn notifier_bientot(&mut self, commande: Commande) {
        let action = self.i18n.t(commande.cle());
        let texte = self
            .i18n
            .t_args("gui.notimplemented", &[("action", action.as_str())]);
        self.notifier(Niveau::Info, texte);
    }

    // ---- commandes du module Séries

    /// Série du nœud sélectionné dans l'arbre.
    pub fn serie_selectionnee(&self) -> Option<&SerieAffichage> {
        let id = self.noeud_serie.as_ref()?.id_serie();
        self.series.iter().find(|s| s.id == id)
    }

    /// Les commandes du menu s'appliquent à la série du nœud sélectionné.
    fn executer_serie(&mut self, commande: Commande) {
        match commande {
            Commande::MettreAJourToutesSources | Commande::MettreAJourSelection => {
                return self.lancer_travail(Travail::MettreAJourSeries);
            }
            Commande::AjouterSource => {
                self.appliquer_sans_recalcul(Message::OuvrirReglagesPage(Page::SeriesSource));
                return self.choisir_dossier(CibleDossier::SourceSeries);
            }
            Commande::ScrapeNonTraites => {
                let ids: Vec<String> = self
                    .series
                    .iter()
                    .filter(|s| !s.traitee() && !s.verrouillee)
                    .map(|s| s.id.clone())
                    .collect();
                if ids.is_empty() {
                    let texte = self.i18n.t("gui.scrape.nothingtv");
                    self.notifier(Niveau::Info, texte);
                } else {
                    self.lancer_travail(Travail::RecupererSeries { ids, artwork: true });
                }
                return;
            }
            _ => {}
        }
        let Some(serie) = self.serie_selectionnee().cloned() else {
            return self.avertir_aucune_selection();
        };
        let id = serie.id.clone();
        match commande {
            Commande::ScrapeInteractif => self.ouvrir_recherche_serie(&serie),
            Commande::ScrapeMeilleur | Commande::ScrapeMetadonnees => {
                self.lancer_travail(Travail::RecupererSeries {
                    ids: vec![id],
                    artwork: commande == Commande::ScrapeMeilleur,
                });
            }
            Commande::Editer => self.ouvrir_editeur_serie(&serie),
            Commande::BasculerVerrou => {
                let mut modifiee = serie;
                modifiee.verrouillee = !modifiee.verrouillee;
                if let Err(e) = self.source.enregistrer_serie(&modifiee) {
                    self.notifier(Niveau::Erreur, e);
                } else if let Some(x) = self.series.iter_mut().find(|s| s.id == modifiee.id) {
                    *x = modifiee;
                }
            }
            Commande::ReecrireNfo => {
                self.lancer_travail(Travail::EcrireNfoSeries { ids: vec![id] })
            }
            Commande::InfosMedia => self.lancer_travail(Travail::AnalyserMedias {
                films: Vec::new(),
                series: vec![id],
            }),
            Commande::Renommer | Commande::ApercuRenommage => {
                self.ouvrir_dialogue(TypeDialogue::ApercuRenommage)
            }
            Commande::TrouverManquants => {
                self.lancer_manquants(
                    "gui.missing.titletv",
                    vec![CibleManquants::Serie(id, serie.titre)],
                );
            }
            Commande::OuvrirDossier => {
                if serie.chemin.is_empty() {
                    let texte = self.i18n.t("gui.open.nofolder");
                    self.notifier(Niveau::Avertissement, texte);
                } else {
                    self.ouvrir(ouverture::cible_dossier(&serie.chemin, None));
                }
            }
            autre => self.notifier_bientot(autre),
        }
    }

    /// Recherche manuelle d'une série (dialogue de choix d'un résultat).
    fn ouvrir_recherche_serie(&mut self, serie: &SerieAffichage) {
        let dossier = std::path::Path::new(&serie.chemin)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (texte, annee) = titre_nettoye(&dossier);
        let texte = if texte.is_empty() {
            serie.titre.clone()
        } else {
            texte
        };
        let extracteur = self
            .source
            .extracteurs()
            .into_iter()
            .next()
            .unwrap_or_default();
        self.dialogue = Some(Dialogue::Recherche(Box::new(EtatRecherche {
            film_id: 0,
            serie: Some(serie.id.clone()),
            requete: RequeteRecherche {
                film: 0,
                extracteur,
                langue: self.reglages.langue.code().into(),
                texte,
                annee: annee.or(Some(serie.annee)).filter(|a| *a > 0),
            },
            resultats: Vec::new(),
            selection: None,
            en_cours: false,
            id_requete: 0,
            erreur: None,
            ne_pas_remplacer: false,
            file: Vec::new(),
            total: 1,
        })));
        self.lancer_recherche();
    }

    // ---- éditeur de série

    fn ouvrir_editeur_serie(&mut self, serie: &SerieAffichage) {
        self.dialogue = Some(Dialogue::EditeurSerie(Box::new(EtatEditeurSerie {
            id: serie.id.clone(),
            brouillon: SerieEdite::depuis(serie),
            erreurs: Vec::new(),
        })));
    }

    fn action_editeur_serie(&mut self, action: ActionEditeurSerie) {
        let Some(Dialogue::EditeurSerie(e)) = &mut self.dialogue else {
            return;
        };
        match action {
            ActionEditeurSerie::Brouillon(b) => {
                e.brouillon = *b;
                // Les erreurs disparaissent au fil de la correction.
                if !e.erreurs.is_empty() {
                    e.erreurs = valider_serie(&e.brouillon);
                }
            }
            ActionEditeurSerie::Abandonner => self.dialogue = None,
            ActionEditeurSerie::Valider => {
                let erreurs = valider_serie(&e.brouillon);
                if !erreurs.is_empty() {
                    e.erreurs = erreurs;
                    return;
                }
                let (id, brouillon) = (e.id.clone(), e.brouillon.clone());
                self.dialogue = None;
                let Some(serie) = self.series.iter_mut().find(|s| s.id == id) else {
                    return;
                };
                brouillon.appliquer(serie);
                let copie = serie.clone();
                if let Err(err) = self.source.enregistrer_serie(&copie) {
                    self.notifier(Niveau::Erreur, err);
                }
            }
        }
    }

    // ---- éléments manquants

    /// Sagas dont chercher les films manquants : celle du nœud choisi dans Collections, sinon
    /// celles des films sélectionnés.
    fn sagas_a_verifier(&self) -> Vec<CibleManquants> {
        let mut v: Vec<CibleManquants> = Vec::new();
        let mut ajouter = |s: &SagaAffichage| {
            if !v
                .iter()
                .any(|c| matches!(c, CibleManquants::Saga(id, _) if *id == s.id))
            {
                v.push(CibleManquants::Saga(s.id.clone(), s.titre.clone()));
            }
        };
        if self.module == Module::Collections
            && let Some(saga) = self
                .noeud_saga
                .as_ref()
                .and_then(|n| self.saga(n.id_saga()))
        {
            ajouter(saga);
            return v;
        }
        for f in self.films_selectionnes() {
            if let Some(titre) = &f.saga
                && let Some(saga) = self.sagas.iter().find(|s| &s.titre == titre)
            {
                ajouter(saga);
            }
        }
        v
    }

    fn trouver_manquants_films(&mut self) {
        let cibles = self.sagas_a_verifier();
        if cibles.is_empty() {
            let texte = self.i18n.t("gui.missing.nosaga");
            self.notifier(Niveau::Avertissement, texte);
            return;
        }
        self.lancer_manquants("gui.missing.title", cibles);
    }

    /// Ouvre le dialogue des manquants et lance la recherche hors du thread de l'interface.
    fn lancer_manquants(&mut self, cle_titre: &'static str, cibles: Vec<CibleManquants>) {
        let id_requete = self.prochain_id_requete;
        self.prochain_id_requete += 1;
        self.dialogue = Some(Dialogue::Manquants(EtatManquants {
            cle_titre,
            en_cours: true,
            id_requete,
            ..EtatManquants::default()
        }));
        let (source, retour) = (self.source.clone(), self.retour.clone());
        std::thread::spawn(move || {
            let mut groupes = Vec::new();
            let mut erreurs = Vec::new();
            for c in cibles {
                let (titre, resultat) = match c {
                    CibleManquants::Saga(id, titre) => (titre, source.films_manquants(&id)),
                    CibleManquants::Serie(id, titre) => (titre, source.episodes_manquants(&id)),
                };
                match resultat {
                    Ok(elements) => groupes.push(GroupeManquants { titre, elements }),
                    Err(e) => erreurs.push(format!("{titre} : {e}")),
                }
            }
            retour.envoyer(Message::ManquantsTrouves {
                id_requete,
                groupes,
                erreurs,
            });
        });
    }

    // ---- sélecteur de fichiers et outils d'analyse

    /// Remplace le sélecteur de fichiers (tests).
    pub fn definir_selecteur_fichier(&mut self, selecteur: SelecteurFichier) {
        self.selecteur_fichier = selecteur;
    }

    fn choisir_fichier(&mut self, cible: CibleFichier) {
        let titre = self.i18n.t("gui.tools.choose");
        let selecteur = self.selecteur_fichier.clone();
        let retour = self.retour.clone();
        let lance = std::thread::Builder::new()
            .name("selecteur-fichier".into())
            .spawn(move || {
                let chemin = selecteur(&titre);
                retour.envoyer(Message::FichierChoisi(cible, chemin));
            });
        if let Err(e) = lance {
            self.notifier(Niveau::Erreur, e.to_string());
        }
    }

    fn fichier_choisi(&mut self, cible: CibleFichier, chemin: PathBuf) {
        match cible {
            CibleFichier::OutilsAnalyse => {
                self.services.chemin_outils = chemin.display().to_string();
                self.services_modifies = true;
                if let Some(Dialogue::Reglages(d)) = &mut self.dialogue {
                    d.test_outils = None;
                } else {
                    self.enregistrer_services();
                }
            }
        }
    }

    /// Teste les outils d'analyse avec le chemin saisi (hors du thread de l'interface).
    fn tester_outils(&mut self) {
        if let Some(Dialogue::Reglages(d)) = &mut self.dialogue {
            d.test_outils_en_cours = true;
            d.test_outils = None;
        }
        let chemin = self.services.chemin_outils.clone();
        let (source, retour) = (self.source.clone(), self.retour.clone());
        std::thread::spawn(move || {
            retour.envoyer(Message::ResultatTestOutils(source.tester_outils(&chemin)));
        });
    }

    /// Ouvre l'incrustation de confirmation, sauf si l'utilisateur l'a masquée.
    fn demander_confirmation(&mut self, action: ActionConfirmee) {
        // Retrait : case « déplacer les fichiers » décochée ; suppression : cochée.
        let case = match action {
            ActionConfirmee::RetirerBase => Some(CaseConfirmation {
                cle: "confirm.movefiles",
                cochee: false,
            }),
            ActionConfirmee::SupprimerFichiers => Some(CaseConfirmation {
                cle: "confirm.movefiles",
                cochee: true,
            }),
            ActionConfirmee::Quitter => None,
        };
        if self.confirmations_masquees.contains(action.cle_masquage()) {
            self.executer_confirme(action, case.is_some_and(|c| c.cochee));
            return;
        }
        let n = self.selection.len().to_string();
        let (titre, texte) = match action {
            ActionConfirmee::RetirerBase => (
                self.i18n.t("movie.remove"),
                self.i18n
                    .t_args("confirm.remove.text", &[("count", n.as_str())]),
            ),
            ActionConfirmee::SupprimerFichiers => (
                self.i18n.t("movie.delete"),
                self.i18n
                    .t_args("confirm.delete.text", &[("count", n.as_str())]),
            ),
            ActionConfirmee::Quitter => (
                self.i18n.t("app.exit.confirmation"),
                self.i18n.t("app.exit.runningtasks"),
            ),
        };
        self.dialogue = Some(Dialogue::Confirmation(EtatConfirmation {
            titre,
            texte,
            action,
            masquable: action != ActionConfirmee::Quitter,
            ne_plus_demander: false,
            case,
        }));
    }

    fn confirmer(&mut self) {
        let Some(Dialogue::Confirmation(c)) = self.dialogue.take() else {
            return;
        };
        if c.masquable && c.ne_plus_demander {
            self.confirmations_masquees
                .insert(c.action.cle_masquage().to_string());
        }
        self.executer_confirme(c.action, c.case.is_some_and(|x| x.cochee));
    }

    fn executer_confirme(&mut self, action: ActionConfirmee, deplacer_fichiers: bool) {
        match action {
            ActionConfirmee::RetirerBase | ActionConfirmee::SupprimerFichiers => {
                let liste: Vec<u64> = self.selection.iter().copied().collect();
                let retires = match self.source.retirer_films(&liste, deplacer_fichiers) {
                    Ok(v) => v,
                    Err(e) => {
                        self.notifier(Niveau::Erreur, e);
                        return;
                    }
                };
                // Retrait immédiat (source de démonstration) ; avec les services, le bilan et
                // la disparition des films arrivent à la fin de la tâche.
                if retires.is_empty() {
                    return;
                }
                self.films.retain(|f| !retires.contains(&f.id));
                self.principal = None;
                self.ancre = None;
                self.recalculer();
                let compte = retires.len().to_string();
                let cle = if deplacer_fichiers {
                    "gui.deleted"
                } else {
                    "gui.removed"
                };
                let texte = self.i18n.t_args(cle, &[("count", compte.as_str())]);
                self.notifier(Niveau::Info, texte);
            }
            ActionConfirmee::Quitter => {
                for t in &self.taches {
                    self.source.annuler_tache(t.id);
                }
                self.quitter = true;
            }
        }
    }

    // ---------------------------------------------------------------- dialogues

    fn ouvrir_dialogue(&mut self, type_dialogue: TypeDialogue) {
        match type_dialogue {
            TypeDialogue::ApercuRenommage if self.module == Module::Series => {
                let Some(id) = self.serie_selectionnee().map(|s| s.id.clone()) else {
                    return self.avertir_aucune_selection();
                };
                let (apercus, erreur) = match self
                    .source
                    .apercu_renommage_series(std::slice::from_ref(&id))
                {
                    Ok(v) => (v, None),
                    Err(e) => (Vec::new(), Some(e)),
                };
                let film_actif = apercus.first().map(|x| x.id);
                self.dialogue = Some(Dialogue::ApercuRenommage(EtatApercu {
                    ids: Vec::new(),
                    masquer_inchanges: false,
                    film_actif,
                    apercus,
                    erreur,
                    series: Some(vec![id]),
                }));
            }
            TypeDialogue::ApercuRenommage => {
                if self.selection.is_empty() {
                    let texte = self.i18n.t("app.nothingselected");
                    self.notifier(Niveau::Avertissement, texte);
                    return;
                }
                let ids = self.selection_ordonnee();
                let (apercus, erreur) = match self.source.apercu_renommage(&ids) {
                    Ok(v) => (v, None),
                    Err(e) => (Vec::new(), Some(e)),
                };
                // Le premier film de l'aperçu est affiché par défaut.
                let film_actif = apercus.first().map(|x| x.id);
                self.dialogue = Some(Dialogue::ApercuRenommage(EtatApercu {
                    ids,
                    masquer_inchanges: false,
                    film_actif,
                    apercus,
                    erreur,
                    series: None,
                }));
            }
            TypeDialogue::Reglages => {
                self.services = self.source.reglages_services();
                self.services_modifies = false;
                self.dialogue = Some(Dialogue::Reglages(EtatDialogueReglages::default()));
            }
            TypeDialogue::Messages => {
                self.dialogue = Some(Dialogue::Messages);
                self.non_lus = 0;
            }
            TypeDialogue::Assistant => {
                self.dialogue = Some(Dialogue::Assistant(Box::new(EtatAssistant::new(
                    self.reglages.langue,
                    self.reglages.theme,
                    self.services.sources_films.clone(),
                ))));
            }
        }
    }

    fn action_dialogue(&mut self, action: ActionDialogue) {
        match (&mut self.dialogue, action) {
            (Some(Dialogue::ApercuRenommage(a)), ActionDialogue::ApercuMasquer(m)) => {
                a.masquer_inchanges = m
            }
            (Some(Dialogue::ApercuRenommage(a)), ActionDialogue::ApercuFilm(id)) => {
                a.film_actif = Some(id)
            }
            (Some(Dialogue::Reglages(r)), ActionDialogue::ReglagesPage(p)) => r.page = p,
            (Some(Dialogue::Reglages(r)), ActionDialogue::ReglagesFiltre(f)) => r.filtre = f,
            (Some(Dialogue::Reglages(r)), ActionDialogue::ReglagesSaisieSource(s)) => {
                r.saisie_source = s
            }
            (Some(Dialogue::Reglages(r)), ActionDialogue::ReglagesSaisieSourceSerie(s)) => {
                r.saisie_source_serie = s
            }
            (Some(Dialogue::Reglages(r)), ActionDialogue::ReglagesSaisieCle(s)) => r.saisie_cle = s,
            (Some(Dialogue::Reglages(r)), ActionDialogue::ReglagesEnregistrerCle) => {
                let cle = std::mem::take(&mut r.saisie_cle);
                if !cle.trim().is_empty() {
                    match self.source.transmettre_secret(SECRET_CLE_TMDB, cle.trim()) {
                        Ok(()) => {
                            self.services.generales.cle_tmdb_definie = true;
                            let texte = self.i18n.t("gui.settings.keysaved");
                            self.notifier(Niveau::Info, texte);
                        }
                        Err(e) => self.notifier(Niveau::Erreur, e),
                    }
                }
            }
            (Some(Dialogue::Confirmation(c)), ActionDialogue::ConfirmationNePlusDemander(v)) => {
                c.ne_plus_demander = v
            }
            (Some(Dialogue::Editeur(_)), ActionDialogue::Editeur(a)) => self.action_editeur(a),
            (Some(Dialogue::Recherche(_)), ActionDialogue::Recherche(a)) => {
                self.action_recherche(a)
            }
            (Some(Dialogue::EditeurSerie(_)), ActionDialogue::EditeurSerie(a)) => {
                self.action_editeur_serie(a)
            }
            (Some(Dialogue::Confirmation(c)), ActionDialogue::ConfirmationCase(v)) => {
                if let Some(case) = &mut c.case {
                    case.cochee = v;
                }
            }
            (Some(Dialogue::Manquants(m)), ActionDialogue::ManquantsChoisir(g, e)) => {
                if m.groupes.get(g).is_some_and(|x| e < x.elements.len()) {
                    m.selection = Some((g, e));
                }
            }
            (Some(Dialogue::Assistant(_)), ActionDialogue::Assistant(a)) => {
                self.action_assistant(a)
            }
            _ => {}
        }
    }

    // ---- éditeur de film

    fn ouvrir_editeur(&mut self, ids: Vec<u64>) {
        let mut file: Vec<u64> = ids.into_iter().rev().collect();
        let total = file.len();
        let Some(id) = file.pop() else { return };
        let Some(film) = self.film(id) else { return };
        self.dialogue = Some(Dialogue::Editeur(Box::new(EtatEditeur {
            id,
            onglet: OngletEditeur::Details,
            brouillon: FilmEdite::depuis(film),
            erreurs: Vec::new(),
            file,
            total,
        })));
    }

    /// Passe au film suivant de la file (ou ferme l'éditeur).
    fn editeur_suivant(&mut self, mut file: Vec<u64>, total: usize, onglet: OngletEditeur) {
        while let Some(id) = file.pop() {
            if let Some(film) = self.film(id) {
                self.dialogue = Some(Dialogue::Editeur(Box::new(EtatEditeur {
                    id,
                    onglet,
                    brouillon: FilmEdite::depuis(film),
                    erreurs: Vec::new(),
                    file,
                    total,
                })));
                return;
            }
        }
        self.dialogue = None;
    }

    fn action_editeur(&mut self, action: ActionEditeur) {
        let Some(Dialogue::Editeur(e)) = &mut self.dialogue else {
            return;
        };
        match action {
            ActionEditeur::Onglet(o) => e.onglet = o,
            ActionEditeur::Brouillon(b) => {
                e.brouillon = *b;
                // Les erreurs disparaissent au fil de la correction.
                if !e.erreurs.is_empty() {
                    e.erreurs = valider(&e.brouillon);
                }
            }
            ActionEditeur::Abandonner => self.dialogue = None,
            ActionEditeur::Ignorer => {
                let (file, total, onglet) = (std::mem::take(&mut e.file), e.total, e.onglet);
                self.editeur_suivant(file, total, onglet);
            }
            ActionEditeur::Valider => {
                let erreurs = valider(&e.brouillon);
                if !erreurs.is_empty() {
                    e.erreurs = erreurs;
                    return;
                }
                let (id, brouillon, file, total, onglet) = (
                    e.id,
                    e.brouillon.clone(),
                    std::mem::take(&mut e.file),
                    e.total,
                    e.onglet,
                );
                if let Some(f) = self.films.iter_mut().find(|f| f.id == id) {
                    brouillon.appliquer(f);
                    let copie = f.clone();
                    if let Err(err) = self.source.enregistrer_films(&[copie]) {
                        self.notifier(Niveau::Erreur, err);
                    }
                }
                self.recalculer();
                self.editeur_suivant(file, total, onglet);
            }
        }
    }

    // ---- recherche d'extracteur

    fn ouvrir_recherche(&mut self, ids: Vec<u64>) {
        let mut file: Vec<u64> = ids.into_iter().rev().collect();
        let total = file.len();
        self.recherche_suivante(&mut file, total, false);
    }

    fn recherche_suivante(&mut self, file: &mut Vec<u64>, total: usize, ne_pas_remplacer: bool) {
        while let Some(id) = file.pop() {
            let Some(film) = self.film(id) else { continue };
            let (texte, annee) = titre_nettoye(&film.dossier);
            let texte = if texte.is_empty() {
                film.titre.clone()
            } else {
                texte
            };
            let extracteur = self
                .source
                .extracteurs()
                .into_iter()
                .next()
                .unwrap_or_default();
            self.dialogue = Some(Dialogue::Recherche(Box::new(EtatRecherche {
                film_id: id,
                serie: None,
                requete: RequeteRecherche {
                    film: id,
                    extracteur,
                    langue: self.reglages.langue.code().into(),
                    texte,
                    annee: annee.or(Some(film.annee)).filter(|a| *a > 0),
                },
                resultats: Vec::new(),
                selection: None,
                en_cours: false,
                id_requete: 0,
                erreur: None,
                ne_pas_remplacer,
                file: std::mem::take(file),
                total,
            })));
            self.lancer_recherche();
            return;
        }
        self.dialogue = None;
    }

    /// Lance la recherche en tâche de fond ; une réponse périmée sera ignorée.
    fn lancer_recherche(&mut self) {
        let id_requete = self.prochain_id_requete;
        let Some(Dialogue::Recherche(r)) = &mut self.dialogue else {
            return;
        };
        self.prochain_id_requete += 1;
        r.id_requete = id_requete;
        r.en_cours = true;
        r.erreur = None;
        r.resultats.clear();
        r.selection = None;
        let requete = r.requete.clone();
        let serie = r.serie.clone();
        let source = self.source.clone();
        let retour = self.retour.clone();
        std::thread::spawn(move || {
            let resultat = match serie {
                Some(serie) => source.rechercher_serie(&RequeteRechercheSerie {
                    serie,
                    texte: requete.texte,
                    annee: requete.annee,
                    langue: requete.langue,
                }),
                None => source.rechercher(&requete),
            };
            retour.envoyer(Message::ResultatsRecherche {
                id_requete,
                resultat,
            });
        });
    }

    fn recevoir_resultats(
        &mut self,
        id_requete: u64,
        resultat: Result<Vec<ResultatRecherche>, String>,
    ) {
        let Some(Dialogue::Recherche(r)) = &mut self.dialogue else {
            return;
        };
        if r.id_requete != id_requete {
            return;
        }
        r.en_cours = false;
        match resultat {
            Ok(v) => {
                r.selection = if v.is_empty() { None } else { Some(0) };
                r.resultats = v;
            }
            Err(e) => r.erreur = Some(e),
        }
    }

    fn action_recherche(&mut self, action: ActionRecherche) {
        let Some(Dialogue::Recherche(r)) = &mut self.dialogue else {
            return;
        };
        match action {
            ActionRecherche::Texte(t) => r.requete.texte = t,
            ActionRecherche::Annee(a) => r.requete.annee = a,
            ActionRecherche::Extracteur(x) => r.requete.extracteur = x,
            ActionRecherche::Langue(l) => r.requete.langue = l,
            ActionRecherche::NePasRemplacer(v) => r.ne_pas_remplacer = v,
            ActionRecherche::Choisir(n) => {
                if n < r.resultats.len() {
                    r.selection = Some(n);
                }
            }
            ActionRecherche::Lancer => self.lancer_recherche(),
            ActionRecherche::Abandonner => self.dialogue = None,
            ActionRecherche::Ignorer => {
                let (mut file, total, npr) =
                    (std::mem::take(&mut r.file), r.total, r.ne_pas_remplacer);
                self.recherche_suivante(&mut file, total, npr);
            }
            ActionRecherche::Valider => {
                let Some(choix) = r.resultat_choisi().cloned() else {
                    return;
                };
                let (id, npr, mut file, total, serie) = (
                    r.film_id,
                    r.ne_pas_remplacer,
                    std::mem::take(&mut r.file),
                    r.total,
                    r.serie.clone(),
                );
                // Métadonnées, images et NFO en tâche principale : le film (ou la série) sera
                // rafraîchi à la fin.
                self.lancer_travail(match serie {
                    Some(serie) => Travail::AppliquerResultatSerie {
                        serie,
                        resultat: choix,
                        ne_pas_remplacer: npr,
                    },
                    None => Travail::AppliquerResultat {
                        film: id,
                        resultat: choix,
                        ne_pas_remplacer: npr,
                    },
                });
                self.recherche_suivante(&mut file, total, npr);
            }
        }
    }

    // ---- sélecteur de dossiers natif

    /// Remplace le sélecteur de dossiers (tests).
    pub fn definir_selecteur(&mut self, selecteur: SelecteurDossier) {
        self.selecteur = selecteur;
    }

    fn choisir_dossier(&mut self, cible: CibleDossier) {
        let titre = self.i18n.t("gui.folder.choose");
        let selecteur = self.selecteur.clone();
        let retour = self.retour.clone();
        let lance = std::thread::Builder::new()
            .name("selecteur-dossier".into())
            .spawn(move || {
                let chemin = selecteur(&titre);
                retour.envoyer(Message::DossierChoisi(cible, chemin));
            });
        if let Err(e) = lance {
            self.notifier(Niveau::Erreur, e.to_string());
        }
    }

    fn dossier_choisi(&mut self, cible: CibleDossier, chemin: PathBuf) {
        let texte = chemin.display().to_string();
        let liste = match cible {
            CibleDossier::Assistant => {
                if let Some(Dialogue::Assistant(a)) = &mut self.dialogue {
                    a.saisie_source = texte;
                    a.ajouter_source();
                }
                return;
            }
            CibleDossier::SourceFilms => &mut self.services.sources_films,
            CibleDossier::SourceSeries => &mut self.services.sources_series,
        };
        if liste.contains(&texte) {
            return;
        }
        liste.push(texte.clone());
        self.services_modifies = true;
        // Dialogue des réglages ouvert : enregistrement à sa fermeture ; sinon immédiat.
        if !matches!(self.dialogue, Some(Dialogue::Reglages(_))) {
            self.enregistrer_services();
            let t = self
                .i18n
                .t_args("gui.folder.added", &[("folder", texte.as_str())]);
            self.notifier(Niveau::Info, t);
        }
    }

    // ---- mise à jour du programme

    fn rechercher_mise_a_jour(&mut self, manuel: bool) {
        let config = self.config_mise_a_jour.clone();
        if manuel {
            self.dialogue = Some(Dialogue::MiseAJour(EtatDialogueMiseAJour {
                phase: PhaseMiseAJour::Recherche,
                publication: None,
            }));
        } else if config.courant.build.is_none() {
            // Compilation locale : pas de vérification automatique.
            return;
        }
        let retour = self.retour.clone();
        let lance = std::thread::Builder::new()
            .name("mise-a-jour".into())
            .spawn(move || {
                let resultat = mise_a_jour::verifier(&config).map_err(|e| e.to_string());
                retour.envoyer(Message::ResultatMiseAJour { manuel, resultat });
            });
        if let Err(e) = lance {
            self.notifier(Niveau::Erreur, e.to_string());
        }
    }

    fn resultat_mise_a_jour(
        &mut self,
        manuel: bool,
        resultat: Result<(Verdict, Publication), String>,
    ) {
        let dialogue_ouvert = matches!(self.dialogue, Some(Dialogue::MiseAJour(_)));
        if manuel && !dialogue_ouvert {
            return; // dialogue fermé pendant la recherche
        }
        let (phase, publication) = match resultat {
            Ok((Verdict::AJour, p)) => (PhaseMiseAJour::AJour, Some(p)),
            Ok((Verdict::Developpement, p)) => (PhaseMiseAJour::Developpement, Some(p)),
            Ok((Verdict::Disponible, p)) => (PhaseMiseAJour::Disponible, Some(p)),
            Err(e) => {
                if !manuel {
                    tracing::warn!("vérification de mise à jour : {e}");
                    return;
                }
                (PhaseMiseAJour::Erreur(e), None)
            }
        };
        if !manuel {
            if phase != PhaseMiseAJour::Disponible {
                return;
            }
            if self.dialogue.is_some() {
                // Un autre dialogue est ouvert : simple message.
                let version = publication
                    .as_ref()
                    .map(|p| p.libelle())
                    .unwrap_or_default();
                let t = self.i18n.t_args(
                    "gui.update.available.notice",
                    &[("version", version.as_str())],
                );
                self.notifier(Niveau::Info, t);
                return;
            }
        }
        self.dialogue = Some(Dialogue::MiseAJour(EtatDialogueMiseAJour {
            phase,
            publication,
        }));
    }

    fn installer_mise_a_jour(&mut self) {
        let Some(Dialogue::MiseAJour(d)) = &mut self.dialogue else {
            return;
        };
        if d.phase != PhaseMiseAJour::Disponible || self.config_mise_a_jour.courant.build.is_none()
        {
            return;
        }
        let Some(publication) = d.publication.clone() else {
            return;
        };
        d.phase = PhaseMiseAJour::Installation(EtapeInstallation::Telechargement {
            fait: 0,
            total: publication.taille_archive,
        });
        let config = self.config_mise_a_jour.clone();
        let retour = self.retour.clone();
        let lance = std::thread::Builder::new()
            .name("installation-mise-a-jour".into())
            .spawn(move || {
                let mut dernier = 0u64;
                let r = mise_a_jour::installer_publication(&config, &publication, &mut |e| {
                    // Progression bornée (un message tous les 256 Kio).
                    if let EtapeInstallation::Telechargement { fait, total } = &e
                        && *fait < dernier + 256 * 1024
                        && Some(*fait) != *total
                    {
                        return;
                    }
                    if let EtapeInstallation::Telechargement { fait, .. } = &e {
                        dernier = *fait;
                    }
                    retour.envoyer(Message::ProgressionMiseAJour(e));
                });
                retour.envoyer(Message::MiseAJourInstallee(r.map_err(|e| e.to_string())));
            });
        if let Err(e) = lance {
            self.notifier(Niveau::Erreur, e.to_string());
        }
    }

    fn mise_a_jour_installee(&mut self, r: Result<PathBuf, String>) {
        match r {
            Ok(_) => {
                let relance = mise_a_jour::exe_courant()
                    .ok_or_else(|| "exécutable introuvable".to_string())
                    .and_then(|exe| mise_a_jour::relancer(&exe).map_err(|e| e.to_string()));
                match relance {
                    Ok(()) => {
                        for t in &self.taches {
                            self.source.annuler_tache(t.id);
                        }
                        self.quitter = true;
                    }
                    Err(e) => {
                        let t = self
                            .i18n
                            .t_args("gui.update.restartfailed", &[("error", e.as_str())]);
                        self.notifier(Niveau::Avertissement, t.clone());
                        if let Some(Dialogue::MiseAJour(d)) = &mut self.dialogue {
                            d.phase = PhaseMiseAJour::Erreur(t);
                        }
                    }
                }
            }
            Err(e) => {
                let t = self
                    .i18n
                    .t_args("gui.update.installfailed", &[("error", e.as_str())]);
                self.notifier(Niveau::Erreur, t.clone());
                if let Some(Dialogue::MiseAJour(d)) = &mut self.dialogue {
                    d.phase = PhaseMiseAJour::Erreur(t);
                }
            }
        }
    }

    // ---- assistant de premier lancement

    fn action_assistant(&mut self, action: ActionAssistant) {
        let Some(Dialogue::Assistant(a)) = &mut self.dialogue else {
            return;
        };
        match action {
            ActionAssistant::Suivante => a.page = a.page.suivante(),
            ActionAssistant::Precedente => a.page = a.page.precedente(),
            ActionAssistant::Langue(l) => {
                // Aperçu immédiat : la langue s'applique dès la sélection.
                a.langue = l;
                if self.reglages.langue != l {
                    self.reglages.langue = l;
                    self.i18n = I18n::new(l);
                }
            }
            ActionAssistant::Theme(t) => {
                a.theme = t;
                self.reglages.theme = t;
            }
            ActionAssistant::SaisieSource(s) => a.saisie_source = s,
            ActionAssistant::AjouterSource => a.ajouter_source(),
            ActionAssistant::RetirerSource(n) => {
                if n < a.sources.len() {
                    a.sources.remove(n);
                }
            }
            ActionAssistant::CleTmdb(c) => a.cle_tmdb = c,
            ActionAssistant::LangueExtracteur(l) => a.langue_extracteur = l,
            ActionAssistant::Terminer => {
                let Some(Dialogue::Assistant(a)) = self.dialogue.take() else {
                    return;
                };
                self.services.sources_films = a.sources.clone();
                if !a.langue_extracteur.trim().is_empty() {
                    self.services.langue_scraping = a.langue_extracteur.trim().to_string();
                }
                self.services_modifies = true;
                self.enregistrer_services();
                if !a.cle_tmdb.trim().is_empty() {
                    // Transmise au coffre à secrets : jamais conservée dans l'état ni dans ui.json.
                    if let Err(e) = self
                        .source
                        .transmettre_secret(SECRET_CLE_TMDB, a.cle_tmdb.trim())
                    {
                        self.notifier(Niveau::Erreur, e);
                    }
                }
                self.assistant_termine = true;
                let texte = self.i18n.t("wizard.done");
                self.notifier(Niveau::Info, texte);
                if !self.services.sources_films.is_empty() {
                    self.lancer_travail(Travail::MettreAJourSources);
                }
            }
        }
    }

    /// Aperçu du renommage affiché par le dialogue (plan calculé à son ouverture).
    pub fn apercu_courant(&self) -> Vec<ApercuFilm> {
        match &self.dialogue {
            Some(Dialogue::ApercuRenommage(a)) => a.apercus.clone(),
            _ => Vec::new(),
        }
    }

    /// « Renommer » du dialogue : lance le renommage des films sans conflit.
    fn appliquer_renommage(&mut self) {
        if let Some(Dialogue::ApercuRenommage(a)) = &self.dialogue
            && let Some(series) = a.series.clone()
        {
            let a_faire = a.apercus.iter().any(|x| x.executable());
            self.dialogue = None;
            if a_faire {
                self.lancer_travail(Travail::RenommerSeries { ids: series });
            }
            return;
        }
        let ids: Vec<u64> = self
            .apercu_courant()
            .iter()
            .filter(|a| a.executable())
            .map(|a| a.id)
            .collect();
        self.dialogue = None;
        if !ids.is_empty() {
            self.lancer_travail(Travail::Renommer { ids });
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
        assert!(matches!(e.dialogue, Some(Dialogue::Confirmation(_))));
        assert_eq!(e.films.len(), total);
        e.appliquer(Message::Confirmer);
        assert_eq!(e.films.len(), total - 1);
        assert!(e.film(id).is_none());
        assert_eq!(e.selection.len(), 1); // la 1re ligne est resélectionnée
    }

    /// Pompe les messages jusqu'à ce que `fini` soit vrai (10 s au plus).
    fn attendre(e: &mut Etat, fini: impl Fn(&Etat) -> bool) {
        let fin = Instant::now() + Duration::from_secs(10);
        while !fini(e) && Instant::now() < fin {
            if let Ok(m) = e.rx.recv_timeout(Duration::from_millis(20)) {
                e.appliquer(m);
            }
        }
        assert!(fini(e), "délai dépassé");
    }

    /// Lanceur factice : mémorise les commandes, échoue sur demande.
    fn lanceur_espion(echec: bool) -> (Lanceur, Arc<Mutex<Vec<ouverture::CommandeSysteme>>>) {
        let vues: Arc<Mutex<Vec<ouverture::CommandeSysteme>>> = Arc::default();
        let copie = vues.clone();
        let l: Lanceur = Arc::new(move |c| {
            copie.lock().unwrap().push(c);
            if echec { Err("refusé".into()) } else { Ok(()) }
        });
        (l, vues)
    }

    #[test]
    fn ouvrir_le_dossier_et_lire_le_film() {
        let mut e = Etat::nouveau();
        let (l, vues) = lanceur_espion(false);
        e.lanceur = l;
        e.systeme = Systeme::Windows;
        let film = e.film_principal().unwrap().clone();
        e.appliquer(Message::Commande(Commande::OuvrirDossier));
        attendre(&mut e, |_| vues.lock().unwrap().len() == 1);
        e.appliquer(Message::Commande(Commande::Lire));
        attendre(&mut e, |_| vues.lock().unwrap().len() == 2);
        let v = vues.lock().unwrap().clone();
        assert_eq!(v[0].programme, "explorer");
        assert!(v[0].bruts);
        assert!(v[0].arguments[0].starts_with("/select,\""));
        assert!(v[0].arguments[0].ends_with(&format!("{}\"", film.nom_fichier)));
        assert_eq!(v[1].arguments.len(), 1);
        assert!(v[1].arguments[0].ends_with(&film.nom_fichier));
        assert!(!v[1].bruts);
        // Adresse refusée sans lancement ; film sans vidéo : avertissement.
        e.appliquer(Message::Ouvrir(CibleOuverture::Adresse(
            "javascript:x".into(),
        )));
        assert_eq!(
            e.notifications.last().unwrap().niveau,
            Niveau::Avertissement
        );
        let id = film.id;
        if let Some(f) = e.films.iter_mut().find(|f| f.id == id) {
            f.nom_fichier.clear();
        }
        e.appliquer(Message::Commande(Commande::Lire));
        assert!(e.notifications.last().unwrap().texte.contains("vidéo"));
        assert_eq!(vues.lock().unwrap().len(), 2);
    }

    #[test]
    fn echec_d_ouverture_notifie() {
        let mut e = Etat::nouveau();
        let (l, _) = lanceur_espion(true);
        e.lanceur = l;
        e.appliquer(Message::Ouvrir(CibleOuverture::Fichier("/x.mkv".into())));
        attendre(&mut e, |e| {
            e.notifications
                .iter()
                .any(|n| n.niveau == Niveau::Erreur && n.texte.contains("refusé"))
        });
    }

    #[test]
    fn collections_selection_et_film_cible() {
        let mut e = Etat::nouveau();
        assert_eq!(e.sagas.len(), 2);
        let dune = e.saga("demo-saga-dune").unwrap().clone();
        assert_eq!(dune.films.len(), 2);
        e.appliquer(Message::ChoisirModule(Module::Collections));
        assert!(e.film_cible().is_none());
        e.appliquer(Message::SelectionnerNoeudSaga(NoeudSaga::Film(
            dune.id.clone(),
            3,
        )));
        assert!(e.sagas_deployees.contains(&dune.id));
        assert_eq!(e.film_cible().map(|f| f.id), Some(3));
        e.appliquer(Message::BasculerSaga(dune.id.clone()));
        assert!(!e.sagas_deployees.contains(&dune.id));
        e.appliquer(Message::ChoisirModule(Module::Films));
        assert_eq!(e.film_cible().map(|f| f.id), e.principal);
    }

    #[test]
    fn tri_des_colonnes_taille_chemin_date_et_format() {
        let mut e = Etat::nouveau();
        let colonne_triee = |e: &mut Etat, c: Colonne| -> Vec<u64> {
            e.appliquer(Message::Trier {
                colonne: c,
                additif: false,
            });
            e.visibles.iter().map(|&i| e.films[i].id).collect()
        };
        for c in [
            Colonne::TailleFichier,
            Colonne::Chemin,
            Colonne::DateAjout,
            Colonne::FormatVideo,
        ] {
            let ids = colonne_triee(&mut e, c);
            let films: Vec<&Film> = ids.iter().map(|id| e.film(*id).unwrap()).collect();
            assert!(
                films.windows(2).all(|w| c.comparer(w[0], w[1]).is_le()),
                "{c:?} croissant"
            );
            // Second clic : ordre décroissant.
            let ids = colonne_triee(&mut e, c);
            let films: Vec<&Film> = ids.iter().map(|id| e.film(*id).unwrap()).collect();
            assert!(
                films.windows(2).all(|w| c.comparer(w[0], w[1]).is_ge()),
                "{c:?} décroissant"
            );
        }
        // Taille numérique (et non lexicographique) ; chemin sans casse ni accents.
        let a = Film {
            taille_octets: 9_000_000_000,
            source: "/films".into(),
            dossier: "évadés".into(),
            date_ajout: "2026-01-02 10:00".into(),
            ..Film::default()
        };
        let b = Film {
            taille_octets: 10_000_000_000,
            source: "/films".into(),
            dossier: "Fargo".into(),
            date_ajout: "2026-11-01 09:00".into(),
            ..Film::default()
        };
        assert!(Colonne::TailleFichier.comparer(&a, &b).is_lt());
        assert!(Colonne::Chemin.comparer(&a, &b).is_lt());
        assert!(Colonne::DateAjout.comparer(&a, &b).is_lt());
    }

    #[test]
    fn tache_en_arriere_plan_via_le_canal() {
        let mut e = Etat::avec_source(source_rapide());
        e.appliquer(Message::Commande(Commande::MettreAJourToutesSources));
        attendre(&mut e, |e| !e.taches.is_empty());
        let t = e.tache_active().unwrap();
        assert!(t.principale);
        assert_eq!(t.nom, "Mise à jour des sources");
        attendre(&mut e, |e| e.taches.is_empty());
        assert_eq!(
            e.notifications.last().unwrap().texte,
            "Tâche terminée : Mise à jour des sources"
        );
    }

    #[test]
    fn annulation_d_une_tache() {
        let mut s = SourceFactice::new(std::env::temp_dir().join("mmr-etat-annulation"));
        s.pas = Duration::from_millis(20);
        let mut e = Etat::avec_source(Arc::new(s));
        e.appliquer(Message::Commande(Commande::MettreAJourToutesSources));
        attendre(&mut e, |e| !e.taches.is_empty());
        let id = e.taches[0].id;
        e.appliquer(Message::ArreterTache(id));
        attendre(&mut e, |e| e.taches.is_empty());
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
        let mut e = Etat::avec_source(source_rapide());
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
        attendre(&mut e, |e| {
            e.film(id).unwrap().dossier == "Blade Runner 2049 (2017)" && e.taches.is_empty()
        });
        assert_eq!(
            e.film(id).unwrap().nom_fichier,
            "Blade Runner 2049 (2017).mkv"
        );
    }

    #[test]
    fn reglages_des_services_enregistres_a_la_fermeture() {
        let source = source_rapide();
        let mut e = Etat::avec_source(source.clone());
        e.appliquer(Message::Commande(Commande::AjouterSource));
        let Some(Dialogue::Reglages(d)) = &e.dialogue else {
            panic!("réglages attendus")
        };
        assert_eq!(d.page, Page::FilmsSource);
        let mut r = e.services.clone();
        r.sources_films.push("F:\\Nouveaux".into());
        e.appliquer(Message::AppliquerServices(Box::new(r)));
        assert!(
            !source
                .reglages_services()
                .sources_films
                .contains(&"F:\\Nouveaux".into())
        );
        e.appliquer(Message::FermerDialogue);
        assert!(
            source
                .reglages_services()
                .sources_films
                .contains(&"F:\\Nouveaux".into())
        );
        // Clé saisie dans les réglages : transmise au coffre, effacée de l'état.
        e.appliquer(Message::OuvrirDialogue(TypeDialogue::Reglages));
        e.appliquer(Message::Dialogue(ActionDialogue::ReglagesSaisieCle(
            "k1".into(),
        )));
        e.appliquer(Message::Dialogue(ActionDialogue::ReglagesEnregistrerCle));
        assert_eq!(source.secret(SECRET_CLE_TMDB).as_deref(), Some("k1"));
        let Some(Dialogue::Reglages(d)) = &e.dialogue else {
            panic!()
        };
        assert!(d.saisie_cle.is_empty());
    }

    #[test]
    fn films_et_series_actualises_par_la_source() {
        let mut e = Etat::nouveau();
        let total = e.films.len();
        let mut f = e.films[0].clone();
        f.titre = "Titre actualisé".into();
        e.appliquer(Message::FilmActualise(Box::new(f.clone())));
        assert_eq!(e.film(f.id).unwrap().titre, "Titre actualisé");
        f.id = 999;
        e.appliquer(Message::FilmActualise(Box::new(f)));
        assert_eq!(e.films.len(), total + 1);
        assert!(e.position(999).is_some());
        e.appliquer(Message::FilmRetire(999));
        assert_eq!(e.films.len(), total);
        assert_eq!(e.series.len(), 2);
        e.series.clear();
        e.appliquer(Message::SeriesModifiees);
        assert_eq!(e.series.len(), 2);
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

    fn source_rapide() -> Arc<SourceFactice> {
        let mut s = SourceFactice::new(
            std::env::temp_dir().join(format!("mmr-etat-{}", std::process::id())),
        );
        s.latence = Duration::ZERO;
        s.pas = Duration::from_millis(1);
        Arc::new(s)
    }

    fn attendre_recherche(e: &mut Etat) {
        let fin = Instant::now() + Duration::from_secs(10);
        while matches!(&e.dialogue, Some(Dialogue::Recherche(r)) if r.en_cours)
            && Instant::now() < fin
        {
            if let Ok(m) = e.rx.recv_timeout(Duration::from_millis(50)) {
                e.appliquer(m);
            }
        }
    }

    #[test]
    fn persistance_aller_retour_de_l_interface() {
        let mut e = Etat::nouveau();
        e.appliquer(Message::ColonneVisible(Colonne::Duree, true));
        e.appliquer(Message::DeplacerColonne(Colonne::Annee, -1));
        e.appliquer(Message::LargeursColonnes(vec![(Colonne::Titre, 333.0)]));
        e.appliquer(Message::LargeurDetails(480.0));
        e.appliquer(Message::ChoisirModule(Module::Series));
        e.appliquer(Message::Trier {
            colonne: Colonne::Note,
            additif: false,
        });
        e.appliquer(Message::Filtre(ActionFiltre::EnregistrerPrereglage(
            "vide".into(),
        )));
        e.confirmations_masquees.insert("confirm.remove".into());
        e.assistant_termine = true;
        let ui = e.instantane_interface();
        assert_eq!(ui.colonnes[0].id, "year");
        let mut f = Etat::nouveau();
        f.restaurer_interface(&ui);
        assert_eq!(f.colonnes_visibles()[..2], [Colonne::Annee, Colonne::Titre]);
        assert!(f.colonnes_visibles().contains(&Colonne::Duree));
        assert_eq!(f.largeur_colonne(Colonne::Titre), 333.0);
        assert_eq!(f.largeur_details, Some(480.0));
        assert_eq!(f.module, Module::Series);
        assert_eq!(f.tri, [(Colonne::Note, Ordre::Croissant)]);
        assert!(f.filtres.prereglages.contains_key("vide"));
        assert!(f.dialogue.is_none()); // assistant déjà terminé
        assert_eq!(f.instantane_interface(), ui);
    }

    #[test]
    fn restauration_tolerante_et_assistant_au_premier_lancement() {
        let mut ui = EtatInterface::default();
        ui.colonnes.push(ColonnePersistee {
            id: "inconnue".into(),
            visible: true,
            largeur: Some(f32::NAN),
        });
        ui.colonnes.push(ColonnePersistee {
            id: "title".into(),
            visible: false,
            largeur: None,
        });
        let mut e = Etat::nouveau();
        e.restaurer_interface(&ui);
        assert_eq!(e.colonnes.len(), Colonne::TOUTES.len());
        assert!(e.colonnes_visibles().contains(&Colonne::Titre));
        assert!(matches!(e.dialogue, Some(Dialogue::Assistant(_))));
    }

    #[test]
    fn confirmation_masquable() {
        let mut e = Etat::nouveau();
        let total = e.films.len();
        e.appliquer(Message::Commande(Commande::SupprimerFichiers));
        e.appliquer(Message::Dialogue(
            ActionDialogue::ConfirmationNePlusDemander(true),
        ));
        e.appliquer(Message::Confirmer);
        assert_eq!(e.films.len(), total - 1);
        assert!(e.confirmations_masquees.contains("confirm.delete"));
        e.appliquer(Message::Commande(Commande::SupprimerFichiers));
        assert!(e.dialogue.is_none());
        assert_eq!(e.films.len(), total - 2);
        // annuler = fermer sans rien faire
        e.appliquer(Message::Commande(Commande::RetirerBase));
        e.appliquer(Message::FermerDialogue);
        assert_eq!(e.films.len(), total - 2);
    }

    #[test]
    fn quitter_avec_des_taches_demande_confirmation() {
        let mut e = Etat::nouveau();
        e.appliquer(Message::DemandeQuitter);
        assert!(e.quitter);
        let mut s = SourceFactice::new(std::env::temp_dir().join("mmr-etat-quitter"));
        s.pas = Duration::from_millis(50);
        let mut e = Etat::avec_source(Arc::new(s));
        e.appliquer(Message::Commande(Commande::MettreAJourToutesSources));
        attendre(&mut e, |e| !e.taches.is_empty());
        assert!(!e.quitter_autorise());
        e.appliquer(Message::DemandeQuitter);
        assert!(matches!(e.dialogue, Some(Dialogue::Confirmation(_))));
        e.appliquer(Message::Confirmer);
        assert!(e.quitter_autorise());
    }

    #[test]
    fn filtres_avances_dans_la_vue() {
        let mut e = Etat::nouveau();
        let mut l = e.filtres.ligne(crate::filtres::IdFiltre::Vu).clone();
        l.etat = crate::filtres::EtatFiltre::Exclure;
        e.appliquer(Message::Filtre(ActionFiltre::Ligne(Box::new(l))));
        assert!(e.filtre_actif());
        assert!(e.visibles.iter().all(|&i| !e.films[i].vu));
        e.appliquer(Message::EffacerFiltres);
        assert!(!e.filtre_actif());
        assert_eq!(e.visibles.len(), e.films.len());
    }

    #[test]
    fn editeur_validation_et_file() {
        let mut e = Etat::nouveau();
        e.appliquer(Message::SelectionnerTout);
        e.appliquer(Message::Commande(Commande::Editer));
        let Some(Dialogue::Editeur(ed)) = &e.dialogue else {
            panic!("éditeur attendu")
        };
        assert_eq!((ed.position(), ed.total), (1, e.films.len()));
        let premier = ed.id;
        let mut b = ed.brouillon.clone();
        b.titre.clear();
        e.appliquer(Message::Dialogue(ActionDialogue::Editeur(
            ActionEditeur::Brouillon(Box::new(b.clone())),
        )));
        e.appliquer(Message::Dialogue(ActionDialogue::Editeur(
            ActionEditeur::Valider,
        )));
        let Some(Dialogue::Editeur(ed)) = &e.dialogue else {
            panic!()
        };
        assert_eq!(ed.id, premier);
        assert!(ed.erreur(crate::dialogues::ChampEditeur::Titre).is_some());
        b.titre = "Alien (modifié)".into();
        e.appliquer(Message::Dialogue(ActionDialogue::Editeur(
            ActionEditeur::Brouillon(Box::new(b)),
        )));
        let Some(Dialogue::Editeur(ed)) = &e.dialogue else {
            panic!()
        };
        assert!(ed.erreurs.is_empty());
        e.appliquer(Message::Dialogue(ActionDialogue::Editeur(
            ActionEditeur::Valider,
        )));
        assert_eq!(e.film(premier).unwrap().titre, "Alien (modifié)");
        let Some(Dialogue::Editeur(ed)) = &e.dialogue else {
            panic!()
        };
        assert_ne!(ed.id, premier);
        e.appliquer(Message::Dialogue(ActionDialogue::Editeur(
            ActionEditeur::Abandonner,
        )));
        assert!(e.dialogue.is_none());
    }

    #[test]
    fn recherche_asynchrone_et_choix_d_un_resultat() {
        let mut e = Etat::avec_source(source_rapide());
        let id = id_de(&e, "Blade Runner 2049", 2017);
        let alien = id_de(&e, "Alien", 1979);
        e.appliquer(Message::Selectionner {
            id,
            ctrl: false,
            maj: false,
        });
        e.appliquer(Message::Selectionner {
            id: alien,
            ctrl: true,
            maj: false,
        });
        e.appliquer(Message::Commande(Commande::ScrapeInteractif));
        let Some(Dialogue::Recherche(r)) = &e.dialogue else {
            panic!("recherche attendue")
        };
        assert_eq!(r.film_id, alien); // ordre d'affichage
        assert!(r.en_cours);
        // une réponse périmée est ignorée
        e.appliquer(Message::ResultatsRecherche {
            id_requete: 999,
            resultat: Ok(vec![]),
        });
        attendre_recherche(&mut e);
        let Some(Dialogue::Recherche(r)) = &e.dialogue else {
            panic!()
        };
        assert_eq!(r.resultats[0].titre, "Alien");
        assert_eq!(r.selection, Some(0));
        e.appliquer(Message::Dialogue(ActionDialogue::Recherche(
            ActionRecherche::Ignorer,
        )));
        attendre_recherche(&mut e);
        let Some(Dialogue::Recherche(r)) = &e.dialogue else {
            panic!()
        };
        assert_eq!(r.film_id, id);
        assert_eq!(r.requete.texte, "Blade Runner 2049");
        let n = r
            .resultats
            .iter()
            .position(|x| x.titre == "Blade Runner")
            .unwrap();
        e.appliquer(Message::Dialogue(ActionDialogue::Recherche(
            ActionRecherche::Choisir(n),
        )));
        e.appliquer(Message::Dialogue(ActionDialogue::Recherche(
            ActionRecherche::Valider,
        )));
        assert!(e.dialogue.is_none());
        attendre(&mut e, |e| e.film(id).unwrap().annee == 1982);
        assert!(e.film(id).unwrap().metadonnees_ok);
    }

    #[test]
    fn assistant_transmet_la_cle_sans_la_persister() {
        let source = source_rapide();
        let mut e = Etat::avec_source(source.clone());
        e.restaurer_interface(&EtatInterface::default());
        let assistant = |a| Message::Dialogue(ActionDialogue::Assistant(a));
        e.appliquer(assistant(ActionAssistant::Suivante));
        e.appliquer(assistant(ActionAssistant::Langue(Langue::Anglais)));
        assert_eq!(e.i18n.t("app.movies"), "Movies");
        e.appliquer(assistant(ActionAssistant::Theme(Theme::Clair)));
        e.appliquer(assistant(ActionAssistant::SaisieSource(
            "E:\\Vidéos".into(),
        )));
        e.appliquer(assistant(ActionAssistant::AjouterSource));
        e.appliquer(assistant(ActionAssistant::CleTmdb("cle-secrete-42".into())));
        e.appliquer(assistant(ActionAssistant::Terminer));
        assert!(e.dialogue.is_none() && e.assistant_termine);
        assert!(e.services.sources_films.contains(&"E:\\Vidéos".to_string()));
        assert!(
            source
                .reglages_services()
                .sources_films
                .contains(&"E:\\Vidéos".to_string())
        );
        assert_eq!(
            source.secret(SECRET_CLE_TMDB).as_deref(),
            Some("cle-secrete-42")
        );
        let json = serde_json::to_string(&e.instantane_interface()).unwrap();
        assert!(!json.contains("cle-secrete-42"));
        assert!(json.contains("\"theme\":\"clair\""));
    }

    /// Draine les messages des threads jusqu'à `condition` (5 s au plus).
    fn attendre_messages(e: &mut Etat, condition: impl Fn(&Etat) -> bool) {
        let limite = Instant::now() + Duration::from_secs(5);
        while !condition(e) {
            assert!(Instant::now() < limite, "délai dépassé");
            std::thread::sleep(Duration::from_millis(10));
            e.drainer();
        }
    }

    #[test]
    fn selecteur_natif_ajoute_les_sources() {
        let mut e = Etat::nouveau();
        e.definir_selecteur(std::sync::Arc::new(|_: &str| {
            Some(PathBuf::from("/media/Films choisis"))
        }));
        // Commande « Ajouter une source » : page des sources + sélecteur.
        e.appliquer(Message::Commande(Commande::AjouterSource));
        attendre_messages(&mut e, |e| {
            e.services
                .sources_films
                .iter()
                .any(|s| s == "/media/Films choisis")
        });
        assert!(matches!(e.dialogue, Some(Dialogue::Reglages(_))));
        // Séries, depuis les réglages ; doublon ignoré.
        e.appliquer(Message::ChoisirDossier(CibleDossier::SourceSeries));
        attendre_messages(&mut e, |e| !e.services.sources_series.is_empty());
        e.appliquer(Message::DossierChoisi(
            CibleDossier::SourceFilms,
            Some(PathBuf::from("/media/Films choisis")),
        ));
        let n = e
            .services
            .sources_films
            .iter()
            .filter(|s| *s == "/media/Films choisis")
            .count();
        assert_eq!(n, 1);
        // Annulation : rien ne change.
        let avant = e.services.clone();
        e.appliquer(Message::DossierChoisi(CibleDossier::SourceFilms, None));
        assert_eq!(e.services, avant);
        // Assistant de premier lancement.
        e.appliquer(Message::OuvrirDialogue(TypeDialogue::Assistant));
        e.appliquer(Message::DossierChoisi(
            CibleDossier::Assistant,
            Some(PathBuf::from("/media/Autres")),
        ));
        let Some(Dialogue::Assistant(a)) = &e.dialogue else {
            panic!("assistant attendu");
        };
        assert!(a.sources.iter().any(|s| s == "/media/Autres"));
    }

    fn publication_test() -> Publication {
        Publication {
            etiquette: "v9.9.9".into(),
            notes: "Nouveautés\nbuild: abcdef1234567".into(),
            build: Some("abcdef1234567".into()),
            url_archive: Some("http://127.0.0.1:9/a.zip".into()),
            ..Default::default()
        }
    }

    #[test]
    fn mise_a_jour_manuelle_erreur_reseau_affichee() {
        let mut e = Etat::nouveau();
        e.config_mise_a_jour.url_api = "http://127.0.0.1:9/releases/latest".into();
        e.appliquer(Message::RechercherMiseAJour { manuel: true });
        assert!(matches!(
            &e.dialogue,
            Some(Dialogue::MiseAJour(d)) if d.phase == PhaseMiseAJour::Recherche
        ));
        attendre_messages(
            &mut e,
            |e| matches!(&e.dialogue, Some(Dialogue::MiseAJour(d)) if d.phase != PhaseMiseAJour::Recherche),
        );
        let Some(Dialogue::MiseAJour(d)) = &e.dialogue else {
            unreachable!()
        };
        assert!(
            matches!(&d.phase, PhaseMiseAJour::Erreur(t) if t.contains("injoignable")),
            "{:?}",
            d.phase
        );
    }

    #[test]
    fn mise_a_jour_disponible_ouvre_le_dialogue() {
        let mut e = Etat::nouveau();
        e.config_mise_a_jour.courant.build = Some("0000000111111".into());
        // Vérification automatique, aucun dialogue ouvert : « Nouvelle version disponible ».
        e.appliquer(Message::ResultatMiseAJour {
            manuel: false,
            resultat: Ok((Verdict::Disponible, publication_test())),
        });
        assert!(matches!(
            &e.dialogue,
            Some(Dialogue::MiseAJour(d)) if d.phase == PhaseMiseAJour::Disponible
        ));
        // Automatique et déjà à jour / en erreur : silencieux.
        e.appliquer(Message::FermerDialogue);
        e.appliquer(Message::ResultatMiseAJour {
            manuel: false,
            resultat: Ok((Verdict::AJour, publication_test())),
        });
        e.appliquer(Message::ResultatMiseAJour {
            manuel: false,
            resultat: Err("panne".into()),
        });
        assert!(e.dialogue.is_none());
        // Automatique avec un autre dialogue ouvert : simple message.
        e.appliquer(Message::OuvrirDialogue(TypeDialogue::Messages));
        let n = e.notifications.len();
        e.appliquer(Message::ResultatMiseAJour {
            manuel: false,
            resultat: Ok((Verdict::Disponible, publication_test())),
        });
        assert!(matches!(e.dialogue, Some(Dialogue::Messages)));
        assert_eq!(e.notifications.len(), n + 1);
        assert!(e.notifications.last().unwrap().texte.contains("v9.9.9"));
    }

    #[test]
    fn version_de_developpement_sans_installation() {
        let mut e = Etat::nouveau();
        e.config_mise_a_jour.courant.build = None;
        // Pas de vérification automatique en compilation locale (aucun thread lancé).
        e.appliquer(Message::RechercherMiseAJour { manuel: false });
        assert!(e.dialogue.is_none());
        e.appliquer(Message::OuvrirDialogue(TypeDialogue::Messages));
        e.dialogue = Some(Dialogue::MiseAJour(EtatDialogueMiseAJour {
            phase: PhaseMiseAJour::Developpement,
            publication: Some(publication_test()),
        }));
        e.appliquer(Message::InstallerMiseAJour);
        assert!(matches!(
            &e.dialogue,
            Some(Dialogue::MiseAJour(d)) if d.phase == PhaseMiseAJour::Developpement
        ));
        // Échec d'installation : erreur affichée proprement, pas de fermeture.
        e.appliquer(Message::MiseAJourInstallee(Err("disque plein".into())));
        assert!(!e.quitter);
        assert!(matches!(
            &e.dialogue,
            Some(Dialogue::MiseAJour(d)) if matches!(&d.phase, PhaseMiseAJour::Erreur(t) if t.contains("disque plein"))
        ));
    }

    // --- Câblage films / séries / analyse (source factice) ---

    fn attendre_etat(e: &mut Etat, fini: impl Fn(&Etat) -> bool) {
        let fin = Instant::now() + std::time::Duration::from_secs(10);
        while !fini(e) && Instant::now() < fin {
            e.drainer();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(fini(e), "délai dépassé");
    }

    #[test]
    fn trouver_les_manquants_ouvre_le_dialogue_puis_le_remplit() {
        let mut e = Etat::nouveau();
        // Sans saga concernée : simple avertissement, aucun dialogue.
        e.appliquer(Message::Commande(Commande::TrouverManquants));
        assert!(e.dialogue.is_none());
        e.appliquer(Message::FermerDialogue);
        e.appliquer(Message::ChoisirModule(Module::Collections));
        e.appliquer(Message::SelectionnerNoeudSaga(NoeudSaga::Saga(
            "demo-saga-dune".into(),
        )));
        e.appliquer(Message::Commande(Commande::TrouverManquants));
        assert!(matches!(&e.dialogue, Some(Dialogue::Manquants(_))));
        attendre_etat(
            &mut e,
            |e| matches!(&e.dialogue, Some(Dialogue::Manquants(m)) if !m.en_cours),
        );
        e.appliquer(Message::FermerDialogue);
        assert!(e.dialogue.is_none());
    }

    #[test]
    fn mise_a_jour_de_la_selection_lance_une_tache() {
        let mut e = Etat::nouveau();
        e.appliquer(Message::Commande(Commande::MettreAJourSelection));
        attendre_etat(&mut e, |e| e.taches.is_empty());
    }

    #[test]
    fn retrait_confirme_avec_la_case_des_fichiers() {
        let mut e = Etat::nouveau();
        let avant = e.films.len();
        e.appliquer(Message::Commande(Commande::RetirerBase));
        assert!(matches!(&e.dialogue, Some(Dialogue::Confirmation(_))));
        e.appliquer(Message::Dialogue(ActionDialogue::ConfirmationCase(true)));
        e.appliquer(Message::Confirmer);
        attendre_etat(&mut e, |e| e.films.len() < avant);
        assert!(e.films.len() < avant);
    }

    #[test]
    fn annuler_le_renommage_sans_historique_ne_plante_pas() {
        let mut e = Etat::nouveau();
        e.appliquer(Message::Commande(Commande::AnnulerRenommage));
        attendre_etat(&mut e, |e| e.taches.is_empty());
    }

    #[test]
    fn module_series_recherche_et_editeur() {
        let mut e = Etat::nouveau();
        e.appliquer(Message::ChoisirModule(Module::Series));
        e.appliquer(Message::SelectionnerNoeudSerie(NoeudSerie::Serie(
            "demo-1".into(),
        )));
        assert!(e.serie_selectionnee().is_some());
        e.appliquer(Message::Commande(Commande::Editer));
        assert!(matches!(&e.dialogue, Some(Dialogue::EditeurSerie(_))));
        e.appliquer(Message::Dialogue(ActionDialogue::EditeurSerie(
            ActionEditeurSerie::Abandonner,
        )));
        assert!(e.dialogue.is_none());
        e.appliquer(Message::Commande(Commande::ScrapeInteractif));
        assert!(matches!(&e.dialogue, Some(Dialogue::Recherche(_))));
        e.appliquer(Message::FermerDialogue);
        e.appliquer(Message::Commande(Commande::ApercuRenommage));
        assert!(matches!(&e.dialogue, Some(Dialogue::ApercuRenommage(_))));
    }

    #[test]
    fn test_des_outils_et_choix_du_fichier() {
        let mut e = Etat::nouveau();
        e.appliquer(Message::OuvrirReglagesPage(
            crate::reglages::Page::OutilsExternes,
        ));
        e.appliquer(Message::FichierChoisi(
            CibleFichier::OutilsAnalyse,
            Some(PathBuf::from("/opt/ffprobe")),
        ));
        e.appliquer(Message::TesterOutils);
        attendre_etat(
            &mut e,
            |e| matches!(&e.dialogue, Some(Dialogue::Reglages(d)) if d.test_outils.is_some()),
        );
    }
}
