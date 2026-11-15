//! Planification du renommage des séries : dossier de la série, dossiers de
//! saison et fichiers des épisodes (multi-épisodes compris).

use std::path::{Path, PathBuf};

use mediamanager_gabarit::{
    EpisodeContexte, FormeDestination, ListesJetonsEpisode, contient_jeton, creer_destination,
    creer_destination_multi,
};

use super::contexte::{contexte_episode, contexte_serie, jetons_serie, principal_episode};
use super::noms::{
    appliquer_motif, avec_extension, base_sans_pile, chaine_pile, dans_dossier_nomme, difference,
    extension_artwork, extension_originale, nom_bif, suffixe_scenepass, suffixe_sous_titres,
};
use super::plan::{
    Conflit, Disposition, ElementPlan, NatureElement, Operation, PlanRenommage, SondeDisque,
    construire, joindre, meme_chemin, nettoyer_ancetres, reloger,
};
use super::profil::ProfilRenommageSerie;
use crate::modele::{Episode, FichierMedia, Serie, TypeFichier};
use crate::scan::types::{DOSSIERS_EXTRAS, est_nom_dossier_disque};

/// Nom du dossier de la série (relatif à la source).
pub fn nom_dossier_serie(serie: &Serie, profil: &ProfilRenommageSerie) -> Result<String, Conflit> {
    let ctx = contexte_serie(serie, &profil.prefixes_titre);
    creer_destination(
        &profil.gabarit_dossier_serie,
        &ctx,
        jetons_serie(),
        FormeDestination::Dossier,
        &profil.options_destination(),
    )
    .map_err(|e| gabarit_invalide(&profil.gabarit_dossier_serie, e))
}

fn gabarit_invalide(g: &str, e: impl std::fmt::Display) -> Conflit {
    Conflit::GabaritInvalide {
        gabarit: g.to_string(),
        message: e.to_string(),
    }
}

/// Gabarits « recommandés » : le fichier reprend le nom d'origine, ou les
/// numéros de saison et d'épisode y figurent une seule fois (saison avant
/// épisode).
pub fn gabarits_recommandes(gabarit_saison: &str, gabarit_fichier: &str) -> bool {
    if contient_jeton(gabarit_fichier, &["originalFilename", "originalBasename"]) {
        return true;
    }
    let l = ListesJetonsEpisode::default();
    let pos = |g: &str, liste: &[String]| -> Vec<usize> {
        let mut v = Vec::new();
        for j in liste {
            let motif = format!("${{{j}");
            let bas = g.to_lowercase();
            let mut debut = 0;
            while let Some(i) = bas[debut..].find(&motif.to_lowercase()) {
                let apres = bas[debut + i + motif.len()..].chars().next();
                if matches!(apres, Some('}' | ';' | '[' | ',')) {
                    v.push(debut + i);
                }
                debut += i + motif.len();
            }
        }
        v.sort_unstable();
        v.dedup();
        v
    };
    let ep = pos(gabarit_fichier, &l.episode);
    let sa_f = pos(gabarit_fichier, &l.saison);
    let sa_d = pos(gabarit_saison, &l.saison);
    if ep.len() != 1 || sa_f.len() > 1 || sa_d.len() > 1 || sa_f.len() + sa_d.len() == 0 {
        return false;
    }
    sa_f.first().is_none_or(|&s| s < ep[0])
}

/// Nom du dossier de saison (`Specials` pour la saison 0 si demandé).
pub fn nom_dossier_saison(
    serie: &Serie,
    ep: &Episode,
    profil: &ProfilRenommageSerie,
) -> Result<String, Conflit> {
    let saison = ep.saison(&serie.groupe_actif);
    if saison < 0 {
        return Ok(String::new());
    }
    let gabarit = &profil.gabarit_dossier_saison;
    if saison == 0 && profil.saison_speciale && !gabarit.trim().is_empty() {
        return Ok("Specials".to_string());
    }
    let ctx = contexte_episode(serie, ep, &profil.prefixes_titre);
    let n = creer_destination(
        gabarit,
        &ctx,
        jetons_serie(),
        FormeDestination::Dossier,
        &profil.options_destination_saison(),
    )
    .map_err(|e| gabarit_invalide(gabarit, e))?;
    if n.is_empty() && !gabarits_recommandes(gabarit, &profil.gabarit_fichier) {
        return Ok(format!("Season {saison}"));
    }
    Ok(n)
}

/// Nom de base des fichiers d'un ou plusieurs épisodes (multi-épisodes).
pub fn nom_fichier_episodes(
    serie: &Serie,
    episodes: &[&Episode],
    profil: &ProfilRenommageSerie,
) -> Result<String, Conflit> {
    let contextes: Vec<EpisodeContexte> = episodes
        .iter()
        .map(|e| EpisodeContexte {
            saison: i64::from(e.saison(&serie.groupe_actif)),
            episode: i64::from(e.episode(&serie.groupe_actif)),
            contexte: contexte_episode(serie, e, &profil.prefixes_titre),
        })
        .collect();
    let r = creer_destination_multi(
        &profil.gabarit_fichier,
        &contextes,
        profil.style_multi_episodes.into(),
        &ListesJetonsEpisode::default(),
        jetons_serie(),
        FormeDestination::Fichier,
        &profil.options_destination(),
    )
    .map_err(|e| gabarit_invalide(&profil.gabarit_fichier, e))?;
    Ok(r)
}

/// Planifie le renommage d'une série : un élément pour le dossier de la
/// série, puis un élément par fichier vidéo (épisodes regroupés).
pub fn planifier_serie(
    serie: &Serie,
    profil: &ProfilRenommageSerie,
    sonde: &dyn SondeDisque,
) -> PlanRenommage {
    let mut plan = PlanRenommage::default();
    let ancien = serie.chemin.clone();
    let source = serie.source_donnees.clone();
    let mut el = ElementPlan::nouveau(
        NatureElement::Serie {
            id: serie.id.clone(),
        },
        serie.titre.clone(),
        &source,
        &ancien,
    );
    if source.as_os_str().is_empty() {
        el.conflits
            .push(Conflit::DonneesManquantes("source de données vide".into()));
    }
    if serie.titre.trim().is_empty() {
        el.conflits
            .push(Conflit::DonneesManquantes("titre vide".into()));
    }
    let mut nouveau = ancien.clone();
    if el.conflits.is_empty()
        && profil.dossier_serie_actif
        && !profil.gabarit_dossier_serie.trim().is_empty()
    {
        match nom_dossier_serie(serie, profil) {
            Ok(n) if !n.is_empty() => nouveau = joindre(&source, &n),
            Ok(_) => {}
            Err(c) => el.conflits.push(c),
        }
    }
    let deplace = nouveau != ancien;
    if deplace {
        let casse_seule = meme_chemin(&nouveau, &ancien);
        if !casse_seule && (nouveau.starts_with(&ancien) || sonde.existe(&nouveau)) {
            el.conflits.push(Conflit::DossierExistant(nouveau.clone()));
        }
        el.operations.push(Operation::DeplacerDossier {
            de: ancien.clone(),
            vers: nouveau.clone(),
        });
    }
    el.nouveau_dossier = nouveau.clone();
    // Fichiers propres à la série (artwork, NFO) : noms génériques, ils
    // suivent le dossier.
    let dispositions: Vec<Disposition> = serie
        .fichiers
        .iter()
        .map(|f| {
            let c = reloger(&f.chemin_complet(), &ancien, &nouveau);
            Disposition {
                fichier: f.clone(),
                courant: c.clone(),
                cibles: vec![c],
                supprimer: false,
            }
        })
        .collect();
    construire(&mut el, dispositions, deplace, sonde);
    if deplace {
        nettoyer_ancetres(&mut el.operations, &ancien, false, &source, &nouveau);
    }
    let bloque = !el.est_executable();
    plan.elements.push(el);
    let indice_serie = 0;

    // Épisodes regroupés par fichier vidéo principal.
    let mut groupes: Vec<(Option<PathBuf>, Vec<&Episode>)> = Vec::new();
    for ep in serie.episodes.iter().filter(|e| !e.est_fictif()) {
        let cle = principal_episode(ep).map(FichierMedia::chemin_complet);
        match groupes.iter_mut().find(|g| g.0.is_some() && g.0 == cle) {
            Some(g) => g.1.push(ep),
            None => groupes.push((cle, vec![ep])),
        }
    }
    for (_, eps) in groupes {
        let mut e = planifier_episodes(serie, &eps, &nouveau, profil, sonde);
        if deplace || bloque {
            e.depend_de = Some(indice_serie);
        }
        plan.elements.push(e);
    }
    plan.verifier();
    plan
}

/// Planifie un groupe d'épisodes partageant leurs fichiers. `dossier_serie`
/// est le dossier de la série **après** son éventuel renommage.
pub fn planifier_episodes(
    serie: &Serie,
    episodes: &[&Episode],
    dossier_serie: &Path,
    profil: &ProfilRenommageSerie,
    sonde: &dyn SondeDisque,
) -> ElementPlan {
    let ancien_serie = serie.chemin.clone();
    let groupe = &serie.groupe_actif;
    let mut eps: Vec<&Episode> = episodes.to_vec();
    eps.sort_by_key(|e| (e.saison(groupe), e.episode(groupe)));
    let premier = eps[0];
    let libelle = eps
        .iter()
        .map(|e| format!("S{:02}E{:02}", e.saison(groupe), e.episode(groupe)))
        .collect::<Vec<_>>()
        .join(" ");
    // Fichiers réunis (dédoublonnés par chemin).
    let mut fichiers: Vec<FichierMedia> = Vec::new();
    for e in &eps {
        for f in &e.fichiers {
            if !fichiers.contains(f) {
                fichiers.push(f.clone());
            }
        }
    }
    let principal = principal_episode(premier).cloned();
    let ancien_dossier = principal
        .as_ref()
        .map(|p| p.chemin.clone())
        .or_else(|| fichiers.first().map(|f| f.chemin.clone()))
        .unwrap_or_else(|| ancien_serie.clone());
    let mut el = ElementPlan::nouveau(
        NatureElement::Episodes {
            ids: eps.iter().map(|e| e.id.clone()).collect(),
        },
        format!("{} {}", serie.titre, libelle),
        &serie.source_donnees,
        &reloger(&ancien_dossier, &ancien_serie, dossier_serie),
    );
    el.ancien_dossier = ancien_dossier.clone();
    if eps
        .iter()
        .any(|e| e.saison(groupe) < 0 || e.episode(groupe) < 0)
    {
        el.conflits.push(Conflit::DonneesManquantes(
            "numéro de saison ou d'épisode inconnu".into(),
        ));
        return el;
    }
    // Emplacement courant d'un fichier (après déplacement du dossier de série).
    let courant = |f: &FichierMedia| reloger(&f.chemin_complet(), &ancien_serie, dossier_serie);

    // Disque : seul le dossier du disque est renommé (gabarit de fichier).
    let disque = premier.est_disque
        || principal
            .as_ref()
            .is_some_and(|p| racine_disque(&p.chemin).is_some());
    // Dossier de saison.
    let dossier_saison = if profil.dossier_saison_actif {
        match nom_dossier_saison(serie, premier, profil) {
            Ok(n) => joindre(dossier_serie, &n),
            Err(c) => {
                el.conflits.push(c);
                return el;
            }
        }
    } else {
        let d = principal
            .as_ref()
            .map(|p| {
                racine_disque(&p.chemin)
                    .and_then(|r| r.parent().map(Path::to_path_buf))
                    .unwrap_or_else(|| p.chemin.clone())
            })
            .unwrap_or_else(|| ancien_dossier.clone());
        reloger(&d, &ancien_serie, dossier_serie)
    };
    // Nom de base.
    let ancienne_base = principal.as_ref().map(base_sans_pile).unwrap_or_default();
    let mut base = ancienne_base.clone();
    if profil.fichier_actif && !profil.gabarit_fichier.trim().is_empty() {
        match nom_fichier_episodes(serie, &eps, profil) {
            Ok(n) if !n.is_empty() => base = n,
            Ok(_) => {}
            Err(c) => {
                el.conflits.push(c);
                return el;
            }
        }
    }
    if disque && let Some(racine) = principal.as_ref().and_then(|p| racine_disque(&p.chemin)) {
        {
            let de = reloger(&racine, &ancien_serie, dossier_serie);
            let nom_dossier = if base.is_empty() {
                racine
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            } else {
                base.clone()
            };
            let vers = dossier_saison.join(nom_dossier);
            el.nouveau_dossier = vers.clone();
            el.ancien_dossier = racine.clone();
            let change = de != vers;
            if change {
                if !meme_chemin(&de, &vers) && sonde.existe(&vers) {
                    el.conflits.push(Conflit::DossierExistant(vers.clone()));
                }
                el.operations.push(Operation::DeplacerDossier {
                    de: de.clone(),
                    vers: vers.clone(),
                });
            }
            let dispositions = fichiers
                .iter()
                .map(|f| {
                    let c = courant(f);
                    let cible = reloger(&c, &de, &vers);
                    let c2 = if change && c.starts_with(&de) {
                        cible.clone()
                    } else {
                        c
                    };
                    Disposition {
                        fichier: f.clone(),
                        courant: c2,
                        cibles: vec![cible],
                        supprimer: false,
                    }
                })
                .collect();
            construire(&mut el, dispositions, change, sonde);
            if change {
                let ancien_parent = de.parent().map(Path::to_path_buf).unwrap_or_default();
                nettoyer_ancetres(
                    &mut el.operations,
                    &ancien_parent,
                    true,
                    dossier_serie,
                    &vers,
                );
            }
            return el;
        }
    }
    el.nouveau_dossier = dossier_saison.clone();
    let sep = profil.separateur_pile();
    let ext_video = principal
        .as_ref()
        .map(|p| extension_originale(&p.nom))
        .unwrap_or_default();
    let renommer = !base.is_empty();
    let dans = |nom: String| vec![dossier_saison.join(nom)];

    // NFO : le plus récent est copié vers chaque nom.
    let nfo_choisi = fichiers
        .iter()
        .filter(|f| f.type_fichier == TypeFichier::Nfo)
        .max_by_key(|f| f.date_fichier)
        .map(FichierMedia::chemin_complet);
    let sub: Vec<&FichierMedia> = fichiers.iter().filter(|f| f.extension() == "sub").collect();

    let mut dispositions = Vec::new();
    for f in &fichiers {
        let c = courant(f);
        let ext = extension_originale(&f.nom);
        let garder = vec![reloger(
            &c,
            &reloger(&ancien_dossier, &ancien_serie, dossier_serie),
            &dossier_saison,
        )];
        let pile = chaine_pile(f, &sep, premier.empile);
        let difference_ou_garder = |suffixe_vide_ok: bool| -> Vec<PathBuf> {
            match difference(&f.nom_de_base(), &ancienne_base) {
                Some(d) if suffixe_vide_ok || !d.is_empty() => {
                    dans(avec_extension(&format!("{base}{d}"), &ext))
                }
                _ => garder.clone(),
            }
        };
        let mut supprimer = false;
        let cibles: Vec<PathBuf> = if !renommer {
            garder.clone()
        } else {
            match f.type_fichier {
                TypeFichier::Video => dans(avec_extension(&format!("{base}{pile}"), &ext)),
                TypeFichier::Nfo => {
                    if Some(f.chemin_complet()) == nfo_choisi {
                        profil
                            .noms
                            .nfo
                            .iter()
                            .filter_map(|m| appliquer_motif(m, &base, ""))
                            .map(|r| dossier_saison.join(format!("{r}.nfo")))
                            .collect()
                    } else {
                        supprimer = true;
                        Vec::new()
                    }
                }
                TypeFichier::Fanart => artwork(&profil.noms.fanart, &base, f, &dossier_saison),
                TypeFichier::Thumb => artwork(&profil.noms.vignette, &base, f, &dossier_saison),
                TypeFichier::Poster
                | TypeFichier::Banner
                | TypeFichier::Clearart
                | TypeFichier::Logo
                | TypeFichier::Clearlogo
                | TypeFichier::Disc
                | TypeFichier::Characterart
                | TypeFichier::Keyart => {
                    let suffixe = match f.type_fichier {
                        TypeFichier::Poster => "poster",
                        TypeFichier::Banner => "banner",
                        TypeFichier::Clearart => "clearart",
                        TypeFichier::Logo | TypeFichier::Clearlogo => "clearlogo",
                        TypeFichier::Disc => "discart",
                        TypeFichier::Characterart => "characterart",
                        _ => "keyart",
                    };
                    dans(avec_extension(
                        &format!("{base}-{suffixe}"),
                        &extension_artwork(f),
                    ))
                }
                TypeFichier::Trailer => dans(avec_extension(&format!("{base}-trailer"), &ext)),
                TypeFichier::Sample => dans(avec_extension(&format!("{base}-sample"), &ext)),
                TypeFichier::SousTitres => {
                    let associe = (f.extension() == "idx")
                        .then(|| {
                            sub.iter().find(|s| {
                                s.chemin == f.chemin && s.nom_de_base() == f.nom_de_base()
                            })
                        })
                        .flatten();
                    let reference = associe.copied().unwrap_or(f);
                    let suffixe = suffixe_sous_titres(
                        reference,
                        &ancienne_base,
                        profil.style_langue_sous_titres,
                    );
                    let p = chaine_pile(reference, &sep, premier.empile);
                    dans(avec_extension(&format!("{base}{p}{suffixe}"), &ext))
                }
                TypeFichier::MediaInfo => dans(format!("{base}-mediainfo.xml")),
                TypeFichier::Vsmeta | TypeFichier::DoubleExt => {
                    dans(avec_extension(&avec_extension(&base, &ext_video), &ext))
                }
                TypeFichier::Skiptro => dans(format!("{base}.skiptro.json")),
                TypeFichier::Scenepass => dans(format!("{base}{}", suffixe_scenepass(&f.nom))),
                TypeFichier::Bif => dans(nom_bif(f, &base)),
                TypeFichier::Extra | TypeFichier::VideoExtra => {
                    if dans_dossier_nomme(f, &ancien_dossier, DOSSIERS_EXTRAS) {
                        garder.clone()
                    } else {
                        difference_ou_garder(true)
                    }
                }
                TypeFichier::Audio | TypeFichier::Texte | TypeFichier::Inconnu => {
                    difference_ou_garder(false)
                }
                _ => garder.clone(),
            }
        };
        dispositions.push(Disposition {
            fichier: f.clone(),
            courant: c,
            cibles,
            supprimer,
        });
    }
    let quitte = reloger(&ancien_dossier, &ancien_serie, dossier_serie);
    construire(&mut el, dispositions, false, sonde);
    if quitte != dossier_saison {
        nettoyer_ancetres(
            &mut el.operations,
            &quitte,
            true,
            dossier_serie,
            &dossier_saison,
        );
    }
    el
}

fn artwork(motifs: &[String], base: &str, f: &FichierMedia, dossier: &Path) -> Vec<PathBuf> {
    let ext = extension_artwork(f);
    motifs
        .iter()
        .filter_map(|m| appliquer_motif(m, base, ""))
        .map(|r| joindre(dossier, &avec_extension(&r, &ext)))
        .collect()
}

/// Dossier racine d'un disque : le dossier qui contient `VIDEO_TS`/`BDMV`.
fn racine_disque(dossier_fichier: &Path) -> Option<PathBuf> {
    let mut c = Some(dossier_fichier);
    while let Some(d) = c {
        if d.file_name()
            .is_some_and(|n| est_nom_dossier_disque(&n.to_string_lossy()))
        {
            return d.parent().map(Path::to_path_buf);
        }
        c = d.parent();
    }
    None
}
