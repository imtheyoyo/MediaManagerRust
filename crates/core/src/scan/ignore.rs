//! Dossiers et fichiers ignorés pendant le scan.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

/// Noms (en majuscules) toujours ignorés.
const NOMS_IGNORES: &[&str] = &[
    ".",
    "..",
    "CERTIFICATE",
    "$RECYCLE.BIN",
    "RECYCLER",
    "SYSTEM VOLUME INFORMATION",
    "@EADIR",
    "ADV_OBJ",
    "PLEX VERSIONS",
    "LOST.DIR",
];

/// Fichiers marqueurs excluant tout le sous-arbre du dossier qui les contient.
pub const MARQUEURS_IGNORE: &[&str] = &[".mmignore", "mmignore"];
/// Marqueur ignoré seulement si l'option `ignorer_nomedia` est active.
pub const MARQUEUR_NOMEDIA: &str = ".nomedia";

/// Motif d'exclusion par défaut.
pub const DOSSIERS_IGNORES_DEFAUT: &[&str] = &["MAKEMKV"];

/// Équivalent de `(?i)^[.@](?!45|buelos)[\w@]+.*` : nom caché ou commençant
/// par `@`, hors `.45…` et `.buelos…`.
pub fn est_nom_cache(nom: &str) -> bool {
    let mut it = nom.chars();
    match it.next() {
        Some('.') | Some('@') => {}
        _ => return false,
    }
    let reste: &str = &nom[1..];
    let bas = reste.to_lowercase();
    if bas.starts_with("45") || bas.starts_with("buelos") {
        return false;
    }
    matches!(reste.chars().next(), Some(c) if c.is_ascii_alphanumeric() || c == '_' || c == '@')
}

/// Motif d'exclusion configurable : regex sur le nom (correspondance entière)
/// ou chemin complet littéral. Un motif invalide est interprété littéralement.
#[derive(Debug, Clone)]
pub struct MotifExclusion {
    brut: String,
    regex: Regex,
}

impl MotifExclusion {
    pub fn nouveau(motif: &str) -> Self {
        let regex = Regex::new(&format!("^(?:{motif})$"))
            .unwrap_or_else(|_| Regex::new(&format!("^{}$", regex::escape(motif))).unwrap());
        Self {
            brut: motif.to_string(),
            regex,
        }
    }

    pub fn correspond(&self, nom: &str, chemin_complet: &str) -> bool {
        self.regex.is_match(nom) || self.brut == chemin_complet
    }
}

/// Vrai si l'élément (dossier ou fichier) doit être ignoré.
pub fn est_ignore(chemin: &Path, motifs: &[MotifExclusion]) -> bool {
    let nom = match chemin.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => return false,
    };
    if NOMS_IGNORES.contains(&nom.to_uppercase().as_str()) || est_nom_cache(&nom) {
        return true;
    }
    let complet = chemin.to_string_lossy();
    motifs.iter().any(|m| m.correspond(&nom, &complet))
}

/// Vrai si le dossier contient un fichier marqueur d'exclusion.
pub fn contient_marqueur(dossier: &Path, ignorer_nomedia: bool) -> bool {
    MARQUEURS_IGNORE.iter().any(|m| dossier.join(m).exists())
        || (ignorer_nomedia && dossier.join(MARQUEUR_NOMEDIA).exists())
}

static RE_DOSSIER_REGROUPEMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(\w|\d{4}|\d{4}s|\d{4}-\d{4})$").unwrap());

/// Genres usuels (noms de dossiers de classement par genre).
const GENRES_CONNUS: &[&str] = &[
    "action",
    "adventure",
    "aventure",
    "animation",
    "anime",
    "biography",
    "biographie",
    "comedy",
    "comedie",
    "comédie",
    "crime",
    "documentary",
    "documentaire",
    "drama",
    "drame",
    "family",
    "famille",
    "fantasy",
    "fantastique",
    "history",
    "histoire",
    "horror",
    "horreur",
    "music",
    "musique",
    "musical",
    "mystery",
    "mystere",
    "mystère",
    "romance",
    "science fiction",
    "science-fiction",
    "sci-fi",
    "scifi",
    "thriller",
    "tv movie",
    "war",
    "guerre",
    "western",
    "sport",
    "suspense",
    "policier",
    "spectacle",
    "concert",
];

/// Un dossier nommé comme une lettre, une année, une décennie ou un genre est
/// un dossier de regroupement (multi-films) et non un film.
pub fn est_dossier_regroupement(nom: &str) -> bool {
    RE_DOSSIER_REGROUPEMENT.is_match(nom)
        || GENRES_CONNUS.contains(&nom.trim().to_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn noms_caches() {
        assert!(est_nom_cache(".git"));
        assert!(est_nom_cache("@eaDir"));
        assert!(est_nom_cache(".hidden file"));
        assert!(est_nom_cache("@@x"));
        assert!(!est_nom_cache(".45 Calibre"));
        assert!(!est_nom_cache(".BUELOS (2019)"));
        assert!(!est_nom_cache("Film"));
        assert!(!est_nom_cache("."));
        assert!(!est_nom_cache(".-"));
    }

    #[test]
    fn liste_fixe_et_motifs() {
        let motifs = vec![
            MotifExclusion::nouveau("MAKEMKV"),
            MotifExclusion::nouveau("tmp.*"),
            MotifExclusion::nouveau("[invalide"),
        ];
        for n in [
            "$RECYCLE.BIN",
            "System Volume Information",
            "lost.dir",
            "@eadir",
            "plex versions",
            "CERTIFICATE",
            "ADV_OBJ",
        ] {
            assert!(est_ignore(&PathBuf::from("/s").join(n), &[]), "{n}");
        }
        assert!(est_ignore(Path::new("/s/MAKEMKV"), &motifs));
        assert!(!est_ignore(Path::new("/s/makemkv"), &motifs)); // casse respectée
        assert!(est_ignore(Path::new("/s/tmp123"), &motifs));
        assert!(!est_ignore(Path::new("/s/atmp"), &motifs)); // correspondance entière
        assert!(est_ignore(Path::new("/s/[invalide"), &motifs)); // littéral
        assert!(!est_ignore(Path::new("/s/.45"), &[]));
        // Chemin complet littéral.
        let m = vec![MotifExclusion::nouveau("/s/Perso/Privé")];
        assert!(est_ignore(Path::new("/s/Perso/Privé"), &m));
    }

    #[test]
    fn regroupement() {
        for n in [
            "A",
            "z",
            "2019",
            "1980s",
            "1980-1989",
            "Action",
            "Comédie",
            "science fiction",
        ] {
            assert!(est_dossier_regroupement(n), "{n}");
        }
        for n in ["Alien", "Se7en", "20191", "Film (2019)"] {
            assert!(!est_dossier_regroupement(n), "{n}");
        }
    }
}
