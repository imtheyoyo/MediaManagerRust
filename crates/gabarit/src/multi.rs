//! Gabarits multi-épisodes : styles `REPEAT` et `RANGE`.
//!
//! Hypothèse (comme le programme d'origine) : dans le gabarit, la partie
//! saison et la partie épisode sont adjacentes (`S${seasonNr2}E${episodeNr2}`,
//! `${seasonNr}x${episodeNr2}`).

use std::sync::LazyLock;

use regex::Regex;

use crate::destination::{FormeDestination, OptionsDestination, nettoyer_destination};
use crate::erreur::Resultat;
use crate::evaluation::{Gabarit, Jetons};
use crate::options::{OptionsRendu, SubstitutionSeparateurs};
use crate::syntaxe::{self, Noeud};
use crate::valeur::Contexte;

/// Style de rendu quand un fichier contient plusieurs épisodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StyleMultiEpisodes {
    /// `S01E01 S01E02`
    #[default]
    Repeat,
    /// `S01E01-E02` (repli sur `Repeat` si saisons différentes ou numéros non consécutifs)
    Range,
}

/// Un épisode à rendre : numéros (pour le tri et la détection de plage)
/// et contexte complet.
#[derive(Debug, Clone)]
pub struct EpisodeContexte {
    pub saison: i64,
    pub episode: i64,
    pub contexte: Contexte,
}

/// Noms de jetons par catégorie (comparaison insensible à la casse sur le
/// chemin complet de la variable).
#[derive(Debug, Clone)]
pub struct ListesJetonsEpisode {
    pub saison: Vec<String>,
    pub episode: Vec<String>,
    pub titre: Vec<String>,
    pub date_diffusion: Vec<String>,
}

impl Default for ListesJetonsEpisode {
    fn default() -> Self {
        let v = |l: &[&str]| l.iter().map(|s| (*s).to_string()).collect();
        ListesJetonsEpisode {
            saison: v(&[
                "seasonNr",
                "seasonNr2",
                "seasonNrAired",
                "seasonNrAired2",
                "seasonNrDvd",
                "seasonNrDvd2",
                "episode.season",
                "episode.dvdSeason",
            ]),
            episode: v(&[
                "episodeNr",
                "episodeNr2",
                "episodeNrAired",
                "episodeNrAired2",
                "episodeNrDvd",
                "episodeNrDvd2",
                "episode.episode",
                "episode.dvdEpisode",
                "absoluteNr",
                "absoluteNr2",
                "episode.absoluteNumber",
            ]),
            titre: v(&["title", "originalTitle", "englishTitle", "titleSortable"]),
            date_diffusion: v(&["airedDate", "episode.firstAired"]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Categorie {
    Saison,
    Episode,
    Titre,
    Date,
}

static DELIM_SAISON: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)((staffel|season|s)\s?)$").expect("motif valide"));
static DELIM_EPISODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(\s?(folge|episode|[epx]+)\s?)$").expect("motif valide"));

fn contient(liste: &[String], chemin: &str) -> bool {
    liste.iter().any(|j| j.eq_ignore_ascii_case(chemin))
}

fn categorie(tag: &str, listes: &ListesJetonsEpisode) -> Option<Categorie> {
    let noeuds = syntaxe::analyser(tag).ok()?;
    let Some(Noeud::Sortie { expr, .. }) = noeuds.first() else {
        return None;
    };
    let chemin = expr
        .chemin
        .iter()
        .map(|s| s.nom.as_str())
        .collect::<Vec<_>>()
        .join(".");
    if contient(&listes.saison, &chemin) {
        Some(Categorie::Saison)
    } else if contient(&listes.episode, &chemin) {
        Some(Categorie::Episode)
    } else if contient(&listes.date_diffusion, &chemin) {
        Some(Categorie::Date)
    } else if contient(&listes.titre, &chemin) || chemin.to_lowercase().starts_with("episode.") {
        Some(Categorie::Titre)
    } else {
        None
    }
}

fn rendre_morceau(
    src: &str,
    ctx: &Contexte,
    jetons: &Jetons,
    opts: &OptionsRendu,
) -> Resultat<String> {
    Gabarit::analyser(src)?.rendre(ctx, jetons, opts)
}

/// Rend un gabarit pour un ou plusieurs épisodes (rendu brut, sans nettoyage ;
/// voir [`creer_destination_multi`]).
///
/// Les épisodes sont triés par (saison, épisode) ; si tous ont les mêmes
/// numéros (empilement), seul le premier est utilisé. Les titres (doublons
/// consécutifs supprimés) et dates de diffusion sont rendus pour chaque
/// épisode et joints par `" - "`.
pub fn rendre_multi_episodes(
    gabarit: &str,
    episodes: &[EpisodeContexte],
    style: StyleMultiEpisodes,
    listes: &ListesJetonsEpisode,
    jetons: &Jetons,
    options: &OptionsRendu,
) -> Resultat<String> {
    let mut eps: Vec<&EpisodeContexte> = episodes.iter().collect();
    eps.sort_by_key(|e| (e.saison, e.episode));
    let Some(premier) = eps.first().copied() else {
        return Ok(String::new());
    };
    if eps
        .iter()
        .all(|e| (e.saison, e.episode) == (premier.saison, premier.episode))
    {
        eps.truncate(1);
    }
    if eps.len() == 1 {
        return rendre_morceau(gabarit, &premier.contexte, jetons, options);
    }

    let balises = syntaxe::lister_balises(gabarit)?;
    let classees: Vec<(usize, usize, Option<Categorie>)> = balises
        .iter()
        .map(|&(d, f)| (d, f, categorie(&gabarit[d..f], listes)))
        .collect();
    let Some(idx_ep) = classees
        .iter()
        .position(|b| b.2 == Some(Categorie::Episode))
    else {
        return rendre_morceau(gabarit, &premier.contexte, jetons, options);
    };
    let idx_sa = classees
        .iter()
        .position(|b| b.2 == Some(Categorie::Saison))
        .filter(|&i| i < idx_ep);

    // Borne basse : fin de la balise précédente (le délimiteur est du texte littéral).
    let borne = |i: usize| if i == 0 { 0 } else { classees[i - 1].1 };
    let (ep_tag_debut, ep_fin, _) = classees[idx_ep];
    let ep_debut = DELIM_EPISODE
        .find(&gabarit[borne(idx_ep)..ep_tag_debut])
        .map_or(ep_tag_debut, |m| borne(idx_ep) + m.start());
    let boucle_debut = match idx_sa {
        Some(i) => {
            let d = classees[i].0;
            DELIM_SAISON
                .find(&gabarit[borne(i)..d])
                .map_or(d, |m| borne(i) + m.start())
        }
        None => ep_debut,
    };
    let texte_boucle = &gabarit[boucle_debut..ep_fin];

    let derniere = eps[eps.len() - 1];
    let memes_saisons = eps.iter().all(|e| e.saison == premier.saison);
    let consecutifs = eps.windows(2).all(|w| w[1].episode == w[0].episode + 1);
    let partie_boucle = if style == StyleMultiEpisodes::Range && memes_saisons && consecutifs {
        let debut = rendre_morceau(texte_boucle, &premier.contexte, jetons, options)?;
        let prefixe = gabarit[ep_debut..ep_tag_debut].trim();
        let suffixe_src = if prefixe.eq_ignore_ascii_case("x") && idx_sa.is_some() {
            &gabarit[ep_tag_debut..ep_fin]
        } else {
            &gabarit[ep_debut..ep_fin]
        };
        let fin = rendre_morceau(suffixe_src, &derniere.contexte, jetons, options)?;
        format!("{debut}-{fin}")
    } else {
        let mut rendus = Vec::new();
        for e in &eps {
            rendus.push(rendre_morceau(texte_boucle, &e.contexte, jetons, options)?);
        }
        rendus.join(" ")
    };

    // Gabarit transformé : zones remplacées par des marqueurs littéraux.
    let mut remplacements: Vec<String> = vec![partie_boucle];
    let mut modele = String::new();
    let mut curseur = 0;
    let mut zones: Vec<(usize, usize, Option<usize>)> = vec![(boucle_debut, ep_fin, Some(0))];
    for &(d, f, cat) in &classees {
        if d >= boucle_debut && f <= ep_fin {
            continue;
        }
        if matches!(cat, Some(Categorie::Titre | Categorie::Date)) {
            let mut valeurs: Vec<String> = Vec::new();
            for e in &eps {
                let v = rendre_morceau(&gabarit[d..f], &e.contexte, jetons, options)?;
                let doublon = cat == Some(Categorie::Titre) && valeurs.last() == Some(&v);
                if !doublon && !v.is_empty() {
                    valeurs.push(v);
                }
            }
            remplacements.push(valeurs.join(" - "));
            zones.push((d, f, Some(remplacements.len() - 1)));
        }
    }
    zones.sort_by_key(|z| z.0);
    for (d, f, i) in zones {
        modele.push_str(&gabarit[curseur..d]);
        modele.push_str(&format!("\u{E000}{}\u{E001}", i.unwrap_or(0)));
        curseur = f;
    }
    modele.push_str(&gabarit[curseur..]);
    let mut rendu = rendre_morceau(&modele, &premier.contexte, jetons, options)?;
    for (i, r) in remplacements.iter().enumerate() {
        rendu = rendu.replace(&format!("\u{E000}{i}\u{E001}"), r);
    }
    Ok(rendu)
}

/// Rend un gabarit multi-épisodes puis applique le nettoyage de destination.
pub fn creer_destination_multi(
    gabarit: &str,
    episodes: &[EpisodeContexte],
    style: StyleMultiEpisodes,
    listes: &ListesJetonsEpisode,
    jetons: &Jetons,
    forme: FormeDestination,
    options: &OptionsDestination,
) -> Resultat<String> {
    if gabarit.trim().is_empty() {
        return Ok(String::new());
    }
    let mut rendu = options.rendu.clone();
    rendu.substitution_separateurs = if options.unicode {
        SubstitutionSeparateurs::Unicode
    } else {
        SubstitutionSeparateurs::Espace
    };
    let brut = rendre_multi_episodes(gabarit, episodes, style, listes, jetons, &rendu)?;
    Ok(nettoyer_destination(&brut, forme, options))
}
