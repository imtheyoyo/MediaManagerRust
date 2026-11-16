//! Données d'une série propres à la fenêtre « Éditer une série » : tout ce que le modèle
//! d'affichage `SerieAffichage` ne porte pas (titres, première diffusion, pays, production,
//! notes par source, identifiants, mots clés, distribution, illustrations et adresses
//! d'artwork). La conversion depuis et vers le modèle des services est faite par
//! `conversion_details_serie.rs`.

use crate::details_film::{ImagesFilm, NoteSource, PersonneFilm, note_principale};
use crate::source_donnees::SerieAffichage;

/// Données éditables d'une série qui s'ajoutent à celles du modèle d'affichage.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DetailsSerie {
    pub titre_anglais: String,
    /// Titre de tri (vide : le titre).
    pub titre_triable: String,
    pub slogan: String,
    /// « Note » : texte libre de l'utilisateur.
    pub note_libre: String,
    /// Date de première diffusion, `jj/mm/aaaa` (vide : inconnue).
    pub premiere_diffusion: String,
    /// Durée d'un épisode, en minutes (0 : inconnue).
    pub duree: u32,
    pub certification: String,
    pub pays: String,
    /// Production / réseau (société de production du modèle).
    pub production: String,
    /// « Évaluation des utilisateurs » (0 = aucune).
    pub evaluation_utilisateur: f32,
    pub top250: u32,
    pub notes: Vec<NoteSource>,
    /// Identifiants externes (clé, valeur).
    pub ids: Vec<(String, String)>,
    pub mots_cles: Vec<String>,
    pub acteurs: Vec<PersonneFilm>,
    pub equipe: Vec<PersonneFilm>,
    pub images: ImagesFilm,
    /// Adresses d'artwork, une par emplacement de `TypeArt::TOUS`.
    pub urls: [String; 9],
}

impl DetailsSerie {
    /// Valeurs de départ déduites d'une série d'affichage (sources de démonstration, tests).
    pub fn depuis_serie(s: &SerieAffichage) -> Self {
        let mut ids = Vec::new();
        for (cle, valeur) in [("tmdb", &s.tmdb), ("imdb", &s.imdb), ("tvdb", &s.tvdb)] {
            if !valeur.is_empty() {
                ids.push((cle.to_string(), valeur.clone()));
            }
        }
        Self {
            notes: if s.note > 0.0 {
                vec![NoteSource {
                    source: "tmdb".into(),
                    valeur: s.note,
                    maximum: 10,
                    votes: 0,
                }]
            } else {
                Vec::new()
            },
            ids,
            ..Self::default()
        }
    }

    /// Notes exploitables du tableau (source renseignée, valeur positive, échelle non nulle).
    pub fn notes_valides(&self) -> Vec<NoteSource> {
        self.notes
            .iter()
            .filter(|n| !n.source.trim().is_empty() && n.valeur > 0.0 && n.maximum > 0)
            .cloned()
            .collect()
    }

    /// Note affichée dans la liste : l'évaluation de l'utilisateur, sinon la note principale.
    pub fn note_affichee(&self) -> f32 {
        if self.evaluation_utilisateur > 0.0 {
            self.evaluation_utilisateur
        } else {
            note_principale(&self.notes_valides()).0
        }
    }

    /// Valeur d'un identifiant (sans casse ni espaces) ; vide si absent.
    pub fn id(&self, cle: &str) -> String {
        self.ids
            .iter()
            .find(|(k, _)| k.trim().eq_ignore_ascii_case(cle))
            .map(|(_, v)| v.trim().to_string())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn details_par_defaut_deduits_de_la_serie() {
        let s = SerieAffichage {
            note: 7.5,
            tmdb: "1399".into(),
            tvdb: "121361".into(),
            ..SerieAffichage::default()
        };
        let d = DetailsSerie::depuis_serie(&s);
        assert_eq!(d.id("tmdb"), "1399");
        assert_eq!(d.id("TVDB"), "121361");
        assert_eq!(d.id("imdb"), "");
        assert_eq!(d.note_affichee(), 7.5);
    }

    #[test]
    fn l_evaluation_de_l_utilisateur_prime() {
        let mut d = DetailsSerie {
            notes: vec![NoteSource {
                source: "imdb".into(),
                valeur: 6.0,
                maximum: 10,
                votes: 10,
            }],
            ..DetailsSerie::default()
        };
        assert_eq!(d.note_affichee(), 6.0);
        d.evaluation_utilisateur = 9.0;
        assert_eq!(d.note_affichee(), 9.0);
    }
}
