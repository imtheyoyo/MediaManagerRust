//! Utilitaires de texte : translittération ASCII, NFD, capitalisation.

use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

/// Normalisation Unicode NFD.
pub fn normaliser_nfd(s: &str) -> String {
    s.nfd().collect()
}

/// Translittération ASCII : décompose (NFKD), retire les diacritiques et
/// traduit quelques lettres sans décomposition (`æ`→`ae`, `œ`→`oe`, `ø`→`o`,
/// `ł`→`l`, `ß`→`ss`…), les guillemets typographiques et tirets longs.
/// Les caractères sans équivalent (CJK, etc.) sont conservés tels quels.
pub fn en_ascii(s: &str) -> String {
    let mut sortie = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() {
            sortie.push(c);
            continue;
        }
        let special = match c {
            'æ' => Some("ae"),
            'Æ' => Some("AE"),
            'œ' => Some("oe"),
            'Œ' => Some("OE"),
            'ø' => Some("o"),
            'Ø' => Some("O"),
            'ł' => Some("l"),
            'Ł' => Some("L"),
            'ß' => Some("ss"),
            'đ' | 'ð' => Some("d"),
            'Đ' | 'Ð' => Some("D"),
            'þ' => Some("th"),
            'Þ' => Some("TH"),
            'ı' => Some("i"),
            '\u{2018}' | '\u{2019}' | '\u{201B}' | '\u{00B4}' => Some("'"),
            '\u{201C}' | '\u{201D}' | '\u{201E}' => Some("\""),
            '\u{2010}'..='\u{2015}' => Some("-"),
            '\u{00A0}' | '\u{2009}' | '\u{202F}' => Some(" "),
            _ => None,
        };
        if let Some(t) = special {
            sortie.push_str(t);
            continue;
        }
        let mut une = [c].into_iter().nfkd().filter(|d| !is_combining_mark(*d));
        for d in &mut une {
            sortie.push(d);
        }
    }
    sortie
}

/// Majuscule initiale après espace, `-`, `_`, `.`, `'`, `(`, `[`, `*` ; le
/// reste est mis en minuscules ; les contractions `'s 'm 'll 't 'd 've 're`
/// restent en minuscules.
pub fn capitaliser_titre(s: &str) -> String {
    let mut sortie = String::with_capacity(s.len());
    let mut debut_mot = true;
    for c in s.chars() {
        if debut_mot {
            sortie.extend(c.to_uppercase());
        } else {
            sortie.extend(c.to_lowercase());
        }
        debut_mot = matches!(c, ' ' | '-' | '_' | '.' | '\'' | '(' | '[' | '*');
    }
    // Contractions : « Don'T » -> « Don't ».
    let mut r = String::with_capacity(sortie.len());
    let ch: Vec<char> = sortie.chars().collect();
    let mut i = 0;
    while i < ch.len() {
        r.push(ch[i]);
        if ch[i] == '\'' && i > 0 && ch[i - 1].is_alphabetic() {
            // Suffixe de 1 ou 2 lettres suivi d'une fin de mot.
            for suffixe in ["ll", "ve", "re", "s", "m", "t", "d"] {
                let l: Vec<char> = suffixe.chars().collect();
                let fin = i + 1 + l.len();
                if fin <= ch.len()
                    && ch[i + 1..fin]
                        .iter()
                        .zip(&l)
                        .all(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
                    && ch.get(fin).is_none_or(|c| !c.is_alphanumeric())
                {
                    for c in &ch[i + 1..fin] {
                        r.extend(c.to_lowercase());
                    }
                    i = fin - 1;
                    break;
                }
            }
        }
        i += 1;
    }
    r
}

/// Premier caractère après translittération : majuscule si c'est une lettre
/// ASCII, sinon `remplacement` (chiffres, symboles). Vide si `s` est vide.
pub fn premier_caractere(s: &str, remplacement: &str) -> String {
    match en_ascii(s).chars().next() {
        None => String::new(),
        Some(c) if c.is_ascii_alphabetic() => c.to_ascii_uppercase().to_string(),
        Some(_) => remplacement.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii() {
        assert_eq!(en_ascii("Léon"), "Leon");
        assert_eq!(en_ascii("Œuvre d'Æsir ø ł ß"), "OEuvre d'AEsir o l ss");
        assert_eq!(en_ascii("l’été – fin"), "l'ete - fin");
        assert_eq!(en_ascii("日本"), "日本");
    }

    #[test]
    fn titre() {
        assert_eq!(
            capitaliser_titre("the matrix RELOADED"),
            "The Matrix Reloaded"
        );
        assert_eq!(capitaliser_titre("don't stop"), "Don't Stop");
        assert_eq!(capitaliser_titre("spider-man (2002)"), "Spider-Man (2002)");
        assert_eq!(capitaliser_titre("l'amour"), "L'Amour");
        assert_eq!(capitaliser_titre("they'll go"), "They'll Go");
    }

    #[test]
    fn premier() {
        assert_eq!(premier_caractere("the matrix", "#"), "T");
        assert_eq!(premier_caractere("12 Monkeys", "#"), "#");
        assert_eq!(premier_caractere("Éclair", "#"), "E");
        assert_eq!(premier_caractere("", "#"), "");
    }

    #[test]
    fn nfd() {
        assert_eq!(normaliser_nfd("é").chars().count(), 2);
    }
}
