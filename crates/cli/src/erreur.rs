//! Erreurs de l'outil et codes de sortie.

use std::fmt;
use std::process::ExitCode;

use mediamanager_app::ErreurApp;
use mediamanager_store::ErreurStockage;

/// Codes de sortie du programme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeSortie {
    Succes,
    Erreur,
    /// Le renommage comporte des conflits (aperçu ou exécution).
    ConflitsRenommage,
    /// Une autre instance utilise déjà le dossier de données.
    InstanceDejaLancee,
    /// Opération interrompue par l'utilisateur (Ctrl+C).
    Annulee,
}

impl CodeSortie {
    pub fn valeur(self) -> u8 {
        match self {
            CodeSortie::Succes => 0,
            CodeSortie::Erreur => 1,
            CodeSortie::ConflitsRenommage => 2,
            CodeSortie::InstanceDejaLancee => 3,
            CodeSortie::Annulee => 130,
        }
    }
}

impl From<CodeSortie> for ExitCode {
    fn from(c: CodeSortie) -> Self {
        ExitCode::from(c.valeur())
    }
}

/// Erreur présentée à l'utilisateur, avec son code de sortie.
#[derive(Debug)]
pub struct ErreurCli {
    pub code: CodeSortie,
    pub message: String,
}

impl ErreurCli {
    pub fn nouvelle(message: impl Into<String>) -> Self {
        Self {
            code: CodeSortie::Erreur,
            message: message.into(),
        }
    }
}

impl fmt::Display for ErreurCli {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<ErreurApp> for ErreurCli {
    fn from(e: ErreurApp) -> Self {
        match e {
            ErreurApp::Stockage(ErreurStockage::InstanceDejaLancee(chemin)) => ErreurCli {
                code: CodeSortie::InstanceDejaLancee,
                message: format!(
                    "une autre instance de l'application utilise déjà ces données \
                     (verrou {}) : fermez-la puis réessayez",
                    chemin.display()
                ),
            },
            ErreurApp::Annulee => ErreurCli {
                code: CodeSortie::Annulee,
                message: "opération annulée".into(),
            },
            ErreurApp::CleTmdbAbsente => ErreurCli::nouvelle(
                "aucune clé d'API TMDB n'est configurée : enregistrez votre clé personnelle \
                 avec « mediamanager cle-tmdb »",
            ),
            autre => ErreurCli::nouvelle(autre.to_string()),
        }
    }
}

/// Résultat d'une commande : code de sortie en cas de succès.
pub type ResultatCli = Result<CodeSortie, ErreurCli>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_distincts() {
        let codes = [
            CodeSortie::Succes,
            CodeSortie::Erreur,
            CodeSortie::ConflitsRenommage,
            CodeSortie::InstanceDejaLancee,
            CodeSortie::Annulee,
        ]
        .map(CodeSortie::valeur);
        assert_eq!(codes, [0, 1, 2, 3, 130]);
    }

    #[test]
    fn conversion_des_erreurs() {
        let e: ErreurCli =
            ErreurApp::Stockage(ErreurStockage::InstanceDejaLancee("/x/.lock".into())).into();
        assert_eq!(e.code, CodeSortie::InstanceDejaLancee);
        let e: ErreurCli = ErreurApp::Annulee.into();
        assert_eq!(e.code, CodeSortie::Annulee);
        let e: ErreurCli = ErreurApp::CleTmdbAbsente.into();
        assert_eq!(e.code, CodeSortie::Erreur);
        assert!(e.message.contains("cle-tmdb"));
    }
}
