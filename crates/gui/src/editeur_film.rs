//! État (sans egui) de la fenêtre « Éditer un film » : onglets, copie éditable du film,
//! sélections des tableaux, validation et transitions. Testable sans interface.

use crate::conversion::date_heure;
use crate::dates;
use crate::details_film::{DetailsFilm, FonctionPersonne, NoteSource, note_principale};
use crate::dialogues::liste;
use crate::modele::Film;

// ------------------------------------------------------------------ onglets

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OngletEditeur {
    Details,
    Details2,
    Distribution,
    Illustrations,
    Urls,
    Fichiers,
}

impl OngletEditeur {
    pub const TOUS: [OngletEditeur; 6] = [
        OngletEditeur::Details,
        OngletEditeur::Details2,
        OngletEditeur::Distribution,
        OngletEditeur::Illustrations,
        OngletEditeur::Urls,
        OngletEditeur::Fichiers,
    ];

    pub fn cle(self) -> &'static str {
        match self {
            OngletEditeur::Details => "editor.tab.details",
            OngletEditeur::Details2 => "editor.tab.details2",
            OngletEditeur::Distribution => "editor.tab.cast",
            OngletEditeur::Illustrations => "editor.tab.artwork",
            OngletEditeur::Urls => "editor.tab.urls",
            OngletEditeur::Fichiers => "editor.tab.media",
        }
    }
}

// ------------------------------------------------------------------ sélections (interface)

/// Sélections, champs d'ajout et fichier courant des onglets. Ils vivent dans le brouillon
/// pour suivre le film édité (ils sont remis à zéro à chaque film).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelectionsEditeur {
    pub acteur: Option<usize>,
    pub equipe: Option<usize>,
    pub genre: Option<usize>,
    pub mot_cle: Option<usize>,
    pub serie: Option<usize>,
    pub identifiant: Option<usize>,
    pub note: Option<usize>,
    pub bande_annonce: Option<usize>,
    /// Fichier affiché de l'onglet « Fichiers médias ».
    pub fichier: usize,
    pub audio: Option<usize>,
    pub sous_titre: Option<usize>,
    pub ajout_genre: String,
    pub ajout_mot_cle: String,
    pub ajout_serie: String,
}

/// Valeurs proposées par les listes déroulantes (calculées à l'ouverture de l'éditeur).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OptionsEditeur {
    pub sagas: Vec<String>,
    pub series: Vec<String>,
    pub genres: Vec<String>,
    pub mots_cles: Vec<String>,
    pub certifications: Vec<String>,
}

// ------------------------------------------------------------------ opérations sur les listes

/// Déplace l'élément sélectionné d'un cran et suit la sélection.
pub fn deplacer<T>(liste: &mut [T], selection: &mut Option<usize>, vers_le_haut: bool) {
    let Some(n) = *selection else { return };
    if vers_le_haut && n > 0 && n < liste.len() {
        liste.swap(n, n - 1);
        *selection = Some(n - 1);
    } else if !vers_le_haut && n + 1 < liste.len() {
        liste.swap(n, n + 1);
        *selection = Some(n + 1);
    }
}

/// Retire l'élément sélectionné ; la sélection passe à l'élément qui prend sa place.
pub fn retirer<T>(liste: &mut Vec<T>, selection: &mut Option<usize>) {
    let Some(n) = *selection else { return };
    if n < liste.len() {
        liste.remove(n);
    }
    *selection = if liste.is_empty() {
        None
    } else {
        Some(n.min(liste.len() - 1))
    };
}

/// Ajoute `valeur` (rognée, non vide, absente de la liste) et la sélectionne.
pub fn ajouter_texte(liste: &mut Vec<String>, selection: &mut Option<usize>, valeur: &str) {
    let v = valeur.trim();
    if v.is_empty() {
        return;
    }
    match liste.iter().position(|x| x.eq_ignore_ascii_case(v)) {
        Some(n) => *selection = Some(n),
        None => {
            liste.push(v.to_string());
            *selection = Some(liste.len() - 1);
        }
    }
}

// ------------------------------------------------------------------ brouillon

/// Copie éditable d'un film (champs numériques sous forme de texte pour la validation).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FilmEdite {
    pub titre: String,
    pub titre_original: String,
    pub titre_triable: String,
    pub annee: String,
    /// `jj/mm/aaaa` (vide : inconnue).
    pub date_sortie: String,
    pub duree: String,
    pub certification: String,
    pub vu: bool,
    pub verrouille: bool,
    pub slogan: String,
    pub resume: String,
    pub genres: Vec<String>,
    /// Listes séparées par des virgules.
    pub pays: String,
    pub production: String,
    pub saga: String,
    pub details: DetailsFilm,
    /// Notes et évaluation d'origine : la note affichée n'est recalculée que si elles changent.
    pub notes_origine: (Vec<NoteSource>, f32),
    pub selections: SelectionsEditeur,
}

impl FilmEdite {
    pub fn depuis(f: &Film, details: DetailsFilm) -> Self {
        Self {
            titre: f.titre.clone(),
            titre_original: f.titre_original.clone(),
            titre_triable: if f.titre_triable == f.titre {
                String::new()
            } else {
                f.titre_triable.clone()
            },
            annee: f.annee.to_string(),
            date_sortie: dates::iso_vers_fr(&f.date_sortie),
            duree: f.duree_min.to_string(),
            certification: f.certification.clone(),
            vu: f.vu,
            verrouille: f.verrouille,
            slogan: f.slogan.clone(),
            resume: f.resume.clone(),
            genres: f.genres.clone(),
            pays: f.pays.join(", "),
            production: f.production.join(", "),
            saga: f.saga.clone().unwrap_or_default(),
            notes_origine: (details.notes.clone(), details.evaluation_utilisateur),
            details,
            selections: SelectionsEditeur::default(),
        }
    }

    /// Écrit le brouillon (supposé valide) dans le film d'affichage.
    pub fn appliquer(&self, f: &mut Film) {
        let d = &self.details;
        f.titre = self.titre.trim().to_string();
        f.titre_original = self.titre_original.trim().to_string();
        f.titre_triable = if self.titre_triable.trim().is_empty() {
            f.titre.clone()
        } else {
            self.titre_triable.trim().to_string()
        };
        f.annee = self.annee.trim().parse().unwrap_or(f.annee);
        f.date_sortie =
            dates::fr_vers_iso(&self.date_sortie).unwrap_or_else(|| f.date_sortie.clone());
        f.duree_min = self.duree.trim().parse().unwrap_or(f.duree_min);
        f.certification = self.certification.trim().to_string();
        f.vu = self.vu;
        f.verrouille = self.verrouille;
        f.slogan = self.slogan.clone();
        f.resume = self.resume.clone();
        f.genres = self
            .genres
            .iter()
            .map(|g| g.trim().to_string())
            .filter(|g| !g.is_empty())
            .collect();
        f.pays = liste(&self.pays);
        f.production = liste(&self.production);
        f.saga = Some(self.saga.trim().to_string()).filter(|s| !s.is_empty());
        let id = |cle: &str| {
            d.ids
                .iter()
                .find(|(k, _)| k.trim().eq_ignore_ascii_case(cle))
                .map(|(_, v)| v.trim().to_string())
                .unwrap_or_default()
        };
        f.imdb = id("imdb");
        f.tmdb = id("tmdb");
        let noms = |fonction: FonctionPersonne| -> Vec<String> {
            d.equipe
                .iter()
                .filter(|p| p.fonction_effective() == fonction && !p.nom.trim().is_empty())
                .map(|p| p.nom.trim().to_string())
                .collect()
        };
        f.realisateurs = noms(FonctionPersonne::Realisateur);
        f.scenaristes = noms(FonctionPersonne::Scenariste);
        f.acteurs = d
            .acteurs
            .iter()
            .filter(|p| !p.nom.trim().is_empty())
            .map(|p| (p.nom.clone(), p.role.clone()))
            .collect();
        if (&d.notes, d.evaluation_utilisateur) != (&self.notes_origine.0, self.notes_origine.1) {
            let valides = d.notes_valides();
            let (note, votes) = note_principale(&valides);
            f.note = note;
            f.votes = votes;
            f.notes_sources = valides
                .iter()
                .map(|n| (n.source.clone(), n.valeur * 10.0 / n.maximum as f32))
                .collect();
            if d.evaluation_utilisateur > 0.0 {
                f.notes_sources
                    .push(("user".to_string(), d.evaluation_utilisateur));
            }
        }
        if let Some(ms) = dates::resoudre_horodatage(&d.date_ajout, Some(d.date_ajout_ms)) {
            f.date_ajout = date_heure(ms);
        }
    }
}

impl DetailsFilm {
    /// Notes exploitables du tableau (source renseignée, valeur positive, échelle non nulle).
    pub fn notes_valides(&self) -> Vec<NoteSource> {
        self.notes
            .iter()
            .filter(|n| !n.source.trim().is_empty() && n.valeur > 0.0 && n.maximum > 0)
            .cloned()
            .collect()
    }
}

// ------------------------------------------------------------------ validation

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChampEditeur {
    Titre,
    Annee,
    DateSortie,
    Duree,
    Imdb,
    Tmdb,
    DateAjout,
    DateVu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErreurChamp {
    pub champ: ChampEditeur,
    pub cle: &'static str,
}

/// Validation du brouillon ; liste vide = valide.
pub fn valider(b: &FilmEdite) -> Vec<ErreurChamp> {
    let mut e = Vec::new();
    let mut err = |champ, cle| e.push(ErreurChamp { champ, cle });
    if b.titre.trim().is_empty() {
        err(ChampEditeur::Titre, "editor.error.title");
    }
    match b.annee.trim().parse::<u32>() {
        Ok(a) if (1870..=2100).contains(&a) => {}
        _ => err(ChampEditeur::Annee, "editor.error.year"),
    }
    if !dates::date_fr_valide(&b.date_sortie) {
        err(ChampEditeur::DateSortie, "editor.error.date");
    }
    if b.duree.trim().parse::<u32>().is_err() {
        err(ChampEditeur::Duree, "editor.error.runtime");
    }
    let id = |cle: &str| {
        b.details
            .ids
            .iter()
            .find(|(k, _)| k.trim().eq_ignore_ascii_case(cle))
            .map(|(_, v)| v.trim().to_string())
            .unwrap_or_default()
    };
    let imdb = id("imdb");
    if !imdb.is_empty()
        && !(imdb.starts_with("tt")
            && imdb.len() >= 9
            && imdb[2..].chars().all(|c| c.is_ascii_digit()))
    {
        err(ChampEditeur::Imdb, "editor.error.imdb");
    }
    let tmdb = id("tmdb");
    if !tmdb.is_empty() && !tmdb.chars().all(|c| c.is_ascii_digit()) {
        err(ChampEditeur::Tmdb, "editor.error.tmdb");
    }
    if dates::fr_heure_vers_ms(&b.details.date_ajout).is_err() {
        err(ChampEditeur::DateAjout, "editor.error.datetime");
    }
    if dates::fr_heure_vers_ms(&b.details.date_vu).is_err() {
        err(ChampEditeur::DateVu, "editor.error.datetime");
    }
    e
}

// ------------------------------------------------------------------ état et actions

#[derive(Clone, Debug)]
pub struct EtatEditeur {
    pub id: u64,
    pub onglet: OngletEditeur,
    pub brouillon: FilmEdite,
    pub options: OptionsEditeur,
    pub erreurs: Vec<ErreurChamp>,
    /// Films restant à éditer après celui-ci (édition à la suite).
    pub file: Vec<u64>,
    pub total: usize,
}

impl EtatEditeur {
    pub fn erreur(&self, champ: ChampEditeur) -> Option<&'static str> {
        self.erreurs
            .iter()
            .find(|e| e.champ == champ)
            .map(|e| e.cle)
    }

    pub fn position(&self) -> usize {
        self.total - self.file.len()
    }
}

#[derive(Clone, Debug)]
pub enum ActionEditeur {
    Onglet(OngletEditeur),
    Brouillon(Box<FilmEdite>),
    Valider,
    /// « Annuler » : abandonne les modifications de ce film et passe au suivant de la file.
    Ignorer,
    Abandonner,
    /// Bouton ▷ du titre : enregistre le brouillon puis ouvre la recherche d'extracteur.
    Rechercher,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deplacement_suit_la_selection() {
        let mut v = vec!["a", "b", "c"];
        let mut s = Some(2);
        deplacer(&mut v, &mut s, true);
        assert_eq!((v.clone(), s), (vec!["a", "c", "b"], Some(1)));
        deplacer(&mut v, &mut s, true);
        deplacer(&mut v, &mut s, true);
        assert_eq!((v.clone(), s), (vec!["c", "a", "b"], Some(0)));
        deplacer(&mut v, &mut s, false);
        assert_eq!((v.clone(), s), (vec!["a", "c", "b"], Some(1)));
        let mut aucune = None;
        deplacer(&mut v, &mut aucune, false);
        assert_eq!(v, vec!["a", "c", "b"]);
    }

    #[test]
    fn retrait_et_ajout() {
        let mut v = vec!["a".to_string(), "b".into(), "c".into()];
        let mut s = Some(2);
        retirer(&mut v, &mut s);
        assert_eq!((v.len(), s), (2, Some(1)));
        retirer(&mut v, &mut s);
        retirer(&mut v, &mut s);
        assert_eq!((v.len(), s), (0, None));
        retirer(&mut v, &mut s);
        ajouter_texte(&mut v, &mut s, "  Action ");
        ajouter_texte(&mut v, &mut s, "ACTION");
        ajouter_texte(&mut v, &mut s, "   ");
        assert_eq!((v, s), (vec!["Action".to_string()], Some(0)));
    }

    #[test]
    fn validation_des_champs() {
        let mut b = FilmEdite {
            titre: "Alien".into(),
            annee: "1979".into(),
            duree: "117".into(),
            ..FilmEdite::default()
        };
        assert!(valider(&b).is_empty());
        b.titre.clear();
        b.annee = "1500".into();
        b.date_sortie = "31/02/2026".into();
        b.details.ids = vec![("imdb".into(), "xx".into()), ("tmdb".into(), "a1".into())];
        b.details.date_ajout = "hier".into();
        b.details.date_vu = "01/01/2026 99:00".into();
        let champs: Vec<ChampEditeur> = valider(&b).into_iter().map(|e| e.champ).collect();
        for c in [
            ChampEditeur::Titre,
            ChampEditeur::Annee,
            ChampEditeur::DateSortie,
            ChampEditeur::Imdb,
            ChampEditeur::Tmdb,
            ChampEditeur::DateAjout,
            ChampEditeur::DateVu,
        ] {
            assert!(champs.contains(&c), "{c:?} non détecté");
        }
    }

    #[test]
    fn brouillon_aller_retour() {
        let f = crate::donnees::films_factices().remove(0);
        let b = FilmEdite::depuis(&f, DetailsFilm::depuis_film(&f));
        assert!(valider(&b).is_empty(), "{:?}", valider(&b));
        let mut g = f.clone();
        b.appliquer(&mut g);
        assert_eq!(g, f);
    }

    #[test]
    fn modifications_reportees_dans_le_film() {
        let f = crate::donnees::films_factices().remove(0);
        let mut b = FilmEdite::depuis(&f, DetailsFilm::depuis_film(&f));
        b.pays = "France, , Italie".into();
        b.date_sortie = "08/07/2026".into();
        b.saga = " Saga X ".into();
        b.details.ids = vec![
            ("tmdb".into(), "348".into()),
            ("imdb".into(), String::new()),
        ];
        b.details.notes = vec![NoteSource {
            source: "tmdb".into(),
            valeur: 8.0,
            maximum: 10,
            votes: 321,
        }];
        b.details.evaluation_utilisateur = 9.0;
        b.details.acteurs = vec![crate::details_film::PersonneFilm {
            nom: "Sigourney Weaver".into(),
            role: "Ripley".into(),
            ..Default::default()
        }];
        b.details.equipe = vec![
            crate::details_film::PersonneFilm {
                nom: "Ridley Scott".into(),
                role: "Director".into(),
                ..Default::default()
            },
            crate::details_film::PersonneFilm {
                nom: "Dan O'Bannon".into(),
                role: "Writer".into(),
                ..Default::default()
            },
        ];
        b.details.date_ajout = "15/11/2026 08:30".into();
        assert!(valider(&b).is_empty(), "{:?}", valider(&b));
        let mut g = f.clone();
        b.appliquer(&mut g);
        assert_eq!(g.pays, ["France", "Italie"]);
        assert_eq!(g.date_sortie, "2026-07-08");
        assert_eq!(g.saga.as_deref(), Some("Saga X"));
        assert_eq!((g.tmdb.as_str(), g.imdb.as_str()), ("348", ""));
        assert_eq!((g.note, g.votes), (8.0, 321));
        assert_eq!(g.notes_sources.len(), 2);
        assert_eq!(
            g.acteurs,
            [("Sigourney Weaver".to_string(), "Ripley".to_string())]
        );
        assert_eq!(g.realisateurs, ["Ridley Scott"]);
        assert_eq!(g.scenaristes, ["Dan O'Bannon"]);
        assert_eq!(g.date_ajout, "2026-11-15 08:30");
    }
}
