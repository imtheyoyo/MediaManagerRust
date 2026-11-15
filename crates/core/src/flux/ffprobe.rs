//! Lecture de la sortie `ffprobe -of flat` (lignes `streams.stream.0.codec_name="h264"`).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;

use super::normalisation as n;
use super::{ErreurFlux, InfosMedia, OutilAnalyse};
use crate::modele::{DrapeauFlux, FluxAudio, FluxSousTitres, FluxVideo};

/// Arguments de l'analyse principale (formats et flux).
pub fn arguments(chemin: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = [
        "-v",
        "error",
        "-hide_banner",
        "-show_format",
        "-show_streams",
        "-of",
        "flat",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    a.push(chemin.as_os_str().to_owned());
    a
}

/// Arguments de lecture des données annexes de la première image vidéo
/// (détection HDR10+).
pub fn arguments_trame(chemin: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = [
        "-v",
        "error",
        "-hide_banner",
        "-select_streams",
        "v:0",
        "-read_intervals",
        "%+#1",
        "-show_frames",
        "-show_entries",
        "frame=side_data_list",
        "-of",
        "flat",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    a.push(chemin.as_os_str().to_owned());
    a
}

/// Vrai si la sortie de trames mentionne des métadonnées dynamiques HDR10+.
pub fn trame_hdr10_plus(sortie: &str) -> bool {
    lire_plat(sortie).iter().any(|(c, v)| {
        c.ends_with("side_data_type") && (v.contains("2094-40") || v.contains("HDR10+"))
    })
}

/// Décode la sortie plate en dictionnaire `clé -> valeur` (valeurs `N/A`
/// et vides ignorées, guillemets et échappements retirés).
pub fn lire_plat(sortie: &str) -> BTreeMap<String, String> {
    let mut table = BTreeMap::new();
    for ligne in sortie.lines() {
        let Some((cle, valeur)) = ligne.split_once('=') else {
            continue;
        };
        let cle = cle.trim();
        if cle.is_empty() || cle.contains(' ') {
            continue;
        }
        let valeur = desechapper(valeur.trim());
        if valeur.is_empty() || valeur == "N/A" {
            continue;
        }
        table.insert(cle.to_string(), valeur);
    }
    table
}

fn desechapper(brut: &str) -> String {
    let Some(interieur) = brut.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
        return brut.to_string();
    };
    let mut res = String::with_capacity(interieur.len());
    let mut car = interieur.chars();
    while let Some(c) = car.next() {
        if c != '\\' {
            res.push(c);
            continue;
        }
        match car.next() {
            Some('n') => res.push('\n'),
            Some('r') => res.push('\r'),
            Some('t') => res.push('\t'),
            Some(autre) => res.push(autre),
            None => res.push('\\'),
        }
    }
    res
}

/// Accès aux champs d'un flux (préfixe `streams.stream.N.` retiré).
struct Champs<'a>(BTreeMap<&'a str, &'a str>);

impl<'a> Champs<'a> {
    fn txt(&self, cle: &str) -> &'a str {
        self.0.get(cle).copied().unwrap_or("")
    }
    fn entier(&self, cle: &str) -> u32 {
        self.txt(cle)
            .trim()
            .parse::<f64>()
            .map_or(0, |v| v.max(0.0) as u32)
    }
    fn vrai(&self, cle: &str) -> bool {
        self.txt(cle) == "1"
    }
    /// Étiquette insensible à la casse (`tags.language`, `tags.LANGUAGE`).
    fn etiquette(&self, nom: &str) -> &'a str {
        self.0
            .iter()
            .find(|(c, _)| {
                c.strip_prefix("tags.")
                    .is_some_and(|t| t.eq_ignore_ascii_case(nom))
            })
            .map_or("", |(_, v)| *v)
    }
    /// Débit en kbit/s : `bit_rate`, sinon étiquette Matroska `BPS`.
    fn debit(&self) -> u32 {
        let mut b = self.entier("bit_rate");
        if b == 0 {
            b = ["BPS", "BPS_eng", "BPS-eng"]
                .iter()
                .map(|t| self.etiquette(t))
                .find_map(|v| v.trim().parse::<u64>().ok())
                .unwrap_or(0) as u32;
        }
        b / 1000
    }
    fn annexes(&self) -> Vec<&'a str> {
        self.0
            .iter()
            .filter(|(c, _)| c.starts_with("side_data_list.") && c.ends_with("side_data_type"))
            .map(|(_, v)| *v)
            .collect()
    }
    /// Valeur d'un champ d'une donnée annexe donnée par son type.
    fn annexe(&self, type_annexe: &str, champ: &str) -> &'a str {
        let prefixe = self
            .0
            .iter()
            .find(|(c, v)| c.ends_with("side_data_type") && **v == type_annexe)
            .and_then(|(c, _)| c.strip_suffix("side_data_type"));
        prefixe
            .and_then(|p| self.0.get(format!("{p}{champ}").as_str()).copied())
            .unwrap_or("")
    }
    fn drapeaux(&self) -> Vec<DrapeauFlux> {
        let mut d = Vec::new();
        for (cle, drapeau) in [
            ("disposition.default", DrapeauFlux::Defaut),
            ("disposition.dub", DrapeauFlux::Doublage),
            ("disposition.original", DrapeauFlux::Original),
            ("disposition.comment", DrapeauFlux::Commentaire),
            ("disposition.lyrics", DrapeauFlux::Paroles),
            ("disposition.karaoke", DrapeauFlux::Karaoke),
            ("disposition.forced", DrapeauFlux::Force),
            ("disposition.hearing_impaired", DrapeauFlux::Malentendants),
            ("disposition.visual_impaired", DrapeauFlux::Malvoyants),
        ] {
            if self.vrai(cle) {
                d.push(drapeau);
            }
        }
        d
    }
}

/// Construit les informations normalisées depuis la sortie plate.
/// `extension` (sans point) affine le conteneur (`mkv`/`webm`, `ts`/`m2ts`…).
pub fn analyser_sortie(sortie: &str, extension: &str) -> Result<InfosMedia, ErreurFlux> {
    let table = lire_plat(sortie);
    if table.is_empty() {
        return Err(ErreurFlux::SortieInvalide {
            outil: "ffprobe".into(),
            raison: "aucune donnée".into(),
        });
    }
    let mut flux: BTreeMap<u32, BTreeMap<&str, &str>> = BTreeMap::new();
    let mut format: BTreeMap<&str, &str> = BTreeMap::new();
    for (cle, valeur) in &table {
        if let Some(reste) = cle.strip_prefix("streams.stream.")
            && let Some((num, champ)) = reste.split_once('.')
            && let Ok(num) = num.parse::<u32>()
        {
            flux.entry(num).or_default().insert(champ, valeur);
        } else if let Some(champ) = cle.strip_prefix("format.") {
            format.insert(champ, valeur);
        }
    }
    if flux.is_empty() && format.is_empty() {
        return Err(ErreurFlux::SortieInvalide {
            outil: "ffprobe".into(),
            raison: "ni flux ni format".into(),
        });
    }
    let format = Champs(format);
    let mut infos = InfosMedia {
        source: OutilAnalyse::Ffprobe,
        conteneur: n::conteneur(format.txt("format_name"), extension),
        duree_secondes: format
            .txt("duration")
            .trim()
            .parse::<f64>()
            .map_or(0, |d| d.max(0.0).round() as u32),
        debit_global: format.entier("bit_rate") / 1000,
        titre: format.etiquette("title").trim().to_string(),
        ..InfosMedia::default()
    };

    for champs in flux.into_values().map(Champs) {
        match champs.txt("codec_type") {
            "video" => {
                if champs.vrai("disposition.attached_pic") || champs.vrai("disposition.still_image")
                {
                    continue;
                }
                let (video, hdr) = flux_video(&champs);
                if infos.videos.is_empty() {
                    infos.formats_hdr = hdr;
                }
                infos.videos.push(video);
            }
            "audio" => infos.audio.push(flux_audio(&champs)),
            "subtitle" => infos.sous_titres.push(flux_sous_titres(&champs)),
            _ => {}
        }
    }
    // Durée de repli : étiquette DURATION du premier flux qui en porte une.
    if infos.duree_secondes == 0 {
        infos.duree_secondes = duree_etiquette(&table);
    }
    Ok(infos)
}

fn duree_etiquette(table: &BTreeMap<String, String>) -> u32 {
    table
        .iter()
        .filter(|(c, _)| {
            c.starts_with("streams.stream.")
                && c.rsplit('.')
                    .next()
                    .is_some_and(|t| t.eq_ignore_ascii_case("duration") && c.contains(".tags."))
        })
        .filter_map(|(_, v)| duree_hms(v))
        .max()
        .unwrap_or(0)
}

/// `01:02:03.500000000` → 3724 s (arrondi).
pub fn duree_hms(brut: &str) -> Option<u32> {
    let mut total = 0f64;
    for partie in brut.trim().split(':') {
        total = total * 60.0 + partie.trim().parse::<f64>().ok()?;
    }
    Some(total.max(0.0).round() as u32)
}

fn flux_video(c: &Champs<'_>) -> (FluxVideo, Vec<String>) {
    let largeur = c.entier("width");
    let hauteur = c.entier("height");
    let ratio_pixel = n::fraction(c.txt("sample_aspect_ratio")).unwrap_or(1.0);
    let images = n::fraction(c.txt("avg_frame_rate"))
        .or_else(|| n::fraction(c.txt("r_frame_rate")))
        .map_or(0.0, |f| n::arrondir(f, 3));
    let mut profondeur = c.entier("bits_per_raw_sample");
    if profondeur == 0 {
        profondeur = n::profondeur_pixel(c.txt("pix_fmt"));
    }
    let annexes = c.annexes();
    let hdr = n::formats_hdr(&n::IndicesHdr {
        transfert: c.txt("color_transfer"),
        primaires: c.txt("color_primaries"),
        annexes: annexes.clone(),
    });
    let mut stereo = c.annexe("Stereo 3D", "type");
    if stereo.is_empty() {
        stereo = c.etiquette("stereo_mode");
    }
    let codec = n::codec_video(c.txt("codec_name"), c.txt("codec_tag_string"), "");
    let video = FluxVideo {
        id_codec: id_codec(c),
        codec,
        format_exact: format_exact(largeur, hauteur, c.txt("field_order")),
        format_3d: n::format_3d(stereo, largeur, hauteur),
        largeur,
        hauteur,
        ratio_aspect: n::ratio_aspect(
            n::fraction(c.txt("display_aspect_ratio")),
            largeur,
            hauteur,
            ratio_pixel,
        ),
        ratio_aspect2: None,
        ratio_pixel: n::arrondir(ratio_pixel, 3) as f32,
        debit: c.debit(),
        profondeur_bits: profondeur,
        images_par_seconde: images,
        format_hdr: hdr.first().cloned().unwrap_or_default(),
    };
    (video, hdr)
}

/// Format exact : hauteur suivie de `p` (progressif) ou `i` (entrelacé).
fn format_exact(largeur: u32, hauteur: u32, ordre_trames: &str) -> String {
    if largeur == 0 || hauteur == 0 {
        return String::new();
    }
    let entrelace = matches!(ordre_trames, "tt" | "bb" | "tb" | "bt");
    format!("{hauteur}{}", if entrelace { 'i' } else { 'p' })
}

/// FourCC lisible (`avc1`, `XVID`), vide s'il est nul (`[0][0][0][0]`).
fn id_codec(c: &Champs<'_>) -> String {
    let t = c.txt("codec_tag_string");
    if t.is_empty() || t.contains('[') {
        String::new()
    } else {
        t.to_string()
    }
}

fn flux_audio(c: &Champs<'_>) -> FluxAudio {
    let titre = c.etiquette("title").trim().to_string();
    let mut profondeur = c.entier("bits_per_raw_sample");
    if profondeur == 0 {
        profondeur = c.entier("bits_per_sample");
    }
    if profondeur == 0 && c.txt("codec_name").starts_with("pcm_") {
        profondeur = n::profondeur_echantillon(c.txt("sample_fmt"));
    }
    let mut drapeaux = c.drapeaux();
    completer_drapeaux_titre(&titre, &mut drapeaux);
    FluxAudio {
        codec: n::codec_audio(c.txt("codec_name"), c.txt("profile")),
        id_codec: id_codec(c),
        langue: n::langue(c.etiquette("language")),
        titre,
        drapeaux,
        canaux: c.entier("channels"),
        debit: c.debit(),
        profondeur_bits: profondeur,
        configuration_canaux: configuration(c),
    }
}

/// Configuration des canaux d'après la disposition ; à défaut, déduite du
/// nombre de canaux pour le mono et la stéréo (seuls cas non ambigus).
fn configuration(c: &Champs<'_>) -> String {
    let conf = n::configuration_canaux(c.txt("channel_layout"));
    if !conf.is_empty() {
        return conf;
    }
    match c.entier("channels") {
        1 => "1/0/0".into(),
        2 => "2/0/0".into(),
        _ => String::new(),
    }
}

fn flux_sous_titres(c: &Champs<'_>) -> FluxSousTitres {
    let titre = c.etiquette("title").trim().to_string();
    let mut drapeaux = c.drapeaux();
    completer_drapeaux_titre(&titre, &mut drapeaux);
    FluxSousTitres {
        codec: n::codec_sous_titres(c.txt("codec_name")),
        langue: n::langue(c.etiquette("language")),
        titre,
        drapeaux,
    }
}

/// Complète les drapeaux d'après le titre du flux (`Forced`, `SDH`,
/// `Commentary`…), fréquent quand les dispositions ne sont pas posées.
pub(super) fn completer_drapeaux_titre(titre: &str, drapeaux: &mut Vec<DrapeauFlux>) {
    let t = titre.to_lowercase();
    let mots: Vec<&str> = t
        .split(|c: char| !c.is_alphanumeric())
        .filter(|m| !m.is_empty())
        .collect();
    let a = |m: &[&str]| mots.iter().any(|x| m.contains(x));
    let mut ajouter = |d: DrapeauFlux| {
        if !drapeaux.contains(&d) {
            drapeaux.push(d);
        }
    };
    if a(&["forced", "forcé", "forces", "forcés", "forced-only"]) {
        ajouter(DrapeauFlux::Force);
    }
    if a(&["sdh", "cc", "hi", "malentendants"]) {
        ajouter(DrapeauFlux::Malentendants);
    }
    if a(&["commentary", "commentaire", "commentaires", "comment"]) {
        ajouter(DrapeauFlux::Commentaire);
    }
    if a(&["ad", "audiodescription", "malvoyants"]) {
        ajouter(DrapeauFlux::Malvoyants);
    }
    drapeaux.sort();
}
