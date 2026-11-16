//! Logique pure des bandes-annonces : sélection de la meilleure, nommage du
//! fichier téléchargé, arguments de `yt-dlp`, fusion des bandes-annonces
//! récupérées chez un fournisseur.
//!
//! Aucun accès réseau ni lancement de processus ici (voir
//! [`crate::flux::executer_en_continu`] pour l'exécution).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::modele::BandeAnnonce;

/// Conteneur du fichier téléchargé.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FormatVideoBandeAnnonce {
    /// Le meilleur flux disponible, conteneur choisi par `yt-dlp`.
    Auto,
    /// MP4 (flux fusionnés au besoin avec ffmpeg).
    #[default]
    Mp4,
    Mkv,
    Webm,
}

impl FormatVideoBandeAnnonce {
    /// Valeur de `--merge-output-format`, absente pour [`Self::Auto`].
    pub fn conteneur(self) -> Option<&'static str> {
        match self {
            Self::Auto => None,
            Self::Mp4 => Some("mp4"),
            Self::Mkv => Some("mkv"),
            Self::Webm => Some("webm"),
        }
    }
}

/// Hauteur en pixels d'une qualité textuelle (`1080p` ⇒ 1080, `720` ⇒ 720).
pub fn hauteur_depuis_qualite(qualite: &str) -> Option<u32> {
    let chiffres: String = qualite
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    chiffres.parse().ok().filter(|h| *h > 0)
}

/// Identifiant de vidéo YouTube d'une URL (`watch?v=`, `youtu.be/`,
/// `/embed/`, `/shorts/`).
pub fn identifiant_youtube(url: &str) -> Option<String> {
    let url = url.trim();
    let sans_schema = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let (hote, reste) = sans_schema.split_once('/').unwrap_or((sans_schema, ""));
    let hote = hote.to_lowercase();
    let hote = hote.strip_prefix("www.").unwrap_or(&hote);
    let valide = |s: &str| {
        let id: String = s
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        (!id.is_empty()).then_some(id)
    };
    match hote {
        "youtu.be" => valide(reste),
        "youtube.com" | "m.youtube.com" | "music.youtube.com" | "youtube-nocookie.com" => {
            for prefixe in ["embed/", "shorts/", "v/"] {
                if let Some(r) = reste.strip_prefix(prefixe) {
                    return valide(r);
                }
            }
            let requete = reste.split_once('?')?.1;
            requete
                .split(['&', '#'])
                .find_map(|p| p.strip_prefix("v="))
                .and_then(valide)
        }
        _ => None,
    }
}

/// Vrai si la bande-annonce est hébergée par YouTube.
pub fn est_youtube(bande: &BandeAnnonce) -> bool {
    bande.fournisseur.eq_ignore_ascii_case("youtube") || identifiant_youtube(&bande.url).is_some()
}

/// Clé d'identité d'une bande-annonce : identifiant YouTube, sinon l'URL.
pub fn cle_bande_annonce(bande: &BandeAnnonce) -> String {
    identifiant_youtube(&bande.url).unwrap_or_else(|| bande.url.trim().to_string())
}

/// URL transmise à `yt-dlp` : URL canonique pour YouTube (sans `&hd=1` ni
/// autres paramètres), URL d'origine sinon.
pub fn url_telechargement(bande: &BandeAnnonce) -> String {
    match identifiant_youtube(&bande.url) {
        Some(id) => format!("https://www.youtube.com/watch?v={id}"),
        None => bande.url.trim().to_string(),
    }
}

/// Rang de langue (0 = meilleur) d'une bande-annonce pour la langue voulue
/// (`fr` ou `fr-FR`) : langue (et région) exacte, même langue, sans langue,
/// anglais, autres.
fn rang_langue(langue_bande: &str, voulue: &str) -> u8 {
    let normaliser = |s: &str| s.trim().replace('_', "-").to_lowercase();
    let (b, v) = (normaliser(langue_bande), normaliser(voulue));
    let principale = |s: &str| s.split('-').next().unwrap_or("").to_string();
    if !v.is_empty() && b == v {
        0
    } else if !v.is_empty() && principale(&b) == principale(&v) && !b.is_empty() {
        1
    } else if b.is_empty() {
        2
    } else if principale(&b) == "en" {
        3
    } else {
        4
    }
}

/// Écart à la qualité préférée : la plus haute qualité ne dépassant pas la
/// préférence est la meilleure ; au-delà, la plus proche ; inconnue en
/// dernier.
fn ecart_qualite(qualite: &str, hauteur_preferee: u32) -> u32 {
    match hauteur_depuis_qualite(qualite) {
        Some(h) if h <= hauteur_preferee => hauteur_preferee - h,
        Some(h) => 10_000 + (h - hauteur_preferee),
        None => 5_000,
    }
}

/// Les `nombre` meilleures bandes-annonces YouTube : langue préférée
/// d'abord, puis qualité la plus proche (sans dépasser) de
/// `hauteur_preferee`, puis ordre d'origine. Les URL en double sont ignorées.
pub fn choisir_bandes_annonces<'a>(
    bandes: &'a [BandeAnnonce],
    langue: &str,
    hauteur_preferee: u32,
    nombre: usize,
) -> Vec<&'a BandeAnnonce> {
    let mut candidates: Vec<(usize, &BandeAnnonce)> = bandes
        .iter()
        .enumerate()
        .filter(|(_, b)| !b.url.trim().is_empty() && est_youtube(b))
        .collect();
    candidates.sort_by_key(|(i, b)| {
        (
            rang_langue(&b.langue, langue),
            ecart_qualite(&b.qualite, hauteur_preferee),
            *i,
        )
    });
    let mut vues = Vec::new();
    let mut sortie = Vec::new();
    for (_, b) in candidates {
        let cle = cle_bande_annonce(b);
        if vues.contains(&cle) {
            continue;
        }
        vues.push(cle);
        sortie.push(b);
        if sortie.len() >= nombre {
            break;
        }
    }
    sortie
}

/// Nom d'une bande-annonce : `<base>-trailer.<ext>` (numéro 1) ou
/// `<base>-trailer.<n>.<ext>` (numéro ≥ 2).
pub fn nom_bande_annonce(base: &str, extension: &str, numero: u32) -> String {
    let ext = extension.trim_start_matches('.');
    if numero <= 1 {
        format!("{base}-trailer.{ext}")
    } else {
        format!("{base}-trailer.{numero}.{ext}")
    }
}

/// Premier chemin libre pour une bande-annonce de `dossier` : un numéro est
/// occupé dès qu'un fichier `<base>-trailer[.n].*` existe, quelle que soit
/// son extension (jamais d'écrasement).
pub fn chemin_bande_annonce_libre(dossier: &Path, base: &str, extension: &str) -> PathBuf {
    let noms: Vec<String> = std::fs::read_dir(dossier)
        .map(|it| {
            it.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().to_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let occupe = |numero: u32| {
        let prefixe = nom_bande_annonce(base, "", numero).to_lowercase();
        noms.iter().any(|n| n.starts_with(&prefixe))
    };
    let mut numero = 1;
    loop {
        let chemin = dossier.join(nom_bande_annonce(base, extension, numero));
        if !occupe(numero) && !chemin.exists() {
            return chemin;
        }
        numero += 1;
    }
}

/// Sélecteur de format `yt-dlp` : meilleure qualité jusqu'à `hauteur_max`,
/// avec repli sur le meilleur flux disponible.
pub fn selecteur_format(hauteur_max: u32, format: FormatVideoBandeAnnonce) -> String {
    let h = hauteur_max.max(144);
    match format {
        FormatVideoBandeAnnonce::Mp4 => format!(
            "bv*[height<={h}][ext=mp4]+ba[ext=m4a]/b[height<={h}][ext=mp4]/\
             bv*[height<={h}]+ba/b[height<={h}]/bv*+ba/b"
        ),
        _ => format!("bv*[height<={h}]+ba/b[height<={h}]/bv*+ba/b"),
    }
}

/// Arguments de `yt-dlp` pour télécharger une seule vidéo vers
/// `modele_sortie` (`.../trailer.%(ext)s`).
pub fn arguments_yt_dlp(
    url: &str,
    modele_sortie: &Path,
    hauteur_max: u32,
    format: FormatVideoBandeAnnonce,
    cookies: Option<&Path>,
) -> Vec<OsString> {
    let mut a: Vec<OsString> = vec![
        "--no-playlist".into(),
        "--no-warnings".into(),
        "--newline".into(),
        "--no-overwrites".into(),
        "--socket-timeout".into(),
        "30".into(),
        "--retries".into(),
        "3".into(),
        "-f".into(),
        selecteur_format(hauteur_max, format).into(),
    ];
    if let Some(c) = format.conteneur() {
        a.push("--merge-output-format".into());
        a.push(c.into());
    }
    if let Some(c) = cookies {
        a.push("--cookies".into());
        a.push(c.as_os_str().to_os_string());
    }
    a.push("-o".into());
    a.push(modele_sortie.as_os_str().to_os_string());
    a.push(url.into());
    a
}

/// Pourcentage lu dans une ligne de progression de `yt-dlp`
/// (`[download]  45.3% of …`).
pub fn progression_yt_dlp(ligne: &str) -> Option<f32> {
    let reste = ligne.trim().strip_prefix("[download]")?.trim_start();
    let fin = reste.find('%')?;
    reste[..fin].trim().parse::<f32>().ok()
}

/// Extensions de fichiers vidéo reconnues pour le résultat d'un téléchargement.
const EXTENSIONS_VIDEO: &[&str] = &[
    "mp4", "mkv", "webm", "mov", "avi", "flv", "m4v", "ts", "3gp",
];

/// Vrai si l'extension (sans point) est celle d'une vidéo téléchargeable.
pub fn est_extension_video(ext: &str) -> bool {
    EXTENSIONS_VIDEO.contains(&ext.to_lowercase().as_str())
}

/// Fusionne les bandes-annonces `nouvelles` (issues du fournisseur
/// `scrape_par`) dans `existantes` :
/// - celles déjà présentes (même vidéo) sont mises à jour sans perdre le
///   drapeau `dans_nfo` ; si elles ne viennent pas de ce fournisseur, seuls
///   leurs champs vides sont complétés ;
/// - les anciennes du même fournisseur qui ont disparu sont retirées ;
/// - celles issues d'autres sources (NFO, saisie manuelle) sont conservées.
pub fn fusionner_bandes_annonces(
    existantes: &mut Vec<BandeAnnonce>,
    nouvelles: Vec<BandeAnnonce>,
    scrape_par: &str,
) {
    existantes.retain(|e| {
        e.scrape_par != scrape_par
            || nouvelles
                .iter()
                .any(|n| cle_bande_annonce(n) == cle_bande_annonce(e))
    });
    for mut n in nouvelles {
        n.scrape_par = scrape_par.to_string();
        let cle = cle_bande_annonce(&n);
        match existantes.iter_mut().find(|e| cle_bande_annonce(e) == cle) {
            Some(e) if e.scrape_par == scrape_par => {
                n.dans_nfo = e.dans_nfo;
                *e = n;
            }
            Some(e) => {
                let completer = |cible: &mut String, v: String| {
                    if cible.is_empty() {
                        *cible = v;
                    }
                };
                completer(&mut e.id, n.id);
                completer(&mut e.qualite, n.qualite);
                completer(&mut e.langue, n.langue);
                completer(&mut e.date, n.date);
                completer(&mut e.fournisseur, n.fournisseur);
            }
            None => existantes.push(n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bande(url: &str, qualite: &str, langue: &str) -> BandeAnnonce {
        BandeAnnonce {
            nom: format!("{langue} {qualite}"),
            url: url.into(),
            qualite: qualite.into(),
            langue: langue.into(),
            fournisseur: "youtube".into(),
            ..BandeAnnonce::default()
        }
    }

    #[test]
    fn identifiants_youtube() {
        assert_eq!(
            identifiant_youtube("https://www.youtube.com/watch?v=AbC_-1&hd=1").as_deref(),
            Some("AbC_-1")
        );
        assert_eq!(
            identifiant_youtube("https://youtu.be/XYZ?t=3").as_deref(),
            Some("XYZ")
        );
        assert_eq!(
            identifiant_youtube("https://www.youtube.com/watch?hd=1&v=K9").as_deref(),
            Some("K9")
        );
        assert_eq!(
            identifiant_youtube("https://youtube.com/embed/E1").as_deref(),
            Some("E1")
        );
        assert_eq!(identifiant_youtube("https://vimeo.com/123"), None);
        assert_eq!(identifiant_youtube("https://www.youtube.com/"), None);
        let b = bande("https://www.youtube.com/watch?v=AAA&hd=1", "1080p", "fr-FR");
        assert_eq!(
            url_telechargement(&b),
            "https://www.youtube.com/watch?v=AAA"
        );
    }

    #[test]
    fn selection_langue_puis_qualite() {
        let v = vec![
            bande("https://www.youtube.com/watch?v=EN1080", "1080p", "en-US"),
            bande("https://www.youtube.com/watch?v=FR480", "480p", "fr-FR"),
            bande("https://www.youtube.com/watch?v=FR1080", "1080p", "fr-FR"),
            bande("https://www.youtube.com/watch?v=FR720", "720p", "fr-FR"),
            bande("https://vimeo.com/99", "720p", "fr-FR"),
        ];
        let mut vimeo = v.clone();
        vimeo[4].fournisseur = "vimeo".into();
        let c = choisir_bandes_annonces(&vimeo, "fr", 720, 2);
        // Qualité préférée 720 : 720 puis 480 (le 1080 dépasse la préférence).
        assert_eq!(cle_bande_annonce(c[0]), "FR720");
        assert_eq!(cle_bande_annonce(c[1]), "FR480");
        let c = choisir_bandes_annonces(&vimeo, "fr", 2160, 1);
        assert_eq!(cle_bande_annonce(c[0]), "FR1080");
        let c = choisir_bandes_annonces(&vimeo, "de", 720, 1);
        assert_eq!(cle_bande_annonce(c[0]), "EN1080");
        assert!(choisir_bandes_annonces(&[], "fr", 720, 3).is_empty());
    }

    #[test]
    fn noms_et_non_ecrasement() {
        assert_eq!(
            nom_bande_annonce("Film (2020)", "mp4", 1),
            "Film (2020)-trailer.mp4"
        );
        assert_eq!(nom_bande_annonce("Film", ".mkv", 3), "Film-trailer.3.mkv");
        let d = tempfile::tempdir().unwrap();
        assert_eq!(
            chemin_bande_annonce_libre(d.path(), "F", "mp4"),
            d.path().join("F-trailer.mp4")
        );
        std::fs::write(d.path().join("F-trailer.mkv"), b"x").unwrap();
        assert_eq!(
            chemin_bande_annonce_libre(d.path(), "F", "mp4"),
            d.path().join("F-trailer.2.mp4")
        );
        std::fs::write(d.path().join("F-trailer.2.avi"), b"x").unwrap();
        assert_eq!(
            chemin_bande_annonce_libre(d.path(), "F", "mp4"),
            d.path().join("F-trailer.3.mp4")
        );
    }

    #[test]
    fn arguments_et_progression() {
        let a = arguments_yt_dlp(
            "https://www.youtube.com/watch?v=A",
            Path::new("/t/trailer.%(ext)s"),
            1080,
            FormatVideoBandeAnnonce::Mkv,
            None,
        );
        let a: Vec<String> = a.iter().map(|s| s.to_string_lossy().into_owned()).collect();
        assert!(a.contains(&"--no-playlist".to_string()));
        assert!(a.iter().any(|s| s.contains("height<=1080")));
        let i = a.iter().position(|s| s == "--merge-output-format").unwrap();
        assert_eq!(a[i + 1], "mkv");
        assert_eq!(a.last().unwrap(), "https://www.youtube.com/watch?v=A");
        assert_eq!(progression_yt_dlp("[download]  45.3% of 10MiB"), Some(45.3));
        assert_eq!(progression_yt_dlp("[download] Destination: x"), None);
        assert_eq!(progression_yt_dlp("autre"), None);
    }

    #[test]
    fn fusion() {
        let mut existantes = vec![
            BandeAnnonce {
                url: "https://www.youtube.com/watch?v=NFO".into(),
                dans_nfo: true,
                fournisseur: "from NFO".into(),
                ..BandeAnnonce::default()
            },
            BandeAnnonce {
                scrape_par: "tmdb".into(),
                ..bande("https://www.youtube.com/watch?v=OLD", "480p", "fr")
            },
        ];
        let nouvelles = vec![
            bande("https://www.youtube.com/watch?v=NFO&hd=1", "1080p", "fr-FR"),
            bande("https://www.youtube.com/watch?v=NEW", "720p", "en-US"),
        ];
        fusionner_bandes_annonces(&mut existantes, nouvelles, "tmdb");
        assert_eq!(existantes.len(), 2);
        assert!(existantes[0].dans_nfo);
        assert_eq!(existantes[0].qualite, "1080p");
        assert_eq!(existantes[0].fournisseur, "from NFO");
        assert_eq!(cle_bande_annonce(&existantes[1]), "NEW");
        assert_eq!(existantes[1].scrape_par, "tmdb");
    }
}
