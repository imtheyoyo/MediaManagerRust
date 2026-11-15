//! Types communs aux films et aux séries : identifiants, notes, personnes,
//! bandes-annonces, éditions et sources média.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Identifiants
// ---------------------------------------------------------------------------

/// Valeur d'un identifiant externe : entier (tmdb, tvdb…) ou texte (imdb…).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ValeurId {
    Entier(i64),
    Texte(String),
}

impl ValeurId {
    /// Représentation textuelle de la valeur.
    pub fn en_texte(&self) -> String {
        match self {
            ValeurId::Entier(n) => n.to_string(),
            ValeurId::Texte(t) => t.clone(),
        }
    }

    /// Valeur entière si elle existe (entier direct ou texte numérique).
    pub fn en_entier(&self) -> Option<i64> {
        match self {
            ValeurId::Entier(n) => Some(*n),
            ValeurId::Texte(t) => t.trim().parse().ok(),
        }
    }
}

/// Clés usuelles des identifiants.
pub mod cles_id {
    pub const IMDB: &str = "imdb";
    pub const TMDB: &str = "tmdb";
    pub const TVDB: &str = "tvdb";
    pub const TRAKT: &str = "trakt";
    /// Identifiant de la collection (saga) chez le fournisseur TMDB.
    pub const TMDB_SAGA: &str = "tmdbSet";
}

static RE_IMDB_VALIDE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^tt\d{6,}$").unwrap());

/// Vrai si `s` est un identifiant IMDb valide (`tt` suivi d'au moins 6 chiffres).
pub fn est_id_imdb_valide(s: &str) -> bool {
    RE_IMDB_VALIDE.is_match(s)
}

/// Table d'identifiants externes (clé = fournisseur).
///
/// Règles de [`Identifiants::definir`] : valeur vide, `"0"` ou `"null"` ⇒ la clé
/// est supprimée ; `imdb` invalide ⇒ rejeté ; texte entièrement numérique ⇒
/// converti en entier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Identifiants(BTreeMap<String, ValeurId>);

impl Identifiants {
    pub fn new() -> Self {
        Self::default()
    }

    /// Définit un identifiant selon les règles décrites sur le type.
    /// Renvoie `true` si la table a été modifiée.
    pub fn definir(&mut self, cle: &str, valeur: &str) -> bool {
        let v = valeur.trim();
        if v.is_empty() || v == "0" || v.eq_ignore_ascii_case("null") {
            return self.0.remove(cle).is_some();
        }
        if cle == cles_id::IMDB && !est_id_imdb_valide(v) {
            return false;
        }
        let nouvelle = if !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()) {
            match v.parse::<i64>() {
                Ok(n) => ValeurId::Entier(n),
                Err(_) => ValeurId::Texte(v.to_string()),
            }
        } else {
            ValeurId::Texte(v.to_string())
        };
        if self.0.get(cle) == Some(&nouvelle) {
            return false;
        }
        self.0.insert(cle.to_string(), nouvelle);
        true
    }

    /// Définit l'identifiant seulement s'il est absent.
    pub fn definir_si_absent(&mut self, cle: &str, valeur: &str) -> bool {
        if self.0.contains_key(cle) {
            return false;
        }
        self.definir(cle, valeur)
    }

    pub fn obtenir(&self, cle: &str) -> Option<&ValeurId> {
        self.0.get(cle)
    }

    pub fn contient(&self, cle: &str) -> bool {
        self.0.contains_key(cle)
    }

    pub fn est_vide(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &ValeurId)> {
        self.0.iter()
    }

    /// Identifiant IMDb (chaîne vide si absent).
    pub fn imdb(&self) -> String {
        self.0
            .get(cles_id::IMDB)
            .map(|v| v.en_texte())
            .unwrap_or_default()
    }

    /// Identifiant TMDB (0 si absent).
    pub fn tmdb(&self) -> i64 {
        self.0
            .get(cles_id::TMDB)
            .and_then(|v| v.en_entier())
            .unwrap_or(0)
    }

    /// Identifiant TVDB (0 si absent).
    pub fn tvdb(&self) -> i64 {
        self.0
            .get(cles_id::TVDB)
            .and_then(|v| v.en_entier())
            .unwrap_or(0)
    }

    /// Identifiant Trakt (0 si absent).
    pub fn trakt(&self) -> i64 {
        self.0
            .get(cles_id::TRAKT)
            .and_then(|v| v.en_entier())
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Notes
// ---------------------------------------------------------------------------

/// Note attribuée par une source (imdb, tmdb, user, metacritic…).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Note {
    /// Source de la note.
    pub id: String,
    /// Valeur ; `-1.0` = absente.
    pub valeur: f32,
    pub votes: u32,
    /// Valeur maximale de l'échelle (10 par défaut).
    pub maximum: u32,
}

impl Default for Note {
    fn default() -> Self {
        Self {
            id: String::new(),
            valeur: -1.0,
            votes: 0,
            maximum: 10,
        }
    }
}

impl Note {
    pub fn nouvelle(id: &str, valeur: f32, votes: u32, maximum: u32) -> Self {
        Self {
            id: id.to_string(),
            valeur,
            votes,
            maximum,
        }
    }

    /// Note ramenée sur 10.
    pub fn sur_dix(&self) -> f32 {
        if self.maximum == 0 {
            0.0
        } else {
            self.valeur * 10.0 / self.maximum as f32
        }
    }

    /// Une note n'est exploitable que si sa valeur est strictement positive.
    pub fn est_valide(&self) -> bool {
        self.valeur > 0.0
    }
}

// ---------------------------------------------------------------------------
// Personnes
// ---------------------------------------------------------------------------

/// Rôle d'une personne dans la production.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypePersonne {
    Acteur,
    Realisateur,
    Scenariste,
    Producteur,
    Invite,
    Compositeur,
    Monteur,
    Camera,
    #[default]
    Autre,
}

/// Acteur ou membre de l'équipe technique.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Personne {
    pub type_personne: TypePersonne,
    pub nom: String,
    /// Rôle joué (acteur) ou fonction précise (équipe).
    pub role: String,
    pub url_vignette: String,
    pub url_profil: String,
    pub ids: Identifiants,
}

// ---------------------------------------------------------------------------
// Bande-annonce
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BandeAnnonce {
    pub id: String,
    pub nom: String,
    pub url: String,
    pub qualite: String,
    pub langue: String,
    pub fournisseur: String,
    /// Vrai si la bande-annonce provient du NFO.
    pub dans_nfo: bool,
    pub date: String,
    pub scrape_par: String,
}

// ---------------------------------------------------------------------------
// Édition
// ---------------------------------------------------------------------------

/// Édition d'un film (version longue, director's cut…).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edition {
    #[default]
    Aucune,
    DirectorsCut,
    Etendue,
    Cinema,
    NonClassee,
    NonCoupee,
    Imax,
    Remasterisee,
    Collector,
    Ultime,
    FinalCut,
    Speciale,
    Criterion,
    OpenMatte,
    /// Édition libre (motif `{edition-XXX}` ou valeur inconnue).
    Personnalisee(String),
}

/// Valeurs « standard » dans l'ordre d'évaluation, avec leur regex de détection
/// (appliquée en recherche partielle, insensible à la casse).
static MOTIFS_EDITION: LazyLock<Vec<(Edition, Regex)>> = LazyLock::new(|| {
    let table: [(Edition, &str); 13] = [
        (Edition::DirectorsCut, r".Director.?s.(Cut|Edition|Version)"),
        (Edition::Etendue, r".Extended.(Cut|Edition|Version)?"),
        (Edition::Cinema, r".Theatrical.(Cut|Edition|Version)?"),
        (Edition::NonClassee, r".Unrated.(Cut|Edition|Version)?"),
        (Edition::NonCoupee, r".Uncut.(Cut|Edition|Version)?"),
        (Edition::Imax, r"^(IMAX|.*?.IMAX).(Cut|Edition|Version)?"),
        (Edition::Remasterisee, r".Remastered.(Cut|Edition|Version)?"),
        (Edition::Collector, r".Collectors.(Cut|Edition|Version)"),
        (Edition::Ultime, r".Ultimate.(Cut|Edition|Version)"),
        (Edition::FinalCut, r".Final.(Cut|Edition|Version)"),
        (Edition::Speciale, r".Special.(Cut|Edition|Version)"),
        (Edition::Criterion, r".Criterion.(Collection|Edition)"),
        (Edition::OpenMatte, r".Open.?Matte"),
    ];
    table
        .into_iter()
        .map(|(e, re)| (e, Regex::new(&format!("(?i){re}")).unwrap()))
        .collect()
});

static RE_EDITION_LIBRE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\{edition-(.+?)\}").unwrap());

impl Edition {
    /// Libellé lisible (vide pour [`Edition::Aucune`]).
    pub fn libelle(&self) -> String {
        match self {
            Edition::Aucune => "",
            Edition::DirectorsCut => "Director's Cut",
            Edition::Etendue => "Extended Edition",
            Edition::Cinema => "Theatrical Edition",
            Edition::NonClassee => "Unrated",
            Edition::NonCoupee => "Uncut",
            Edition::Imax => "IMAX",
            Edition::Remasterisee => "Remastered",
            Edition::Collector => "Collectors Edition",
            Edition::Ultime => "Ultimate Edition",
            Edition::FinalCut => "Final Edition",
            Edition::Speciale => "Special Edition",
            Edition::Criterion => "Criterion Collection",
            Edition::OpenMatte => "Open Matte",
            Edition::Personnalisee(s) => s,
        }
        .to_string()
    }

    /// Détecte l'édition dans un nom : libellé exact, regex de chaque édition
    /// (recherche partielle), puis motif `{edition-XXX}`.
    pub fn depuis_chaine(s: &str) -> Edition {
        let s = s.trim();
        if s.is_empty() {
            return Edition::Aucune;
        }
        for (e, re) in MOTIFS_EDITION.iter() {
            if e.libelle().eq_ignore_ascii_case(s) {
                return e.clone();
            }
            // Les motifs encadrent le mot par des caractères jokers : on ajoute
            // un séparateur de chaque côté pour trouver aussi une édition placée
            // en tête ou en fin de chaîne.
            if re.is_match(&format!(".{s}.")) {
                return e.clone();
            }
        }
        if let Some(c) = RE_EDITION_LIBRE.captures(s) {
            return Edition::Personnalisee(c[1].trim().to_string());
        }
        Edition::Aucune
    }

    /// Teste si `s` (un ou plusieurs jetons joints par `.`) désigne **en
    /// entier** une édition standard (utilisé par le nettoyage de titre : un
    /// groupe de jetons dont seule une partie serait une édition est refusé).
    pub fn est_motif_edition(s: &str) -> bool {
        let sans_fin = format!(".{s}");
        let avec_fin = format!(".{s}.");
        MOTIFS_EDITION.iter().any(|(_, re)| {
            [&sans_fin, &avec_fin].iter().any(|t| {
                re.find(t)
                    .is_some_and(|m| m.start() == 0 && m.end() == t.len())
            })
        })
    }
}

// ---------------------------------------------------------------------------
// Source média
// ---------------------------------------------------------------------------

/// Origine du support (Blu-ray, DVD, web…).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceMedia {
    UhdBluray,
    Bluray,
    Dvd,
    HdDvd,
    Tv,
    Vhs,
    Laserdisc,
    DVhs,
    HdRip,
    Cam,
    Ts,
    Tc,
    DvdScr,
    R5,
    WebRip,
    WebDl,
    Stream,
    #[default]
    Inconnue,
}

const SEPARATEURS_SOURCE: &str = r"[\/\\ _,.()\[\]-]";

/// Sources testables avec le nom d'énumération d'origine (la longueur de ce
/// nom fixe l'ordre d'essai : les plus longs d'abord).
static MOTIFS_SOURCE: LazyLock<Vec<(SourceMedia, Regex)>> = LazyLock::new(|| {
    let table: [(SourceMedia, &str, &str); 17] = [
        (
            SourceMedia::UhdBluray,
            "UHD_BLURAY",
            r"(uhd[ ._-]?blu[ -]?ray|uhd[ ._-]?bd|uhdbd|bd[ ._-]?uhd|4k[ ._-]?blu[ -]?ray|uhd[ ._-]?remux)",
        ),
        (
            SourceMedia::Bluray,
            "BLURAY",
            r"(bluray|blueray|bdrip|brrip|dbrip|bd5|bd9|bd25|bd50|bd66|bd100|bdr|bdmv|bdiso|blu\-ray|bdremux|fullbluray|fullblu-ray)",
        ),
        (
            SourceMedia::Dvd,
            "DVD",
            r"(dvd|dvdrip|dvdr|dvd5|dvd9|dvdmux|dvdiso|video_ts)",
        ),
        (
            SourceMedia::HdDvd,
            "HDDVD",
            r"(hddvd|hd[ -]dvd|hddvdrip|hvdvd_ts)",
        ),
        (
            SourceMedia::Tv,
            "TV",
            r"(hdtv|pdtv|dsr|dvb|tvrip|sdtv|tv[ ._-]?rip)",
        ),
        (SourceMedia::Vhs, "VHS", r"(vhs|vhsrip)"),
        (
            SourceMedia::Laserdisc,
            "LASERDISC",
            r"(laserdisc|ld[ ._-]?rip)",
        ),
        (SourceMedia::DVhs, "D_VHS", r"(d[ -]?vhs)"),
        (SourceMedia::HdRip, "HDRIP", r"(hdrip|hd[ -]rip)"),
        (SourceMedia::Cam, "CAM", r"(cam|camrip|hdcam)"),
        (SourceMedia::Ts, "TS", r"(ts|telesync|hdts|pdvd)"),
        (SourceMedia::Tc, "TC", r"(tc|telecine|hdtc)"),
        (
            SourceMedia::DvdScr,
            "DVDSCR",
            r"(dvdscr|dvdscreener|scr|screener)",
        ),
        (SourceMedia::R5, "R5", r"(r5)"),
        (SourceMedia::WebRip, "WEBRIP", r"(webrip|web[ -]rip)"),
        (SourceMedia::WebDl, "WEB_DL", r"(web[ -]?dl|webdl|webhd)"),
        (SourceMedia::Stream, "STREAM", r"(stream|streaming)"),
    ];
    let mut v: Vec<(SourceMedia, &str, &str)> = table.into_iter().collect();
    v.sort_by_key(|(_, nom, _)| std::cmp::Reverse(nom.len()));
    v.into_iter()
        .map(|(s, _, re)| {
            let motif = format!("(?i)(?:^|{SEPARATEURS_SOURCE}){re}(?:{SEPARATEURS_SOURCE}|$)");
            (s, Regex::new(&motif).unwrap())
        })
        .collect()
});

impl SourceMedia {
    /// Teste une composante de chemin (sans extension vidéo).
    fn depuis_composante(c: &str) -> SourceMedia {
        for (s, re) in MOTIFS_SOURCE.iter() {
            if re.is_match(c) {
                return *s;
            }
        }
        SourceMedia::Inconnue
    }

    /// Détecte la source dans un chemin relatif : chaque composante est testée
    /// en remontant depuis le fichier. À défaut, repli sur l'extension
    /// (`m2ts`/`mts` ⇒ Blu-ray, `vob`/`ifo` ⇒ DVD).
    pub fn depuis_chemin(chemin_relatif: &str) -> SourceMedia {
        let norm = chemin_relatif.replace('\\', "/");
        let composantes: Vec<&str> = norm.split('/').filter(|c| !c.is_empty()).collect();
        let n = composantes.len();
        for (i, c) in composantes.iter().enumerate().rev() {
            let mut texte = (*c).to_string();
            if i == n - 1
                && let Some((base, ext)) = c.rsplit_once('.')
                && crate::scan::extensions::est_video(&ext.to_lowercase())
            {
                texte = base.to_string();
            }
            let s = Self::depuis_composante(&texte);
            if s != SourceMedia::Inconnue {
                return s;
            }
        }
        let ext = composantes
            .last()
            .and_then(|c| c.rsplit_once('.'))
            .map(|(_, e)| e.to_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "m2ts" | "mts" => SourceMedia::Bluray,
            "vob" | "ifo" => SourceMedia::Dvd,
            _ => SourceMedia::Inconnue,
        }
    }
}
