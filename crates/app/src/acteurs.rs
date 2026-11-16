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

/// Télécharge les photos manquantes ; renvoie le nombre de photos écrites.
/// Seule l'annulation interrompt le traitement.
pub(crate) fn telecharger_photos_acteurs(
    ctx: &ContexteTache,
    base: &Path,
    acteurs: &[Personne],
) -> crate::erreur::Resultat<usize> {
    let plan = planifier_photos_acteurs(base, acteurs);
    if plan.is_empty() {
        return Ok(0);
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
    Ok(ecrites)
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
}
