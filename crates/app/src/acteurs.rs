//! Photos des acteurs : téléchargement dans `.actors/<Nom_Prénom>.jpg`
//! (convention des NFO Kodi), à côté du film ou dans le dossier de la série.
//!
//! Les fichiers existants ne sont jamais écrasés ; une erreur réseau sur une
//! photo produit un avertissement sans interrompre le reste.

use std::path::{Path, PathBuf};

use mediamanager_core::modele::Personne;
use mediamanager_core::nfo::nom_fichier_acteur;

use crate::artwork::{self, ecrire_atomiquement, extension_url};
use crate::evenements::NiveauMessage;
use crate::taches::ContexteTache;

/// Nom du dossier des photos d'acteurs.
pub const DOSSIER_ACTEURS: &str = ".actors";

/// Nombre maximal d'avertissements individuels émis par téléchargement.
const MAX_AVERTISSEMENTS: usize = 3;

/// Extensions sous lesquelles une photo peut déjà exister.
const EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];

/// Bilan d'un téléchargement de photos d'acteurs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BilanPhotos {
    /// Photos écrites.
    pub telechargees: usize,
    /// Photos en échec (réseau, statut HTTP, écriture).
    pub echecs: usize,
}

impl BilanPhotos {
    /// Cumule un autre bilan.
    pub fn ajouter(&mut self, autre: BilanPhotos) {
        self.telechargees += autre.telechargees;
        self.echecs += autre.echecs;
    }
}

/// Chemin du dossier des photos d'acteurs d'un dossier de film ou de série.
pub fn dossier_acteurs_de(base: &Path) -> PathBuf {
    base.join(DOSSIER_ACTEURS)
}

/// Marque le dossier comme caché (attribut caché de Windows ; sans effet
/// ailleurs, où le point initial suffit). Une erreur est ignorée.
pub fn masquer_dossier_acteurs(dossier: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use std::os::windows::process::CommandExt;
        const CACHE: u32 = 0x2;
        const SANS_FENETRE: u32 = 0x0800_0000;
        let deja = std::fs::metadata(dossier)
            .map(|m| m.file_attributes() & CACHE != 0)
            .unwrap_or(true);
        if !deja {
            let _ = std::process::Command::new("attrib")
                .arg("+h")
                .arg(dossier)
                .creation_flags(SANS_FENETRE)
                .status();
        }
    }
    #[cfg(not(windows))]
    {
        let _ = dossier;
    }
}

/// Après un renommage où le dossier du film a changé SANS être déplacé en
/// bloc (fichiers déplacés un à un), emporte `.actors` de l'ancien dossier
/// vers le nouveau (sans jamais écraser). À n'appeler que si l'ancien dossier
/// n'était pas partagé avec d'autres films. Renvoie vrai si quelque chose a
/// été déplacé ; les erreurs sont ignorées (au mieux).
pub fn emporter_dossier_acteurs(ancien: &Path, nouveau: &Path) -> bool {
    if ancien == nouveau {
        return false;
    }
    let (de, vers) = (dossier_acteurs_de(ancien), dossier_acteurs_de(nouveau));
    if !de.is_dir() || !nouveau.is_dir() {
        return false;
    }
    if !vers.exists() {
        let ok =
            mediamanager_core::renommage::execution::deplacer_dossier(&de, &vers, false).is_ok();
        if ok {
            masquer_dossier_acteurs(&vers);
        }
        return ok;
    }
    let mut deplace = false;
    if let Ok(entrees) = std::fs::read_dir(&de) {
        for e in entrees.flatten() {
            let cible = vers.join(e.file_name());
            if !cible.exists()
                && mediamanager_core::renommage::execution::deplacer_fichier(
                    &e.path(),
                    &cible,
                    false,
                )
                .is_ok()
            {
                deplace = true;
            }
        }
    }
    let _ = std::fs::remove_dir(&de);
    deplace
}

/// Photo d'acteur à télécharger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhotoActeur {
    pub url: String,
    pub destination: PathBuf,
}

/// Photos à télécharger pour `acteurs` dans `<base>/.actors` : acteurs avec
/// URL de photo, sans doublon de nom, dont le fichier n'existe pas encore.
pub fn planifier_photos_acteurs(base: &Path, acteurs: &[Personne]) -> Vec<PhotoActeur> {
    let dossier = base.join(DOSSIER_ACTEURS);
    let mut sortie: Vec<PhotoActeur> = Vec::new();
    for a in acteurs {
        let url = a.url_vignette.trim();
        let nom = nom_fichier_acteur(&a.nom);
        if url.is_empty() || nom.is_empty() {
            continue;
        }
        if EXTENSIONS
            .iter()
            .any(|e| dossier.join(format!("{nom}.{e}")).is_file())
        {
            continue;
        }
        let destination = dossier.join(format!("{nom}.{}", extension_url(url)));
        if sortie.iter().any(|p| p.destination == destination) {
            continue;
        }
        sortie.push(PhotoActeur {
            url: url.to_string(),
            destination,
        });
    }
    sortie
}

/// Télécharge les photos manquantes dans `<base>/.actors` ; renvoie le bilan.
/// `base` doit être le dossier existant du film ou de la série (un dossier
/// absent ou vide n'est jamais créé). Seule l'annulation interrompt le
/// traitement.
pub(crate) fn telecharger_photos_acteurs(
    ctx: &ContexteTache,
    base: &Path,
    acteurs: &[Personne],
) -> crate::erreur::Resultat<BilanPhotos> {
    if base.as_os_str().is_empty() || !base.is_dir() {
        return Ok(BilanPhotos::default());
    }
    let dossier = dossier_acteurs_de(base);
    if dossier.is_dir() {
        masquer_dossier_acteurs(&dossier);
    }
    let plan = planifier_photos_acteurs(base, acteurs);
    if plan.is_empty() {
        return Ok(BilanPhotos::default());
    }
    let client = artwork::client_telechargement()?;
    let (mut ecrites, mut echecs) = (0, 0);
    for p in plan {
        ctx.verifier_annulation()?;
        let resultat = ctx
            .bloquer(artwork::telecharger(&client, &p.url))
            .and_then(|octets| ecrire_atomiquement(&p.destination, &octets));
        match resultat {
            Ok(()) => ecrites += 1,
            Err(crate::erreur::ErreurApp::Annulee) => {
                return Err(crate::erreur::ErreurApp::Annulee);
            }
            Err(e) => {
                echecs += 1;
                if echecs <= MAX_AVERTISSEMENTS {
                    ctx.message(
                        NiveauMessage::Avertissement,
                        format!("photo d'acteur indisponible : {e}"),
                    );
                }
            }
        }
    }
    if echecs > MAX_AVERTISSEMENTS {
        ctx.message(
            NiveauMessage::Avertissement,
            format!("{echecs} photo(s) d'acteurs n'ont pas pu être téléchargées"),
        );
    }
    masquer_dossier_acteurs(&dossier);
    Ok(BilanPhotos {
        telechargees: ecrites,
        echecs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acteur(nom: &str, url: &str) -> Personne {
        Personne {
            nom: nom.into(),
            url_vignette: url.into(),
            ..Personne::default()
        }
    }

    #[test]
    fn plan_sans_doublon_ni_existant() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join(".actors")).unwrap();
        std::fs::write(d.path().join(".actors/Neo_Un.png"), b"x").unwrap();
        let acteurs = vec![
            acteur("Keanu Reeves", "http://h/k.jpg"),
            acteur("Keanu Reeves", "http://h/k2.jpg"),
            acteur("Neo Un", "http://h/n.jpg"),
            acteur("Sans Photo", ""),
        ];
        let p = planifier_photos_acteurs(d.path(), &acteurs);
        assert_eq!(p.len(), 1);
        assert_eq!(
            p[0].destination,
            d.path().join(".actors").join("Keanu_Reeves.jpg")
        );
    }

    #[test]
    fn noms_de_fichiers_sans_caracteres_interdits() {
        let d = tempfile::tempdir().unwrap();
        let acteurs = vec![acteur("Jean Luc: \"O'Brien\" ?", "http://h/a.png")];
        let p = planifier_photos_acteurs(d.path(), &acteurs);
        assert_eq!(
            p[0].destination,
            d.path().join(".actors").join("Jean_Luc_O'Brien.png")
        );
    }

    #[test]
    fn emporter_actors_vers_nouveau_dossier_sans_ecraser() {
        let d = tempfile::tempdir().unwrap();
        let (a, n) = (d.path().join("A"), d.path().join("A/Sous"));
        std::fs::create_dir_all(a.join(".actors")).unwrap();
        std::fs::create_dir_all(&n).unwrap();
        std::fs::write(a.join(".actors/X.jpg"), b"ancien").unwrap();
        assert!(emporter_dossier_acteurs(&a, &n));
        assert!(!a.join(".actors").exists());
        assert_eq!(std::fs::read(n.join(".actors/X.jpg")).unwrap(), b"ancien");
        // Fusion sans écrasement quand la destination en a déjà un.
        std::fs::create_dir_all(a.join(".actors")).unwrap();
        std::fs::write(a.join(".actors/X.jpg"), b"autre").unwrap();
        std::fs::write(a.join(".actors/Y.jpg"), b"y").unwrap();
        assert!(emporter_dossier_acteurs(&a, &n));
        assert_eq!(std::fs::read(n.join(".actors/X.jpg")).unwrap(), b"ancien");
        assert_eq!(std::fs::read(n.join(".actors/Y.jpg")).unwrap(), b"y");
        // Le doublon existant n'est jamais écrasé ni supprimé.
        assert_eq!(std::fs::read(a.join(".actors/X.jpg")).unwrap(), b"autre");
    }
}
