//! Génération d'un nom de dossier ou de fichier à partir d'un gabarit :
//! évaluation + nettoyage (caractères interdits, parenthèses vides,
//! séparateurs, substitution d'espaces, ASCII…).

use std::sync::LazyLock;

use regex::Regex;

use crate::erreur::Resultat;
use crate::evaluation::{Gabarit, Jetons};
use crate::nom_fichier::{
    Plateforme, assainir_chemin_relatif, assainir_nom_fichier, en_ascii, equivalents_unicode,
    remplacer_caracteres_invalides, remplacer_espaces,
};
use crate::options::{OptionsRendu, SubstitutionSeparateurs};
use crate::valeur::Contexte;

/// Nature du nom à produire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormeDestination {
    /// Chemin relatif ; `/` (et `\`) du gabarit séparent des dossiers.
    Dossier,
    /// Nom de fichier sans extension ; `/` et `\` deviennent des espaces.
    Fichier,
}

/// Options de génération de destination.
#[derive(Debug, Clone)]
pub struct OptionsDestination {
    /// Remplacement de `:` (défaut `-`).
    pub remplacement_deux_points: String,
    /// Remplacement des espaces pour les dossiers (`None` = inchangé).
    pub remplacement_espaces_dossier: Option<String>,
    /// Remplacement des espaces pour les fichiers (`None` = inchangé).
    pub remplacement_espaces_fichier: Option<String>,
    /// Translittération ASCII.
    pub ascii: bool,
    /// Caractères interdits remplacés par des équivalents Unicode.
    pub unicode: bool,
    /// Plateforme cible (noms réservés, points finaux…).
    pub plateforme: Plateforme,
    /// Options de rendu du moteur (jeton `first`, tables…). La
    /// substitution des séparateurs est pilotée par `unicode`.
    pub rendu: OptionsRendu,
}

impl Default for OptionsDestination {
    fn default() -> Self {
        OptionsDestination {
            remplacement_deux_points: "-".to_string(),
            remplacement_espaces_dossier: None,
            remplacement_espaces_fichier: None,
            ascii: false,
            unicode: false,
            plateforme: Plateforme::courante(),
            rendu: OptionsRendu::default(),
        }
    }
}

macro_rules! regex_statique {
    ($nom:ident, $motif:expr) => {
        static $nom: LazyLock<Regex> = LazyLock::new(|| Regex::new($motif).expect("motif valide"));
    };
}

regex_statique!(PARENTHESES_VIDES, r"\([ ]?\)");
regex_statique!(CROCHETS_VIDES, r"\[[ ]?\]");
regex_statique!(ACCOLADES_VIDES, r"\{[ ]?\}");
regex_statique!(ESPACES_AVANT_SEP, r"\s+/");
regex_statique!(ESPACES_APRES_SEP, r"/\s+");
regex_statique!(PONCT_AVANT_SEP, r"[ .\-_]+/");
regex_statique!(SEPARATEURS_MULTIPLES, r"/{2,}");
regex_statique!(DEBUT, r"^[ .\-]+");
regex_statique!(FIN, r"[ .\-_]+$");
regex_statique!(RETOURS_LIGNE, r"\r?\n");
regex_statique!(ESPACES_MULTIPLES, r" +");

/// Évalue le gabarit puis applique le nettoyage complet.
///
/// Un gabarit blanc donne une chaîne vide. Les erreurs de syntaxe ou
/// d'évaluation sont renvoyées ; utiliser [`creer_destination_ou_brut`] pour
/// retrouver le comportement du programme d'origine (gabarit brut).
pub fn creer_destination(
    gabarit: &str,
    contexte: &Contexte,
    jetons: &Jetons,
    forme: FormeDestination,
    options: &OptionsDestination,
) -> Resultat<String> {
    if gabarit.trim().is_empty() {
        return Ok(String::new());
    }
    let mut rendu = options.rendu.clone();
    rendu.substitution_separateurs = if options.unicode {
        SubstitutionSeparateurs::Unicode
    } else {
        SubstitutionSeparateurs::Espace
    };
    let brut = Gabarit::analyser(gabarit)?.rendre(contexte, jetons, &rendu)?;
    Ok(nettoyer_destination(&brut, forme, options))
}

/// Comme [`creer_destination`] ; en cas d'erreur, nettoie le gabarit brut.
pub fn creer_destination_ou_brut(
    gabarit: &str,
    contexte: &Contexte,
    jetons: &Jetons,
    forme: FormeDestination,
    options: &OptionsDestination,
) -> String {
    creer_destination(gabarit, contexte, jetons, forme, options)
        .unwrap_or_else(|_| nettoyer_destination(gabarit, forme, options))
}

/// Nettoyage d'un rendu brut (voir la spécification du renommeur) :
///
/// 1. caractères interdits (ou équivalents Unicode) ;
/// 2. `()`, `[]`, `{}` vides retirés ;
/// 3. dossier : séparateurs normalisés en `/` (pas de `//`, pas de `/` initial,
///    pas d'espaces ni de `.-_` autour) ; fichier : `/` et `\` → espace ;
/// 4. substitution d'espaces ; `...` → `…` si Unicode ;
/// 5. début : `[ .-]+` retirés ; fin : `[ .\-_]+` retirés ;
/// 6. ASCII si demandé ; caractères interdits ; retours ligne et espaces
///    multiples ; assainissement par composant selon la plateforme.
///
/// Le séparateur de sortie est toujours `/`.
pub fn nettoyer_destination(s: &str, forme: FormeDestination, o: &OptionsDestination) -> String {
    let mut s = if o.unicode {
        equivalents_unicode(s)
    } else {
        s.to_string()
    };
    s = remplacer_caracteres_invalides(&s, &o.remplacement_deux_points);
    for re in [&PARENTHESES_VIDES, &CROCHETS_VIDES, &ACCOLADES_VIDES] {
        s = re.replace_all(&s, "").into_owned();
    }
    let remplacement_espaces = match forme {
        FormeDestination::Dossier => {
            s = s.replace('\\', "/");
            s = ESPACES_AVANT_SEP.replace_all(&s, "/").into_owned();
            s = ESPACES_APRES_SEP.replace_all(&s, "/").into_owned();
            s = PONCT_AVANT_SEP.replace_all(&s, "/").into_owned();
            s = SEPARATEURS_MULTIPLES.replace_all(&s, "/").into_owned();
            s = s.trim_start_matches('/').to_string();
            &o.remplacement_espaces_dossier
        }
        FormeDestination::Fichier => {
            s = s.replace(['/', '\\'], " ");
            &o.remplacement_espaces_fichier
        }
    };
    if let Some(r) = remplacement_espaces {
        s = remplacer_espaces(&s, r);
    }
    if o.unicode {
        s = s.replace("...", "\u{2026}");
    }
    s = DEBUT.replace(&s, "").into_owned();
    s = FIN.replace(&s, "").into_owned();
    if o.ascii {
        s = en_ascii(&s);
    }
    s = remplacer_caracteres_invalides(&s, &o.remplacement_deux_points);
    s = RETOURS_LIGNE.replace_all(&s, " ").into_owned();
    s = ESPACES_MULTIPLES.replace_all(&s, " ").into_owned();
    let s = s.trim();
    match forme {
        FormeDestination::Dossier => assainir_chemin_relatif(s, o.plateforme),
        FormeDestination::Fichier if s.is_empty() => String::new(),
        FormeDestination::Fichier => assainir_nom_fichier(s, o.plateforme),
    }
}

/// `${...}` contenant l'un des jetons donnés (insensible à la casse, par
/// sous-chaîne comme le programme d'origine).
pub fn contient_jeton(gabarit: &str, jetons: &[&str]) -> bool {
    if jetons.is_empty() {
        return false;
    }
    let alternatives = jetons
        .iter()
        .map(|j| regex::escape(j))
        .collect::<Vec<_>>()
        .join("|");
    Regex::new(&format!(r"(?i)\$\{{[^}}]*?({alternatives})[^}}]*?\}}"))
        .map(|re| re.is_match(gabarit))
        .unwrap_or(false)
}

/// Gabarit de dossier de film « unique » : (titre ET année/imdb/tmdb) OU
/// imdb/tmdb. Sinon plusieurs films peuvent aboutir dans le même dossier.
pub fn gabarit_dossier_unique(gabarit: &str) -> bool {
    let titre = contient_jeton(gabarit, &["title", "originalTitle", "englishTitle"]);
    let identifiant = contient_jeton(gabarit, &["year", "imdb", "tmdb"]);
    (titre && identifiant) || contient_jeton(gabarit, &["imdb", "tmdb"])
}

/// Gabarit de fichier de film valide : contient un titre, `originalFilename`
/// ou un identifiant imdb/tmdb.
pub fn gabarit_fichier_valide(gabarit: &str) -> bool {
    contient_jeton(
        gabarit,
        &[
            "title",
            "originalTitle",
            "englishTitle",
            "originalFilename",
            "imdb",
            "tmdb",
        ],
    )
}
