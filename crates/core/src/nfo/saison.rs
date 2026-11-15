//! NFO de saisons (racine `<season>`).
//!
//! Le même format est écrit quel que soit le connecteur : c'est un format
//! propre aux lecteurs qui exploitent les NFO de saison ; le connecteur n'est
//! repris que dans le commentaire d'en-tête.

use std::path::Path;

use crate::modele::{Saison, Serie};

use super::arbre::{Element, decoder_octets, racines_nommees};
use super::commun::*;
use super::outils::{annee_de_date, entier_en_tete, normaliser_date, resume_court};

/// Résultat de la lecture d'un NFO de saison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaisonLue {
    pub saison: Saison,
    /// NFO valide = numéro de saison > -1.
    pub valide: bool,
    pub conserves: DonneesConservees,
}

/// Balises gérées ; `epbookmark` et `resume` sont volontairement ignorées
/// (ni lues ni conservées).
const BALISES_SAISON: &[&str] = &[
    "title",
    "showtitle",
    "sorttitle",
    "plot",
    "outline",
    "year",
    "premiered",
    "aired",
    "releasedate",
    "seasonnumber",
    "lockdata",
    "epbookmark",
    "resume",
];

/// Lit un NFO de saison depuis ses octets.
pub fn lire_nfo_saison_octets(octets: &[u8]) -> SaisonLue {
    lire_nfo_saison(&decoder_octets(octets))
}

/// Lit un fichier NFO de saison.
pub fn lire_fichier_nfo_saison(chemin: &Path) -> std::io::Result<SaisonLue> {
    Ok(lire_nfo_saison_octets(&std::fs::read(chemin)?))
}

/// Lit un NFO de saison (XML éventuellement bruité).
pub fn lire_nfo_saison(texte: &str) -> SaisonLue {
    let racines = racines_nommees(texte, &["season"]);
    let Some(r) = racines.first() else {
        return SaisonLue {
            saison: Saison {
                numero: -1,
                ..Saison::default()
            },
            valide: false,
            conserves: DonneesConservees::default(),
        };
    };
    let numero = r
        .texte_unique("seasonnumber")
        .and_then(|t| entier_en_tete(&t))
        .map_or(-1, |n| n as i32);
    let saison = Saison {
        numero,
        titre: r.texte_unique("title").unwrap_or_default(),
        resume: r
            .texte_complet_unique("plot")
            .or_else(|| r.texte_complet_unique("outline"))
            .unwrap_or_default(),
        premiere_diffusion: ["premiered", "aired", "releasedate"]
            .iter()
            .find_map(|b| r.texte_unique(b).and_then(|t| normaliser_date(&t))),
        fichiers: Vec::new(),
    };
    SaisonLue {
        valide: numero > -1,
        saison,
        conserves: DonneesConservees {
            inconnues: elements_inconnus(r, BALISES_SAISON),
            ..DonneesConservees::default()
        },
    }
}

/// Construit l'élément racine `<season>`.
pub fn element_nfo_saison(
    serie: &Serie,
    saison: &Saison,
    conserves: &DonneesConservees,
    options: &OptionsNfo,
) -> Element {
    let mut r = Element::nouveau("season");
    r.ajouter_texte("title", &saison.titre);
    r.ajouter_texte("showtitle", &serie.titre);
    r.ajouter_texte("plot", &saison.resume);
    if options.creer_resume_court {
        r.ajouter_texte(
            "outline",
            &resume_court(&saison.resume, options.resume_court_premiere_phrase),
        );
    }
    let date = saison.premiere_diffusion.as_deref().unwrap_or("");
    let annee = annee_de_date(date);
    r.ajouter_texte(
        "year",
        &if annee > 0 {
            annee.to_string()
        } else {
            String::new()
        },
    );
    r.ajouter_texte("premiered", date);
    r.ajouter_texte("releasedate", date);
    r.ajouter_texte("seasonnumber", &saison.numero.to_string());
    if options.ecrire_lockdata {
        r.ajouter_texte("lockdata", "true");
    }
    for e in &conserves.inconnues {
        r.ajouter(e.clone());
    }
    r
}

/// Produit le contenu complet d'un NFO de saison (UTF-8, CRLF).
pub fn ecrire_nfo_saison(
    serie: &Serie,
    saison: &Saison,
    conserves: &DonneesConservees,
    connecteur: Connecteur,
    options: &OptionsNfo,
) -> String {
    let racine = element_nfo_saison(serie, saison, conserves, options);
    assembler(&[racine], &commentaire_entete(connecteur, options))
}
