//! Codes de langue des sous-titres et leur écriture selon un style.

use serde::{Deserialize, Serialize};

/// Écriture du code de langue dans le nom d'un sous-titre.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleLangue {
    /// `fr`
    Iso2,
    /// `fra` (terminologique, défaut)
    #[default]
    Iso3T,
    /// `fre` (bibliographique)
    Iso3B,
    /// `French`
    NomAnglais,
    /// `français`
    NomLocalise,
    /// Code tel que trouvé, sans conversion.
    Original,
}

/// (ISO 639-1, ISO 639-2/T, ISO 639-2/B, nom anglais, nom français)
const LANGUES: &[(&str, &str, &str, &str, &str)] = &[
    ("fr", "fra", "fre", "French", "français"),
    ("en", "eng", "eng", "English", "anglais"),
    ("de", "deu", "ger", "German", "allemand"),
    ("es", "spa", "spa", "Spanish", "espagnol"),
    ("it", "ita", "ita", "Italian", "italien"),
    ("pt", "por", "por", "Portuguese", "portugais"),
    ("nl", "nld", "dut", "Dutch", "néerlandais"),
    ("ru", "rus", "rus", "Russian", "russe"),
    ("ja", "jpn", "jpn", "Japanese", "japonais"),
    ("zh", "zho", "chi", "Chinese", "chinois"),
    ("ko", "kor", "kor", "Korean", "coréen"),
    ("ar", "ara", "ara", "Arabic", "arabe"),
    ("sv", "swe", "swe", "Swedish", "suédois"),
    ("no", "nor", "nor", "Norwegian", "norvégien"),
    ("da", "dan", "dan", "Danish", "danois"),
    ("fi", "fin", "fin", "Finnish", "finnois"),
    ("pl", "pol", "pol", "Polish", "polonais"),
    ("cs", "ces", "cze", "Czech", "tchèque"),
    ("hu", "hun", "hun", "Hungarian", "hongrois"),
    ("el", "ell", "gre", "Greek", "grec"),
    ("tr", "tur", "tur", "Turkish", "turc"),
    ("he", "heb", "heb", "Hebrew", "hébreu"),
    ("hi", "hin", "hin", "Hindi", "hindi"),
    ("th", "tha", "tha", "Thai", "thaï"),
    ("ro", "ron", "rum", "Romanian", "roumain"),
    ("bg", "bul", "bul", "Bulgarian", "bulgare"),
    ("uk", "ukr", "ukr", "Ukrainian", "ukrainien"),
    ("hr", "hrv", "hrv", "Croatian", "croate"),
    ("sr", "srp", "srp", "Serbian", "serbe"),
    ("sk", "slk", "slo", "Slovak", "slovaque"),
    ("sl", "slv", "slv", "Slovenian", "slovène"),
    ("ca", "cat", "cat", "Catalan", "catalan"),
    ("eu", "eus", "baq", "Basque", "basque"),
    ("vi", "vie", "vie", "Vietnamese", "vietnamien"),
    ("id", "ind", "ind", "Indonesian", "indonésien"),
    ("ms", "msa", "may", "Malay", "malais"),
    ("fa", "fas", "per", "Persian", "persan"),
    ("et", "est", "est", "Estonian", "estonien"),
    ("lv", "lav", "lav", "Latvian", "letton"),
    ("lt", "lit", "lit", "Lithuanian", "lituanien"),
    ("is", "isl", "ice", "Icelandic", "islandais"),
    ("ga", "gle", "gle", "Irish", "irlandais"),
    ("cy", "cym", "wel", "Welsh", "gallois"),
    ("la", "lat", "lat", "Latin", "latin"),
];

/// Marqueurs de sous-titres reconnus (hors langue).
pub const DRAPEAUX_SOUS_TITRES: &[&str] = &["forced", "sdh", "cc", "hi", "default"];

fn chercher(
    code: &str,
) -> Option<&'static (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
)> {
    let c = code.trim();
    if c.is_empty() {
        return None;
    }
    LANGUES.iter().find(|l| {
        l.0.eq_ignore_ascii_case(c)
            || l.1.eq_ignore_ascii_case(c)
            || l.2.eq_ignore_ascii_case(c)
            || l.3.eq_ignore_ascii_case(c)
            || l.4.to_lowercase() == c.to_lowercase()
    })
}

/// Vrai si `code` désigne une langue connue (code ISO ou nom).
pub fn est_langue_connue(code: &str) -> bool {
    chercher(code).is_some()
        || code
            .split(['-', '_'])
            .next()
            .is_some_and(|b| b != code && chercher(b).is_some())
}

/// Écrit un code de langue selon le style. Un code inconnu est renvoyé tel
/// quel ; une variante régionale (`pt-BR`) conserve son suffixe.
pub fn ecrire_langue(code: &str, style: StyleLangue) -> String {
    let code = code.trim();
    if style == StyleLangue::Original {
        return code.to_string();
    }
    let convertir = |l: &(&str, &str, &str, &str, &str)| -> String {
        match style {
            StyleLangue::Iso2 => l.0,
            StyleLangue::Iso3T => l.1,
            StyleLangue::Iso3B => l.2,
            StyleLangue::NomAnglais => l.3,
            StyleLangue::NomLocalise => l.4,
            StyleLangue::Original => unreachable!(),
        }
        .to_string()
    };
    if let Some(l) = chercher(code) {
        return convertir(l);
    }
    if let Some((base, region)) = code.split_once(['-', '_'])
        && let Some(l) = chercher(base)
    {
        return format!("{}-{}", convertir(l), region);
    }
    code.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles() {
        assert_eq!(ecrire_langue("fr", StyleLangue::Iso3T), "fra");
        assert_eq!(ecrire_langue("fre", StyleLangue::Iso2), "fr");
        assert_eq!(ecrire_langue("French", StyleLangue::Iso3B), "fre");
        assert_eq!(ecrire_langue("deu", StyleLangue::NomAnglais), "German");
        assert_eq!(ecrire_langue("en", StyleLangue::NomLocalise), "anglais");
        assert_eq!(ecrire_langue("fr", StyleLangue::Original), "fr");
        assert_eq!(ecrire_langue("pt-BR", StyleLangue::Iso3T), "por-BR");
        assert_eq!(ecrire_langue("xx", StyleLangue::Iso3T), "xx");
        assert!(est_langue_connue("FRA"));
        assert!(est_langue_connue("pt-BR"));
        assert!(!est_langue_connue("forced"));
    }
}
