//! Réglages propres à l'interface (langue, thème, comportement), mémorisés dans `ui.json`.
//! Les réglages des services (sources, renommeur, réseau…) vivent dans le fichier de réglages
//! du stockage : voir [`ReglagesServices`](crate::source_donnees::ReglagesServices).

use crate::i18n::Langue;
use std::collections::HashSet;

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

    /// Niveau d'indentation de la page dans l'arbre (0 = nœud racine).
    pub fn niveau(self) -> u8 {
        Page::ARBRE
            .iter()
            .find(|(p, _, _)| *p == self)
            .map(|(_, n, _)| *n)
            .unwrap_or(0)
    }

    /// `true` si la page de niveau 0 possède des sous-pages (nœud dépliable).
    pub fn a_des_enfants(self) -> bool {
        let Some(n) = Page::ARBRE.iter().position(|(p, _, _)| *p == self) else {
            return false;
        };
        self.niveau() == 0 && Page::ARBRE.get(n + 1).is_some_and(|(_, niv, _)| *niv > 0)
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

/// Touche de navigation dans l'arbre des pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToucheArbre {
    Haut,
    Bas,
    Gauche,
    Droite,
    Debut,
    Fin,
}

/// Pages visibles dans l'arbre, dans l'ordre d'affichage : celles que `retenue` accepte
/// (filtre) ; si `filtre_actif` est faux, les sous-pages des nœuds de `repliees` sont masquées.
pub fn pages_visibles(
    retenue: impl Fn(Page) -> bool,
    filtre_actif: bool,
    repliees: &HashSet<Page>,
) -> Vec<Page> {
    Page::ARBRE
        .iter()
        .map(|(p, _, _)| *p)
        .filter(|p| retenue(*p))
        .filter(|p| filtre_actif || p.niveau() == 0 || !repliees.contains(&p.racine()))
        .collect()
}

/// Page à sélectionner après `touche`, en modifiant au besoin `repliees` (gauche/droite
/// réduisent ou déplient le nœud). `visibles` est la liste de [`pages_visibles`] ; si la page
/// courante n'y figure pas, Haut/Bas partent de l'extrémité correspondante.
pub fn naviguer(
    touche: ToucheArbre,
    courante: Page,
    visibles: &[Page],
    repliees: &mut HashSet<Page>,
) -> Page {
    let position = visibles.iter().position(|p| *p == courante);
    match touche {
        ToucheArbre::Debut => visibles.first().copied().unwrap_or(courante),
        ToucheArbre::Fin => visibles.last().copied().unwrap_or(courante),
        ToucheArbre::Haut => match position {
            Some(n) => visibles[n.saturating_sub(1)],
            None => visibles.last().copied().unwrap_or(courante),
        },
        ToucheArbre::Bas => match position {
            Some(n) => visibles[(n + 1).min(visibles.len() - 1)],
            None => visibles.first().copied().unwrap_or(courante),
        },
        ToucheArbre::Gauche => {
            if courante.niveau() > 0 {
                courante.racine()
            } else {
                if courante.a_des_enfants() {
                    repliees.insert(courante);
                }
                courante
            }
        }
        ToucheArbre::Droite => {
            // Feuille, ou nœud réduit que l'on déplie : la sélection ne bouge pas.
            if courante.niveau() > 0 || !courante.a_des_enfants() || repliees.remove(&courante) {
                courante
            } else {
                visibles
                    .iter()
                    .copied()
                    .find(|p| p.niveau() > 0 && p.racine() == courante)
                    .unwrap_or(courante)
            }
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

    fn tout(_: Page) -> bool {
        true
    }

    #[test]
    fn navigation_haut_bas_debut_fin() {
        let mut rep = HashSet::new();
        let v = pages_visibles(tout, false, &rep);
        assert_eq!(v.len(), Page::ARBRE.len());
        assert_eq!(
            naviguer(ToucheArbre::Bas, Page::General, &v, &mut rep),
            Page::TypesFichiers
        );
        assert_eq!(
            naviguer(ToucheArbre::Haut, Page::TypesFichiers, &v, &mut rep),
            Page::General
        );
        // Bornes : pas de bouclage.
        assert_eq!(
            naviguer(ToucheArbre::Haut, Page::General, &v, &mut rep),
            Page::General
        );
        assert_eq!(
            naviguer(ToucheArbre::Bas, Page::SeriesPostTraitement, &v, &mut rep),
            Page::SeriesPostTraitement
        );
        assert_eq!(
            naviguer(ToucheArbre::Fin, Page::General, &v, &mut rep),
            Page::SeriesPostTraitement
        );
        assert_eq!(
            naviguer(ToucheArbre::Debut, Page::Films, &v, &mut rep),
            Page::General
        );
        // Passage d'une racine à la suivante : dernière sous-page des films -> Collections.
        assert_eq!(
            naviguer(ToucheArbre::Bas, Page::FilmsPostTraitement, &v, &mut rep),
            Page::Collections
        );
        assert!(rep.is_empty());
    }

    #[test]
    fn navigation_gauche_droite_replie_et_deplie() {
        let mut rep = HashSet::new();
        let v = pages_visibles(tout, false, &rep);
        // Droite sur un nœud déplié : premier enfant.
        assert_eq!(
            naviguer(ToucheArbre::Droite, Page::Films, &v, &mut rep),
            Page::FilmsInterface
        );
        // Gauche sur un enfant : le parent.
        assert_eq!(
            naviguer(ToucheArbre::Gauche, Page::FilmsNfo, &v, &mut rep),
            Page::Films
        );
        // Gauche sur un nœud déplié : on le réduit, la sélection reste.
        assert_eq!(
            naviguer(ToucheArbre::Gauche, Page::Films, &v, &mut rep),
            Page::Films
        );
        assert!(rep.contains(&Page::Films));
        let v = pages_visibles(tout, false, &rep);
        assert!(!v.contains(&Page::FilmsSource) && v.contains(&Page::Collections));
        // Bas depuis un nœud réduit : saute ses enfants.
        assert_eq!(
            naviguer(ToucheArbre::Bas, Page::Films, &v, &mut rep),
            Page::Collections
        );
        // Gauche sur un nœud déjà réduit : rien ne change.
        assert_eq!(
            naviguer(ToucheArbre::Gauche, Page::Films, &v, &mut rep),
            Page::Films
        );
        // Droite sur un nœud réduit : on le déplie.
        assert_eq!(
            naviguer(ToucheArbre::Droite, Page::Films, &v, &mut rep),
            Page::Films
        );
        assert!(rep.is_empty());
        // Nœud sans enfant : rien.
        assert_eq!(
            naviguer(ToucheArbre::Droite, Page::Collections, &v, &mut rep),
            Page::Collections
        );
        assert_eq!(
            naviguer(ToucheArbre::Gauche, Page::Collections, &v, &mut rep),
            Page::Collections
        );
        assert!(rep.is_empty());
        // Droite sur une feuille : rien.
        assert_eq!(
            naviguer(ToucheArbre::Droite, Page::FilmsNfo, &v, &mut rep),
            Page::FilmsNfo
        );
    }

    #[test]
    fn navigation_respecte_le_filtre_et_les_pages_masquees() {
        let mut rep = HashSet::from([Page::General]);
        // Filtre actif : le repli est ignoré, seules les pages retenues comptent.
        let retenue = |p: Page| matches!(p, Page::FilmsSource | Page::SeriesSource | Page::General);
        let v = pages_visibles(retenue, true, &rep);
        assert_eq!(
            v,
            vec![Page::General, Page::FilmsSource, Page::SeriesSource]
        );
        assert_eq!(
            naviguer(ToucheArbre::Bas, Page::General, &v, &mut rep),
            Page::FilmsSource
        );
        // Page courante hors de la liste : Bas -> première, Haut -> dernière.
        assert_eq!(
            naviguer(ToucheArbre::Bas, Page::Divers, &v, &mut rep),
            Page::General
        );
        assert_eq!(
            naviguer(ToucheArbre::Haut, Page::Divers, &v, &mut rep),
            Page::SeriesSource
        );
        // Sans filtre, « Général » réduit masque ses sous-pages.
        let v = pages_visibles(tout, false, &rep);
        assert!(
            !v.contains(&Page::Divers) && v.contains(&Page::General) && v.contains(&Page::Films)
        );
        // Liste vide : on reste sur place.
        assert_eq!(
            naviguer(ToucheArbre::Bas, Page::Divers, &[], &mut rep),
            Page::Divers
        );
    }

    #[test]
    fn niveaux_et_enfants() {
        assert_eq!(Page::General.niveau(), 0);
        assert_eq!(Page::FilmsNfo.niveau(), 1);
        assert!(Page::General.a_des_enfants() && Page::Series.a_des_enfants());
        assert!(!Page::Collections.a_des_enfants() && !Page::Divers.a_des_enfants());
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
