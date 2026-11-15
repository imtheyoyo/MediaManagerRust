//! Définition des arguments de la ligne de commande (clap, dérivation).

use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand};

/// Gestion de la médiathèque (films et séries) en ligne de commande.
#[derive(Debug, Parser)]
#[command(
    name = "mediamanager",
    version,
    about = "Gestion de la médiathèque (films et séries) en ligne de commande",
    long_about = None,
    after_help = "Codes de sortie : 0 succès, 1 erreur, 2 conflits de renommage, \
                  3 instance déjà lancée, 130 opération annulée (Ctrl+C)."
)]
pub struct Arguments {
    #[command(flatten)]
    pub globales: OptionsGlobales,

    #[command(subcommand)]
    pub commande: Commande,
}

/// Options valables pour toutes les sous-commandes.
#[derive(Debug, Clone, Default, Args)]
pub struct OptionsGlobales {
    /// Racine des données de l'application (équivaut à la variable
    /// d'environnement MEDIAMANAGER_DOSSIER).
    #[arg(id = "racine", long = "dossier", global = true, value_name = "DOSSIER")]
    pub racine: Option<PathBuf>,

    /// Plus de détails (répétable : -vv).
    #[arg(short = 'v', long = "verbeux", global = true, action = ArgAction::Count)]
    pub verbeux: u8,

    /// N'affiche que les résultats et les erreurs (pas de progression).
    #[arg(
        short = 'q',
        long = "silencieux",
        global = true,
        conflicts_with = "verbeux"
    )]
    pub silencieux: bool,
}

/// Sous-commandes.
#[derive(Debug, Subcommand)]
pub enum Commande {
    /// Gère les dossiers sources (films par défaut, séries avec --series).
    Sources {
        #[command(subcommand)]
        action: ActionSources,
    },
    /// Met à jour la bibliothèque depuis les sources.
    Maj {
        /// Sources de séries au lieu des films.
        #[arg(long, conflicts_with = "tout")]
        series: bool,
        /// Films puis séries.
        #[arg(long)]
        tout: bool,
    },
    /// Liste les films de la bibliothèque.
    Lister {
        /// Seulement les films ajoutés par la dernière mise à jour.
        #[arg(long)]
        nouveaux: bool,
        /// Seulement les films sans métadonnées (ni identifiant ni résumé).
        #[arg(long)]
        sans_metadonnees: bool,
        /// Filtre sur le titre (insensible à la casse).
        #[arg(long, value_name = "TEXTE")]
        recherche: Option<String>,
        /// Sortie JSON (pour les scripts).
        #[arg(long)]
        json: bool,
    },
    /// Récupère automatiquement les métadonnées (TMDB).
    Scraper {
        #[command(flatten)]
        selection: Selection,
    },
    /// Réécrit les fichiers NFO (tous les films par défaut).
    Nfo {
        #[command(flatten)]
        selection: Selection,
    },
    /// Renomme les films selon les réglages (aperçu par défaut).
    Renommer {
        #[command(flatten)]
        selection: Selection,
        /// Exécute réellement le renommage.
        #[arg(long)]
        appliquer: bool,
        /// Ne demande pas de confirmation.
        #[arg(long, requires = "appliquer")]
        oui: bool,
    },
    /// Enregistre la clé d'API TMDB personnelle (saisie masquée ou variable
    /// d'environnement MEDIAMANAGER_CLE_TMDB).
    #[command(name = "cle-tmdb")]
    CleTmdb {
        /// Efface la clé enregistrée.
        #[arg(long)]
        supprimer: bool,
    },
    /// Affiche les dossiers utilisés, la version et le contenu de la bibliothèque.
    Info,
}

/// Actions sur les sources.
#[derive(Debug, Subcommand)]
pub enum ActionSources {
    /// Ajoute un dossier source.
    Ajouter {
        #[arg(value_name = "DOSSIER")]
        chemin: PathBuf,
        /// Source de séries.
        #[arg(long)]
        series: bool,
    },
    /// Retire un dossier source (les fichiers ne sont pas touchés).
    Retirer {
        #[arg(value_name = "DOSSIER")]
        chemin: PathBuf,
        /// Source de séries.
        #[arg(long)]
        series: bool,
    },
    /// Liste les dossiers sources.
    Lister {
        /// Sources de séries.
        #[arg(long)]
        series: bool,
    },
}

/// Sélection de films (options exclusives).
#[derive(Debug, Clone, Default, PartialEq, Eq, Args)]
#[group(multiple = false)]
pub struct Selection {
    /// Films ajoutés par la dernière mise à jour.
    #[arg(long)]
    pub nouveaux: bool,
    /// Tous les films.
    #[arg(long)]
    pub tous: bool,
    /// Identifiant (ou début d'identifiant) d'un film ; répétable.
    #[arg(long = "id", value_name = "ID")]
    pub ids: Vec<String>,
}

/// Sélection effective, une fois la valeur par défaut appliquée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cible {
    Nouveaux,
    Tous,
    Ids(Vec<String>),
}

impl Selection {
    /// Cible retenue ; `defaut` s'applique si aucune option n'est donnée.
    pub fn cible(&self, defaut: Cible) -> Cible {
        if !self.ids.is_empty() {
            Cible::Ids(self.ids.clone())
        } else if self.tous {
            Cible::Tous
        } else if self.nouveaux {
            Cible::Nouveaux
        } else {
            defaut
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap::error::ErrorKind;

    fn analyser(args: &[&str]) -> Result<Arguments, clap::Error> {
        Arguments::try_parse_from(std::iter::once("mediamanager").chain(args.iter().copied()))
    }

    #[test]
    fn definition_valide() {
        Arguments::command().debug_assert();
    }

    #[test]
    fn options_globales_partout() {
        let a = analyser(&["info", "--dossier", "/tmp/x", "-vv"]).unwrap();
        assert_eq!(a.globales.racine, Some(PathBuf::from("/tmp/x")));
        assert_eq!(a.globales.verbeux, 2);
        assert!(matches!(a.commande, Commande::Info));
        let a = analyser(&["-q", "lister"]).unwrap();
        assert!(a.globales.silencieux);
    }

    #[test]
    fn verbeux_et_silencieux_incompatibles() {
        let e = analyser(&["-v", "-q", "info"]).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn sources() {
        let a = analyser(&["sources", "ajouter", "/films"]).unwrap();
        match a.commande {
            Commande::Sources {
                action: ActionSources::Ajouter { chemin, series },
            } => {
                assert_eq!(chemin, PathBuf::from("/films"));
                assert!(!series);
            }
            c => panic!("inattendu : {c:?}"),
        }
        let a = analyser(&["sources", "retirer", "--series", "/s"]).unwrap();
        assert!(matches!(
            a.commande,
            Commande::Sources {
                action: ActionSources::Retirer { series: true, .. }
            }
        ));
        assert!(analyser(&["sources", "ajouter"]).is_err());
        assert!(analyser(&["sources", "lister", "--series"]).is_ok());
    }

    #[test]
    fn dossier_global_distinct_du_dossier_source() {
        let a = analyser(&["--dossier", "/app", "sources", "ajouter", "/films"]).unwrap();
        assert_eq!(a.globales.racine, Some(PathBuf::from("/app")));
        let a = analyser(&["sources", "ajouter", "/films", "--dossier", "/app"]).unwrap();
        assert_eq!(a.globales.racine, Some(PathBuf::from("/app")));
        match a.commande {
            Commande::Sources {
                action: ActionSources::Ajouter { chemin, .. },
            } => assert_eq!(chemin, PathBuf::from("/films")),
            c => panic!("inattendu : {c:?}"),
        }
    }

    #[test]
    fn maj() {
        let a = analyser(&["maj", "--tout"]).unwrap();
        assert!(matches!(
            a.commande,
            Commande::Maj {
                tout: true,
                series: false
            }
        ));
        assert!(analyser(&["maj", "--tout", "--series"]).is_err());
    }

    #[test]
    fn lister() {
        let a = analyser(&[
            "lister",
            "--sans-metadonnees",
            "--recherche",
            "matrix",
            "--json",
        ])
        .unwrap();
        match a.commande {
            Commande::Lister {
                nouveaux,
                sans_metadonnees,
                recherche,
                json,
            } => {
                assert!(!nouveaux && sans_metadonnees && json);
                assert_eq!(recherche.as_deref(), Some("matrix"));
            }
            c => panic!("inattendu : {c:?}"),
        }
    }

    #[test]
    fn selection_exclusive_et_defaut() {
        let a = analyser(&["scraper"]).unwrap();
        let Commande::Scraper { selection } = a.commande else {
            panic!()
        };
        assert_eq!(selection.cible(Cible::Nouveaux), Cible::Nouveaux);

        let a = analyser(&["scraper", "--id", "abc", "--id", "def"]).unwrap();
        let Commande::Scraper { selection } = a.commande else {
            panic!()
        };
        assert_eq!(
            selection.cible(Cible::Nouveaux),
            Cible::Ids(vec!["abc".into(), "def".into()])
        );

        let e = analyser(&["scraper", "--tous", "--nouveaux"]).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::ArgumentConflict);
        assert!(analyser(&["nfo", "--tous", "--id", "x"]).is_err());
        let a = analyser(&["nfo"]).unwrap();
        let Commande::Nfo { selection } = a.commande else {
            panic!()
        };
        assert_eq!(selection.cible(Cible::Tous), Cible::Tous);
    }

    #[test]
    fn renommer_oui_exige_appliquer() {
        let e = analyser(&["renommer", "--oui"]).unwrap_err();
        assert_eq!(e.kind(), ErrorKind::MissingRequiredArgument);
        let a = analyser(&["renommer", "--appliquer", "--oui", "--nouveaux"]).unwrap();
        assert!(matches!(
            a.commande,
            Commande::Renommer {
                appliquer: true,
                oui: true,
                ..
            }
        ));
    }

    #[test]
    fn cle_tmdb_sans_valeur_en_argument() {
        assert!(analyser(&["cle-tmdb"]).is_ok());
        assert!(analyser(&["cle-tmdb", "--supprimer"]).is_ok());
        // La clé ne doit jamais transiter par la ligne de commande.
        assert!(analyser(&["cle-tmdb", "secret"]).is_err());
    }

    #[test]
    fn sous_commande_obligatoire() {
        assert!(analyser(&[]).is_err());
        assert!(analyser(&["inconnue"]).is_err());
    }
}
