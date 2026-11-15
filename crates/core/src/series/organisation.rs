//! Organisation d'une série : saisons dérivées des épisodes, tri, changement
//! de l'ordre d'épisodes actif.

use std::collections::BTreeMap;

use crate::modele::{Episode, GroupeEpisodes, Saison, Serie, TypeFichier};

/// Nom du premier fichier vidéo d'un épisode (clé de tri secondaire).
fn premier_fichier_video(ep: &Episode) -> String {
    ep.fichiers
        .iter()
        .find(|f| f.type_fichier == TypeFichier::Video)
        .map(|f| f.nom.clone())
        .unwrap_or_default()
}

/// Trie les épisodes (réels et fictifs) par saison, épisode puis nom du
/// premier fichier vidéo, dans l'ordre actif de la série.
pub fn trier_episodes(serie: &mut Serie) {
    let groupe = serie.groupe_actif.clone();
    let cle = |e: &Episode| {
        (
            e.saison(&groupe),
            e.episode(&groupe),
            premier_fichier_video(e),
            e.titre.clone(),
        )
    };
    serie.episodes.sort_by_key(cle);
    serie.episodes_fictifs.sort_by_key(cle);
}

/// Reconstruit la liste des saisons à partir des épisodes réels et fictifs
/// dans l'ordre actif. Les saisons existantes conservent titre, résumé et
/// artworks ; une saison sans épisode est gardée si elle a des artworks.
/// La première diffusion d'une saison est la plus ancienne de ses épisodes.
pub fn recalculer_saisons(serie: &mut Serie) {
    let groupe = serie.groupe_actif.clone();
    let mut anciennes: BTreeMap<i32, Saison> =
        serie.saisons.drain(..).map(|s| (s.numero, s)).collect();
    let mut nouvelles: BTreeMap<i32, Saison> = BTreeMap::new();

    for ep in serie.episodes.iter().chain(serie.episodes_fictifs.iter()) {
        let n = ep.saison(&groupe);
        if n < 0 {
            continue;
        }
        let saison = nouvelles.entry(n).or_insert_with(|| {
            // La date ancienne a pu être fixée par un autre ordre : on la recalcule.
            let mut s = anciennes.remove(&n).unwrap_or_else(|| Saison {
                numero: n,
                ..Saison::default()
            });
            s.premiere_diffusion = None;
            s
        });
        if let Some(d) = &ep.premiere_diffusion {
            saison.premiere_diffusion = Some(match saison.premiere_diffusion.take() {
                Some(actuelle) if actuelle.as_str() <= d.as_str() => actuelle,
                _ => d.clone(),
            });
        }
    }
    for (n, s) in anciennes {
        if !s.fichiers.is_empty() || !s.titre.is_empty() || !s.resume.is_empty() {
            nouvelles.entry(n).or_insert(Saison {
                premiere_diffusion: None,
                ..s
            });
        }
    }
    serie.saisons = nouvelles.into_values().collect();
}

/// Change l'ordre d'épisodes actif puis recalcule saisons et tri.
pub fn definir_groupe_actif(serie: &mut Serie, groupe: GroupeEpisodes) {
    if !serie.groupes.contains(&groupe) {
        serie.groupes.push(groupe.clone());
    }
    serie.groupe_actif = groupe;
    recalculer_saisons(serie);
    trier_episodes(serie);
}
