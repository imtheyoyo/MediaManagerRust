//! Correspondance entre épisodes locaux et liste d'épisodes scrapés : choix
//! de l'ordre (diffusion, DVD, absolu…), appariement, épisodes manquants
//! (« fictifs »), application des métadonnées.
//!
//! Les épisodes scrapés sont décrits par un type local simple
//! ([`EpisodeScrape`]) pour ne pas dépendre des scrapers.

use std::collections::{BTreeMap, BTreeSet};

use crate::modele::{
    Episode, GroupeEpisodes, Identifiants, NumeroEpisode, Personne, Serie, TypeEpisode, TypeGroupe,
};

use super::organisation::{definir_groupe_actif, recalculer_saisons, trier_episodes};

/// Épisode tel que fourni par un scraper.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EpisodeScrape {
    /// Un numéro par ordre connu (diffusion, DVD, absolu…).
    pub numeros: Vec<NumeroEpisode>,
    pub titre: String,
    pub titre_original: String,
    pub resume: String,
    /// `aaaa-mm-jj`.
    pub premiere_diffusion: Option<String>,
    pub duree_minutes: u32,
    pub ids: Identifiants,
    pub invites: Vec<Personne>,
    pub equipe: Vec<Personne>,
}

impl EpisodeScrape {
    /// Numéro dans un ordre donné.
    pub fn numero_dans(&self, groupe: &GroupeEpisodes) -> Option<&NumeroEpisode> {
        self.numeros.iter().find(|n| &n.groupe == groupe)
    }

    /// `(saison, épisode)` valide dans un ordre donné.
    fn cle(&self, groupe: &GroupeEpisodes) -> Option<(i32, i32)> {
        self.numero_dans(groupe)
            .filter(|n| n.est_valide())
            .map(|n| (n.saison, n.episode))
    }
}

/// `(saison, épisode)` d'un épisode local dans l'ordre demandé, ou à défaut
/// dans l'ordre de diffusion (numéros issus du nom de fichier).
fn cle_locale(ep: &Episode, groupe: &GroupeEpisodes) -> Option<(i32, i32)> {
    ep.numero_dans(groupe)
        .filter(|n| n.est_valide())
        .or_else(|| {
            ep.numero_dans(&GroupeEpisodes::diffusion())
                .filter(|n| n.est_valide())
        })
        .map(|n| (n.saison, n.episode))
}

// ---------------------------------------------------------------------------
// Similarité de titres
// ---------------------------------------------------------------------------

/// Normalisation pour la comparaison : `[!?,._-]` ⇒ espace, minuscules,
/// espaces multiples réduits.
fn normaliser(s: &str) -> String {
    s.chars()
        .map(|c| if "!?,._-".contains(c) { ' ' } else { c })
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn levenshtein(a: &[char], b: &[char]) -> usize {
    let mut ligne: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diag = ligne[0];
        ligne[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let haut = ligne[j + 1];
            ligne[j + 1] = if ca == cb {
                diag
            } else {
                1 + diag.min(haut).min(ligne[j])
            };
            diag = haut;
        }
    }
    ligne[b.len()]
}

/// Similarité entre deux titres, de 0 (rien de commun) à 1 (identiques après
/// normalisation) : `1 - distance de Levenshtein / longueur max`.
pub fn similarite(a: &str, b: &str) -> f64 {
    let (a, b) = (normaliser(a), normaliser(b));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    let (ca, cb): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let max = ca.len().max(cb.len()) as f64;
    1.0 - levenshtein(&ca, &cb) as f64 / max
}

/// Texte local servant à la comparaison : titre, sinon nom du fichier.
fn texte_local(ep: &Episode) -> String {
    if ep.titre.is_empty() {
        crate::modele::fichier::nom_de_base(&ep.nom_fichier_origine)
    } else {
        ep.titre.clone()
    }
}

// ---------------------------------------------------------------------------
// Choix de l'ordre d'épisodes
// ---------------------------------------------------------------------------

/// Ordres candidats : ceux proposés, sinon ceux présents dans les numéros
/// scrapés (ordre de diffusion en tête).
fn groupes_candidats(
    scrapes: &[EpisodeScrape],
    groupes_proposes: &[GroupeEpisodes],
) -> Vec<GroupeEpisodes> {
    let mut v: Vec<GroupeEpisodes> = Vec::new();
    let source: Vec<&GroupeEpisodes> = if groupes_proposes.is_empty() {
        scrapes
            .iter()
            .flat_map(|s| s.numeros.iter().map(|n| &n.groupe))
            .collect()
    } else {
        groupes_proposes.iter().collect()
    };
    for g in source {
        if !v.contains(g) {
            v.push(g.clone());
        }
    }
    v.sort_by_key(|g| g.type_groupe != TypeGroupe::Diffusion);
    v
}

/// Choisit l'ordre d'épisodes qui explique le mieux les fichiers locaux.
///
/// Pour chaque ordre candidat, on compte les épisodes locaux dont le numéro
/// (détecté depuis le nom, ordre de diffusion) existe dans cet ordre ;
/// l'ordre au compte maximal l'emporte, départagé par la similarité des
/// titres (titre local ou nom de fichier ↔ titre scrapé), puis par la
/// position dans la liste (diffusion d'abord). `None` si aucun épisode ne
/// correspond dans aucun ordre.
pub fn choisir_meilleur_groupe(
    locaux: &[Episode],
    scrapes: &[EpisodeScrape],
    groupes_proposes: &[GroupeEpisodes],
) -> Option<GroupeEpisodes> {
    let diffusion = GroupeEpisodes::diffusion();
    let mut meilleur: Option<(usize, f64, GroupeEpisodes)> = None;
    for g in groupes_candidats(scrapes, groupes_proposes) {
        let index: BTreeMap<(i32, i32), &EpisodeScrape> = scrapes
            .iter()
            .filter_map(|s| s.cle(&g).map(|k| (k, s)))
            .collect();
        let mut compte = 0usize;
        let mut score = 0.0f64;
        for ep in locaux.iter().filter(|e| !e.est_fictif()) {
            let Some(cle) = ep
                .numero_dans(&diffusion)
                .filter(|n| n.est_valide())
                .map(|n| (n.saison, n.episode))
            else {
                continue;
            };
            if let Some(s) = index.get(&cle) {
                compte += 1;
                score += similarite(&texte_local(ep), &s.titre);
            }
        }
        let mieux = match &meilleur {
            None => true,
            Some((c, sc, _)) => compte > *c || (compte == *c && score > *sc + 1e-9),
        };
        if mieux {
            meilleur = Some((compte, score, g));
        }
    }
    meilleur.filter(|(c, _, _)| *c > 0).map(|(_, _, g)| g)
}

// ---------------------------------------------------------------------------
// Appariement
// ---------------------------------------------------------------------------

/// Résultat d'un appariement (indices dans les listes fournies).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Correspondance {
    /// `(indice local, indice scrapé)`.
    pub paires: Vec<(usize, usize)>,
    /// Épisodes locaux sans équivalent scrapé.
    pub locaux_orphelins: Vec<usize>,
    /// Épisodes scrapés sans fichier local (manquants).
    pub manquants: Vec<usize>,
}

/// Apparie épisodes locaux et scrapés par `(saison, épisode)` dans l'ordre
/// `groupe` (numéro local dans cet ordre, ou à défaut de diffusion).
/// Les épisodes fictifs locaux sont ignorés.
pub fn apparier_episodes(
    locaux: &[Episode],
    scrapes: &[EpisodeScrape],
    groupe: &GroupeEpisodes,
) -> Correspondance {
    let mut index: BTreeMap<(i32, i32), usize> = BTreeMap::new();
    for (i, s) in scrapes.iter().enumerate() {
        if let Some(k) = s.cle(groupe) {
            index.entry(k).or_insert(i);
        }
    }
    let mut c = Correspondance::default();
    let mut utilises: BTreeSet<usize> = BTreeSet::new();
    for (i, ep) in locaux.iter().enumerate() {
        if ep.fictif {
            continue;
        }
        match cle_locale(ep, groupe).and_then(|k| index.get(&k)) {
            Some(&j) => {
                c.paires.push((i, j));
                utilises.insert(j);
            }
            None => c.locaux_orphelins.push(i),
        }
    }
    c.manquants = (0..scrapes.len())
        .filter(|j| !utilises.contains(j) && scrapes[*j].cle(groupe).is_some())
        .collect();
    c
}

/// Applique les métadonnées scrapées à un épisode local.
///
/// Les numéros sont toujours mis à jour pour chaque ordre fourni (règle
/// « saison/épisode ») ; les autres champs ne sont remplacés que si la valeur
/// scrapée est non vide et que le champ local est vide ou `ecraser` est vrai.
pub fn appliquer_scrape_episode(ep: &mut Episode, s: &EpisodeScrape, ecraser: bool) {
    for n in &s.numeros {
        match ep.numeros.iter_mut().find(|m| m.groupe == n.groupe) {
            Some(m) => *m = n.clone(),
            None => ep.numeros.push(n.clone()),
        }
    }
    fn texte(local: &mut String, scrape: &str, ecraser: bool) {
        if !scrape.is_empty() && (ecraser || local.is_empty()) {
            *local = scrape.to_string();
        }
    }
    texte(&mut ep.titre, &s.titre, ecraser);
    texte(&mut ep.titre_original, &s.titre_original, ecraser);
    texte(&mut ep.resume, &s.resume, ecraser);
    if s.premiere_diffusion.is_some() && (ecraser || ep.premiere_diffusion.is_none()) {
        ep.premiere_diffusion = s.premiere_diffusion.clone();
    }
    if s.duree_minutes > 0 && (ecraser || ep.duree_minutes == 0) {
        ep.duree_minutes = s.duree_minutes;
    }
    for (cle, valeur) in s.ids.iter() {
        if ecraser {
            ep.ids.definir(cle, &valeur.en_texte());
        } else {
            ep.ids.definir_si_absent(cle, &valeur.en_texte());
        }
    }
    if !s.invites.is_empty() && (ecraser || ep.invites.is_empty()) {
        ep.invites = s.invites.clone();
    }
    if !s.equipe.is_empty() && (ecraser || ep.equipe.is_empty()) {
        ep.equipe = s.equipe.clone();
    }
}

/// Crée les épisodes fictifs (sans fichier) correspondant à la liste scrapée.
pub fn creer_episodes_fictifs(scrapes: &[EpisodeScrape], id_serie: &str) -> Vec<Episode> {
    scrapes
        .iter()
        .map(|s| {
            let mut ep = Episode {
                id_serie: id_serie.to_string(),
                fictif: true,
                ..Episode::default()
            };
            appliquer_scrape_episode(&mut ep, s, true);
            ep
        })
        .collect()
}

/// Mise à jour complète d'une série après scrape :
/// 1. si l'ordre actif est encore celui de diffusion par défaut, choix du
///    meilleur ordre ([`choisir_meilleur_groupe`]) ;
/// 2. la liste scrapée devient la liste des épisodes fictifs ;
/// 3. appariement et application des métadonnées aux épisodes locaux ;
/// 4. saisons recalculées, épisodes triés.
pub fn mettre_a_jour_depuis_scrape(
    serie: &mut Serie,
    scrapes: &[EpisodeScrape],
    groupes_proposes: &[GroupeEpisodes],
    ecraser: bool,
) -> Correspondance {
    let candidats = groupes_candidats(scrapes, groupes_proposes);
    for g in &candidats {
        if !serie.groupes.contains(g) {
            serie.groupes.push(g.clone());
        }
    }
    if serie.groupe_actif == GroupeEpisodes::diffusion()
        && let Some(g) = choisir_meilleur_groupe(&serie.episodes, scrapes, &candidats)
    {
        serie.groupe_actif = g;
    }
    let groupe = serie.groupe_actif.clone();
    serie.episodes_fictifs = creer_episodes_fictifs(scrapes, &serie.id);
    let c = apparier_episodes(&serie.episodes, scrapes, &groupe);
    for &(i, j) in &c.paires {
        // Le numéro local (issu du nom) vaut dans l'ordre actif : on le
        // conserve tel quel pour cet ordre, puis on applique les autres.
        appliquer_scrape_episode(&mut serie.episodes[i], &scrapes[j], ecraser);
    }
    definir_groupe_actif(serie, groupe);
    c
}

// ---------------------------------------------------------------------------
// Affichage des épisodes manquants
// ---------------------------------------------------------------------------

/// Réglages d'affichage des épisodes fictifs (tous désactivés par défaut).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReglagesFictifs {
    /// Épisodes normaux manquants (déjà diffusés).
    pub afficher_manquants: bool,
    /// Spéciaux manquants (saison 0).
    pub afficher_speciaux_manquants: bool,
    /// Épisodes pas encore diffusés ou sans date.
    pub afficher_non_diffuses: bool,
}

/// Épisodes fictifs sans épisode réel de même `(saison, épisode)` dans
/// l'ordre actif.
pub fn episodes_manquants(serie: &Serie) -> Vec<&Episode> {
    let g = &serie.groupe_actif;
    let reels: BTreeSet<(i32, i32)> = serie
        .episodes
        .iter()
        .map(|e| (e.saison(g), e.episode(g)))
        .collect();
    serie
        .episodes_fictifs
        .iter()
        .filter(|f| !reels.contains(&(f.saison(g), f.episode(g))))
        .collect()
}

/// Épisodes à afficher : épisodes réels, plus les fictifs non doublons
/// autorisés par les réglages, triés par saison, épisode puis titre.
/// `date_courante` (`aaaa-mm-jj`) sert à distinguer diffusés / non diffusés.
pub fn episodes_a_afficher<'a>(
    serie: &'a Serie,
    reglages: ReglagesFictifs,
    date_courante: Option<&str>,
) -> Vec<&'a Episode> {
    let g = &serie.groupe_actif;
    let mut v: Vec<&Episode> = serie.episodes.iter().collect();
    for f in episodes_manquants(serie) {
        let autorise = match f.type_episode(g, date_courante) {
            TypeEpisode::FictifSpecial => reglages.afficher_speciaux_manquants,
            TypeEpisode::FictifNormal => reglages.afficher_manquants,
            TypeEpisode::FictifNonDiffuse => reglages.afficher_non_diffuses,
            _ => false,
        };
        if autorise {
            v.push(f);
        }
    }
    v.sort_by(|a, b| {
        (a.saison(g), a.episode(g), &a.titre).cmp(&(b.saison(g), b.episode(g), &b.titre))
    });
    v
}

/// Choix automatique d'un épisode scrapé pour un fichier : (1) titre
/// identique au nom de base de la vidéo, sinon (2) identique au titre local,
/// sinon (3) meilleure similarité si elle dépasse 0,8.
pub fn choisir_episode_manuel(
    nom_base_video: &str,
    titre_local: &str,
    candidats: &[EpisodeScrape],
) -> Option<usize> {
    let base = normaliser(nom_base_video);
    let titre = normaliser(titre_local);
    if let Some(i) = candidats
        .iter()
        .position(|c| !base.is_empty() && normaliser(&c.titre) == base)
    {
        return Some(i);
    }
    if let Some(i) = candidats
        .iter()
        .position(|c| !titre.is_empty() && normaliser(&c.titre) == titre)
    {
        return Some(i);
    }
    candidats
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let s = similarite(&c.titre, titre_local).max(similarite(&c.titre, nom_base_video));
            (i, s)
        })
        .filter(|(_, s)| *s > 0.8)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Recalcule les saisons et le tri après une modification manuelle des
/// épisodes (raccourci pratique).
pub fn reorganiser(serie: &mut Serie) {
    recalculer_saisons(serie);
    trier_episodes(serie);
}
