//! Construction des films à partir des fichiers d'un dossier : dossier par
//! film, fichiers à plat (multi-films), disques.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use super::OptionsScan;
use super::pile::{detecter_empilement, nettoyer_marqueur_pile, nettoyer_marqueurs_pile};
use super::titre::{
    contient_3d, detecter_titre_et_annee_avec, extraire_ids, retirer_titre_triable,
};
use super::types::{type_fichier, type_image};
use crate::modele::fichier::comparer_fichiers_film;
use crate::modele::{Edition, FichierMedia, Film, SourceMedia, TypeFichier};

/// Taille maximale des fichiers texte analysés (NFO, `.txt`).
const TAILLE_MAX_TEXTE: u64 = 1024 * 1024;

static RE_BDINFO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Disc Title:\s+(.*?)[\n\r]").unwrap());
static RE_DISQUE_NOM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"di:name>(.*?)<").unwrap());
static RE_TMDB_FILM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"themoviedb\.org/movie/(\d+)").unwrap());
static RE_IMDB_TEXTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"tt\d{6,}").unwrap());

/// Chemin relatif à la racine, séparateurs normalisés en `/`.
pub(crate) fn relatif(chemin: &Path, racine: &Path) -> String {
    chemin
        .strip_prefix(racine)
        .unwrap_or(chemin)
        .to_string_lossy()
        .replace('\\', "/")
}

fn nom_de(chemin: &Path) -> String {
    chemin
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Crée le [`FichierMedia`] d'un chemin (type déduit, taille et date lues).
pub(crate) fn creer_fichier(chemin: &Path, racine: &Path) -> FichierMedia {
    let mut f = FichierMedia::nouveau(chemin, type_fichier(chemin, racine));
    if let Ok(m) = std::fs::metadata(chemin) {
        f.taille = m.len();
        f.date_fichier = m
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
    }
    f
}

fn lire_texte(chemin: &Path) -> Option<String> {
    let m = std::fs::metadata(chemin).ok()?;
    if m.len() > TAILLE_MAX_TEXTE {
        return None;
    }
    std::fs::read(chemin)
        .ok()
        .map(|o| String::from_utf8_lossy(&o).into_owned())
}

/// Ré-évalue l'empilement : avec au moins deux vidéos (hors disque), les
/// fichiers de types concernés reçoivent numéro et marqueur ; sinon tout est effacé.
pub fn reevaluer_empilement(film: &mut Film) {
    use TypeFichier::*;
    let nb_videos = film
        .fichiers
        .iter()
        .filter(|f| f.type_fichier == Video)
        .count();
    if nb_videos >= 2 && !film.est_disque {
        for f in film.fichiers.iter_mut() {
            if matches!(
                f.type_fichier,
                Video | Audio | SousTitres | MediaInfo | Vsmeta | Skiptro | Scenepass | DoubleExt
            ) {
                let parent = f
                    .chemin
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let (n, m) = detecter_empilement(&f.nom, &parent);
                f.empilement = n;
                f.marqueur_empilement = m;
            }
        }
        film.empile = film.fichiers.iter().any(|f| f.empilement > 0);
        if !film.empile {
            for f in film.fichiers.iter_mut() {
                f.marqueur_empilement.clear();
            }
        }
    } else {
        film.empile = false;
        for f in film.fichiers.iter_mut() {
            f.empilement = 0;
            f.marqueur_empilement.clear();
        }
    }
}

/// Rattache les fichiers à un film selon les règles d'acceptation par type,
/// puis promeut les images non classées (voir la spécification du scan).
pub fn rattacher_fichiers(film: &mut Film, mut fichiers: Vec<FichierMedia>) {
    use TypeFichier::*;
    fichiers.sort_by(comparer_fichiers_film);

    // Noms de base des vidéos (avec et sans marqueur de pile), en minuscules.
    let mut bases_videos: Vec<String> = Vec::new();
    for f in fichiers.iter().filter(|f| f.type_fichier == Video) {
        let b = f.nom_de_base().to_lowercase();
        let sans = nettoyer_marqueur_pile(&b);
        bases_videos.push(b);
        bases_videos.push(sans);
    }
    let est_base_video = |b: &str| bases_videos.iter().any(|v| v == b);
    let titre_min = film.titre.to_lowercase();

    let mut retenus: Vec<FichierMedia> = Vec::new();
    let mut graphiques: Vec<FichierMedia> = Vec::new();

    // Suffixes de nommage par type d'image : `Film-poster.jpg` appartient au
    // film `Film`. Un préfixe différent ⇒ image d'un autre film (rétrogradée).
    let suffixes = |t: TypeFichier| -> &'static [&'static str] {
        match t {
            Fanart => &["-fanart"],
            Thumb => &["-thumb"],
            Poster => &["-poster"],
            Banner => &["-banner"],
            Clearlogo => &["-clearlogo"],
            Logo => &["-logo"],
            Clearart => &["-clearart"],
            Keyart => &["-keyart"],
            Squareart => &["-squareart"],
            Disc => &["-disc", "-discart"],
            _ => &[],
        }
    };

    for mut f in fichiers {
        let t = f.type_fichier;
        let base = f.nom_de_base().to_lowercase();
        match t {
            Video | Trailer | Extra | Sample | Nfo | Texte | ExtraFanart | ExtraThumb | Audio
            | MediaInfo | Vsmeta | Theme | Characterart | DoubleExt | Bif => retenus.push(f),
            SousTitres => {
                if !f.est_archive() {
                    retenus.push(f);
                }
            }
            Fanart | Thumb | Poster | Banner | Clearlogo | Logo | Clearart | Keyart | Squareart
            | Disc => {
                let chemin_min = f.chemin.to_string_lossy().to_lowercase();
                if t == Fanart && chemin_min.contains("extrafanart") {
                    continue;
                }
                if t == Thumb && chemin_min.contains("extrathumbs") {
                    continue;
                }
                let autre_film = suffixes(t).iter().any(|s| {
                    base.strip_suffix(s)
                        .is_some_and(|prefixe| !est_base_video(prefixe))
                });
                if autre_film {
                    f.type_fichier = Graphic;
                    graphiques.push(f);
                } else {
                    retenus.push(f);
                }
            }
            Skiptro | Scenepass => {
                let nom = f.nom.to_lowercase();
                let sans = [".skiptro.json", ".sp.hash.json", ".sp.json"]
                    .iter()
                    .find_map(|s| nom.strip_suffix(s))
                    .unwrap_or(&nom)
                    .to_string();
                let sans = match sans.rsplit_once('.') {
                    Some((b, e)) if super::extensions::est_video(e) => b.to_string(),
                    _ => sans,
                };
                if est_base_video(&sans) || est_base_video(&nettoyer_marqueur_pile(&sans)) {
                    retenus.push(f);
                }
            }
            Graphic => {
                if est_base_video(&base) || (!titre_min.is_empty() && base == titre_min) {
                    f.type_fichier = Poster;
                    retenus.push(f);
                } else {
                    graphiques.push(f);
                }
            }
            SaisonPoster | SaisonFanart | SaisonBanner | SaisonThumb | VideoExtra | Inconnu => {}
        }
    }

    // Promotion : le premier graphique dont le nom donne le type manquant.
    for manquant in [
        Fanart, Poster, Banner, Clearlogo, Logo, Clearart, Keyart, Disc,
    ] {
        if retenus.iter().any(|f| f.type_fichier == manquant) {
            continue;
        }
        if let Some(pos) = graphiques
            .iter()
            .position(|g| type_image(&g.nom, &g.chemin) == manquant)
        {
            let mut g = graphiques.remove(pos);
            g.type_fichier = manquant;
            retenus.push(g);
        }
    }

    retenus.sort_by(comparer_fichiers_film);
    film.fichiers = retenus;
    reevaluer_empilement(film);
}

/// Complète les identifiants absents depuis des textes (noms, NFO, `.txt`).
fn identifiants_depuis(film: &mut Film, texte: &str) {
    let ids = extraire_ids(texte);
    if let Some(i) = ids.imdb {
        film.ids.definir_si_absent("imdb", &i);
    }
    if let Some(i) = ids.tmdb {
        film.ids.definir_si_absent("tmdb", &i.to_string());
    }
    if let Some(i) = ids.tvdb {
        film.ids.definir_si_absent("tvdb", &i.to_string());
    }
}

/// Lit les NFO et textes du film pour compléter les identifiants manquants.
fn identifiants_depuis_fichiers(film: &mut Film) {
    let chemins: Vec<PathBuf> = film
        .fichiers
        .iter()
        .filter(|f| matches!(f.type_fichier, TypeFichier::Nfo | TypeFichier::Texte))
        .map(|f| f.chemin_complet())
        .collect();
    for c in chemins {
        if film.ids.contient("imdb") && film.ids.contient("tmdb") {
            break;
        }
        if let Some(t) = lire_texte(&c) {
            if let Some(m) = RE_IMDB_TEXTE.find(&t) {
                film.ids.definir_si_absent("imdb", m.as_str());
            }
            if let Some(c) = RE_TMDB_FILM.captures(&t) {
                film.ids.definir_si_absent("tmdb", &c[1]);
            }
        }
    }
}

/// Renseigne les champs communs d'un film nouvellement détecté.
fn finaliser_film(film: &mut Film, racine: &Path, reference: &str) {
    film.source_donnees = racine.to_path_buf();
    if let Some(p) = film.fichier_principal() {
        film.nom_fichier_origine = p.nom.clone();
    }
    film.hors_ligne = film
        .fichiers
        .iter()
        .any(|f| f.type_fichier == TypeFichier::Video && f.extension() == "disc");
    film.video_3d = contient_3d(reference)
        || film
            .fichier_principal()
            .is_some_and(|p| contient_3d(&p.nom));
    identifiants_depuis_fichiers(film);
    film.source_media = SourceMedia::Inconnue;
}

/// Résultat de la construction d'un film depuis un dossier.
pub(crate) enum ResultatDossier {
    Film(Box<Film>),
    /// Le dossier est un dossier de regroupement (lettre, année, genre…).
    Regroupement,
    /// Aucune vidéo exploitable.
    Aucun,
}

/// Film = dossier. `chemins` : tous les fichiers du dossier (récursif).
pub(crate) fn film_depuis_dossier(
    dossier: &Path,
    chemins: &[PathBuf],
    racine: &Path,
    options: &OptionsScan,
    est_disque: bool,
) -> ResultatDossier {
    let nom_dossier = nom_de(dossier);
    if !est_disque && super::ignore::est_dossier_regroupement(&nom_dossier) {
        return ResultatDossier::Regroupement;
    }

    let mut fichiers: Vec<FichierMedia> =
        chemins.iter().map(|c| creer_fichier(c, racine)).collect();

    // Fichiers inconnus dont le nom commence par le nom complet d'une vidéo.
    let noms_videos: Vec<String> = fichiers
        .iter()
        .filter(|f| f.type_fichier == TypeFichier::Video)
        .map(|f| f.nom.to_lowercase())
        .collect();
    for f in fichiers
        .iter_mut()
        .filter(|f| f.type_fichier == TypeFichier::Inconnu)
    {
        let n = f.nom.to_lowercase();
        if noms_videos
            .iter()
            .any(|v| n.len() > v.len() && n.starts_with(v.as_str()))
        {
            f.type_fichier = TypeFichier::DoubleExt;
        }
    }

    // Titres de repli : BDInfo (`.txt`) et métadonnées de disque.
    let mut titre_bdinfo = String::new();
    for f in fichiers
        .iter()
        .filter(|f| f.type_fichier == TypeFichier::Texte)
    {
        if let Some(t) = lire_texte(&f.chemin_complet())
            && let Some(c) = RE_BDINFO.captures(&t)
        {
            titre_bdinfo = c[1].trim().to_string();
            break;
        }
    }
    let mut titre_disque = String::new();
    if est_disque
        && let Some(t) = lire_texte(
            &dossier
                .join("BDMV")
                .join("META")
                .join("DL")
                .join("bdmt_eng.xml"),
        )
        && let Some(c) = RE_DISQUE_NOM.captures(&t)
    {
        titre_disque = c[1].trim().to_string();
    }

    let mut film = Film::nouveau();
    film.chemin = dossier.to_path_buf();
    film.est_disque = est_disque;

    let ta = detecter_titre_et_annee_avec(
        &nom_dossier,
        &options.mots_interdits,
        &options.prefixes_titre,
    );
    film.annee = ta.annee;
    let mut titre = if titre_disque.is_empty() {
        ta.titre
    } else {
        titre_disque
    };
    titre = retirer_titre_triable(&titre, &options.prefixes_titre);
    if titre.trim().is_empty() {
        titre = titre_bdinfo.clone();
    }
    if titre.trim().is_empty() {
        titre = fichiers
            .iter()
            .find(|f| f.type_fichier == TypeFichier::Video)
            .map(|f| f.nom_de_base())
            .unwrap_or_else(|| nom_dossier.clone());
    }
    film.titre = titre;
    film.edition = Edition::depuis_chaine(&nom_dossier);

    // Identifiants : nom du dossier, puis noms de fichiers, puis textes.
    identifiants_depuis(&mut film, &nom_dossier);
    for f in &fichiers {
        identifiants_depuis(&mut film, &f.nom);
    }
    for f in fichiers
        .iter()
        .filter(|f| f.type_fichier == TypeFichier::Texte)
    {
        if let Some(t) = lire_texte(&f.chemin_complet())
            && let Some(m) = RE_IMDB_TEXTE.find(&t)
        {
            film.ids.definir_si_absent("imdb", m.as_str());
        }
    }

    rattacher_fichiers(&mut film, fichiers);
    if !film.a_fichier_video() {
        return ResultatDossier::Aucun;
    }
    let reference = film
        .fichier_principal()
        .map(|p| relatif(&p.chemin_complet(), racine))
        .unwrap_or_else(|| relatif(dossier, racine));
    finaliser_film(&mut film, racine, &nom_dossier);
    film.source_media = SourceMedia::depuis_chemin(&reference);
    ResultatDossier::Film(Box::new(film))
}

/// Films posés à plat dans un dossier (ou à la racine de la source) : chaque
/// fichier vidéo forme un film avec ses fichiers associés.
///
/// Un fichier est rattaché à une vidéo si son nom de base (sans extension)
/// est identique à celui de la vidéo (sans marqueur de pile) ou le prolonge
/// après un séparateur (`Film.fr.srt`, `Film-poster.jpg`). Les fichiers sont
/// traités par longueur de nom décroissante, afin qu'un film au nom plus court
/// ne s'approprie pas les fichiers d'un film au nom plus long.
pub(crate) fn films_a_plat(
    dossier: &Path,
    chemins: &[PathBuf],
    racine: &Path,
    options: &OptionsScan,
) -> Vec<Film> {
    let mut fichiers: Vec<FichierMedia> =
        chemins.iter().map(|c| creer_fichier(c, racine)).collect();
    fichiers.sort_by(|a, b| {
        b.nom
            .chars()
            .count()
            .cmp(&a.nom.chars().count())
            .then_with(|| a.nom.cmp(&b.nom))
    });

    let mut restants: Vec<Option<FichierMedia>> = fichiers.into_iter().map(Some).collect();
    let mut films = Vec::new();

    for i in 0..restants.len() {
        let graine = match &restants[i] {
            Some(f) if f.type_fichier == TypeFichier::Video => f.clone(),
            _ => continue,
        };
        let base_sans_pile = nettoyer_marqueur_pile(&graine.nom_de_base());
        let base_min = base_sans_pile.to_lowercase();

        let mut groupe: Vec<FichierMedia> = Vec::new();
        for slot in restants.iter_mut() {
            let rattache = match slot {
                Some(f) => {
                    let b = f.nom_de_base().to_lowercase();
                    b == base_min
                        || (b.starts_with(&base_min)
                            && b[base_min.len()..].chars().next().is_some_and(|c| {
                                c.is_whitespace() || matches!(c, '.' | ',' | '_' | '-')
                            }))
                }
                None => false,
            };
            if rattache && let Some(f) = slot.take() {
                groupe.push(f);
            }
        }

        let mut film = Film::nouveau();
        film.chemin = dossier.to_path_buf();
        film.dossier_multi_films = true;
        let nom_sans_pile = nettoyer_marqueurs_pile(&graine.nom);
        let ta = detecter_titre_et_annee_avec(
            &nom_sans_pile,
            &options.mots_interdits,
            &options.prefixes_titre,
        );
        film.annee = ta.annee;
        film.titre = retirer_titre_triable(&ta.titre, &options.prefixes_titre);
        if film.titre.trim().is_empty() {
            film.titre = base_sans_pile.clone();
        }
        film.edition = Edition::depuis_chaine(&graine.nom_de_base());
        let reference = relatif(&graine.chemin_complet(), racine);
        identifiants_depuis(&mut film, &reference);
        rattacher_fichiers(&mut film, groupe);
        if !film.a_fichier_video() {
            continue;
        }
        finaliser_film(&mut film, racine, &graine.nom);
        film.source_media = SourceMedia::depuis_chemin(&reference);
        films.push(film);
    }
    films
}
