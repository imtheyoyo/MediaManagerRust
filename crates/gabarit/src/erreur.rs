//! Erreurs du moteur de gabarits.

use thiserror::Error;

/// Erreur de lecture (syntaxe) ou d'évaluation d'un gabarit.
///
/// Les positions sont des décalages en octets dans le gabarit source.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ErreurGabarit {
    /// Une balise `${` n'est pas refermée par `}`.
    #[error("balise `${{` non fermée à la position {position}")]
    BaliseNonFermee { position: usize },

    /// Une balise est mal formée.
    #[error("syntaxe invalide à la position {position} : {message}")]
    Syntaxe { position: usize, message: String },

    /// Un bloc `if` ou `foreach` n'est pas terminé par `${end}`.
    #[error("bloc `{bloc}` non terminé : `${{end}}` manquant")]
    BlocNonTermine { bloc: String },

    /// `${else}` ou `${end}` sans bloc ouvert (ou `else` en double).
    #[error("`{mot}` inattendu à la position {position}")]
    MotInattendu { mot: String, position: usize },

    /// Indice d'accès `[...]` invalide.
    #[error("indice invalide `{indice}`")]
    IndiceInvalide { indice: String },

    /// Rendu nommé (`;nom(...)`) inconnu.
    #[error("rendu nommé inconnu : `{nom}`")]
    RenduInconnu { nom: String },

    /// Argument d'un rendu nommé invalide.
    #[error("argument invalide pour le rendu `{rendu}` : {message}")]
    ArgumentRendu { rendu: String, message: String },

    /// Expression régulière invalide (annotation `@regexp` ou rendu).
    #[error("expression régulière invalide `{motif}` : {message}")]
    Regex { motif: String, message: String },

    /// Format de nombre (`%d`, `%.2f`…) invalide ou incompatible avec la valeur.
    #[error("format de nombre invalide `{format}` : {message}")]
    FormatNombre { format: String, message: String },

    /// Format de date invalide.
    #[error("format de date invalide `{format}` : {message}")]
    FormatDate { format: String, message: String },

    /// Jeton (raccourci) mal défini.
    #[error("jeton `{jeton}` invalide : {message}")]
    Jeton { jeton: String, message: String },

    /// Imbrication de rendus trop profonde (protection).
    #[error("imbrication trop profonde")]
    ProfondeurExcessive,
}

/// Alias de résultat du crate.
pub type Resultat<T> = Result<T, ErreurGabarit>;
