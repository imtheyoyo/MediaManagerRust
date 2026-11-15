//! Lecture de la sortie `mediainfo --Language=raw --Full` (sections
//! `General`, `Video`, `Audio #1`… et lignes `Clé    : valeur`).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;

use super::ffprobe::completer_drapeaux_titre;
use super::normalisation as n;
use super::{ErreurFlux, InfosMedia, OutilAnalyse};
use crate::modele::{DrapeauFlux, FluxAudio, FluxSousTitres, FluxVideo};

/// Arguments de l'analyse (noms de paramètres internes, tous les champs).
pub fn arguments(chemin: &Path) -> Vec<OsString> {
    vec![
        OsString::from("--Language=raw"),
        OsString::from("--Full"),
        chemin.as_os_str().to_owned(),
    ]
}

/// Section de la sortie : type (`General`, `Video`, `Audio`, `Text`…) et
/// champs (première occurrence de chaque clé).
#[derive(Debug, Default)]
pub struct Section {
    pub genre: String,
    pub champs: BTreeMap<String, String>,
}

impl Section {
    fn txt(&self, cle: &str) -> &str {
        self.champs.get(cle).map_or("", |v| v.as_str())
    }
    /// Premier nombre de la valeur (`8 / 6` → 8, `1 920 pixels` exclu car
    /// les noms internes donnent des valeurs brutes).
    fn nombre(&self, cle: &str) -> Option<f64> {
        let v = self.txt(cle);
        let premier = v.split(['/', ' ']).find(|p| !p.is_empty())?;
        premier.trim().parse::<f64>().ok()
    }
    fn entier(&self, cle: &str) -> u32 {
        self.nombre(cle).map_or(0, |v| v.max(0.0).round() as u32)
    }
    fn oui(&self, cle: &str) -> bool {
        self.txt(cle).eq_ignore_ascii_case("yes")
    }
}

/// Découpe la sortie en sections.
pub fn lire_sections(sortie: &str) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    for ligne in sortie.lines() {
        let ligne = ligne.trim_end();
        if ligne.trim().is_empty() {
            continue;
        }
        match ligne
            .find(" : ")
            .or_else(|| ligne.strip_suffix(" :").map(|s| s.len()))
        {
            Some(pos) => {
                let cle = ligne[..pos].trim();
                let valeur = ligne.get(pos + 3..).unwrap_or("").trim();
                if let Some(s) = sections.last_mut()
                    && !cle.is_empty()
                    && !valeur.is_empty()
                {
                    s.champs
                        .entry(cle.to_string())
                        .or_insert_with(|| valeur.to_string());
                }
            }
            None => {
                let genre = ligne.split(" #").next().unwrap_or("").trim().to_string();
                sections.push(Section {
                    genre,
                    champs: BTreeMap::new(),
                });
            }
        }
    }
    sections
}

/// Construit les informations normalisées depuis la sortie MediaInfo.
pub fn analyser_sortie(sortie: &str, extension: &str) -> Result<InfosMedia, ErreurFlux> {
    let sections = lire_sections(sortie);
    let Some(general) = sections.iter().find(|s| s.genre == "General") else {
        return Err(ErreurFlux::SortieInvalide {
            outil: "mediainfo".into(),
            raison: "section General absente".into(),
        });
    };
    let titre = [general.txt("Title"), general.txt("Movie")]
        .into_iter()
        .find(|t| !t.is_empty())
        .unwrap_or("")
        .to_string();
    let mut infos = InfosMedia {
        source: OutilAnalyse::MediaInfo,
        conteneur: n::conteneur(general.txt("Format"), extension),
        duree_secondes: general
            .nombre("Duration")
            .map_or(0, |ms| (ms / 1000.0).max(0.0).round() as u32),
        debit_global: general
            .nombre("OverallBitRate")
            .map_or(0, |b| (b / 1000.0) as u32),
        titre,
        ..InfosMedia::default()
    };
    for s in &sections {
        match s.genre.as_str() {
            "Video" => {
                let (video, hdr) = flux_video(s);
                if infos.videos.is_empty() {
                    infos.formats_hdr = hdr;
                }
                infos.videos.push(video);
            }
            "Audio" => infos.audio.push(flux_audio(s)),
            "Text" => infos.sous_titres.push(flux_sous_titres(s)),
            _ => {}
        }
    }
    Ok(infos)
}

fn debit(s: &Section) -> u32 {
    s.nombre("BitRate")
        .or_else(|| s.nombre("BitRate_Nominal"))
        .map_or(0, |b| (b / 1000.0) as u32)
}

fn flux_video(s: &Section) -> (FluxVideo, Vec<String>) {
    let largeur = s.entier("Width");
    let hauteur = s.entier("Height");
    let ratio_pixel = s
        .nombre("PixelAspectRatio")
        .filter(|r| *r > 0.0)
        .unwrap_or(1.0);
    let hdr = n::formats_hdr(&n::IndicesHdr {
        transfert: s.txt("transfer_characteristics"),
        primaires: s.txt("colour_primaries"),
        annexes: [
            s.txt("HDR_Format"),
            s.txt("HDR_Format_Compatibility"),
            s.txt("HDR_Format_Commercial"),
        ]
        .into_iter()
        .filter(|v| !v.is_empty())
        .collect(),
    });
    let mut stereo = s.txt("MultiView_Layout").to_string();
    if stereo.is_empty() && s.entier("MultiView_Count") >= 2 {
        stereo = "mvc".into();
    }
    let entrelace = matches!(s.txt("ScanType"), "Interlaced" | "MBAFF");
    let video = FluxVideo {
        codec: n::codec_video(s.txt("Format"), s.txt("CodecID"), s.txt("Format_Version")),
        id_codec: s.txt("CodecID").to_string(),
        format_exact: if largeur > 0 && hauteur > 0 {
            format!("{hauteur}{}", if entrelace { 'i' } else { 'p' })
        } else {
            String::new()
        },
        format_3d: n::format_3d(&stereo, largeur, hauteur),
        largeur,
        hauteur,
        ratio_aspect: n::ratio_aspect(
            s.nombre("DisplayAspectRatio"),
            largeur,
            hauteur,
            ratio_pixel,
        ),
        ratio_aspect2: None,
        ratio_pixel: n::arrondir(ratio_pixel, 3) as f32,
        debit: debit(s),
        profondeur_bits: s.entier("BitDepth"),
        images_par_seconde: s.nombre("FrameRate").map_or(0.0, |f| n::arrondir(f, 3)),
        format_hdr: hdr.first().cloned().unwrap_or_default(),
    };
    (video, hdr)
}

fn drapeaux(s: &Section) -> Vec<DrapeauFlux> {
    let mut d = Vec::new();
    if s.oui("Default") {
        d.push(DrapeauFlux::Defaut);
    }
    if s.oui("Forced") {
        d.push(DrapeauFlux::Force);
    }
    match s.txt("ServiceKind") {
        "HI" => d.push(DrapeauFlux::Malentendants),
        "VI" => d.push(DrapeauFlux::Malvoyants),
        "CO" => d.push(DrapeauFlux::Commentaire),
        "KA" => d.push(DrapeauFlux::Karaoke),
        _ => {}
    }
    d
}

fn flux_audio(s: &Section) -> FluxAudio {
    // Variante : nom commercial, profil et fonctions additionnelles réunis
    // (`DTS-HD Master Audio`, `MA / Core`, `XLL X`, `JOC`…).
    let profil = [
        s.txt("Format_Commercial_IfAny"),
        s.txt("Format_Profile"),
        s.txt("Format_AdditionalFeatures"),
    ]
    .join(" ");
    let titre = s.txt("Title").to_string();
    let mut d = drapeaux(s);
    completer_drapeaux_titre(&titre, &mut d);
    FluxAudio {
        codec: n::codec_audio(s.txt("Format"), &profil),
        id_codec: s.txt("CodecID").to_string(),
        langue: n::langue(s.txt("Language")),
        titre,
        drapeaux: d,
        canaux: s.entier("Channel(s)"),
        debit: debit(s),
        profondeur_bits: s.entier("BitDepth"),
        configuration_canaux: s.txt("ChannelPositions/String2").to_string(),
    }
}

fn flux_sous_titres(s: &Section) -> FluxSousTitres {
    let titre = s.txt("Title").to_string();
    let mut d = drapeaux(s);
    completer_drapeaux_titre(&titre, &mut d);
    let mut codec = n::codec_sous_titres(s.txt("Format"));
    if codec.is_empty() {
        codec = n::codec_sous_titres(s.txt("CodecID"));
    }
    FluxSousTitres {
        codec,
        langue: n::langue(s.txt("Language")),
        titre,
        drapeaux: d,
    }
}
