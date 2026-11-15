//! Analyse des flux audio/vidéo/sous-titres des fichiers médias (films et
//! épisodes) par un outil externe (`ffprobe`, `mediainfo`), et diagnostic de
//! la présence de ces outils.
//!
//! Le chemin réglé par l'utilisateur (réglages généraux, « chemin de
//! ffprobe / MediaInfo ») est prioritaire ; viennent ensuite le dossier
//! `outils/` à côté de l'exécutable puis le `PATH`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mediamanager_core::flux::{
    ConfigOutils, ErreurFlux, Outils, analyser_fichier, appliquer_au_fichier, trouver_outils,
    version_outil,
};
use mediamanager_core::modele::{Film, Serie, TypeFichier};

use crate::application::Noyau;
use crate::erreur::{ErreurApp, Resultat};
use crate::evenements::{Evenement, NiveauMessage};
use crate::taches::ContexteTache;

/// Bilan d'une analyse de fichiers médias.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapportAnalyse {
    /// Fichiers réellement analysés.
    pub analyses: usize,
    /// Films dont au moins un fichier a été mis à jour.
    pub films: Vec<String>,
    /// Séries dont au moins un fichier a été mis à jour.
    pub series: Vec<String>,
    /// Films et séries verrouillés, laissés intacts.
    pub verrouilles: Vec<String>,
    /// (chemin ou identifiant, raison) : fichiers non analysés.
    pub echecs: Vec<(String, String)>,
    /// Aucun fichier vidéo à analyser dans la sélection.
    pub aucun_fichier: bool,
}

/// Un outil d'analyse localisé : chemin et version annoncée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutilTrouve {
    pub chemin: PathBuf,
    /// Première ligne de la sortie de version ; `None` si l'outil ne répond pas.
    pub version: Option<String>,
}

/// Résultat du diagnostic des outils d'analyse.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticOutils {
    pub ffprobe: Option<OutilTrouve>,
    pub mediainfo: Option<OutilTrouve>,
}

impl DiagnosticOutils {
    /// Vrai si au moins un outil a été trouvé.
    pub fn disponible(&self) -> bool {
        self.ffprobe.is_some() || self.mediainfo.is_some()
    }
}

/// Configuration de recherche : base du service, puis réglage utilisateur
/// (`chemin` vide = recherche automatique).
fn config_outils(base: &ConfigOutils, chemin: &str) -> ConfigOutils {
    let mut c = base.clone();
    let chemin = chemin.trim();
    if !chemin.is_empty() {
        c.chemin_ffprobe = Some(PathBuf::from(chemin));
        c.chemin_mediainfo = Some(PathBuf::from(chemin));
    }
    c
}

/// Outils localisés d'après les réglages courants (aucun lancement).
pub(crate) fn outils_reglages(n: &Noyau) -> Outils {
    let chemin = n.verrou().reglages.general.chemin_ffmpeg.clone();
    trouver_outils(&config_outils(&n.config.outils, &chemin))
}

/// Outils localisés pour un chemin donné, avec leur version (les outils sont
/// lancés : appel bloquant de quelques secondes au plus).
pub(crate) fn diagnostiquer(n: &Noyau, chemin: &str) -> DiagnosticOutils {
    let outils = trouver_outils(&config_outils(&n.config.outils, chemin));
    let decrire = |p: Option<PathBuf>| {
        p.map(|chemin| OutilTrouve {
            version: version_outil(&chemin),
            chemin,
        })
    };
    DiagnosticOutils {
        ffprobe: decrire(outils.ffprobe),
        mediainfo: decrire(outils.mediainfo),
    }
}

fn libelle_erreur(e: &ErreurFlux) -> String {
    e.to_string()
}

/// Analyse un fichier une seule fois par chemin (cache de la tâche).
struct Analyseur<'a> {
    outils: &'a Outils,
    cache: BTreeMap<PathBuf, Result<mediamanager_core::flux::InfosMedia, String>>,
    rapport: &'a mut RapportAnalyse,
}

impl Analyseur<'_> {
    fn infos(&mut self, chemin: &Path) -> Result<mediamanager_core::flux::InfosMedia, String> {
        if let Some(r) = self.cache.get(chemin) {
            return r.clone();
        }
        let r = analyser_fichier(chemin, self.outils).map_err(|e| libelle_erreur(&e));
        match &r {
            Ok(_) => self.rapport.analyses += 1,
            Err(e) => self
                .rapport
                .echecs
                .push((chemin.display().to_string(), e.clone())),
        }
        self.cache.insert(chemin.to_path_buf(), r.clone());
        r
    }
}

/// Tâche : analyse les fichiers vidéo des films et des séries (tous leurs
/// épisodes) indiqués, reporte les flux dans le modèle et enregistre en
/// base. Sans outil disponible, échoue d'emblée avec une explication.
pub(crate) fn analyser_medias(
    n: &Noyau,
    ctx: &ContexteTache,
    films: &[String],
    series: &[String],
) -> Resultat<RapportAnalyse> {
    let outils = outils_reglages(n);
    if !outils.disponible() {
        return Err(ErreurApp::OutilsAnalyseAbsents);
    }
    let mut rapport = RapportAnalyse::default();
    let total = (films.len() + series.len()) as u64;
    let mut fait = 0u64;
    let mut a_analyser = 0usize;

    for id in films {
        ctx.verifier_annulation()?;
        let Some(mut film) = n.verrou().bibliotheque.obtenir::<Film>(id)? else {
            rapport
                .echecs
                .push((id.clone(), format!("film {id} introuvable")));
            continue;
        };
        ctx.progression(fait, total, film.titre.clone());
        fait += 1;
        if film.verrouille {
            rapport.verrouilles.push(id.clone());
            continue;
        }
        let mut modifie = false;
        let mut analyseur = Analyseur {
            outils: &outils,
            cache: BTreeMap::new(),
            rapport: &mut rapport,
        };
        for f in film
            .fichiers
            .iter_mut()
            .filter(|f| f.type_fichier == TypeFichier::Video)
        {
            a_analyser += 1;
            if let Ok(infos) = analyseur.infos(&f.chemin_complet()) {
                appliquer_au_fichier(f, &infos);
                modifie = true;
            }
        }
        if modifie {
            n.verrou().bibliotheque.enregistrer(&mut film)?;
            ctx.emettre(Evenement::FilmModifie { id: id.clone() });
            rapport.films.push(id.clone());
        }
    }

    for id in series {
        ctx.verifier_annulation()?;
        let Some(mut serie) = n.verrou().bibliotheque.obtenir::<Serie>(id)? else {
            rapport
                .echecs
                .push((id.clone(), format!("série {id} introuvable")));
            continue;
        };
        ctx.progression(fait, total, serie.titre.clone());
        fait += 1;
        if serie.verrouille {
            rapport.verrouilles.push(id.clone());
            continue;
        }
        let mut modifie = false;
        let mut analyseur = Analyseur {
            outils: &outils,
            cache: BTreeMap::new(),
            rapport: &mut rapport,
        };
        for f in serie
            .episodes
            .iter_mut()
            .flat_map(|e| e.fichiers.iter_mut())
            .filter(|f| f.type_fichier == TypeFichier::Video)
        {
            a_analyser += 1;
            if let Ok(infos) = analyseur.infos(&f.chemin_complet()) {
                appliquer_au_fichier(f, &infos);
                modifie = true;
            }
        }
        if modifie {
            n.verrou().bibliotheque.enregistrer(&mut serie)?;
            ctx.emettre(Evenement::SerieModifiee { id: id.clone() });
            rapport.series.push(id.clone());
        }
    }

    rapport.aucun_fichier = a_analyser == 0 && rapport.verrouilles.is_empty();
    if rapport.aucun_fichier {
        ctx.message(
            NiveauMessage::Avertissement,
            "aucun fichier vidéo à analyser dans la sélection",
        );
    }
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}
