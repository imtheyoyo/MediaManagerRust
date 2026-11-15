//! Réglages modifiables depuis le dialogue (valeurs factices, non persistées pour l'instant).

use crate::i18n::Langue;
use crate::renommage::{Profil, profils_par_defaut};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Theme {
    Clair,
    #[default]
    Sombre,
}

impl Theme {
    /// Identifiant de persistance (`ui.json`).
    pub fn id(self) -> &'static str {
        match self {
            Theme::Clair => "clair",
            Theme::Sombre => "sombre",
        }
    }

    pub fn depuis_id(id: &str) -> Option<Theme> {
        match id {
            "clair" => Some(Theme::Clair),
            "sombre" => Some(Theme::Sombre),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reglages {
    pub langue: Langue,
    pub theme: Theme,
    pub memoriser_module: bool,
    pub afficher_memoire: bool,
    pub proxy_hote: String,
    pub proxy_port: String,
    pub proxy_utilisateur: String,
    pub proxy_mot_de_passe: String,
    pub ignorer_erreurs_ssl: bool,
    pub telechargements_paralleles: u32,
    pub sources: Vec<String>,
    pub exclusions: Vec<String>,
    pub profils: Vec<Profil>,
    pub profil_actif: usize,
    pub renommer_dossiers: bool,
    pub renommer_fichiers: bool,
    pub corbeille: bool,
    pub cache_image: bool,
}

impl Default for Reglages {
    fn default() -> Self {
        Self {
            langue: Langue::Francais,
            theme: Theme::Sombre,
            memoriser_module: true,
            afficher_memoire: false,
            proxy_hote: String::new(),
            proxy_port: String::new(),
            proxy_utilisateur: String::new(),
            proxy_mot_de_passe: String::new(),
            ignorer_erreurs_ssl: false,
            telechargements_paralleles: 4,
            sources: vec!["D:\\Films".into(), "E:\\Films 4K".into()],
            exclusions: vec!["@eaDir".into(), "$RECYCLE.BIN".into()],
            profils: profils_par_defaut(),
            profil_actif: 0,
            renommer_dossiers: true,
            renommer_fichiers: true,
            corbeille: true,
            cache_image: true,
        }
    }
}

/// Pages de l'arbre des réglages (§3.1 de la spécification).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    General,
    TypesFichiers,
    TriTitre,
    OutilsExternes,
    MaterielsExternes,
    ServicesExternes,
    Systeme,
    Ard,
    Divers,
    Films,
    FilmsInterface,
    FilmsSource,
    FilmsNfo,
    FilmsExtracteur,
    FilmsIllustrations,
    FilmsBandesAnnonces,
    FilmsSousTitres,
    FilmsRenommeur,
    FilmsPostTraitement,
    Collections,
    Series,
}

impl Page {
    /// (page, niveau d'indentation, clé de traduction), dans l'ordre d'affichage.
    pub const ARBRE: &'static [(Page, u8, &'static str)] = &[
        (Page::General, 0, "Settings.general"),
        (Page::TypesFichiers, 1, "Settings.filetypes"),
        (Page::TriTitre, 1, "Settings.sorttitle"),
        (Page::OutilsExternes, 1, "Settings.externaltools"),
        (Page::MaterielsExternes, 1, "Settings.externaldevices"),
        (Page::ServicesExternes, 1, "Settings.externalservices"),
        (Page::Systeme, 1, "Settings.system"),
        (Page::Ard, 1, "Settings.ard"),
        (Page::Divers, 1, "Settings.misc"),
        (Page::Films, 0, "Settings.movies"),
        (Page::FilmsInterface, 1, "Settings.movie.ui"),
        (Page::FilmsSource, 1, "Settings.source"),
        (Page::FilmsNfo, 1, "Settings.nfo"),
        (Page::FilmsExtracteur, 1, "Settings.scraper"),
        (Page::FilmsIllustrations, 1, "Settings.artwork"),
        (Page::FilmsBandesAnnonces, 1, "Settings.trailer"),
        (Page::FilmsSousTitres, 1, "Settings.subtitle"),
        (Page::FilmsRenommeur, 1, "Settings.renamer"),
        (Page::FilmsPostTraitement, 1, "Settings.postprocessing"),
        (Page::Collections, 0, "Settings.moviesets"),
        (Page::Series, 0, "Settings.tvshows"),
    ];

    pub fn cle(self) -> &'static str {
        Page::ARBRE
            .iter()
            .find(|(p, _, _)| *p == self)
            .map(|(_, _, c)| *c)
            .unwrap_or("Settings.general")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::I18n;

    #[test]
    fn arbre_complet_et_traduit() {
        let i = I18n::default();
        assert_eq!(Page::ARBRE.len(), 21);
        for (p, _, cle) in Page::ARBRE {
            assert!(i.brut(cle).is_some());
            assert_eq!(p.cle(), *cle);
        }
    }
}
