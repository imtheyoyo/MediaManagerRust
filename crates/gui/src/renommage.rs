//! Logique (factice) de l'aperçu du renommage : profils, modèles simples `{titre}`, `{annee}`,
//! `{format}`, détection de doublons. Sera remplacée par le moteur du crate `gabarit`.

use crate::modele::{Film, TypeFichier, normaliser};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profil {
    pub nom: String,
    pub modele_dossier: String,
    pub modele_fichier: String,
}

pub fn profils_par_defaut() -> Vec<Profil> {
    vec![
        Profil {
            nom: "Default".into(),
            modele_dossier: "{titre} ({annee})".into(),
            modele_fichier: "{titre} ({annee})".into(),
        },
        Profil {
            nom: "Minimal".into(),
            modele_dossier: "{titre}".into(),
            modele_fichier: "{titre}".into(),
        },
        Profil {
            nom: "Qualité".into(),
            modele_dossier: "{titre} ({annee})".into(),
            modele_fichier: "{titre} ({annee}) - {format}".into(),
        },
    ]
}

/// Remplace les caractères interdits dans un nom de fichier Windows.
pub fn assainir(nom: &str) -> String {
    let remplace: String = nom
        .chars()
        .map(|c| match c {
            ':' => '-',
            '/' | '\\' | '?' | '*' | '"' | '<' | '>' | '|' => '_',
            autre => autre,
        })
        .collect();
    remplace.trim().trim_end_matches('.').to_string()
}

pub fn appliquer_modele(modele: &str, film: &Film) -> String {
    let brut = modele
        .replace("{titre}", &film.titre)
        .replace("{annee}", &film.annee.to_string())
        .replace("{format}", &film.format_video());
    assainir(&brut)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LigneFichier {
    pub type_fichier: TypeFichier,
    pub ancien: String,
    pub nouveau: String,
}

impl LigneFichier {
    pub fn inchange(&self) -> bool {
        self.ancien == self.nouveau
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApercuFilm {
    pub id: u64,
    pub titre: String,
    pub source: String,
    pub ancien_dossier: String,
    pub nouveau_dossier: String,
    pub fichiers: Vec<LigneFichier>,
    /// Vrai si un autre film produit le même dossier cible.
    pub probleme: bool,
}

impl ApercuFilm {
    pub fn rien_a_renommer(&self) -> bool {
        self.ancien_dossier == self.nouveau_dossier
            && self.fichiers.iter().all(LigneFichier::inchange)
    }
}

/// Calcule l'aperçu pour des films, triés par titre, avec détection des doublons de dossier.
pub fn calculer(films: &[&Film], profil: &Profil) -> Vec<ApercuFilm> {
    let mut tries: Vec<&Film> = films.to_vec();
    tries.sort_by_key(|f| (normaliser(&f.titre_triable), f.annee));
    let mut apercus: Vec<ApercuFilm> = tries
        .into_iter()
        .map(|f| {
            let base = appliquer_modele(&profil.modele_fichier, f);
            let fichiers = f
                .fichiers
                .iter()
                .map(|fic| LigneFichier {
                    type_fichier: fic.type_fichier,
                    ancien: fic.nom.clone(),
                    nouveau: match &fic.suffixe {
                        Some(suffixe) => format!("{base}{suffixe}"),
                        None => fic.nom.clone(),
                    },
                })
                .collect();
            ApercuFilm {
                id: f.id,
                titre: f.titre.clone(),
                source: f.source.clone(),
                ancien_dossier: f.dossier.clone(),
                nouveau_dossier: appliquer_modele(&profil.modele_dossier, f),
                fichiers,
                probleme: false,
            }
        })
        .collect();
    let mut compte: HashMap<(String, String), usize> = HashMap::new();
    for a in &apercus {
        *compte
            .entry((a.source.clone(), a.nouveau_dossier.to_lowercase()))
            .or_default() += 1;
    }
    for a in &mut apercus {
        a.probleme = compte[&(a.source.clone(), a.nouveau_dossier.to_lowercase())] > 1;
    }
    apercus
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::donnees::films_factices;

    #[test]
    fn modele_et_assainissement() {
        let films = films_factices();
        let f = films.iter().find(|f| f.id == 2).unwrap();
        assert_eq!(
            appliquer_modele("{titre} ({annee})", f),
            "Blade Runner 2049 (2017)"
        );
        let mad = films.iter().find(|f| f.id == 8).unwrap();
        assert_eq!(
            appliquer_modele("{titre} ({annee})", mad),
            "Mad Max- Fury Road (2015)"
        );
    }

    #[test]
    fn apercu_default_renomme_et_garde_les_images() {
        let films = films_factices();
        let f = films.iter().find(|f| f.id == 2).unwrap();
        let apercus = calculer(&[f], &profils_par_defaut()[0]);
        let a = &apercus[0];
        assert_eq!(a.nouveau_dossier, "Blade Runner 2049 (2017)");
        let video = &a.fichiers[0];
        assert_eq!(video.nouveau, "Blade Runner 2049 (2017).mkv");
        let affiche = a
            .fichiers
            .iter()
            .find(|l| l.type_fichier == TypeFichier::Affiche)
            .unwrap();
        assert!(affiche.inchange());
        assert!(!a.rien_a_renommer());
    }

    #[test]
    fn doublons_signales_avec_le_profil_minimal() {
        let films = films_factices();
        let dunes: Vec<&Film> = films.iter().filter(|f| f.titre == "Dune").collect();
        let minimal = &profils_par_defaut()[1];
        assert!(calculer(&dunes, minimal).iter().all(|a| a.probleme));
        assert!(
            calculer(&dunes, &profils_par_defaut()[0])
                .iter()
                .all(|a| !a.probleme)
        );
    }
}
