//! Utilitaires communs aux NFO : dates, nombres, découpage de listes,
//! extraction d'identifiants depuis du texte brut.

use std::sync::LazyLock;

use regex::Regex;

use crate::modele::Identifiants;
use crate::modele::commun::cles_id;

// ---------------------------------------------------------------------------
// Dates (calendrier grégorien proleptique, UTC)
// ---------------------------------------------------------------------------

/// Nombre de jours depuis le 1970-01-01 pour une date civile.
fn jours_depuis_civil(a: i64, m: u32, j: u32) -> i64 {
    let a = if m <= 2 { a - 1 } else { a };
    let ere = (if a >= 0 { a } else { a - 399 }) / 400;
    let ade = a - ere * 400;
    let m = m as i64;
    let jda = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + j as i64 - 1;
    let jde = ade * 365 + ade / 4 - ade / 100 + jda;
    ere * 146_097 + jde - 719_468
}

/// Date civile pour un nombre de jours depuis le 1970-01-01.
fn civil_depuis_jours(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let ere = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let jde = z - ere * 146_097;
    let ade = (jde - jde / 1460 + jde / 36_524 - jde / 146_096) / 365;
    let a = ade + ere * 400;
    let jda = jde - (365 * ade + ade / 4 - ade / 100);
    let mp = (5 * jda + 2) / 153;
    let j = (jda - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { a + 1 } else { a }, m, j)
}

/// Formate un instant (ms depuis l'époque Unix) en `aaaa-mm-jj hh:mm:ss` (UTC).
pub fn ms_vers_date_heure(ms: i64) -> String {
    let secondes = ms.div_euclid(1000);
    let jours = secondes.div_euclid(86_400);
    let reste = secondes.rem_euclid(86_400);
    let (a, m, j) = civil_depuis_jours(jours);
    format!(
        "{a:04}-{m:02}-{j:02} {:02}:{:02}:{:02}",
        reste / 3600,
        (reste % 3600) / 60,
        reste % 60
    )
}

fn date_valide(a: i64, m: u32, j: u32) -> bool {
    (1..=9999).contains(&a) && (1..=12).contains(&m) && (1..=31).contains(&j)
}

static RE_ISO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{4})[-/.](\d{1,2})[-/.](\d{1,2})(?:[ T](\d{1,2}):(\d{2})(?::(\d{2}))?)?")
        .unwrap()
});
static RE_EUROPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\d{1,2})[./-](\d{1,2})[./-](\d{4})(?:[ T](\d{1,2}):(\d{2})(?::(\d{2}))?)?")
        .unwrap()
});
static RE_COMPACTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\d{4})(\d{2})(\d{2})$").unwrap());

/// Date et heure reconnues : (année, mois, jour, heure, minute, seconde).
type DateHeure = (i64, u32, u32, u32, u32, u32);

/// Formats acceptés : `aaaa-mm-jj[ hh:mm[:ss]]` (séparateurs `-`, `/`, `.`,
/// `T`), `jj.mm.aaaa` / `jj/mm/aaaa` / `jj-mm-aaaa`, `aaaammjj`.
fn analyser_date_heure(s: &str) -> Option<DateHeure> {
    let s = s.trim();
    let nombre = |c: Option<regex::Match>| c.map_or(0, |m| m.as_str().parse::<u32>().unwrap_or(0));
    let r = if let Some(c) = RE_ISO.captures(s) {
        let a = c[1].parse().ok()?;
        (
            a,
            nombre(c.get(2)),
            nombre(c.get(3)),
            nombre(c.get(4)),
            nombre(c.get(5)),
            nombre(c.get(6)),
        )
    } else if let Some(c) = RE_EUROPE.captures(s) {
        let a = c[3].parse().ok()?;
        (
            a,
            nombre(c.get(2)),
            nombre(c.get(1)),
            nombre(c.get(4)),
            nombre(c.get(5)),
            nombre(c.get(6)),
        )
    } else {
        let c = RE_COMPACTE.captures(s)?;
        (
            c[1].parse().ok()?,
            nombre(c.get(2)),
            nombre(c.get(3)),
            0,
            0,
            0,
        )
    };
    if !date_valide(r.0, r.1, r.2) || r.3 > 23 || r.4 > 59 || r.5 > 59 {
        return None;
    }
    Some(r)
}

/// Normalise une date en `aaaa-mm-jj` (`None` si non reconnue).
pub fn normaliser_date(s: &str) -> Option<String> {
    analyser_date_heure(s).map(|(a, m, j, ..)| format!("{a:04}-{m:02}-{j:02}"))
}

/// Convertit une date (avec ou sans heure, UTC) en ms depuis l'époque Unix.
pub fn date_vers_ms(s: &str) -> Option<i64> {
    let (a, m, j, h, mi, se) = analyser_date_heure(s)?;
    let jours = jours_depuis_civil(a, m, j);
    Some(((jours * 86_400) + (h as i64) * 3600 + (mi as i64) * 60 + se as i64) * 1000)
}

/// Année d'une date `aaaa-…` (0 si non reconnue).
pub fn annee_de_date(s: &str) -> u32 {
    analyser_date_heure(s).map_or(0, |(a, ..)| a as u32)
}

/// Horodatage courant pour le commentaire d'en-tête.
pub fn horodatage_courant() -> String {
    ms_vers_date_heure(crate::modele::film::maintenant_ms())
}

// ---------------------------------------------------------------------------
// Nombres et textes
// ---------------------------------------------------------------------------

/// Entier en tête de texte (`"120 min"` ⇒ 120).
pub fn entier_en_tete(s: &str) -> Option<i64> {
    let s = s.trim();
    let (signe, corps) = match s.strip_prefix('-') {
        Some(r) => (-1, r),
        None => (1, s),
    };
    let chiffres: String = corps.chars().take_while(char::is_ascii_digit).collect();
    chiffres.parse::<i64>().ok().map(|n| n * signe)
}

/// Nombre décimal (virgule acceptée comme séparateur).
pub fn decimal(s: &str) -> Option<f32> {
    s.trim()
        .replace(',', ".")
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
}

/// Nombre de votes : seuls les chiffres comptent (`"1,234"` ⇒ 1234).
pub fn votes(s: &str) -> u32 {
    let chiffres: String = s.chars().filter(char::is_ascii_digit).collect();
    chiffres.parse().unwrap_or(0)
}

/// Booléen NFO (`true`, `1`, `yes`).
pub fn booleen(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "oui"
    )
}

/// Note au format `%.1f`.
pub fn format_note(v: f32) -> String {
    format!("{v:.1}")
}

/// Note au format `#.#` (`8.0` ⇒ `8`).
pub fn format_note_courte(v: f32) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

/// Découpe une liste écrite dans une seule balise (`a / b, c | d`).
pub fn decouper_liste(s: &str) -> Vec<String> {
    s.split(['/', ',', '|', ';'])
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(str::to_string)
        .collect()
}

/// Résumé court : texte entier, ou premières phrases jusqu'à au moins 20
/// caractères si `premiere_phrase`.
pub fn resume_court(texte: &str, premiere_phrase: bool) -> String {
    if !premiere_phrase {
        return texte.to_string();
    }
    let mut resultat = String::new();
    let mut courant = String::new();
    for c in texte.chars() {
        courant.push(c);
        if matches!(c, '.' | '!' | '?') {
            resultat.push_str(&courant);
            courant.clear();
            if resultat.trim().chars().count() >= 20 {
                return resultat.trim().to_string();
            }
        }
    }
    resultat.push_str(&courant);
    resultat.trim().to_string()
}

// ---------------------------------------------------------------------------
// Identifiants dans du texte brut
// ---------------------------------------------------------------------------

static RE_IMDB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"tt\d{6,}").unwrap());
static RE_TMDB_FILM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)themoviedb\.org/movie/(\d+)").unwrap());
static RE_TMDB_SERIE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)themoviedb\.org/tv/(\d+)").unwrap());
static RE_TVDB: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)thetvdb\.com/(?:\?tab=series&id=|series/|dereferrer/series/|\?id=)(\d+)")
        .unwrap()
});

/// Type de média recherché dans un texte brut (choix de l'URL TMDB).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenreMedia {
    Film,
    Serie,
}

/// Extrait les identifiants imdb / tmdb / tvdb d'un texte quelconque (URL,
/// ID seul, NFO non XML). Seuls les identifiants absents sont complétés.
pub fn completer_ids_depuis_texte(ids: &mut Identifiants, texte: &str, genre: GenreMedia) {
    if let Some(m) = RE_IMDB.find(texte) {
        ids.definir_si_absent(cles_id::IMDB, m.as_str());
    }
    let re_tmdb = match genre {
        GenreMedia::Film => &*RE_TMDB_FILM,
        GenreMedia::Serie => &*RE_TMDB_SERIE,
    };
    if let Some(c) = re_tmdb.captures(texte) {
        ids.definir_si_absent(cles_id::TMDB, &c[1]);
    }
    if let Some(c) = RE_TVDB.captures(texte) {
        ids.definir_si_absent(cles_id::TVDB, &c[1]);
    }
}

/// Identifiants trouvés dans un texte brut.
pub fn ids_depuis_texte(texte: &str, genre: GenreMedia) -> Identifiants {
    let mut ids = Identifiants::new();
    completer_ids_depuis_texte(&mut ids, texte, genre);
    ids
}

// ---------------------------------------------------------------------------
// Bandes-annonces YouTube
// ---------------------------------------------------------------------------

static RE_YOUTUBE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:youtube\.com/watch\?(?:.*&)?v=|youtu\.be/|youtube\.com/embed/)([\w-]{11})")
        .unwrap()
});
static RE_YOUTUBE_EXTENSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^plugin://plugin\.video\.youtube/(?:play/\?video_id=|\?action=play_video&videoid=)([\w-]+)")
        .unwrap()
});

/// Format d'URL de bande-annonce attendu par le lecteur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatBandeAnnonce {
    /// `plugin://plugin.video.youtube/play/?video_id=ID`
    Moderne,
    /// `plugin://plugin.video.youtube/?action=play_video&videoid=ID`
    Ancien,
}

/// Convertit une URL YouTube en URL d'extension du lecteur (autres URL inchangées).
pub fn url_bande_annonce_vers_nfo(url: &str, format: FormatBandeAnnonce) -> String {
    match RE_YOUTUBE.captures(url) {
        Some(c) => match format {
            FormatBandeAnnonce::Moderne => {
                format!("plugin://plugin.video.youtube/play/?video_id={}", &c[1])
            }
            FormatBandeAnnonce::Ancien => {
                format!(
                    "plugin://plugin.video.youtube/?action=play_video&videoid={}",
                    &c[1]
                )
            }
        },
        None => url.to_string(),
    }
}

/// Reconvertit une URL d'extension YouTube en URL web (autres URL inchangées).
pub fn url_bande_annonce_depuis_nfo(url: &str) -> String {
    match RE_YOUTUBE_EXTENSION.captures(url.trim()) {
        Some(c) => format!("https://www.youtube.com/watch?v={}", &c[1]),
        None => url.trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(ms_vers_date_heure(0), "1970-01-01 00:00:00");
        let ms = date_vers_ms("2021-03-04 05:06:07").unwrap();
        assert_eq!(ms_vers_date_heure(ms), "2021-03-04 05:06:07");
        assert_eq!(date_vers_ms("1969-12-31"), Some(-86_400_000));
        assert_eq!(normaliser_date("4.3.2021").as_deref(), Some("2021-03-04"));
        assert_eq!(normaliser_date("2021/3/4").as_deref(), Some("2021-03-04"));
        assert_eq!(normaliser_date("20210304").as_deref(), Some("2021-03-04"));
        assert_eq!(
            normaliser_date("2021-03-04T10:00:00Z").as_deref(),
            Some("2021-03-04")
        );
        assert_eq!(normaliser_date("n'importe quoi"), None);
        assert_eq!(normaliser_date("2021-13-01"), None);
        assert_eq!(annee_de_date("1999-05-01"), 1999);
    }

    #[test]
    fn nombres() {
        assert_eq!(entier_en_tete(" 120 min"), Some(120));
        assert_eq!(entier_en_tete("-1"), Some(-1));
        assert_eq!(decimal("7,5"), Some(7.5));
        assert_eq!(votes("1,234,567"), 1_234_567);
        assert_eq!(format_note_courte(8.0), "8");
        assert_eq!(format_note_courte(7.46), "7.5");
    }

    #[test]
    fn resume() {
        let t = "Court. Une deuxième phrase longue. Trois.";
        assert_eq!(resume_court(t, false), t);
        assert_eq!(resume_court(t, true), "Court. Une deuxième phrase longue.");
    }

    #[test]
    fn ids_texte() {
        let ids = ids_depuis_texte(
            "https://www.imdb.com/title/tt0133093/ https://www.themoviedb.org/movie/603-the-matrix",
            GenreMedia::Film,
        );
        assert_eq!(ids.imdb(), "tt0133093");
        assert_eq!(ids.tmdb(), 603);
        let ids = ids_depuis_texte(
            "https://thetvdb.com/?tab=series&id=81189",
            GenreMedia::Serie,
        );
        assert_eq!(ids.tvdb(), 81189);
    }

    #[test]
    fn bandes_annonces() {
        let u = "https://www.youtube.com/watch?v=vKQi3bBA1y8";
        let k = url_bande_annonce_vers_nfo(u, FormatBandeAnnonce::Moderne);
        assert_eq!(
            k,
            "plugin://plugin.video.youtube/play/?video_id=vKQi3bBA1y8"
        );
        assert_eq!(url_bande_annonce_depuis_nfo(&k), u);
        let x = url_bande_annonce_vers_nfo(u, FormatBandeAnnonce::Ancien);
        assert_eq!(url_bande_annonce_depuis_nfo(&x), u);
        assert_eq!(
            url_bande_annonce_vers_nfo("http://a/b.mp4", FormatBandeAnnonce::Moderne),
            "http://a/b.mp4"
        );
    }
}
