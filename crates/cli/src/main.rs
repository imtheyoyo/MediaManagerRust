//! `mediamanager` : gestion de la médiathèque en ligne de commande.
//!
//! Toutes les opérations passent par la couche de services
//! (`mediamanager-app`) : même stockage, mêmes réglages et même verrou
//! d'instance que l'interface graphique.

mod arguments;
mod commandes;
mod erreur;
mod sortie;

use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::Parser;
use mediamanager_app::{Application, ConfigServices, OptionsDemarrage, OptionsJournal};
use mediamanager_store::{DossiersApplication, ModeDossiers};

use arguments::{Arguments, Commande, OptionsGlobales};
use erreur::{CodeSortie, ErreurCli, ResultatCli};
use sortie::Sortie;

fn main() -> ExitCode {
    let arguments = match Arguments::try_parse() {
        Ok(a) => a,
        Err(e) => {
            // Aide et version : sortie standard, code 0 ; erreur d'arguments : code 1
            // (le code 2 est réservé aux conflits de renommage).
            let _ = e.print();
            return if e.use_stderr() {
                CodeSortie::Erreur.into()
            } else {
                CodeSortie::Succes.into()
            };
        }
    };
    let sortie = Sortie {
        verbeux: arguments.globales.verbeux,
        silencieux: arguments.globales.silencieux,
    };
    match executer(arguments) {
        Ok(code) => code.into(),
        Err(e) => {
            if e.code != CodeSortie::Annulee || !sortie.silencieux {
                eprintln!("erreur : {e}");
            }
            e.code.into()
        }
    }
}

/// Options de démarrage de la couche de services.
fn options_demarrage(g: &OptionsGlobales) -> OptionsDemarrage {
    let dossiers = g.racine.as_ref().map(|d| {
        let absolu = std::path::absolute(d).unwrap_or_else(|_| d.clone());
        DossiersApplication::depuis_racine(absolu, ModeDossiers::Explicite)
    });
    let filtre = match g.verbeux {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };
    OptionsDemarrage {
        dossiers,
        coffre: None,
        journal: OptionsJournal {
            filtre: filtre.into(),
            // Les messages des tâches sont déjà affichés par l'outil ; la copie
            // console du journal ne sert qu'en mode verbeux.
            console: g.verbeux > 0,
            ..OptionsJournal::default()
        },
        services: ConfigServices::default(),
    }
}

/// Ctrl+C : annule la tâche en cours (arrêt propre) ; sans tâche active ou au
/// second appui, quitte immédiatement.
fn installer_interruption(app: &Application) {
    static DEJA_DEMANDE: AtomicBool = AtomicBool::new(false);
    let taches = app.taches().clone();
    let r = ctrlc::set_handler(move || {
        if taches.est_inactif() || DEJA_DEMANDE.swap(true, Ordering::SeqCst) {
            eprintln!("\ninterrompu");
            std::process::exit(i32::from(CodeSortie::Annulee.valeur()));
        }
        eprintln!("\nannulation en cours… (Ctrl+C à nouveau pour quitter immédiatement)");
        taches.annuler_tout();
    });
    if let Err(e) = r {
        eprintln!("avertissement : gestion de Ctrl+C indisponible : {e}");
    }
}

fn executer(arguments: Arguments) -> ResultatCli {
    let Arguments { globales, commande } = arguments;
    let sortie = Sortie {
        verbeux: globales.verbeux,
        silencieux: globales.silencieux,
    };
    let app = Application::demarrer(options_demarrage(&globales))?;
    installer_interruption(&app);
    let resultat = lancer(&app, &sortie, commande);
    let arret = app.arreter().map_err(ErreurCli::from);
    match (resultat, arret) {
        (Ok(code), Ok(())) => Ok(code),
        (Ok(_), Err(e)) => Err(e),
        (Err(e), _) => Err(e),
    }
}

fn lancer(app: &Application, s: &Sortie, commande: Commande) -> ResultatCli {
    match commande {
        Commande::Sources { action } => commandes::sources(app, s, action),
        Commande::Maj { series, tout } => commandes::maj(app, s, series, tout),
        Commande::Lister {
            nouveaux,
            sans_metadonnees,
            recherche,
            json,
        } => commandes::lister(
            app,
            s,
            nouveaux,
            sans_metadonnees,
            recherche.as_deref(),
            json,
        ),
        Commande::Scraper { selection } => commandes::scraper(app, s, &selection),
        Commande::Nfo { selection } => commandes::nfo(app, s, &selection),
        Commande::Renommer {
            selection,
            appliquer,
            oui,
        } => commandes::renommer(app, s, &selection, appliquer, oui),
        Commande::CleTmdb { supprimer } => commandes::cle_tmdb(app, s, supprimer),
        Commande::Info => commandes::info(app),
    }
}
