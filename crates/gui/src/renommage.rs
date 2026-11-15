//! Aperçu du renommage tel qu'affiché par le dialogue : lignes ancien → nouveau par film,
//! conflits. Alimenté par le vrai plan de renommage des services (voir `conversion.rs`) ou,
//! pour la source de démonstration, par un calcul local simplifié ([`calculer`]).

use crate::modele::{Film, normaliser};
use std::collections::HashMap;

/// Une ligne de fichier de l'aperçu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LigneFichier {
    /// Libellé du type (`VIDEO`, `NFO`, `POSTER`…).
    pub type_fichier: String,
    pub ancien: String,
    /// Nouveau(x) nom(s) ; vide si le fichier est supprimé.
    pub nouveau: String,
    pub inchange: bool,
}

impl LigneFichier {
    pub fn inchange(&self) -> bool {
        self.inchange
    }
}

/// Aperçu d'un film.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApercuFilm {
    pub id: u64,
    pub titre: String,
    pub source: String,
    pub ancien_dossier: String,
    pub nouveau_dossier: String,
    pub fichiers: Vec<LigneFichier>,
    /// Conflits bloquants (le film ne sera pas renommé).
    pub conflits: Vec<String>,
    /// Remarques non bloquantes.
    pub avertissements: Vec<String>,
    /// Rien ne change pour ce film.
    pub inchange: bool,
}

impl ApercuFilm {
    /// Vrai si un conflit bloque le renommage.
    pub fn probleme(&self) -> bool {
        !self.conflits.is_empty()
    }

    pub fn rien_a_renommer(&self) -> bool {
        self.inchange
    }

    /// Le film sera effectivement renommé.
    pub fn executable(&self) -> bool {
        !self.probleme() && !self.rien_a_renommer()
    }
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

/// Gabarit simplifié (démonstration) : `${title}`, `${year}`, `${videoFormat}` ; les autres
/// jetons `${…}` sont retirés, les espaces multiples réduits.
pub fn appliquer_modele(modele: &str, film: &Film) -> String {
    let mut brut = modele
        .replace("${title}", &film.titre)
        .replace("${year}", &film.annee.to_string())
        .replace("${videoFormat}", &film.format_video());
    while let Some(debut) = brut.find("${") {
        match brut[debut..].find('}') {
            Some(fin) => brut.replace_range(debut..debut + fin + 1, ""),
            None => break,
        }
    }
    let compact = brut.split_whitespace().collect::<Vec<_>>().join(" ");
    assainir(&compact)
}

/// Calcul local de démonstration : films triés par titre, doublons de dossier signalés.
pub fn calculer(films: &[&Film], modele_dossier: &str, modele_fichier: &str) -> Vec<ApercuFilm> {
    let mut tries: Vec<&Film> = films.to_vec();
    tries.sort_by_key(|f| (normaliser(&f.titre_triable), f.annee));
    let mut apercus: Vec<ApercuFilm> = tries
        .into_iter()
        .map(|f| {
            let base = appliquer_modele(modele_fichier, f);
            let fichiers: Vec<LigneFichier> = f
                .fichiers
                .iter()
                .map(|fic| {
                    let nouveau = match &fic.suffixe {
                        Some(suffixe) => format!("{base}{suffixe}"),
                        None => fic.nom.clone(),
                    };
                    LigneFichier {
                        type_fichier: fic.type_fichier.libelle().to_string(),
                        inchange: nouveau == fic.nom,
                        ancien: fic.nom.clone(),
                        nouveau,
                    }
                })
                .collect();
            let nouveau_dossier = appliquer_modele(modele_dossier, f);
            ApercuFilm {
                id: f.id,
                titre: f.titre.clone(),
                source: f.source.clone(),
                inchange: f.dossier == nouveau_dossier && fichiers.iter().all(|l| l.inchange),
                ancien_dossier: f.dossier.clone(),
                nouveau_dossier,
                fichiers,
                conflits: Vec::new(),
                avertissements: Vec::new(),
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
        if compte[&(a.source.clone(), a.nouveau_dossier.to_lowercase())] > 1 {
            a.conflits.push(format!(
                "plusieurs films visent le dossier {}",
                a.nouveau_dossier
            ));
        }
    }
    apercus
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::donnees::films_factices;

    const DEFAUT: &str = "${title} (${year})";

    #[test]
    fn modele_et_assainissement() {
        let films = films_factices();
        let f = films.iter().find(|f| f.id == 2).unwrap();
        assert_eq!(appliquer_modele(DEFAUT, f), "Blade Runner 2049 (2017)");
        assert_eq!(
            appliquer_modele("${title}${ - ,edition,} ${audioCodec}", f),
            "Blade Runner 2049"
        );
        let mad = films.iter().find(|f| f.id == 8).unwrap();
        assert_eq!(appliquer_modele(DEFAUT, mad), "Mad Max- Fury Road (2015)");
    }

    #[test]
    fn apercu_renomme_et_garde_les_images() {
        let films = films_factices();
        let f = films.iter().find(|f| f.id == 2).unwrap();
        let apercus = calculer(&[f], DEFAUT, DEFAUT);
        let a = &apercus[0];
        assert_eq!(a.nouveau_dossier, "Blade Runner 2049 (2017)");
        assert_eq!(a.fichiers[0].nouveau, "Blade Runner 2049 (2017).mkv");
        let affiche = a
            .fichiers
            .iter()
            .find(|l| l.type_fichier == "POSTER")
            .unwrap();
        assert!(affiche.inchange());
        assert!(a.executable());
    }

    #[test]
    fn doublons_signales() {
        let films = films_factices();
        let dunes: Vec<&Film> = films.iter().filter(|f| f.titre == "Dune").collect();
        assert!(
            calculer(&dunes, "${title}", "${title}")
                .iter()
                .all(|a| a.probleme())
        );
        assert!(
            calculer(&dunes, DEFAUT, DEFAUT)
                .iter()
                .all(|a| !a.probleme())
        );
    }
}
