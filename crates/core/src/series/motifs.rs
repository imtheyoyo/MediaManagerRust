//! Expressions rationnelles de la détection saison / épisode.
//!
//! Les motifs sont repris de la spécification (§3.2), en `(?i)` (insensibles
//! à la casse) sauf mention. `fancy-regex` est nécessaire pour les lookahead.
//! Écarts volontaires, signalés au cas par cas : quelques gardes de début de
//! mot (`(?<![a-z])`, `\b`) pour éviter des faux positifs au milieu d'un mot.

use std::sync::LazyLock;

use fancy_regex::{Captures, Regex};

pub(crate) fn rx(motif: &str) -> Regex {
    Regex::new(motif).unwrap_or_else(|e| panic!("motif invalide {motif:?} : {e}"))
}

/// Captures d'une recherche (une erreur d'exécution, ex. limite de retour
/// arrière, équivaut à « pas de correspondance »).
pub(crate) fn captures<'t>(re: &Regex, texte: &'t str) -> Option<Captures<'t>> {
    re.captures(texte).ok().flatten()
}

/// Groupe `i` converti en entier.
pub(crate) fn groupe_entier(c: &Captures<'_>, i: usize) -> Option<i32> {
    c.get(i).and_then(|m| m.as_str().parse().ok())
}

// ---------------------------------------------------------------------------
// Dates
// ---------------------------------------------------------------------------

pub(crate) static DATE_1: LazyLock<Regex> =
    LazyLock::new(|| rx(r"([0-9]{4})[.-]([0-9]{2})[.-]([0-9]{2})"));
pub(crate) static DATE_2: LazyLock<Regex> =
    LazyLock::new(|| rx(r"([0-9]{2})[.-]([0-9]{2})[.-]([0-9]{4})"));

// ---------------------------------------------------------------------------
// Saisons et épisodes
// ---------------------------------------------------------------------------

/// Mots « saison » dans plusieurs langues (ordre : les plus longs d'abord en
/// cas de préfixe commun, pour que l'alternance choisisse le bon).
pub(crate) const MOTS_SAISON: &[&str] = &[
    "series",
    "season",
    "الموسم",
    "sezóna",
    "sæson",
    "staffel",
    "σεζόν",
    "temporada",
    "فصل",
    "kausi",
    "saison",
    "sezona",
    "évad",
    "þáttaröð",
    "stagione",
    "시즌",
    "seizoen",
    "sesong",
    "сезона",
    "сезон",
    "sezon",
    "säsong",
    "சீசன்",
];

pub(crate) static SEASON_LONG: LazyLock<Regex> = LazyLock::new(|| {
    rx(&format!(
        r"(?i)({})[\s_.-]?(\d{{1,4}})",
        MOTS_SAISON.join("|")
    ))
});
pub(crate) static SEASON_ONLY: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(?i)[\s_.-]s[\s_.-]?(\d{1,4})"));
pub(crate) static EPISODE_ONLY: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(?i)[\s_.-]ep?[\s_.-]?(\d{1,4})"));
pub(crate) static EPISODE_PATTERN: LazyLock<Regex> = LazyLock::new(|| rx(r"(?i)[epx_-]+(\d{1,4})"));
/// Écart : garde `\b` en tête (sinon « Creep 2 » donnerait l'épisode 2).
pub(crate) static EPISODE_PATTERN_2: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(?i)\b(?:episode|ep)[\. _-]*(\d{1,4})"));
pub(crate) static EPISODE_PATTERN_NR: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(\d{1,2})[⧸/⁄](\d{1,2})"));
/// Écart : `\b` final (sinon « Part Deux » donnerait D = 500).
pub(crate) static ROMAN_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(?i)\b(part|pt)[\._\s]+([MDCLXVI]+)\b"));
pub(crate) static SEASON_MULTI_EP: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(?i)s(\d{1,4})[ _]?((?:([epx.-]+\d{1,4})+))"));
pub(crate) static SEASON_MULTI_EP_2: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(?i)(\d{1,4})(?=x)((?:([epx]+\d{1,4})+))"));
pub(crate) static SEASON_EP_RANGE: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(?i)s(\d{1,4})[ _]?[eE](\d{1,4})-[eE]?(\d{1,4})"));
pub(crate) static RESOLUTION: LazyLock<Regex> = LazyLock::new(|| rx(r"\d{3,4}x\d{3,4}"));
/// Année entre crochets ou parenthèses.
pub(crate) static ANNEE_ENCADREE: LazyLock<Regex> = LazyLock::new(|| rx(r"[\(\[]\d{4}[\)\]]"));
/// Somme de contrôle de 8 caractères hexadécimaux entre `[]`, `()` ou `{}`.
pub(crate) static HASH_8: LazyLock<Regex> = LazyLock::new(|| rx(r"(?i)[\[({][\da-f]{8}[\])}]"));
/// Blocs `[..]` et `{..}` (ignorés dans la recherche des nombres seuls).
pub(crate) static BLOCS: LazyLock<Regex> = LazyLock::new(|| rx(r"\[[^\]]*\]|\{[^}]*\}"));

/// Dossier de saison (`Season 1`, `S01`, `Staffel 2`…), nom entier.
pub(crate) static SEASON_ONLY_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(?i)^(s|staffel|season|series)[\s_.-]*(\d{1,4})$"));
pub(crate) static SEASON_NUMBER: LazyLock<Regex> = LazyLock::new(|| rx(r"(?i)season\s?(\d+).*"));
pub(crate) static SEASON_FOLDER_NUMBER: LazyLock<Regex> = LazyLock::new(|| rx(r"(?i).*?(\d+).*"));
/// `sNNeNN` dans le seul nom de fichier (corrige l'interprétation anime n° 2).
pub(crate) static SXXEXX: LazyLock<Regex> = LazyLock::new(|| rx(r"(?i)s(\d{1,4})e(\d{1,4})"));

// ---------------------------------------------------------------------------
// Animés
// ---------------------------------------------------------------------------

/// Fin commune des motifs « avec hash » : blocs optionnels puis `[ABCD1234]`.
const FIN_HASH: &str = r"[^\])}]*?(?:[\[({][^\])}]+[\])}][ _.-]*)*?(?:[\[({][\da-f]{8}[\])}])";
/// Fin commune des motifs « sans hash ».
const FIN_SANS_HASH: &str = r"[^\])}]*?(?:[\[({][^\])}]+[\])}][ _.-]*)*?[^\]\[)(}{\\/]*$";

/// Écart : `(?<![a-z])` devant le mot-clé (sinon « Crisp 05 » serait un spécial).
const SPECIAL: &str = r"(?<![a-z])(Special|SP|OVA|OAV|Picture Drama)(?:[ _.-]*(?:ep?[ .]?)?(\d{1,4})(?:[_ ]?v\d+)?)+(?=\b|_)";
const SAISON_DOSSIER_HASH: &str = r"(?:S(?:eason)?\s*(?=\d))?(Specials|\d{1,4})/(?:[^/]+/)*[^/]+(?:\b|_)(?:[ _.-]*(?:ep?[ .]?)?(\d{1,4})(?:[_ ]?v\d+)?)+(?=\b|_)";
const SAISON_DOSSIER_SANS_HASH: &str = r"(?:S(?:eason)?\s*(?=\d))?(Specials|\d{1,4})[\\/](?:[^\\/]+[\\/])*[^\\/]+(?:\b|_)[ _.-]*(?:ep?[ .]?)?(\d{1,4})(?:[_ ]?v\d+)?(?:\b|_)";
const SAISON_EPISODE: &str = r"[-._ ]+S(?:eason ?)?(\d{1,4})[ _.-]*(?:ep?[ .]?)?(\d{1,4})(?:[_ ]?v\d+)?((?:[ _.-]*[eExX]p?[ .]?\d{1,4}(?:[_ ]?v\d+)?|-[0-9]{1,4})*)(?=\b|_)";
const EPISODE_SEUL: &str =
    r"((?=\b|_))(?:[ _.-]*(?:ep?[ .]?)?(\d{1,4})(?:-(\d{1,3}))?(?:[_ ]?v\d+)?)+(?=\b|_)";

pub(crate) static ANIME_PREPEND1: LazyLock<Regex> =
    LazyLock::new(|| rx(&format!("(?i){SPECIAL}{FIN_HASH}")));
pub(crate) static ANIME_PREPEND2: LazyLock<Regex> =
    LazyLock::new(|| rx(&format!("(?i){SAISON_DOSSIER_HASH}{FIN_HASH}")));
pub(crate) static ANIME_PREPEND3: LazyLock<Regex> =
    LazyLock::new(|| rx(&format!("(?i){SAISON_EPISODE}{FIN_HASH}")));
pub(crate) static ANIME_PREPEND4: LazyLock<Regex> =
    LazyLock::new(|| rx(&format!("(?i){EPISODE_SEUL}{FIN_HASH}")));
/// Sensible à la casse.
pub(crate) static ANIME_PREPEND4_2: LazyLock<Regex> =
    LazyLock::new(|| rx(r"((\d{1,3})(?:-(\d{1,3})){1,10})"));
pub(crate) static ANIME_APPEND1: LazyLock<Regex> =
    LazyLock::new(|| rx(&format!("(?i){SPECIAL}{FIN_SANS_HASH}")));
pub(crate) static ANIME_APPEND2: LazyLock<Regex> =
    LazyLock::new(|| rx(&format!("(?i){SAISON_DOSSIER_SANS_HASH}{FIN_SANS_HASH}")));
pub(crate) static ANIME_APPEND3: LazyLock<Regex> =
    LazyLock::new(|| rx(&format!("(?i){SAISON_EPISODE}{FIN_SANS_HASH}")));
/// Suite d'épisodes de l'interprétation n° 3 : `-N`, `eN`, `xN`.
pub(crate) static SUITE_PLAGE: LazyLock<Regex> = LazyLock::new(|| rx(r"^-[eExX]?(\d{1,4})$"));
pub(crate) static NOMBRE: LazyLock<Regex> = LazyLock::new(|| rx(r"(\d{1,4})"));
/// Jetons d'une suite (`-06`, ` E07`, `x08`…).
pub(crate) static JETON_SUITE: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(?i)-[eExX]?\d{1,4}|[ _.-]*[eExX]p?[ .]?\d{1,4}"));

// ---------------------------------------------------------------------------
// Mots d'arrêt
// ---------------------------------------------------------------------------

/// Mots d'arrêt « durs » des noms d'épisodes (qualité, codec, source, audio…).
/// Les mots courts ambigus des noms de films (`ts`, `tc`, `hd`, `cam`, `pal`…)
/// sont volontairement absents : ils apparaissent dans des titres de séries.
pub(crate) const MOTS_ARRET_SERIES: &[&str] = &[
    r"2160p",
    r"1080p",
    r"1080i",
    r"720p",
    r"576p",
    r"576i",
    r"480p",
    r"480i",
    r"4320p",
    r"4k",
    r"8k",
    r"uhd",
    r"bluray",
    r"blu-ray",
    r"blueray",
    r"bdrip",
    r"brrip",
    r"bdremux",
    r"dvdrip",
    r"dvdr",
    r"dvd5",
    r"dvd9",
    r"dvdscr",
    r"hddvd",
    r"webrip",
    r"web-rip",
    r"webdl",
    r"web-dl",
    r"webhd",
    r"web",
    r"hdrip",
    r"hdtv",
    r"pdtv",
    r"sdtv",
    r"dsr",
    r"tvrip",
    r"x264",
    r"x265",
    r"h264",
    r"h265",
    r"h\.264",
    r"h\.265",
    r"hevc",
    r"avc",
    r"xvid",
    r"divx",
    r"mpeg2",
    r"vc-?1",
    r"av1",
    r"10bits?",
    r"8bits?",
    r"(?:dd\+?|ddp|aac|dts|e?ac3|truehd|atmos|flac|opus|lpcm)?[1-7]\.[01]",
    r"e?ac3",
    r"ddp?",
    r"dts(?:-?hd)?",
    r"dtsma",
    r"truehd",
    r"atmos",
    r"aac",
    r"flac",
    r"opus",
    r"mp3",
    r"lpcm",
    r"hdr(?:10)?(?:plus|\+)?",
    r"dolbyvision",
    r"dv",
    r"sdr",
    r"remux",
    r"repack",
    r"rerip",
    r"proper",
    r"internal",
    r"readnfo",
    r"nfofix",
    r"subbed",
    r"dubbed",
    r"multisubs",
    r"multi",
    r"vostfr",
    r"vost",
    r"truefrench",
    r"amzn",
    r"nf",
    r"hmax",
    r"dsnp",
    r"atvp",
    r"pcok",
    r"hulu",
    r"cd[1-9]",
    r"dis[ck][1-9]",
];

const SEPARATEURS: &str = r"[\s_.\-\[\](){}+,]";

/// Mots d'arrêt précédés d'un séparateur (ou du début) et suivis d'un
/// séparateur (ou de la fin). Le séparateur initial est conservé (`$1`).
pub(crate) static RE_MOTS_ARRET: LazyLock<Regex> = LazyLock::new(|| {
    rx(&format!(
        r"(?i)(^|{SEPARATEURS})(?:{})(?=$|{SEPARATEURS})",
        MOTS_ARRET_SERIES.join("|")
    ))
});

/// Mots techniques non ambigus : dans un titre d'épisode, tout ce qui suit le
/// premier d'entre eux est du bruit (qualité, source, codec).
const MOTS_COUPURE_TITRE: &[&str] = &[
    r"2160p",
    r"1080p",
    r"1080i",
    r"720p",
    r"576p",
    r"480p",
    r"4320p",
    r"4k",
    r"uhd",
    r"bluray",
    r"blu-ray",
    r"bdrip",
    r"brrip",
    r"bdremux",
    r"dvdrip",
    r"hddvd",
    r"webrip",
    r"web-rip",
    r"webdl",
    r"web-dl",
    r"hdrip",
    r"hdtv",
    r"pdtv",
    r"sdtv",
    r"tvrip",
    r"x264",
    r"x265",
    r"h264",
    r"h265",
    r"h\.264",
    r"h\.265",
    r"hevc",
    r"xvid",
    r"divx",
    r"10bits?",
    r"remux",
    r"hdr(?:10)?",
    r"dts(?:-?hd)?",
    r"e?ac3",
    r"truehd",
    r"atmos",
    r"aac(?:[1-7]\.[01])?",
    r"ddp?[1-7]\.[01]",
];

/// Premier mot technique d'un titre (voir [`MOTS_COUPURE_TITRE`]).
pub(crate) static RE_COUPURE_TITRE: LazyLock<Regex> = LazyLock::new(|| {
    rx(&format!(
        r"(?i)(^|{SEPARATEURS})(?:{})(?=$|{SEPARATEURS})",
        MOTS_COUPURE_TITRE.join("|")
    ))
});

/// Fichiers internes des structures de disque.
pub(crate) static FICHIER_DISQUE: LazyLock<Regex> = LazyLock::new(|| {
    rx(
        r"(?i)^(?:(?:video_ts|vts_\d\d_\d)\.(?:vob|bup|ifo)|index\.bdmv|movieobject\.bdmv|\d{5}\.m2ts)$",
    )
});
