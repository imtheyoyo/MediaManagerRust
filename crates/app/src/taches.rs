//! Gestionnaire de tâches d'arrière-plan.
//!
//! - Chaque tâche s'exécute sur son propre fil d'exécution (nommé
//!   `mm-tache-<id>`) ; le code des services est synchrone et utilise
//!   [`ContexteTache::bloquer`] pour attendre les appels réseau asynchrones
//!   sur le moteur tokio interne de l'application.
//! - Les tâches **principales** (scan, scrap, renommage) sont exclusives et
//!   servies dans l'ordre de soumission (file FIFO à tickets) ; les tâches
//!   **de fond** (recherche pour un dialogue…) s'exécutent immédiatement.
//! - Annulation coopérative : [`ContexteTache::verifier_annulation`] entre
//!   deux unités de travail.
//! - Une panique dans une tâche est capturée : la tâche échoue, l'application
//!   continue.
//!
//! Utilisation depuis un code synchrone : [`GestionnaireTaches::lancer`]
//! renvoie une [`PoigneeTache`] (identifiant, récepteur d'événements propre à
//! la tâche, résultat typé).

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::erreur::{ErreurApp, Resultat};
use crate::evenements::{Diffuseur, Evenement, IdTache, IssueTache, NiveauMessage};

/// Nature d'une tâche.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeTache {
    /// Exclusive : une seule à la fois, ordre de soumission respecté.
    Principale,
    /// Exécutée tout de suite, en parallèle des autres.
    Fond,
}

/// État d'une tâche connue du gestionnaire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatTache {
    /// En attente de son tour dans la file principale.
    EnAttente,
    EnCours,
}

/// Instantané d'une tâche active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoTache {
    pub id: IdTache,
    pub nom: String,
    pub type_tache: TypeTache,
    pub etat: EtatTache,
    pub fait: u64,
    pub total: u64,
    pub description: String,
}

/// Jeton d'annulation partagé entre la poignée et la tâche.
#[derive(Debug, Clone, Default)]
pub struct JetonAnnulation(Arc<AtomicBool>);

impl JetonAnnulation {
    pub fn annuler(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn est_annule(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Contexte passé au code d'une tâche : progression, messages, annulation.
pub struct ContexteTache {
    id: IdTache,
    annulation: JetonAnnulation,
    diffuseur: Arc<Diffuseur>,
    propre: Sender<Evenement>,
    registre: Arc<Mutex<BTreeMap<IdTache, InfoTache>>>,
    runtime: tokio::runtime::Handle,
}

impl ContexteTache {
    /// Identifiant de la tâche.
    pub fn id(&self) -> IdTache {
        self.id
    }

    /// Publie un événement (abonnés globaux et poignée de la tâche).
    pub fn emettre(&self, e: Evenement) {
        self.diffuseur.publier(&e);
        let _ = self.propre.send(e);
    }

    /// Avancement de la tâche.
    pub fn progression(&self, fait: u64, total: u64, description: impl Into<String>) {
        let description = description.into();
        if let Some(i) = self
            .registre
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(&self.id)
        {
            i.fait = fait;
            i.total = total;
            i.description = description.clone();
        }
        self.emettre(Evenement::Progression {
            tache: self.id,
            fait,
            total,
            description,
        });
    }

    /// Message pour l'utilisateur (également journalisé).
    pub fn message(&self, niveau: NiveauMessage, texte: impl Into<String>) {
        let texte = texte.into();
        match niveau {
            NiveauMessage::Info => tracing::info!("{texte}"),
            NiveauMessage::Avertissement => tracing::warn!("{texte}"),
            NiveauMessage::Erreur => tracing::error!("{texte}"),
        }
        self.emettre(Evenement::Message {
            tache: Some(self.id),
            niveau,
            texte,
        });
    }

    /// Vrai si l'annulation a été demandée.
    pub fn est_annulee(&self) -> bool {
        self.annulation.est_annule()
    }

    /// `Err(Annulee)` si l'annulation a été demandée.
    pub fn verifier_annulation(&self) -> Resultat<()> {
        if self.est_annulee() {
            Err(ErreurApp::Annulee)
        } else {
            Ok(())
        }
    }

    /// Attend un futur sur le moteur asynchrone interne.
    pub fn bloquer<F: std::future::Future>(&self, f: F) -> F::Output {
        self.runtime.block_on(f)
    }
}

/// Poignée d'une tâche lancée.
pub struct PoigneeTache<T> {
    /// Identifiant de la tâche.
    pub id: IdTache,
    /// Événements de cette tâche uniquement (progression, messages,
    /// entités modifiées, fin).
    pub evenements: Receiver<Evenement>,
    annulation: JetonAnnulation,
    resultat: Receiver<Resultat<T>>,
}

impl<T> PoigneeTache<T> {
    /// Demande l'annulation (coopérative).
    pub fn annuler(&self) {
        self.annulation.annuler();
    }

    /// Jeton d'annulation (à conserver si la poignée est consommée).
    pub fn jeton(&self) -> JetonAnnulation {
        self.annulation.clone()
    }

    /// Attend la fin de la tâche et renvoie son résultat.
    pub fn attendre(self) -> Resultat<T> {
        self.resultat
            .recv()
            .unwrap_or_else(|_| Err(ErreurApp::Interne("tâche interrompue".into())))
    }

    /// Résultat s'il est disponible, sans bloquer (boucle d'interface).
    pub fn essayer_resultat(&self) -> Option<Resultat<T>> {
        match self.resultat.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(Err(ErreurApp::Interne("tâche interrompue".into())))
            }
        }
    }
}

/// File FIFO à tickets des tâches principales.
#[derive(Default)]
struct FilePrincipale {
    etat: Mutex<(u64, u64)>, // (prochain ticket, ticket servi)
    signal: Condvar,
}

impl FilePrincipale {
    fn prendre_ticket(&self) -> u64 {
        let mut e = self.etat.lock().unwrap_or_else(|e| e.into_inner());
        let t = e.0;
        e.0 += 1;
        t
    }
    fn attendre_tour(&self, ticket: u64) {
        let mut e = self.etat.lock().unwrap_or_else(|e| e.into_inner());
        while e.1 != ticket {
            e = self.signal.wait(e).unwrap_or_else(|e| e.into_inner());
        }
    }
    fn liberer(&self) {
        let mut e = self.etat.lock().unwrap_or_else(|e| e.into_inner());
        e.1 += 1;
        self.signal.notify_all();
    }
}

struct Interne {
    compteur: AtomicU64,
    arret: AtomicBool,
    diffuseur: Arc<Diffuseur>,
    registre: Arc<Mutex<BTreeMap<IdTache, InfoTache>>>,
    jetons: Mutex<BTreeMap<IdTache, JetonAnnulation>>,
    fils: Mutex<Vec<JoinHandle<()>>>,
    file: FilePrincipale,
    runtime: tokio::runtime::Handle,
}

/// Gestionnaire des tâches d'arrière-plan (clonable, partagé).
#[derive(Clone)]
pub struct GestionnaireTaches {
    interne: Arc<Interne>,
}

impl std::fmt::Debug for GestionnaireTaches {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GestionnaireTaches")
            .field("taches", &self.taches_actives().len())
            .finish()
    }
}

impl GestionnaireTaches {
    /// Gestionnaire publiant sur `diffuseur`, attendant les futurs sur `runtime`.
    pub fn nouveau(diffuseur: Arc<Diffuseur>, runtime: tokio::runtime::Handle) -> Self {
        Self {
            interne: Arc::new(Interne {
                compteur: AtomicU64::new(1),
                arret: AtomicBool::new(false),
                diffuseur,
                registre: Arc::default(),
                jetons: Mutex::default(),
                fils: Mutex::default(),
                file: FilePrincipale::default(),
                runtime,
            }),
        }
    }

    /// Lance une tâche. Le résultat typé est disponible par la poignée ; les
    /// événements sont publiés à la fois sur la poignée et aux abonnés
    /// globaux. Après [`arreter`](Self::arreter), la tâche échoue aussitôt
    /// avec [`ErreurApp::ArretEnCours`].
    pub fn lancer<T, F>(&self, nom: &str, type_tache: TypeTache, travail: F) -> PoigneeTache<T>
    where
        T: Send + 'static,
        F: FnOnce(&ContexteTache) -> Resultat<T> + Send + 'static,
    {
        let i = &self.interne;
        let id = i.compteur.fetch_add(1, Ordering::SeqCst);
        let (tx_evt, rx_evt) = channel();
        let (tx_res, rx_res) = channel();
        let annulation = JetonAnnulation::default();
        let poignee = PoigneeTache {
            id,
            evenements: rx_evt,
            annulation: annulation.clone(),
            resultat: rx_res,
        };
        if i.arret.load(Ordering::SeqCst) {
            let _ = tx_res.send(Err(ErreurApp::ArretEnCours));
            return poignee;
        }
        let ticket = (type_tache == TypeTache::Principale).then(|| i.file.prendre_ticket());
        i.registre.lock().unwrap_or_else(|e| e.into_inner()).insert(
            id,
            InfoTache {
                id,
                nom: nom.to_string(),
                type_tache,
                etat: EtatTache::EnAttente,
                fait: 0,
                total: 0,
                description: String::new(),
            },
        );
        i.jetons
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, annulation.clone());

        let ctx = ContexteTache {
            id,
            annulation,
            diffuseur: i.diffuseur.clone(),
            propre: tx_evt,
            registre: i.registre.clone(),
            runtime: i.runtime.clone(),
        };
        let interne = self.interne.clone();
        let nom = nom.to_string();
        let lancement = std::thread::Builder::new()
            .name(format!("mm-tache-{id}"))
            .spawn(move || {
                if let Some(t) = ticket {
                    interne.file.attendre_tour(t);
                }
                let resultat = executer(&interne, &ctx, &nom, travail);
                if ticket.is_some() {
                    interne.file.liberer();
                }
                interne
                    .registre
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id);
                interne
                    .jetons
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id);
                let issue = match &resultat {
                    Ok(_) => IssueTache::Reussie,
                    Err(ErreurApp::Annulee) => IssueTache::Annulee,
                    Err(e) => IssueTache::Echouee(e.to_string()),
                };
                // Résultat d'abord : qui reçoit `TacheTerminee` peut le lire aussitôt.
                let _ = tx_res.send(resultat);
                ctx.emettre(Evenement::TacheTerminee {
                    tache: id,
                    nom,
                    issue,
                });
            });
        match lancement {
            Ok(fil) => {
                let mut fils = i.fils.lock().unwrap_or_else(|e| e.into_inner());
                fils.retain(|f| !f.is_finished());
                fils.push(fil);
            }
            Err(e) => {
                tracing::error!("impossible de créer le fil de la tâche {id} : {e}");
                i.registre
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id);
                if let Some(t) = ticket {
                    // Le ticket pris doit être consommé pour ne pas bloquer la file.
                    let file = &self.interne.file;
                    file.attendre_tour(t);
                    file.liberer();
                }
            }
        }
        poignee
    }

    /// Demande l'annulation d'une tâche ; `false` si elle est inconnue.
    pub fn annuler(&self, id: IdTache) -> bool {
        match self
            .interne
            .jetons
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&id)
        {
            Some(j) => {
                j.annuler();
                true
            }
            None => false,
        }
    }

    /// Demande l'annulation de toutes les tâches actives.
    pub fn annuler_tout(&self) {
        for j in self
            .interne
            .jetons
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
        {
            j.annuler();
        }
    }

    /// Tâches en attente ou en cours.
    pub fn taches_actives(&self) -> Vec<InfoTache> {
        self.interne
            .registre
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect()
    }

    /// Vrai si aucune tâche n'est active.
    pub fn est_inactif(&self) -> bool {
        self.interne
            .registre
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    }

    /// Attend que plus aucune tâche ne soit active ; `false` si le délai expire.
    pub fn attendre_inactivite(&self, delai: Duration) -> bool {
        let debut = Instant::now();
        loop {
            let fini = {
                let fils = self.interne.fils.lock().unwrap_or_else(|e| e.into_inner());
                fils.iter().all(|f| f.is_finished())
            };
            if fini && self.est_inactif() {
                return true;
            }
            if debut.elapsed() >= delai {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Arrêt : refuse les nouvelles tâches, annule les actives, attend leur
    /// fin au plus `delai`. Renvoie `true` si toutes se sont terminées.
    pub fn arreter(&self, delai: Duration) -> bool {
        self.interne.arret.store(true, Ordering::SeqCst);
        self.annuler_tout();
        let ok = self.attendre_inactivite(delai);
        if ok {
            let fils: Vec<JoinHandle<()>> =
                std::mem::take(&mut *self.interne.fils.lock().unwrap_or_else(|e| e.into_inner()));
            for f in fils {
                let _ = f.join();
            }
        }
        ok
    }
}

fn executer<T, F>(interne: &Interne, ctx: &ContexteTache, nom: &str, travail: F) -> Resultat<T>
where
    F: FnOnce(&ContexteTache) -> Resultat<T>,
{
    if let Some(i) = interne
        .registre
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_mut(&ctx.id)
    {
        i.etat = EtatTache::EnCours;
    }
    if ctx.est_annulee() {
        return Err(ErreurApp::Annulee);
    }
    tracing::info!("tâche {} « {nom} » démarrée", ctx.id);
    ctx.emettre(Evenement::TacheDemarree {
        tache: ctx.id,
        nom: nom.to_string(),
    });
    let debut = Instant::now();
    let r = match catch_unwind(AssertUnwindSafe(|| travail(ctx))) {
        Ok(r) => r,
        Err(p) => {
            let msg = p
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panique".into());
            tracing::error!("panique dans la tâche {} « {nom} » : {msg}", ctx.id);
            Err(ErreurApp::Interne(msg))
        }
    };
    match &r {
        Ok(_) => tracing::info!(
            "tâche {} « {nom} » terminée en {:?}",
            ctx.id,
            debut.elapsed()
        ),
        Err(ErreurApp::Annulee) => tracing::info!("tâche {} « {nom} » annulée", ctx.id),
        Err(e) => {
            ctx.message(NiveauMessage::Erreur, format!("{nom} : {e}"));
        }
    }
    r
}
