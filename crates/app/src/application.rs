//! Point d'entrée des interfaces : [`Application`].

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use mediamanager_core::modele::{Film, Saga, Serie};
use mediamanager_core::nfo::NommageNfoFilm;
use mediamanager_core::renommage::PlanRenommage;
use mediamanager_scrapers::ResultatRecherche;
use mediamanager_store::reglages::noms_secrets;
use mediamanager_store::{
    CoffreSecrets, DossiersApplication, RapportDemarrage, Reglages, Stockage,
};

use crate::artwork::ConfigArtwork;
use crate::conversion::ConfigMetadonnees;
use crate::erreur::{ErreurApp, Resultat};
use crate::evenements::{Diffuseur, Evenement};
use crate::journal::{self, OptionsJournal};
use crate::metadonnees::{self, RapportScrap, RequeteFilm};
use crate::renommage::{self, RapportRenommage};
use crate::sources::{self, OptionsMiseAJour, RapportMiseAJour};
use crate::taches::{GestionnaireTaches, PoigneeTache, TypeTache};

/// Réglages propres à la couche de services (absents du fichier de réglages).
#[derive(Debug, Clone)]
pub struct ConfigServices {
    /// URL de base de l'API TMDB (miroir, tests) ; `None` = service officiel.
    pub url_api_tmdb: Option<String>,
    /// Cache disque des réponses HTTP dans `cache/http`.
    pub cache_http: bool,
    pub mise_a_jour: OptionsMiseAJour,
    pub artwork: ConfigArtwork,
    /// Noms des NFO de films écrits.
    pub nommage_nfo_film: Vec<NommageNfoFilm>,
    /// Horodatage dans l'en-tête des NFO (faux : sorties reproductibles).
    pub horodatage_nfo: bool,
}

impl Default for ConfigServices {
    fn default() -> Self {
        Self {
            url_api_tmdb: None,
            cache_http: true,
            mise_a_jour: OptionsMiseAJour::default(),
            artwork: ConfigArtwork::default(),
            nommage_nfo_film: vec![NommageNfoFilm::NomFichier],
            horodatage_nfo: true,
        }
    }
}

/// Options de [`Application::demarrer`].
#[derive(Default)]
pub struct OptionsDemarrage {
    /// Dossiers imposés ; `None` = résolution standard (portable par défaut).
    pub dossiers: Option<DossiersApplication>,
    /// Coffre à secrets ; `None` = coffre par défaut du système.
    pub coffre: Option<Box<dyn CoffreSecrets>>,
    pub journal: OptionsJournal,
    pub services: ConfigServices,
}

/// État partagé entre l'application et ses tâches.
pub(crate) struct Noyau {
    pub(crate) stockage: Mutex<Stockage>,
    pub(crate) config: ConfigServices,
}

impl Noyau {
    pub(crate) fn verrou(&self) -> MutexGuard<'_, Stockage> {
        self.stockage.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn reglages(&self) -> Reglages {
        self.verrou().reglages.clone()
    }

    /// Clé TMDB lue dans le coffre à secrets (jamais en dur).
    pub(crate) fn cle_tmdb(&self) -> Resultat<String> {
        let s = self.verrou();
        let cle = s
            .gestionnaire_reglages
            .coffre()
            .lire(noms_secrets::CLE_API_TMDB)?
            .filter(|c| !c.trim().is_empty())
            .or_else(|| {
                let c = s.reglages.general.cle_api_tmdb.trim().to_string();
                (!c.is_empty()).then_some(c)
            });
        cle.ok_or(ErreurApp::CleTmdbAbsente)
    }

    pub(crate) fn dossier_cache_http(&self) -> Option<PathBuf> {
        self.config
            .cache_http
            .then(|| self.verrou().dossiers.cache.join("http"))
    }
}

/// Application démarrée : stockage ouvert, journal actif, gestionnaire de
/// tâches prêt. Toutes les méthodes prennent `&self` et sont utilisables
/// depuis un code synchrone ; les traitements longs renvoient une
/// [`PoigneeTache`].
pub struct Application {
    noyau: Arc<Noyau>,
    taches: GestionnaireTaches,
    diffuseur: Arc<Diffuseur>,
    rapport: RapportDemarrage,
    journal: Option<PathBuf>,
    runtime: Option<tokio::runtime::Runtime>,
}

impl std::fmt::Debug for Application {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Application")
            .field("rapport", &self.rapport)
            .finish_non_exhaustive()
    }
}

impl Application {
    /// Démarre : dossiers (portable par défaut), journalisation, stockage
    /// (verrou d'instance, sauvegarde du jour, base, réglages), moteur
    /// asynchrone et gestionnaire de tâches.
    pub fn demarrer(options: OptionsDemarrage) -> Resultat<Self> {
        let dossiers = match options.dossiers {
            Some(d) => d,
            None => DossiersApplication::resoudre()?,
        };
        dossiers.creer()?;
        let journal = journal::initialiser(&dossiers.journaux, &options.journal)?;
        tracing::info!(
            "dossiers : mode {:?}, racine {:?}, données {:?}",
            dossiers.mode,
            dossiers.racine,
            dossiers.donnees
        );
        let (stockage, rapport) = match options.coffre {
            Some(c) => Stockage::demarrer_avec(dossiers, c)?,
            None => Stockage::demarrer(dossiers)?,
        };
        tracing::info!("stockage ouvert : {rapport:?}");
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("mm-async")
            .enable_all()
            .build()
            .map_err(|e| ErreurApp::Interne(format!("moteur asynchrone : {e}")))?;
        let diffuseur = Arc::new(Diffuseur::default());
        let taches = GestionnaireTaches::nouveau(diffuseur.clone(), runtime.handle().clone());
        Ok(Self {
            noyau: Arc::new(Noyau {
                stockage: Mutex::new(stockage),
                config: options.services,
            }),
            taches,
            diffuseur,
            rapport,
            journal,
            runtime: Some(runtime),
        })
    }

    /// Arrêt propre : annulation des tâches (attente 5 s au plus),
    /// enregistrement des réglages modifiés, fermeture de la base, libération
    /// du verrou d'instance.
    pub fn arreter(self) -> Resultat<()> {
        tracing::info!("arrêt de l'application");
        let Application {
            noyau,
            taches,
            runtime,
            ..
        } = self;
        let toutes_finies = taches.arreter(Duration::from_secs(5));
        if let Some(rt) = runtime {
            rt.shutdown_background();
        }
        match Arc::try_unwrap(noyau) {
            Ok(n) => {
                let stockage = n.stockage.into_inner().unwrap_or_else(|e| e.into_inner());
                stockage.fermer()?;
            }
            Err(n) => {
                tracing::warn!(
                    "tâches encore actives à l'arrêt (terminées : {toutes_finies}) : \
                     réglages enregistrés, base fermée à la fin du processus"
                );
                let mut s = n.verrou();
                let reglages = s.reglages.clone();
                s.gestionnaire_reglages.enregistrer_si_modifie(&reglages)?;
            }
        }
        Ok(())
    }

    // -- État ---------------------------------------------------------------

    /// Ce qui s'est passé au démarrage (sauvegarde, restauration…).
    pub fn rapport_demarrage(&self) -> &RapportDemarrage {
        &self.rapport
    }

    /// Chemin du journal, s'il a été installé par ce démarrage.
    pub fn journal(&self) -> Option<&PathBuf> {
        self.journal.as_ref()
    }

    /// Dossiers de l'application.
    pub fn dossiers(&self) -> DossiersApplication {
        self.noyau.verrou().dossiers.clone()
    }

    /// Copie des réglages courants.
    pub fn reglages(&self) -> Reglages {
        self.noyau.reglages()
    }

    /// Modifie les réglages puis les enregistre (secrets dans le coffre).
    pub fn modifier_reglages(&self, f: impl FnOnce(&mut Reglages)) -> Resultat<()> {
        let mut s = self.noyau.verrou();
        f(&mut s.reglages);
        let r = s.reglages.clone();
        s.gestionnaire_reglages.enregistrer_si_modifie(&r)?;
        Ok(())
    }

    /// Enregistre la clé TMDB personnelle de l'utilisateur dans le coffre.
    pub fn definir_cle_tmdb(&self, cle: &str) -> Resultat<()> {
        self.modifier_reglages(|r| r.general.cle_api_tmdb = cle.trim().to_string())
    }

    /// Accès direct au stockage (lectures ponctuelles, outils). Ne pas
    /// conserver le verrou longtemps : les tâches en ont besoin.
    pub fn avec_stockage<R>(&self, f: impl FnOnce(&mut Stockage) -> R) -> R {
        f(&mut self.noyau.verrou())
    }

    /// Tous les films, triés par titre.
    pub fn films(&self) -> Resultat<Vec<Film>> {
        Ok(self.noyau.verrou().bibliotheque.lister::<Film>()?)
    }

    /// Film par identifiant interne.
    pub fn film(&self, id: &str) -> Resultat<Option<Film>> {
        Ok(self.noyau.verrou().bibliotheque.obtenir::<Film>(id)?)
    }

    /// Toutes les séries, triées par titre.
    pub fn series(&self) -> Resultat<Vec<Serie>> {
        Ok(self.noyau.verrou().bibliotheque.lister::<Serie>()?)
    }

    /// Toutes les sagas.
    pub fn sagas(&self) -> Resultat<Vec<Saga>> {
        Ok(self.noyau.verrou().bibliotheque.lister::<Saga>()?)
    }

    /// Films marqués « nouveau » par la dernière mise à jour des sources.
    pub fn films_nouveaux(&self) -> Resultat<Vec<String>> {
        sources::lire_marques(&self.noyau.verrou(), sources::CLE_FILMS_NOUVEAUX)
    }

    /// Films marqués « disparu » (fichiers absents, conservés en base).
    pub fn films_disparus(&self) -> Resultat<Vec<String>> {
        sources::lire_marques(&self.noyau.verrou(), sources::CLE_FILMS_DISPARUS)
    }

    /// Séries marquées « nouvelle » par la dernière mise à jour.
    pub fn series_nouvelles(&self) -> Resultat<Vec<String>> {
        sources::lire_marques(&self.noyau.verrou(), sources::CLE_SERIES_NOUVELLES)
    }

    /// Abonnement à tous les événements (toutes tâches confondues).
    pub fn abonner(&self) -> Receiver<Evenement> {
        self.diffuseur.abonner()
    }

    /// Gestionnaire de tâches (liste, annulation, attente).
    pub fn taches(&self) -> &GestionnaireTaches {
        &self.taches
    }

    fn lancer<T, F>(&self, nom: &str, t: TypeTache, f: F) -> PoigneeTache<T>
    where
        T: Send + 'static,
        F: FnOnce(&Noyau, &crate::taches::ContexteTache) -> Resultat<T> + Send + 'static,
    {
        let noyau = self.noyau.clone();
        self.taches.lancer(nom, t, move |ctx| f(&noyau, ctx))
    }

    // -- Sources --------------------------------------------------------------

    /// Mise à jour de toutes les sources de films (tâche principale).
    pub fn mettre_a_jour_sources_films(&self) -> PoigneeTache<RapportMiseAJour> {
        self.lancer(
            "Mise à jour des sources de films",
            TypeTache::Principale,
            sources::mettre_a_jour_films,
        )
    }

    /// Mise à jour de toutes les sources de séries (tâche principale).
    pub fn mettre_a_jour_sources_series(&self) -> PoigneeTache<RapportMiseAJour> {
        self.lancer(
            "Mise à jour des sources de séries",
            TypeTache::Principale,
            sources::mettre_a_jour_series,
        )
    }

    // -- Métadonnées ----------------------------------------------------------

    /// Recherche des candidats pour un film (tâche de fond, pour un dialogue
    /// de choix) : résultats triés par score décroissant.
    pub fn rechercher_film(
        &self,
        id_film: &str,
        requete: Option<RequeteFilm>,
    ) -> PoigneeTache<Vec<ResultatRecherche>> {
        let id = id_film.to_string();
        self.lancer("Recherche de film", TypeTache::Fond, move |n, ctx| {
            metadonnees::rechercher(n, ctx, &id, requete)
        })
    }

    /// Applique le résultat choisi : métadonnées selon `config`, images,
    /// NFO, base (tâche principale). Renvoie le film mis à jour.
    pub fn appliquer_resultat(
        &self,
        id_film: &str,
        choix: ResultatRecherche,
        config: ConfigMetadonnees,
    ) -> PoigneeTache<Film> {
        let id = id_film.to_string();
        self.lancer(
            "Récupération des métadonnées",
            TypeTache::Principale,
            move |n, ctx| metadonnees::appliquer_choix(n, ctx, &id, &choix, &config),
        )
    }

    /// Récupération automatique (meilleur résultat au-dessus du seuil des
    /// réglages) pour une sélection de films (tâche principale).
    pub fn scraper_films(
        &self,
        ids: Vec<String>,
        config: ConfigMetadonnees,
    ) -> PoigneeTache<RapportScrap> {
        self.lancer(
            "Récupération automatique des métadonnées",
            TypeTache::Principale,
            move |n, ctx| metadonnees::scraper_automatiquement(n, ctx, &ids, &config),
        )
    }

    /// Réécrit les NFO d'une sélection de films (tâche principale) ; renvoie
    /// le nombre de fichiers réellement modifiés.
    pub fn ecrire_nfo_films(&self, ids: Vec<String>) -> PoigneeTache<usize> {
        self.lancer("Écriture des NFO", TypeTache::Principale, move |n, ctx| {
            crate::ecriture_nfo::reecrire_films(n, ctx, &ids)
        })
    }

    // -- Renommage ------------------------------------------------------------

    /// Aperçu du renommage d'une sélection (rien n'est écrit). Les films
    /// verrouillés sont exclus.
    pub fn apercu_renommage_films(&self, ids: &[String]) -> Resultat<PlanRenommage> {
        renommage::apercu_films(&self.noyau, ids)
    }

    /// Renomme une sélection (tâche principale) : plan, exécution, mise à
    /// jour des chemins en base et réécriture des NFO.
    pub fn renommer_films(&self, ids: Vec<String>) -> PoigneeTache<RapportRenommage> {
        self.lancer(
            "Renommage des films",
            TypeTache::Principale,
            move |n, ctx| renommage::renommer_films(n, ctx, &ids),
        )
    }
}
