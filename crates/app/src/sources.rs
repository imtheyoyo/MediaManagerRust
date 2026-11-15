//! Mise à jour des sources de données (films et séries).
//!
//! Pour chaque source : scan du disque (sans verrou sur la base), puis fusion
//! avec les entités existantes de cette source :
//! - correspondance **par chemin** (dossier), départagée par le nom du
//!   fichier vidéo quand plusieurs films partagent un dossier ;
//! - entités **verrouillées** laissées telles quelles (ni modifiées, ni
//!   retirées) ;
//! - métadonnées existantes conservées : seuls les fichiers et les
//!   indicateurs de structure (disque, pile, multi-films…) sont rafraîchis ;
//! - nouvelles entités complétées par leurs **NFO** présents, puis marquées
//!   « nouveau » ;
//! - entités disparues **supprimées** ou **marquées** selon
//!   [`OptionsMiseAJour::disparus`] ;
//! - enregistrement par lots transactionnels.
//!
//! Une source introuvable ou vide (disque non monté) est signalée et rien
//! n'est retiré de la base pour elle.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use mediamanager_core::modele::{
    Edition, Episode, FichierMedia, Film, Saga, Serie, SourceMedia, TypeFichier,
    comparer_fichiers_film,
};
use mediamanager_core::nfo::{
    lire_fichier_nfo_episodes, lire_fichier_nfo_film, lire_fichier_nfo_serie,
};
use mediamanager_core::scan::{OptionsScan, scanner_source};
use mediamanager_core::series::{recalculer_saisons, scanner_source_series, trier_episodes};
use mediamanager_store::base::nouvel_id;
use mediamanager_store::{Reglages, Stockage};

use crate::application::Noyau;
use crate::erreur::Resultat;
use crate::evenements::{Evenement, NiveauMessage};
use crate::taches::ContexteTache;

/// Clé du magasin clé/valeur : films marqués « nouveau ».
pub const CLE_FILMS_NOUVEAUX: &str = "app.films_nouveaux";
/// Clé du magasin clé/valeur : films marqués « disparu ».
pub const CLE_FILMS_DISPARUS: &str = "app.films_disparus";
/// Clé du magasin clé/valeur : séries marquées « nouvelle ».
pub const CLE_SERIES_NOUVELLES: &str = "app.series_nouvelles";
/// Clé du magasin clé/valeur : séries marquées « disparue ».
pub const CLE_SERIES_DISPARUES: &str = "app.series_disparues";

/// Sort des entités dont les fichiers ont disparu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TraitementDisparus {
    /// Retirées de la base (comportement par défaut).
    #[default]
    Supprimer,
    /// Conservées en base et marquées « disparu ».
    Marquer,
}

/// Réglages de la mise à jour des sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionsMiseAJour {
    pub disparus: TraitementDisparus,
    /// Nombre d'entités par transaction.
    pub taille_lot: usize,
}

impl Default for OptionsMiseAJour {
    fn default() -> Self {
        Self {
            disparus: TraitementDisparus::Supprimer,
            taille_lot: 200,
        }
    }
}

/// Bilan d'une mise à jour.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapportMiseAJour {
    pub sources_traitees: Vec<PathBuf>,
    /// Sources inutilisables et raison (rien n'a été retiré pour elles).
    pub sources_indisponibles: Vec<(PathBuf, String)>,
    pub ajoutes: Vec<String>,
    pub modifies: Vec<String>,
    pub supprimes: Vec<String>,
    pub marques_disparus: Vec<String>,
    /// Entités verrouillées laissées intactes.
    pub verrouilles: usize,
    pub avertissements: Vec<String>,
}

/// Résultat de la fusion d'une source.
#[derive(Debug, Clone, Default)]
pub struct Fusion<E> {
    /// Entités à enregistrer (nouvelles et modifiées), identifiant attribué.
    pub a_enregistrer: Vec<E>,
    pub nouveaux: Vec<String>,
    pub modifies: Vec<String>,
    /// Entités existantes dont les fichiers ont disparu.
    pub disparus: Vec<String>,
    pub verrouilles: usize,
}

// ---------------------------------------------------------------------------
// Marques (nouveau / disparu)
// ---------------------------------------------------------------------------

/// Lit une liste d'identifiants marqués.
pub fn lire_marques(s: &Stockage, cle: &str) -> Resultat<Vec<String>> {
    Ok(s.bibliotheque
        .lire_valeur(cle)?
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_default())
}

fn ecrire_marques(s: &mut Stockage, cle: &str, ids: &BTreeSet<String>) -> Resultat<()> {
    let v: Vec<&String> = ids.iter().collect();
    let json = serde_json::to_string(&v).unwrap_or_else(|_| "[]".into());
    s.bibliotheque.ecrire_valeur(cle, &json)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Outils
// ---------------------------------------------------------------------------

/// Source « propriétaire » d'un chemin : la plus profonde qui le contient.
pub fn source_de<'a>(chemin: &Path, sources: &'a [PathBuf]) -> Option<&'a PathBuf> {
    sources
        .iter()
        .filter(|s| chemin.starts_with(s))
        .max_by_key(|s| s.components().count())
}

fn options_scan(
    ignores: &[String],
    nomedia: bool,
    mots: &[String],
    reglages: &Reglages,
    source: &Path,
    toutes: &[PathBuf],
) -> OptionsScan {
    OptionsScan {
        dossiers_ignores: ignores.to_vec(),
        ignorer_nomedia: nomedia,
        mots_interdits: mots.to_vec(),
        prefixes_titre: reglages.general.prefixes_titre.clone(),
        sources_exclues: toutes
            .iter()
            .filter(|s| s.as_path() != source && s.starts_with(source))
            .cloned()
            .collect(),
    }
}

fn noms_videos(fichiers: &[FichierMedia]) -> BTreeSet<String> {
    fichiers
        .iter()
        .filter(|f| f.type_fichier == TypeFichier::Video)
        .map(|f| f.nom.to_lowercase())
        .collect()
}

/// Nouvelle liste de fichiers : celle du scan, en conservant les fichiers
/// déjà connus inchangés (même chemin, même taille) avec leurs informations
/// techniques.
fn fusionner_fichiers(existants: &[FichierMedia], scannes: &[FichierMedia]) -> Vec<FichierMedia> {
    let mut v: Vec<FichierMedia> = scannes
        .iter()
        .map(|s| {
            existants
                .iter()
                .find(|e| *e == s && e.taille == s.taille && e.type_fichier == s.type_fichier)
                .cloned()
                .unwrap_or_else(|| s.clone())
        })
        .collect();
    v.sort_by(comparer_fichiers_film);
    v
}

fn enregistrer_par_lots<E: mediamanager_store::Entite>(
    s: &mut Stockage,
    entites: &mut [E],
    supprimer: &[String],
    taille: usize,
) -> Resultat<()> {
    for paquet in entites.chunks_mut(taille.max(1)) {
        let mut lot = s.bibliotheque.lot()?;
        for e in paquet.iter_mut() {
            lot.enregistrer(e)?;
        }
        lot.valider()?;
    }
    for paquet in supprimer.chunks(taille.max(1)) {
        let mut lot = s.bibliotheque.lot()?;
        for id in paquet {
            lot.supprimer::<E>(id)?;
        }
        lot.valider()?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Films
// ---------------------------------------------------------------------------

/// Complète un film nouvellement détecté avec le premier NFO lisible de ses
/// fichiers. Les données de structure du scan (chemin, fichiers, source…)
/// priment ; titre et année du nom de fichier servent de repli.
pub fn completer_film_depuis_nfo(film: &mut Film) {
    let nfos: Vec<PathBuf> = film
        .fichiers_de_type(TypeFichier::Nfo)
        .map(|f| f.chemin_complet())
        .collect();
    for chemin in nfos {
        let lu = match lire_fichier_nfo_film(&chemin) {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("NFO illisible {chemin:?} : {e}");
                continue;
            }
        };
        if !lu.valide && lu.film.ids.est_vide() {
            continue;
        }
        let mut n = lu.film;
        n.id = String::new();
        n.chemin = film.chemin.clone();
        n.source_donnees = film.source_donnees.clone();
        n.fichiers = std::mem::take(&mut film.fichiers);
        n.est_disque = film.est_disque;
        n.empile = film.empile;
        n.hors_ligne = film.hors_ligne;
        n.dossier_multi_films = film.dossier_multi_films;
        n.nom_fichier_origine = film.nom_fichier_origine.clone();
        n.video_3d |= film.video_3d;
        if n.titre.trim().is_empty() {
            n.titre = film.titre.clone();
            if n.annee == 0 {
                n.annee = film.annee;
            }
        }
        if n.annee == 0 {
            n.annee = film.annee;
        }
        if n.source_media == SourceMedia::Inconnue {
            n.source_media = film.source_media;
        }
        if n.edition == Edition::Aucune {
            n.edition = film.edition.clone();
        }
        for (k, v) in film.ids.iter() {
            n.ids.definir_si_absent(k, &v.en_texte());
        }
        *film = n;
        return;
    }
}

/// Fusionne le résultat du scan d'une source avec les films existants de
/// cette source (fonction pure, hors lecture des NFO des nouveaux films).
pub fn fusionner_films(existants: &[Film], scannes: Vec<Film>) -> Fusion<Film> {
    let mut f = Fusion::default();
    let mut pris = vec![false; existants.len()];
    let mut par_dossier: BTreeMap<PathBuf, usize> = BTreeMap::new();
    for s in &scannes {
        *par_dossier.entry(s.chemin.clone()).or_default() += 1;
    }
    for mut s in scannes {
        let candidats: Vec<usize> = (0..existants.len())
            .filter(|&i| !pris[i] && existants[i].chemin == s.chemin)
            .collect();
        let videos = noms_videos(&s.fichiers);
        let choix = candidats
            .iter()
            .copied()
            .find(|&i| !noms_videos(&existants[i].fichiers).is_disjoint(&videos))
            .or_else(|| {
                (candidats.len() == 1
                    && par_dossier.get(&s.chemin) == Some(&1)
                    && !s.dossier_multi_films)
                    .then(|| candidats[0])
            });
        match choix {
            Some(i) => {
                pris[i] = true;
                let e = &existants[i];
                if e.verrouille {
                    f.verrouilles += 1;
                    continue;
                }
                let mut m = e.clone();
                m.fichiers = fusionner_fichiers(&e.fichiers, &s.fichiers);
                m.source_donnees = s.source_donnees.clone();
                m.est_disque = s.est_disque;
                m.empile = s.empile;
                m.hors_ligne = s.hors_ligne;
                m.dossier_multi_films = s.dossier_multi_films;
                for (k, v) in s.ids.iter() {
                    m.ids.definir_si_absent(k, &v.en_texte());
                }
                if m != *e {
                    f.modifies.push(m.id.clone());
                    f.a_enregistrer.push(m);
                }
            }
            None => {
                completer_film_depuis_nfo(&mut s);
                s.id = nouvel_id();
                f.nouveaux.push(s.id.clone());
                f.a_enregistrer.push(s);
            }
        }
    }
    for (i, e) in existants.iter().enumerate() {
        if pris[i] {
            continue;
        }
        if e.verrouille {
            f.verrouilles += 1;
        } else {
            f.disparus.push(e.id.clone());
        }
    }
    f
}

/// Tâche : mise à jour de toutes les sources de films.
pub(crate) fn mettre_a_jour_films(n: &Noyau, ctx: &ContexteTache) -> Resultat<RapportMiseAJour> {
    let reglages = n.reglages();
    let sources = reglages.films.sources.clone();
    let opts = n.config.mise_a_jour.clone();
    let mut rapport = RapportMiseAJour::default();
    let mut disparus_marques: BTreeSet<String> = lire_marques(&n.verrou(), CLE_FILMS_DISPARUS)?
        .into_iter()
        .collect();
    let total = sources.len() as u64;

    for (i, source) in sources.iter().enumerate() {
        ctx.verifier_annulation()?;
        ctx.progression(i as u64, total, format!("Analyse de {}", source.display()));
        let options = options_scan(
            &reglages.films.dossiers_ignores,
            reglages.films.ignorer_dossiers_nomedia,
            &reglages.films.mots_exclus,
            &reglages,
            source,
            &sources,
        );
        let scan = match scanner_source(source, &options) {
            Ok(r) => r,
            Err(e) => {
                ctx.message(NiveauMessage::Avertissement, e.to_string());
                rapport
                    .sources_indisponibles
                    .push((source.clone(), e.to_string()));
                continue;
            }
        };
        for a in &scan.avertissements {
            rapport.avertissements.push(format!("{a:?}"));
        }
        ctx.verifier_annulation()?;

        let mut s = n.verrou();
        let existants: Vec<Film> = s
            .bibliotheque
            .lister::<Film>()?
            .into_iter()
            .filter(|f| source_de(&f.chemin, &sources) == Some(source))
            .collect();
        let ids_source: BTreeSet<String> = existants.iter().map(|f| f.id.clone()).collect();
        let mut fusion = fusionner_films(&existants, scan.films);

        let supprimes = match opts.disparus {
            TraitementDisparus::Supprimer => fusion.disparus.clone(),
            TraitementDisparus::Marquer => Vec::new(),
        };
        enregistrer_par_lots(
            &mut s,
            &mut fusion.a_enregistrer,
            &supprimes,
            opts.taille_lot,
        )?;
        if !supprimes.is_empty() {
            retirer_des_sagas(&mut s, &supprimes)?;
        }
        disparus_marques.retain(|id| !ids_source.contains(id));
        if opts.disparus == TraitementDisparus::Marquer {
            disparus_marques.extend(fusion.disparus.iter().cloned());
            rapport
                .marques_disparus
                .extend(fusion.disparus.iter().cloned());
        }
        drop(s);

        for id in &fusion.nouveaux {
            ctx.emettre(Evenement::FilmAjoute { id: id.clone() });
        }
        for id in &fusion.modifies {
            ctx.emettre(Evenement::FilmModifie { id: id.clone() });
        }
        for id in &supprimes {
            ctx.emettre(Evenement::FilmSupprime { id: id.clone() });
        }
        ctx.message(
            NiveauMessage::Info,
            format!(
                "{} : {} ajouté(s), {} modifié(s), {} disparu(s)",
                source.display(),
                fusion.nouveaux.len(),
                fusion.modifies.len(),
                fusion.disparus.len()
            ),
        );
        rapport.sources_traitees.push(source.clone());
        rapport.ajoutes.extend(fusion.nouveaux);
        rapport.modifies.extend(fusion.modifies);
        rapport.supprimes.extend(supprimes);
        rapport.verrouilles += fusion.verrouilles;
    }

    let mut s = n.verrou();
    ecrire_marques(
        &mut s,
        CLE_FILMS_NOUVEAUX,
        &rapport.ajoutes.iter().cloned().collect(),
    )?;
    ecrire_marques(&mut s, CLE_FILMS_DISPARUS, &disparus_marques)?;
    drop(s);
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}

fn retirer_des_sagas(s: &mut Stockage, supprimes: &[String]) -> Resultat<()> {
    let mut sagas = s.bibliotheque.lister::<Saga>()?;
    let mut modifiees: Vec<Saga> = Vec::new();
    for saga in sagas.iter_mut() {
        let avant = saga.films.len();
        saga.films.retain(|id| !supprimes.contains(id));
        if saga.films.len() != avant {
            modifiees.push(saga.clone());
        }
    }
    if !modifiees.is_empty() {
        s.bibliotheque.enregistrer_lot(&mut modifiees)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Séries
// ---------------------------------------------------------------------------

fn premier_numero(e: &Episode) -> Option<(i32, i32)> {
    e.numeros.first().map(|n| (n.saison, n.episode))
}

/// Complète un épisode nouvellement détecté avec son NFO.
pub fn completer_episode_depuis_nfo(ep: &mut Episode) {
    let nfos: Vec<PathBuf> = ep
        .fichiers
        .iter()
        .filter(|f| f.type_fichier == TypeFichier::Nfo)
        .map(|f| f.chemin_complet())
        .collect();
    for chemin in nfos {
        let Ok(lus) = lire_fichier_nfo_episodes(&chemin) else {
            continue;
        };
        let cible = premier_numero(ep);
        let lu = lus
            .episodes
            .iter()
            .find(|l| cible.is_some() && premier_numero(&l.episode) == cible)
            .or_else(|| lus.episodes.first());
        let Some(lu) = lu else { continue };
        let mut n = lu.episode.clone();
        n.id = String::new();
        n.id_serie = ep.id_serie.clone();
        n.fichiers = std::mem::take(&mut ep.fichiers);
        if n.numeros.is_empty() {
            n.numeros = ep.numeros.clone();
        }
        n.est_disque = ep.est_disque;
        n.empile = ep.empile;
        n.multi_episodes = ep.multi_episodes;
        n.nom_fichier_origine = ep.nom_fichier_origine.clone();
        if n.titre.trim().is_empty() {
            n.titre = ep.titre.clone();
        }
        if n.source_media == SourceMedia::Inconnue {
            n.source_media = ep.source_media;
        }
        if n.edition == Edition::Aucune {
            n.edition = ep.edition.clone();
        }
        *ep = n;
        return;
    }
}

/// Complète une série nouvellement détectée avec son `tvshow.nfo`.
pub fn completer_serie_depuis_nfo(serie: &mut Serie) {
    let nfos: Vec<PathBuf> = serie
        .fichiers
        .iter()
        .filter(|f| f.type_fichier == TypeFichier::Nfo)
        .map(|f| f.chemin_complet())
        .collect();
    for chemin in nfos {
        let Ok(lu) = lire_fichier_nfo_serie(&chemin) else {
            continue;
        };
        if !lu.valide && lu.serie.ids.est_vide() {
            continue;
        }
        let mut n = lu.serie;
        n.id = String::new();
        n.chemin = serie.chemin.clone();
        n.source_donnees = serie.source_donnees.clone();
        n.fichiers = std::mem::take(&mut serie.fichiers);
        n.episodes = std::mem::take(&mut serie.episodes);
        n.saisons = std::mem::take(&mut serie.saisons);
        if n.titre.trim().is_empty() {
            n.titre = serie.titre.clone();
        }
        if n.annee == 0 {
            n.annee = serie.annee;
        }
        for (k, v) in serie.ids.iter() {
            n.ids.definir_si_absent(k, &v.en_texte());
        }
        *serie = n;
        return;
    }
}

fn videos_episode(e: &Episode) -> BTreeSet<PathBuf> {
    e.fichiers
        .iter()
        .filter(|f| f.type_fichier == TypeFichier::Video)
        .map(|f| f.chemin_complet())
        .collect()
}

/// Fusionne les épisodes scannés d'une série avec ceux déjà connus :
/// métadonnées conservées pour les épisodes retrouvés (même vidéo, à défaut
/// même numéro), NFO lus pour les nouveaux, épisodes disparus retirés.
pub fn fusionner_episodes(existants: &[Episode], scannes: Vec<Episode>) -> Vec<Episode> {
    let mut pris = vec![false; existants.len()];
    let mut sortie = Vec::new();
    for mut s in scannes {
        let videos = videos_episode(&s);
        let trouve = (0..existants.len())
            .find(|&i| !pris[i] && !videos_episode(&existants[i]).is_disjoint(&videos))
            .or_else(|| {
                (0..existants.len()).find(|&i| {
                    !pris[i]
                        && premier_numero(&existants[i]).is_some()
                        && premier_numero(&existants[i]) == premier_numero(&s)
                })
            });
        match trouve {
            Some(i) => {
                pris[i] = true;
                let mut m = existants[i].clone();
                m.fichiers = fusionner_fichiers(&m.fichiers, &s.fichiers);
                if m.numeros.is_empty() {
                    m.numeros = s.numeros;
                }
                m.est_disque = s.est_disque;
                m.empile = s.empile;
                m.multi_episodes = s.multi_episodes;
                sortie.push(m);
            }
            None => {
                completer_episode_depuis_nfo(&mut s);
                sortie.push(s);
            }
        }
    }
    sortie
}

/// Fusionne le scan d'une source de séries avec les séries existantes.
pub fn fusionner_series(existants: &[Serie], scannees: Vec<Serie>) -> Fusion<Serie> {
    let mut f = Fusion::default();
    let mut pris = vec![false; existants.len()];
    for mut s in scannees {
        match (0..existants.len()).find(|&i| !pris[i] && existants[i].chemin == s.chemin) {
            Some(i) => {
                pris[i] = true;
                let e = &existants[i];
                if e.verrouille {
                    f.verrouilles += 1;
                    continue;
                }
                let mut m = e.clone();
                m.fichiers = fusionner_fichiers(&e.fichiers, &s.fichiers);
                m.source_donnees = s.source_donnees.clone();
                m.episodes = fusionner_episodes(&e.episodes, std::mem::take(&mut s.episodes));
                trier_episodes(&mut m);
                recalculer_saisons(&mut m);
                // Les titres de saison déjà connus sont conservés.
                for saison in m.saisons.iter_mut() {
                    if let Some(a) = e.saisons.iter().find(|a| a.numero == saison.numero) {
                        if saison.titre.is_empty() {
                            saison.titre = a.titre.clone();
                        }
                        if saison.resume.is_empty() {
                            saison.resume = a.resume.clone();
                        }
                    }
                }
                if m != *e {
                    f.modifies.push(m.id.clone());
                    f.a_enregistrer.push(m);
                }
            }
            None => {
                completer_serie_depuis_nfo(&mut s);
                let episodes = std::mem::take(&mut s.episodes);
                s.episodes = fusionner_episodes(&[], episodes);
                trier_episodes(&mut s);
                recalculer_saisons(&mut s);
                s.id = nouvel_id();
                f.nouveaux.push(s.id.clone());
                f.a_enregistrer.push(s);
            }
        }
    }
    for (i, e) in existants.iter().enumerate() {
        if pris[i] {
            continue;
        }
        if e.verrouille {
            f.verrouilles += 1;
        } else {
            f.disparus.push(e.id.clone());
        }
    }
    f
}

/// Tâche : mise à jour de toutes les sources de séries.
pub(crate) fn mettre_a_jour_series(n: &Noyau, ctx: &ContexteTache) -> Resultat<RapportMiseAJour> {
    let reglages = n.reglages();
    let sources = reglages.series.sources.clone();
    let opts = n.config.mise_a_jour.clone();
    let mut rapport = RapportMiseAJour::default();
    let mut disparues: BTreeSet<String> = lire_marques(&n.verrou(), CLE_SERIES_DISPARUES)?
        .into_iter()
        .collect();
    let total = sources.len() as u64;

    for (i, source) in sources.iter().enumerate() {
        ctx.verifier_annulation()?;
        ctx.progression(i as u64, total, format!("Analyse de {}", source.display()));
        let options = options_scan(
            &reglages.series.dossiers_ignores,
            reglages.series.ignorer_dossiers_nomedia,
            &reglages.series.mots_exclus,
            &reglages,
            source,
            &sources,
        );
        let scan = match scanner_source_series(source, &options) {
            Ok(r) => r,
            Err(e) => {
                ctx.message(NiveauMessage::Avertissement, e.to_string());
                rapport
                    .sources_indisponibles
                    .push((source.clone(), e.to_string()));
                continue;
            }
        };
        for a in &scan.avertissements {
            rapport.avertissements.push(format!("{a:?}"));
        }
        ctx.verifier_annulation()?;

        let mut s = n.verrou();
        let existants: Vec<Serie> = s
            .bibliotheque
            .lister::<Serie>()?
            .into_iter()
            .filter(|x| source_de(&x.chemin, &sources) == Some(source))
            .collect();
        let ids_source: BTreeSet<String> = existants.iter().map(|x| x.id.clone()).collect();
        let mut fusion = fusionner_series(&existants, scan.series);
        let supprimes = match opts.disparus {
            TraitementDisparus::Supprimer => fusion.disparus.clone(),
            TraitementDisparus::Marquer => Vec::new(),
        };
        enregistrer_par_lots(
            &mut s,
            &mut fusion.a_enregistrer,
            &supprimes,
            opts.taille_lot,
        )?;
        disparues.retain(|id| !ids_source.contains(id));
        if opts.disparus == TraitementDisparus::Marquer {
            disparues.extend(fusion.disparus.iter().cloned());
            rapport
                .marques_disparus
                .extend(fusion.disparus.iter().cloned());
        }
        drop(s);

        for id in &fusion.nouveaux {
            ctx.emettre(Evenement::SerieAjoutee { id: id.clone() });
        }
        for id in &fusion.modifies {
            ctx.emettre(Evenement::SerieModifiee { id: id.clone() });
        }
        for id in &supprimes {
            ctx.emettre(Evenement::SerieSupprimee { id: id.clone() });
        }
        rapport.sources_traitees.push(source.clone());
        rapport.ajoutes.extend(fusion.nouveaux);
        rapport.modifies.extend(fusion.modifies);
        rapport.supprimes.extend(supprimes);
        rapport.verrouilles += fusion.verrouilles;
    }

    let mut s = n.verrou();
    ecrire_marques(
        &mut s,
        CLE_SERIES_NOUVELLES,
        &rapport.ajoutes.iter().cloned().collect(),
    )?;
    ecrire_marques(&mut s, CLE_SERIES_DISPARUES, &disparues)?;
    drop(s);
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn film(dossier: &str, video: &str) -> Film {
        let chemin = PathBuf::from(dossier);
        Film {
            titre: video.into(),
            chemin: chemin.clone(),
            fichiers: vec![FichierMedia::nouveau(
                &chemin.join(video),
                TypeFichier::Video,
            )],
            ..Film::default()
        }
    }

    #[test]
    fn fusion_conserve_les_metadonnees_et_detecte_les_disparus() {
        let mut a = film("/s/A", "A.mkv");
        a.id = "a".into();
        a.resume = "garder".into();
        let mut b = film("/s/B", "B.mkv");
        b.id = "b".into();
        let mut c = film("/s/C", "C.mkv");
        c.id = "c".into();
        c.verrouille = true;
        let scannes = vec![film("/s/A", "A-renomme.mkv"), film("/s/D", "D.mkv")];
        let f = fusionner_films(&[a, b, c], scannes);
        assert_eq!(f.modifies, vec!["a".to_string()]);
        assert_eq!(f.nouveaux.len(), 1);
        assert_eq!(f.disparus, vec!["b".to_string()]);
        assert_eq!(f.verrouilles, 1);
        let a2 = f.a_enregistrer.iter().find(|x| x.id == "a").unwrap();
        assert_eq!(a2.resume, "garder");
        assert_eq!(a2.fichiers[0].nom, "A-renomme.mkv");
    }

    #[test]
    fn dossier_multi_films_departage_par_nom_de_video() {
        let mut x = film("/s/M", "X.mkv");
        x.id = "x".into();
        x.dossier_multi_films = true;
        let mut y = film("/s/M", "Y.mkv");
        y.id = "y".into();
        y.dossier_multi_films = true;
        let mut sx = film("/s/M", "X.mkv");
        sx.dossier_multi_films = true;
        let mut sy = film("/s/M", "Y.mkv");
        sy.dossier_multi_films = true;
        let f = fusionner_films(&[x, y], vec![sy, sx]);
        assert!(f.nouveaux.is_empty());
        assert!(f.disparus.is_empty());
    }

    #[test]
    fn source_la_plus_profonde() {
        let sources = vec![PathBuf::from("/a"), PathBuf::from("/a/b")];
        assert_eq!(
            source_de(Path::new("/a/b/film"), &sources),
            Some(&PathBuf::from("/a/b"))
        );
        assert_eq!(
            source_de(Path::new("/a/c"), &sources),
            Some(&PathBuf::from("/a"))
        );
        assert_eq!(source_de(Path::new("/z"), &sources), None);
    }
}
