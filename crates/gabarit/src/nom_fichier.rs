//! Sanitisation de noms de fichiers : fonctions indépendantes du moteur,
//! utilisables directement par le renommeur.

pub use crate::texte::en_ascii;

/// Plateforme cible pour l'assainissement des noms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plateforme {
    Windows,
    Unix,
}

impl Plateforme {
    /// Plateforme de compilation.
    pub fn courante() -> Self {
        if cfg!(windows) {
            Plateforme::Windows
        } else {
            Plateforme::Unix
        }
    }
}

/// Caractères interdits dans un nom de fichier Windows.
pub const CARACTERES_INTERDITS_WINDOWS: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Caractères supprimés par le renommeur (`"`, `:`, `<`, `>`, `|`, `?`, `*`).
const CARACTERES_SUPPRIMES: [char; 7] = ['"', ':', '<', '>', '|', '?', '*'];

const NOMS_RESERVES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Remplace `:` puis supprime `" : < > | ? *`.
///
/// Si `remplacement_deux_points` vaut `"-"`, `": "` devient `" - "` et `":"`
/// devient `"-"` ; sinon `":"` est remplacé par la valeur donnée.
pub fn remplacer_caracteres_invalides(s: &str, remplacement_deux_points: &str) -> String {
    let t = if remplacement_deux_points == "-" {
        s.replace(": ", " - ").replace(':', "-")
    } else {
        s.replace(':', remplacement_deux_points)
    };
    t.chars()
        .filter(|c| !CARACTERES_SUPPRIMES.contains(c))
        .collect()
}

/// Remplace les caractères interdits par des équivalents Unicode visuellement
/// proches (`:`→`꞉`, `"`→`″`, `'`→`’`, `*`→`∗`, `<`→`‹`, `>`→`›`, `?`→`？`).
pub fn equivalents_unicode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '"' => '\u{2033}',
            '\'' => '\u{2019}',
            '*' => '\u{2217}',
            ':' => '\u{A789}',
            '<' => '\u{2039}',
            '>' => '\u{203A}',
            '?' => '\u{FF1F}',
            autre => autre,
        })
        .collect()
}

/// Remplace chaque espace par `remplacement` puis fusionne les répétitions
/// de ce remplacement.
pub fn remplacer_espaces(s: &str, remplacement: &str) -> String {
    let t = s.replace(' ', remplacement);
    if remplacement.is_empty() {
        return t;
    }
    let mut sortie = String::with_capacity(t.len());
    let mut reste = t.as_str();
    while let Some(i) = reste.find(remplacement) {
        sortie.push_str(&reste[..i + remplacement.len()]);
        reste = &reste[i + remplacement.len()..];
        while let Some(r) = reste.strip_prefix(remplacement) {
            reste = r;
        }
    }
    sortie.push_str(reste);
    sortie
}

/// Vrai si le nom (extension ignorée) est un nom réservé Windows
/// (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`).
pub fn est_nom_reserve_windows(nom: &str) -> bool {
    let radical = nom.split('.').next().unwrap_or("").trim_end();
    NOMS_RESERVES
        .iter()
        .any(|r| r.eq_ignore_ascii_case(radical))
}

fn tronquer_utf16(s: &str, max: usize) -> String {
    let mut n = 0;
    let mut sortie = String::new();
    for c in s.chars() {
        n += c.len_utf16();
        if n > max {
            break;
        }
        sortie.push(c);
    }
    sortie
}

fn tronquer_octets(s: &str, max: usize) -> String {
    let mut fin = s.len().min(max);
    while !s.is_char_boundary(fin) {
        fin -= 1;
    }
    s[..fin].to_string()
}

/// Assainit un NOM (un seul composant, sans séparateur) pour Windows :
/// caractères interdits et de contrôle remplacés, points et espaces finaux
/// retirés, nom réservé suffixé par `_`, longueur limitée à 255 unités UTF-16.
/// Un résultat vide devient `_`.
pub fn assainir_nom_fichier_windows(nom: &str, remplacement: &str) -> String {
    let t: String = nom
        .chars()
        .map(|c| {
            if CARACTERES_INTERDITS_WINDOWS.contains(&c) || c.is_control() {
                remplacement.to_string()
            } else {
                c.to_string()
            }
        })
        .collect();
    let t = t.trim_end_matches(['.', ' ']);
    let mut t = tronquer_utf16(t, 255)
        .trim_end_matches(['.', ' '])
        .to_string();
    if est_nom_reserve_windows(&t) {
        let i = t.find('.').unwrap_or(t.len());
        let (radical, ext) = t.split_at(i);
        t = format!("{}_{ext}", radical.trim_end());
    }
    if t.is_empty() { "_".to_string() } else { t }
}

/// Assainit un NOM pour Unix : `/` et NUL remplacés, `.` et `..` neutralisés,
/// longueur limitée à 255 octets. Un résultat vide devient `_`.
pub fn assainir_nom_fichier_unix(nom: &str, remplacement: &str) -> String {
    let t: String = nom
        .chars()
        .map(|c| {
            if c == '/' || c == '\0' {
                remplacement.to_string()
            } else {
                c.to_string()
            }
        })
        .collect();
    let t = tronquer_octets(&t, 255);
    if t.is_empty() || t == "." || t == ".." {
        "_".to_string()
    } else {
        t
    }
}

/// Assainit un nom selon la plateforme (remplacement par défaut : `_`).
pub fn assainir_nom_fichier(nom: &str, plateforme: Plateforme) -> String {
    match plateforme {
        Plateforme::Windows => assainir_nom_fichier_windows(nom, "_"),
        Plateforme::Unix => assainir_nom_fichier_unix(nom, "_"),
    }
}

/// Assainit chaque composant d'un chemin relatif séparé par `/` (ou `\`) ;
/// les composants vides sont écartés. Renvoie un chemin séparé par `/`.
pub fn assainir_chemin_relatif(chemin: &str, plateforme: Plateforme) -> String {
    chemin
        .split(['/', '\\'])
        .filter(|c| !c.is_empty())
        .map(|c| assainir_nom_fichier(c, plateforme))
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deux_points() {
        assert_eq!(remplacer_caracteres_invalides("A: B", "-"), "A - B");
        assert_eq!(remplacer_caracteres_invalides("A:B", "-"), "A-B");
        assert_eq!(remplacer_caracteres_invalides("A:B", ""), "AB");
        assert_eq!(remplacer_caracteres_invalides("A:B", "_"), "A_B");
        assert_eq!(remplacer_caracteres_invalides("a<b>|c?*\"d", "-"), "abcd");
    }

    #[test]
    fn reserves() {
        assert!(est_nom_reserve_windows("con"));
        assert!(est_nom_reserve_windows("NUL.txt"));
        assert!(est_nom_reserve_windows("com1.tar.gz"));
        assert!(!est_nom_reserve_windows("console"));
        assert!(!est_nom_reserve_windows("COM0"));
        assert_eq!(assainir_nom_fichier_windows("CON", "_"), "CON_");
        assert_eq!(assainir_nom_fichier_windows("aux.txt", "_"), "aux_.txt");
    }

    #[test]
    fn windows() {
        assert_eq!(assainir_nom_fichier_windows("a<b>c", "_"), "a_b_c");
        assert_eq!(
            assainir_nom_fichier_windows("a/b\\c|d?e*f:g\"h", ""),
            "abcdefgh"
        );
        assert_eq!(assainir_nom_fichier_windows("fin. . ", "_"), "fin");
        assert_eq!(assainir_nom_fichier_windows("...", "_"), "_");
        assert_eq!(assainir_nom_fichier_windows("", "_"), "_");
        assert_eq!(assainir_nom_fichier_windows("a\u{1}b", "_"), "a_b");
        assert_eq!(
            assainir_nom_fichier_windows(&"é".repeat(300), "_")
                .chars()
                .count(),
            255
        );
    }

    #[test]
    fn unix() {
        assert_eq!(assainir_nom_fichier_unix("a/b", "_"), "a_b");
        assert_eq!(assainir_nom_fichier_unix("..", "_"), "_");
        assert_eq!(assainir_nom_fichier_unix("", "_"), "_");
        assert_eq!(assainir_nom_fichier_unix("a:b*", "_"), "a:b*");
        let long = assainir_nom_fichier_unix(&"é".repeat(300), "_");
        assert!(long.len() <= 255 && long.chars().all(|c| c == 'é'));
    }

    #[test]
    fn chemin() {
        assert_eq!(
            assainir_chemin_relatif("a//CON/b.", Plateforme::Windows),
            "a/CON_/b"
        );
        assert_eq!(assainir_chemin_relatif("a\\b", Plateforme::Unix), "a/b");
    }

    #[test]
    fn espaces_unicode() {
        assert_eq!(remplacer_espaces("a  b c", "_"), "a_b_c");
        assert_eq!(remplacer_espaces("a b", ".."), "a..b");
        assert_eq!(remplacer_espaces("a b", ""), "ab");
        assert_eq!(equivalents_unicode("a:b"), "a\u{A789}b");
    }
}
