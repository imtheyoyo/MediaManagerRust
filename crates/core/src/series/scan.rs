//! Scan d'une source de séries.
//!
//! Structure attendue :
//!
//! ```text
//! <source>/
//!   <Série>/                 tvshow.nfo, poster.jpg, season01-poster.jpg…
//!     Season 01/             dossier de saison (optionnel)
//!       Série - S01E02.mkv   (+ .nfo, -thumb.jpg, .fr.srt…)
//!     Série - S01E01.mkv     épisodes à plat acceptés
//!     S01E03/VIDEO_TS/       disque : un épisode
//!   A/ B/                    dossier d'un seul caractère : ses sous-dossiers sont des séries
//! ```
//!
//! Réutilise le scan des films pour les règles d'exclusion, le typage des
//! fichiers, l'empilement et l'extraction titre / année / identifiants.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use walkdir::WalkDir;

use super::detection::{
    ResultatDetection, detecter_episodes, detecter_saison_fichier_dossier, extraire_titre,
};
use super::organisation::{recalculer_saisons, trier_episodes};
use crate::modele::fichier::nom_de_base;
use crate::modele::{
    Edition, Episode, FichierMedia, GroupeEpisodes, NumeroEpisode, Saison, Serie, SourceMedia,
    TypeFichier,
};
use crate::scan::ignore::{MotifExclusion, contient_marqueur, est_ignore};
use crate::scan::pile::{detecter_empilement, nettoyer_marqueur_pile};
use crate::scan::regroupement::creer_fichier;
use crate::scan::titre::{detecter_titre_et_annee_avec, extraire_ids};
use crate::scan::types::{est_fichier_identifiant_disque, est_nom_dossier_disque};
use crate::scan::{ErreurScan, OptionsScan};

/// Problème non bloquant rencontré pendant le scan d'une source de séries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AvertissementSeries {
    /// Fichier vidéo posé directement à la racine de la source (ignoré).
    EpisodeALaRacine(PathBuf),
    /// Vidéo dont ni la saison ni l'épisode n'ont pu être détectés (l'épisode
    /// est créé « non catégorisé »).
    EpisodeNonDetecte(PathBuf),
    /// Erreur de lecture (droits, boucle de liens…).
    Lecture {
        chemin: Option<PathBuf>,
        message: String,
    },
}

/// Résultat du scan d'une source de séries.
#[derive(Debug, Clone, Default)]
pub struct ResultatScanSeries {
    /// Séries détectées, triées par dossier.
    pub series: Vec<Serie>,
    pub avertissements: Vec<AvertissementSeries>,
}

/// Fichiers d'un dossier de série.
#[derive(Default)]
struct ArbreSerie {
    fichiers: Vec<PathBuf>,
    /// `(dossier de l'épisode, dossier de structure VIDEO_TS/BDMV/HVDVD_TS)`.
    disques: Vec<(PathBuf, PathBuf)>,
    avertissements: Vec<AvertissementSeries>,
}

fn nom_de(chemin: &Path) -> String {
    chemin
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Chemin relatif au dossier de la série, séparateurs `/`.
fn relatif(chemin: &Path, dossier_serie: &Path) -> String {
    chemin
        .strip_prefix(dossier_serie)
        .unwrap_or(chemin)
        .to_string_lossy()
        .replace('\\', "/")
}

fn motifs_exclusion(options: &OptionsScan) -> Vec<MotifExclusion> {
    options
        .dossiers_ignores
        .iter()
        .map(|m| MotifExclusion::nouveau(m))
        .collect()
}

fn dossier_exclu(chemin: &Path, options: &OptionsScan, motifs: &[MotifExclusion]) -> bool {
    est_ignore(chemin, motifs)
        || options.sources_exclues.iter().any(|s| s == chemin)
        || contient_marqueur(chemin, options.ignorer_nomedia)
}

fn explorer_serie(dossier: &Path, options: &OptionsScan, motifs: &[MotifExclusion]) -> ArbreSerie {
    let mut arbre = ArbreSerie::default();
    let mut it = WalkDir::new(dossier)
        .follow_links(true)
        .sort_by_file_name()
        .into_iter();
    while let Some(entree) = it.next() {
        let e = match entree {
            Ok(e) => e,
            Err(err) => {
                arbre.avertissements.push(AvertissementSeries::Lecture {
                    chemin: err.path().map(Path::to_path_buf),
                    message: err.to_string(),
                });
                continue;
            }
        };
        let chemin = e.path();
        if e.depth() == 0 {
            if contient_marqueur(chemin, options.ignorer_nomedia) {
                return arbre;
            }
            continue;
        }
        if e.file_type().is_dir() {
            if dossier_exclu(chemin, options, motifs) {
                it.skip_current_dir();
                continue;
            }
            if est_nom_dossier_disque(&nom_de(chemin)) {
                // Structure de disque : un épisode, sans descente.
                it.skip_current_dir();
                let parent = chemin.parent().unwrap_or(dossier).to_path_buf();
                arbre.disques.push((parent, chemin.to_path_buf()));
            }
        } else if e.file_type().is_file() && !est_ignore(chemin, &[]) {
            arbre.fichiers.push(chemin.to_path_buf());
        }
    }
    arbre
}

// ---------------------------------------------------------------------------
// Éditions d'épisode
// ---------------------------------------------------------------------------

static EDITIONS_EPISODE: LazyLock<Vec<(Edition, Regex)>> = LazyLock::new(|| {
    [
        (
            Edition::DirectorsCut,
            r"(?i).Director.?s.(Cut|Edition|Version)",
        ),
        (Edition::NonCoupee, r"(?i).Uncut.(Cut|Edition|Version)?"),
        (
            Edition::Remasterisee,
            r"(?i).Remastered.(Cut|Edition|Version)?",
        ),
    ]
    .into_iter()
    .map(|(e, m)| (e, Regex::new(m).unwrap()))
    .collect()
});

/// Édition d'un épisode d'après son nom de fichier (Director's Cut, Uncut,
/// Remastered ; aucune sinon).
pub fn edition_episode(nom_fichier: &str) -> Edition {
    EDITIONS_EPISODE
        .iter()
        .find(|(_, re)| re.is_match(nom_fichier))
        .map_or(Edition::Aucune, |(e, _)| e.clone())
}

// ---------------------------------------------------------------------------
// Construction des épisodes
// ---------------------------------------------------------------------------

/// Vidéos d'un même épisode (parties empilées comprises) et leurs annexes.
struct GroupeVideo {
    /// Dossier relatif à la série (sans `/` final).
    dossier_relatif: String,
    /// Noms de base comparables (minuscules, avec et sans marqueur de pile).
    bases: Vec<String>,
    videos: Vec<FichierMedia>,
    annexes: Vec<FichierMedia>,
    est_disque: bool,
    /// Chemin servant à la détection (fichier vidéo ou dossier de l'épisode).
    chemin_detection: String,
}

/// Le nom de base `base` d'un fichier annexe commence par le nom `video`
/// suivi d'un séparateur (`.fr`, `-thumb`, `.skiptro`…) ou lui est égal.
fn prefixe_de(base: &str, video: &str) -> bool {
    base == video || (base.starts_with(video) && base[video.len()..].starts_with(['.', '-', '_']))
}

/// Types d'images rattachées à un épisode : poster ou image non classée
/// voisine d'une vidéo deviennent la vignette de l'épisode.
fn adapter_type_annexe(f: &mut FichierMedia) {
    if matches!(f.type_fichier, TypeFichier::Poster | TypeFichier::Graphic) {
        f.type_fichier = TypeFichier::Thumb;
    }
}

/// Fichier vidéo représentant un disque : fichier identifiant
/// (`VIDEO_TS.IFO`, `index.bdmv`, `HV000I01.IFO`) ou, à défaut, le dossier.
fn fichier_disque(dossier_disque: &Path) -> FichierMedia {
    if let Ok(entrees) = std::fs::read_dir(dossier_disque) {
        let mut identifiants: Vec<PathBuf> = entrees
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && est_fichier_identifiant_disque(&nom_de(p)))
            .collect();
        identifiants.sort();
        if let Some(p) = identifiants.first() {
            let mut f = creer_fichier(p, dossier_disque);
            f.type_fichier = TypeFichier::Video;
            return f;
        }
    }
    FichierMedia::nouveau(dossier_disque, TypeFichier::Video)
}

/// Applique la règle d'empilement : avec au moins deux vidéos dans le
/// groupe, chaque fichier reçoit numéro et marqueur de pile.
fn appliquer_empilement(groupe: &mut GroupeVideo) -> bool {
    if groupe.est_disque || groupe.videos.len() < 2 {
        return false;
    }
    let mut empile = false;
    for f in groupe.videos.iter_mut().chain(groupe.annexes.iter_mut()) {
        if matches!(
            f.type_fichier,
            TypeFichier::Video
                | TypeFichier::Audio
                | TypeFichier::SousTitres
                | TypeFichier::MediaInfo
                | TypeFichier::Vsmeta
                | TypeFichier::Skiptro
                | TypeFichier::Scenepass
        ) {
            let (n, m) = detecter_empilement(&f.nom, &nom_de(&f.chemin));
            f.empilement = n;
            f.marqueur_empilement = m;
            empile |= n > 0;
        }
    }
    empile
}

/// Crée les épisodes (un par numéro détecté) d'un groupe de vidéos.
fn episodes_du_groupe(
    groupe: &GroupeVideo,
    detection: &ResultatDetection,
    empile: bool,
) -> Vec<Episode> {
    let premiere = &groupe.videos[0];
    let titre = detection.nom_nettoye.clone();
    let mut fichiers: Vec<FichierMedia> = groupe
        .videos
        .iter()
        .chain(groupe.annexes.iter())
        .cloned()
        .collect();
    fichiers.sort_by(|a, b| {
        a.type_fichier
            .cmp(&b.type_fichier)
            .then_with(|| a.nom.cmp(&b.nom))
    });
    let numeros: Vec<i32> = if detection.episodes.is_empty() {
        vec![-1]
    } else {
        detection.episodes.clone()
    };
    let multi = numeros.len() > 1;
    numeros
        .into_iter()
        .map(|n| Episode {
            numeros: vec![NumeroEpisode {
                groupe: GroupeEpisodes::diffusion(),
                saison: detection.saison,
                episode: n,
            }],
            titre: titre.clone(),
            premiere_diffusion: detection.date.clone(),
            source_media: SourceMedia::depuis_chemin(&groupe.chemin_detection),
            edition: edition_episode(&premiere.nom),
            est_disque: groupe.est_disque,
            multi_episodes: multi,
            empile,
            nom_fichier_origine: if groupe.est_disque {
                nom_de(&premiere.chemin)
            } else {
                premiere.nom.clone()
            },
            fichiers: fichiers.clone(),
            ..Episode::default()
        })
        .collect()
}

/// Rattache un fichier resté seul (artwork de saison, fichier de série ou
/// annexe d'un épisode reconnu par son nom).
fn rattacher_restant(
    mut f: FichierMedia,
    rel_dossier: &str,
    serie: &mut Serie,
    saisons_art: &mut BTreeMap<i32, Vec<FichierMedia>>,
) {
    use TypeFichier::*;
    let base_min = nom_de_base(&f.nom).to_lowercase();
    let hors_racine = !rel_dossier.is_empty();
    let nom_dossier = rel_dossier.rsplit('/').next().unwrap_or("");

    // Artworks de série placés dans un sous-dossier, ou nommés `season…`.
    if matches!(f.type_fichier, Poster | Fanart | Banner)
        && (hors_racine || base_min.starts_with("season"))
    {
        f.type_fichier = match f.type_fichier {
            Poster => SaisonPoster,
            Fanart => SaisonFanart,
            _ => SaisonBanner,
        };
    } else if f.type_fichier == Thumb
        && hors_racine
        && (base_min == "thumb" || base_min == "landscape")
    {
        f.type_fichier = SaisonThumb;
    }
    if matches!(
        f.type_fichier,
        SaisonPoster | SaisonFanart | SaisonBanner | SaisonThumb
    ) {
        match detecter_saison_fichier_dossier(&f.nom, nom_dossier) {
            Some(n) if n >= 0 => saisons_art.entry(n).or_default().push(f),
            _ => serie.fichiers.push(f),
        }
        return;
    }

    // Annexe d'un épisode reconnu par son nom (sous-titres nommés autrement…).
    if !(f.type_fichier == Nfo && base_min == "tvshow") {
        let rel = if hors_racine {
            format!("{rel_dossier}/{}", f.nom)
        } else {
            f.nom.clone()
        };
        let d = detecter_episodes(&rel, Some(&serie.titre));
        if let Some(&ep) = d.episodes.first() {
            let groupe = GroupeEpisodes::diffusion();
            let cibles: Vec<usize> = serie
                .episodes
                .iter()
                .enumerate()
                .filter(|(_, e)| e.saison(&groupe) == d.saison && e.episode(&groupe) == ep)
                .map(|(i, _)| i)
                .collect();
            if !cibles.is_empty() {
                // Un multi-épisodes partage ses fichiers : on ajoute à tous les
                // épisodes issus de la même vidéo.
                let video = serie.episodes[cibles[0]].nom_fichier_origine.clone();
                adapter_type_annexe(&mut f);
                for e in serie
                    .episodes
                    .iter_mut()
                    .filter(|e| e.nom_fichier_origine == video)
                {
                    if !e.fichiers.contains(&f) {
                        e.fichiers.push(f.clone());
                    }
                }
                return;
            }
        }
    }
    serie.fichiers.push(f);
}

/// Scanne le dossier d'une série. Renvoie `None` si le dossier ne contient
/// aucune vidéo (ou est exclu). `source` : racine de la source de données.
pub fn scanner_dossier_serie(
    dossier: &Path,
    source: &Path,
    options: &OptionsScan,
) -> (Option<Serie>, Vec<AvertissementSeries>) {
    let motifs = motifs_exclusion(options);
    let arbre = explorer_serie(dossier, options, &motifs);
    let mut avertissements = arbre.avertissements;
    // Racine de typage : le parent de la série, pour que le premier composant
    // du chemin relatif soit le dossier de la série (règle des extras).
    let racine_typage = dossier.parent().unwrap_or(dossier);

    let mut videos: Vec<FichierMedia> = Vec::new();
    let mut autres: Vec<FichierMedia> = Vec::new();
    for p in &arbre.fichiers {
        let f = creer_fichier(p, racine_typage);
        if f.type_fichier == TypeFichier::Video {
            videos.push(f);
        } else {
            autres.push(f);
        }
    }
    if videos.is_empty() && arbre.disques.is_empty() {
        return (None, avertissements);
    }

    // Série : titre, année et identifiants depuis le nom du dossier.
    let nom_dossier = nom_de(dossier);
    let ta = detecter_titre_et_annee_avec(
        &nom_dossier,
        &options.mots_interdits,
        &options.prefixes_titre,
    );
    let mut serie = Serie {
        source_donnees: source.to_path_buf(),
        chemin: dossier.to_path_buf(),
        titre: if ta.titre.is_empty() {
            nom_dossier.clone()
        } else {
            ta.titre
        },
        annee: ta.annee,
        groupe_actif: GroupeEpisodes::diffusion(),
        ..Serie::default()
    };
    let ids = extraire_ids(&nom_dossier);
    if let Some(imdb) = ids.imdb {
        serie.ids.definir("imdb", &imdb);
    }
    if let Some(tmdb) = ids.tmdb {
        serie.ids.definir("tmdb", &tmdb.to_string());
    }
    if let Some(tvdb) = ids.tvdb {
        serie.ids.definir("tvdb", &tvdb.to_string());
    }

    // Groupes de vidéos : même dossier et même nom sans marqueur de pile.
    let mut groupes: Vec<GroupeVideo> = Vec::new();
    let mut index: BTreeMap<(String, String), usize> = BTreeMap::new();
    videos.sort_by_key(FichierMedia::chemin_complet);
    for v in videos {
        let rel_dossier = relatif(&v.chemin, dossier);
        let base = v.nom_de_base().to_lowercase();
        let sans_pile = nettoyer_marqueur_pile(&base);
        let cle = (rel_dossier.clone(), sans_pile.clone());
        match index.get(&cle) {
            Some(&i) => {
                groupes[i].bases.push(base);
                groupes[i].videos.push(v);
            }
            None => {
                index.insert(cle, groupes.len());
                let chemin_detection = relatif(&v.chemin_complet(), dossier);
                groupes.push(GroupeVideo {
                    dossier_relatif: rel_dossier,
                    bases: vec![base, sans_pile],
                    videos: vec![v],
                    annexes: Vec::new(),
                    est_disque: false,
                    chemin_detection,
                });
            }
        }
    }
    for (dossier_episode, dossier_disque) in &arbre.disques {
        groupes.push(GroupeVideo {
            dossier_relatif: relatif(dossier_episode, dossier),
            bases: Vec::new(),
            videos: vec![fichier_disque(dossier_disque)],
            annexes: Vec::new(),
            est_disque: true,
            chemin_detection: relatif(dossier_disque, dossier),
        });
    }

    // Annexes : même dossier, nom commençant par celui de la vidéo (le plus
    // long l'emporte) ; sinon fichiers du dossier d'un épisode disque.
    let mut restants: Vec<(FichierMedia, String)> = Vec::new();
    for mut f in autres {
        let rel_dossier = relatif(&f.chemin, dossier);
        let base = f.nom_de_base().to_lowercase();
        let meilleur = groupes
            .iter()
            .enumerate()
            .filter(|(_, g)| !g.est_disque && g.dossier_relatif == rel_dossier)
            .filter_map(|(i, g)| {
                g.bases
                    .iter()
                    .filter(|b| prefixe_de(&base, b))
                    .map(|b| b.len())
                    .max()
                    .map(|l| (l, i))
            })
            .max();
        let disque = || {
            groupes.iter().position(|g| {
                g.est_disque && !rel_dossier.is_empty() && g.dossier_relatif == rel_dossier
            })
        };
        let est_annexe_saison = matches!(
            f.type_fichier,
            TypeFichier::SaisonPoster
                | TypeFichier::SaisonFanart
                | TypeFichier::SaisonBanner
                | TypeFichier::SaisonThumb
        );
        if let Some((_, i)) = meilleur {
            adapter_type_annexe(&mut f);
            groupes[i].annexes.push(f);
        } else if let (false, Some(i)) = (est_annexe_saison, disque()) {
            adapter_type_annexe(&mut f);
            groupes[i].annexes.push(f);
        } else {
            restants.push((f, rel_dossier));
        }
    }

    // Épisodes.
    let mut empiles: BTreeMap<(String, i32, Vec<i32>), usize> = BTreeMap::new();
    for mut groupe in groupes {
        let empile = appliquer_empilement(&mut groupe);
        let d = detecter_episodes(&groupe.chemin_detection, Some(&serie.titre));
        if d.episodes.is_empty() && d.date.is_none() {
            avertissements.push(AvertissementSeries::EpisodeNonDetecte(
                groupe.videos[0].chemin_complet(),
            ));
        }
        // Partie empilée d'un épisode déjà créé (noms différents) : fusion.
        let cle = (groupe.dossier_relatif.clone(), d.saison, d.episodes.clone());
        if d.marqueur_pile_trouve && !d.episodes.is_empty() {
            if let Some(&i) = empiles.get(&cle) {
                let video = serie.episodes[i].nom_fichier_origine.clone();
                for e in serie
                    .episodes
                    .iter_mut()
                    .filter(|e| e.nom_fichier_origine == video)
                {
                    e.empile = true;
                    e.fichiers
                        .extend(groupe.videos.iter().chain(groupe.annexes.iter()).cloned());
                }
                continue;
            }
            empiles.insert(cle, serie.episodes.len());
        }
        serie
            .episodes
            .extend(episodes_du_groupe(&groupe, &d, empile));
    }

    // Fichiers restants : artworks de saison, fichiers de série.
    let mut saisons_art: BTreeMap<i32, Vec<FichierMedia>> = BTreeMap::new();
    for (f, rel_dossier) in restants {
        rattacher_restant(f, &rel_dossier, &mut serie, &mut saisons_art);
    }
    serie.saisons = saisons_art
        .into_iter()
        .map(|(numero, mut fichiers)| {
            fichiers.sort();
            Saison {
                numero,
                fichiers,
                ..Saison::default()
            }
        })
        .collect();
    serie.fichiers.sort_by(|a, b| {
        a.type_fichier
            .cmp(&b.type_fichier)
            .then_with(|| a.nom.cmp(&b.nom))
    });
    recalculer_saisons(&mut serie);
    trier_episodes(&mut serie);
    (Some(serie), avertissements)
}

/// Scanne une source de séries : chaque sous-dossier est une série (les
/// dossiers d'un seul caractère `A/`, `B/`… sont des regroupements dont les
/// sous-dossiers sont des séries). Les vidéos posées à la racine sont
/// ignorées avec un avertissement.
pub fn scanner_source_series(
    racine: &Path,
    options: &OptionsScan,
) -> Result<ResultatScanSeries, ErreurScan> {
    if !racine.exists() {
        return Err(ErreurScan::SourceIntrouvable(racine.to_path_buf()));
    }
    if !racine.is_dir() {
        return Err(ErreurScan::PasUnDossier(racine.to_path_buf()));
    }
    if !cfg!(windows)
        && std::fs::read_dir(racine)
            .map(|mut d| d.next().is_none())
            .unwrap_or(true)
    {
        return Err(ErreurScan::SourceVide(racine.to_path_buf()));
    }
    let mut resultat = ResultatScanSeries::default();
    if contient_marqueur(racine, options.ignorer_nomedia) {
        return Ok(resultat);
    }
    let motifs = motifs_exclusion(options);

    let lister = |d: &Path, resultat: &mut ResultatScanSeries| -> Vec<PathBuf> {
        match std::fs::read_dir(d) {
            Ok(entrees) => {
                let mut v: Vec<PathBuf> = entrees.flatten().map(|e| e.path()).collect();
                v.sort();
                v
            }
            Err(err) => {
                resultat.avertissements.push(AvertissementSeries::Lecture {
                    chemin: Some(d.to_path_buf()),
                    message: err.to_string(),
                });
                Vec::new()
            }
        }
    };

    let mut dossiers_series: BTreeSet<PathBuf> = BTreeSet::new();
    for p in lister(racine, &mut resultat) {
        if p.is_file() {
            if !est_ignore(&p, &[]) && creer_fichier(&p, racine).type_fichier == TypeFichier::Video
            {
                resultat
                    .avertissements
                    .push(AvertissementSeries::EpisodeALaRacine(p));
            }
            continue;
        }
        if !p.is_dir() || dossier_exclu(&p, options, &motifs) {
            continue;
        }
        if nom_de(&p).chars().count() == 1 {
            for sous in lister(&p, &mut resultat) {
                if sous.is_dir() && !dossier_exclu(&sous, options, &motifs) {
                    dossiers_series.insert(sous);
                }
            }
        } else {
            dossiers_series.insert(p);
        }
    }

    for d in dossiers_series {
        let (serie, avert) = scanner_dossier_serie(&d, racine, options);
        resultat.avertissements.extend(avert);
        if let Some(s) = serie {
            resultat.series.push(s);
        }
    }
    resultat.series.sort_by(|a, b| a.chemin.cmp(&b.chemin));
    Ok(resultat)
}

/// Titre d'épisode extrait d'un nom de fichier (vide si rien d'exploitable),
/// sans repli sur le nom d'origine (voir aussi
/// [`nettoyer_titre_episode`](super::detection::nettoyer_titre_episode)).
pub fn titre_depuis_nom(nom_fichier: &str, nom_serie: Option<&str>) -> String {
    extraire_titre(nom_fichier, nom_serie)
}
