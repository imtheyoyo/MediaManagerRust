//! Détermination du type d'un fichier (vidéo, nfo, artwork, sous-titres…) et
//! détection des structures de disques DVD / Blu-ray / HD-DVD.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use super::extensions::{est_audio, est_image, est_sous_titres, est_video};
use crate::modele::TypeFichier;
use crate::modele::fichier::{extension_minuscule, nom_de_base};

/// Dossiers d'extras reconnus (en minuscules).
pub const DOSSIERS_EXTRAS: &[&str] = &[
    "extra",
    "extras",
    "behind the scenes",
    "behindthescenes",
    "deleted scenes",
    "deletedscenes",
    "deleted",
    "featurette",
    "featurettes",
    "interview",
    "interviews",
    "scene",
    "scenes",
    "short",
    "shorts",
    "other",
    "others",
    "bloopers",
    "trickplay",
];

/// Dossiers de structure de disque : jamais traités comme un film.
pub const DOSSIERS_DISQUE: &[&str] = &["BDMV", "VIDEO_TS", "HVDVD_TS"];

fn rx(motif: &str) -> Regex {
    Regex::new(motif).unwrap()
}

// ---------------------------------------------------------------------------
// Disques
// ---------------------------------------------------------------------------

static RE_DVD: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(?i)^(video_ts|vts_\d\d_\d)\.(vob|bup|ifo)$"));
static RE_BLURAY: LazyLock<Regex> = LazyLock::new(|| {
    rx(r"(?i)^(index\.bdmv|movieobject\.bdmv|\d{5}\.m2ts|\d{5}\.clpi|\d{5}\.mpls)$")
});
static RE_HDDVD: LazyLock<Regex> = LazyLock::new(|| {
    rx(r"(?i)^(hv\d{3}[imt]\d{2}|hv[as]\d{5}|title\d{3})\.(evo|bup|ifo|vti|map)$")
});

/// Dossier de structure de disque (`BDMV`, `VIDEO_TS`, `HVDVD_TS`), insensible à la casse.
pub fn est_nom_dossier_disque(nom: &str) -> bool {
    DOSSIERS_DISQUE.iter().any(|d| d.eq_ignore_ascii_case(nom))
}

pub fn est_fichier_dvd(nom: &str) -> bool {
    nom.eq_ignore_ascii_case("video_ts") || RE_DVD.is_match(nom)
}

pub fn est_fichier_bluray(nom: &str) -> bool {
    nom.eq_ignore_ascii_case("bdmv") || RE_BLURAY.is_match(nom)
}

pub fn est_fichier_hddvd(nom: &str) -> bool {
    nom.eq_ignore_ascii_case("hvdvd_ts") || RE_HDDVD.is_match(nom)
}

/// Fichier appartenant à la structure interne d'un disque.
pub fn est_fichier_disque(nom: &str) -> bool {
    est_fichier_dvd(nom) || est_fichier_bluray(nom) || est_fichier_hddvd(nom)
}

/// Fichiers « identifiants » conservés à l'intérieur d'un dossier de disque.
pub fn est_fichier_identifiant_disque(nom: &str) -> bool {
    matches!(
        nom.to_lowercase().as_str(),
        "video_ts.ifo" | "index.bdmv" | "hv000i01.ifo"
    )
}

// ---------------------------------------------------------------------------
// Images
// ---------------------------------------------------------------------------

const EXT_IMG: &str = r"\.(?:jpg|jpeg|png|tbn|gif|bmp|webp)";
const OPT: &str = r"(?:.+[_.-])?";

fn rx_image(corps: &str) -> Regex {
    rx(&format!("(?i)^{corps}{EXT_IMG}$"))
}

struct MotifsImage {
    movieset: Regex,
    saison_poster: Regex,
    saison_fanart: Regex,
    saison_banner: Regex,
    saison_thumb: Regex,
    poster: Regex,
    extrafanart: Regex,
    fanart: Regex,
    banner: Regex,
    thumb: Regex,
    clearart: Regex,
    clearlogo: Regex,
    disc: Regex,
    characterart: Regex,
    keyart: Regex,
    squareart: Regex,
}

static MOTIFS_IMAGE: LazyLock<MotifsImage> = LazyLock::new(|| MotifsImage {
    movieset: rx_image(
        r"movieset-(?:poster|fanart|backdrop|banner|disc|discart|logo|clearlogo|clearart|thumb)",
    ),
    saison_poster: rx_image(r"season(?:[0-9]{1,6}|-specials|-all)(?:-poster)?"),
    saison_fanart: rx_image(r"season(?:[0-9]{1,6}|-specials|-all)-(?:fanart|backdrop|background)"),
    saison_banner: rx_image(r"season(?:[0-9]{1,6}|-specials|-all)-banner"),
    saison_thumb: rx_image(r"season(?:[0-9]{1,6}|-specials|-all)-(?:thumb|landscape)"),
    poster: rx_image(&format!("{OPT}(?:poster|folder|movie|cover)")),
    extrafanart: rx_image(&format!("{OPT}(?:fanart|backdrop|background)[0-9]+")),
    fanart: rx_image(&format!("{OPT}(?:backdrop|fanart|background)")),
    banner: rx_image(&format!("{OPT}banner")),
    thumb: rx_image(&format!("{OPT}(?:thumb|landscape)[0-9]{{0,2}}")),
    clearart: rx_image(&format!("{OPT}clearart")),
    clearlogo: rx_image(&format!("{OPT}(?:clearlogo|logo)")),
    disc: rx_image(&format!("{OPT}(?:discart|disc)")),
    characterart: rx_image(&format!("{OPT}characterart[0-9]{{0,2}}")),
    keyart: rx_image(&format!("{OPT}keyart")),
    squareart: rx_image(&format!("{OPT}(?:square|squareart|backgroundsquare)")),
});

/// Type d'une image d'après son nom et son dossier. `dossier` : chemin du
/// dossier parent (le dernier composant sert aux règles de nom de dossier).
pub fn type_image(nom_fichier: &str, dossier: &Path) -> TypeFichier {
    use TypeFichier::*;
    let m = &*MOTIFS_IMAGE;
    let dossier_complet = dossier.to_string_lossy().to_lowercase().replace('\\', "/");
    let nom_dossier = dossier
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if m.movieset.is_match(nom_fichier) {
        return Graphic;
    }
    if m.saison_poster.is_match(nom_fichier) {
        return SaisonPoster;
    }
    if m.saison_fanart.is_match(nom_fichier) {
        return SaisonFanart;
    }
    if m.saison_banner.is_match(nom_fichier) {
        return SaisonBanner;
    }
    if m.saison_thumb.is_match(nom_fichier) {
        return SaisonThumb;
    }
    if m.poster.is_match(nom_fichier) {
        return Poster;
    }
    if m.extrafanart.is_match(nom_fichier) {
        return ExtraFanart;
    }
    if m.fanart.is_match(nom_fichier) {
        return if dossier_complet.ends_with("extrafanart") {
            ExtraFanart
        } else {
            Fanart
        };
    }
    if m.banner.is_match(nom_fichier) {
        return Banner;
    }
    if m.thumb.is_match(nom_fichier) {
        return if dossier_complet.ends_with("extrathumbs") {
            ExtraThumb
        } else {
            Thumb
        };
    }
    if m.clearart.is_match(nom_fichier) {
        return Clearart;
    }
    if m.clearlogo.is_match(nom_fichier) {
        return Clearlogo;
    }
    if m.disc.is_match(nom_fichier) {
        return Disc;
    }
    if m.characterart.is_match(nom_fichier) || dossier_complet.ends_with("characterart") {
        return Characterart;
    }
    if m.keyart.is_match(nom_fichier) {
        return Keyart;
    }
    if m.squareart.is_match(nom_fichier) {
        return Squareart;
    }
    if nom_dossier == "extrafanart" || nom_dossier == "extrafanarts" {
        return ExtraFanart;
    }
    if nom_dossier == "extrathumb" || nom_dossier == "extrathumbs" {
        return ExtraThumb;
    }
    if !nom_dossier.is_empty() && nom_de_base(nom_fichier).to_lowercase() == nom_dossier {
        return Poster;
    }
    Graphic
}

// ---------------------------------------------------------------------------
// Type de fichier
// ---------------------------------------------------------------------------

static RE_THEME: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(?i)^(?:.*[_.-]+)?(?:theme|soundtrack)\d*$"));
static RE_EXTRA_NOM: LazyLock<Regex> = LazyLock::new(|| {
    rx(
        r"(?i)^.*[_.-](?:extras?|behindthescenes?|deleted|featurettes?|interviews?|scenes?|shorts?|others?|bloopers?)\d*$",
    )
});
static RE_TRAILER: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(?i)^.*[\[\]\(\)_.-]+trailer[\[\]\(\)_.-]?(?:\d)*$"));
static RE_SAMPLE: LazyLock<Regex> =
    LazyLock::new(|| rx(r"(?i)^.*[\[\]\(\)_.-]+sample[\[\]\(\)]?$"));

fn contient_ci(texte: &str, motif: &str) -> bool {
    texte.to_lowercase().contains(motif)
}

/// Détermine le type d'un fichier. `racine` : racine de la source de données
/// (les composantes intermédiaires du chemin relatif servent à repérer les
/// dossiers d'extras ; la première composante est le dossier du film).
pub fn type_fichier(chemin: &Path, racine: &Path) -> TypeFichier {
    use TypeFichier::*;
    let nom = chemin
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = extension_minuscule(&nom);
    let base = nom_de_base(&nom);
    let base_min = base.to_lowercase();
    let nom_min = nom.to_lowercase();
    let dossier = chemin.parent().unwrap_or(Path::new(""));
    let nom_parent = dossier
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    // 1. Dossier d'extras parmi les composantes intermédiaires.
    let relatif = chemin.strip_prefix(racine).unwrap_or(Path::new(&nom));
    let composantes: Vec<String> = relatif
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect();
    if composantes.len() > 2
        && composantes[1..composantes.len() - 1]
            .iter()
            .any(|c| DOSSIERS_EXTRAS.contains(&c.as_str()))
    {
        return Extra;
    }

    // 2. Fichiers de métadonnées.
    if ext == "nfo" {
        return Nfo;
    }
    if ext == "vsmeta" {
        return Vsmeta;
    }
    if nom_min.ends_with(".skiptro.json") {
        return Skiptro;
    }
    if nom_min.ends_with(".sp.hash.json") || nom_min.ends_with(".sp.json") {
        return Scenepass;
    }
    if ext == "xml" && (base_min.ends_with("-mediainfo") || base_min.ends_with(".mediainfo")) {
        return MediaInfo;
    }

    // 3. Images.
    if est_image(&ext) {
        return type_image(&nom, dossier);
    }

    // 4./5. Thèmes musicaux, audio, sous-titres.
    if est_audio(&ext) {
        return if RE_THEME.is_match(&base) {
            Theme
        } else {
            Audio
        };
    }
    if est_sous_titres(&ext) {
        return SousTitres;
    }

    // 6. Extras signalés par le nom.
    if contient_ci(&nom, ".extras.")
        || contient_ci(&nom, "[extras]")
        || contient_ci(&nom, "(extras)")
        || RE_EXTRA_NOM.is_match(&base)
    {
        return Extra;
    }

    // 7. Vidéos : bande-annonce, échantillon ou vidéo.
    if est_video(&ext) {
        if RE_TRAILER.is_match(&base)
            || base_min == "movie-trailer"
            || base_min == "trailer"
            || nom_parent == "trailer"
            || nom_parent == "trailers"
        {
            return Trailer;
        }
        if RE_SAMPLE.is_match(&base) || base_min == "sample" || nom_parent == "sample" {
            return Sample;
        }
        return Video;
    }

    // 8. Divers.
    if est_nom_dossier_disque(&nom) {
        return Video;
    }
    match ext.as_str() {
        "txt" => Texte,
        "bif" => Bif,
        _ => Inconnu,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn t(rel: &str) -> TypeFichier {
        let racine = PathBuf::from("/src");
        type_fichier(&racine.join(rel), &racine)
    }

    #[test]
    fn videos_et_assimiles() {
        assert_eq!(t("Film/Film.mkv"), TypeFichier::Video);
        assert_eq!(t("Film/Film-trailer.mkv"), TypeFichier::Trailer);
        assert_eq!(t("Film/Film.trailer.mp4"), TypeFichier::Trailer);
        assert_eq!(t("Film/trailer.mp4"), TypeFichier::Trailer);
        assert_eq!(t("Film/Trailers/bande.mp4"), TypeFichier::Trailer);
        assert_eq!(t("Film/Film-sample.mkv"), TypeFichier::Sample);
        assert_eq!(t("Film/sample.mkv"), TypeFichier::Sample);
        assert_eq!(t("Film/Sample/x.mkv"), TypeFichier::Sample);
        assert_eq!(t("Film/Film.ISO"), TypeFichier::Video);
        assert_eq!(t("Film/BDMV/index.bdmv"), TypeFichier::Video);
    }

    #[test]
    fn extras() {
        assert_eq!(t("Film/Extras/making of.mkv"), TypeFichier::Extra);
        assert_eq!(t("Film/Behind The Scenes/x.mkv"), TypeFichier::Extra);
        assert_eq!(t("Film/Deleted Scenes/x.mkv"), TypeFichier::Extra);
        assert_eq!(t("Film/Film-featurette.mkv"), TypeFichier::Extra);
        assert_eq!(t("Film/Film-behindthescenes2.mkv"), TypeFichier::Extra);
        assert_eq!(t("Film/Film.EXTRAS.mkv"), TypeFichier::Extra);
        // Le premier composant est le dossier du film : pas un extra.
        assert_eq!(t("Extras/Film.mkv"), TypeFichier::Video);
    }

    #[test]
    fn metadonnees_et_divers() {
        assert_eq!(t("Film/Film.nfo"), TypeFichier::Nfo);
        assert_eq!(t("Film/Film.mkv.vsmeta"), TypeFichier::Vsmeta);
        assert_eq!(t("Film/Film.skiptro.json"), TypeFichier::Skiptro);
        assert_eq!(t("Film/Film.sp.json"), TypeFichier::Scenepass);
        assert_eq!(t("Film/Film.sp.hash.json"), TypeFichier::Scenepass);
        assert_eq!(t("Film/Film-mediainfo.xml"), TypeFichier::MediaInfo);
        assert_eq!(t("Film/Film.srt"), TypeFichier::SousTitres);
        assert_eq!(t("Film/Film.fr.forced.ass"), TypeFichier::SousTitres);
        assert_eq!(t("Film/Film.mka"), TypeFichier::Audio);
        assert_eq!(t("Film/theme.mp3"), TypeFichier::Theme);
        assert_eq!(t("Film/Film-soundtrack.mp3"), TypeFichier::Theme);
        assert_eq!(t("Film/infos.txt"), TypeFichier::Texte);
        assert_eq!(t("Film/x.bif"), TypeFichier::Bif);
        assert_eq!(t("Film/inconnu.xyz"), TypeFichier::Inconnu);
    }

    #[test]
    fn images() {
        let d = Path::new("/src/Film");
        let ti = |n: &str| type_image(n, d);
        assert_eq!(ti("poster.jpg"), TypeFichier::Poster);
        assert_eq!(ti("folder.PNG"), TypeFichier::Poster);
        assert_eq!(ti("Film-poster.jpg"), TypeFichier::Poster);
        assert_eq!(ti("cover.tbn"), TypeFichier::Poster);
        assert_eq!(ti("fanart.jpg"), TypeFichier::Fanart);
        assert_eq!(ti("Film-backdrop.jpg"), TypeFichier::Fanart);
        assert_eq!(ti("fanart1.jpg"), TypeFichier::ExtraFanart);
        assert_eq!(ti("banner.jpg"), TypeFichier::Banner);
        assert_eq!(ti("thumb.jpg"), TypeFichier::Thumb);
        assert_eq!(ti("landscape.jpg"), TypeFichier::Thumb);
        assert_eq!(ti("clearart.png"), TypeFichier::Clearart);
        assert_eq!(ti("clearlogo.png"), TypeFichier::Clearlogo);
        assert_eq!(ti("logo.png"), TypeFichier::Clearlogo);
        assert_eq!(ti("disc.png"), TypeFichier::Disc);
        assert_eq!(ti("Film-discart.png"), TypeFichier::Disc);
        assert_eq!(ti("characterart.png"), TypeFichier::Characterart);
        assert_eq!(ti("keyart.jpg"), TypeFichier::Keyart);
        assert_eq!(ti("squareart.jpg"), TypeFichier::Squareart);
        assert_eq!(ti("season01-poster.jpg"), TypeFichier::SaisonPoster);
        assert_eq!(ti("season-specials-poster.jpg"), TypeFichier::SaisonPoster);
        assert_eq!(ti("season-all.jpg"), TypeFichier::SaisonPoster);
        assert_eq!(ti("season02-fanart.jpg"), TypeFichier::SaisonFanart);
        assert_eq!(ti("season02-banner.jpg"), TypeFichier::SaisonBanner);
        assert_eq!(ti("season02-landscape.jpg"), TypeFichier::SaisonThumb);
        assert_eq!(ti("movieset-poster.jpg"), TypeFichier::Graphic);
        assert_eq!(ti("quelconque.jpg"), TypeFichier::Graphic);
        // Image portant le nom du dossier : affiche.
        assert_eq!(ti("Film.jpg"), TypeFichier::Poster);
    }

    #[test]
    fn images_selon_le_dossier() {
        assert_eq!(
            type_image("fanart.jpg", Path::new("/s/F/extrafanart")),
            TypeFichier::ExtraFanart
        );
        assert_eq!(
            type_image("x.jpg", Path::new("/s/F/extrafanart")),
            TypeFichier::ExtraFanart
        );
        assert_eq!(
            type_image("thumb.jpg", Path::new("/s/F/extrathumbs")),
            TypeFichier::ExtraThumb
        );
        assert_eq!(
            type_image("x.jpg", Path::new("/s/F/extrathumbs")),
            TypeFichier::ExtraThumb
        );
        assert_eq!(
            type_image("x.jpg", Path::new("/s/F/characterart")),
            TypeFichier::Characterart
        );
    }

    #[test]
    fn disques() {
        assert!(est_fichier_dvd("VIDEO_TS.IFO"));
        assert!(est_fichier_dvd("VTS_01_1.VOB"));
        assert!(est_fichier_dvd("VIDEO_TS"));
        assert!(!est_fichier_dvd("film.vob"));
        assert!(est_fichier_bluray("index.bdmv"));
        assert!(est_fichier_bluray("00001.m2ts"));
        assert!(est_fichier_bluray("00000.mpls"));
        assert!(!est_fichier_bluray("film.m2ts"));
        assert!(est_fichier_hddvd("HV000I01.IFO"));
        assert!(est_fichier_hddvd("HVDVD_TS"));
        assert!(est_nom_dossier_disque("bdmv"));
        assert!(est_nom_dossier_disque("VIDEO_TS"));
        assert!(!est_nom_dossier_disque("STREAM"));
        assert!(est_fichier_identifiant_disque("INDEX.BDMV"));
    }
}
