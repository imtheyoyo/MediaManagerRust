//! Analyse des fichiers inutiles d'un dossier de film (« Nettoyer ») :
//! logique pure, qui ne touche pas aux fichiers (lecture seule).
//!
//! Sont signalés, dans le dossier du film et ses sous-dossiers :
//! - les fichiers dont le nom correspond à un motif de nettoyage
//!   ([`MOTIFS_PAR_DEFAUT`] : `.txt`, `.url`, `.html`, `.sfv`) ;
//! - les fichiers parasites des systèmes (`Thumbs.db`, `.DS_Store`,
//!   `desktop.ini`…) ;
//! - les extraits (« sample ») : vidéos dont le nom contient le mot `sample`
//!   et dossiers nommés `sample`/`samples` ;
//! - les `.nfo` orphelins : non rattachés au film (absents de ses fichiers
//!   connus ; la mise à jour des sources rattache tout NFO du dossier).
//!
//! Les fichiers *protégés* (ceux que le film référence : vidéos, sous-titres,
//! images, NFO…) ne sont jamais signalés, ni les dossiers qui en contiennent.
//! Les liens symboliques et les dossiers cachés (`.corbeille`…) sont ignorés.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use regex::{Regex, RegexBuilder};
use walkdir::WalkDir;

/// Motifs par défaut (expressions rationnelles sur le nom du fichier).
pub const MOTIFS_PAR_DEFAUT: &[&str] = &[r"\.txt$", r"\.url$", r"\.html$", r"\.sfv$"];

/// Noms de fichiers parasites (comparaison insensible à la casse).
const FICHIERS_PARASITES: &[&str] = &[
    "thumbs.db",
    "ehthumbs.db",
    ".ds_store",
    "desktop.ini",
    "albumart.jpg",
    "albumartsmall.jpg",
];

/// Pourquoi un fichier est jugé inutile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RaisonInutile {
    /// Le nom correspond à ce motif de nettoyage.
    Motif(String),
    /// Fichier parasite d'un système (miniatures, `desktop.ini`…).
    Parasite,
    /// Extrait vidéo ou dossier `sample`.
    Extrait,
    /// NFO non rattaché au film.
    NfoOrphelin,
}

/// Élément signalé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidat {
    /// Chemin absolu (sous le dossier analysé).
    pub chemin: PathBuf,
    pub est_dossier: bool,
    pub raison: RaisonInutile,
    /// Taille en octets (somme récursive pour un dossier).
    pub taille: u64,
}

/// Motifs de nettoyage compilés (insensibles à la casse).
#[derive(Debug, Clone, Default)]
pub struct ReglesNettoyage {
    motifs: Vec<(String, Regex)>,
    /// Motifs invalides, ignorés.
    pub invalides: Vec<String>,
}

impl ReglesNettoyage {
    /// Compile les motifs ; les invalides sont mémorisés dans `invalides`.
    pub fn nouvelles(motifs: &[String]) -> Self {
        let mut r = Self::default();
        for m in motifs {
            let m = m.trim();
            if m.is_empty() {
                continue;
            }
            match RegexBuilder::new(m).case_insensitive(true).build() {
                Ok(re) => r.motifs.push((m.to_string(), re)),
                Err(_) => r.invalides.push(m.to_string()),
            }
        }
        r
    }

    /// Règles avec les motifs par défaut.
    pub fn par_defaut() -> Self {
        let v: Vec<String> = MOTIFS_PAR_DEFAUT.iter().map(|s| s.to_string()).collect();
        Self::nouvelles(&v)
    }

    fn motif_correspondant(&self, nom: &str) -> Option<&str> {
        self.motifs
            .iter()
            .find(|(_, re)| re.is_match(nom))
            .map(|(m, _)| m.as_str())
    }
}

/// Le nom (sans extension) contient le mot `sample` comme mot entier.
pub fn est_nom_extrait(nom_sans_extension: &str) -> bool {
    let bas = nom_sans_extension.to_lowercase();
    let mut debut = 0;
    while let Some(p) = bas[debut..].find("sample") {
        let a = debut + p;
        let b = a + "sample".len();
        let avant_ok = bas[..a]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        let apres_ok = bas[b..].chars().next().is_none_or(|c| !c.is_alphanumeric());
        if avant_ok && apres_ok {
            return true;
        }
        debut = b;
    }
    false
}

fn est_dossier_extrait(nom: &str) -> bool {
    matches!(nom.to_lowercase().as_str(), "sample" | "samples")
}

fn taille_dossier(d: &Path) -> u64 {
    WalkDir::new(d)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Analyse un dossier de film. `proteges` : chemins absolus des fichiers du
/// film à ne jamais signaler (leurs dossiers parents non plus).
pub fn analyser_dossier(
    dossier: &Path,
    proteges: &BTreeSet<PathBuf>,
    regles: &ReglesNettoyage,
    extensions_video: &[String],
) -> std::io::Result<Vec<Candidat>> {
    if !dossier.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "dossier introuvable",
        ));
    }
    let est_video = |ext: &str| {
        extensions_video
            .iter()
            .any(|e| e.trim_start_matches('.').eq_ignore_ascii_case(ext))
    };
    let mut sortie = Vec::new();
    let mut it = WalkDir::new(dossier)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter();
    while let Some(entree) = it.next() {
        let Ok(entree) = entree else { continue };
        if entree.depth() == 0 || entree.path_is_symlink() {
            if entree.depth() > 0 && entree.file_type().is_dir() {
                it.skip_current_dir();
            }
            continue;
        }
        let chemin = entree.path();
        let nom = entree.file_name().to_string_lossy().into_owned();
        if entree.file_type().is_dir() {
            if nom.starts_with('.') {
                it.skip_current_dir();
                continue;
            }
            let contient_protege = proteges.iter().any(|p| p.starts_with(chemin));
            if est_dossier_extrait(&nom) && !contient_protege {
                sortie.push(Candidat {
                    chemin: chemin.to_path_buf(),
                    est_dossier: true,
                    raison: RaisonInutile::Extrait,
                    taille: taille_dossier(chemin),
                });
                it.skip_current_dir();
            }
            continue;
        }
        if !entree.file_type().is_file() || proteges.contains(chemin) {
            continue;
        }
        let (base, ext) = match nom.rsplit_once('.') {
            Some((b, e)) if !b.is_empty() => (b.to_string(), e.to_lowercase()),
            _ => (nom.clone(), String::new()),
        };
        let raison = if FICHIERS_PARASITES.contains(&nom.to_lowercase().as_str()) {
            Some(RaisonInutile::Parasite)
        } else if est_video(&ext) && est_nom_extrait(&base) {
            Some(RaisonInutile::Extrait)
        } else if let Some(m) = regles.motif_correspondant(&nom) {
            Some(RaisonInutile::Motif(m.to_string()))
        } else if ext == "nfo" {
            Some(RaisonInutile::NfoOrphelin)
        } else {
            None
        };
        if let Some(raison) = raison {
            sortie.push(Candidat {
                chemin: chemin.to_path_buf(),
                est_dossier: false,
                raison,
                taille: entree.metadata().map(|m| m.len()).unwrap_or(0),
            });
        }
    }
    Ok(sortie)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ecrire(p: &Path, c: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, c).unwrap();
    }

    fn videos() -> Vec<String> {
        vec![".mkv".into(), ".avi".into()]
    }

    #[test]
    fn mot_sample_entier() {
        assert!(est_nom_extrait("Film-sample"));
        assert!(est_nom_extrait("sample"));
        assert!(est_nom_extrait("Film.SAMPLE.1080p"));
        assert!(!est_nom_extrait("Sampler"));
        assert!(!est_nom_extrait("Example"));
    }

    #[test]
    fn detecte_les_inutiles_et_protege_le_reste() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("Film (2000)");
        let film = f.join("Film (2000).mkv");
        let nfo = f.join("Film (2000).nfo");
        ecrire(&film, "v");
        ecrire(&nfo, "<movie/>");
        ecrire(&f.join("Film (2000)-poster.jpg"), "i");
        ecrire(&f.join("Visitez-nous.URL"), "[InternetShortcut]");
        ecrire(&f.join("notes.txt"), "n");
        ecrire(&f.join("Thumbs.db"), "t");
        ecrire(&f.join("Film-sample.mkv"), "s");
        ecrire(&f.join("Sample/extrait.mkv"), "s");
        ecrire(&f.join("autre.nfo"), "<movie/>");
        ecrire(&f.join(".corbeille/vieux.txt"), "x");
        let proteges: BTreeSet<PathBuf> = [film.clone(), nfo.clone()].into();
        let r = analyser_dossier(&f, &proteges, &ReglesNettoyage::par_defaut(), &videos()).unwrap();
        let noms: Vec<String> = r
            .iter()
            .map(|c| {
                c.chemin
                    .strip_prefix(&f)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(
            noms,
            vec![
                "Film-sample.mkv",
                "Sample",
                "Thumbs.db",
                "Visitez-nous.URL",
                "autre.nfo",
                "notes.txt"
            ],
            "{r:?}"
        );
        let par_nom = |n: &str| r.iter().find(|c| c.chemin.ends_with(n)).unwrap();
        assert_eq!(par_nom("Sample").raison, RaisonInutile::Extrait);
        assert!(par_nom("Sample").est_dossier);
        assert_eq!(par_nom("Sample").taille, 1);
        assert_eq!(par_nom("autre.nfo").raison, RaisonInutile::NfoOrphelin);
        assert_eq!(par_nom("Thumbs.db").raison, RaisonInutile::Parasite);
        assert_eq!(
            par_nom("notes.txt").raison,
            RaisonInutile::Motif(r"\.txt$".into())
        );
    }

    #[test]
    fn dossier_sample_contenant_un_fichier_protege_est_conserve() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("F");
        let film = f.join("Sample/film.mkv");
        ecrire(&film, "v");
        let proteges: BTreeSet<PathBuf> = [film].into();
        let r = analyser_dossier(&f, &proteges, &ReglesNettoyage::par_defaut(), &videos()).unwrap();
        assert!(r.is_empty(), "{r:?}");
    }

    #[test]
    fn motifs_personnalises_et_invalides() {
        let r = ReglesNettoyage::nouvelles(&["(".into(), r"\.jpg\.bak$".into(), "".into()]);
        assert_eq!(r.invalides, vec!["(".to_string()]);
        let d = tempfile::tempdir().unwrap();
        ecrire(&d.path().join("a.JPG.BAK"), "x");
        ecrire(&d.path().join("b.txt"), "x");
        let c = analyser_dossier(d.path(), &BTreeSet::new(), &r, &videos()).unwrap();
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn dossier_absent_est_une_erreur() {
        assert!(
            analyser_dossier(
                Path::new("/inexistant-xyz"),
                &BTreeSet::new(),
                &ReglesNettoyage::default(),
                &[]
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn liens_symboliques_ignores() {
        let d = tempfile::tempdir().unwrap();
        let hors = d.path().join("hors");
        ecrire(&hors.join("a.txt"), "x");
        let f = d.path().join("F");
        std::fs::create_dir_all(&f).unwrap();
        std::os::unix::fs::symlink(&hors, f.join("lien")).unwrap();
        std::os::unix::fs::symlink(hors.join("a.txt"), f.join("b.txt")).unwrap();
        let c = analyser_dossier(
            &f,
            &BTreeSet::new(),
            &ReglesNettoyage::par_defaut(),
            &videos(),
        )
        .unwrap();
        assert!(c.is_empty(), "{c:?}");
    }
}
