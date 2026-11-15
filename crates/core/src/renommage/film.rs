//! Planification du renommage des films.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use mediamanager_gabarit::{
    FormeDestination, creer_destination, gabarit_dossier_unique, gabarit_fichier_valide,
};

use super::contexte::{contexte_film, jetons_film};
use super::noms::{
    appliquer_motif, avec_extension, base_sans_pile, chaine_pile, dans_dossier_nomme, difference,
    extension_artwork, extension_originale, indice_bande_annonce, indice_final, nom_bif,
    suffixe_scenepass, suffixe_sous_titres,
};
use super::plan::{
    Conflit, Disposition, ElementPlan, NatureElement, Operation, PlanRenommage, SondeDisque,
    construire, joindre, meme_chemin, nettoyer_ancetres, reloger,
};
use super::profil::{MARQUE_BASE, NomsFichiersFilm, ProfilRenommageFilm};
use crate::modele::{FichierMedia, Film, Saga, TypeFichier};
use crate::scan::types::DOSSIERS_EXTRAS;

/// Types d'artwork renommés avec le film (copie 1:N).
const TYPES_ARTWORK: [TypeFichier; 10] = [
    TypeFichier::Fanart,
    TypeFichier::Poster,
    TypeFichier::Banner,
    TypeFichier::Clearart,
    TypeFichier::Thumb,
    TypeFichier::Logo,
    TypeFichier::Clearlogo,
    TypeFichier::Disc,
    TypeFichier::Keyart,
    TypeFichier::Squareart,
];

/// Contexte d'un film à planifier.
#[derive(Debug, Clone, Copy)]
pub struct FilmARenommer<'a> {
    pub film: &'a Film,
    pub saga: Option<&'a Saga>,
}

/// Nom de dossier (relatif à la source) produit par le gabarit du profil.
pub fn nom_dossier_film(
    film: &Film,
    saga: Option<&Saga>,
    profil: &ProfilRenommageFilm,
) -> Result<String, Conflit> {
    rendre(
        film,
        saga,
        profil,
        &profil.gabarit_dossier,
        FormeDestination::Dossier,
    )
}

/// Nom de base (sans extension ni pile) produit par le gabarit de fichier.
pub fn nom_fichier_film(
    film: &Film,
    saga: Option<&Saga>,
    profil: &ProfilRenommageFilm,
) -> Result<String, Conflit> {
    rendre(
        film,
        saga,
        profil,
        &profil.gabarit_fichier,
        FormeDestination::Fichier,
    )
}

fn rendre(
    film: &Film,
    saga: Option<&Saga>,
    profil: &ProfilRenommageFilm,
    gabarit: &str,
    forme: FormeDestination,
) -> Result<String, Conflit> {
    let ctx = contexte_film(
        film,
        saga,
        profil.saga_pour_film_seul,
        &profil.prefixes_titre,
    );
    creer_destination(
        gabarit,
        &ctx,
        jetons_film(),
        forme,
        &profil.options_destination(),
    )
    .map_err(|e| Conflit::GabaritInvalide {
        gabarit: gabarit.to_string(),
        message: e.to_string(),
    })
}

/// Planifie plusieurs films et détecte les doublons entre eux. La saga de
/// chaque film est retrouvée dans `sagas` par son identifiant.
pub fn planifier_films(
    films: &[&Film],
    sagas: &[Saga],
    profil: &ProfilRenommageFilm,
    sonde: &dyn SondeDisque,
) -> PlanRenommage {
    let mut plan = PlanRenommage::default();
    for f in films {
        let saga = f
            .saga_id
            .as_ref()
            .and_then(|id| sagas.iter().find(|s| &s.id == id));
        plan.elements.push(planifier_film(
            FilmARenommer { film: f, saga },
            profil,
            sonde,
        ));
    }
    plan.verifier();
    plan
}

/// Planifie le renommage d'un film (sans aucune écriture).
pub fn planifier_film(
    a: FilmARenommer<'_>,
    profil: &ProfilRenommageFilm,
    sonde: &dyn SondeDisque,
) -> ElementPlan {
    let film = a.film;
    let ancien = film.chemin.clone();
    let source = film.source_donnees.clone();
    let libelle = if film.annee > 0 {
        format!("{} ({})", film.titre, film.annee)
    } else {
        film.titre.clone()
    };
    let mut el = ElementPlan::nouveau(
        NatureElement::Film {
            id: film.id.clone(),
        },
        libelle,
        &source,
        &ancien,
    );
    el.dossier_multi_films = film.dossier_multi_films;
    if source.as_os_str().is_empty() {
        el.conflits
            .push(Conflit::DonneesManquantes("source de données vide".into()));
    }
    if film.titre.trim().is_empty() {
        el.conflits
            .push(Conflit::DonneesManquantes("titre vide".into()));
    }
    if !el.conflits.is_empty() {
        return el;
    }

    // --- Dossier -----------------------------------------------------------
    let mut nouveau = ancien.clone();
    let gabarit_dossier_actif = profil.dossier_actif && !profil.gabarit_dossier.trim().is_empty();
    if gabarit_dossier_actif {
        match nom_dossier_film(film, a.saga, profil) {
            Ok(n) if !n.is_empty() => nouveau = joindre(&source, &n),
            Ok(_) => el
                .avertissements
                .push("le gabarit de dossier donne un nom vide : dossier conservé".into()),
            Err(c) => {
                el.conflits.push(c);
                return el;
            }
        }
        if !gabarit_dossier_unique(&profil.gabarit_dossier) {
            el.avertissements.push(
                "gabarit de dossier non unique : plusieurs films peuvent partager un dossier"
                    .into(),
            );
        }
    }
    let source_mmd = film.dossier_multi_films;
    let change = nouveau != ancien;
    let mut dest_mmd = source_mmd;
    if change {
        dest_mmd = !gabarit_dossier_unique(&profil.gabarit_dossier)
            || (profil.autoriser_multi_films_meme_dossier
                && !meme_chemin(&nouveau, &ancien)
                && sonde.est_dossier_non_vide(&nouveau));
    }
    el.nouveau_dossier = nouveau.clone();
    el.dossier_multi_films = dest_mmd;

    let imbrique = nouveau.starts_with(&ancien) || ancien.starts_with(&nouveau);
    let casse_seule = change && meme_chemin(&nouveau, &ancien);
    let deplacement_dossier = change && !source_mmd && !dest_mmd && (!imbrique || casse_seule);
    if change {
        if deplacement_dossier {
            if !casse_seule && sonde.existe(&nouveau) {
                el.conflits.push(Conflit::DossierExistant(nouveau.clone()));
            }
            el.operations.push(Operation::DeplacerDossier {
                de: ancien.clone(),
                vers: nouveau.clone(),
            });
        } else {
            if !dest_mmd && sonde.existe(&nouveau) {
                el.conflits.push(Conflit::DossierExistant(nouveau.clone()));
            }
            el.operations.push(Operation::CreerDossier {
                chemin: nouveau.clone(),
            });
        }
    }
    let courant = |f: &FichierMedia| -> PathBuf {
        let c = f.chemin_complet();
        if deplacement_dossier {
            reloger(&c, &ancien, &nouveau)
        } else {
            c
        }
    };
    let dest_relatif = |f: &FichierMedia| -> PathBuf {
        // Dossier du fichier transposé dans le nouveau dossier du film.
        reloger(&f.chemin, &ancien, &nouveau)
    };

    // --- Nom de base --------------------------------------------------------
    let principal = film.fichier_principal();
    let ancienne_base = principal.map(base_sans_pile).unwrap_or_default();
    let mut base = ancienne_base.clone();
    if profil.fichier_actif {
        if gabarit_fichier_valide(&profil.gabarit_fichier) {
            match nom_fichier_film(film, a.saga, profil) {
                Ok(n) if !n.is_empty() => base = n,
                Ok(_) => {}
                Err(c) => {
                    el.conflits.push(c);
                    return el;
                }
            }
        } else {
            el.avertissements.push(
                "gabarit de fichier sans titre ni identifiant : fichiers non renommés".into(),
            );
        }
    }
    let renommer = !base.is_empty();
    let sep = profil.separateur_pile();

    // --- Dispositions --------------------------------------------------------
    let mut dispositions: Vec<Disposition> = Vec::new();
    let mut traite = vec![false; film.fichiers.len()];
    let garder = |f: &FichierMedia| Disposition {
        fichier: f.clone(),
        courant: courant(f),
        cibles: vec![dest_relatif(f).join(&f.nom)],
        supprimer: false,
    };

    // Artwork : le plus récent de chaque type est copié vers chaque nom.
    let mut par_type: BTreeMap<TypeFichier, Vec<usize>> = BTreeMap::new();
    for (i, f) in film.fichiers.iter().enumerate() {
        let t = match f.type_fichier {
            TypeFichier::Logo => TypeFichier::Clearlogo,
            t => t,
        };
        if TYPES_ARTWORK.contains(&t) {
            par_type.entry(t).or_default().push(i);
        }
    }
    for (t, indices) in &par_type {
        let motifs = motifs_artwork(*t, &profil.noms, dest_mmd, film.est_disque);
        let choisi = plus_recent(film, indices);
        // Fichiers (autres que le choisi) déjà présents sous un nom voulu.
        let voulus: Vec<PathBuf> = {
            let ext = extension_artwork(&film.fichiers[choisi]);
            motifs
                .iter()
                .filter_map(|m| appliquer_motif(m, &base, ""))
                .map(|r| joindre(&nouveau, &avec_extension(&r, &ext)))
                .collect()
        };
        let deja_en_place: Vec<PathBuf> = indices
            .iter()
            .filter(|&&i| i != choisi)
            .map(|&i| dest_relatif(&film.fichiers[i]).join(&film.fichiers[i].nom))
            .filter(|c| voulus.contains(c))
            .collect();
        for &i in indices {
            traite[i] = true;
            let f = &film.fichiers[i];
            if motifs.is_empty() || !renommer {
                dispositions.push(garder(f));
                continue;
            }
            if i == choisi {
                let ext = extension_artwork(f);
                let cibles: Vec<PathBuf> = motifs
                    .iter()
                    .filter_map(|m| appliquer_motif(m, &base, ""))
                    .map(|r| joindre(&nouveau, &avec_extension(&r, &ext)))
                    .filter(|c| !deja_en_place.contains(c))
                    .collect();
                dispositions.push(Disposition {
                    fichier: f.clone(),
                    courant: courant(f),
                    cibles,
                    supprimer: false,
                });
            } else if deja_en_place.contains(&dest_relatif(f).join(&f.nom)) {
                // Copie déjà présente sous un nom voulu : conservée.
                dispositions.push(garder(f));
            } else {
                dispositions.push(Disposition {
                    fichier: f.clone(),
                    courant: courant(f),
                    cibles: Vec::new(),
                    supprimer: true,
                });
            }
        }
    }

    // NFO : le plus récent est copié vers chaque nom configuré.
    let nfos: Vec<usize> = film
        .fichiers
        .iter()
        .enumerate()
        .filter(|(_, f)| f.type_fichier == TypeFichier::Nfo)
        .map(|(i, _)| i)
        .collect();
    if !nfos.is_empty() {
        let motifs: Vec<String> = if dest_mmd {
            vec![MARQUE_BASE.to_string()]
        } else {
            profil.noms.nfo.clone()
        };
        let choisi = plus_recent(film, &nfos);
        for &i in &nfos {
            traite[i] = true;
            let f = &film.fichiers[i];
            if film.est_disque || !renommer || motifs.is_empty() {
                dispositions.push(garder(f));
            } else if i == choisi {
                let cibles = motifs
                    .iter()
                    .filter_map(|m| appliquer_motif(m, &base, ""))
                    .map(|r| joindre(&nouveau, &format!("{r}.nfo")))
                    .collect();
                dispositions.push(Disposition {
                    fichier: f.clone(),
                    courant: courant(f),
                    cibles,
                    supprimer: false,
                });
            } else if profil.nettoyage_nfo {
                dispositions.push(Disposition {
                    fichier: f.clone(),
                    courant: courant(f),
                    cibles: Vec::new(),
                    supprimer: true,
                });
            } else {
                dispositions.push(garder(f));
            }
        }
    }

    // Autres fichiers.
    let ext_video = principal
        .map(|p| extension_originale(&p.nom))
        .unwrap_or_default();
    let sous_titres_sub: Vec<&FichierMedia> = film
        .fichiers
        .iter()
        .filter(|f| f.extension() == "sub")
        .collect();
    for (i, f) in film.fichiers.iter().enumerate() {
        if traite[i] {
            continue;
        }
        let pile = |detecter: bool| chaine_pile(f, &sep, detecter && film.empile);
        let ext = extension_originale(&f.nom);
        let dans = |nom: String| vec![joindre(&nouveau, &nom)];
        let cibles: Vec<PathBuf> = if !renommer {
            vec![dest_relatif(f).join(&f.nom)]
        } else {
            match f.type_fichier {
                TypeFichier::Video if film.est_disque => vec![dest_relatif(f).join(&f.nom)],
                TypeFichier::Video => dans(avec_extension(&format!("{base}{}", pile(false)), &ext)),
                TypeFichier::Trailer => {
                    if dans_dossier_nomme(f, &ancien, &["trailer", "trailers"]) {
                        vec![dest_relatif(f).join(&f.nom)]
                    } else {
                        let motifs: Vec<String> = if dest_mmd || film.est_disque {
                            vec!["<b>-trailer".to_string()]
                        } else {
                            profil.noms.trailer.clone()
                        };
                        let indice = indice_bande_annonce(f);
                        motifs
                            .iter()
                            .filter_map(|m| appliquer_motif(m, &base, ""))
                            .map(|r| {
                                joindre(&nouveau, &avec_extension(&format!("{r}{indice}"), &ext))
                            })
                            .collect()
                    }
                }
                TypeFichier::Extra | TypeFichier::VideoExtra => {
                    if dans_dossier_nomme(f, &ancien, DOSSIERS_EXTRAS) {
                        vec![dest_relatif(f).join(&f.nom)]
                    } else {
                        match difference(&f.nom_de_base(), &ancienne_base) {
                            Some(d) => vec![
                                dest_relatif(f).join(avec_extension(&format!("{base}{d}"), &ext)),
                            ],
                            None => vec![dest_relatif(f).join(&f.nom)],
                        }
                    }
                }
                TypeFichier::Sample => dans(avec_extension(&format!("{base}-sample"), &ext)),
                TypeFichier::Bif => dans(nom_bif(f, &base)),
                TypeFichier::MediaInfo if film.est_disque => vec![dest_relatif(f).join(&f.nom)],
                TypeFichier::MediaInfo => dans(avec_extension(
                    &format!("{base}{}-mediainfo", pile(true)),
                    &ext,
                )),
                TypeFichier::DoubleExt | TypeFichier::Vsmeta if film.est_disque => {
                    vec![dest_relatif(f).join(&f.nom)]
                }
                TypeFichier::DoubleExt | TypeFichier::Vsmeta => dans(avec_extension(
                    &avec_extension(&format!("{base}{}", pile(true)), &ext_video),
                    &ext,
                )),
                TypeFichier::Skiptro => dans(format!("{base}{}.skiptro.json", pile(true))),
                TypeFichier::Scenepass => {
                    dans(format!("{base}{}{}", pile(true), suffixe_scenepass(&f.nom)))
                }
                TypeFichier::SousTitres => {
                    let ext_min = f.extension();
                    let associe = (ext_min == "idx")
                        .then(|| {
                            sous_titres_sub.iter().find(|s| {
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
                    let p = chaine_pile(reference, &sep, film.empile);
                    dans(avec_extension(&format!("{base}{p}{suffixe}"), &ext))
                }
                TypeFichier::ExtraFanart => {
                    let motifs = motifs_artwork(
                        TypeFichier::ExtraFanart,
                        &profil.noms,
                        dest_mmd,
                        film.est_disque,
                    );
                    let indice = indice_final(&f.nom_de_base());
                    match motifs
                        .first()
                        .and_then(|m| appliquer_motif(m, &base, &indice))
                    {
                        Some(r) => dans(avec_extension(&r, &extension_artwork(f))),
                        None => vec![dest_relatif(f).join(&f.nom)],
                    }
                }
                _ => vec![dest_relatif(f).join(&f.nom)],
            }
        };
        dispositions.push(Disposition {
            fichier: f.clone(),
            courant: courant(f),
            cibles,
            supprimer: false,
        });
    }

    construire(&mut el, dispositions, deplacement_dossier, sonde);

    // --- Nettoyage ------------------------------------------------------------
    if change {
        if deplacement_dossier {
            nettoyer_ancetres(&mut el.operations, &ancien, false, &source, &nouveau);
        } else {
            nettoyer_sous_dossiers_quittes(&mut el.operations, film, &ancien, &nouveau);
            nettoyer_ancetres(&mut el.operations, &ancien, true, &source, &nouveau);
        }
    }
    if !dest_mmd {
        if profil.nettoyer_indesirables {
            el.operations.push(Operation::NettoyerIndesirables {
                racine: nouveau.clone(),
                motifs: profil.motifs_indesirables.clone(),
            });
        }
        el.operations.push(Operation::NettoyerSousDossiersVides {
            racine: nouveau.clone(),
        });
    }
    el
}

/// Sous-dossiers quittés par des fichiers lors d'un déplacement fichier par
/// fichier (extras, trailers…), du plus profond au moins profond.
fn nettoyer_sous_dossiers_quittes(
    ops: &mut Vec<Operation>,
    film: &Film,
    ancien: &Path,
    nouveau: &Path,
) {
    let mut dossiers: Vec<PathBuf> = film
        .fichiers
        .iter()
        .map(|f| f.chemin.clone())
        .filter(|c| c != ancien && c.starts_with(ancien) && !nouveau.starts_with(c))
        .collect();
    dossiers.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    dossiers.dedup();
    for d in dossiers {
        let mut courant = Some(d);
        while let Some(c) = courant {
            if c == ancien {
                break;
            }
            let op = Operation::SupprimerDossierVide { chemin: c.clone() };
            if !ops.contains(&op) {
                ops.push(op);
            }
            courant = c.parent().map(Path::to_path_buf);
        }
    }
}

/// Indice du fichier le plus récent (le premier en cas d'égalité).
fn plus_recent(film: &Film, indices: &[usize]) -> usize {
    let mut meilleur = indices[0];
    for &i in &indices[1..] {
        if film.fichiers[i].date_fichier > film.fichiers[meilleur].date_fichier {
            meilleur = i;
        }
    }
    meilleur
}

/// Motifs de nom d'un type d'artwork selon le contexte (film normal,
/// dossier multi-films, disque).
pub fn motifs_artwork(
    t: TypeFichier,
    noms: &NomsFichiersFilm,
    multi_films: bool,
    disque: bool,
) -> Vec<String> {
    let configures: &Vec<String> = match t {
        TypeFichier::Poster => &noms.poster,
        TypeFichier::Fanart => &noms.fanart,
        TypeFichier::ExtraFanart => &noms.extrafanart,
        TypeFichier::Banner => &noms.banner,
        TypeFichier::Clearart => &noms.clearart,
        TypeFichier::Clearlogo | TypeFichier::Logo => &noms.clearlogo,
        TypeFichier::Thumb => &noms.thumb,
        TypeFichier::Disc => &noms.disc,
        TypeFichier::Keyart => &noms.keyart,
        TypeFichier::Squareart => &noms.squareart,
        TypeFichier::Trailer => &noms.trailer,
        _ => return Vec::new(),
    };
    if configures.is_empty() {
        return Vec::new();
    }
    if multi_films {
        let fichiers: Vec<String> = configures
            .iter()
            .filter(|m| m.contains(MARQUE_BASE))
            .cloned()
            .collect();
        if !fichiers.is_empty() {
            return fichiers;
        }
        let a = |m: &str| configures.iter().any(|c| c.eq_ignore_ascii_case(m));
        let repli = match t {
            TypeFichier::Poster => "<b>-poster",
            TypeFichier::Fanart => "<b>-fanart",
            TypeFichier::ExtraFanart => "<b>-fanart<n>",
            TypeFichier::Thumb if a("thumb") => "<b>-thumb",
            TypeFichier::Thumb => "<b>-landscape",
            TypeFichier::Disc if a("disc") => "<b>-disc",
            TypeFichier::Disc => "<b>-discart",
            TypeFichier::Banner => "<b>-banner",
            TypeFichier::Clearart => "<b>-clearart",
            TypeFichier::Clearlogo | TypeFichier::Logo => "<b>-clearlogo",
            TypeFichier::Keyart => "<b>-keyart",
            TypeFichier::Squareart => "<b>-squareart",
            _ => "<b>-trailer",
        };
        return vec![repli.to_string()];
    }
    if disque {
        let generiques: Vec<String> = configures
            .iter()
            .filter(|m| !m.contains(MARQUE_BASE))
            .cloned()
            .collect();
        if !generiques.is_empty() {
            return generiques;
        }
        let premier = &configures[0];
        let r = premier
            .replace("<b>-", "")
            .replace("<b>.", "")
            .replace(MARQUE_BASE, "");
        return vec![r];
    }
    configures.clone()
}
