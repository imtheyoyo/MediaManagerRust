//! Conversion des types « séries » des scrapers vers le modèle partagé et
//! application des métadonnées d'une série.

use mediamanager_core::modele::{
    GroupeEpisodes, NumeroEpisode, Personne, Serie, StatutDiffusion, TypeFichier, TypeGroupe,
};
use mediamanager_core::series::EpisodeScrape;
use mediamanager_scrapers as sc;
use mediamanager_store::ReglagesSeries;

use crate::conversion::{
    choisir_certification, depuis_identifiants, genre_lisible, vers_identifiants, vers_note,
    vers_personne,
};

/// Réglages d'une récupération de métadonnées de série.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSerie {
    /// Ne remplir que les champs vides (série et épisodes).
    pub ne_pas_ecraser: bool,
    /// Ordre d'épisodes imposé ; `None` = choix automatique (ordre qui
    /// explique le mieux les fichiers locaux, diffusion par défaut).
    pub ordre: Option<GroupeEpisodes>,
    /// Récupère la liste des épisodes et l'apparie aux épisodes locaux.
    pub episodes: bool,
    /// Conserve la liste scrapée comme épisodes fictifs (épisodes manquants).
    pub episodes_fictifs: bool,
    /// Télécharge les images de la série et des saisons.
    pub artwork: bool,
    /// Télécharge les vignettes des épisodes.
    pub artwork_episodes: bool,
}

impl Default for ConfigSerie {
    fn default() -> Self {
        Self {
            ne_pas_ecraser: false,
            ordre: None,
            episodes: true,
            episodes_fictifs: true,
            artwork: true,
            artwork_episodes: true,
        }
    }
}

impl ConfigSerie {
    /// Valeurs par défaut (aucun réglage « ne pas écraser » pour les séries).
    pub fn depuis_reglages(_r: &ReglagesSeries) -> Self {
        Self::default()
    }
}

/// Type d'ordre d'épisodes.
pub fn vers_type_groupe(t: sc::TypeGroupeEpisodes) -> TypeGroupe {
    match t {
        sc::TypeGroupeEpisodes::Diffusion => TypeGroupe::Diffusion,
        sc::TypeGroupeEpisodes::Dvd => TypeGroupe::Dvd,
        sc::TypeGroupeEpisodes::Absolu => TypeGroupe::Absolu,
        sc::TypeGroupeEpisodes::Alternatif => TypeGroupe::Alternatif,
        sc::TypeGroupeEpisodes::Affichage => TypeGroupe::Affichage,
    }
}

/// Ordre d'épisodes du scraper → modèle.
pub fn vers_groupe(g: &sc::GroupeEpisodes) -> GroupeEpisodes {
    GroupeEpisodes {
        type_groupe: vers_type_groupe(g.type_groupe),
        nom: g.nom.clone(),
    }
}

/// Ordre d'épisodes du modèle → scraper.
pub fn depuis_groupe(g: &GroupeEpisodes) -> sc::GroupeEpisodes {
    let t = match g.type_groupe {
        TypeGroupe::Diffusion => sc::TypeGroupeEpisodes::Diffusion,
        TypeGroupe::Dvd => sc::TypeGroupeEpisodes::Dvd,
        TypeGroupe::Absolu => sc::TypeGroupeEpisodes::Absolu,
        TypeGroupe::Alternatif => sc::TypeGroupeEpisodes::Alternatif,
        TypeGroupe::Affichage => sc::TypeGroupeEpisodes::Affichage,
    };
    sc::GroupeEpisodes::nouveau(t, g.nom.clone())
}

/// Statut de diffusion.
pub fn vers_statut(s: sc::StatutDiffusion) -> StatutDiffusion {
    match s {
        sc::StatutDiffusion::Inconnu => StatutDiffusion::Inconnu,
        sc::StatutDiffusion::EnCours => StatutDiffusion::EnCours,
        sc::StatutDiffusion::Terminee => StatutDiffusion::Terminee,
    }
}

fn personnes(m: &[sc::Personne], garder: impl Fn(sc::TypePersonne) -> bool) -> Vec<Personne> {
    m.iter()
        .filter(|p| garder(p.type_personne))
        .map(vers_personne)
        .collect()
}

/// Épisode scrapé → épisode du cœur (correspondance, épisodes fictifs).
pub fn vers_episode_scrape(m: &sc::MetadonneesEpisode) -> EpisodeScrape {
    EpisodeScrape {
        numeros: m
            .numeros
            .iter()
            .map(|n| NumeroEpisode {
                groupe: vers_groupe(&n.groupe),
                saison: n.saison as i32,
                episode: n.episode as i32,
            })
            .collect(),
        titre: m.titre.clone(),
        titre_original: m.titre_original.clone(),
        resume: m.resume.clone(),
        premiere_diffusion: m.date_diffusion.clone(),
        duree_minutes: m.duree,
        ids: vers_identifiants(&m.ids),
        invites: personnes(&m.personnes, |t| {
            matches!(t, sc::TypePersonne::Invite | sc::TypePersonne::Acteur)
        }),
        equipe: personnes(&m.personnes, |t| {
            !matches!(t, sc::TypePersonne::Invite | sc::TypePersonne::Acteur)
        }),
    }
}

/// Options de recherche d'une série d'après ses identifiants, son titre et
/// les réglages de langue et de pays.
pub fn options_recherche_serie(serie: &Serie, r: &ReglagesSeries) -> sc::OptionsRecherche {
    let mut o = sc::OptionsRecherche::avec_requete(serie.titre.clone());
    o.annee = (serie.annee > 0).then_some(serie.annee as i32);
    o.ids = depuis_identifiants(&serie.ids);
    o.langue = r.langue_scraping.clone();
    o.pays_classification = r.pays_certification.clone();
    o.pays_sortie = r.pays_certification.clone();
    o
}

fn affecter<T: PartialEq + Default>(ne_pas_ecraser: bool, cible: &mut T, v: T) {
    if v == T::default() || (ne_pas_ecraser && *cible != T::default()) {
        return;
    }
    *cible = v;
}

/// Type de fichier du modèle correspondant à une image de série (hors
/// images de saison).
pub fn type_fichier_serie(t: sc::TypeArtworkEtendu) -> Option<TypeFichier> {
    use sc::TypeArtworkEtendu as T;
    Some(match t {
        T::Affiche => TypeFichier::Poster,
        T::Fond => TypeFichier::Fanart,
        T::Banniere => TypeFichier::Banner,
        T::Vignette => TypeFichier::Thumb,
        T::LogoTransparent => TypeFichier::Clearlogo,
        T::ClearArt => TypeFichier::Clearart,
        T::Disque => TypeFichier::Disc,
        T::ArtCle => TypeFichier::Keyart,
        T::ArtPersonnage => TypeFichier::Characterart,
        T::Carre => TypeFichier::Squareart,
        T::AfficheSaison => TypeFichier::SaisonPoster,
        T::FondSaison => TypeFichier::SaisonFanart,
        T::BanniereSaison => TypeFichier::SaisonBanner,
        T::VignetteSaison => TypeFichier::SaisonThumb,
    })
}

/// Applique les métadonnées d'une série (hors épisodes). Les titres et
/// résumés de saison de l'ordre actif sont repris des saisons scrapées.
pub fn appliquer_metadonnees_serie(
    serie: &mut Serie,
    m: &sc::MetadonneesSerie,
    ne_pas_ecraser: bool,
    pays_certification: &str,
) {
    let ne = ne_pas_ecraser;
    affecter(ne, &mut serie.titre, m.titre.clone());
    affecter(ne, &mut serie.titre_original, m.titre_original.clone());
    affecter(ne, &mut serie.titre_anglais, m.titre_anglais.clone());
    affecter(ne, &mut serie.resume, m.resume.clone());
    affecter(ne, &mut serie.slogan, m.accroche.clone());
    affecter(ne, &mut serie.annee, m.annee.max(0) as u32);
    affecter(ne, &mut serie.annee_fin, m.annee_fin.max(0) as u32);
    affecter(
        ne,
        &mut serie.premiere_diffusion,
        m.date_premiere_diffusion.clone(),
    );
    if m.statut != sc::StatutDiffusion::Inconnu && (!ne || serie.statut == StatutDiffusion::Inconnu)
    {
        serie.statut = vers_statut(m.statut);
    }
    affecter(ne, &mut serie.duree_minutes, m.duree);
    affecter(
        ne,
        &mut serie.certification,
        choisir_certification(&m.classifications, pays_certification),
    );
    affecter(ne, &mut serie.pays, m.pays.join("/"));
    affecter(
        ne,
        &mut serie.societe_production,
        m.societes_production.join(" / "),
    );
    affecter(
        ne,
        &mut serie.genres,
        m.genres.iter().map(|g| genre_lisible(g)).collect(),
    );
    affecter(
        ne,
        &mut serie.acteurs,
        personnes(&m.personnes, |t| t == sc::TypePersonne::Acteur),
    );
    affecter(
        ne,
        &mut serie.equipe,
        personnes(&m.personnes, |t| t != sc::TypePersonne::Acteur),
    );
    for e in &m.evaluations {
        if ne && serie.notes.contains_key(&e.source) {
            continue;
        }
        serie.notes.insert(e.source.clone(), vers_note(e));
    }
    for t in &m.mots_cles {
        if !serie.tags.iter().any(|x| x.eq_ignore_ascii_case(t)) {
            serie.tags.push(t.clone());
        }
    }
    for (k, v) in &m.ids {
        if ne {
            serie.ids.definir_si_absent(k, v);
        } else {
            serie.ids.definir(k, v);
        }
    }
    for g in &m.groupes_episodes {
        let g = vers_groupe(g);
        if !serie.groupes.contains(&g) {
            serie.groupes.push(g);
        }
    }
    for a in m.artwork.iter().filter(|a| a.saison.is_none()) {
        let Some(t) = type_fichier_serie(a.type_artwork) else {
            continue;
        };
        let url = if a.url_originale.is_empty() {
            &a.url_apercu
        } else {
            &a.url_originale
        };
        if !url.is_empty() && !(ne && serie.urls_artwork.contains_key(&t)) {
            serie.urls_artwork.entry(t).or_insert_with(|| url.clone());
        }
    }
    appliquer_saisons(serie, m, ne);
}

/// Titres et résumés des saisons de l'ordre actif.
pub fn appliquer_saisons(serie: &mut Serie, m: &sc::MetadonneesSerie, ne_pas_ecraser: bool) {
    let groupe = depuis_groupe(&serie.groupe_actif);
    for s in m.saisons_du_groupe(&groupe) {
        if let Some(saison) = serie
            .saisons
            .iter_mut()
            .find(|x| x.numero == s.numero as i32)
        {
            affecter(ne_pas_ecraser, &mut saison.titre, s.nom.clone());
            affecter(ne_pas_ecraser, &mut saison.resume, s.resume.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion_episode_et_groupes() {
        let mut m = sc::MetadonneesEpisode {
            titre: "Pilote".into(),
            date_diffusion: Some("2020-01-05".into()),
            ..Default::default()
        };
        m.definir_numero(sc::GroupeEpisodes::diffusion(), 1, 2);
        m.definir_numero(sc::GroupeEpisodes::dvd(), 1, 3);
        m.ids.insert("tmdb".into(), "42".into());
        let e = vers_episode_scrape(&m);
        assert_eq!(e.numeros.len(), 2);
        assert_eq!(
            e.numero_dans(&GroupeEpisodes::dvd()).map(|n| n.episode),
            Some(3)
        );
        assert_eq!(e.ids.tmdb(), 42);
        assert_eq!(
            depuis_groupe(&vers_groupe(&sc::GroupeEpisodes::absolu())),
            sc::GroupeEpisodes::absolu()
        );
    }

    #[test]
    fn application_serie_ne_pas_ecraser() {
        let mut s = Serie {
            titre: "Local".into(),
            ..Serie::default()
        };
        let m = sc::MetadonneesSerie {
            titre: "Distant".into(),
            resume: "Résumé".into(),
            annee: 2020,
            statut: sc::StatutDiffusion::Terminee,
            ..Default::default()
        };
        appliquer_metadonnees_serie(&mut s, &m, true, "FR");
        assert_eq!(s.titre, "Local");
        assert_eq!(s.resume, "Résumé");
        assert_eq!(s.statut, StatutDiffusion::Terminee);
        appliquer_metadonnees_serie(&mut s, &m, false, "FR");
        assert_eq!(s.titre, "Distant");
    }
}
