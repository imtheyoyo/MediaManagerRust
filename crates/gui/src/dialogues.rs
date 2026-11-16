//! États (sans egui) des dialogues : éditeur de film, recherche d'extracteur, confirmation,
//! assistant de premier lancement. La validation et les transitions sont testables.

use crate::i18n::Langue;
use crate::modele::Film;
use crate::reglages::Theme;
use crate::source_donnees::{
    EDITIONS, ElementManquant, FichierInutileAffiche, GroupeManquants, ModeleExportAffiche,
    ModifsLot, RapportExportAffiche, RequeteRecherche, ResultatRecherche, SOURCES_MEDIA,
};
use std::fmt;

// ------------------------------------------------------------------ éditeur de film

pub use crate::editeur_film::{
    ActionEditeur, ChampEditeur, ErreurChamp, EtatEditeur, FilmEdite, OngletEditeur,
    OptionsEditeur, SelectionsEditeur, valider,
};

pub(crate) fn liste(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

// ------------------------------------------------------------------ recherche d'extracteur

#[derive(Clone, Debug)]
pub struct EtatRecherche {
    pub film_id: u64,
    /// Série cherchée (recherche de série) ; `film_id` est alors 0.
    pub serie: Option<String>,
    pub requete: RequeteRecherche,
    pub resultats: Vec<ResultatRecherche>,
    pub selection: Option<usize>,
    pub en_cours: bool,
    /// Identifiant de la requête attendue : les réponses périmées sont ignorées.
    pub id_requete: u64,
    pub erreur: Option<String>,
    pub ne_pas_remplacer: bool,
    pub file: Vec<u64>,
    pub total: usize,
}

impl EtatRecherche {
    pub fn resultat_choisi(&self) -> Option<&ResultatRecherche> {
        self.selection.and_then(|n| self.resultats.get(n))
    }

    pub fn position(&self) -> usize {
        self.total - self.file.len()
    }
}

/// Applique un résultat de recherche au film (« ne pas remplacer » : seuls les champs vides).
pub fn appliquer_resultat(f: &mut Film, r: &ResultatRecherche, ne_pas_remplacer: bool) {
    let remplacer = |champ: &mut String, valeur: &str| {
        if !ne_pas_remplacer || champ.trim().is_empty() {
            *champ = valeur.to_string();
        }
    };
    remplacer(&mut f.titre, &r.titre);
    remplacer(&mut f.titre_original, &r.titre_original);
    remplacer(&mut f.resume, &r.resume);
    remplacer(&mut f.tmdb, &r.id);
    if !ne_pas_remplacer || f.annee == 0 {
        f.annee = r.annee;
    }
    if !ne_pas_remplacer || f.titre_triable.trim().is_empty() {
        f.titre_triable = f.titre.clone();
    }
    f.metadonnees_ok = true;
}

#[derive(Clone, Debug)]
pub enum ActionRecherche {
    Texte(String),
    Annee(Option<u32>),
    Extracteur(String),
    Langue(String),
    Lancer,
    Choisir(usize),
    NePasRemplacer(bool),
    Valider,
    /// « Annuler » : ignore ce film et passe au suivant de la file.
    Ignorer,
    Abandonner,
}

// ------------------------------------------------------------------ éditeur de série

pub use crate::editeur_serie::{
    ActionEditeurSerie, ChampSerie, ErreurChampSerie, EtatEditeurSerie, OngletSerie, SerieEdite,
    valider_serie,
};

// ------------------------------------------------------------------ film hors ligne

/// Dialogue « Créer un film hors ligne » : titre, année et source de données.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EtatHorsLigne {
    pub titre: String,
    /// Année (vide : inconnue).
    pub annee: String,
    /// Sources de films proposées.
    pub sources: Vec<String>,
    /// Rang de la source choisie.
    pub source: usize,
    /// Clé de traduction de l'erreur affichée.
    pub erreur: Option<&'static str>,
}

impl EtatHorsLigne {
    /// Titre rogné et année lue ; `Err` donne la clé de traduction de l'erreur.
    pub fn valider(&self) -> Result<(String, u32), &'static str> {
        let titre = self.titre.trim().to_string();
        if titre.is_empty() {
            return Err("editor.error.title");
        }
        let annee = self.annee.trim();
        let annee = if annee.is_empty() {
            0
        } else {
            match annee.parse::<u32>() {
                Ok(a) if (1870..=2100).contains(&a) => a,
                _ => return Err("editor.error.year"),
            }
        };
        if self.sources.get(self.source).is_none() {
            return Err("gui.offline.nosource");
        }
        Ok((titre, annee))
    }
}

#[derive(Clone, Debug)]
pub enum ActionHorsLigne {
    Titre(String),
    Annee(String),
    Source(usize),
    Valider,
    Abandonner,
}

// ------------------------------------------------------------------ éléments manquants

/// Dialogue « Trouver les manquants » : parties absentes d'une ou plusieurs sagas, ou
/// épisodes absents d'une série.
#[derive(Clone, Debug, Default)]
pub struct EtatManquants {
    /// Clé de traduction du titre du dialogue.
    pub cle_titre: &'static str,
    pub groupes: Vec<GroupeManquants>,
    /// (groupe, élément) choisi pour la fiche.
    pub selection: Option<(usize, usize)>,
    pub en_cours: bool,
    pub erreur: Option<String>,
    /// Identifiant de la requête attendue : les réponses périmées sont ignorées.
    pub id_requete: u64,
}

impl EtatManquants {
    pub fn element_choisi(&self) -> Option<&ElementManquant> {
        let (g, e) = self.selection?;
        self.groupes.get(g)?.elements.get(e)
    }

    /// Nombre total d'éléments manquants.
    pub fn total(&self) -> usize {
        self.groupes.iter().map(|g| g.elements.len()).sum()
    }
}

// ------------------------------------------------------------------ export

/// Avancement du dialogue d'export.
#[derive(Clone, Debug, PartialEq)]
pub enum PhaseExport {
    /// Choix du modèle et des options.
    Saisie,
    /// Tâche lancée : la progression est lue dans les tâches de l'état.
    EnCours,
    Termine(RapportExportAffiche),
    Echec(String),
}

/// Dialogue « Exporter » (films ou séries selon le module).
#[derive(Clone, Debug)]
pub struct EtatExport {
    /// Export des séries (sinon des films).
    pub series: bool,
    pub modeles: Vec<ModeleExportAffiche>,
    /// Rang du modèle choisi dans `modeles`.
    pub modele: usize,
    /// Dossier de sortie saisi.
    pub dossier: String,
    /// Copie des affiches (vignettes) dans `images/`.
    pub vignettes: bool,
    /// Largeur des vignettes en pixels (0 = taille d'origine).
    pub largeur: u32,
    pub titre: String,
    /// Exporte la sélection (sinon toute la bibliothèque).
    pub selection_seule: bool,
    pub nb_selection: usize,
    pub nb_total: usize,
    /// Message de validation (dossier manquant…), clé de traduction.
    pub erreur: Option<&'static str>,
    pub phase: PhaseExport,
}

/// Largeur par défaut des vignettes d'export.
pub const LARGEUR_VIGNETTES: u32 = 200;

impl EtatExport {
    pub fn nouveau(
        series: bool,
        modeles: Vec<ModeleExportAffiche>,
        nb_selection: usize,
        nb_total: usize,
        dossier: String,
    ) -> Self {
        Self {
            series,
            modeles,
            modele: 0,
            dossier,
            vignettes: false,
            largeur: LARGEUR_VIGNETTES,
            titre: String::new(),
            selection_seule: nb_selection > 0,
            nb_selection,
            nb_total,
            erreur: None,
            phase: PhaseExport::Saisie,
        }
    }

    pub fn modele_choisi(&self) -> Option<&ModeleExportAffiche> {
        self.modeles.get(self.modele)
    }

    /// Nombre d'éléments qui seront exportés.
    pub fn nb_exportes(&self) -> usize {
        if self.selection_seule {
            self.nb_selection
        } else {
            self.nb_total
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ActionExport {
    Modele(usize),
    Dossier(String),
    Vignettes(bool),
    Largeur(u32),
    Titre(String),
    SelectionSeule(bool),
    Lancer,
    /// Revient à la saisie après un échec.
    Recommencer,
}

// ------------------------------------------------------------------ nettoyage

/// Avancement du dialogue de nettoyage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PhaseNettoyage {
    /// Analyse des dossiers en cours.
    Analyse,
    Liste,
    /// Dernière confirmation avant le déplacement dans la corbeille.
    Confirmation,
    Echec(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LigneNettoyage {
    pub fichier: FichierInutileAffiche,
    pub coche: bool,
}

/// Dialogue « Nettoyer » : fichiers inutiles des films choisis.
#[derive(Clone, Debug)]
pub struct EtatNettoyage {
    /// Identifiant de la requête d'analyse attendue (les réponses périmées sont ignorées).
    pub id_requete: u64,
    pub nb_films: usize,
    pub phase: PhaseNettoyage,
    pub lignes: Vec<LigneNettoyage>,
}

impl EtatNettoyage {
    /// Éléments cochés.
    pub fn nb_coches(&self) -> usize {
        self.lignes.iter().filter(|l| l.coche).count()
    }

    /// Octets libérés par les éléments cochés.
    pub fn octets_coches(&self) -> u64 {
        self.lignes
            .iter()
            .filter(|l| l.coche)
            .map(|l| l.fichier.taille)
            .sum()
    }

    /// Éléments cochés, prêts à être transmis au nettoyage.
    pub fn liste_cochee(&self) -> Vec<FichierInutileAffiche> {
        self.lignes
            .iter()
            .filter(|l| l.coche)
            .map(|l| l.fichier.clone())
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionNettoyage {
    Cocher(usize, bool),
    ToutCocher(bool),
    /// « Déplacer vers la corbeille » : demande la confirmation.
    Executer,
    ConfirmerExecution,
    AnnulerConfirmation,
}

// ------------------------------------------------------------------ édition globale

/// Saisies de l'édition globale ; un champ vide ou « inchangé » laisse la valeur intacte.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BrouillonLot {
    /// Listes séparées par des virgules.
    pub genres_ajouter: String,
    pub genres_retirer: String,
    pub tags_ajouter: String,
    pub tags_retirer: String,
    pub certification: String,
    pub studio: String,
    pub pays: String,
    pub langue: String,
    pub note: String,
    /// Libellé d'une entrée de [`EDITIONS`].
    pub edition: Option<String>,
    /// Code d'une entrée de [`SOURCES_MEDIA`].
    pub source_media: Option<String>,
    pub vu: Option<bool>,
}

/// Valeurs d'une liste saisie (séparées par des virgules ou des points-virgules).
fn valeurs(texte: &str) -> Vec<String> {
    texte
        .split([',', ';'])
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

fn texte_facultatif(texte: &str) -> Option<String> {
    let t = texte.trim();
    (!t.is_empty()).then(|| t.to_string())
}

impl BrouillonLot {
    /// Modifications correspondantes ; pour des séries, les champs propres aux films sont
    /// ignorés.
    pub fn modifications(&self, series: bool) -> ModifsLot {
        ModifsLot {
            genres_ajouter: valeurs(&self.genres_ajouter),
            genres_retirer: valeurs(&self.genres_retirer),
            tags_ajouter: valeurs(&self.tags_ajouter),
            tags_retirer: valeurs(&self.tags_retirer),
            certification: texte_facultatif(&self.certification),
            studio: texte_facultatif(&self.studio),
            pays: texte_facultatif(&self.pays),
            note: texte_facultatif(&self.note),
            langue: if series {
                None
            } else {
                texte_facultatif(&self.langue)
            },
            edition: if series {
                None
            } else {
                self.edition
                    .clone()
                    .filter(|e| EDITIONS.contains(&e.as_str()))
            },
            source_media: if series {
                None
            } else {
                self.source_media
                    .clone()
                    .filter(|c| SOURCES_MEDIA.iter().any(|(code, _)| code == c))
            },
            vu: if series { None } else { self.vu },
        }
    }
}

/// Dialogue « Édition globale » : mêmes modifications pour plusieurs films ou une série.
#[derive(Clone, Debug)]
pub struct EtatEditionLot {
    pub series: bool,
    pub ids_films: Vec<u64>,
    pub ids_series: Vec<String>,
    /// Éléments verrouillés, ignorés par l'édition.
    pub nb_verrouilles: usize,
    pub brouillon: BrouillonLot,
}

impl EtatEditionLot {
    /// Éléments choisis.
    pub fn nb_elements(&self) -> usize {
        if self.series {
            self.ids_series.len()
        } else {
            self.ids_films.len()
        }
    }

    /// Éléments qui seront réellement modifiés.
    pub fn nb_modifies(&self) -> usize {
        self.nb_elements().saturating_sub(self.nb_verrouilles)
    }

    pub fn modifications(&self) -> ModifsLot {
        self.brouillon.modifications(self.series)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionEditionLot {
    Brouillon(Box<BrouillonLot>),
    Appliquer,
}

// ------------------------------------------------------------------ confirmation

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionConfirmee {
    RetirerBase,
    SupprimerFichiers,
    Quitter,
}

impl ActionConfirmee {
    /// Clé de mémorisation « Ne pas afficher ce message à l'avenir ».
    pub fn cle_masquage(self) -> &'static str {
        match self {
            ActionConfirmee::RetirerBase => "confirm.remove",
            ActionConfirmee::SupprimerFichiers => "confirm.delete",
            ActionConfirmee::Quitter => "confirm.exit",
        }
    }
}

#[derive(Clone, Debug)]
pub struct EtatConfirmation {
    pub titre: String,
    pub texte: String,
    pub action: ActionConfirmee,
    /// La case « ne plus demander » est-elle proposée ?
    pub masquable: bool,
    pub ne_plus_demander: bool,
    /// Case à cocher propre à l'action (« déplacer les fichiers vers la corbeille »).
    pub case: Option<CaseConfirmation>,
}

/// Case à cocher d'une confirmation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaseConfirmation {
    /// Clé de traduction du libellé.
    pub cle: &'static str,
    pub cochee: bool,
}

// ------------------------------------------------------------------ assistant

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageAssistant {
    Accueil,
    Interface,
    Sources,
    Extracteur,
    Fin,
}

impl PageAssistant {
    pub const TOUTES: [PageAssistant; 5] = [
        PageAssistant::Accueil,
        PageAssistant::Interface,
        PageAssistant::Sources,
        PageAssistant::Extracteur,
        PageAssistant::Fin,
    ];

    pub fn rang(self) -> usize {
        Self::TOUTES.iter().position(|p| *p == self).unwrap_or(0)
    }

    pub fn suivante(self) -> Self {
        Self::TOUTES[(self.rang() + 1).min(Self::TOUTES.len() - 1)]
    }

    pub fn precedente(self) -> Self {
        Self::TOUTES[self.rang().saturating_sub(1)]
    }

    pub fn cle(self) -> &'static str {
        match self {
            PageAssistant::Accueil => "wizard.page.welcome",
            PageAssistant::Interface => "wizard.page.ui",
            PageAssistant::Sources => "wizard.page.sources",
            PageAssistant::Extracteur => "wizard.page.scraper",
            PageAssistant::Fin => "wizard.page.finish",
        }
    }
}

#[derive(Clone)]
pub struct EtatAssistant {
    pub page: PageAssistant,
    pub langue: Langue,
    pub theme: Theme,
    pub sources: Vec<String>,
    pub saisie_source: String,
    /// Clé d'API saisie dans un champ masqué ; transmise au coffre à secrets à la fin.
    pub cle_tmdb: String,
    pub langue_extracteur: String,
    pub avertissement: Option<&'static str>,
}

impl fmt::Debug for EtatAssistant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EtatAssistant")
            .field("page", &self.page)
            .field("langue", &self.langue)
            .field("theme", &self.theme)
            .field("sources", &self.sources)
            .field(
                "cle_tmdb",
                &if self.cle_tmdb.is_empty() { "" } else { "***" },
            )
            .finish_non_exhaustive()
    }
}

/// Dossier « dangereux » comme source : racine d'un disque ou du système.
pub fn dossier_dangereux(chemin: &str) -> bool {
    let c = chemin.trim().trim_end_matches(['\\', '/']);
    if c.is_empty() {
        return true;
    }
    let lettre = c.len() == 2
        && c.ends_with(':')
        && c.chars().next().is_some_and(|x| x.is_ascii_alphabetic());
    lettre
        || matches!(
            c.to_lowercase().as_str(),
            "/home" | "/usr" | "/etc" | "/var" | "/mnt" | "/media" | "c:\\windows" | "c:\\users"
        )
}

impl EtatAssistant {
    pub fn new(langue: Langue, theme: Theme, sources: Vec<String>) -> Self {
        Self {
            page: PageAssistant::Accueil,
            langue,
            theme,
            sources,
            saisie_source: String::new(),
            cle_tmdb: String::new(),
            langue_extracteur: langue.code().to_string(),
            avertissement: None,
        }
    }

    /// Ajoute la saisie courante ; refuse les doublons et les dossiers dangereux.
    pub fn ajouter_source(&mut self) {
        let s = self.saisie_source.trim().to_string();
        self.avertissement = None;
        if s.is_empty() {
            return;
        }
        if dossier_dangereux(&s) {
            self.avertissement = Some("wizard.source.dangerous");
            return;
        }
        if self.sources.contains(&s) {
            self.avertissement = Some("wizard.source.duplicate");
            return;
        }
        self.sources.push(s);
        self.saisie_source.clear();
    }
}

#[derive(Clone, Debug)]
pub enum ActionAssistant {
    Suivante,
    Precedente,
    Langue(Langue),
    Theme(Theme),
    SaisieSource(String),
    AjouterSource,
    RetirerSource(usize),
    CleTmdb(String),
    LangueExtracteur(String),
    Terminer,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::donnees::films_factices;

    #[test]
    fn application_d_un_resultat() {
        let mut f = films_factices().remove(0);
        f.resume.clear();
        let r = ResultatRecherche {
            id: "42".into(),
            titre: "Nouveau".into(),
            titre_original: "New".into(),
            annee: 1980,
            score: 0.9,
            resume: "Résumé".into(),
            affiche: None,
        };
        let mut g = f.clone();
        appliquer_resultat(&mut g, &r, true);
        assert_eq!(
            (g.titre.as_str(), g.annee, g.resume.as_str()),
            ("Alien", 1979, "Résumé")
        );
        appliquer_resultat(&mut f, &r, false);
        assert_eq!(
            (f.titre.as_str(), f.annee, f.tmdb.as_str()),
            ("Nouveau", 1980, "42")
        );
        assert!(f.metadonnees_ok);
    }

    #[test]
    fn assistant_pages_et_sources() {
        let mut a = EtatAssistant::new(Langue::Francais, Theme::Sombre, vec![]);
        assert_eq!(a.page.precedente(), PageAssistant::Accueil);
        assert_eq!(PageAssistant::Fin.suivante(), PageAssistant::Fin);
        for dangereux in ["C:\\", "d:", "/", "/home/"] {
            a.saisie_source = dangereux.into();
            a.ajouter_source();
            assert_eq!(
                a.avertissement,
                Some("wizard.source.dangerous"),
                "{dangereux}"
            );
        }
        a.saisie_source = "D:\\Films".into();
        a.ajouter_source();
        a.saisie_source = "D:\\Films".into();
        a.ajouter_source();
        assert_eq!(a.sources, ["D:\\Films"]);
        assert_eq!(a.avertissement, Some("wizard.source.duplicate"));
        a.cle_tmdb = "secret123".into();
        assert!(!format!("{a:?}").contains("secret123"));
    }
}
