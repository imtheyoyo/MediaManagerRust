//! Réglages propres à l'interface (langue, thème, comportement), mémorisés dans `ui.json`.
//! Les réglages des services (sources, renommeur, réseau…) vivent dans le fichier de réglages
//! du stockage : voir [`ReglagesServices`](crate::source_donnees::ReglagesServices).

use crate::i18n::Langue;

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
    /// Recherche d'une nouvelle version du programme au démarrage.
    pub verifier_mises_a_jour: bool,
}

impl Default for Reglages {
    fn default() -> Self {
        Self {
            langue: Langue::Francais,
            theme: Theme::Sombre,
            memoriser_module: true,
            afficher_memoire: false,
            verifier_mises_a_jour: true,
        }
    }
}

/// Pages de l'arbre des réglages (§3.1 de la spécification).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Page {
    #[default]
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
    SeriesInterface,
    SeriesSource,
    SeriesNfo,
    SeriesExtracteur,
    SeriesIllustrations,
    SeriesBandesAnnonces,
    SeriesSousTitres,
    SeriesRenommeur,
    SeriesPostTraitement,
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
        (Page::SeriesInterface, 1, "Settings.movie.ui"),
        (Page::SeriesSource, 1, "Settings.source"),
        (Page::SeriesNfo, 1, "Settings.nfo"),
        (Page::SeriesExtracteur, 1, "Settings.scraper"),
        (Page::SeriesIllustrations, 1, "Settings.artwork"),
        (Page::SeriesBandesAnnonces, 1, "Settings.trailer"),
        (Page::SeriesSousTitres, 1, "Settings.subtitle"),
        (Page::SeriesRenommeur, 1, "Settings.renamer"),
        (Page::SeriesPostTraitement, 1, "Settings.postprocessing"),
    ];

    pub fn cle(self) -> &'static str {
        Page::ARBRE
            .iter()
            .find(|(p, _, _)| *p == self)
            .map(|(_, _, c)| *c)
            .unwrap_or("Settings.general")
    }
}

impl Page {
    /// Page de niveau 0 qui contient cette page (elle-même pour une page de niveau 0).
    pub fn racine(self) -> Page {
        let mut courante = Page::General;
        for (p, niveau, _) in Page::ARBRE {
            if *niveau == 0 {
                courante = *p;
            }
            if *p == self {
                return courante;
            }
        }
        self
    }

    /// Titre affiché au-dessus de la page : « Films › Source de données » pour une sous-page
    /// des films ou des séries (leurs sous-pages portent le même libellé), le libellé seul
    /// sinon.
    pub fn titre(self, i18n: &crate::i18n::I18n) -> String {
        let racine = self.racine();
        if matches!(racine, Page::Films | Page::Series) && racine != self {
            format!("{} › {}", i18n.t(racine.cle()), i18n.t(self.cle()))
        } else {
            i18n.t(self.cle())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::I18n;

    #[test]
    fn titres_des_sous_pages() {
        let i = I18n::default();
        assert_eq!(Page::FilmsSource.racine(), Page::Films);
        assert_eq!(Page::SeriesSource.racine(), Page::Series);
        assert_eq!(Page::Systeme.racine(), Page::General);
        assert_eq!(Page::Collections.racine(), Page::Collections);
        assert_eq!(Page::Series.titre(&i), i.t("Settings.tvshows"));
        assert_eq!(
            Page::SeriesSource.titre(&i),
            format!("{} › {}", i.t("Settings.tvshows"), i.t("Settings.source"))
        );
        assert_ne!(Page::FilmsSource.titre(&i), Page::SeriesSource.titre(&i));
        assert_eq!(Page::Divers.titre(&i), i.t("Settings.misc"));
    }

    #[test]
    fn arbre_complet_et_traduit() {
        let i = I18n::default();
        assert_eq!(Page::ARBRE.len(), 30);
        let mut vues = std::collections::HashSet::new();
        for (p, _, _) in Page::ARBRE {
            assert!(vues.insert(*p), "page en double : {p:?}");
        }
        for (p, _, cle) in Page::ARBRE {
            assert!(i.brut(cle).is_some());
            assert_eq!(p.cle(), *cle);
        }
    }
}
