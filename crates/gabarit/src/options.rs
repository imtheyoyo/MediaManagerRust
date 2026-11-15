//! Options d'évaluation d'un gabarit.

use std::fmt;
use std::sync::Arc;

/// Traitement de `/` et `\` dans les VALEURS injectées (jamais dans le texte
/// littéral du gabarit, où ils restent des séparateurs de dossiers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubstitutionSeparateurs {
    /// Valeurs laissées intactes.
    #[default]
    Aucune,
    /// `/` et `\` deviennent une espace.
    Espace,
    /// `/` → `⁄` (U+2044) et `\` → `∖` (U+2216).
    Unicode,
}

/// Résolveur de tables de remplacement : reçoit le nom du fichier
/// (`replace(table.csv)`) et renvoie son contenu CSV. Le moteur ne touche
/// jamais au système de fichiers ; la mise en cache est à la charge de
/// l'appelant.
pub type ResolveurTable = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Options de rendu.
#[derive(Clone)]
pub struct OptionsRendu {
    /// Résultat du rendu `first` pour un chiffre ou un symbole (défaut `#`).
    pub remplacement_premier_caractere: String,
    /// Substitution des séparateurs de chemin dans les valeurs.
    pub substitution_separateurs: SubstitutionSeparateurs,
    /// Jetons (premier segment du chemin, insensible à la casse) dont la
    /// valeur n'est jamais filtrée (défaut : `parent`).
    pub jetons_sans_filtre: Vec<String>,
    /// Source des tables CSV pour `replace(fichier.csv)`.
    pub resolveur_table: Option<ResolveurTable>,
}

impl Default for OptionsRendu {
    fn default() -> Self {
        OptionsRendu {
            remplacement_premier_caractere: "#".to_string(),
            substitution_separateurs: SubstitutionSeparateurs::Aucune,
            jetons_sans_filtre: vec!["parent".to_string()],
            resolveur_table: None,
        }
    }
}

impl fmt::Debug for OptionsRendu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OptionsRendu")
            .field(
                "remplacement_premier_caractere",
                &self.remplacement_premier_caractere,
            )
            .field("substitution_separateurs", &self.substitution_separateurs)
            .field("jetons_sans_filtre", &self.jetons_sans_filtre)
            .field("resolveur_table", &self.resolveur_table.is_some())
            .finish()
    }
}
