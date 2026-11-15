//! Conversion entre les types du crate de scrapers et le modèle partagé.
//!
//! Le crate de scrapers est autonome (aucune dépendance au modèle) : toutes
//! les correspondances vivent ici.

use std::collections::BTreeMap;

use mediamanager_core::modele::{Film, Identifiants, Note, Personne, TypeFichier, TypePersonne};
use mediamanager_scrapers as sc;
use mediamanager_store::ReglagesFilms;

/// Champs à écraser lors de l'application de métadonnées à un film.
///
/// Un champ à `false` n'est jamais touché. Avec
/// [`ne_pas_ecraser`](Self::ne_pas_ecraser), un champ autorisé n'est rempli
/// que s'il est encore vide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigMetadonnees {
    pub titre: bool,
    pub titre_original: bool,
    pub annee: bool,
    pub resume: bool,
    pub slogan: bool,
    pub notes: bool,
    pub genres: bool,
    pub acteurs: bool,
    pub equipe: bool,
    pub certification: bool,
    pub date_sortie: bool,
    pub duree: bool,
    pub societe_production: bool,
    pub pays: bool,
    pub langues: bool,
    /// Mots-clés du fournisseur ajoutés aux tags.
    pub tags: bool,
    pub identifiants: bool,
    /// Rattachement à la saga (collection) du fournisseur.
    pub saga: bool,
    /// Téléchargement des images (affiche, fond…).
    pub artwork: bool,
    /// Ne remplir que les champs vides.
    pub ne_pas_ecraser: bool,
}

impl Default for ConfigMetadonnees {
    fn default() -> Self {
        Self::tout()
    }
}

impl ConfigMetadonnees {
    /// Tous les champs, écrasement autorisé.
    pub fn tout() -> Self {
        Self {
            titre: true,
            titre_original: true,
            annee: true,
            resume: true,
            slogan: true,
            notes: true,
            genres: true,
            acteurs: true,
            equipe: true,
            certification: true,
            date_sortie: true,
            duree: true,
            societe_production: true,
            pays: true,
            langues: true,
            tags: true,
            identifiants: true,
            saga: true,
            artwork: true,
            ne_pas_ecraser: false,
        }
    }

    /// Tous les champs, avec le drapeau « ne pas écraser » des réglages.
    pub fn depuis_reglages(r: &ReglagesFilms) -> Self {
        Self {
            ne_pas_ecraser: r.ne_pas_ecraser,
            ..Self::tout()
        }
    }
}

/// Table du scraper → identifiants du modèle (règles de validation du modèle).
pub fn vers_identifiants(ids: &BTreeMap<String, String>) -> Identifiants {
    let mut r = Identifiants::new();
    for (k, v) in ids {
        r.definir(k, v);
    }
    r
}

/// Identifiants du modèle → table du scraper.
pub fn depuis_identifiants(ids: &Identifiants) -> BTreeMap<String, String> {
    ids.iter().map(|(k, v)| (k.clone(), v.en_texte())).collect()
}

/// Rôle d'une personne.
pub fn vers_type_personne(t: sc::TypePersonne) -> TypePersonne {
    match t {
        sc::TypePersonne::Acteur => TypePersonne::Acteur,
        sc::TypePersonne::Realisateur => TypePersonne::Realisateur,
        sc::TypePersonne::Scenariste => TypePersonne::Scenariste,
        sc::TypePersonne::Producteur => TypePersonne::Producteur,
        sc::TypePersonne::Monteur => TypePersonne::Monteur,
        sc::TypePersonne::Compositeur => TypePersonne::Compositeur,
        sc::TypePersonne::Photographie => TypePersonne::Camera,
        sc::TypePersonne::Invite => TypePersonne::Invite,
        sc::TypePersonne::Autre => TypePersonne::Autre,
    }
}

/// Personne du générique.
pub fn vers_personne(p: &sc::Personne) -> Personne {
    Personne {
        type_personne: vers_type_personne(p.type_personne),
        nom: p.nom.clone(),
        role: p.role.clone(),
        url_vignette: p.url_photo.clone(),
        url_profil: p.url_profil.clone(),
        ids: vers_identifiants(&p.ids),
    }
}

/// Note d'une source.
pub fn vers_note(e: &sc::Evaluation) -> Note {
    let maximum = if e.maximum > 0.0 {
        e.maximum.round() as u32
    } else {
        10
    };
    Note::nouvelle(&e.source, e.valeur, e.votes, maximum)
}

/// Type de fichier du modèle correspondant à un type d'image du scraper.
pub fn vers_type_fichier(t: sc::TypeArtwork) -> TypeFichier {
    match t {
        sc::TypeArtwork::Affiche => TypeFichier::Poster,
        sc::TypeArtwork::Fond => TypeFichier::Fanart,
        sc::TypeArtwork::Vignette => TypeFichier::Thumb,
        sc::TypeArtwork::LogoTransparent => TypeFichier::Clearlogo,
    }
}

/// Classification retenue : celle du pays demandé (`FR:12` ⇒ `12`), sinon
/// celle des États-Unis, sinon la première.
pub fn choisir_certification(classes: &[sc::Classification], pays: &str) -> String {
    classes
        .iter()
        .find(|c| c.pays.eq_ignore_ascii_case(pays) && !c.valeur.is_empty())
        .or_else(|| {
            classes
                .iter()
                .find(|c| c.pays.eq_ignore_ascii_case("US") && !c.valeur.is_empty())
        })
        .or_else(|| classes.iter().find(|c| !c.valeur.is_empty()))
        .map(|c| format!("{}:{}", c.pays.to_uppercase(), c.valeur))
        .unwrap_or_default()
}

/// Genre lisible : un identifiant interne (`SCIENCE_FICTION`) devient
/// `Science Fiction` ; un nom brut est conservé tel quel.
pub fn genre_lisible(g: &str) -> String {
    let interne = !g.is_empty()
        && g.chars()
            .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit());
    if !interne {
        return g.to_string();
    }
    g.split('_')
        .filter(|m| !m.is_empty())
        .map(|m| {
            let mut c = m.chars();
            match c.next() {
                Some(p) => p.to_string() + &c.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Options de recherche pour un film, d'après ses identifiants, son titre et
/// les réglages de langue et de pays.
pub fn options_recherche(film: &Film, reglages: &ReglagesFilms) -> sc::OptionsRecherche {
    let mut o = sc::OptionsRecherche::avec_requete(film.titre.clone());
    o.annee = (film.annee > 0).then_some(film.annee as i32);
    o.ids = depuis_identifiants(&film.ids);
    o.langue = reglages.langue_scraping.clone();
    o.pays_classification = reglages.pays_certification.clone();
    o.pays_sortie = reglages.pays_certification.clone();
    o
}

fn affecter<T: PartialEq + Default>(autorise: bool, ne_pas_ecraser: bool, cible: &mut T, v: T) {
    if !autorise || v == T::default() {
        return;
    }
    if ne_pas_ecraser && *cible != T::default() {
        return;
    }
    *cible = v;
}

/// Applique des métadonnées au film selon `config`. Les images ne sont pas
/// téléchargées ici : seules leurs URL sont retenues (`urls_artwork`, pour les
/// types sans URL déjà connue quand l'écrasement est interdit).
pub fn appliquer_metadonnees(
    film: &mut Film,
    m: &sc::MetadonneesFilm,
    config: &ConfigMetadonnees,
    pays_certification: &str,
) {
    let ne = config.ne_pas_ecraser;
    affecter(config.titre, ne, &mut film.titre, m.titre.clone());
    affecter(
        config.titre_original,
        ne,
        &mut film.titre_original,
        m.titre_original.clone(),
    );
    affecter(
        config.titre_original,
        ne,
        &mut film.titre_anglais,
        m.titre_anglais.clone(),
    );
    affecter(config.annee, ne, &mut film.annee, m.annee.max(0) as u32);
    affecter(config.resume, ne, &mut film.resume, m.resume.clone());
    affecter(config.slogan, ne, &mut film.slogan, m.accroche.clone());
    affecter(config.duree, ne, &mut film.duree_minutes, m.duree);
    affecter(
        config.date_sortie,
        ne,
        &mut film.date_sortie,
        m.date_sortie.clone(),
    );
    affecter(
        config.certification,
        ne,
        &mut film.certification,
        choisir_certification(&m.classifications, pays_certification),
    );
    affecter(
        config.societe_production,
        ne,
        &mut film.societe_production,
        m.societes_production.join(" / "),
    );
    affecter(config.pays, ne, &mut film.pays, m.pays.join("/"));
    affecter(
        config.langues,
        ne,
        &mut film.langues_parlees,
        m.langues_parlees.join(", "),
    );
    affecter(
        config.genres,
        ne,
        &mut film.genres,
        m.genres.iter().map(|g| genre_lisible(g)).collect(),
    );
    affecter(
        config.acteurs,
        ne,
        &mut film.acteurs,
        m.personnes
            .iter()
            .filter(|p| p.type_personne == sc::TypePersonne::Acteur)
            .map(vers_personne)
            .collect(),
    );
    affecter(
        config.equipe,
        ne,
        &mut film.equipe,
        m.personnes
            .iter()
            .filter(|p| p.type_personne != sc::TypePersonne::Acteur)
            .map(vers_personne)
            .collect(),
    );
    if config.notes {
        for e in &m.evaluations {
            if ne && film.notes.contains_key(&e.source) {
                continue;
            }
            film.notes.insert(e.source.clone(), vers_note(e));
        }
    }
    if config.tags {
        for t in &m.mots_cles {
            if !film.tags.iter().any(|x| x.eq_ignore_ascii_case(t)) {
                film.tags.push(t.clone());
            }
        }
    }
    if config.identifiants {
        for (k, v) in &m.ids {
            if ne {
                film.ids.definir_si_absent(k, v);
            } else {
                film.ids.definir(k, v);
            }
        }
    }
    for a in &m.artwork {
        let t = vers_type_fichier(a.type_artwork);
        let url = if a.url_originale.is_empty() {
            &a.url_apercu
        } else {
            &a.url_originale
        };
        if url.is_empty() || (ne && film.urls_artwork.contains_key(&t)) {
            continue;
        }
        film.urls_artwork.entry(t).or_insert_with(|| url.clone());
    }
    if !m.fournisseur.is_empty() {
        film.dernier_scraper = m.fournisseur.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> sc::MetadonneesFilm {
        let mut m = sc::MetadonneesFilm {
            fournisseur: "tmdb".into(),
            titre: "Origine".into(),
            titre_original: "Inception".into(),
            annee: 2010,
            resume: "Résumé.".into(),
            duree: 148,
            genres: vec!["SCIENCE_FICTION".into(), "Polar".into()],
            classifications: vec![
                sc::Classification {
                    pays: "US".into(),
                    valeur: "PG-13".into(),
                },
                sc::Classification {
                    pays: "FR".into(),
                    valeur: "12".into(),
                },
            ],
            evaluations: vec![sc::Evaluation {
                source: "tmdb".into(),
                valeur: 8.4,
                votes: 100,
                maximum: 10.0,
            }],
            personnes: vec![
                sc::Personne {
                    type_personne: sc::TypePersonne::Acteur,
                    nom: "A".into(),
                    role: "Cobb".into(),
                    ids: BTreeMap::new(),
                    url_photo: String::new(),
                    url_profil: String::new(),
                },
                sc::Personne {
                    type_personne: sc::TypePersonne::Realisateur,
                    nom: "R".into(),
                    role: "Director".into(),
                    ids: BTreeMap::new(),
                    url_photo: String::new(),
                    url_profil: String::new(),
                },
            ],
            ..Default::default()
        };
        m.ids.insert("tmdb".into(), "27205".into());
        m.ids.insert("imdb".into(), "tt1375666".into());
        m
    }

    #[test]
    fn application_complete() {
        let mut f = Film {
            titre: "inception".into(),
            ..Film::default()
        };
        appliquer_metadonnees(&mut f, &meta(), &ConfigMetadonnees::tout(), "FR");
        assert_eq!(f.titre, "Origine");
        assert_eq!(f.annee, 2010);
        assert_eq!(f.certification, "FR:12");
        assert_eq!(f.genres, vec!["Science Fiction", "Polar"]);
        assert_eq!(f.acteurs.len(), 1);
        assert_eq!(f.realisateurs().len(), 1);
        assert_eq!(f.ids.tmdb(), 27205);
        assert_eq!(f.ids.imdb(), "tt1375666");
        assert!((f.notes["tmdb"].valeur - 8.4).abs() < 1e-6);
        assert_eq!(f.dernier_scraper, "tmdb");
    }

    #[test]
    fn champs_desactives_et_ne_pas_ecraser() {
        let mut f = Film {
            titre: "Mon titre".into(),
            resume: String::new(),
            annee: 1999,
            ..Film::default()
        };
        let config = ConfigMetadonnees {
            ne_pas_ecraser: true,
            genres: false,
            ..ConfigMetadonnees::tout()
        };
        appliquer_metadonnees(&mut f, &meta(), &config, "FR");
        assert_eq!(f.titre, "Mon titre");
        assert_eq!(f.annee, 1999);
        assert_eq!(f.resume, "Résumé.");
        assert!(f.genres.is_empty());
    }

    #[test]
    fn identifiants_aller_retour() {
        let mut t = BTreeMap::new();
        t.insert("tmdb".to_string(), "603".to_string());
        t.insert("imdb".to_string(), "invalide".to_string());
        let ids = vers_identifiants(&t);
        assert_eq!(ids.tmdb(), 603);
        assert!(!ids.contient("imdb"));
        assert_eq!(depuis_identifiants(&ids)["tmdb"], "603");
    }
}
