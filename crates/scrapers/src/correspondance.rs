//! Algorithmes de correspondance : nettoyage de requête, scores de similarité,
//! choix du meilleur résultat, détection d'identifiants dans un nom de fichier.

use std::sync::LazyLock;

use regex::Regex;

use crate::types::{OptionsRecherche, ResultatRecherche, cles_id};

fn re(motif: &str) -> Regex {
    Regex::new(motif).expect("expression régulière valide")
}

static RE_IMDB_ID: LazyLock<Regex> = LazyLock::new(|| re(r"^tt\d{6,}$"));
static RE_IMDB_DANS_TEXTE: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)(tt\d{6,})"));
static RE_IMDB_URL_ANCIENNE: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)imdb\.com/Title\?(\d{6,})"));
static RE_TMDB_TEXTE: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)(?:tmdbid|tmdb)[ ._=-]?(\d+)"));
static RE_TMDB_URL: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)themoviedb\.org/(?:movie|tv)/(\d+)"));
static RE_TVDB_TEXTE: LazyLock<Regex> = LazyLock::new(|| re(r"(?i)(?:tvdbid|tvdb)[ ._=-]?(\d+)"));
static RE_TVDB_URL: LazyLock<Regex> =
    LazyLock::new(|| re(r"(?i)thetvdb\.com/(?:movies|series)/(\d+)"));
static RE_ANNEE_FIN: LazyLock<Regex> = LazyLock::new(|| re(r"^(.*) \d{4}$"));
static RE_ANNEE_PARENTHESES: LazyLock<Regex> = LazyLock::new(|| re(r"\((\d{4})\)"));
static RE_TMDB_URL_REQUETE: LazyLock<Regex> =
    LazyLock::new(|| re(r"^https://www\.themoviedb\.org/movie/(\d+)"));

/// Vrai si `s` est un identifiant IMDb (`tt` + au moins 6 chiffres).
pub fn est_id_imdb(s: &str) -> bool {
    RE_IMDB_ID.is_match(s)
}

/// Détecte un identifiant IMDb dans un nom de dossier ou de fichier.
pub fn detecter_id_imdb(nom: &str) -> Option<String> {
    if let Some(c) = RE_IMDB_DANS_TEXTE.captures(nom) {
        return Some(c[1].to_lowercase());
    }
    RE_IMDB_URL_ANCIENNE
        .captures(nom)
        .map(|c| format!("tt{}", &c[1]))
}

/// Détecte un identifiant TMDB (`tmdbid-27205`, `tmdb=27205`, URL themoviedb.org).
pub fn detecter_id_tmdb(nom: &str) -> Option<u64> {
    let c = RE_TMDB_TEXTE
        .captures(nom)
        .or_else(|| RE_TMDB_URL.captures(nom))?;
    c[1].parse().ok()
}

/// Détecte un identifiant TheTVDB (`tvdbid-81189`, `tvdb 81189`, URL thetvdb.com).
pub fn detecter_id_tvdb(nom: &str) -> Option<u64> {
    let c = RE_TVDB_TEXTE
        .captures(nom)
        .or_else(|| RE_TVDB_URL.captures(nom))?;
    c[1].parse().ok()
}

/// Remplace par une espace les caractères `\ _ . : |`.
///
/// Comportement d'origine reproduit : les crochets ne sont pas remplacés.
pub fn nettoyer_caracteres(s: &str) -> String {
    s.chars()
        .map(|c| {
            if matches!(c, '\\' | '_' | '.' | ':' | '|') {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// « Matrix, The » -> « The Matrix » selon une liste de préfixes (articles).
pub fn retirer_nom_triable(titre: &str, prefixes: &[&str]) -> String {
    let t = titre.trim();
    let bas = t.to_lowercase();
    for p in prefixes {
        let suffixe = format!(", {}", p.to_lowercase());
        if bas.ends_with(&suffixe) && t.len() > suffixe.len() {
            // Les suffixes en minuscules ont la même longueur d'octets que l'original pour les articles courants.
            let coupe = t.len() - suffixe.len();
            if let (Some(reste), Some(prefixe)) = (t.get(..coupe), t.get(coupe + 2..)) {
                let sep = if p.ends_with('\'') || p.ends_with('`') {
                    ""
                } else {
                    " "
                };
                return format!("{prefixe}{sep}{reste}");
            }
        }
    }
    t.to_string()
}

/// Paires de lettres adjacentes, mot par mot (mot d'un caractère -> `"x "`).
fn paires_de_lettres(s: &str) -> Vec<String> {
    let mut paires = Vec::new();
    for mot in s.split_whitespace() {
        let c: Vec<char> = mot.chars().collect();
        if c.len() == 1 {
            paires.push(format!("{} ", c[0]));
        } else {
            for f in c.windows(2) {
                paires.push(f.iter().collect());
            }
        }
    }
    paires
}

/// Coefficient de Dice sur paires de lettres (0..=1).
pub fn dice(a: &str, b: &str) -> f32 {
    if a.to_lowercase() == b.to_lowercase() {
        return 1.0;
    }
    let p1 = paires_de_lettres(&a.to_uppercase());
    let mut p2 = paires_de_lettres(&b.to_uppercase());
    let total = (p1.len() + p2.len()) as f32;
    if total == 0.0 {
        return 0.0;
    }
    let mut communes = 0usize;
    for p in &p1 {
        if let Some(i) = p2.iter().position(|q| q == p) {
            p2.swap_remove(i);
            communes += 1;
        }
    }
    let s = 2.0 * communes as f32 / total;
    if s.is_nan() { 0.0 } else { s }
}

fn normaliser(s: &str) -> Vec<char> {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
        .chars()
        .collect()
}

/// Similarité de Jaro-Winkler (0..=1), insensible à la casse et aux espaces multiples.
pub fn jaro_winkler(a: &str, b: &str) -> f32 {
    let (a, b) = (normaliser(a), normaliser(b));
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let fenetre = (a.len().max(b.len()) / 2).saturating_sub(1);
    let mut pris_a = vec![false; a.len()];
    let mut pris_b = vec![false; b.len()];
    let mut m = 0usize;
    for (i, ca) in a.iter().enumerate() {
        let debut = i.saturating_sub(fenetre);
        let fin = (i + fenetre + 1).min(b.len());
        for j in debut..fin {
            if !pris_b[j] && b[j] == *ca {
                pris_a[i] = true;
                pris_b[j] = true;
                m += 1;
                break;
            }
        }
    }
    if m == 0 {
        return 0.0;
    }
    let mut transpositions = 0usize;
    let mut k = 0usize;
    for (i, ca) in a.iter().enumerate() {
        if pris_a[i] {
            while !pris_b[k] {
                k += 1;
            }
            if *ca != b[k] {
                transpositions += 1;
            }
            k += 1;
        }
    }
    let m = m as f32;
    let jaro =
        (m / a.len() as f32 + m / b.len() as f32 + (m - (transpositions / 2) as f32) / m) / 3.0;
    let prefixe = a
        .iter()
        .zip(b.iter())
        .take(4)
        .take_while(|(x, y)| x == y)
        .count() as f32;
    jaro + prefixe * 0.1 * (1.0 - jaro)
}

/// Score de similarité entre la requête et un titre (max de plusieurs mesures).
pub fn score_texte(requete: &str, titre: &str) -> f32 {
    if requete.is_empty() || titre.is_empty() {
        return 0.0;
    }
    let s1 = dice(requete, titre);
    let s2 = dice(requete, &nettoyer_caracteres(titre));
    let s3 = match RE_ANNEE_FIN.captures(requete) {
        Some(c) => dice(&c[1], titre),
        None => 0.0,
    };
    let s4 = jaro_winkler(requete, titre);
    s1.max(s2).max(s3).max(s4)
}

/// Pénalité liée à l'écart d'année.
pub fn penalite_annee(recherche: Option<i32>, resultat: i32) -> f32 {
    let Some(r) = recherche.filter(|r| *r > 1900) else {
        return 0.0;
    };
    if resultat == 0 {
        return 0.11;
    }
    let d = (r - resultat).abs();
    match d {
        0 => 0.0,
        d if d > 100 => 0.11,
        d => 0.01 + d as f32 / 1000.0,
    }
}

/// Calcule et enregistre le score d'un résultat. Un identifiant IMDb/TMDB
/// identique à celui demandé force le score à 1.0.
pub fn calculer_score(options: &OptionsRecherche, resultat: &mut ResultatRecherche) {
    let identique = |cle: &str| match (options.ids.get(cle), resultat.ids.get(cle)) {
        (Some(a), Some(b)) => !a.trim().is_empty() && a.trim().eq_ignore_ascii_case(b.trim()),
        _ => false,
    };
    if identique(cles_id::IMDB) || identique(cles_id::TMDB) {
        resultat.score = 1.0;
        return;
    }
    let q = options.requete.as_str();
    let mut s = score_texte(q, &resultat.titre).max(score_texte(q, &resultat.titre_original));
    let p = penalite_annee(options.annee, resultat.annee);
    if p > 0.0 {
        s -= p;
    }
    if resultat.url_affiche.is_empty() {
        s -= 0.01;
    }
    resultat.score = s;
}

/// Tri naturel : score décroissant, puis année croissante.
pub fn trier_resultats(resultats: &mut [ResultatRecherche]) {
    resultats.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.annee.cmp(&b.annee)));
}

/// Choisit le meilleur résultat d'une liste triée : `None` si ex æquo en tête
/// ou si le score est sous le seuil (0.75 par défaut côté films).
pub fn choisir_meilleur(resultats: &[ResultatRecherche], seuil: f32) -> Option<&ResultatRecherche> {
    let premier = resultats.first()?;
    if resultats.len() > 1 && (resultats[1].score - premier.score).abs() < f32::EPSILON {
        return None;
    }
    if premier.score < seuil {
        return None;
    }
    Some(premier)
}

/// Requête préparée à partir d'un titre saisi.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequetePreparee {
    pub requete: String,
    pub annee: Option<i32>,
    pub id_imdb: Option<String>,
    pub id_tmdb: Option<u64>,
}

impl RequetePreparee {
    /// Convertit en options de recherche (langue par défaut).
    pub fn en_options(&self) -> OptionsRecherche {
        let mut o = OptionsRecherche::avec_requete(self.requete.clone());
        o.annee = self.annee;
        if let Some(i) = &self.id_imdb {
            o.ids.insert(cles_id::IMDB.into(), i.clone());
        }
        if let Some(t) = self.id_tmdb {
            o.ids.insert(cles_id::TMDB.into(), t.to_string());
        }
        o
    }
}

/// Prépare la requête (spec §3.3) : détection `tt…`, `imdb:`, `tmdb:`, URLs,
/// et extraction d'un `(AAAA)` du titre.
pub fn preparer_requete(titre: &str, annee: Option<i32>) -> RequetePreparee {
    let bas = titre.trim().to_lowercase();
    let mut p = RequetePreparee {
        requete: titre.to_string(),
        annee: annee.filter(|a| *a > 0),
        ..Default::default()
    };
    if est_id_imdb(&bas) {
        p.id_imdb = Some(bas);
    } else if let Some(reste) = bas.strip_prefix("imdb:") {
        let reste = reste.trim();
        if est_id_imdb(reste) {
            p.id_imdb = Some(reste.to_string());
        }
    } else if let Some(reste) = bas.strip_prefix("https://www.imdb.com/title/") {
        let id = reste.split('/').next().unwrap_or("");
        if est_id_imdb(id) {
            p.id_imdb = Some(id.to_string());
        }
    } else if let Some(reste) = bas.strip_prefix("tmdb:") {
        p.id_tmdb = reste.trim().parse::<u64>().ok().filter(|n| *n > 0);
    } else if let Some(c) = RE_TMDB_URL_REQUETE.captures(&bas) {
        p.id_tmdb = c[1].parse().ok();
    }
    if let Some(c) = RE_ANNEE_PARENTHESES.captures(titre) {
        let sans = RE_ANNEE_PARENTHESES.replace(titre, "").trim().to_string();
        if !sans.is_empty() {
            p.requete = sans;
            p.annee = c[1].parse().ok();
        }
    }
    p
}

/// Titre et année déduits d'un nom de fichier ou de dossier (version simplifiée
/// de la logique complète : extension, `[...]`/`{...}`, séparateurs, mots
/// parasites courants, année).
pub fn titre_et_annee_depuis_nom(nom: &str) -> (String, Option<i32>) {
    static RE_EXT: LazyLock<Regex> = LazyLock::new(|| re(r"\.\w{2,4}$"));
    static RE_BLOCS: LazyLock<Regex> = LazyLock::new(|| re(r"\[[^\]]*\]|\{[^}]*\}"));
    const PARASITES: [&str; 40] = [
        "1080p",
        "720p",
        "480p",
        "2160p",
        "4k",
        "x264",
        "x265",
        "h264",
        "h265",
        "hevc",
        "bluray",
        "brrip",
        "bdrip",
        "remux",
        "webrip",
        "web-dl",
        "webdl",
        "dvdrip",
        "hdtv",
        "hdr",
        "hdr10",
        "dts",
        "truehd",
        "atmos",
        "aac",
        "ac3",
        "multi",
        "vostfr",
        "french",
        "truefrench",
        "repack",
        "proper",
        "uncut",
        "unrated",
        "remastered",
        "extended",
        "cd1",
        "cd2",
        "imax",
        "3d",
    ];
    let mut s = RE_EXT.replace(nom.trim(), "").into_owned();
    s = RE_BLOCS.replace_all(&s, " ").into_owned();
    let jetons: Vec<&str> = s
        .split(|c: char| c.is_whitespace() || matches!(c, '.' | '_' | '(' | ')' | ','))
        .filter(|j| !j.is_empty())
        .collect();
    let annee_max = 2100;
    let mut annee = None;
    let mut fin = jetons.len();
    for (i, j) in jetons.iter().enumerate().skip(1) {
        let bas = j.to_lowercase();
        if annee.is_none()
            && j.len() == 4
            && j.chars().all(|c| c.is_ascii_digit())
            && let Ok(n) = j.parse::<i32>()
            && n > 1800
            && n < annee_max
        {
            annee = Some(n);
            fin = fin.min(i);
            continue;
        }
        if PARASITES.contains(&bas.as_str()) || est_id_imdb(&bas) {
            fin = fin.min(i);
        }
    }
    let titre = jetons[..fin].join(" ");
    let titre = if titre.is_empty() {
        s.trim().to_string()
    } else {
        titre
    };
    (titre, annee)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn res(titre: &str, annee: i32, affiche: bool) -> ResultatRecherche {
        ResultatRecherche {
            titre: titre.into(),
            annee,
            url_affiche: if affiche {
                "http://x/a.jpg".into()
            } else {
                String::new()
            },
            ..Default::default()
        }
    }

    #[test]
    fn dice_cas_de_base() {
        assert_eq!(dice("Matrix", "matrix"), 1.0);
        assert_eq!(dice("abc", "xyz"), 0.0);
        // "FRANCE" / "FRENCH" : paires FR, RA/RE, AN/EN, NC/NC, CE/CH -> FR et NC communes = 2*2/10
        assert!((dice("France", "French") - 0.4).abs() < 1e-6);
        assert_eq!(dice("", ""), 1.0);
        assert_eq!(dice("", "a"), 0.0);
    }

    #[test]
    fn dice_mots_dun_caractere() {
        assert!((dice("a b", "a c") - 0.5).abs() < 1e-6);
    }

    #[test]
    fn jaro_winkler_valeurs_connues() {
        assert_eq!(jaro_winkler("abc", "ABC"), 1.0);
        assert!((jaro_winkler("MARTHA", "MARHTA") - 0.9611).abs() < 1e-3);
        assert!((jaro_winkler("DIXON", "DICKSONX") - 0.8133).abs() < 1e-3);
        assert_eq!(jaro_winkler("", ""), 1.0);
        assert_eq!(jaro_winkler("", "a"), 0.0);
        assert_eq!(jaro_winkler("abc", "xyz"), 0.0);
    }

    #[test]
    fn nettoyage_caracteres() {
        assert_eq!(nettoyer_caracteres("a.b_c:d|e\\f[g]"), "a b c d e f[g]");
    }

    #[test]
    fn penalites_annee() {
        assert_eq!(penalite_annee(None, 2000), 0.0);
        assert_eq!(penalite_annee(Some(1900), 2000), 0.0);
        assert_eq!(penalite_annee(Some(2010), 0), 0.11);
        assert_eq!(penalite_annee(Some(2010), 2010), 0.0);
        assert!((penalite_annee(Some(2010), 2012) - 0.012).abs() < 1e-6);
        assert_eq!(penalite_annee(Some(2010), 1900), 0.11);
    }

    #[test]
    fn score_exact_et_penalites() {
        let mut o = OptionsRecherche::avec_requete("Inception");
        o.annee = Some(2010);
        let mut r = res("Inception", 2010, true);
        calculer_score(&o, &mut r);
        assert!((r.score - 1.0).abs() < 1e-6);
        let mut r2 = res("Inception", 2012, false);
        calculer_score(&o, &mut r2);
        assert!((r2.score - (1.0 - 0.012 - 0.01)).abs() < 1e-5);
    }

    #[test]
    fn score_force_par_id() {
        let o = OptionsRecherche::avec_requete("zzz").avec_id("tmdb", "27205");
        let mut r = res("Inception", 2010, true);
        r.ids.insert("tmdb".into(), "27205".into());
        calculer_score(&o, &mut r);
        assert_eq!(r.score, 1.0);
    }

    #[test]
    fn score_annee_en_fin_de_requete() {
        assert!(score_texte("Inception 2010", "Inception") > 0.99);
    }

    #[test]
    fn choix_du_meilleur() {
        let mut v = vec![res("A", 2000, true), res("B", 2001, true)];
        v[0].score = 0.9;
        v[1].score = 0.8;
        assert_eq!(choisir_meilleur(&v, 0.75).unwrap().titre, "A");
        assert!(choisir_meilleur(&v, 0.95).is_none());
        v[1].score = 0.9;
        assert!(choisir_meilleur(&v, 0.75).is_none());
        assert!(choisir_meilleur(&[], 0.75).is_none());
    }

    #[test]
    fn tri_score_puis_annee() {
        let mut v = vec![
            res("A", 2005, true),
            res("B", 1999, true),
            res("C", 2000, true),
        ];
        v[0].score = 0.5;
        v[1].score = 0.5;
        v[2].score = 0.9;
        trier_resultats(&mut v);
        let noms: Vec<_> = v.iter().map(|r| r.titre.as_str()).collect();
        assert_eq!(noms, ["C", "B", "A"]);
    }

    #[test]
    fn detection_identifiants() {
        assert_eq!(
            detecter_id_imdb("Film (2010) {imdb-tt1375666}").as_deref(),
            Some("tt1375666")
        );
        assert_eq!(
            detecter_id_imdb("https://www.imdb.com/Title?1375666").as_deref(),
            Some("tt1375666")
        );
        assert_eq!(detecter_id_imdb("Film (2010)"), None);
        assert_eq!(detecter_id_tmdb("Inception [tmdbid-27205]"), Some(27205));
        assert_eq!(detecter_id_tmdb("Inception tmdb=27205"), Some(27205));
        assert_eq!(
            detecter_id_tmdb("https://www.themoviedb.org/movie/27205-inception"),
            Some(27205)
        );
        assert_eq!(detecter_id_tvdb("Serie {tvdb-81189}"), Some(81189));
        assert_eq!(
            detecter_id_tvdb("https://thetvdb.com/series/81189"),
            Some(81189)
        );
        assert_eq!(detecter_id_tvdb("Serie"), None);
    }

    #[test]
    fn preparation_requete() {
        let p = preparer_requete("TT1375666", None);
        assert_eq!(p.id_imdb.as_deref(), Some("tt1375666"));
        let p = preparer_requete("imdb:tt1375666", None);
        assert_eq!(p.id_imdb.as_deref(), Some("tt1375666"));
        let p = preparer_requete("https://www.imdb.com/title/tt1375666/", None);
        assert_eq!(p.id_imdb.as_deref(), Some("tt1375666"));
        let p = preparer_requete("tmdb:27205", None);
        assert_eq!(p.id_tmdb, Some(27205));
        let p = preparer_requete("https://www.themoviedb.org/movie/27205-inception", None);
        assert_eq!(p.id_tmdb, Some(27205));
        let p = preparer_requete("Inception (2010)", None);
        assert_eq!((p.requete.as_str(), p.annee), ("Inception", Some(2010)));
        let p = preparer_requete("(2010)", None);
        assert_eq!(p.requete, "(2010)");
        assert_eq!(p.en_options().annee, None);
    }

    #[test]
    fn nom_triable() {
        let pre = ["The", "Le", "L'"];
        assert_eq!(retirer_nom_triable("Matrix, The", &pre), "The Matrix");
        assert_eq!(retirer_nom_triable("Aventure, L'", &pre), "L'Aventure");
        assert_eq!(retirer_nom_triable("Inception", &pre), "Inception");
    }

    #[test]
    fn nom_de_fichier() {
        assert_eq!(
            titre_et_annee_depuis_nom("Inception.2010.1080p.BluRay.x264.mkv"),
            ("Inception".into(), Some(2010))
        );
        assert_eq!(
            titre_et_annee_depuis_nom("Le Fabuleux Destin (2001) [tmdbid-194].avi"),
            ("Le Fabuleux Destin".into(), Some(2001))
        );
        assert_eq!(titre_et_annee_depuis_nom("Heat"), ("Heat".into(), None));
        assert_eq!(
            titre_et_annee_depuis_nom("2001.A.Space.Odyssey.1968.mkv"),
            ("2001 A Space Odyssey".into(), Some(1968))
        );
    }
}
