//! Extraction du titre, de l'année et des identifiants depuis un nom de
//! fichier ou de dossier.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use super::extensions::est_extension_connue;
use crate::modele::Edition;
use crate::modele::fichier::extension_minuscule;

fn rx(motif: &str) -> Regex {
    Regex::new(motif).unwrap()
}

/// Mots d'arrêt « durs » : tout ce qui suit est du bruit (qualité, codec…).
const MOTS_ARRET_DURS: &[&str] = &[
    "1080p",
    "1080i",
    "720p",
    "720i",
    "480p",
    "576p",
    "2160p",
    "4320p",
    "4k",
    "8k",
    "uhd",
    "fhd",
    "hd",
    "hq",
    "bluray",
    "blu-ray",
    "blueray",
    "bdrip",
    "brrip",
    "bdremux",
    "bdmv",
    "bd25",
    "bd50",
    "dvdrip",
    "dvdr",
    "dvd5",
    "dvd9",
    "dvdscr",
    "dvdmux",
    "hddvd",
    "webrip",
    "webdl",
    "web-dl",
    "webhd",
    "hdrip",
    "hdtv",
    "pdtv",
    "dsr",
    "tvrip",
    "vhsrip",
    "x264",
    "x265",
    "h264",
    "h265",
    "h.264",
    "h.265",
    "hevc",
    "avc",
    "xvid",
    "divx",
    "mpeg2",
    "vc1",
    "av1",
    "10bit",
    "10bits",
    "8bit",
    "ac3",
    "eac3",
    "dd5",
    "dd51",
    "dts",
    "dtshd",
    "dts-hd",
    "dtsma",
    "truehd",
    "atmos",
    "aac",
    "aac2",
    "flac",
    "mp3",
    "lpcm",
    "hdr",
    "hdr10",
    "hdr10plus",
    "dolbyvision",
    "sdr",
    "remux",
    "repack",
    "rerip",
    "proper",
    "uncut",
    "unrated",
    "remastered",
    "restored",
    "cd1",
    "cd2",
    "cd3",
    "cd4",
    "cd5",
    "cd6",
    "cd7",
    "cd8",
    "cd9",
    "disc1",
    "disc2",
    "disc3",
    "disc4",
    "disc5",
    "disc6",
    "disc7",
    "disc8",
    "disc9",
    "disk1",
    "disk2",
    "disk3",
    "disk4",
    "dvd1",
    "dvd2",
    "dvd3",
    "dvd4",
    "dvd5",
    "dvd6",
    "dvd7",
    "dvd8",
    "dvd9",
    "ntsc",
    "pal",
    "secam",
    "subbed",
    "dubbed",
    "multisubs",
    "multi",
    "vostfr",
    "vost",
    "vff",
    "vfq",
    "vfi",
    "vf",
    "vf2",
    "truefrench",
    "subfrench",
    "3d",
    "hsbs",
    "cam",
    "camrip",
    "hdcam",
    "ts",
    "hdts",
    "tc",
    "hdtc",
    "telesync",
    "telecine",
    "screener",
    "scr",
    "r5",
    "r6",
    "workprint",
    "amzn",
    "hmax",
    "dsnp",
    "atvp",
    "pcok",
    "hulu",
    "readnfo",
    "nfofix",
    "dirfix",
    "extended",
    "theatrical",
    "imax",
];

/// Mots d'arrêt « souples » : recherchés à partir de la position de l'année.
/// Ils n'influencent pas le titre (celui-ci s'arrête de toute façon à l'année).
#[allow(dead_code)]
const MOTS_ARRET_SOUPLES: &[&str] = &[
    "complete", "custom", "dc", "docu", "doku", "extended", "fragment", "internal", "limited",
    "local", "ma", "multi", "pal", "proper", "read", "retail", "se", "www", "xxx",
];

/// Expressions retirées du nom avant découpage (comparaison insensible à la casse).
const MOTS_NETTOYAGE: &[&str] = &[
    "24.000", "23.976", "23.98", "24.00", "web-dl", "web-rip", "blue-ray", "blu-ray", "dvd-rip",
];

static ENSEMBLE_DURS: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| MOTS_ARRET_DURS.iter().copied().collect());
static RE_RESOLUTION: LazyLock<Regex> = LazyLock::new(|| rx(r"\d{3,4}x\d{3,4}"));
static RE_CROCHETS: LazyLock<Regex> = LazyLock::new(|| rx(r"\[(.*?)\]"));
static RE_ACCOLADES: LazyLock<Regex> = LazyLock::new(|| rx(r"\{.*?\}"));
static RE_OTR: LazyLock<Regex> = LazyLock::new(|| rx(r"_\d\d.\d\d.\d\d_\d\d-\d\d_"));
static RE_SEPARATEURS: LazyLock<Regex> = LazyLock::new(|| rx(r"[\[\](){} _,.-]"));
static RE_ANNEE: LazyLock<Regex> = LazyLock::new(|| rx(r"^\d{4}$"));
static RE_IMDB_JETON: LazyLock<Regex> = LazyLock::new(|| rx(r"(?i)^tt\d{6,}$"));
static RE_ROMAIN: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(?i)^(?:i|ii|iii|iv|v|vi|vii|viii|ix|x)$"));
static RE_AKA: LazyLock<Regex> = LazyLock::new(|| rx(r"(?i) aka .*"));

/// Année courante (calendrier grégorien, UTC).
pub fn annee_courante() -> i32 {
    let secondes = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let jours = secondes.div_euclid(86_400) + 719_468;
    let ere = jours.div_euclid(146_097);
    let jour_ere = jours.rem_euclid(146_097);
    let annee_ere = (jour_ere - jour_ere / 1460 + jour_ere / 36_524 - jour_ere / 146_096) / 365;
    let jour_annee = jour_ere - (365 * annee_ere + annee_ere / 4 - annee_ere / 100);
    let mp = (5 * jour_annee + 2) / 153;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    let annee = annee_ere + ere * 400 + i64::from(mois <= 2);
    annee as i32
}

fn retirer_ci(texte: &str, motif: &str) -> String {
    let re = Regex::new(&format!("(?i){}", regex::escape(motif))).unwrap();
    re.replace_all(texte, "").into_owned()
}

/// Titre et année détectés (`annee` = 0 si inconnue).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TitreAnnee {
    pub titre: String,
    pub annee: u32,
}

/// Détecte titre et année dans un nom de fichier ou de dossier.
///
/// `mots_interdits` : expressions rationnelles (insensibles à la casse)
/// retirées du nom ; si tout disparaît, le nom d'origine est conservé.
/// L'extension n'est retirée que si elle est connue (vidéo, audio,
/// sous-titres, image, nfo…), pour ne pas confondre `Film.2019` avec un fichier.
pub fn detecter_titre_et_annee(nom: &str, mots_interdits: &[String]) -> TitreAnnee {
    detecter_titre_et_annee_avec(nom, mots_interdits, &prefixes_titre_defaut())
}

/// Comme [`detecter_titre_et_annee`], avec une liste d'articles personnalisée
/// pour la forme « Matrix, The (1999) » ⇒ « The Matrix ».
pub fn detecter_titre_et_annee_avec(
    nom: &str,
    mots_interdits: &[String],
    prefixes: &[String],
) -> TitreAnnee {
    let mut s = deplacer_article(nom.trim(), prefixes);

    // 1. Extension, résolution, expressions de nettoyage.
    if let Some((base, ext)) = s.rsplit_once('.')
        && !base.is_empty()
        && est_extension_connue(&ext.to_lowercase())
    {
        s = base.to_string();
    }
    s = RE_RESOLUTION.replace_all(&s, "").into_owned();
    for m in MOTS_NETTOYAGE {
        s = retirer_ci(&s, m);
    }

    // 2. Mots interdits utilisateur.
    let avant = s.clone();
    for m in mots_interdits {
        if m.is_empty() {
            continue;
        }
        if let Ok(re) = Regex::new(&format!("(?i){m}")) {
            s = re.replace_all(&s, "").into_owned();
        }
    }
    if s.trim().is_empty() {
        s = avant;
    }

    // 3. Options entre crochets (mises en réserve), accolades supprimées.
    let options: Vec<String> = RE_CROCHETS
        .captures_iter(&s)
        .map(|c| c[1].to_string())
        .collect();
    s = RE_CROCHETS.replace_all(&s, " ").into_owned();
    s = RE_ACCOLADES.replace_all(&s, " ").into_owned();
    if let Some(m) = RE_OTR.find(&s)
        && m.start() > 10
    {
        s.truncate(m.start());
    }

    // 4. Découpage en jetons.
    let mut jetons: Vec<String> = RE_SEPARATEURS
        .split(&s)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect();
    let n = jetons.len();
    let mut arret = n;

    // VIDEO + TS (dossier de DVD) : bruit.
    for i in 0..n.saturating_sub(1) {
        if jetons[i].eq_ignore_ascii_case("video") && jetons[i + 1].eq_ignore_ascii_case("ts") {
            jetons[i].clear();
            jetons[i + 1].clear();
            if i >= 2 && i < arret {
                arret = i;
            }
        }
    }

    // 5. Noms d'édition sur 1, 2 ou 3 jetons consécutifs.
    for i in 1..n {
        for longueur in 1..=3 {
            if i + longueur > n {
                break;
            }
            let groupe = &jetons[i..i + longueur];
            if groupe.iter().any(String::is_empty) {
                break;
            }
            if Edition::est_motif_edition(&groupe.join(".")) {
                jetons[i..i + longueur].iter_mut().for_each(String::clear);
                arret = arret.min(i);
                break;
            }
        }
    }

    // 6. Mots d'arrêt durs et identifiants IMDb.
    for (i, jeton) in jetons.iter_mut().enumerate() {
        if jeton.is_empty() {
            continue;
        }
        if RE_IMDB_JETON.is_match(jeton) {
            jeton.clear();
            continue;
        }
        if i >= 1 && ENSEMBLE_DURS.contains(jeton.to_lowercase().as_str()) {
            jeton.clear();
            if i >= 2 && i < arret {
                arret = i;
            }
        }
    }

    // 7. Année : en partant de la fin ; sinon un jeton entre crochets.
    let max_annee = annee_courante() + 5;
    let valide = |a: i32| a > 1800 && a < max_annee;
    let mut annee = 0u32;
    let mut position_annee = n;
    for i in (1..n).rev() {
        if RE_ANNEE.is_match(&jetons[i]) {
            let a: i32 = jetons[i].parse().unwrap_or(0);
            if valide(a) {
                annee = a as u32;
                position_annee = i;
                break;
            }
        }
    }
    if annee == 0 {
        for o in &options {
            let o = o.trim();
            if RE_ANNEE.is_match(o) {
                let a: i32 = o.parse().unwrap_or(0);
                if valide(a) {
                    annee = a as u32;
                    break;
                }
            }
        }
    }

    // 8. (Mots d'arrêt souples : sans effet, le titre s'arrête déjà à l'année.)

    // 9. Titre.
    let fin = arret.min(position_annee);
    let mut titre = jetons[..fin]
        .iter()
        .filter(|t| !t.is_empty())
        .map(|t| {
            if RE_ROMAIN.is_match(t) {
                t.to_uppercase()
            } else {
                t.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    titre = RE_AKA.replace(&titre, "").trim().to_string();
    if titre.is_empty() {
        titre = RE_SEPARATEURS
            .split(&s)
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
    }
    TitreAnnee { titre, annee }
}

/// « Matrix, The (1999) » ⇒ « The Matrix (1999) » : déplace un article placé
/// après une virgule, avant le découpage en jetons (la virgule est un séparateur).
fn deplacer_article(nom: &str, prefixes: &[String]) -> String {
    let Some((base, reste)) = nom.split_once(',') else {
        return nom.to_string();
    };
    let reste_net = reste.trim_start();
    for p in prefixes {
        if p.is_empty() || reste_net.len() < p.len() || !reste_net.is_char_boundary(p.len()) {
            continue;
        }
        let (tete, suite) = reste_net.split_at(p.len());
        let fin_ok = suite.is_empty() || suite.starts_with([' ', '.', '(', '[', '_', '-']);
        if tete.eq_ignore_ascii_case(p) && fin_ok && !base.trim().is_empty() {
            let colle = p.ends_with(['\'', '´', '`']);
            let sep = if colle { "" } else { " " };
            return format!("{tete}{sep}{}{suite}", base.trim());
        }
    }
    nom.to_string()
}

/// « Matrix, The » ⇒ « The Matrix » selon la liste de préfixes (articles).
pub fn retirer_titre_triable(titre: &str, prefixes: &[String]) -> String {
    let t = titre.trim();
    if let Some((base, fin)) = t.rsplit_once(',') {
        let article = fin.trim();
        if !base.trim().is_empty() && prefixes.iter().any(|p| p.eq_ignore_ascii_case(article)) {
            let colle = article.ends_with(['\'', '´', '`']);
            return if colle {
                format!("{}{}", article, base.trim())
            } else {
                format!("{} {}", article, base.trim())
            };
        }
    }
    t.to_string()
}

/// Préfixes (articles) usuels pour [`retirer_titre_triable`].
pub fn prefixes_titre_defaut() -> Vec<String> {
    [
        "A", "An", "The", "Der", "Die", "Das", "Ein", "Eine", "Le", "La", "Les", "L'", "L´", "L`",
        "Un", "Une", "Des", "Du", "D'", "D´", "D`",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

// ---------------------------------------------------------------------------
// Identifiants dans un texte
// ---------------------------------------------------------------------------

static RE_IMDB: LazyLock<Regex> = LazyLock::new(|| rx(r"(?i)(?:^|[^A-Za-z])(tt\d{6,})"));
static RE_TMDB: LazyLock<Regex> = LazyLock::new(|| {
    rx(r"(?i)(?:(?:tmdbid|tmdb)[ ._=-]?(\d+)|themoviedb\.org/(?:movie|tv)/(\d+))")
});
static RE_TVDB: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(?i)(?:(?:tvdbid|tvdb)[ ._=-]?(\d+)|thetvdb\.com/series/(\d+))"));

/// Identifiants trouvés dans un nom ou un texte.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdsDetectes {
    pub imdb: Option<String>,
    pub tmdb: Option<i64>,
    pub tvdb: Option<i64>,
}

/// Cherche IMDb (`tt` + ≥ 6 chiffres), TMDB et TVDB dans un texte.
pub fn extraire_ids(texte: &str) -> IdsDetectes {
    let imdb = RE_IMDB.captures(texte).map(|c| c[1].to_lowercase());
    let tmdb = RE_TMDB.captures(texte).and_then(|c| {
        c.get(1)
            .or_else(|| c.get(2))
            .and_then(|m| m.as_str().parse().ok())
    });
    let tvdb = RE_TVDB.captures(texte).and_then(|c| {
        c.get(1)
            .or_else(|| c.get(2))
            .and_then(|m| m.as_str().parse().ok())
    });
    IdsDetectes { imdb, tmdb, tvdb }
}

static RE_3D: LazyLock<Regex> = LazyLock::new(|| rx(r"(?i)[ .,_\(\[-]3D[ .,_\)\]-]?"));

/// Détecte la mention « 3D » dans un nom.
pub fn contient_3d(nom: &str) -> bool {
    RE_3D.is_match(nom)
}

/// Extension en minuscules d'un nom de fichier (réexport pratique).
pub fn extension(nom: &str) -> String {
    extension_minuscule(nom)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ta(nom: &str) -> (String, u32) {
        let r = detecter_titre_et_annee(nom, &[]);
        (r.titre, r.annee)
    }

    #[test]
    fn nom_de_release_classique() {
        assert_eq!(
            ta("The.Matrix.1999.1080p.BluRay.x264-GROUP.mkv"),
            ("The Matrix".into(), 1999)
        );
    }

    #[test]
    fn annee_entre_parentheses() {
        assert_eq!(ta("Movie Name (2010)"), ("Movie Name".into(), 2010));
    }

    #[test]
    fn annee_entre_crochets_en_repli() {
        assert_eq!(
            ta("Spider-Man Homecoming [2017]"),
            ("Spider Man Homecoming".into(), 2017)
        );
    }

    #[test]
    fn titre_contenant_une_annee() {
        assert_eq!(
            ta("Blade Runner 2049 (2017)"),
            ("Blade Runner 2049".into(), 2017)
        );
        // Une année trop lointaine n'est pas une année.
        assert_eq!(ta("Blade Runner 2049"), ("Blade Runner 2049".into(), 0));
        // Jeton unique : jamais pris pour l'année.
        assert_eq!(ta("2012"), ("2012".into(), 0));
        assert_eq!(
            ta("2001 A Space Odyssey 1968"),
            ("2001 A Space Odyssey".into(), 1968)
        );
    }

    #[test]
    fn edition_et_identifiant_imdb_sont_retires() {
        assert_eq!(ta("Alien.Director's.Cut.1979.720p"), ("Alien".into(), 1979));
        assert_eq!(ta("Film.tt1234567.2019"), ("Film".into(), 2019));
    }

    #[test]
    fn extension_inconnue_conservee() {
        // `.2019` n'est pas une extension de fichier.
        assert_eq!(ta("Film.2019"), ("Film".into(), 2019));
    }

    #[test]
    fn chiffres_romains_et_aka() {
        assert_eq!(ta("Rocky iv 1985"), ("Rocky IV".into(), 1985));
        assert_eq!(ta("Titre aka Autre Titre 2000"), ("Titre".into(), 2000));
    }

    #[test]
    fn marqueurs_de_pile_et_codecs() {
        assert_eq!(
            ta("Dune.Part.Two.2024.CD1.x265"),
            ("Dune Part Two".into(), 2024)
        );
        assert_eq!(ta("Some Long Film CD1"), ("Some Long Film".into(), 0));
    }

    #[test]
    fn mots_interdits_utilisateur() {
        let r = detecter_titre_et_annee("Film [GroupeXYZ] 2015", &["groupexyz".to_string()]);
        assert_eq!((r.titre.as_str(), r.annee), ("Film", 2015));
        // Si tout est retiré, le nom d'origine est conservé.
        let r = detecter_titre_et_annee("Bruit", &["bruit".to_string()]);
        assert_eq!(r.titre, "Bruit");
    }

    #[test]
    fn un_mot_d_arret_en_tete_n_efface_pas_le_titre() {
        assert_eq!(ta("Uncut Gems 2019").0, "Uncut Gems");
    }

    #[test]
    fn nom_sans_titre_exploitable() {
        assert_eq!(ta("1080p").0, "1080p");
    }

    #[test]
    fn titre_triable() {
        let p = prefixes_titre_defaut();
        assert_eq!(retirer_titre_triable("Matrix, The", &p), "The Matrix");
        assert_eq!(retirer_titre_triable("Aventure, L'", &p), "L'Aventure");
        assert_eq!(
            retirer_titre_triable("Bonjour, Monde", &p),
            "Bonjour, Monde"
        );
    }

    #[test]
    fn identifiants_dans_un_nom() {
        let i = extraire_ids("Film (2019) {imdb-tt1234567} [tmdbid=603] tvdb_42");
        assert_eq!(i.imdb.as_deref(), Some("tt1234567"));
        assert_eq!(i.tmdb, Some(603));
        assert_eq!(i.tvdb, Some(42));
        let i = extraire_ids("https://www.themoviedb.org/movie/550-fight-club");
        assert_eq!(i.tmdb, Some(550));
        let i = extraire_ids("https://www.themoviedb.org/tv/1396");
        assert_eq!(i.tmdb, Some(1396));
        assert_eq!(extraire_ids("Film tt12345"), IdsDetectes::default());
    }

    #[test]
    fn detection_3d() {
        assert!(contient_3d("Avatar (2009) 3D"));
        assert!(contient_3d("Avatar.3D.HSBS.mkv"));
        assert!(!contient_3d("Avatar 2009"));
        assert!(!contient_3d("Film3Dx"));
    }

    #[test]
    fn annee_courante_plausible() {
        let a = annee_courante();
        assert!((2024..2200).contains(&a));
    }
}

#[cfg(test)]
mod tests_article {
    use super::*;

    #[test]
    fn article_apres_virgule_dans_un_nom() {
        let r = detecter_titre_et_annee("Matrix, The (1999)", &[]);
        assert_eq!((r.titre.as_str(), r.annee), ("The Matrix", 1999));
        let r = detecter_titre_et_annee("Aventure, L' (1999)", &[]);
        assert_eq!(r.titre, "L'Aventure");
        let r = detecter_titre_et_annee("Bonjour, Theo (2001)", &[]);
        assert_eq!(r.titre, "Bonjour Theo");
    }
}
