//! Normalisation des valeurs brutes des outils vers le vocabulaire de la
//! médiathèque (codecs, conteneurs, HDR, 3D, canaux, langues, ratios).
//!
//! Vocabulaire produit :
//! - codecs vidéo en minuscules : `h264`, `h265`, `av1`, `vp9`, `mpeg2`,
//!   `mpeg4`, `xvid`, `divx`, `vc1`… ;
//! - codecs audio : `AC3`, `EAC3`, `EAC3/Atmos`, `TrueHD`, `TrueHD/Atmos`,
//!   `DTS`, `DTS-ES`, `DTS-96/24`, `DTSHD-HRA`, `DTSHD-MA`, `DTS-X`,
//!   `DTS-X-IMAX`, `AAC`, `FLAC`, `MP3`, `PCM`, `Opus`… (clés comprises par
//!   l'écriture NFO) ;
//! - sous-titres : `srt`, `ass`, `ssa`, `pgs`, `vobsub`, `vtt`, `tx3g`… ;
//! - langues en ISO 639-2/T (`fra`, `eng`), vide si indéterminée.

use crate::renommage::{StyleLangue, ecrire_langue};

pub const DOLBY_VISION: &str = "Dolby Vision";
pub const HDR10_PLUS: &str = "HDR10+";
pub const HDR10: &str = "HDR10";
pub const HLG: &str = "HLG";

/// Catégorie de hauteur (`720p`, `1080p`…) avec la tolérance de 1 % de la
/// spécification ; vide si une dimension est nulle.
pub fn format_video(largeur: u32, hauteur: u32) -> String {
    crate::renommage::technique::format_video(largeur, hauteur)
}

/// Catégorie de définition (`LD`, `SD`, `HD`, `UHD`) d'un format vidéo.
pub fn definition(format: &str) -> &'static str {
    match format {
        "96p" | "120p" | "144p" | "240p" | "288p" | "360p" => "LD",
        "480p" | "540p" | "576p" => "SD",
        "720p" | "1080p" | "1440p" => "HD",
        "2160p" | "4320p" => "UHD",
        _ => "",
    }
}

/// Codec vidéo normalisé. `etiquette` est le FourCC éventuel (`XVID`,
/// `DX50`…), `version` la version de format (MPEG).
pub fn codec_video(brut: &str, etiquette: &str, version: &str) -> String {
    let c = brut.trim().to_ascii_lowercase();
    let e = etiquette.trim().to_ascii_lowercase();
    match c.as_str() {
        "" => String::new(),
        "h264" | "avc" | "avc1" | "h.264" | "x264" => "h264".into(),
        "hevc" | "h265" | "h.265" | "x265" | "hvc1" | "hev1" => "h265".into(),
        "av1" | "av01" => "av1".into(),
        "vp9" | "vp09" => "vp9".into(),
        "vp8" => "vp8".into(),
        "mpeg2video" | "mpeg-2 video" | "mpeg2" => "mpeg2".into(),
        "mpeg1video" | "mpeg-1 video" | "mpeg1" => "mpeg1".into(),
        "mpeg video" => {
            if version.contains('1') {
                "mpeg1".into()
            } else {
                "mpeg2".into()
            }
        }
        "mpeg4" | "mpeg-4 visual" | "mp4v" => {
            if e.starts_with("xvid") {
                "xvid".into()
            } else if e.starts_with("divx") || e.starts_with("dx50") || e == "div3" {
                "divx".into()
            } else {
                "mpeg4".into()
            }
        }
        "msmpeg4v3" | "msmpeg4v2" | "msmpeg4" => "divx".into(),
        "vc1" | "vc-1" | "wvc1" => "vc1".into(),
        "wmv3" | "wmv2" | "wmv1" => "wmv".into(),
        "prores" => "prores".into(),
        "mjpeg" => "mjpeg".into(),
        _ => c.replace(' ', ""),
    }
}

/// Codec audio normalisé. `profil` porte la variante (profil ffprobe ou
/// nom commercial / profil MediaInfo).
pub fn codec_audio(brut: &str, profil: &str) -> String {
    let c = brut.trim().to_ascii_lowercase();
    let p = profil.trim().to_ascii_lowercase();
    let atmos = p.contains("atmos") || p.contains("joc");
    match c.as_str() {
        "" => String::new(),
        "ac3" | "ac-3" | "a_ac3" => "AC3".into(),
        "eac3" | "e-ac-3" | "ec-3" => if atmos { "EAC3/Atmos" } else { "EAC3" }.into(),
        "truehd" | "mlp fba" | "mlp" => if atmos { "TrueHD/Atmos" } else { "TrueHD" }.into(),
        "dts" | "dca" => codec_dts(&p).into(),
        "aac" | "aac lc" | "he-aac" | "mp4a" | "aac_latm" => "AAC".into(),
        "flac" => "FLAC".into(),
        "mp3" | "mp3float" => "MP3".into(),
        "mp2" | "mp1" => "MP2".into(),
        "mpeg audio" => {
            if p.contains('3') {
                "MP3".into()
            } else {
                "MP2".into()
            }
        }
        "opus" => "Opus".into(),
        "vorbis" => "Vorbis".into(),
        "alac" => "ALAC".into(),
        "wmav2" | "wmav1" | "wmapro" | "wma" => "WMA".into(),
        "pcm" => "PCM".into(),
        _ if c.starts_with("pcm_") => "PCM".into(),
        _ if c.starts_with("adpcm") => "ADPCM".into(),
        _ => c.to_ascii_uppercase(),
    }
}

/// Variante DTS d'après le profil (`DTS-HD MA`, `DTS-HD Master Audio`,
/// `XLL X`, `DTS:X`…).
fn codec_dts(p: &str) -> &'static str {
    let x = p.contains("dts:x") || p.contains("dts-x") || p.contains("xll x") || p.ends_with(" x");
    if x && p.contains("imax") {
        "DTS-X-IMAX"
    } else if x {
        "DTS-X"
    } else if p.contains("ma") && (p.contains("hd") || p.contains("master") || p.contains("xll")) {
        "DTSHD-MA"
    } else if p.contains("hra") || p.contains("high resolution") || p.contains("xbr") {
        "DTSHD-HRA"
    } else if p.contains("express") || p.contains("lbr") {
        "DTS-Express"
    } else if p.contains("96/24") || p.contains("96") {
        "DTS-96/24"
    } else if p.contains("es") {
        "DTS-ES"
    } else {
        "DTS"
    }
}

/// Codec de sous-titres normalisé.
pub fn codec_sous_titres(brut: &str) -> String {
    let c = brut.trim().to_ascii_lowercase();
    match c.as_str() {
        "subrip" | "srt" | "utf-8" | "s_text/utf8" | "text" => "srt".into(),
        "ass" | "s_text/ass" => "ass".into(),
        "ssa" | "s_text/ssa" => "ssa".into(),
        "hdmv_pgs_subtitle" | "pgs" | "s_hdmv/pgs" => "pgs".into(),
        "dvd_subtitle" | "vobsub" | "rle" | "s_vobsub" => "vobsub".into(),
        "dvb_subtitle" | "dvb subtitle" => "dvbsub".into(),
        "webvtt" | "vtt" => "vtt".into(),
        "mov_text" | "timed text" | "tx3g" => "tx3g".into(),
        "eia_608" | "eia-608" => "cc608".into(),
        "hdmv_text_subtitle" => "textst".into(),
        _ => c,
    }
}

/// Conteneur normalisé d'après le nom de format de l'outil et l'extension.
pub fn conteneur(brut: &str, extension: &str) -> String {
    let c = brut.trim().to_ascii_lowercase();
    let ext = extension.trim().to_ascii_lowercase();
    let premier = c.split(',').next().unwrap_or("");
    match premier {
        "" => ext,
        "matroska" => if ext == "webm" {
            "webm"
        } else if ext == "mk3d" {
            "mk3d"
        } else {
            "mkv"
        }
        .into(),
        "webm" => "webm".into(),
        "mov" | "mpeg-4" | "quicktime" => match ext.as_str() {
            "mov" | "m4v" | "3gp" | "qt" => ext,
            _ => "mp4".into(),
        },
        "avi" => "avi".into(),
        "mpegts" | "mpeg-ts" => match ext.as_str() {
            "m2ts" | "mts" => ext,
            _ => "ts".into(),
        },
        "bdav" => "m2ts".into(),
        "mpeg" | "mpeg-ps" | "mpegps" => {
            if ext == "vob" {
                "vob".into()
            } else {
                "mpg".into()
            }
        }
        "asf" | "windows media" => if ext == "asf" { "asf" } else { "wmv" }.into(),
        "flv" | "flash video" => "flv".into(),
        "ogg" => "ogm".into(),
        "realmedia" | "rm" => "rm".into(),
        "dvd video" => "ifo".into(),
        "iso 9660" | "udf" => "iso".into(),
        _ => premier.replace(' ', ""),
    }
}

/// Ordonne les formats HDR du plus riche au plus simple et retire les doublons.
pub fn trier_formats_hdr(formats: &mut Vec<String>) {
    let rang = |f: &str| match f {
        DOLBY_VISION => 0,
        HDR10_PLUS => 1,
        HDR10 => 2,
        HLG => 3,
        _ => 4,
    };
    formats.sort_by_key(|f| rang(f));
    formats.dedup();
}

/// Indices HDR collectés sur un flux vidéo.
#[derive(Debug, Clone, Default)]
pub struct IndicesHdr<'a> {
    /// Fonction de transfert (`smpte2084`, `PQ`, `arib-std-b67`, `HLG`…).
    pub transfert: &'a str,
    /// Primaires de couleur (`bt2020`, `BT.2020`…).
    pub primaires: &'a str,
    /// Types de données annexes (side data) ou formats HDR déclarés par
    /// l'outil (`DOVI configuration record`, `SMPTE ST 2094 App 4`…).
    pub annexes: Vec<&'a str>,
}

/// Formats HDR déduits, du plus riche au plus simple (vide = SDR).
pub fn formats_hdr(indices: &IndicesHdr<'_>) -> Vec<String> {
    let t = indices.transfert.trim().to_ascii_lowercase();
    let mut formats = Vec::new();
    for a in &indices.annexes {
        let a = a.to_ascii_lowercase();
        if a.contains("dovi") || a.contains("dolby vision") {
            formats.push(DOLBY_VISION.to_string());
        }
        if a.contains("2094-40") || a.contains("2094 app 4") || a.contains("hdr10+") {
            formats.push(HDR10_PLUS.to_string());
        }
        if a.contains("hdr10") && !a.contains("hdr10+") {
            formats.push(HDR10.to_string());
        }
        if a.contains("hlg") {
            formats.push(HLG.to_string());
        }
    }
    let pq = t.contains("2084") || t == "pq";
    let hlg = t.contains("b67") || t.contains("hlg");
    // PQ suffit à qualifier le HDR10 (les primaires BT.2020 l'accompagnent
    // presque toujours, mais certains fichiers les omettent).
    if pq {
        formats.push(HDR10.to_string());
    }
    if hlg {
        formats.push(HLG.to_string());
    }
    trier_formats_hdr(&mut formats);
    formats
}

/// Format 3D (`3D SBS`, `3D HSBS`, `3D TAB`, `3D HTAB`, `3D MVC`, `3D`)
/// d'après une disposition stéréo brute ; vide si 2D.
pub fn format_3d(disposition: &str, largeur: u32, hauteur: u32) -> String {
    let d = disposition.trim().to_ascii_lowercase();
    if d.is_empty() || d == "mono" || d == "2d" {
        return String::new();
    }
    let ratio = if hauteur > 0 {
        largeur as f32 / hauteur as f32
    } else {
        0.0
    };
    let cote_a_cote = d.contains("side")
        || d.contains("left_right")
        || d.contains("right_left")
        || d.contains("sbs")
        || d.contains("col_interleaved");
    let haut_bas = d.contains("top")
        || d.contains("bottom")
        || d.contains("tab")
        || d.contains("row_interleaved");
    if cote_a_cote {
        // Image pleine : deux vues complètes côte à côte (ratio > 3).
        if ratio > 3.0 { "3D SBS" } else { "3D HSBS" }.into()
    } else if haut_bas {
        // Image pleine : deux vues complètes empilées (ratio < 1).
        if ratio > 0.0 && ratio < 1.0 {
            "3D TAB"
        } else {
            "3D HTAB"
        }
        .into()
    } else if d.contains("mvc") || d.contains("frame") || d.contains("block") {
        "3D MVC".into()
    } else {
        "3D".into()
    }
}

/// Configuration des canaux `avant/côtés/arrière.lfe` d'après une
/// disposition ffprobe (`5.1(side)`, `stereo`…) ; vide si inconnue.
pub fn configuration_canaux(disposition: &str) -> String {
    let d = disposition.trim().to_ascii_lowercase();
    let base = d.split('(').next().unwrap_or("").trim();
    match base {
        "mono" => "1/0/0",
        "stereo" | "downmix" => "2/0/0",
        "2.1" => "2/0/0.1",
        "3.0" => "3/0/0",
        "3.1" => "3/0/0.1",
        "4.0" => "3/0/1",
        "quad" => "2/0/2",
        "4.1" => "3/0/1.1",
        "5.0" => "3/2/0",
        "5.1" => "3/2/0.1",
        "6.0" => "3/2/1",
        "6.1" => "3/2/1.1",
        "7.0" => "3/2/2",
        "7.1" => "3/2/2.1",
        _ => "",
    }
    .to_string()
}

/// Notation pointée (`2.0`, `5.1`, `7.1`) : depuis la configuration si
/// connue, sinon `n.0` sous 6 canaux et `(n-1).1` au-delà ; vide si 0.
pub fn notation_canaux(canaux: u32, configuration: &str) -> String {
    let flux = crate::modele::FluxAudio {
        canaux,
        configuration_canaux: configuration.to_string(),
        ..Default::default()
    };
    crate::renommage::technique::canaux_point(&flux)
}

/// Langue normalisée en ISO 639-2/T ; vide pour « indéterminée ».
/// Un code inconnu de la table est conservé en minuscules.
pub fn langue(brut: &str) -> String {
    let c = brut.trim();
    let minuscule = c.to_ascii_lowercase();
    if matches!(
        minuscule.as_str(),
        "" | "und" | "unk" | "zxx" | "mis" | "mul" | "qaa" | "xx" | "none" | "unknown"
    ) {
        return String::new();
    }
    let converti = ecrire_langue(c, StyleLangue::Iso3T);
    if converti == c { minuscule } else { converti }
}

/// Fraction `a/b` ou `a:b` ou décimal → nombre ; `None` si invalide ou nul.
pub fn fraction(brut: &str) -> Option<f64> {
    let b = brut.trim();
    let (n, d) = match b.split_once(['/', ':']) {
        Some((n, d)) => (n.trim().parse::<f64>().ok()?, d.trim().parse::<f64>().ok()?),
        None => (b.parse::<f64>().ok()?, 1.0),
    };
    let v = n / d;
    (v.is_finite() && v > 0.0).then_some(v)
}

/// Arrondi à `n` décimales.
pub fn arrondir(x: f64, n: i32) -> f64 {
    let f = 10f64.powi(n);
    (x * f).round() / f
}

/// Ratio d'aspect d'affichage (deux décimales) : valeur déclarée si
/// valide, sinon largeur × ratio de pixel / hauteur ; 0 si inconnu.
pub fn ratio_aspect(declare: Option<f64>, largeur: u32, hauteur: u32, ratio_pixel: f64) -> f32 {
    let calcule = || {
        (largeur > 0 && hauteur > 0)
            .then(|| largeur as f64 * ratio_pixel.max(f64::MIN_POSITIVE) / hauteur as f64)
    };
    declare
        .filter(|r| *r > 0.1 && *r < 10.0)
        .or_else(calcule)
        .map_or(0.0, |r| arrondir(r, 2) as f32)
}

/// Profondeur de bits déduite d'un format de pixel (`yuv420p10le` → 10).
pub fn profondeur_pixel(format_pixel: &str) -> u32 {
    let f = format_pixel.trim().to_ascii_lowercase();
    if f.is_empty() {
        return 0;
    }
    for n in [16u32, 14, 12, 10, 9] {
        if f.contains(&format!("p{n}"))
            || f.ends_with(&format!("{n}le"))
            || f.ends_with(&format!("{n}be"))
        {
            return n;
        }
    }
    8
}

/// Profondeur de bits déduite d'un format d'échantillon audio (`s16`, `s32p`…).
pub fn profondeur_echantillon(format: &str) -> u32 {
    let f = format.trim().to_ascii_lowercase();
    if f.starts_with("s16") {
        16
    } else if f.starts_with("s32") || f.starts_with("flt") {
        32
    } else if f.starts_with("s64") || f.starts_with("dbl") {
        64
    } else if f.starts_with("u8") {
        8
    } else {
        0
    }
}
