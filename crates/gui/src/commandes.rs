//! Commandes utilisateur (une par action de menu), table unique partagée par la barre d'outils,
//! le menu contextuel et les raccourcis clavier.

use eframe::egui::{Key, KeyboardShortcut, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Commande {
    MettreAJourToutesSources,
    MettreAJourSelection,
    TrouverManquants,
    CreerHorsLigne,
    AjouterSource,
    ScrapeInteractif,
    ScrapeMeilleur,
    ScrapeMetadonnees,
    ScrapeNonTraites,
    Editer,
    EditionGlobale,
    BasculerVerrou,
    BasculerVu,
    RecupererEvaluations,
    ReecrireNfo,
    InfosMedia,
    Exporter,
    Renommer,
    ApercuRenommage,
    AnnulerRenommage,
    Nettoyer,
    OuvrirDossier,
    Lire,
    RetirerBase,
    SupprimerFichiers,
}

/// Élément d'un menu déclaratif.
#[derive(Clone, Copy, Debug)]
pub enum Entree {
    Cmd(Commande),
    Separateur,
}

use Commande as C;
use Entree::{Cmd, Separateur as Sep};

pub const MENU_MISE_A_JOUR: &[Entree] = &[
    Cmd(C::MettreAJourToutesSources),
    Sep,
    Cmd(C::MettreAJourSelection),
    Sep,
    Cmd(C::TrouverManquants),
    Cmd(C::CreerHorsLigne),
    Sep,
    Cmd(C::AjouterSource),
];

pub const MENU_RECHERCHE: &[Entree] = &[
    Cmd(C::ScrapeInteractif),
    Cmd(C::ScrapeMeilleur),
    Cmd(C::ScrapeMetadonnees),
    Cmd(C::ScrapeNonTraites),
];

pub const MENU_EDITION: &[Entree] = &[
    Cmd(C::Editer),
    Cmd(C::EditionGlobale),
    Cmd(C::BasculerVerrou),
    Cmd(C::BasculerVu),
    Cmd(C::RecupererEvaluations),
    Cmd(C::ReecrireNfo),
    Sep,
    Cmd(C::InfosMedia),
    Sep,
    Cmd(C::Exporter),
];

pub const MENU_RENOMMAGE: &[Entree] = &[
    Cmd(C::Renommer),
    Cmd(C::ApercuRenommage),
    Cmd(C::AnnulerRenommage),
    Cmd(C::Nettoyer),
];

/// Menu contextuel du tableau (regroupe l'essentiel des menus ci-dessus).
pub const MENU_CONTEXTUEL: &[Entree] = &[
    Cmd(C::ScrapeInteractif),
    Cmd(C::ScrapeMeilleur),
    Sep,
    Cmd(C::MettreAJourSelection),
    Cmd(C::Editer),
    Cmd(C::EditionGlobale),
    Cmd(C::BasculerVu),
    Sep,
    Cmd(C::Renommer),
    Cmd(C::ApercuRenommage),
    Sep,
    Cmd(C::Exporter),
    Cmd(C::OuvrirDossier),
    Cmd(C::Lire),
    Sep,
    Cmd(C::RetirerBase),
    Cmd(C::SupprimerFichiers),
];

impl Commande {
    /// Clé de traduction du libellé.
    pub fn cle(self) -> &'static str {
        match self {
            C::MettreAJourToutesSources => "update.datasource.all",
            C::MettreAJourSelection => "update.datasource.selected",
            C::TrouverManquants => "update.datasource.missing",
            C::CreerHorsLigne => "movie.createoffline",
            C::AjouterSource => "update.datasource.add",
            C::ScrapeInteractif => "movie.scrape.selected",
            C::ScrapeMeilleur => "movie.scrape.selected.force",
            C::ScrapeMetadonnees => "movie.scrape.metadata",
            C::ScrapeNonTraites => "movie.scrape.unscraped",
            C::Editer => "movie.edit",
            C::EditionGlobale => "movie.bulkedit",
            C::BasculerVerrou => "movie.togglelock",
            C::BasculerVu => "movie.togglewatchedflag",
            C::RecupererEvaluations => "movie.fetchratings",
            C::ReecrireNfo => "movie.rewritenfo",
            C::InfosMedia => "movie.mediainfo",
            C::Exporter => "movie.export",
            C::Renommer => "movie.rename",
            C::ApercuRenommage => "movie.renamepreview",
            C::AnnulerRenommage => "movie.undorename",
            C::Nettoyer => "movie.cleanup",
            C::OuvrirDossier => "movie.openfolder",
            C::Lire => "movie.play",
            C::RetirerBase => "movie.remove",
            C::SupprimerFichiers => "movie.delete",
        }
    }

    /// Vrai si la commande agit sur la sélection (sinon : « Aucune entrée sélectionnée »).
    pub fn requiert_selection(self) -> bool {
        !matches!(
            self,
            C::MettreAJourToutesSources
                | C::TrouverManquants
                | C::CreerHorsLigne
                | C::AjouterSource
                | C::ScrapeNonTraites
        )
    }

    /// Raccourci clavier (texte affiché, modificateurs, touche).
    pub fn raccourci(self) -> Option<(&'static str, Modifiers, Key)> {
        let cm = Modifiers::COMMAND | Modifiers::SHIFT;
        Some(match self {
            C::MettreAJourToutesSources => ("Ctrl+Maj+U", cm, Key::U),
            C::CreerHorsLigne => ("Ctrl+Maj+O", cm, Key::O),
            C::ScrapeInteractif => ("Ctrl+Maj+S", cm, Key::S),
            C::ScrapeMeilleur => ("Ctrl+Maj+F", cm, Key::F),
            C::ScrapeNonTraites => ("Ctrl+Maj+A", cm, Key::A),
            C::Editer => ("Ctrl+Maj+E", cm, Key::E),
            C::EditionGlobale => ("Ctrl+Maj+B", cm, Key::B),
            C::BasculerVu => ("Ctrl+Maj+W", cm, Key::W),
            C::RecupererEvaluations => ("Ctrl+Maj+I", cm, Key::I),
            C::InfosMedia => ("Ctrl+Maj+M", cm, Key::M),
            C::Exporter => ("Ctrl+Maj+X", cm, Key::X),
            C::Renommer => ("Ctrl+Maj+R", cm, Key::R),
            C::OuvrirDossier => ("Ctrl+E", Modifiers::COMMAND, Key::E),
            C::RetirerBase => ("Suppr", Modifiers::NONE, Key::Delete),
            C::SupprimerFichiers => ("Ctrl+Maj+Suppr", cm, Key::Delete),
            _ => return None,
        })
    }

    pub fn raccourci_egui(self) -> Option<KeyboardShortcut> {
        self.raccourci()
            .map(|(_, m, k)| KeyboardShortcut::new(m, k))
    }

    pub fn raccourci_texte(self) -> Option<&'static str> {
        self.raccourci().map(|(t, _, _)| t)
    }

    pub const TOUTES: [Commande; 25] = [
        C::MettreAJourToutesSources,
        C::MettreAJourSelection,
        C::TrouverManquants,
        C::CreerHorsLigne,
        C::AjouterSource,
        C::ScrapeInteractif,
        C::ScrapeMeilleur,
        C::ScrapeMetadonnees,
        C::ScrapeNonTraites,
        C::Editer,
        C::EditionGlobale,
        C::BasculerVerrou,
        C::BasculerVu,
        C::RecupererEvaluations,
        C::ReecrireNfo,
        C::InfosMedia,
        C::Exporter,
        C::Renommer,
        C::ApercuRenommage,
        C::AnnulerRenommage,
        C::Nettoyer,
        C::OuvrirDossier,
        C::Lire,
        C::RetirerBase,
        C::SupprimerFichiers,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::I18n;

    #[test]
    fn toutes_les_commandes_ont_une_traduction() {
        let i = I18n::default();
        for c in Commande::TOUTES {
            assert!(i.brut(c.cle()).is_some(), "clé manquante : {}", c.cle());
        }
    }

    #[test]
    fn raccourcis_sans_collision() {
        let mut vus = Vec::new();
        for c in Commande::TOUTES {
            if let Some((_, m, k)) = c.raccourci() {
                assert!(!vus.contains(&(m, k)), "raccourci en double : {:?}", c);
                vus.push((m, k));
            }
        }
    }
}
