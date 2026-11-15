//! Valeurs techniques dérivées des fichiers (format vidéo, canaux audio…)
//! exposées aux gabarits.

use crate::modele::{DrapeauFlux, FichierMedia, FluxAudio, SourceMedia, TypeFichier};

/// Catégorie de hauteur (`720p`, `1080p`…) ; vide si une dimension est nulle.
pub fn format_video(largeur: u32, hauteur: u32) -> String {
    if largeur == 0 || hauteur == 0 {
        return String::new();
    }
    const TABLE: &[(u32, u32, &str)] = &[
        (128, 96, "96p"),
        (160, 120, "120p"),
        (176, 144, "144p"),
        (256, 144, "144p"),
        (320, 240, "240p"),
        (352, 240, "240p"),
        (426, 240, "240p"),
        (480, 272, "288p"),
        (480, 360, "360p"),
        (640, 360, "360p"),
        (640, 480, "480p"),
        (720, 480, "480p"),
        (800, 480, "480p"),
        (853, 480, "480p"),
        (776, 592, "576p"),
        (960, 544, "540p"),
        (912, 384, "540p"),
        (1024, 576, "576p"),
        (1332, 720, "720p"),
        (1998, 1080, "1080p"),
        (2664, 1440, "1440p"),
        (3840, 2160, "2160p"),
        (3840, 1600, "2160p"),
        (4096, 2160, "2160p"),
        (4096, 1716, "2160p"),
        (3996, 2160, "2160p"),
    ];
    let flou = |x: u32| x + x / 100;
    TABLE
        .iter()
        .find(|(w, h, _)| largeur <= flou(*w) && hauteur <= flou(*h))
        .map_or("4320p", |t| t.2)
        .to_string()
}

/// Flux audio principal : le flux par défaut, sinon celui qui a le plus de canaux.
pub fn flux_audio_principal(fichier: &FichierMedia) -> Option<&FluxAudio> {
    fichier
        .flux_audio
        .iter()
        .find(|f| f.drapeaux.contains(&DrapeauFlux::Defaut))
        .or_else(|| {
            fichier
                .flux_audio
                .iter()
                .fold(None, |m: Option<&FluxAudio>, f| match m {
                    Some(x) if x.canaux >= f.canaux => Some(x),
                    _ => Some(f),
                })
        })
}

/// `6ch` ; vide si inconnu.
pub fn canaux(f: &FluxAudio) -> String {
    if f.canaux == 0 {
        String::new()
    } else {
        format!("{}ch", f.canaux)
    }
}

/// Notation pointée (`5.1`) : depuis la configuration (`3/2/0.1`) si connue,
/// sinon `n.0` sous 6 canaux et `(n-1).1` au-delà.
pub fn canaux_point(f: &FluxAudio) -> String {
    let conf = f.configuration_canaux.trim();
    if !conf.is_empty() {
        let mut somme = 0u32;
        let mut lfe = 0u32;
        let mut valide = true;
        for partie in conf.split('/') {
            let (principal, basse) = partie.split_once('.').unwrap_or((partie, ""));
            match principal.trim().parse::<u32>() {
                Ok(n) => somme += n,
                Err(_) => valide = false,
            }
            if !basse.is_empty() {
                lfe += basse.trim().parse::<u32>().unwrap_or(0);
            }
        }
        if valide && somme + lfe > 0 {
            return format!("{somme}.{lfe}");
        }
    }
    match f.canaux {
        0 => String::new(),
        n if n < 6 => format!("{n}.0"),
        n => format!("{}.1", n - 1),
    }
}

/// Ratio d'aspect avec deux décimales (`2.39`), vide si nul.
pub fn ratio(r: f32) -> String {
    if r > 0.0 {
        format!("{r:.2}")
    } else {
        String::new()
    }
}

/// Libellé lisible d'une source média (vide si inconnue).
pub fn libelle_source(s: SourceMedia) -> &'static str {
    match s {
        SourceMedia::UhdBluray => "UHD Blu-ray",
        SourceMedia::Bluray => "Blu-ray",
        SourceMedia::Dvd => "DVD",
        SourceMedia::HdDvd => "HD-DVD",
        SourceMedia::Tv => "TV",
        SourceMedia::Vhs => "VHS",
        SourceMedia::Laserdisc => "LaserDisc",
        SourceMedia::DVhs => "D-VHS",
        SourceMedia::HdRip => "HDRip",
        SourceMedia::Cam => "Cam",
        SourceMedia::Ts => "Telesync",
        SourceMedia::Tc => "Telecine",
        SourceMedia::DvdScr => "DVD Screener",
        SourceMedia::R5 => "R5",
        SourceMedia::WebRip => "WebRip",
        SourceMedia::WebDl => "Web-DL",
        SourceMedia::Stream => "Stream",
        SourceMedia::Inconnue => "",
    }
}

/// Convertit un `f32` en `f64` sans artefacts binaires (`7.8` reste `7.8`).
pub fn decimal_propre(x: f32) -> f64 {
    format!("{x}").parse().unwrap_or(f64::from(x))
}

/// Informations techniques agrégées pour les gabarits.
#[derive(Debug, Clone, Default)]
pub struct InfosTechniques {
    pub codec_video: String,
    pub format_video: String,
    pub resolution: String,
    pub ratio: String,
    pub ratio2: String,
    pub profondeur_bits: u32,
    pub debit_video: u32,
    pub images_par_seconde: f64,
    pub format_3d: String,
    pub hdr: String,
    pub format_hdr: String,
    pub codec_audio: String,
    pub codecs_audio: Vec<String>,
    pub canaux: String,
    pub liste_canaux: Vec<String>,
    pub canaux_point: String,
    pub liste_canaux_point: Vec<String>,
    pub langue_audio: String,
    pub langues_audio: Vec<String>,
    pub langues_sous_titres: Vec<String>,
    pub taille_videos: u64,
    pub crc32: String,
}

impl InfosTechniques {
    /// Agrège les informations du fichier principal, des sous-titres
    /// externes et la taille cumulée des vidéos.
    pub fn depuis(principal: Option<&FichierMedia>, fichiers: &[FichierMedia]) -> Self {
        let mut i = InfosTechniques {
            taille_videos: fichiers
                .iter()
                .filter(|f| f.type_fichier == TypeFichier::Video)
                .map(|f| f.taille)
                .sum(),
            ..Default::default()
        };
        if let Some(p) = principal {
            let v = &p.video;
            i.codec_video = v.codec.clone();
            i.format_video = format_video(v.largeur, v.hauteur);
            if v.largeur > 0 && v.hauteur > 0 {
                i.resolution = format!("{}x{}", v.largeur, v.hauteur);
            }
            i.ratio = ratio(v.ratio_aspect);
            i.ratio2 = v.ratio_aspect2.map(ratio).unwrap_or_default();
            i.profondeur_bits = v.profondeur_bits;
            i.debit_video = v.debit;
            i.images_par_seconde = v.images_par_seconde;
            i.format_3d = v.format_3d.clone();
            i.format_hdr = v.format_hdr.clone();
            if !v.format_hdr.is_empty() {
                i.hdr = "HDR".to_string();
            }
            if let Some(a) = flux_audio_principal(p) {
                i.codec_audio = a.codec.clone();
                i.canaux = canaux(a);
                i.canaux_point = canaux_point(a);
                i.langue_audio = a.langue.clone();
            }
            for a in &p.flux_audio {
                i.codecs_audio.push(a.codec.clone());
                i.liste_canaux.push(canaux(a));
                i.liste_canaux_point.push(canaux_point(a));
                if !a.langue.is_empty() {
                    i.langues_audio.push(a.langue.clone());
                }
            }
            for s in &p.sous_titres {
                if !s.langue.is_empty() {
                    i.langues_sous_titres.push(s.langue.clone());
                }
            }
            i.crc32 = p.sommes_controle.get("crc32").cloned().unwrap_or_default();
        }
        for f in fichiers
            .iter()
            .filter(|f| f.type_fichier == TypeFichier::SousTitres)
        {
            for s in &f.sous_titres {
                if !s.langue.is_empty() {
                    i.langues_sous_titres.push(s.langue.clone());
                }
            }
        }
        i
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(format_video(1920, 1080), "1080p");
        assert_eq!(format_video(1920, 800), "1080p");
        assert_eq!(format_video(1280, 720), "720p");
        assert_eq!(format_video(3840, 2160), "2160p");
        assert_eq!(format_video(7680, 4320), "4320p");
        assert_eq!(format_video(0, 1080), "");
    }

    #[test]
    fn canaux_audio() {
        let f = FluxAudio {
            canaux: 6,
            ..Default::default()
        };
        assert_eq!(canaux(&f), "6ch");
        assert_eq!(canaux_point(&f), "5.1");
        let f = FluxAudio {
            canaux: 2,
            ..Default::default()
        };
        assert_eq!(canaux_point(&f), "2.0");
        let f = FluxAudio {
            canaux: 8,
            configuration_canaux: "3/2/2.1".into(),
            ..Default::default()
        };
        assert_eq!(canaux_point(&f), "7.1");
    }

    #[test]
    fn decimal() {
        assert_eq!(decimal_propre(7.8), 7.8);
    }
}
