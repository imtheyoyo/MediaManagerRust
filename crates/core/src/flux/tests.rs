//! Tests de l'analyse des flux : parseurs sur sorties embarquées,
//! normalisations en table, recherche des outils, exécution réelle si
//! `ffprobe` est installé.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::normalisation as n;
use super::*;
use crate::modele::{DrapeauFlux, FichierMedia, TypeFichier};

/// Film UHD : HEVC 10 bits Dolby Vision + HDR10, TrueHD Atmos, DTS-HD MA,
/// AC3 commentaire, sous-titres PGS forcés et SRT SDH, jaquette jointe.
const FFPROBE_UHD: &str = r#"streams.stream.0.index=0
streams.stream.0.codec_name="hevc"
streams.stream.0.codec_long_name="H.265 / HEVC (High Efficiency Video Coding)"
streams.stream.0.profile="Main 10"
streams.stream.0.codec_type="video"
streams.stream.0.codec_tag_string="[0][0][0][0]"
streams.stream.0.width=3840
streams.stream.0.height=1600
streams.stream.0.sample_aspect_ratio="1:1"
streams.stream.0.display_aspect_ratio="12:5"
streams.stream.0.pix_fmt="yuv420p10le"
streams.stream.0.color_transfer="smpte2084"
streams.stream.0.color_primaries="bt2020"
streams.stream.0.field_order="progressive"
streams.stream.0.r_frame_rate="24000/1001"
streams.stream.0.avg_frame_rate="24000/1001"
streams.stream.0.bit_rate="N/A"
streams.stream.0.bits_per_raw_sample="N/A"
streams.stream.0.disposition.default=1
streams.stream.0.disposition.attached_pic=0
streams.stream.0.tags.BPS="58123456"
streams.stream.0.tags.DURATION="02:16:00.125000000"
streams.stream.0.side_data_list.side_data.0.side_data_type="DOVI configuration record"
streams.stream.0.side_data_list.side_data.0.dv_profile=8
streams.stream.1.index=1
streams.stream.1.codec_name="truehd"
streams.stream.1.profile="Dolby TrueHD + Dolby Atmos"
streams.stream.1.codec_type="audio"
streams.stream.1.sample_fmt="s32"
streams.stream.1.channels=8
streams.stream.1.channel_layout="7.1"
streams.stream.1.bits_per_raw_sample="24"
streams.stream.1.disposition.default=1
streams.stream.1.tags.language="eng"
streams.stream.1.tags.title="TrueHD Atmos 7.1"
streams.stream.1.tags.BPS="4500000"
streams.stream.2.index=2
streams.stream.2.codec_name="dts"
streams.stream.2.profile="DTS-HD MA"
streams.stream.2.codec_type="audio"
streams.stream.2.channels=6
streams.stream.2.channel_layout="5.1(side)"
streams.stream.2.bits_per_raw_sample="24"
streams.stream.2.disposition.default=0
streams.stream.2.tags.language="fre"
streams.stream.2.tags.title="VFF"
streams.stream.3.index=3
streams.stream.3.codec_name="ac3"
streams.stream.3.codec_type="audio"
streams.stream.3.channels=2
streams.stream.3.channel_layout="stereo"
streams.stream.3.bit_rate="192000"
streams.stream.3.disposition.comment=1
streams.stream.3.tags.language="eng"
streams.stream.3.tags.title="Commentaire du réalisateur"
streams.stream.4.index=4
streams.stream.4.codec_name="hdmv_pgs_subtitle"
streams.stream.4.codec_type="subtitle"
streams.stream.4.disposition.forced=1
streams.stream.4.tags.language="fre"
streams.stream.4.tags.title="Forcés"
streams.stream.5.index=5
streams.stream.5.codec_name="subrip"
streams.stream.5.codec_type="subtitle"
streams.stream.5.tags.language="eng"
streams.stream.5.tags.title="English \"SDH\""
streams.stream.6.index=6
streams.stream.6.codec_name="mjpeg"
streams.stream.6.codec_type="video"
streams.stream.6.width=600
streams.stream.6.height=900
streams.stream.6.disposition.attached_pic=1
streams.stream.6.tags.filename="cover.jpg"
format.filename="/films/Film (2021)/Film.mkv"
format.nb_streams=7
format.format_name="matroska,webm"
format.duration="8160.125000"
format.size="61234567890"
format.bit_rate="60032145"
format.tags.title="Film (2021)"
"#;

/// Ancien AVI : Xvid 720×304 entrelacé, MP3 stéréo, langue indéterminée.
const FFPROBE_AVI: &str = r#"streams.stream.0.codec_name="mpeg4"
streams.stream.0.codec_type="video"
streams.stream.0.codec_tag_string="XVID"
streams.stream.0.width=720
streams.stream.0.height=304
streams.stream.0.sample_aspect_ratio="1:1"
streams.stream.0.display_aspect_ratio="45:19"
streams.stream.0.pix_fmt="yuv420p"
streams.stream.0.field_order="tt"
streams.stream.0.avg_frame_rate="25/1"
streams.stream.0.bit_rate="1012345"
streams.stream.1.codec_name="mp3"
streams.stream.1.codec_type="audio"
streams.stream.1.codec_tag_string="U[0][0][0]"
streams.stream.1.channels=2
streams.stream.1.channel_layout="stereo"
streams.stream.1.bit_rate="128000"
streams.stream.1.tags.language="und"
format.format_name="avi"
format.duration="5400.400000"
format.bit_rate="1150000"
"#;

/// Même film UHD vu par MediaInfo (noms internes).
const MEDIAINFO_UHD: &str = "General
Count                                    : 331
StreamKind                               : General
Format                                   : Matroska
Format_Version                           : 4
FileSize                                 : 61234567890
Duration                                 : 8160125.000000
Duration/String                          : 2 h 16 min
OverallBitRate                           : 60032145
Movie                                    : Film (2021)

Video
StreamKind                               : Video
ID                                       : 1
Format                                   : HEVC
Format_Profile                           : Main 10@L5.1@High
HDR_Format                               : Dolby Vision / SMPTE ST 2086
HDR_Format_Compatibility                 : HDR10 / HDR10
CodecID                                  : V_MPEGH/ISO/HEVC
BitRate                                  : 58123456
Width                                    : 3840
Height                                   : 1600
PixelAspectRatio                         : 1.000
DisplayAspectRatio                       : 2.400
FrameRate                                : 23.976
ScanType                                 : Progressive
BitDepth                                 : 10
colour_primaries                         : BT.2020
transfer_characteristics                 : PQ
Default                                  : Yes

Audio #1
StreamKind                               : Audio
Format                                   : MLP FBA
Format_Commercial_IfAny                  : Dolby TrueHD with Dolby Atmos
Format_AdditionalFeatures                : 16-ch
CodecID                                  : A_TRUEHD
BitRate                                  : 4500000
Channel(s)                               : 8
ChannelPositions/String2                 : 3/2/2.1
BitDepth                                 : 24
Title                                    : TrueHD Atmos 7.1
Language                                 : en
Default                                  : Yes
Forced                                   : No

Audio #2
StreamKind                               : Audio
Format                                   : DTS
Format_Commercial_IfAny                  : DTS-HD Master Audio
Format_Profile                           : MA / Core
Format_AdditionalFeatures                : XLL
CodecID                                  : A_DTS
Channel(s)                               : 6
ChannelPositions/String2                 : 3/2/0.1
BitDepth                                 : 24
Title                                    : VFF
Language                                 : fr
Default                                  : No

Audio #3
StreamKind                               : Audio
Format                                   : AC-3
CodecID                                  : A_AC3
BitRate                                  : 192000
Channel(s)                               : 2
ChannelPositions/String2                 : 2/0/0
Title                                    : Commentaire du réalisateur
Language                                 : en
ServiceKind                              : CO

Text #1
StreamKind                               : Text
Format                                   : PGS
CodecID                                  : S_HDMV/PGS
Title                                    : Forcés
Language                                 : fr
Default                                  : No
Forced                                   : Yes

Text #2
StreamKind                               : Text
Format                                   : UTF-8
CodecID                                  : S_TEXT/UTF8
Title                                    : English SDH
Language                                 : en
Forced                                   : No

Menu
StreamKind                               : Menu
00:00:00.000                             : en:Chapitre 1
";

// ---------------------------------------------------------------------------
// ffprobe
// ---------------------------------------------------------------------------

#[test]
fn flux_ffprobe_lecture_plate_et_echappements() {
    let t = ffprobe::lire_plat("a.b=\"x \\\"y\\\" \\\\ z\"\na.c=N/A\na.d=12\nligne sans egal\n");
    assert_eq!(t.get("a.b").map(String::as_str), Some("x \"y\" \\ z"));
    assert!(!t.contains_key("a.c"));
    assert_eq!(t.get("a.d").map(String::as_str), Some("12"));
}

#[test]
fn flux_ffprobe_film_uhd() {
    let i = ffprobe::analyser_sortie(FFPROBE_UHD, "mkv").unwrap();
    assert_eq!(i.source, OutilAnalyse::Ffprobe);
    assert_eq!(i.conteneur, "mkv");
    assert_eq!(i.duree_secondes, 8160);
    assert_eq!(i.debit_global, 60032);
    assert_eq!(i.titre, "Film (2021)");
    assert_eq!(i.videos.len(), 1, "la jaquette jointe est ignorée");
    let v = i.video().unwrap();
    assert_eq!(v.codec, "h265");
    assert_eq!((v.largeur, v.hauteur), (3840, 1600));
    assert_eq!(i.format_video(), "2160p");
    assert_eq!(v.format_exact, "1600p");
    assert!((v.ratio_aspect - 2.4).abs() < 1e-6);
    assert_eq!(v.profondeur_bits, 10);
    assert_eq!(v.debit, 58123);
    assert!((v.images_par_seconde - 23.976).abs() < 1e-9);
    assert_eq!(i.formats_hdr, vec!["Dolby Vision", "HDR10"]);
    assert_eq!(v.format_hdr, "Dolby Vision");
    assert_eq!(v.format_3d, "");

    assert_eq!(i.audio.len(), 3);
    let a = &i.audio[0];
    assert_eq!(a.codec, "TrueHD/Atmos");
    assert_eq!(a.langue, "eng");
    assert_eq!(a.canaux, 8);
    assert_eq!(a.configuration_canaux, "3/2/2.1");
    assert_eq!(a.profondeur_bits, 24);
    assert_eq!(a.debit, 4500);
    assert_eq!(a.drapeaux, vec![DrapeauFlux::Defaut]);
    assert_eq!(i.audio[1].codec, "DTSHD-MA");
    assert_eq!(i.audio[1].langue, "fra");
    assert_eq!(
        n::notation_canaux(i.audio[1].canaux, &i.audio[1].configuration_canaux),
        "5.1"
    );
    assert_eq!(i.audio[2].codec, "AC3");
    assert_eq!(i.audio[2].debit, 192);
    assert_eq!(i.audio[2].drapeaux, vec![DrapeauFlux::Commentaire]);

    assert_eq!(i.sous_titres.len(), 2);
    assert_eq!(i.sous_titres[0].codec, "pgs");
    assert_eq!(i.sous_titres[0].langue, "fra");
    assert_eq!(i.sous_titres[0].drapeaux, vec![DrapeauFlux::Force]);
    assert_eq!(i.sous_titres[1].codec, "srt");
    assert_eq!(i.sous_titres[1].titre, "English \"SDH\"");
    assert_eq!(i.sous_titres[1].drapeaux, vec![DrapeauFlux::Malentendants]);
}

#[test]
fn flux_ffprobe_avi_ancien() {
    let i = ffprobe::analyser_sortie(FFPROBE_AVI, "avi").unwrap();
    let v = i.video().unwrap();
    assert_eq!(v.codec, "xvid");
    assert_eq!(v.id_codec, "XVID");
    assert_eq!(v.format_exact, "304i");
    assert_eq!(i.format_video(), "480p");
    assert_eq!(v.profondeur_bits, 8);
    assert!((v.ratio_aspect - 2.37).abs() < 1e-6);
    assert!(i.formats_hdr.is_empty());
    assert_eq!(i.audio[0].codec, "MP3");
    assert_eq!(i.audio[0].langue, "");
    assert_eq!(i.audio[0].id_codec, "");
    assert_eq!(i.duree_secondes, 5400);
    assert_eq!(i.conteneur, "avi");
}

#[test]
fn flux_ffprobe_duree_de_repli_et_3d() {
    let sortie = "streams.stream.0.codec_name=\"h264\"\n\
        streams.stream.0.codec_type=\"video\"\n\
        streams.stream.0.width=1920\n\
        streams.stream.0.height=1080\n\
        streams.stream.0.tags.stereo_mode=\"top_bottom\"\n\
        streams.stream.0.tags.DURATION=\"01:30:00.600000000\"\n\
        format.format_name=\"matroska,webm\"\n";
    let i = ffprobe::analyser_sortie(sortie, "mk3d").unwrap();
    assert_eq!(i.duree_secondes, 5401);
    assert_eq!(i.conteneur, "mk3d");
    assert_eq!(i.video().unwrap().format_3d, "3D HTAB");
}

#[test]
fn flux_ffprobe_sortie_vide_refusee() {
    assert!(matches!(
        ffprobe::analyser_sortie("", "mkv"),
        Err(ErreurFlux::SortieInvalide { .. })
    ));
}

#[test]
fn flux_ffprobe_trame_hdr10_plus() {
    let t = "frames.frame.0.side_data_list.side_data.0.side_data_type=\"HDR Dynamic Metadata SMPTE2094-40 (HDR10+)\"\n";
    assert!(ffprobe::trame_hdr10_plus(t));
    assert!(!ffprobe::trame_hdr10_plus(
        "frames.frame.0.side_data_list.side_data.0.side_data_type=\"Mastering display metadata\"\n"
    ));
}

#[test]
fn flux_ffprobe_arguments() {
    let a = ffprobe::arguments(Path::new("/x/f.mkv"));
    let a: Vec<String> = a.iter().map(|s| s.to_string_lossy().into_owned()).collect();
    assert_eq!(
        a,
        [
            "-v",
            "error",
            "-hide_banner",
            "-show_format",
            "-show_streams",
            "-of",
            "flat",
            "/x/f.mkv"
        ]
    );
}

// ---------------------------------------------------------------------------
// MediaInfo
// ---------------------------------------------------------------------------

#[test]
fn flux_mediainfo_sections() {
    let s = mediainfo::lire_sections(MEDIAINFO_UHD);
    let genres: Vec<&str> = s.iter().map(|x| x.genre.as_str()).collect();
    assert_eq!(
        genres,
        [
            "General", "Video", "Audio", "Audio", "Audio", "Text", "Text", "Menu"
        ]
    );
    assert_eq!(s[0].champs["Duration"], "8160125.000000");
}

#[test]
fn flux_mediainfo_film_uhd_equivalent_a_ffprobe() {
    let m = mediainfo::analyser_sortie(MEDIAINFO_UHD, "mkv").unwrap();
    let f = ffprobe::analyser_sortie(FFPROBE_UHD, "mkv").unwrap();
    assert_eq!(m.source, OutilAnalyse::MediaInfo);
    assert_eq!(m.conteneur, f.conteneur);
    assert_eq!(m.duree_secondes, f.duree_secondes);
    assert_eq!(m.debit_global, f.debit_global);
    assert_eq!(m.titre, f.titre);
    assert_eq!(m.formats_hdr, f.formats_hdr);
    let (vm, vf) = (m.video().unwrap(), f.video().unwrap());
    assert_eq!(vm.codec, vf.codec);
    assert_eq!((vm.largeur, vm.hauteur), (vf.largeur, vf.hauteur));
    assert_eq!(vm.ratio_aspect, vf.ratio_aspect);
    assert_eq!(vm.profondeur_bits, vf.profondeur_bits);
    assert_eq!(vm.format_exact, vf.format_exact);
    let codecs = |i: &InfosMedia| i.audio.iter().map(|a| a.codec.clone()).collect::<Vec<_>>();
    assert_eq!(codecs(&m), codecs(&f));
    let langues = |i: &InfosMedia| {
        i.audio
            .iter()
            .map(|a| a.langue.clone())
            .chain(i.sous_titres.iter().map(|s| s.langue.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(langues(&m), langues(&f));
    for (a, b) in m.audio.iter().zip(&f.audio) {
        assert_eq!(a.canaux, b.canaux);
        assert_eq!(a.configuration_canaux, b.configuration_canaux);
        assert_eq!(a.drapeaux, b.drapeaux);
    }
    for (a, b) in m.sous_titres.iter().zip(&f.sous_titres) {
        assert_eq!(a.codec, b.codec);
        assert_eq!(a.drapeaux, b.drapeaux);
    }
}

#[test]
fn flux_mediainfo_sans_general_refuse() {
    assert!(mediainfo::analyser_sortie("Video\nFormat : AVC\n", "mkv").is_err());
}

// ---------------------------------------------------------------------------
// Normalisations
// ---------------------------------------------------------------------------

#[test]
fn flux_normalisation_codecs_video() {
    for (brut, etiquette, version, attendu) in [
        ("h264", "avc1", "", "h264"),
        ("AVC", "", "", "h264"),
        ("hevc", "", "", "h265"),
        ("HEVC", "", "", "h265"),
        ("av1", "", "", "av1"),
        ("vp9", "", "", "vp9"),
        ("mpeg2video", "", "", "mpeg2"),
        ("MPEG Video", "", "Version 1", "mpeg1"),
        ("MPEG Video", "", "Version 2", "mpeg2"),
        ("mpeg4", "XVID", "", "xvid"),
        ("MPEG-4 Visual", "DX50", "", "divx"),
        ("mpeg4", "FMP4", "", "mpeg4"),
        ("msmpeg4v3", "DIV3", "", "divx"),
        ("VC-1", "WVC1", "", "vc1"),
        ("", "", "", ""),
    ] {
        assert_eq!(
            n::codec_video(brut, etiquette, version),
            attendu,
            "{brut}/{etiquette}"
        );
    }
}

#[test]
fn flux_normalisation_codecs_audio() {
    for (brut, profil, attendu) in [
        ("ac3", "", "AC3"),
        ("AC-3", "Dolby Digital", "AC3"),
        ("eac3", "", "EAC3"),
        ("eac3", "Dolby Digital Plus + Dolby Atmos", "EAC3/Atmos"),
        (
            "E-AC-3",
            "Dolby Digital Plus with Dolby Atmos  JOC",
            "EAC3/Atmos",
        ),
        ("truehd", "", "TrueHD"),
        ("truehd", "Dolby TrueHD + Dolby Atmos", "TrueHD/Atmos"),
        ("MLP FBA", "Dolby TrueHD  ", "TrueHD"),
        ("dts", "DTS", "DTS"),
        ("dts", "DTS-ES", "DTS-ES"),
        ("dts", "DTS 96/24", "DTS-96/24"),
        ("dts", "DTS-HD HRA", "DTSHD-HRA"),
        ("dts", "DTS-HD MA", "DTSHD-MA"),
        ("DTS", "DTS-HD Master Audio MA / Core XLL", "DTSHD-MA"),
        ("dts", "DTS-HD MA + DTS:X", "DTS-X"),
        ("dts", "DTS-HD MA + DTS:X IMAX", "DTS-X-IMAX"),
        ("DTS", "DTS-HD Master Audio MA / Core XLL X", "DTS-X"),
        ("dts", "DTS Express", "DTS-Express"),
        ("aac", "LC", "AAC"),
        ("flac", "", "FLAC"),
        ("mp3", "", "MP3"),
        ("MPEG Audio", "Layer 3", "MP3"),
        ("MPEG Audio", "Layer 2", "MP2"),
        ("pcm_s24le", "", "PCM"),
        ("opus", "", "Opus"),
        ("vorbis", "", "Vorbis"),
    ] {
        assert_eq!(n::codec_audio(brut, profil), attendu, "{brut}/{profil}");
    }
}

#[test]
fn flux_normalisation_codecs_sous_titres_et_conteneurs() {
    for (brut, attendu) in [
        ("subrip", "srt"),
        ("UTF-8", "srt"),
        ("ass", "ass"),
        ("hdmv_pgs_subtitle", "pgs"),
        ("PGS", "pgs"),
        ("dvd_subtitle", "vobsub"),
        ("VobSub", "vobsub"),
        ("webvtt", "vtt"),
        ("mov_text", "tx3g"),
    ] {
        assert_eq!(n::codec_sous_titres(brut), attendu, "{brut}");
    }
    for (brut, ext, attendu) in [
        ("matroska,webm", "mkv", "mkv"),
        ("matroska,webm", "webm", "webm"),
        ("Matroska", "MKV", "mkv"),
        ("mov,mp4,m4a,3gp,3g2,mj2", "mp4", "mp4"),
        ("mov,mp4,m4a,3gp,3g2,mj2", "mov", "mov"),
        ("MPEG-4", "m4v", "m4v"),
        ("avi", "avi", "avi"),
        ("AVI", "divx", "avi"),
        ("mpegts", "m2ts", "m2ts"),
        ("mpegts", "ts", "ts"),
        ("BDAV", "m2ts", "m2ts"),
        ("mpeg", "vob", "vob"),
        ("MPEG-PS", "mpg", "mpg"),
        ("asf", "wmv", "wmv"),
        ("Windows Media", "wmv", "wmv"),
        ("", "mkv", "mkv"),
    ] {
        assert_eq!(n::conteneur(brut, ext), attendu, "{brut}/{ext}");
    }
}

#[test]
fn flux_normalisation_formats_video() {
    for (l, h, attendu, def) in [
        (720, 576, "576p", "SD"),
        (720, 480, "480p", "SD"),
        (1280, 720, "720p", "HD"),
        (1280, 536, "720p", "HD"),
        (1920, 1080, "1080p", "HD"),
        (1920, 800, "1080p", "HD"),
        (1940, 1090, "1080p", "HD"), // dans la tolérance de 1 %
        (2560, 1440, "1440p", "HD"),
        (3840, 2160, "2160p", "UHD"),
        (3840, 1600, "2160p", "UHD"),
        (4096, 1716, "2160p", "UHD"),
        (7680, 4320, "4320p", "UHD"),
        (640, 360, "360p", "LD"),
        (0, 1080, "", ""),
    ] {
        let f = n::format_video(l, h);
        assert_eq!(f, attendu, "{l}x{h}");
        assert_eq!(n::definition(&f), def, "{l}x{h}");
    }
}

#[test]
fn flux_normalisation_hdr() {
    let cas: &[(&str, &str, &[&str], &[&str])] = &[
        ("smpte2084", "bt2020", &[], &["HDR10"]),
        ("PQ", "BT.2020", &[], &["HDR10"]),
        ("arib-std-b67", "bt2020", &[], &["HLG"]),
        ("HLG", "BT.2020", &[], &["HLG"]),
        ("bt709", "bt709", &[], &[]),
        ("", "", &[], &[]),
        (
            "smpte2084",
            "bt2020",
            &["DOVI configuration record"],
            &["Dolby Vision", "HDR10"],
        ),
        ("", "", &["Dolby Vision"], &["Dolby Vision"]),
        (
            "PQ",
            "BT.2020",
            &["SMPTE ST 2094 App 4", "HDR10+ Profile B"],
            &["HDR10+", "HDR10"],
        ),
        (
            "",
            "",
            &["Dolby Vision / SMPTE ST 2086", "HDR10 / HDR10"],
            &["Dolby Vision", "HDR10"],
        ),
        (
            "arib-std-b67",
            "bt2020",
            &["DOVI configuration record"],
            &["Dolby Vision", "HLG"],
        ),
    ];
    for (t, p, annexes, attendu) in cas {
        let r = n::formats_hdr(&n::IndicesHdr {
            transfert: t,
            primaires: p,
            annexes: annexes.to_vec(),
        });
        assert_eq!(r, attendu.to_vec(), "{t}/{p}/{annexes:?}");
    }
}

#[test]
fn flux_normalisation_3d() {
    for (d, l, h, attendu) in [
        ("side by side", 1920, 1080, "3D HSBS"),
        ("left_right", 3840, 1080, "3D SBS"),
        ("Side by Side (left eye first)", 1920, 1080, "3D HSBS"),
        ("top and bottom", 1920, 1080, "3D HTAB"),
        ("top_bottom", 1920, 2160, "3D TAB"),
        ("Top-Bottom (left eye first)", 1920, 1080, "3D HTAB"),
        ("mvc", 1920, 1080, "3D MVC"),
        ("frame alternate", 1920, 1080, "3D MVC"),
        ("mono", 1920, 1080, ""),
        ("", 1920, 1080, ""),
    ] {
        assert_eq!(n::format_3d(d, l, h), attendu, "{d}");
    }
}

#[test]
fn flux_normalisation_canaux() {
    for (disposition, config, canaux, point) in [
        ("mono", "1/0/0", 1, "1.0"),
        ("stereo", "2/0/0", 2, "2.0"),
        ("2.1", "2/0/0.1", 3, "2.1"),
        ("5.1", "3/2/0.1", 6, "5.1"),
        ("5.1(side)", "3/2/0.1", 6, "5.1"),
        ("6.1", "3/2/1.1", 7, "6.1"),
        ("7.1", "3/2/2.1", 8, "7.1"),
        ("7.1(wide)", "3/2/2.1", 8, "7.1"),
    ] {
        assert_eq!(
            n::configuration_canaux(disposition),
            config,
            "{disposition}"
        );
        assert_eq!(n::notation_canaux(canaux, config), point, "{disposition}");
    }
    // Sans configuration : n.0 sous 6 canaux, (n-1).1 au-delà.
    assert_eq!(n::configuration_canaux("hexadecagonal"), "");
    assert_eq!(n::notation_canaux(2, ""), "2.0");
    assert_eq!(n::notation_canaux(6, ""), "5.1");
    assert_eq!(n::notation_canaux(8, ""), "7.1");
    assert_eq!(n::notation_canaux(0, ""), "");
}

#[test]
fn flux_normalisation_langues() {
    for (brut, attendu) in [
        ("fre", "fra"),
        ("fra", "fra"),
        ("fr", "fra"),
        ("FR", "fra"),
        ("eng", "eng"),
        ("en", "eng"),
        ("ger", "deu"),
        ("de", "deu"),
        ("chi", "zho"),
        ("pt-BR", "por-BR"),
        ("und", ""),
        ("", ""),
        ("zxx", ""),
        ("tlh", "tlh"),
        ("Klingon", "klingon"),
    ] {
        assert_eq!(n::langue(brut), attendu, "{brut}");
    }
}

#[test]
fn flux_normalisation_nombres() {
    assert_eq!(
        n::fraction("24000/1001").map(|f| n::arrondir(f, 3)),
        Some(23.976)
    );
    assert_eq!(n::fraction("16:9").map(|f| n::arrondir(f, 3)), Some(1.778));
    assert_eq!(n::fraction("0/0"), None);
    assert_eq!(n::fraction("abc"), None);
    assert_eq!(n::fraction("2.400"), Some(2.4));
    assert_eq!(n::ratio_aspect(None, 1920, 1080, 1.0), 1.78);
    assert_eq!(n::ratio_aspect(None, 720, 576, 64.0 / 45.0), 1.78);
    assert_eq!(n::ratio_aspect(Some(2.35), 1920, 816, 1.0), 2.35);
    assert_eq!(n::ratio_aspect(None, 0, 0, 1.0), 0.0);
    for (pix, bits) in [
        ("yuv420p", 8),
        ("yuv420p10le", 10),
        ("yuv444p12be", 12),
        ("p010le", 10),
        ("", 0),
    ] {
        assert_eq!(n::profondeur_pixel(pix), bits, "{pix}");
    }
    assert_eq!(ffprobe::duree_hms("01:02:03.500000000"), Some(3724));
    assert_eq!(ffprobe::duree_hms("x"), None);
}

// ---------------------------------------------------------------------------
// Report dans le modèle
// ---------------------------------------------------------------------------

#[test]
fn flux_application_au_fichier() {
    let infos = ffprobe::analyser_sortie(FFPROBE_UHD, "mkv").unwrap();
    let mut f = FichierMedia::nouveau(Path::new("/films/Film (2021)/Film.mkv"), TypeFichier::Video);
    f.duree_secondes = 1;
    appliquer_au_fichier(&mut f, &infos);
    assert_eq!(f.format_conteneur, "mkv");
    assert_eq!(f.duree_secondes, 8160);
    assert_eq!(f.debit_global, 60032);
    assert_eq!(f.titre, "Film (2021)");
    assert_eq!(f.video.codec, "h265");
    assert_eq!(f.video.format_hdr, "Dolby Vision");
    assert_eq!(f.flux_audio.len(), 3);
    assert_eq!(f.sous_titres.len(), 2);
    assert!(!f.est_graphique_anime);
    // Les valeurs techniques sont lisibles par le renommeur.
    assert_eq!(
        crate::renommage::technique::format_video(f.video.largeur, f.video.hauteur),
        "2160p"
    );

    // Une analyse vide ne remet pas à zéro durée/conteneur connus.
    appliquer_au_fichier(&mut f, &InfosMedia::default());
    assert_eq!(f.duree_secondes, 8160);
    assert_eq!(f.format_conteneur, "mkv");
    assert_eq!(f.video.codec, "h265");
    assert!(f.flux_audio.is_empty());
}

// ---------------------------------------------------------------------------
// Recherche des outils et exécution
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn creer_executable(chemin: &Path, contenu: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(chemin.parent().unwrap()).unwrap();
    std::fs::write(chemin, contenu).unwrap();
    std::fs::set_permissions(chemin, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[test]
fn flux_recherche_des_outils_par_ordre_de_priorite() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tmp.path().join("app");
    let systeme = tmp.path().join("bin");
    let regle = tmp.path().join("regle");
    creer_executable(&systeme.join("ffprobe"), "#!/bin/sh\n");
    creer_executable(&systeme.join("mediainfo"), "#!/bin/sh\n");
    // Fichier non exécutable : ignoré.
    std::fs::create_dir_all(app.join("outils")).unwrap();
    std::fs::write(app.join("outils").join("mediainfo"), "").unwrap();

    let mut config = ConfigOutils {
        dossier_application: Some(app.clone()),
        path: Some(systeme.clone().into_os_string()),
        delai: Some(Duration::from_secs(7)),
        ..ConfigOutils::default()
    };
    let o = trouver_outils(&config);
    assert_eq!(o.ffprobe, Some(systeme.join("ffprobe")));
    assert_eq!(o.mediainfo, Some(systeme.join("mediainfo")));
    assert_eq!(o.delai, Duration::from_secs(7));

    // addons/ à côté de l'application avant le PATH, outils/ avant addons/.
    creer_executable(&app.join("addons").join("ffprobe"), "#!/bin/sh\n");
    assert_eq!(
        trouver_outils(&config).ffprobe,
        Some(app.join("addons").join("ffprobe"))
    );
    creer_executable(&app.join("outils").join("ffprobe"), "#!/bin/sh\n");
    assert_eq!(
        trouver_outils(&config).ffprobe,
        Some(app.join("outils").join("ffprobe"))
    );

    // Le réglage passe en premier : exécutable voisin `ffmpeg` transposé.
    creer_executable(&regle.join("ffprobe"), "#!/bin/sh\n");
    config.chemin_ffprobe = Some(regle.join("ffmpeg"));
    assert_eq!(trouver_outils(&config).ffprobe, Some(regle.join("ffprobe")));
    config.chemin_ffprobe = Some(regle.clone());
    assert_eq!(trouver_outils(&config).ffprobe, Some(regle.join("ffprobe")));
    // Réglage invalide : repli sur la recherche normale.
    config.chemin_ffprobe = Some(tmp.path().join("absent").join("ffprobe"));
    assert_eq!(
        trouver_outils(&config).ffprobe,
        Some(app.join("outils").join("ffprobe"))
    );

    let rien = trouver_outils(&ConfigOutils {
        dossier_application: Some(tmp.path().join("vide")),
        path: Some(std::ffi::OsString::new()),
        ..ConfigOutils::default()
    });
    assert!(!rien.disponible());
}

#[cfg(unix)]
#[test]
fn flux_execution_delai_et_echec() {
    let tmp = tempfile::tempdir().unwrap();
    let lent = tmp.path().join("lent");
    creer_executable(&lent, "#!/bin/sh\nsleep 5\n");
    let debut = std::time::Instant::now();
    let r = execution::executer(&lent, &[], Duration::from_millis(200));
    assert!(matches!(r, Err(ErreurFlux::DelaiDepasse { .. })), "{r:?}");
    assert!(debut.elapsed() < Duration::from_secs(4));

    let echec = tmp.path().join("echec");
    creer_executable(&echec, "#!/bin/sh\necho 'fichier illisible' >&2\nexit 3\n");
    match execution::executer(&echec, &[], Duration::from_secs(5)) {
        Err(ErreurFlux::Echec { code, message, .. }) => {
            assert_eq!(code, Some(3));
            assert_eq!(message, "fichier illisible");
        }
        autre => panic!("{autre:?}"),
    }

    // Un faux ffprobe qui renvoie une sortie plate est analysé de bout en bout.
    let faux = tmp.path().join("ffprobe");
    let script = format!("#!/bin/sh\ncat <<'FIN'\n{FFPROBE_AVI}FIN\n");
    creer_executable(&faux, &script);
    let video = tmp.path().join("film.avi");
    std::fs::write(&video, b"RIFF").unwrap();
    let outils = Outils {
        ffprobe: Some(faux),
        mediainfo: None,
        delai: Duration::from_secs(10),
    };
    let i = analyser_fichier(&video, &outils).unwrap();
    assert_eq!(i.video().unwrap().codec, "xvid");
}

#[test]
fn flux_analyse_erreurs_preliminaires() {
    assert!(matches!(
        analyser_fichier(Path::new("/nulle/part.mkv"), &Outils::default()),
        Err(ErreurFlux::AucunOutil)
    ));
    let outils = Outils {
        ffprobe: Some(PathBuf::from("/bin/true")),
        ..Outils::default()
    };
    assert!(matches!(
        analyser_fichier(Path::new("/nulle/part.mkv"), &outils),
        Err(ErreurFlux::FichierIntrouvable(_))
    ));
}

/// Fichier WAV PCM 16 bits stéréo de `secondes` secondes (silence).
fn ecrire_wav(chemin: &Path, secondes: u32) {
    let frequence = 8000u32;
    let canaux = 2u16;
    let octets_par_echantillon = 2u16;
    let donnees = frequence * secondes * u32::from(canaux * octets_par_echantillon);
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + donnees).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&canaux.to_le_bytes());
    b.extend_from_slice(&frequence.to_le_bytes());
    b.extend_from_slice(&(frequence * u32::from(canaux * octets_par_echantillon)).to_le_bytes());
    b.extend_from_slice(&(canaux * octets_par_echantillon).to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&donnees.to_le_bytes());
    b.resize(b.len() + donnees as usize, 0);
    std::fs::write(chemin, b).unwrap();
}

/// Exécution réelle : ignorée proprement si `ffprobe` est absent.
#[test]
fn flux_execution_reelle_ffprobe() {
    let outils = trouver_outils(&ConfigOutils::default());
    let Some(ffprobe) = outils.ffprobe.clone() else {
        eprintln!("ffprobe absent : test d'exécution réelle ignoré");
        return;
    };
    assert!(version_outil(&ffprobe).is_some_and(|v| v.contains("ffprobe")));
    let tmp = tempfile::tempdir().unwrap();
    let wav = tmp.path().join("silence.wav");
    ecrire_wav(&wav, 3);
    let seul_ffprobe = Outils {
        mediainfo: None,
        ..outils
    };
    let i = analyser_fichier(&wav, &seul_ffprobe).unwrap();
    assert_eq!(i.source, OutilAnalyse::Ffprobe);
    assert_eq!(i.conteneur, "wav");
    assert_eq!(i.duree_secondes, 3);
    assert!(i.videos.is_empty());
    assert_eq!(i.audio.len(), 1);
    assert_eq!(i.audio[0].codec, "PCM");
    assert_eq!(i.audio[0].canaux, 2);
    assert_eq!(i.audio[0].configuration_canaux, "2/0/0");
    assert_eq!(i.audio[0].profondeur_bits, 16);
}
