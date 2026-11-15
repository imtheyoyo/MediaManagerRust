//! Module Collections : arbre saga → films (logique pure, sans egui).

use crate::modele::{Film, normaliser};
use crate::source_donnees::SagaAffichage;
use std::collections::BTreeSet;

/// Nœud sélectionné dans l'arbre des sagas : la saga, ou un de ses films.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoeudSaga {
    Saga(String),
    Film(String, u64),
}

impl NoeudSaga {
    pub fn id_saga(&self) -> &str {
        match self {
            NoeudSaga::Saga(id) | NoeudSaga::Film(id, _) => id,
        }
    }
}

/// Une ligne affichée de l'arbre des sagas.
#[derive(Clone, Debug, PartialEq)]
pub struct LigneSaga {
    pub noeud: NoeudSaga,
    pub profondeur: u8,
    pub titre: String,
    /// Année du film, ou première année des films de la saga (0 : inconnue).
    pub annee: u32,
    /// Nombre de films (lignes de saga seulement).
    pub films: Option<usize>,
    /// `Some(déployée)` si la saga a des films.
    pub deploiement: Option<bool>,
}

/// Films d'une saga présents dans la bibliothèque : identifiants listés par la saga, plus les
/// films qui portent son titre ; triés par année puis titre.
pub fn films_de_saga<'a>(films: &'a [Film], saga: &SagaAffichage) -> Vec<&'a Film> {
    let mut v: Vec<&Film> = films
        .iter()
        .filter(|f| saga.films.contains(&f.id) || f.saga.as_deref() == Some(saga.titre.as_str()))
        .collect();
    v.sort_by(|a, b| {
        let cle = |f: &Film| (f.annee == 0, f.annee);
        cle(a)
            .cmp(&cle(b))
            .then_with(|| normaliser(&a.titre_triable).cmp(&normaliser(&b.titre_triable)))
            .then(a.id.cmp(&b.id))
    });
    v
}

/// Sagas retenues par la recherche (titre de la saga ou d'un de ses films), triées par titre.
pub fn sagas_filtrees<'a>(
    sagas: &'a [SagaAffichage],
    films: &[Film],
    recherche: &str,
) -> Vec<&'a SagaAffichage> {
    let motif = normaliser(recherche.trim());
    let mut v: Vec<&SagaAffichage> = sagas
        .iter()
        .filter(|s| {
            motif.is_empty()
                || normaliser(&s.titre).contains(&motif)
                || films_de_saga(films, s)
                    .iter()
                    .any(|f| normaliser(&f.titre).contains(&motif))
        })
        .collect();
    v.sort_by(|a, b| {
        normaliser(&a.titre)
            .cmp(&normaliser(&b.titre))
            .then(a.id.cmp(&b.id))
    });
    v
}

/// Lignes visibles de l'arbre (films des sagas déployées seulement).
pub fn lignes_sagas(
    sagas: &[SagaAffichage],
    films: &[Film],
    recherche: &str,
    deployees: &BTreeSet<String>,
) -> Vec<LigneSaga> {
    let mut lignes = Vec::new();
    for s in sagas_filtrees(sagas, films, recherche) {
        let membres = films_de_saga(films, s);
        let deployee = deployees.contains(&s.id);
        lignes.push(LigneSaga {
            noeud: NoeudSaga::Saga(s.id.clone()),
            profondeur: 0,
            titre: s.titre.clone(),
            annee: membres
                .iter()
                .map(|f| f.annee)
                .filter(|a| *a > 0)
                .min()
                .unwrap_or(0),
            films: Some(membres.len()),
            deploiement: (!membres.is_empty()).then_some(deployee),
        });
        if deployee {
            lignes.extend(membres.iter().map(|f| LigneSaga {
                noeud: NoeudSaga::Film(s.id.clone(), f.id),
                profondeur: 1,
                titre: f.titre.clone(),
                annee: f.annee,
                films: None,
                deploiement: None,
            }));
        }
    }
    lignes
}

/// Taille cumulée des films d'une saga.
pub fn taille_saga(films: &[Film], saga: &SagaAffichage) -> u64 {
    films_de_saga(films, saga)
        .iter()
        .map(|f| f.taille_octets)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn film(id: u64, titre: &str, annee: u32, saga: Option<&str>) -> Film {
        Film {
            id,
            titre: titre.into(),
            titre_triable: titre.into(),
            annee,
            saga: saga.map(Into::into),
            taille_octets: 10,
            ..Film::default()
        }
    }

    fn donnees() -> (Vec<SagaAffichage>, Vec<Film>) {
        let films = vec![
            film(1, "Alien, le retour", 1986, None),
            film(2, "Alien", 1979, None),
            film(3, "Matrix", 1999, Some("Matrix")),
            film(4, "Prometheus", 0, None),
            film(5, "Seul", 2000, None),
        ];
        let sagas = vec![
            SagaAffichage {
                id: "m".into(),
                titre: "Matrix".into(),
                ..Default::default()
            },
            SagaAffichage {
                id: "a".into(),
                titre: "Alien".into(),
                films: vec![1, 4, 2, 99],
                ..Default::default()
            },
            SagaAffichage {
                id: "v".into(),
                titre: "Vide".into(),
                ..Default::default()
            },
        ];
        (sagas, films)
    }

    #[test]
    fn arbre_trie_et_deploiement() {
        let (sagas, films) = donnees();
        let mut deployees = BTreeSet::new();
        let l = lignes_sagas(&sagas, &films, "", &deployees);
        let titres: Vec<&str> = l.iter().map(|x| x.titre.as_str()).collect();
        assert_eq!(titres, ["Alien", "Matrix", "Vide"]);
        assert_eq!(l[0].films, Some(3));
        assert_eq!(l[0].annee, 1979);
        assert_eq!(l[0].deploiement, Some(false));
        // Saga sans film : pas de triangle ; rattachement par titre pour Matrix.
        assert_eq!(l[2].deploiement, None);
        assert_eq!(l[1].films, Some(1));

        deployees.insert("a".to_string());
        let l = lignes_sagas(&sagas, &films, "", &deployees);
        assert_eq!(l.len(), 3 + 3);
        // Films par année, l'année inconnue en dernier.
        let enfants: Vec<&NoeudSaga> = l[1..4].iter().map(|x| &x.noeud).collect();
        assert_eq!(
            enfants,
            [
                &NoeudSaga::Film("a".into(), 2),
                &NoeudSaga::Film("a".into(), 1),
                &NoeudSaga::Film("a".into(), 4)
            ]
        );
        assert_eq!(l[1].profondeur, 1);
        assert_eq!(taille_saga(&films, &sagas[1]), 30);
    }

    #[test]
    fn recherche_sur_la_saga_ou_ses_films() {
        let (sagas, films) = donnees();
        let aucune = BTreeSet::new();
        let l = lignes_sagas(&sagas, &films, "prométhéus", &aucune);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].noeud, NoeudSaga::Saga("a".into()));
        assert_eq!(lignes_sagas(&sagas, &films, "vid", &aucune).len(), 1);
        assert_eq!(NoeudSaga::Film("a".into(), 1).id_saga(), "a");
    }
}
