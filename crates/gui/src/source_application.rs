//! Source de données adossée aux services réels (`mediamanager_app::Application`).
//!
//! - Identifiants : les films du modèle ont un identifiant texte ; l'interface manipule des
//!   `u64` attribués ici (stables pendant la session).
//! - Événements : [`brancher`](SourceDonnees::brancher) installe un fil relais qui traduit
//!   les événements des services en messages de l'état (tâches, notifications, films et
//!   séries modifiés).
//! - Tâches : chaque tâche lancée est suivie (principale ou non, bilan à afficher à la fin).

use crate::conversion::{self, ContexteConversion};
use crate::etat::{Message, Niveau};
use crate::modele::Film;
use crate::renommage::ApercuFilm;
use crate::source_donnees::{
    GenreImage, ReglagesServices, RequeteRecherche, ResultatRecherche, Retour, SECRET_CLE_TMDB,
    SerieAffichage, SourceDonnees, Travail,
};
use crate::taches::{IssueTache, compteur};
use mediamanager_app::{
    Application as Services, ConfigMetadonnees, Evenement, IdTache, IssueTache as IssueServices,
    NiveauMessage, PoigneeTache, RapportMiseAJour, RequeteFilm,
};
use mediamanager_core::modele::Film as FilmModele;
use mediamanager_scrapers::ResultatRecherche as ResultatScraper;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, RwLock, RwLockReadGuard, Weak};

/// Message affiché quand l'application a déjà été arrêtée.
const ARRETEE: &str = "l'application est arrêtée";

/// Bilan d'une tâche terminée (lu sur la poignée), à présenter à l'utilisateur.
type Bilan = Box<dyn FnOnce() -> Option<(Niveau, String)> + Send>;

struct Suivi {
    principale: bool,
    bilan: Option<Bilan>,
}

/// Correspondance identifiant texte ↔ identifiant affiché.
#[derive(Default)]
struct Correspondance {
    vers_affichage: HashMap<String, u64>,
    vers_modele: HashMap<u64, String>,
    prochain: u64,
}

impl Correspondance {
    fn attribuer(&mut self, id: &str) -> u64 {
        if let Some(n) = self.vers_affichage.get(id) {
            return *n;
        }
        self.prochain += 1;
        let n = self.prochain;
        self.vers_affichage.insert(id.to_string(), n);
        self.vers_modele.insert(n, id.to_string());
        n
    }
}

struct Interne {
    app: RwLock<Option<Services>>,
    ids: Mutex<Correspondance>,
    images: Mutex<HashMap<u64, [Option<PathBuf>; 4]>>,
    nouveaux: Mutex<HashSet<String>>,
    resultats: Mutex<HashMap<String, ResultatScraper>>,
    suivis: Mutex<HashMap<IdTache, Suivi>>,
}

fn verrou<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn niveau(n: NiveauMessage) -> Niveau {
    match n {
        NiveauMessage::Info => Niveau::Info,
        NiveauMessage::Avertissement => Niveau::Avertissement,
        NiveauMessage::Erreur => Niveau::Erreur,
    }
}

fn bilan_mise_a_jour(r: RapportMiseAJour, quoi: &str) -> Option<(Niveau, String)> {
    let mut texte = format!(
        "{quoi} : {} ajout(s), {} modification(s), {} suppression(s)",
        r.ajoutes.len(),
        r.modifies.len(),
        r.supprimes.len()
    );
    for (source, raison) in &r.sources_indisponibles {
        texte.push_str(&format!("\n{} indisponible : {raison}", source.display()));
    }
    let n = if r.sources_indisponibles.is_empty() {
        Niveau::Info
    } else {
        Niveau::Avertissement
    };
    Some((n, texte))
}

impl Interne {
    fn app(&self) -> RwLockReadGuard<'_, Option<Services>> {
        self.app.read().unwrap_or_else(|e| e.into_inner())
    }

    fn id_affichage(&self, id: &str) -> u64 {
        verrou(&self.ids).attribuer(id)
    }

    fn id_modele(&self, id: u64) -> Option<String> {
        verrou(&self.ids).vers_modele.get(&id).cloned()
    }

    fn ids_modele(&self, ids: &[u64]) -> Vec<String> {
        let c = verrou(&self.ids);
        ids.iter()
            .filter_map(|i| c.vers_modele.get(i).cloned())
            .collect()
    }

    fn contexte(&self, app: &Services) -> ContexteConversion {
        ContexteConversion {
            nouveaux: verrou(&self.nouveaux).clone(),
            sagas: app
                .sagas()
                .unwrap_or_default()
                .into_iter()
                .map(|s| (s.id, s.titre))
                .collect(),
            prefixes: app.reglages().general.prefixes_titre,
        }
    }

    /// Conversion d'un film du modèle (images mémorisées au passage).
    fn convertir(&self, f: &FilmModele, ctx: &ContexteConversion) -> Film {
        let id = self.id_affichage(&f.id);
        verrou(&self.images).insert(id, conversion::chemins_images(&f.fichiers));
        conversion::film(f, id, ctx)
    }

    /// Enregistre le suivi d'une tâche (à appeler avec le verrou `suivis` détenu depuis le
    /// lancement, pour que le relais ne voie jamais la fin d'une tâche inconnue).
    fn suivre<T: Send + 'static>(
        suivis: &mut HashMap<IdTache, Suivi>,
        poignee: PoigneeTache<T>,
        principale: bool,
        bilan: impl FnOnce(T) -> Option<(Niveau, String)> + Send + 'static,
    ) {
        let id = poignee.id;
        let bilan: Bilan = Box::new(move || match poignee.essayer_resultat() {
            Some(Ok(v)) => bilan(v),
            _ => None,
        });
        suivis.insert(
            id,
            Suivi {
                principale,
                bilan: Some(bilan),
            },
        );
    }

    /// Traduit un événement des services en messages de l'état.
    fn traduire(&self, evt: Evenement) -> Vec<Message> {
        match evt {
            Evenement::TacheDemarree { tache, nom } => {
                let principale = verrou(&self.suivis)
                    .get(&tache)
                    .is_some_and(|s| s.principale);
                vec![Message::TacheDemarree {
                    id: tache,
                    nom,
                    principale,
                }]
            }
            Evenement::Progression {
                tache,
                fait,
                total,
                description,
            } => vec![Message::TacheProgression {
                id: tache,
                fait: compteur(fait),
                total: compteur(total),
                description,
            }],
            Evenement::Message {
                niveau: n, texte, ..
            } => vec![Message::Notifier(niveau(n), texte)],
            Evenement::FilmAjoute { id } | Evenement::FilmModifie { id } => {
                let garde = self.app();
                let Some(app) = garde.as_ref() else {
                    return Vec::new();
                };
                match app.film(&id) {
                    Ok(Some(f)) => {
                        let ctx = self.contexte(app);
                        vec![Message::FilmActualise(Box::new(self.convertir(&f, &ctx)))]
                    }
                    Ok(None) => vec![Message::FilmRetire(self.id_affichage(&id))],
                    Err(e) => vec![Message::Notifier(Niveau::Erreur, e.to_string())],
                }
            }
            Evenement::FilmSupprime { id } => {
                let n = self.id_affichage(&id);
                verrou(&self.images).remove(&n);
                vec![Message::FilmRetire(n)]
            }
            Evenement::SerieAjoutee { .. }
            | Evenement::SerieModifiee { .. }
            | Evenement::SerieSupprimee { .. } => vec![Message::SeriesModifiees],
            Evenement::TacheTerminee { tache, issue, .. } => {
                let suivi = verrou(&self.suivis).remove(&tache);
                let issue = match issue {
                    IssueServices::Reussie => IssueTache::Reussie,
                    IssueServices::Annulee => IssueTache::Annulee,
                    IssueServices::Echouee(e) => IssueTache::Echouee(e),
                };
                let mut v = vec![Message::TacheTerminee { id: tache, issue }];
                if let Some((n, texte)) = suivi.and_then(|s| s.bilan).and_then(|b| b()) {
                    v.push(Message::Notifier(n, texte));
                }
                v
            }
        }
    }
}

/// Source adossée à une [`mediamanager_app::Application`] démarrée.
pub struct SourceApplication {
    interne: Arc<Interne>,
}

impl SourceApplication {
    pub fn nouvelle(app: Services) -> Self {
        let nouveaux: HashSet<String> = app
            .films_nouveaux()
            .unwrap_or_default()
            .into_iter()
            .collect();
        Self {
            interne: Arc::new(Interne {
                app: RwLock::new(Some(app)),
                ids: Mutex::default(),
                images: Mutex::default(),
                nouveaux: Mutex::new(nouveaux),
                resultats: Mutex::default(),
                suivis: Mutex::default(),
            }),
        }
    }

    /// Dossier de données du stockage (`None` après l'arrêt).
    pub fn dossier_donnees(&self) -> Option<PathBuf> {
        self.interne.app().as_ref().map(|a| a.dossiers().donnees)
    }

    /// Identifiant texte (modèle) d'un film affiché.
    pub fn id_modele(&self, id: u64) -> Option<String> {
        self.interne.id_modele(id)
    }

    /// Arrêt propre des services (tâches annulées, réglages enregistrés, verrou libéré).
    /// Sans effet si déjà arrêtés.
    pub fn arreter(&self) -> Result<(), String> {
        let app = self
            .interne
            .app
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        match app {
            Some(a) => a.arreter().map_err(|e| e.to_string()),
            None => Ok(()),
        }
    }

    fn avec_app<R>(&self, f: impl FnOnce(&Services) -> Result<R, String>) -> Result<R, String> {
        let garde = self.interne.app();
        let app = garde.as_ref().ok_or_else(|| ARRETEE.to_string())?;
        f(app)
    }
}

impl SourceDonnees for SourceApplication {
    fn films(&self) -> Vec<Film> {
        let i = &self.interne;
        let garde = i.app();
        let Some(app) = garde.as_ref() else {
            return Vec::new();
        };
        let films = match app.films() {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("lecture des films impossible : {e}");
                return Vec::new();
            }
        };
        let ctx = i.contexte(app);
        films.iter().map(|f| i.convertir(f, &ctx)).collect()
    }

    fn series(&self) -> Vec<SerieAffichage> {
        self.avec_app(|app| app.series().map_err(|e| e.to_string()))
            .map(|v| v.iter().map(conversion::serie).collect())
            .unwrap_or_default()
    }

    fn chemin_image(&self, film: &Film, genre: GenreImage) -> Option<PathBuf> {
        verrou(&self.interne.images)
            .get(&film.id)
            .and_then(|c| c[genre.rang()].clone())
    }

    fn rechercher(&self, requete: &RequeteRecherche) -> Result<Vec<ResultatRecherche>, String> {
        let i = &self.interne;
        let id = i
            .id_modele(requete.film)
            .ok_or_else(|| format!("film {} inconnu", requete.film))?;
        let poignee = self.avec_app(|app| {
            let mut suivis = verrou(&i.suivis);
            let p = app.rechercher_film(
                &id,
                Some(RequeteFilm {
                    texte: requete.texte.trim().to_string(),
                    annee: requete.annee.and_then(|a| i32::try_from(a).ok()),
                    ids: BTreeMap::new(),
                }),
            );
            suivis.insert(
                p.id,
                Suivi {
                    principale: false,
                    bilan: None,
                },
            );
            Ok(p)
        })?;
        let trouves = poignee.attendre().map_err(|e| e.to_string())?;
        let mut cache = verrou(&i.resultats);
        Ok(trouves
            .into_iter()
            .map(|r| {
                let cle = format!(
                    "{}:{}",
                    r.fournisseur,
                    r.ids
                        .iter()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect::<Vec<_>>()
                        .join(",")
                );
                let affiche = ResultatRecherche {
                    id: cle.clone(),
                    titre: r.titre.clone(),
                    titre_original: r.titre_original.clone(),
                    annee: u32::try_from(r.annee).unwrap_or(0),
                    score: r.score.clamp(0.0, 1.0),
                    resume: r.resume.clone(),
                    affiche: None,
                };
                cache.insert(cle, r);
                affiche
            })
            .collect())
    }

    fn enregistrer_films(&self, films: &[Film]) -> Result<(), String> {
        let i = &self.interne;
        self.avec_app(|app| {
            let mut ids = Vec::new();
            app.avec_stockage(|s| -> Result<(), String> {
                for g in films {
                    let Some(id) = i.id_modele(g.id) else {
                        continue;
                    };
                    let Some(mut f) = s
                        .bibliotheque
                        .obtenir::<FilmModele>(&id)
                        .map_err(|e| e.to_string())?
                    else {
                        continue;
                    };
                    conversion::appliquer_edition(g, &mut f);
                    s.bibliotheque
                        .enregistrer(&mut f)
                        .map_err(|e| e.to_string())?;
                    ids.push(id);
                }
                Ok(())
            })?;
            if !ids.is_empty() {
                // Les NFO suivent la base (tâche principale ; FilmModifie rafraîchira l'affichage).
                let mut suivis = verrou(&i.suivis);
                Interne::suivre(&mut suivis, app.ecrire_nfo_films(ids), true, |_| None);
            }
            Ok(())
        })
    }

    fn retirer_films(&self, ids: &[u64], supprimer_fichiers: bool) -> Result<(), String> {
        if supprimer_fichiers {
            return Err("la suppression des fichiers n'est pas encore disponible".into());
        }
        let i = &self.interne;
        let modele = i.ids_modele(ids);
        self.avec_app(|app| {
            app.avec_stockage(|s| -> Result<(), String> {
                for id in &modele {
                    s.bibliotheque
                        .supprimer::<FilmModele>(id)
                        .map_err(|e| e.to_string())?;
                }
                Ok(())
            })
        })?;
        let mut images = verrou(&i.images);
        for id in ids {
            images.remove(id);
        }
        Ok(())
    }

    fn transmettre_secret(&self, nom: &str, valeur: &str) -> Result<(), String> {
        match nom {
            SECRET_CLE_TMDB => {
                self.avec_app(|app| app.definir_cle_tmdb(valeur).map_err(|e| e.to_string()))
            }
            autre => Err(format!("secret inconnu : {autre}")),
        }
    }

    fn reglages_services(&self) -> ReglagesServices {
        self.avec_app(|app| Ok(conversion::services(&app.reglages())))
            .unwrap_or_default()
    }

    fn enregistrer_reglages_services(&self, r: &ReglagesServices) -> Result<(), String> {
        self.avec_app(|app| {
            app.modifier_reglages(|g| conversion::appliquer_services(r, g))
                .map_err(|e| e.to_string())
        })
    }

    fn apercu_renommage(&self, ids: &[u64]) -> Result<Vec<ApercuFilm>, String> {
        let i = &self.interne;
        let modele = i.ids_modele(ids);
        let plan = self.avec_app(|app| {
            app.apercu_renommage_films(&modele)
                .map_err(|e| e.to_string())
        })?;
        Ok(conversion::plan_apercu(&plan, |id| i.id_affichage(id)))
    }

    fn lancer(&self, travail: Travail, _retour: &Retour) -> Result<(), String> {
        let i = &self.interne;
        self.avec_app(|app| {
            let mut suivis = verrou(&i.suivis);
            let config = |artwork: bool, ne_pas_remplacer: bool| {
                let mut c = ConfigMetadonnees::depuis_reglages(&app.reglages().films);
                c.artwork = artwork;
                c.ne_pas_ecraser |= ne_pas_remplacer;
                c
            };
            match travail {
                Travail::MettreAJourSources => {
                    Interne::suivre(&mut suivis, app.mettre_a_jour_sources_films(), true, |r| {
                        bilan_mise_a_jour(r, "Films")
                    });
                    Interne::suivre(&mut suivis, app.mettre_a_jour_sources_series(), true, |r| {
                        bilan_mise_a_jour(r, "Séries")
                    });
                }
                Travail::MettreAJourSeries => {
                    Interne::suivre(&mut suivis, app.mettre_a_jour_sources_series(), true, |r| {
                        bilan_mise_a_jour(r, "Séries")
                    });
                }
                Travail::RecupererAuto { ids, artwork } => {
                    let p = app.scraper_films(i.ids_modele(&ids), config(artwork, false));
                    Interne::suivre(&mut suivis, p, true, |r| {
                        let mut t = format!(
                            "Récupération : {} réussie(s), {} sans résultat",
                            r.reussis.len(),
                            r.sans_resultat.len()
                        );
                        for (_, e) in &r.erreurs {
                            t.push_str(&format!("\n{e}"));
                        }
                        let n = if r.erreurs.is_empty() && r.sans_resultat.is_empty() {
                            Niveau::Info
                        } else {
                            Niveau::Avertissement
                        };
                        Some((n, t))
                    });
                }
                Travail::AppliquerResultat {
                    film,
                    resultat,
                    ne_pas_remplacer,
                } => {
                    let id = i
                        .id_modele(film)
                        .ok_or_else(|| format!("film {film} inconnu"))?;
                    let choix =
                        verrou(&i.resultats)
                            .get(&resultat.id)
                            .cloned()
                            .ok_or_else(|| {
                                "résultat de recherche périmé : relancez la recherche".to_string()
                            })?;
                    let p = app.appliquer_resultat(&id, choix, config(true, ne_pas_remplacer));
                    Interne::suivre(&mut suivis, p, true, |_| None);
                }
                Travail::EcrireNfo { ids } => {
                    let p = app.ecrire_nfo_films(i.ids_modele(&ids));
                    Interne::suivre(&mut suivis, p, true, |n| {
                        Some((Niveau::Info, format!("NFO écrits : {n}")))
                    });
                }
                Travail::Renommer { ids } => {
                    let p = app.renommer_films(i.ids_modele(&ids));
                    Interne::suivre(&mut suivis, p, true, |r| {
                        let mut t = format!(
                            "Renommage : {} renommé(s), {} inchangé(s)",
                            r.renommes.len(),
                            r.inchanges.len()
                        );
                        for (_, e) in &r.echecs {
                            t.push_str(&format!("\n{e}"));
                        }
                        let n = if r.echecs.is_empty() {
                            Niveau::Info
                        } else {
                            Niveau::Erreur
                        };
                        Some((n, t))
                    });
                }
            }
            Ok(())
        })
    }

    fn annuler_tache(&self, id: u64) {
        if let Some(app) = self.interne.app().as_ref() {
            app.taches().annuler(id);
        }
    }

    fn brancher(&self, retour: Retour) {
        let Some(rx) = self.interne.app().as_ref().map(|a| a.abonner()) else {
            return;
        };
        let faible: Weak<Interne> = Arc::downgrade(&self.interne);
        let lancement = std::thread::Builder::new()
            .name("mm-gui-relais".into())
            .spawn(move || {
                for evt in rx.iter() {
                    if let Evenement::FilmAjoute { id } = &evt
                        && let Some(i) = faible.upgrade()
                    {
                        verrou(&i.nouveaux).insert(id.clone());
                    }
                    let Some(i) = faible.upgrade() else { return };
                    let messages = i.traduire(evt);
                    drop(i);
                    for m in messages {
                        if retour.tx.send(m).is_err() {
                            return;
                        }
                    }
                    (retour.repaint)();
                }
            });
        if let Err(e) = lancement {
            tracing::error!("relais des événements impossible : {e}");
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::etat::Etat;
    use mediamanager_app::{ConfigServices, OptionsDemarrage, OptionsJournal};
    use mediamanager_store::{CoffreMemoire, DossiersApplication, ModeDossiers};
    use std::path::Path;
    use std::time::{Duration, Instant};

    /// Services démarrés sur un dossier temporaire, sans réseau (URL TMDB locale invalide).
    pub(crate) fn demarrer(racine: &Path) -> Services {
        Services::demarrer(OptionsDemarrage {
            dossiers: Some(DossiersApplication::depuis_racine(
                racine.join("app"),
                ModeDossiers::Explicite,
            )),
            coffre: Some(Box::new(CoffreMemoire::default())),
            journal: OptionsJournal {
                actif: false,
                ..OptionsJournal::default()
            },
            services: ConfigServices {
                url_api_tmdb: Some("http://127.0.0.1:9".into()),
                cache_http: false,
                horodatage_nfo: false,
                ..ConfigServices::default()
            },
        })
        .unwrap()
    }

    /// Médiathèque factice : vidéos vides dans un dossier par film.
    pub(crate) fn mediatheque(racine: &Path) -> PathBuf {
        let films = racine.join("films");
        for (dossier, fichier) in [
            ("Alien (1979)", "Alien.1979.mkv"),
            ("Blade Runner (1982)", "Blade.Runner.1982.mkv"),
        ] {
            std::fs::create_dir_all(films.join(dossier)).unwrap();
            std::fs::write(films.join(dossier).join(fichier), b"").unwrap();
        }
        films
    }

    /// Source réelle avec une source de films configurée.
    pub(crate) fn source_reelle(racine: &Path) -> Arc<SourceApplication> {
        let films = mediatheque(racine);
        let app = demarrer(racine);
        app.modifier_reglages(|r| r.films.sources = vec![films])
            .unwrap();
        Arc::new(SourceApplication::nouvelle(app))
    }

    /// Pompe les messages de l'état jusqu'à ce que `fini` soit vrai (10 s au plus).
    pub(crate) fn attendre(e: &mut Etat, fini: impl Fn(&Etat) -> bool) {
        let fin = Instant::now() + Duration::from_secs(10);
        while !fini(e) && Instant::now() < fin {
            e.drainer();
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(fini(e), "délai dépassé");
    }

    #[test]
    fn correspondance_stable() {
        let mut c = Correspondance::default();
        let a = c.attribuer("x");
        let b = c.attribuer("y");
        assert_ne!(a, b);
        assert_eq!(c.attribuer("x"), a);
        assert_eq!(c.vers_modele[&b], "y");
    }

    #[test]
    fn mise_a_jour_puis_renommage_avec_la_source_reelle() {
        let d = tempfile::tempdir().unwrap();
        let source = source_reelle(d.path());
        let mut e = Etat::avec_source(source.clone());
        assert!(e.films.is_empty());
        assert_eq!(e.services.sources_films.len(), 1);

        // Mise à jour des sources (films puis séries) : films ajoutés par les événements.
        e.appliquer(Message::Commande(
            crate::commandes::Commande::MettreAJourToutesSources,
        ));
        attendre(&mut e, |e| e.films.len() == 2 && e.taches.is_empty());
        let alien = e.films.iter().find(|f| f.titre == "Alien").unwrap().clone();
        assert_eq!(alien.annee, 1979);
        assert!(alien.nouveau);
        assert_eq!(alien.nom_fichier, "Alien.1979.mkv");
        assert!(
            e.notifications
                .iter()
                .any(|n| n.texte.starts_with("Films : 2 ajout(s)")),
            "{:?}",
            e.notifications
        );

        // Aperçu réel puis renommage.
        e.appliquer(Message::Selectionner {
            id: alien.id,
            ctrl: false,
            maj: false,
        });
        e.appliquer(Message::Commande(
            crate::commandes::Commande::ApercuRenommage,
        ));
        let Some(crate::etat::Dialogue::ApercuRenommage(a)) = &e.dialogue else {
            panic!("aperçu attendu : {:?}", e.notifications)
        };
        let apercu = &a.apercus[0];
        assert_eq!(apercu.id, alien.id);
        assert_eq!(apercu.ancien_dossier, "Alien (1979)");
        assert!(apercu.executable(), "{apercu:?}");
        let video = apercu
            .fichiers
            .iter()
            .find(|l| l.type_fichier == "VIDEO")
            .unwrap();
        assert_eq!(video.ancien, "Alien.1979.mkv");
        assert!(video.nouveau.starts_with("Alien (1979)"), "{video:?}");
        e.appliquer(Message::ApercuAppliquer);
        assert!(e.dialogue.is_none());
        attendre(&mut e, |e| {
            e.film(alien.id).unwrap().nom_fichier != "Alien.1979.mkv" && e.taches.is_empty()
        });
        let renomme = e.film(alien.id).unwrap();
        assert!(renomme.nom_fichier.starts_with("Alien (1979)"));
        assert!(
            d.path()
                .join("films/Alien (1979)")
                .join(&renomme.nom_fichier)
                .is_file()
        );

        // Écriture des NFO (bilan) puis retrait de la base.
        e.appliquer(Message::Commande(crate::commandes::Commande::ReecrireNfo));
        attendre(&mut e, |e| {
            e.notifications
                .iter()
                .any(|n| n.texte.starts_with("NFO écrits"))
                && e.taches.is_empty()
        });
        e.appliquer(Message::Commande(crate::commandes::Commande::RetirerBase));
        e.appliquer(Message::Confirmer);
        assert!(e.film(alien.id).is_none());
        assert_eq!(source.films().len(), 1);
        source.arreter().unwrap();
        source.arreter().unwrap();
        assert!(source.films().is_empty());
    }

    #[test]
    fn edition_reglages_secret_et_series() {
        let d = tempfile::tempdir().unwrap();
        let source = source_reelle(d.path());
        let mut e = Etat::avec_source(source.clone());

        // Réglages : écrits dans le vrai fichier à la fermeture du dialogue.
        e.appliquer(Message::OuvrirDialogue(crate::etat::TypeDialogue::Reglages));
        let mut r = e.services.clone();
        let series = d.path().join("series");
        std::fs::create_dir_all(series.join("Ma Série (2020)/Saison 1")).unwrap();
        std::fs::write(
            series.join("Ma Série (2020)/Saison 1/Ma.Serie.S01E01.mkv"),
            b"",
        )
        .unwrap();
        std::fs::write(
            series.join("Ma Série (2020)/Saison 1/Ma.Serie.S01E02.mkv"),
            b"",
        )
        .unwrap();
        r.sources_series = vec![series.display().to_string()];
        r.gabarit_dossier_film = "${title}".into();
        e.appliquer(Message::AppliquerServices(Box::new(r)));
        e.appliquer(Message::FermerDialogue);
        let relu = source.reglages_services();
        assert_eq!(relu.gabarit_dossier_film, "${title}");
        assert_eq!(relu.sources_series.len(), 1);

        // Séries : arbre série → saisons → épisodes après mise à jour.
        e.appliquer(Message::MettreAJourSeries);
        attendre(&mut e, |e| !e.series.is_empty() && e.taches.is_empty());
        let s = &e.series[0];
        assert_eq!(s.nombre_episodes(), 2, "{s:?}");
        assert_eq!(s.saisons[0].numero, 1);

        // Clé TMDB : confiée au coffre (lue par les services, absente du fichier).
        source
            .transmettre_secret(SECRET_CLE_TMDB, " cle-42 ")
            .unwrap();
        source
            .avec_app(|app| {
                assert_eq!(app.reglages().general.cle_api_tmdb, "cle-42");
                let texte =
                    std::fs::read_to_string(app.dossiers().fichier_reglages()).unwrap_or_default();
                assert!(!texte.contains("cle-42"));
                Ok(())
            })
            .unwrap();
        assert!(source.transmettre_secret("autre", "x").is_err());

        // Édition d'un film : base + NFO.
        e.appliquer(Message::Commande(
            crate::commandes::Commande::MettreAJourToutesSources,
        ));
        attendre(&mut e, |e| e.films.len() == 2 && e.taches.is_empty());
        let mut f = e.films[0].clone();
        f.resume = "Saisi dans l'éditeur".into();
        f.vu = true;
        source.enregistrer_films(&[f.clone()]).unwrap();
        attendre(&mut e, |e| e.taches.is_empty());
        let id = source.id_modele(f.id).unwrap();
        let relu = source
            .avec_app(|app| Ok(app.film(&id).unwrap().unwrap()))
            .unwrap();
        assert_eq!(relu.resume, "Saisi dans l'éditeur");
        assert!(relu.vu);
        assert!(source.retirer_films(&[f.id], true).is_err());
        source.arreter().unwrap();
    }

    #[test]
    fn recherche_sans_reseau_echoue_proprement() {
        let d = tempfile::tempdir().unwrap();
        let source = source_reelle(d.path());
        let mut e = Etat::avec_source(source.clone());
        e.appliquer(Message::Commande(
            crate::commandes::Commande::MettreAJourToutesSources,
        ));
        attendre(&mut e, |e| e.films.len() == 2 && e.taches.is_empty());
        let id = e.films[0].id;
        // Sans clé TMDB : erreur lisible, pas de panique.
        let r = source.rechercher(&RequeteRecherche {
            film: id,
            extracteur: "TMDB".into(),
            langue: "fr".into(),
            texte: "Alien".into(),
            annee: Some(1979),
        });
        assert!(r.unwrap_err().contains("TMDB"));
        // Résultat inconnu : refusé avant tout appel réseau.
        let travail = Travail::AppliquerResultat {
            film: id,
            resultat: ResultatRecherche {
                id: "inconnu".into(),
                titre: String::new(),
                titre_original: String::new(),
                annee: 0,
                score: 0.0,
                resume: String::new(),
                affiche: None,
            },
            ne_pas_remplacer: false,
        };
        let (tx, _rx) = std::sync::mpsc::channel();
        let retour = Retour {
            tx,
            repaint: Arc::new(|| {}),
        };
        assert!(source.lancer(travail, &retour).is_err());
        // Récupération automatique : la tâche échoue (clé absente) et le signale.
        e.appliquer(Message::Commande(
            crate::commandes::Commande::ScrapeMeilleur,
        ));
        attendre(&mut e, |e| {
            e.taches.is_empty()
                && e.notifications
                    .iter()
                    .any(|n| n.niveau == Niveau::Erreur && n.texte.contains("TMDB"))
        });
        source.arreter().unwrap();
        // Après l'arrêt : erreurs propres.
        assert!(source.lancer(Travail::MettreAJourSources, &retour).is_err());
        assert!(source.apercu_renommage(&[id]).is_err());
    }
}
