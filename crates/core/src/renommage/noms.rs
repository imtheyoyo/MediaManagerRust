//! Briques de calcul des nouveaux noms communes aux films et aux épisodes.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::langues::{DRAPEAUX_SOUS_TITRES, StyleLangue, ecrire_langue, est_langue_connue};
use super::profil::{MARQUE_BASE, MARQUE_INDICE};
use crate::modele::fichier::nom_de_base;
use crate::modele::{DrapeauFlux, FichierMedia};
use crate::scan::pile::{marqueur_pile, nettoyer_marqueur_pile, numero_pile};

static RE_INDICE_FINAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+)$").expect("motif valide"));
static RE_BIF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(-\d+-\d+)\.bif$").expect("motif valide"));
static RE_INDICE_BANDE_ANNONCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)trailer[ ._-]?(\d+)$").expect("motif valide"));

/// Extension telle qu'écrite dans le nom (casse conservée), sans point.
pub(crate) fn extension_originale(nom: &str) -> String {
    match nom.rsplit_once('.') {
        Some((b, e)) if !b.is_empty() => e.to_string(),
        _ => String::new(),
    }
}

/// `base` + `.ext` (sans point si l'extension est vide).
pub(crate) fn avec_extension(base: &str, ext: &str) -> String {
    if ext.is_empty() {
        base.to_string()
    } else {
        format!("{base}.{ext}")
    }
}

/// Chaîne de pile d'un fichier : séparateur + marqueur d'origine, sinon
/// séparateur + `CD<n>`, sinon vide. Si `detecter`, le marqueur est cherché
/// dans le nom quand les champs du fichier sont vides.
pub(crate) fn chaine_pile(f: &FichierMedia, separateur: &str, detecter: bool) -> String {
    let mut marqueur = f.marqueur_empilement.clone();
    let mut numero = f.empilement;
    if marqueur.is_empty() && numero == 0 && detecter {
        marqueur = marqueur_pile(&f.nom);
        numero = numero_pile(&marqueur);
        if numero == 0 {
            marqueur.clear();
        }
    }
    if !marqueur.is_empty() {
        format!("{separateur}{marqueur}")
    } else if numero != 0 {
        format!("{separateur}CD{numero}")
    } else {
        String::new()
    }
}

/// Nom de base sans marqueur de pile.
pub(crate) fn base_sans_pile(f: &FichierMedia) -> String {
    nettoyer_marqueur_pile(&f.nom_de_base())
}

/// Extension d'une image : minuscules, `jpeg` ⇒ `jpg`, `tbn` ⇒ `png`/`jpg`
/// selon le conteneur.
pub(crate) fn extension_artwork(f: &FichierMedia) -> String {
    let e = f.extension();
    match e.as_str() {
        "jpeg" => "jpg".to_string(),
        "tbn" => {
            if f.format_conteneur.eq_ignore_ascii_case("png") {
                "png".to_string()
            } else {
                "jpg".to_string()
            }
        }
        _ => e,
    }
}

/// Indice final d'un nom de base (`fanart3` ⇒ `3`), vide sinon.
pub(crate) fn indice_final(base: &str) -> String {
    RE_INDICE_FINAL
        .captures(base)
        .map(|c| c[1].to_string())
        .unwrap_or_default()
}

/// Applique un motif (`<b>-poster`, `extrafanart/fanart<n>`) : chemin
/// relatif sans extension. `None` si le motif exige un nom de base vide.
pub(crate) fn appliquer_motif(motif: &str, base: &str, indice: &str) -> Option<String> {
    if motif.contains(MARQUE_BASE) && base.is_empty() {
        return None;
    }
    let r = motif
        .replace(MARQUE_BASE, base)
        .replace(MARQUE_INDICE, indice);
    let r = r.trim_matches('/').to_string();
    (!r.is_empty()).then_some(r)
}

/// Vrai si un composant du dossier du fichier (relatif à `racine`) figure
/// dans `noms` (insensible à la casse).
pub(crate) fn dans_dossier_nomme(f: &FichierMedia, racine: &Path, noms: &[&str]) -> bool {
    let Ok(rel) = f.chemin.strip_prefix(racine) else {
        return false;
    };
    rel.components().any(|c| {
        let n = c.as_os_str().to_string_lossy().to_lowercase();
        noms.iter().any(|x| x.eq_ignore_ascii_case(&n))
    })
}

/// Suffixe d'un nom de base après l'ancien nom de la vidéo
/// (`Film-behind` − `Film` ⇒ `-behind`). `None` si le préfixe ne correspond pas.
pub(crate) fn difference(base: &str, ancienne_base_video: &str) -> Option<String> {
    if ancienne_base_video.is_empty() {
        return None;
    }
    base.strip_prefix(ancienne_base_video)
        .map(str::to_string)
        .or_else(|| {
            let b = base.to_lowercase();
            let a = ancienne_base_video.to_lowercase();
            b.starts_with(&a).then(|| {
                base.get(ancienne_base_video.len()..)
                    .unwrap_or("")
                    .to_string()
            })
        })
}

/// Suffixe de langue d'un sous-titre : `[.langue][.forced][.sdh][.titre]`.
///
/// Depuis le premier flux du fichier s'il est renseigné ; sinon déduit de
/// l'ancien nom (`Film.fr.forced.srt` ⇒ `.fra.forced` en style ISO 639-2/T).
/// Un nom sans rapport avec la vidéo et sans code reconnu est conservé en
/// suffixe pour éviter les collisions.
pub(crate) fn suffixe_sous_titres(
    f: &FichierMedia,
    ancienne_base_video: &str,
    style: StyleLangue,
) -> String {
    if let Some(flux) = f.sous_titres.first()
        && !flux.langue.is_empty()
    {
        let mut s = format!(".{}", ecrire_langue(&flux.langue, style));
        if flux.drapeaux.contains(&DrapeauFlux::Force) {
            s.push_str(".forced");
        }
        if flux.drapeaux.contains(&DrapeauFlux::Malentendants) {
            s.push_str(".sdh");
        }
        if !flux.titre.trim().is_empty() {
            s.push('.');
            s.push_str(flux.titre.trim());
        }
        return s;
    }
    let base = nettoyer_marqueur_pile(&nom_de_base(&f.nom));
    let reste = match difference(&base, ancienne_base_video) {
        Some(d) => d,
        None => {
            let parties: Vec<&str> = base.split('.').filter(|p| !p.is_empty()).collect();
            let toutes_connues = !parties.is_empty()
                && parties.iter().all(|p| {
                    est_langue_connue(p)
                        || DRAPEAUX_SOUS_TITRES
                            .iter()
                            .any(|d| d.eq_ignore_ascii_case(p))
                });
            if toutes_connues {
                format!(".{base}")
            } else {
                return format!(".{base}");
            }
        }
    };
    reste
        .split(['.', '_'])
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            if est_langue_connue(p) {
                format!(".{}", ecrire_langue(p, style))
            } else {
                format!(".{p}")
            }
        })
        .collect()
}

/// Nouveau nom d'un fichier BIF : le motif `-\d+-\d+` est conservé.
pub(crate) fn nom_bif(f: &FichierMedia, base: &str) -> String {
    match RE_BIF.captures(&f.nom) {
        Some(c) => format!("{base}{}.bif", &c[1]),
        None => format!("{base}.bif"),
    }
}

/// Indice d'une bande-annonce (`Film-trailer2` ⇒ `.2`), vide sinon.
pub(crate) fn indice_bande_annonce(f: &FichierMedia) -> String {
    RE_INDICE_BANDE_ANNONCE
        .captures(&f.nom_de_base())
        .map(|c| format!(".{}", &c[1]))
        .unwrap_or_default()
}

/// Suffixe des fichiers `*.sp.json` / `*.sp.hash.json`.
pub(crate) fn suffixe_scenepass(nom: &str) -> &'static str {
    if nom.to_lowercase().ends_with(".sp.hash.json") {
        ".sp.hash.json"
    } else {
        ".sp.json"
    }
}
