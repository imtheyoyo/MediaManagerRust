//! Modifications appliquées à plusieurs films ou séries à la fois (« édition
//! globale ») : logique pure, sans accès à la base.

use crate::modele::film::maintenant_ms;
use crate::modele::{Edition, Film, Serie, SourceMedia};

/// Ajoute les valeurs absentes (comparaison insensible à la casse) ;
/// renvoie vrai si la liste a changé.
fn ajouter(liste: &mut Vec<String>, valeurs: &[String]) -> bool {
    let mut change = false;
    for v in valeurs {
        let v = v.trim();
        if v.is_empty() || liste.iter().any(|x| x.eq_ignore_ascii_case(v)) {
            continue;
        }
        liste.push(v.to_string());
        change = true;
    }
    change
}

/// Retire les valeurs présentes (insensible à la casse) ; vrai si changé.
fn retirer(liste: &mut Vec<String>, valeurs: &[String]) -> bool {
    let avant = liste.len();
    liste.retain(|x| !valeurs.iter().any(|v| v.trim().eq_ignore_ascii_case(x)));
    liste.len() != avant
}

fn definir<T: PartialEq + Clone>(cible: &mut T, valeur: &Option<T>) -> bool {
    match valeur {
        Some(v) if v != cible => {
            *cible = v.clone();
            true
        }
        _ => false,
    }
}

/// Modifications d'un lot de films. Chaque champ à `None` (ou liste vide)
/// laisse la valeur existante intacte.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModificationsLot {
    pub genres_ajouter: Vec<String>,
    pub genres_retirer: Vec<String>,
    pub tags_ajouter: Vec<String>,
    pub tags_retirer: Vec<String>,
    pub certification: Option<String>,
    /// Société de production (« studio »).
    pub societe_production: Option<String>,
    /// Pays (séparés par `/`).
    pub pays: Option<String>,
    /// Marque vu / non vu (le nombre de lectures et la date suivent).
    pub vu: Option<bool>,
    pub edition: Option<Edition>,
    pub source_media: Option<SourceMedia>,
    /// Langue d'origine : codes ISO 639-1 séparés par `, `.
    pub langues_parlees: Option<String>,
    pub video_3d: Option<bool>,
    pub note_utilisateur: Option<String>,
}

impl ModificationsLot {
    /// Aucune modification demandée.
    pub fn est_vide(&self) -> bool {
        *self == Self::default()
    }

    /// Applique les modifications ; renvoie vrai si le film a changé.
    pub fn appliquer(&self, film: &mut Film) -> bool {
        let mut c = false;
        c |= ajouter(&mut film.genres, &self.genres_ajouter);
        c |= retirer(&mut film.genres, &self.genres_retirer);
        c |= ajouter(&mut film.tags, &self.tags_ajouter);
        c |= retirer(&mut film.tags, &self.tags_retirer);
        c |= definir(&mut film.certification, &self.certification);
        c |= definir(&mut film.societe_production, &self.societe_production);
        c |= definir(&mut film.pays, &self.pays);
        c |= definir(&mut film.edition, &self.edition);
        c |= definir(&mut film.source_media, &self.source_media);
        c |= definir(&mut film.langues_parlees, &self.langues_parlees);
        c |= definir(&mut film.video_3d, &self.video_3d);
        c |= definir(&mut film.note_utilisateur, &self.note_utilisateur);
        if let Some(vu) = self.vu {
            c |= marquer_vu(
                vu,
                &mut film.vu,
                &mut film.nombre_lectures,
                &mut film.derniere_lecture,
            );
        }
        c
    }
}

/// Marque vu / non vu avec ses compteurs : vu ⇒ au moins une lecture et une
/// date de dernière lecture ; non vu ⇒ compteurs remis à zéro (comme dans le
/// NFO, où `playcount` vaut 0 pour un élément non vu).
pub fn marquer_vu(
    vu: bool,
    drapeau: &mut bool,
    lectures: &mut u32,
    derniere: &mut Option<i64>,
) -> bool {
    let avant = (*drapeau, *lectures, *derniere);
    *drapeau = vu;
    if vu {
        *lectures = (*lectures).max(1);
        if derniere.is_none() {
            *derniere = Some(maintenant_ms());
        }
    } else {
        *lectures = 0;
        *derniere = None;
    }
    avant != (*drapeau, *lectures, *derniere)
}

/// Modifications d'un lot de séries.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModificationsLotSeries {
    pub genres_ajouter: Vec<String>,
    pub genres_retirer: Vec<String>,
    pub tags_ajouter: Vec<String>,
    pub tags_retirer: Vec<String>,
    pub certification: Option<String>,
    pub societe_production: Option<String>,
    pub pays: Option<String>,
    pub note_utilisateur: Option<String>,
}

impl ModificationsLotSeries {
    pub fn est_vide(&self) -> bool {
        *self == Self::default()
    }

    /// Applique les modifications ; vrai si la série a changé.
    pub fn appliquer(&self, serie: &mut Serie) -> bool {
        let mut c = false;
        c |= ajouter(&mut serie.genres, &self.genres_ajouter);
        c |= retirer(&mut serie.genres, &self.genres_retirer);
        c |= ajouter(&mut serie.tags, &self.tags_ajouter);
        c |= retirer(&mut serie.tags, &self.tags_retirer);
        c |= definir(&mut serie.certification, &self.certification);
        c |= definir(&mut serie.societe_production, &self.societe_production);
        c |= definir(&mut serie.pays, &self.pays);
        c |= definir(&mut serie.note_utilisateur, &self.note_utilisateur);
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn genres_ajout_sans_doublon_et_retrait_insensible_a_la_casse() {
        let mut f = Film {
            genres: s(&["Action", "Drame"]),
            ..Film::default()
        };
        let m = ModificationsLot {
            genres_ajouter: s(&["action", "Comédie", " "]),
            genres_retirer: s(&["DRAME"]),
            ..ModificationsLot::default()
        };
        assert!(m.appliquer(&mut f));
        assert_eq!(f.genres, s(&["Action", "Comédie"]));
        assert!(!m.appliquer(&mut f), "idempotent");
    }

    #[test]
    fn champs_scalaires_et_vide() {
        assert!(ModificationsLot::default().est_vide());
        let mut f = Film::default();
        let m = ModificationsLot {
            certification: Some("FR:U".into()),
            edition: Some(Edition::DirectorsCut),
            source_media: Some(SourceMedia::Bluray),
            video_3d: Some(true),
            langues_parlees: Some("fr, en".into()),
            ..ModificationsLot::default()
        };
        assert!(!m.est_vide());
        assert!(m.appliquer(&mut f));
        assert_eq!(f.certification, "FR:U");
        assert_eq!(f.edition, Edition::DirectorsCut);
        assert_eq!(f.source_media, SourceMedia::Bluray);
        assert!(f.video_3d);
        assert_eq!(f.langues_parlees, "fr, en");
    }

    #[test]
    fn vu_met_a_jour_les_compteurs() {
        let mut f = Film::default();
        let vu = ModificationsLot {
            vu: Some(true),
            ..ModificationsLot::default()
        };
        assert!(vu.appliquer(&mut f));
        assert!(f.vu && f.nombre_lectures == 1 && f.derniere_lecture.is_some());
        let non = ModificationsLot {
            vu: Some(false),
            ..ModificationsLot::default()
        };
        assert!(non.appliquer(&mut f));
        assert!(!f.vu && f.nombre_lectures == 0 && f.derniere_lecture.is_none());
    }

    #[test]
    fn series() {
        let mut sr = Serie::default();
        let m = ModificationsLotSeries {
            tags_ajouter: s(&["à voir"]),
            pays: Some("FR".into()),
            ..ModificationsLotSeries::default()
        };
        assert!(m.appliquer(&mut sr));
        assert_eq!(sr.tags, s(&["à voir"]));
        assert_eq!(sr.pays, "FR");
        assert!(!m.appliquer(&mut sr));
    }
}
