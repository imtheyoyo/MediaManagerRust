//! Ouverture de fichiers, de dossiers et d'adresses avec les applications du système :
//! construction de la commande (fonctions pures, testées pour chaque système) puis exécution
//! non bloquante (processus lancé et attendu dans un fil dédié).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Système cible de la commande.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Systeme {
    Windows,
    MacOs,
    /// Linux et autres Unix (bureau XDG).
    Unix,
}

impl Systeme {
    /// Système de compilation.
    pub fn courant() -> Self {
        if cfg!(windows) {
            Systeme::Windows
        } else if cfg!(target_os = "macos") {
            Systeme::MacOs
        } else {
            Systeme::Unix
        }
    }
}

/// Ce que l'utilisateur veut ouvrir.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CibleOuverture {
    /// Dossier dans le gestionnaire de fichiers, en sélectionnant `selection` si possible.
    Dossier {
        dossier: PathBuf,
        selection: Option<PathBuf>,
    },
    /// Fichier avec l'application associée (lecture d'une vidéo...).
    Fichier(PathBuf),
    /// Adresse web (navigateur par défaut).
    Adresse(String),
}

/// Commande prête à lancer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandeSysteme {
    pub programme: String,
    pub arguments: Vec<String>,
    /// Arguments transmis tels quels, sans les guillemets automatiques de Windows
    /// (`explorer /select,"chemin"` n'accepte pas l'argument entièrement entre guillemets).
    pub bruts: bool,
}

impl CommandeSysteme {
    fn simple(programme: &str, arguments: Vec<String>) -> Self {
        Self {
            programme: programme.into(),
            arguments,
            bruts: false,
        }
    }
}

fn texte(p: &Path) -> String {
    p.display().to_string()
}

/// Chemin au format Windows (barres obliques inverses).
fn chemin_windows(p: &Path) -> String {
    texte(p).replace('/', "\\")
}

/// Commande d'ouverture d'une cible sur un système donné.
pub fn commande(cible: &CibleOuverture, systeme: Systeme) -> CommandeSysteme {
    match (cible, systeme) {
        (
            CibleOuverture::Dossier {
                selection: Some(f), ..
            },
            Systeme::Windows,
        ) => CommandeSysteme {
            programme: "explorer".into(),
            arguments: vec![format!("/select,\"{}\"", chemin_windows(f))],
            bruts: true,
        },
        (CibleOuverture::Dossier { dossier, .. }, Systeme::Windows) => {
            CommandeSysteme::simple("explorer", vec![chemin_windows(dossier)])
        }
        (
            CibleOuverture::Dossier {
                selection: Some(f), ..
            },
            Systeme::MacOs,
        ) => CommandeSysteme::simple("open", vec!["-R".into(), texte(f)]),
        (CibleOuverture::Dossier { dossier, .. }, Systeme::MacOs) => {
            CommandeSysteme::simple("open", vec![texte(dossier)])
        }
        // Le bureau XDG ne sait pas sélectionner un fichier : on ouvre son dossier.
        (CibleOuverture::Dossier { dossier, .. }, Systeme::Unix) => {
            CommandeSysteme::simple("xdg-open", vec![texte(dossier)])
        }
        (CibleOuverture::Fichier(f), Systeme::Windows) => {
            CommandeSysteme::simple("explorer", vec![chemin_windows(f)])
        }
        (CibleOuverture::Fichier(f), Systeme::MacOs) => {
            CommandeSysteme::simple("open", vec![texte(f)])
        }
        (CibleOuverture::Fichier(f), Systeme::Unix) => {
            CommandeSysteme::simple("xdg-open", vec![texte(f)])
        }
        (CibleOuverture::Adresse(u), Systeme::Windows) => {
            CommandeSysteme::simple("explorer", vec![u.clone()])
        }
        (CibleOuverture::Adresse(u), Systeme::MacOs) => {
            CommandeSysteme::simple("open", vec![u.clone()])
        }
        (CibleOuverture::Adresse(u), Systeme::Unix) => {
            CommandeSysteme::simple("xdg-open", vec![u.clone()])
        }
    }
}

/// Cible « dossier » d'un fichier ou d'un dossier affiché : un chemin de fichier sélectionne
/// le fichier dans son dossier parent.
pub fn cible_dossier(dossier: &str, fichier: Option<&str>) -> CibleOuverture {
    let dossier = PathBuf::from(dossier);
    let selection = fichier.filter(|f| !f.is_empty()).map(|f| dossier.join(f));
    CibleOuverture::Dossier { dossier, selection }
}

/// Vrai si l'adresse peut être confiée au navigateur (web ou fichier local).
pub fn adresse_valide(url: &str) -> bool {
    let u = url.trim().to_lowercase();
    u.starts_with("http://") || u.starts_with("https://") || u.starts_with("file://")
}

/// Adresse web ouvrable dans le navigateur ou téléchargeable : `http://` ou `https://` seulement
/// (jamais `file://` ni un schéma inconnu).
pub fn adresse_web_valide(url: &str) -> bool {
    let u = url.trim().to_lowercase();
    (u.starts_with("http://") && u.len() > 7) || (u.starts_with("https://") && u.len() > 8)
}

/// Signature d'un lanceur de commandes (remplaçable en test).
pub type Lanceur = std::sync::Arc<dyn Fn(CommandeSysteme) -> Result<(), String> + Send + Sync>;

/// Lanceur réel : démarre le processus (sans attendre sa fin dans le fil appelant) ; le
/// processus est attendu par un fil dédié pour ne pas laisser de zombie.
pub fn lanceur_systeme() -> Lanceur {
    std::sync::Arc::new(|c: CommandeSysteme| {
        let mut cmd = Command::new(&c.programme);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            if c.bruts {
                for a in &c.arguments {
                    cmd.raw_arg(a);
                }
            } else {
                cmd.args(&c.arguments);
            }
        }
        #[cfg(not(windows))]
        cmd.args(&c.arguments);
        let mut enfant = cmd.spawn().map_err(|e| format!("{} : {e}", c.programme))?;
        std::thread::spawn(move || {
            let _ = enfant.wait();
        });
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explorateur_windows_avec_selection() {
        let c = commande(
            &cible_dossier("G:\\Films\\Alien (1979)", Some("Alien.mkv")),
            Systeme::Windows,
        );
        assert_eq!(c.programme, "explorer");
        assert_eq!(
            c.arguments,
            ["/select,\"G:\\Films\\Alien (1979)\\Alien.mkv\""]
        );
        assert!(c.bruts);
        let sans = commande(&cible_dossier("G:/Films/Alien", None), Systeme::Windows);
        assert_eq!(sans.arguments, ["G:\\Films\\Alien"]);
        assert!(!sans.bruts);
    }

    #[test]
    fn autres_systemes() {
        let cible = cible_dossier("/srv/films/Alien", Some("Alien.mkv"));
        let x = commande(&cible, Systeme::Unix);
        assert_eq!(x.programme, "xdg-open");
        assert_eq!(x.arguments, ["/srv/films/Alien"]);
        let m = commande(&cible, Systeme::MacOs);
        // Le séparateur dépend du système qui exécute le test : on normalise.
        let m_normalise: Vec<String> = m.arguments.iter().map(|a| a.replace('\\', "/")).collect();
        assert_eq!(m_normalise, ["-R", "/srv/films/Alien/Alien.mkv"]);
        let f = CibleOuverture::Fichier("/srv/films/Alien/Alien.mkv".into());
        assert_eq!(
            commande(&f, Systeme::Unix).arguments,
            ["/srv/films/Alien/Alien.mkv"]
        );
        let u = CibleOuverture::Adresse("https://exemple.org/v".into());
        assert_eq!(commande(&u, Systeme::Windows).programme, "explorer");
        assert_eq!(commande(&u, Systeme::MacOs).programme, "open");
        // Nom de fichier vide : simple ouverture du dossier.
        assert_eq!(
            cible_dossier("/a", Some("")),
            CibleOuverture::Dossier {
                dossier: "/a".into(),
                selection: None
            }
        );
    }

    #[test]
    fn adresses_acceptees() {
        assert!(adresse_valide("https://www.youtube.com/watch?v=x"));
        assert!(adresse_valide("file:///srv/a.mkv"));
        assert!(!adresse_valide("youtube:abc"));
        assert!(!adresse_valide(""));
        assert!(adresse_web_valide(" HTTPS://youtu.be/x "));
        assert!(adresse_web_valide("http://exemple.org"));
        assert!(!adresse_web_valide("file:///srv/a.mkv"));
        assert!(!adresse_web_valide("https://"));
        assert!(!adresse_web_valide("ftp://x/y"));
        assert!(!adresse_web_valide(""));
    }

    #[test]
    fn programme_absent_signale() {
        let r = lanceur_systeme()(CommandeSysteme::simple(
            "programme-qui-n-existe-pas-mmr",
            vec![],
        ));
        assert!(r.is_err());
    }
}
