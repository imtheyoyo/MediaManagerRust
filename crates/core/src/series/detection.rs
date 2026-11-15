//! Détection de la saison et des épisodes dans un nom de fichier ou un chemin
//! relatif au dossier de la série.
//!
//! Point d'entrée : [`detecter_episodes`]. Fonctions pures, sans accès disque.

use std::sync::LazyLock;

use regex::Regex as RegexSimple;

use super::motifs::*;
use crate::scan::extensions::est_extension_connue;
use crate::scan::pile::{marqueur_pile, nettoyer_marqueur_pile};
use crate::scan::types::est_nom_dossier_disque;

/// Écart maximal entre le premier et le dernier épisode d'une plage
/// (`S01E01-E06` est accepté, `S01E01-E07` ne l'est pas).
pub const ETENDUE_MAX_PLAGE: i32 = 5;

/// Écart maximal entre deux épisodes successifs d'un multi-épisodes
/// (`S01E01E02`) : au-delà, le nombre est ignoré (année, résolution…).
const ECART_MAX_MULTI: i32 = 20;

/// Résultat de la détection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultatDetection {
    /// Saison détectée, `-1` si inconnue (0 = spéciaux).
    pub saison: i32,
    /// Épisodes détectés, triés et sans doublon (vide si aucun).
    pub episodes: Vec<i32>,
    /// Nom de base analysé (sans extension ni mots techniques).
    pub nom: String,
    /// Titre d'épisode déduit du nom (vide si rien d'exploitable).
    pub nom_nettoye: String,
    /// Date de diffusion trouvée dans le nom (`aaaa-mm-jj`).
    pub date: Option<String>,
    /// Un marqueur de pile (`CD1`, `part2`…) figure dans le nom du fichier.
    pub marqueur_pile_trouve: bool,
}

impl Default for ResultatDetection {
    fn default() -> Self {
        Self {
            saison: -1,
            episodes: Vec::new(),
            nom: String::new(),
            nom_nettoye: String::new(),
            date: None,
            marqueur_pile_trouve: false,
        }
    }
}

impl ResultatDetection {
    /// Au moins un épisode détecté.
    pub fn a_des_episodes(&self) -> bool {
        !self.episodes.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Utilitaires de chaînes
// ---------------------------------------------------------------------------

/// Séparateurs `/`, sans `/` initial ni final.
fn normaliser_chemin(nom: &str) -> String {
    nom.replace('\\', "/").trim_matches('/').to_string()
}

/// Dernier composant d'un chemin normalisé.
fn nom_final(chemin: &str) -> &str {
    chemin.rsplit_once('/').map_or(chemin, |(_, n)| n)
}

/// `(dossier avec « / » final, nom)`.
fn separer(chemin: &str) -> (&str, &str) {
    match chemin.rfind('/') {
        Some(i) => (&chemin[..=i], &chemin[i + 1..]),
        None => ("", chemin),
    }
}

/// Retire une extension connue (vidéo, sous-titres, image, nfo…).
///
/// Écart : la spécification retire tout `\.\w{1,4}$` ; on se limite aux
/// extensions connues pour ne pas perdre `Show.E05` ou `Show.102`.
fn retirer_extension(chemin: &str) -> String {
    let (dossier, nom) = separer(chemin);
    match nom.rsplit_once('.') {
        Some((base, ext))
            if !base.is_empty()
                && (1..=5).contains(&ext.chars().count())
                && est_extension_connue(&ext.to_lowercase()) =>
        {
            format!("{dossier}{base}")
        }
        _ => chemin.to_string(),
    }
}

/// Retire la plage `[debut, fin)` d'un texte en la remplaçant par un espace.
fn retirer_plage(texte: &str, debut: usize, fin: usize) -> String {
    format!("{} {}", &texte[..debut], &texte[fin..])
}

/// Chemin ramené au dossier de l'épisode pour les structures de disque :
/// `S01E03/VIDEO_TS/VTS_01_1.VOB` ⇒ `S01E03`, `Ep/BDMV/STREAM/00001.m2ts` ⇒ `Ep`.
/// Renvoie aussi un indicateur « fichier disque ».
fn reduire_disque(chemin: &str) -> (String, bool) {
    let composants: Vec<&str> = chemin.split('/').collect();
    if let Some(i) = composants.iter().position(|c| est_nom_dossier_disque(c)) {
        return (composants[..i].join("/"), true);
    }
    let nom = nom_final(chemin);
    if FICHIER_DISQUE.is_match(nom).unwrap_or(false) {
        let (dossier, _) = separer(chemin);
        return (dossier.trim_end_matches('/').to_string(), true);
    }
    (chemin.to_string(), false)
}

/// Retire la résolution (`1920x1080`) puis les mots techniques.
pub(crate) fn retirer_mots_arret(texte: &str) -> String {
    let sans_resolution = RESOLUTION.replace_all(texte, "");
    RE_MOTS_ARRET
        .replace_all(&sans_resolution, "$1")
        .into_owned()
}

/// Date (`aaaa-mm-jj`, `aaaa.mm.jj`, `jj-mm-aaaa`, `jj.mm.aaaa`) : renvoie le
/// texte sans la date et la date normalisée.
fn extraire_date(texte: &str) -> (String, Option<(String, i32)>) {
    // (motif, indices des groupes année / mois / jour)
    type Essai = (&'static LazyLock<fancy_regex::Regex>, [usize; 3]);
    let essais: [Essai; 2] = [(&DATE_1, [1, 2, 3]), (&DATE_2, [3, 2, 1])];
    for (re, [ia, im, ij]) in essais {
        for c in re.captures_iter(texte).flatten() {
            let (Some(a), Some(m), Some(j)) = (
                groupe_entier(&c, ia),
                groupe_entier(&c, im),
                groupe_entier(&c, ij),
            ) else {
                continue;
            };
            if (1..=12).contains(&m) && (1..=31).contains(&j) && (1800..=2200).contains(&a) {
                let g = c.get(0).expect("groupe 0");
                let reste = retirer_plage(texte, g.start(), g.end());
                return (reste, Some((format!("{a:04}-{m:02}-{j:02}"), a)));
            }
        }
    }
    (texte.to_string(), None)
}

/// Convertit des chiffres romains (MDCLXVI, insensible à la casse).
pub fn decoder_romain(texte: &str) -> Option<i32> {
    let mut total = 0i32;
    let mut precedent = 0i32;
    if texte.is_empty() {
        return None;
    }
    for c in texte.chars().rev() {
        let v = match c.to_ascii_uppercase() {
            'I' => 1,
            'V' => 5,
            'X' => 10,
            'L' => 50,
            'C' => 100,
            'D' => 500,
            'M' => 1000,
            _ => return None,
        };
        if v < precedent {
            total -= v;
        } else {
            total += v;
            precedent = v;
        }
    }
    (total > 0).then_some(total)
}

/// Motif insensible à la casse correspondant au nom de la série, mots joints
/// par des séparateurs quelconques. Avec et sans année finale.
fn motifs_nom_serie(nom_serie: &str) -> Vec<RegexSimple> {
    static ANNEE_FINALE: LazyLock<RegexSimple> =
        LazyLock::new(|| RegexSimple::new(r"[\s(\[]*\d{4}[)\]]?\s*$").unwrap());
    let mut variantes = vec![nom_serie.trim().to_string()];
    let sans_annee = ANNEE_FINALE.replace(nom_serie.trim(), "").into_owned();
    if !sans_annee.is_empty() && sans_annee != variantes[0] {
        variantes.push(sans_annee);
    }
    variantes
        .iter()
        .filter_map(|v| {
            let mots: Vec<String> = v
                .split(|c: char| !c.is_alphanumeric())
                .filter(|m| !m.is_empty())
                .map(regex::escape)
                .collect();
            if mots.is_empty() {
                return None;
            }
            RegexSimple::new(&format!(
                r"(?i)(?:^|[^\pL\pN]){}(?:$|[^\pL\pN])",
                mots.join(r"[^\pL\pN]*")
            ))
            .ok()
        })
        .collect()
}

/// Retire le nom de la série d'un texte (première occurrence).
fn retirer_nom_serie(texte: &str, nom_serie: Option<&str>) -> String {
    let Some(nom) = nom_serie.filter(|n| !n.trim().is_empty()) else {
        return texte.to_string();
    };
    for re in motifs_nom_serie(nom) {
        if let Some(m) = re.find(texte) {
            return retirer_plage(texte, m.start(), m.end());
        }
    }
    texte.to_string()
}

// ---------------------------------------------------------------------------
// Analyseurs élémentaires
// ---------------------------------------------------------------------------

/// Épisodes d'une chaîne `E01E02`, `x01x02`, `E01-02`… via `EPISODE_PATTERN`.
/// Les épisodes suivants doivent croître (écart borné) ; `zero_permis`
/// autorise l'épisode 0.
fn episodes_suite(texte: &str, zero_permis: bool) -> Vec<i32> {
    let mut v: Vec<i32> = Vec::new();
    for c in EPISODE_PATTERN.captures_iter(texte).flatten() {
        let Some(ep) = groupe_entier(&c, 1) else {
            continue;
        };
        if ep == 0 && !zero_permis {
            continue;
        }
        if let Some(&dernier) = v.last()
            && (ep <= dernier || ep - dernier > ECART_MAX_MULTI)
        {
            break;
        }
        v.push(ep);
    }
    v
}

/// 1. `S01E01-E03` ⇒ (1, [1, 2, 3]).
fn plage_saison_episodes(texte: &str) -> Option<(i32, Vec<i32>)> {
    let c = captures(&SEASON_EP_RANGE, texte)?;
    let s = groupe_entier(&c, 1)?;
    let debut = groupe_entier(&c, 2)?;
    let fin = groupe_entier(&c, 3)?;
    (fin > debut && fin - debut <= ETENDUE_MAX_PLAGE).then(|| (s, (debut..=fin).collect()))
}

/// 2. `S01E01E02`, `S01E01-02`, `S01.E02`, `S01 E02` (épisode 0 permis).
///
/// Écart : le premier séparateur doit contenir `e`, `p` ou `x` (`S01.2009`
/// n'est pas l'épisode 2009).
fn multi_episodes(texte: &str) -> Option<(i32, Vec<i32>)> {
    for c in SEASON_MULTI_EP.captures_iter(texte).flatten() {
        let (Some(s), Some(g)) = (groupe_entier(&c, 1), c.get(2)) else {
            continue;
        };
        let prefixe: String = g
            .as_str()
            .chars()
            .take_while(|c| !c.is_ascii_digit())
            .collect();
        if !prefixe
            .chars()
            .any(|c| matches!(c, 'e' | 'E' | 'p' | 'P' | 'x' | 'X'))
        {
            continue;
        }
        let eps = episodes_suite(g.as_str(), true);
        if !eps.is_empty() {
            return Some((s, eps));
        }
    }
    None
}

/// 3. `1x01`, `1x01x02` (épisode > 0).
fn multi_episodes_2(texte: &str) -> Option<(i32, Vec<i32>)> {
    for c in SEASON_MULTI_EP_2.captures_iter(texte).flatten() {
        let (Some(s), Some(g)) = (groupe_entier(&c, 1), c.get(2)) else {
            continue;
        };
        let eps = episodes_suite(g.as_str(), false);
        if !eps.is_empty() {
            return Some((s, eps));
        }
    }
    None
}

/// `1⧸2` : épisode 1 sur 2 (valide si épisode ≤ total).
fn episode_sur_total(texte: &str) -> Option<i32> {
    let c = captures(&EPISODE_PATTERN_NR, texte)?;
    let ep = groupe_entier(&c, 1)?;
    let total = groupe_entier(&c, 2)?;
    (ep > 0 && ep <= total).then_some(ep)
}

/// Saison et épisode d'un jeton de 4 chiffres, 3 chiffres, 2 ou 1 chiffre,
/// en parcourant les jetons de la fin vers le début.
fn nombres_seuls(texte: &str, saison_connue: i32) -> Option<(i32, i32)> {
    let sans_blocs = BLOCS.replace_all(texte, " ").into_owned();
    let avec_contenu: String = texte
        .chars()
        .map(|c| if "[]{}".contains(c) { ' ' } else { c })
        .collect();
    for t in [sans_blocs, avec_contenu] {
        if let Some(r) = nombres_dans_jetons(&t, saison_connue) {
            return Some(r);
        }
    }
    None
}

fn nombres_dans_jetons(texte: &str, saison_connue: i32) -> Option<(i32, i32)> {
    let jetons: Vec<&str> = texte
        .split(|c: char| c.is_whitespace() || "|_.-()".contains(c))
        .filter(|j| !j.is_empty() && j.chars().all(|c| c.is_ascii_digit()))
        .collect();
    let inverses = || jetons.iter().rev();
    // SSEE : la saison doit coïncider avec la saison déjà connue.
    if saison_connue >= 0 {
        for j in inverses().filter(|j| j.len() == 4) {
            let (s, e) = (j[..2].parse::<i32>().ok()?, j[2..].parse::<i32>().ok()?);
            if s == saison_connue {
                return Some((s, e));
            }
        }
    }
    // SEE : saison = premier chiffre.
    for j in inverses().filter(|j| j.len() == 3) {
        let (s, e) = (j[..1].parse::<i32>().ok()?, j[1..].parse::<i32>().ok()?);
        if saison_connue < 0 || s == saison_connue {
            return Some((s, e));
        }
    }
    for longueur in [2, 1] {
        if let Some(j) = inverses().find(|j| j.len() == longueur) {
            return Some((saison_connue, j.parse().ok()?));
        }
    }
    None
}

/// Saison seule (`S2`, ` s 3`) : renvoie la saison et le texte sans elle.
fn saison_seule(texte: &str) -> Option<(i32, String)> {
    let entoure = format!(" {} ", texte.replace('/', " / "));
    let c = captures(&SEASON_ONLY, &entoure)?;
    let s = groupe_entier(&c, 1)?;
    let g = c.get(0)?;
    Some((s, retirer_plage(&entoure, g.start(), g.end())))
}

// ---------------------------------------------------------------------------
// Animés
// ---------------------------------------------------------------------------

/// Épisodes d'une suite `-N` / `eN` après un premier épisode (plage ≤ 5).
fn suite_anime(premier: i32, suite: &str) -> Vec<i32> {
    let mut eps = vec![premier];
    for m in JETON_SUITE.find_iter(suite).flatten() {
        let jeton = m.as_str().trim_start_matches([' ', '_', '.']);
        let dernier = *eps.last().expect("non vide");
        if let Some(c) = captures(&SUITE_PLAGE, jeton) {
            if let Some(fin) = groupe_entier(&c, 1)
                && fin > dernier
                && fin - premier <= ETENDUE_MAX_PLAGE
            {
                eps.extend(dernier + 1..=fin);
            }
        } else if let Some(c) = captures(&NOMBRE, jeton)
            && let Some(n) = groupe_entier(&c, 1)
            && n > dernier
        {
            eps.push(n);
        }
    }
    eps
}

/// Interprète une correspondance anime selon son numéro de motif (1 à 4).
fn interpreter_anime(
    numero: u8,
    c: &fancy_regex::Captures<'_>,
    nom_fichier: &str,
) -> Option<(i32, Vec<i32>)> {
    match numero {
        1 => Some((0, vec![groupe_entier(c, 2)?])),
        2 => {
            let s = c.get(1)?.as_str();
            let saison = if s.eq_ignore_ascii_case("specials") {
                0
            } else {
                s.parse().ok()?
            };
            let ep = groupe_entier(c, 2)?;
            if let Some(sxe) = captures(&SXXEXX, nom_fichier) {
                return Some((groupe_entier(&sxe, 1)?, vec![groupe_entier(&sxe, 2)?]));
            }
            Some((saison, vec![ep]))
        }
        3 => {
            let s = groupe_entier(c, 1)?;
            let ep = groupe_entier(c, 2)?;
            let suite = c.get(3).map_or("", |m| m.as_str());
            Some((s, suite_anime(ep, suite)))
        }
        _ => {
            // `01-02-03` : tous les nombres de la suite.
            let g0 = c.get(0)?;
            let fin_ep = [c.get(2), c.get(3)]
                .into_iter()
                .flatten()
                .map(|m| m.end())
                .max()?;
            let portion = &nom_fichier_ou(c, g0.start(), fin_ep);
            if let Some(m) = captures(&ANIME_PREPEND4_2, portion) {
                let eps: Vec<i32> = m
                    .get(1)?
                    .as_str()
                    .split('-')
                    .filter_map(|n| n.parse().ok())
                    .collect();
                if !eps.is_empty() {
                    return Some((1, eps));
                }
            }
            Some((1, vec![groupe_entier(c, 2)?]))
        }
    }
}

/// Sous-chaîne du texte analysé (l'objet `Captures` ne l'expose pas : on la
/// reconstruit à partir du groupe 0).
fn nom_fichier_ou(c: &fancy_regex::Captures<'_>, debut: usize, fin: usize) -> String {
    let g0 = c.get(0).expect("groupe 0");
    let base = g0.start();
    let texte = g0.as_str();
    let (d, f) = (debut - base, (fin - base).min(texte.len()));
    texte.get(d..f).unwrap_or("").to_string()
}

fn essayer_motifs_anime(
    chemin: &str,
    motifs: &[(u8, &LazyLock<fancy_regex::Regex>)],
) -> Option<ResultatDetection> {
    let nom_fichier = nom_final(chemin);
    for (numero, re) in motifs {
        // Les motifs 1, 3 et 4 portent sur le nom du fichier ; le 2 a besoin
        // du dossier de saison.
        let texte = if *numero == 2 { chemin } else { nom_fichier };
        if let Some(c) = captures(re, texte)
            && let Some((saison, mut eps)) = interpreter_anime(*numero, &c, nom_fichier)
        {
            eps.sort_unstable();
            eps.dedup();
            return Some(ResultatDetection {
                saison,
                episodes: eps,
                ..ResultatDetection::default()
            });
        }
    }
    None
}

/// Motifs animés exigeant un hash `[ABCD1234]`.
fn analyser_anime_exclusif(chemin: &str) -> Option<ResultatDetection> {
    essayer_motifs_anime(
        chemin,
        &[
            (1, &ANIME_PREPEND1),
            (2, &ANIME_PREPEND2),
            (3, &ANIME_PREPEND3),
            (4, &ANIME_PREPEND4),
        ],
    )
}

/// Motifs animés sans hash (spéciaux, dossier de saison, `S2 - 05`).
fn analyser_anime_sans_hash(chemin: &str) -> Option<ResultatDetection> {
    essayer_motifs_anime(
        chemin,
        &[
            (1, &ANIME_APPEND1),
            (2, &ANIME_APPEND2),
            (3, &ANIME_APPEND3),
        ],
    )
}

// ---------------------------------------------------------------------------
// Détection
// ---------------------------------------------------------------------------

/// Détection sur un chemin (nom seul ou chemin relatif complet).
fn detecter(chemin: &str, nom_serie: Option<&str>, est_disque: bool) -> ResultatDetection {
    let mut r = ResultatDetection::default();
    let nom_brut = nom_final(chemin);
    let nettoye = retirer_mots_arret(chemin);
    let (dossier, base) = separer(&nettoye);
    let base = retirer_extension(base);
    let base = ANNEE_ENCADREE.replace_all(&base, " ").into_owned();
    let base = HASH_8.replace_all(&base, " ").into_owned();
    r.nom = base.trim().to_string();

    // 1 à 3 : formes saison + épisodes explicites, sur le nom du fichier.
    if let Some((s, eps)) = plage_saison_episodes(&base)
        .or_else(|| multi_episodes(&base))
        .or_else(|| multi_episodes_2(&base))
    {
        r.saison = s;
        r.episodes = eps;
        return finaliser(r, nom_brut, nom_serie);
    }

    // 4 : « Season 3 », « Staffel 2 »… sur dossier + nom.
    let mut texte = base.clone();
    let complet = format!("{dossier}{base}");
    if let Some(c) = captures(&SEASON_LONG, &complet)
        && let Some(s) = groupe_entier(&c, 2)
    {
        r.saison = s;
        let g = c.get(0).expect("groupe 0");
        if g.start() >= dossier.len() {
            texte = retirer_plage(&base, g.start() - dossier.len(), g.end() - dossier.len());
        }
        if let Some(ep) = episode_sur_total(&texte) {
            r.episodes = vec![ep];
        } else if let Some(c) = captures(&EPISODE_PATTERN_2, &texte)
            && let Some(ep) = groupe_entier(&c, 1)
        {
            r.episodes = vec![ep];
        }
    }

    // 5 : saison seule sur le dossier (`S2/`).
    if r.saison == -1
        && !dossier.is_empty()
        && let Some((s, _)) = saison_seule(dossier)
    {
        r.saison = s;
    }
    if r.a_des_episodes() {
        return finaliser(r, nom_brut, nom_serie);
    }

    // Replis : nom de la série retiré, chiffres romains, saison seule,
    // épisode seul, puis nombres seuls.
    texte = retirer_nom_serie(&texte, nom_serie);
    if let Some(c) = captures(&ROMAN_PATTERN, &texte)
        && let Some(ep) = c.get(2).and_then(|m| decoder_romain(m.as_str()))
    {
        r.episodes = vec![ep];
        return finaliser(r, nom_brut, nom_serie);
    }
    if est_disque {
        return r;
    }
    if r.saison == -1
        && let Some((s, reste)) = saison_seule(&texte)
    {
        r.saison = s;
        texte = reste;
    }
    let entoure = format!(" {texte} ");
    if let Some(c) = captures(&EPISODE_ONLY, &entoure)
        && let Some(ep) = groupe_entier(&c, 1)
    {
        r.episodes = vec![ep];
    } else if let Some(ep) = episode_sur_total(&texte) {
        r.episodes = vec![ep];
    } else if let Some(c) = captures(&EPISODE_PATTERN_2, &texte)
        && let Some(ep) = groupe_entier(&c, 1)
    {
        r.episodes = vec![ep];
    } else if let Some((s, ep)) = nombres_seuls(&texte, r.saison) {
        r.saison = s;
        r.episodes = vec![ep];
    }
    if r.a_des_episodes() {
        return finaliser(r, nom_brut, nom_serie);
    }
    r
}

/// Tri, dédoublonnage et titre nettoyé (`nom_brut` : nom de fichier
/// d'origine, mots techniques compris, pour couper le titre au bon endroit).
fn finaliser(
    mut r: ResultatDetection,
    nom_brut: &str,
    nom_serie: Option<&str>,
) -> ResultatDetection {
    r.episodes.sort_unstable();
    r.episodes.dedup();
    r.nom_nettoye = extraire_titre(nom_brut, nom_serie);
    r
}

/// Détecte saison et épisode(s) dans `nom` : nom de fichier, ou de préférence
/// chemin relatif au dossier de la série (`Season 2/Show - 05.mkv`), ce qui
/// permet d'exploiter les dossiers de saison. `nom_serie` sert à retirer le
/// titre de la série avant la recherche de nombres seuls (`The 100 - 105`).
///
/// Algorithme (spécification §3.3) :
/// 1. date (`aaaa-mm-jj`, `jj.mm.aaaa`) retirée et mémorisée ; extension retirée ;
/// 2. motifs animés avec hash `[ABCD1234]` (prioritaires) ;
/// 3. détection sur le nom du fichier seul ; si la saison manque, détection sur
///    le chemin complet (dossier de saison) puis motifs animés sans hash ;
/// 4. rien trouvé et pas de date : détection sur le chemin complet ;
/// 5. épisodes sans saison ⇒ saison 1 ; date sans saison ⇒ saison = année
///    (épisodes vides : « épisode -1 »).
pub fn detecter_episodes(nom: &str, nom_serie: Option<&str>) -> ResultatDetection {
    let normalise = normaliser_chemin(nom);
    let marqueur = !marqueur_pile(nom_final(&normalise)).is_empty();
    let (chemin, est_disque) = reduire_disque(&normalise);
    let (chemin, date) = extraire_date(&chemin);
    let chemin = retirer_extension(chemin.trim());

    if let Some(mut r) = analyser_anime_exclusif(&chemin) {
        r.nom = nom_final(&chemin).trim().to_string();
        r.nom_nettoye = extraire_titre(nom_final(&chemin), nom_serie);
        r.marqueur_pile_trouve = marqueur;
        r.date = date.map(|(d, _)| d);
        return r;
    }

    let mut r = detecter(nom_final(&chemin), nom_serie, est_disque);
    if r.a_des_episodes() && r.saison == -1 && chemin.contains('/') {
        let r2 = detecter(&chemin, nom_serie, est_disque);
        if r2.saison != -1 && r2.episodes.len() == r.episodes.len() {
            r = r2;
        } else if let Some(mut a) = analyser_anime_sans_hash(&chemin) {
            a.nom = r.nom.clone();
            a.nom_nettoye = r.nom_nettoye.clone();
            r = a;
        } else {
            r.saison = r2.saison;
        }
    }
    if !r.a_des_episodes() && date.is_none() && chemin.contains('/') {
        r = detecter(&chemin, nom_serie, est_disque);
    }
    if r.a_des_episodes() && r.saison == -1 {
        r.saison = 1;
    }
    if let Some((d, annee)) = date {
        if r.saison == -1 {
            r.saison = annee;
        }
        r.date = Some(d);
    }
    r.marqueur_pile_trouve = marqueur;
    r
}

// ---------------------------------------------------------------------------
// Titre d'épisode
// ---------------------------------------------------------------------------

static VARIANTES_EPISODE: LazyLock<Vec<fancy_regex::Regex>> = LazyLock::new(|| {
    [
        r"(?i)\bs\d{1,4}[ _.]?(?:[epx.-]+\d{1,4})+",
        r"(?i)\b\d{1,4}x\d{1,4}(?:[x-]\d{1,4})*\b",
        r"(?i)\b(?:episode|ep|e)[ ._-]?\d{1,4}\b",
        r"(?i)\b(?:season|saison|staffel|series)[ ._-]?\d{1,4}\b",
        r"(?i)\bs\d{1,4}\b",
        r"\d{1,2}[⧸⁄]\d{1,2}",
    ]
    .iter()
    .map(|m| rx(m))
    .collect()
});

/// Titre d'épisode extrait d'un nom de base (vide si rien d'exploitable).
pub(crate) fn extraire_titre(nom: &str, nom_serie: Option<&str>) -> String {
    let mut s = nettoyer_marqueur_pile(&retirer_extension(nom_final(&normaliser_chemin(nom))));
    if let Some(m) = RE_COUPURE_TITRE.find(&s).ok().flatten() {
        s.truncate(m.start());
    }
    s = extraire_date(&s).0;
    s = ANNEE_ENCADREE.replace_all(&s, " ").into_owned();
    s = BLOCS.replace_all(&s, " ").into_owned();
    s = HASH_8.replace_all(&s, " ").into_owned();
    // Noms de release : « . » sert de séparateur s'il n'y a pas d'espace.
    let points_separateurs = !s.trim().contains(' ') || s.matches('.').count() >= 2;
    s = retirer_nom_serie(&s, nom_serie);
    // Première occurrence de chaque variante (« 3x07 - The 4x4 Car » garde 4x4).
    for re in VARIANTES_EPISODE.iter() {
        s = re.replacen(&s, 1, " - ").into_owned();
    }
    s = s.replace('_', " ");
    if points_separateurs {
        s = s.replace('.', " ");
    }
    // Segments séparés par « - » : on retire ceux qui sont vides ou numériques.
    let mut segments: Vec<String> = s
        .split(" - ")
        .map(|seg| seg.split_whitespace().collect::<Vec<_>>().join(" "))
        .map(|seg| {
            seg.trim_matches(|c: char| " -–_.,:".contains(c))
                .to_string()
        })
        .filter(|seg| !seg.is_empty() && !seg.chars().all(|c| c.is_ascii_digit()))
        .collect();
    // Numéro isolé en fin de titre (« Mars 05 ») : c'est l'épisode.
    if let Some(dernier) = segments.last_mut()
        && let Some((avant, apres)) = dernier.rsplit_once(' ')
        && apres.len() <= 4
        && apres.chars().all(|c| c.is_ascii_digit())
    {
        *dernier = avant.to_string();
    }
    segments.join(" - ")
}

/// Nettoie un titre d'épisode tiré d'un nom de fichier : retire le dossier,
/// l'extension, les mots techniques, années, hash, le nom de la série et les
/// variantes saison/épisode. Si le résultat est vide, le titre d'origine est
/// conservé.
pub fn nettoyer_titre_episode(titre: &str, nom_serie: Option<&str>) -> String {
    let t = extraire_titre(titre, nom_serie);
    if t.is_empty() { titre.to_string() } else { t }
}

/// Saison d'un fichier annexe (artwork de saison) d'après son nom puis son
/// dossier (`dossier` : nom du dossier parent, vide à la racine de la série).
///
/// `season-specials*` ou dossier `specials` ⇒ 0 ; `season-all*` ⇒ -1 ;
/// `seasonNN` dans le nom, puis dans le dossier ; puis premier nombre du
/// dossier. `None` si rien n'est trouvé.
pub fn detecter_saison_fichier_dossier(nom_fichier: &str, dossier: &str) -> Option<i32> {
    let bas = nom_fichier.to_lowercase();
    if bas.starts_with("season-specials") || dossier.eq_ignore_ascii_case("specials") {
        return Some(0);
    }
    if bas.starts_with("season-all") {
        return Some(-1);
    }
    for texte in [nom_fichier, dossier] {
        if let Some(c) = captures(&SEASON_NUMBER, texte)
            && let Some(n) = groupe_entier(&c, 1)
        {
            return Some(n);
        }
    }
    if !dossier.is_empty()
        && let Some(c) = captures(&SEASON_FOLDER_NUMBER, dossier)
    {
        return groupe_entier(&c, 1);
    }
    None
}

/// Vrai si le nom de dossier désigne un dossier de saison (`Season 1`, `S01`,
/// `Staffel 2`, `Specials`).
pub fn est_dossier_saison(nom: &str) -> bool {
    nom.eq_ignore_ascii_case("specials")
        || SEASON_ONLY_PATTERN.is_match(nom.trim()).unwrap_or(false)
        || SEASON_LONG
            .find(nom)
            .ok()
            .flatten()
            .is_some_and(|m| m.start() == 0 && m.end() == nom.trim_end().len())
}
