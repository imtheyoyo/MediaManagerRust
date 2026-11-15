//! Détection des fichiers empilés (CD1/CD2, part1/part2, a/b, 1 of 2).

use std::sync::LazyLock;

use regex::Regex;

fn re(motif: &str) -> Regex {
    // Équivalent des `matches` Java : correspondance sur toute la chaîne.
    Regex::new(&format!("(?i)^(?:{motif})$")).unwrap()
}

static P1: LazyLock<Regex> =
    LazyLock::new(|| re(r"(.*)[ _.-]+((?:cd|dvd|p(?:ar)?t|dis[ck])[1-9][0-9]?)([ _.-].+)"));
static P1A: LazyLock<Regex> =
    LazyLock::new(|| re(r"((?:cd|dvd|p(?:ar)?t|dis[ck])[1-9][0-9]?)([ _.-].+)"));
static P2: LazyLock<Regex> =
    LazyLock::new(|| re(r"(.*)[ _.-]+((?:cd|dvd|p(?:ar)?t|dis[ck])[a-d])([ _.-].+)"));
static P3: LazyLock<Regex> = LazyLock::new(|| re(r"(.*?)[_.-]+([a-d])(\.[^.]+)"));
static P4: LazyLock<Regex> = LazyLock::new(|| {
    re(r"(.*?)[ (_.-]+([1-9][0-9]?[ .]?(?:of|⧸|∕|/)[ .]?[1-9][0-9]?)[ )_-]?([ _.-].+)")
});
static P_DOSSIER: LazyLock<Regex> =
    LazyLock::new(|| re(r"(.*?)[ _.-]*((?:cd|dvd|p(?:ar)?t|dis[ck])[1-9][0-9]?)"));

/// Marqueur de pile d'un **nom de fichier** (avec extension), ex. `CD1`.
/// Chaîne vide si aucun.
pub fn marqueur_pile(nom_fichier: &str) -> String {
    for (r, groupe) in [(&*P1, 2), (&*P1A, 1), (&*P2, 2), (&*P3, 2), (&*P4, 2)] {
        if let Some(c) = r.captures(nom_fichier) {
            return c[groupe].to_string();
        }
    }
    String::new()
}

/// Nom de fichier débarrassé de son marqueur de pile (extension conservée).
pub fn nettoyer_marqueurs_pile(nom_fichier: &str) -> String {
    for r in [&*P1, &*P2, &*P3, &*P4] {
        if let Some(c) = r.captures(nom_fichier) {
            return format!("{}{}", &c[1], &c[3]);
        }
    }
    if let Some(c) = P1A.captures(nom_fichier) {
        return c[2].trim_start_matches([' ', '_', '.', '-']).to_string();
    }
    nom_fichier.to_string()
}

/// Comme [`nettoyer_marqueurs_pile`] mais pour un nom de base (sans extension) :
/// un marqueur placé en fin de nom est retiré avec son séparateur.
pub fn nettoyer_marqueur_pile(nom_de_base: &str) -> String {
    // On ajoute une extension factice pour réutiliser les motifs de fichier.
    let factice = format!("{nom_de_base}.xyz");
    let nettoye = nettoyer_marqueurs_pile(&factice);
    nettoye.strip_suffix(".xyz").unwrap_or(&nettoye).to_string()
}

/// Numéro de pile d'un marqueur : a→1 … d→4, `1of2`→1, sinon les chiffres ;
/// 0 si aucun.
pub fn numero_pile(marqueur: &str) -> u32 {
    let m = marqueur.trim().to_lowercase();
    if m.is_empty() {
        return 0;
    }
    if !m.chars().any(|c| c.is_ascii_digit()) {
        // Marqueurs alphabétiques : `a`, `cda`, `partb`… (la dernière lettre compte).
        return match m.chars().last() {
            Some('a') => 1,
            Some('b') => 2,
            Some('c') => 3,
            Some('d') => 4,
            _ => 0,
        };
    }
    // « 1 of 2 », « 1of2 » : premier nombre ; « cd2 » : ses chiffres.
    let chiffres: String = m
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    chiffres.parse().unwrap_or(0)
}

/// Marqueur de pile d'un nom de dossier (`Film CD1` ⇒ `CD1`), vide sinon.
pub fn marqueur_pile_dossier(nom_dossier: &str) -> String {
    P_DOSSIER
        .captures(nom_dossier)
        .map(|c| c[2].to_string())
        .unwrap_or_default()
}

/// Vrai si le nom de dossier se termine par un marqueur de pile.
pub fn est_dossier_de_pile(nom_dossier: &str) -> bool {
    P_DOSSIER.is_match(nom_dossier)
}

/// Numéro et marqueur d'un fichier : du nom de fichier, sinon du dossier parent.
pub fn detecter_empilement(nom_fichier: &str, nom_dossier_parent: &str) -> (u32, String) {
    let m = marqueur_pile(nom_fichier);
    if !m.is_empty() {
        return (numero_pile(&m), m);
    }
    let m = marqueur_pile_dossier(nom_dossier_parent);
    if !m.is_empty() {
        return (numero_pile(&m), m);
    }
    (0, String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marqueurs_de_fichier() {
        assert_eq!(marqueur_pile("Film.CD1.mkv"), "CD1");
        assert_eq!(marqueur_pile("Film cd2.avi"), "cd2");
        assert_eq!(marqueur_pile("Film-part3.mkv"), "part3");
        assert_eq!(marqueur_pile("Film pt2.mkv"), "pt2");
        assert_eq!(marqueur_pile("Film Disc1.mkv"), "Disc1");
        assert_eq!(marqueur_pile("CD1 Film.mkv"), "CD1");
        assert_eq!(marqueur_pile("Film-a.mkv"), "a");
        assert_eq!(marqueur_pile("Film_b.avi"), "b");
        assert_eq!(marqueur_pile("Film (1 of 2).mkv"), "1 of 2");
        assert_eq!(marqueur_pile("Film cda.mkv"), "cda");
        assert_eq!(marqueur_pile("Film.mkv"), "");
        assert_eq!(marqueur_pile("Film 2019.mkv"), "");
    }

    #[test]
    fn nettoyage_des_marqueurs() {
        assert_eq!(nettoyer_marqueurs_pile("Film.CD1.mkv"), "Film.mkv");
        assert_eq!(nettoyer_marqueurs_pile("Film cd2.avi"), "Film.avi");
        assert_eq!(nettoyer_marqueurs_pile("Film-a.mkv"), "Film.mkv");
        assert_eq!(nettoyer_marqueurs_pile("Film (1 of 2).mkv"), "Film.mkv");
        assert_eq!(nettoyer_marqueurs_pile("CD1 Film.mkv"), "Film.mkv");
        assert_eq!(nettoyer_marqueurs_pile("Film.mkv"), "Film.mkv");
        assert_eq!(nettoyer_marqueur_pile("Film CD1"), "Film");
        assert_eq!(nettoyer_marqueur_pile("Film"), "Film");
    }

    #[test]
    fn numeros() {
        assert_eq!(numero_pile("CD1"), 1);
        assert_eq!(numero_pile("cd12"), 12);
        assert_eq!(numero_pile("part3"), 3);
        assert_eq!(numero_pile("a"), 1);
        assert_eq!(numero_pile("d"), 4);
        assert_eq!(numero_pile("cdb"), 2);
        assert_eq!(numero_pile("1 of 2"), 1);
        assert_eq!(numero_pile("2of3"), 2);
        assert_eq!(numero_pile(""), 0);
    }

    #[test]
    fn dossiers_de_pile() {
        assert!(est_dossier_de_pile("CD1"));
        assert!(est_dossier_de_pile("Film CD2"));
        assert!(est_dossier_de_pile("Film.disc1"));
        assert!(!est_dossier_de_pile("Film"));
        assert_eq!(marqueur_pile_dossier("Film CD2"), "CD2");
    }

    #[test]
    fn empilement_depuis_le_dossier() {
        assert_eq!(detecter_empilement("Film.avi", "CD2"), (2, "CD2".into()));
        assert_eq!(
            detecter_empilement("Film cd1.avi", "CD2"),
            (1, "cd1".into())
        );
        assert_eq!(detecter_empilement("Film.avi", "Film"), (0, String::new()));
    }
}
