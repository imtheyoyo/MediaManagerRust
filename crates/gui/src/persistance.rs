//! Persistance de l'interface : fichier `ui.json` (colonnes, tri, séparateur, dernier module,
//! fenêtre, thème, langue, filtres et préréglages, confirmations masquées, assistant terminé).
//!
//! Le chemin est fourni par l'appelant (le dossier de données viendra du store). Écriture
//! atomique : fichier temporaire voisin, `sync_all`, puis renommage. Aucun secret n'y est écrit.

use crate::filtres::LigneFiltre;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const VERSION_FORMAT: u32 = 1;
pub const NOM_FICHIER: &str = "ui.json";

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeometrieFenetre {
    pub largeur: f32,
    pub hauteur: f32,
    pub maximisee: bool,
}

impl Default for GeometrieFenetre {
    fn default() -> Self {
        Self {
            largeur: 1100.0,
            hauteur: 720.0,
            maximisee: false,
        }
    }
}

impl GeometrieFenetre {
    /// Taille bornée au minimum de la spécification (1050 x 700).
    pub fn taille_bornee(&self) -> [f32; 2] {
        let ok = |v: f32, min: f32, defaut: f32| {
            if v.is_finite() && v > 0.0 {
                v.max(min)
            } else {
                defaut
            }
        };
        [
            ok(self.largeur, 1050.0, 1100.0),
            ok(self.hauteur, 700.0, 720.0),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColonnePersistee {
    pub id: String,
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub largeur: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TriPersiste {
    pub colonne: String,
    pub croissant: bool,
}

/// Contenu du fichier `ui.json` (tous les champs ont une valeur par défaut : un fichier
/// partiel ou d'une version antérieure se charge sans erreur).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EtatInterface {
    pub version: u32,
    pub fenetre: GeometrieFenetre,
    pub dernier_module: String,
    pub theme: String,
    pub langue: String,
    /// Largeur du panneau de détails (position du séparateur).
    pub separateur: Option<f32>,
    /// Ordre d'affichage = ordre de la liste.
    pub colonnes: Vec<ColonnePersistee>,
    pub tri: Vec<TriPersiste>,
    pub filtrage_actif: bool,
    pub filtres: Vec<LigneFiltre>,
    pub prereglages_filtres: BTreeMap<String, Vec<LigneFiltre>>,
    pub confirmations_masquees: BTreeSet<String>,
    pub assistant_termine: bool,
    /// Recherche d'une nouvelle version au démarrage (réglage Général).
    pub verifier_mises_a_jour: bool,
}

impl Default for EtatInterface {
    fn default() -> Self {
        Self {
            version: VERSION_FORMAT,
            fenetre: GeometrieFenetre::default(),
            dernier_module: "movies".into(),
            theme: "sombre".into(),
            langue: "fr".into(),
            separateur: None,
            colonnes: Vec::new(),
            tri: Vec::new(),
            filtrage_actif: true,
            filtres: Vec::new(),
            prereglages_filtres: BTreeMap::new(),
            confirmations_masquees: BTreeSet::new(),
            assistant_termine: false,
            verifier_mises_a_jour: true,
        }
    }
}

#[derive(Debug)]
pub enum ErreurPersistance {
    Lecture(io::Error),
    Format(serde_json::Error),
}

impl std::fmt::Display for ErreurPersistance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErreurPersistance::Lecture(e) => write!(f, "lecture impossible : {e}"),
            ErreurPersistance::Format(e) => write!(f, "format invalide : {e}"),
        }
    }
}

/// Charge l'état : fichier absent => valeurs par défaut ; illisible ou invalide => erreur
/// (l'appelant notifie et repart des valeurs par défaut).
pub fn charger(chemin: &Path) -> Result<EtatInterface, ErreurPersistance> {
    match fs::read_to_string(chemin) {
        Ok(texte) => serde_json::from_str(&texte).map_err(ErreurPersistance::Format),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(EtatInterface::default()),
        Err(e) => Err(ErreurPersistance::Lecture(e)),
    }
}

fn chemin_temporaire(chemin: &Path) -> PathBuf {
    let mut nom = chemin
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| NOM_FICHIER.into());
    nom.push(".tmp");
    chemin.with_file_name(nom)
}

/// Écriture atomique : `ui.json.tmp` complet et synchronisé, puis renommage par-dessus `ui.json`.
pub fn enregistrer(chemin: &Path, etat: &EtatInterface) -> io::Result<()> {
    if let Some(parent) = chemin.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    let texte = serde_json::to_string_pretty(etat).map_err(io::Error::other)?;
    let tmp = chemin_temporaire(chemin);
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(texte.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, chemin).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// Chemin par défaut en attendant le dossier de données du store : variable
/// `MEDIAMANAGER_UI`, sinon `donnees/ui.json` à côté de l'exécutable.
pub fn chemin_par_defaut() -> PathBuf {
    if let Some(p) = std::env::var_os("MEDIAMANAGER_UI") {
        return PathBuf::from(p);
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("donnees")
        .join(NOM_FICHIER)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filtres::{EtatFiltre, IdFiltre};

    fn dossier(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mmr-persist-{}-{nom}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn fichier_absent_donne_les_valeurs_par_defaut() {
        let d = dossier("absent");
        assert_eq!(
            charger(&d.join("ui.json")).unwrap(),
            EtatInterface::default()
        );
    }

    #[test]
    fn aller_retour_et_ecriture_atomique() {
        let d = dossier("aller");
        let chemin = d.join("sous").join("ui.json");
        let mut e = EtatInterface {
            separateur: Some(512.5),
            theme: "clair".into(),
            assistant_termine: true,
            ..Default::default()
        };
        e.colonnes.push(ColonnePersistee {
            id: "year".into(),
            visible: true,
            largeur: Some(80.0),
        });
        let mut l = LigneFiltre::nouvelle(IdFiltre::Vu);
        l.etat = EtatFiltre::Exclure;
        e.prereglages_filtres.insert("Non vus".into(), vec![l]);
        enregistrer(&chemin, &e).unwrap();
        assert!(!chemin_temporaire(&chemin).exists());
        assert_eq!(charger(&chemin).unwrap(), e);
        // une seconde écriture remplace le fichier existant
        e.separateur = None;
        enregistrer(&chemin, &e).unwrap();
        assert_eq!(charger(&chemin).unwrap().separateur, None);
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn fichier_partiel_ou_invalide() {
        let d = dossier("partiel");
        fs::create_dir_all(&d).unwrap();
        let chemin = d.join("ui.json");
        fs::write(&chemin, r#"{"theme":"clair","inconnu":3}"#).unwrap();
        let e = charger(&chemin).unwrap();
        assert_eq!(e.theme, "clair");
        assert_eq!(e.fenetre, GeometrieFenetre::default());
        fs::write(&chemin, "{ pas du json").unwrap();
        assert!(matches!(
            charger(&chemin),
            Err(ErreurPersistance::Format(_))
        ));
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn taille_de_fenetre_bornee() {
        let g = GeometrieFenetre {
            largeur: 800.0,
            hauteur: f32::NAN,
            maximisee: false,
        };
        assert_eq!(g.taille_bornee(), [1050.0, 720.0]);
    }
}
