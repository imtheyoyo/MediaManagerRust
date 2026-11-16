//! État (sans egui) de la fenêtre « Éditer une série » : onglets, copie éditable de la série,
//! validation et transitions. Testable sans interface.

use crate::dates;
use crate::details_film::NoteSource;
use crate::details_serie::DetailsSerie;
use crate::editeur_film::{OptionsEditeur, SelectionsEditeur};
use crate::etat::NoeudSerie;
use crate::source_donnees::{SerieAffichage, StatutSerie};

// ------------------------------------------------------------------ onglets

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OngletSerie {
    Details,
    Distribution,
    Illustrations,
    Urls,
    Episodes,
}

impl OngletSerie {
    pub const TOUS: [OngletSerie; 5] = [
        OngletSerie::Details,
        OngletSerie::Distribution,
        OngletSerie::Illustrations,
        OngletSerie::Urls,
        OngletSerie::Episodes,
    ];

    pub fn cle(self) -> &'static str {
        match self {
            OngletSerie::Details => "editor.tab.details",
            OngletSerie::Distribution => "editor.tab.cast",
            OngletSerie::Illustrations => "editor.tab.artwork",
            OngletSerie::Urls => "serieeditor.tab.urls",
            OngletSerie::Episodes => "serieeditor.tab.episodes",
        }
    }
}

// ------------------------------------------------------------------ brouillon

/// Copie éditable d'une série (champs numériques sous forme de texte pour la validation).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct SerieEdite {
    pub titre: String,
    pub titre_original: String,
    pub annee: String,
    pub resume: String,
    pub genres: Vec<String>,
    pub statut: StatutSerie,
    pub verrouillee: bool,
    pub details: DetailsSerie,
    /// Notes et évaluation d'origine : la note affichée n'est recalculée que si elles changent.
    pub notes_origine: (Vec<NoteSource>, f32),
    pub selections: SelectionsEditeur,
}

impl SerieEdite {
    pub fn depuis(s: &SerieAffichage, details: DetailsSerie) -> Self {
        Self {
            titre: s.titre.clone(),
            titre_original: s.titre_original.clone(),
            annee: if s.annee > 0 {
                s.annee.to_string()
            } else {
                String::new()
            },
            resume: s.resume.clone(),
            genres: s.genres.clone(),
            statut: s.statut,
            verrouillee: s.verrouillee,
            notes_origine: (details.notes.clone(), details.evaluation_utilisateur),
            details,
            selections: SelectionsEditeur::default(),
        }
    }

    /// Écrit le brouillon (supposé valide) dans la série affichée.
    pub fn appliquer(&self, s: &mut SerieAffichage) {
        let d = &self.details;
        s.titre = self.titre.trim().to_string();
        s.titre_original = self.titre_original.trim().to_string();
        s.annee = self.annee.trim().parse().unwrap_or(0);
        s.resume = self.resume.clone();
        s.genres = self
            .genres
            .iter()
            .map(|g| g.trim().to_string())
            .filter(|g| !g.is_empty())
            .collect();
        s.statut = self.statut;
        s.verrouillee = self.verrouillee;
        s.imdb = d.id("imdb");
        s.tmdb = d.id("tmdb");
        s.tvdb = d.id("tvdb");
        if (&d.notes, d.evaluation_utilisateur) != (&self.notes_origine.0, self.notes_origine.1) {
            s.note = d.note_affichee();
        }
    }
}

// ------------------------------------------------------------------ validation

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChampSerie {
    Titre,
    Annee,
    PremiereDiffusion,
    Imdb,
    Tmdb,
    Tvdb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErreurChampSerie {
    pub champ: ChampSerie,
    pub cle: &'static str,
}

/// Validation du brouillon d'une série ; liste vide = valide. L'année, la première diffusion et
/// les identifiants peuvent rester vides.
pub fn valider_serie(b: &SerieEdite) -> Vec<ErreurChampSerie> {
    let mut e = Vec::new();
    let mut err = |champ, cle| e.push(ErreurChampSerie { champ, cle });
    if b.titre.trim().is_empty() {
        err(ChampSerie::Titre, "editor.error.title");
    }
    let annee = b.annee.trim();
    if !annee.is_empty()
        && !annee
            .parse::<u32>()
            .is_ok_and(|a| (1870..=2100).contains(&a))
    {
        err(ChampSerie::Annee, "editor.error.year");
    }
    if !dates::date_fr_valide(&b.details.premiere_diffusion) {
        err(ChampSerie::PremiereDiffusion, "editor.error.date");
    }
    let imdb = b.details.id("imdb");
    if !imdb.is_empty()
        && !(imdb.starts_with("tt")
            && imdb.len() >= 9
            && imdb[2..].chars().all(|c| c.is_ascii_digit()))
    {
        err(ChampSerie::Imdb, "editor.error.imdb");
    }
    let tmdb = b.details.id("tmdb");
    if !tmdb.is_empty() && !tmdb.chars().all(|c| c.is_ascii_digit()) {
        err(ChampSerie::Tmdb, "editor.error.tmdb");
    }
    let tvdb = b.details.id("tvdb");
    if !tvdb.is_empty() && !tvdb.chars().all(|c| c.is_ascii_digit()) {
        err(ChampSerie::Tvdb, "editor.error.tvdb");
    }
    e
}

// ------------------------------------------------------------------ état et actions

#[derive(Clone, Debug)]
pub struct EtatEditeurSerie {
    /// Identifiant de la série éditée.
    pub id: String,
    pub onglet: OngletSerie,
    pub brouillon: SerieEdite,
    pub options: OptionsEditeur,
    pub erreurs: Vec<ErreurChampSerie>,
}

impl EtatEditeurSerie {
    pub fn erreur(&self, champ: ChampSerie) -> Option<&'static str> {
        self.erreurs
            .iter()
            .find(|e| e.champ == champ)
            .map(|e| e.cle)
    }
}

#[derive(Clone, Debug)]
pub enum ActionEditeurSerie {
    Onglet(OngletSerie),
    Brouillon(Box<SerieEdite>),
    /// Bascule « vu » d'un nœud de l'arbre des épisodes (appliquée tout de suite).
    BasculerVu(NoeudSerie),
    Valider,
    Abandonner,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brouillon() -> SerieEdite {
        let s = SerieAffichage {
            id: "s1".into(),
            titre: "Série".into(),
            annee: 2019,
            note: 7.8,
            tmdb: "1399".into(),
            ..SerieAffichage::default()
        };
        SerieEdite::depuis(&s, DetailsSerie::depuis_serie(&s))
    }

    #[test]
    fn brouillon_aller_retour() {
        let s = crate::source_donnees::series_demo().remove(0);
        let b = SerieEdite::depuis(&s, DetailsSerie::depuis_serie(&s));
        assert!(valider_serie(&b).is_empty(), "{:?}", valider_serie(&b));
        let mut g = s.clone();
        b.appliquer(&mut g);
        assert_eq!(g, s);
    }

    #[test]
    fn validation_des_champs() {
        let mut b = brouillon();
        assert!(valider_serie(&b).is_empty());
        b.titre.clear();
        b.annee = "1500".into();
        b.details.premiere_diffusion = "31/02/2026".into();
        b.details.ids = vec![
            ("imdb".into(), "xx".into()),
            ("tmdb".into(), "a1".into()),
            ("tvdb".into(), "b".into()),
        ];
        let champs: Vec<ChampSerie> = valider_serie(&b).into_iter().map(|e| e.champ).collect();
        for c in [
            ChampSerie::Titre,
            ChampSerie::Annee,
            ChampSerie::PremiereDiffusion,
            ChampSerie::Imdb,
            ChampSerie::Tmdb,
            ChampSerie::Tvdb,
        ] {
            assert!(champs.contains(&c), "{c:?} non détecté");
        }
    }

    #[test]
    fn modifications_reportees_dans_la_serie() {
        let mut b = brouillon();
        b.annee = "2020".into();
        b.genres = vec![" Drame ".into(), String::new()];
        b.details.ids = vec![("imdb".into(), "tt1234567".into())];
        b.details.evaluation_utilisateur = 9.0;
        let mut s = SerieAffichage::default();
        b.appliquer(&mut s);
        assert_eq!(
            (s.annee, s.genres.clone()),
            (2020, vec!["Drame".to_string()])
        );
        assert_eq!((s.imdb.as_str(), s.tmdb.as_str()), ("tt1234567", ""));
        assert_eq!(s.note, 9.0);
    }
}
