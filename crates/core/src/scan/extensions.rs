//! Listes d'extensions de fichiers (données du programme d'origine).

/// Extensions vidéo par défaut (avec le point initial).
pub const EXTENSIONS_VIDEO: &[&str] = &[
    ".3gp", ".asf", ".asx", ".avc", ".avi", ".bdmv", ".bin", ".bivx", ".braw", ".dat", ".divx",
    ".dv", ".dvr-ms", ".disc", ".evo", ".fli", ".flv", ".h264", ".ifo", ".img", ".iso", ".mts",
    ".mt2s", ".m2ts", ".m2v", ".m4v", ".mkv", ".mk3d", ".mov", ".mp4", ".mpeg", ".mpg", ".nrg",
    ".nsv", ".nuv", ".ogm", ".pva", ".qt", ".rm", ".rmvb", ".strm", ".svq3", ".ts", ".ty", ".viv",
    ".vob", ".vp3", ".wmv", ".webm", ".xvid",
];

/// Extensions audio par défaut.
pub const EXTENSIONS_AUDIO: &[&str] = &[
    ".a52", ".aa3", ".aac", ".ac3", ".adt", ".adts", ".aif", ".aiff", ".alac", ".ape", ".at3",
    ".atrac", ".au", ".dts", ".dtshd", ".flac", ".m4a", ".m4b", ".m4p", ".mid", ".midi", ".mka",
    ".mp3", ".mpa", ".mlp", ".oga", ".ogg", ".pcm", ".ra", ".ram", ".tta", ".thd", ".wav", ".wave",
    ".wma",
];

/// Extensions de sous-titres par défaut.
pub const EXTENSIONS_SOUS_TITRES: &[&str] = &[
    ".aqt", ".cvd", ".dks", ".jss", ".sub", ".sup", ".ttxt", ".mpl", ".pjs", ".psb", ".rt", ".srt",
    ".smi", ".ssf", ".ssa", ".svcd", ".usf", ".ass", ".pgs", ".vobsub", ".vtt",
];

/// Extensions d'images (sans le point).
pub const EXTENSIONS_IMAGE: &[&str] = &["jpg", "jpeg", "png", "tbn", "gif", "bmp", "webp"];

/// Extensions « binaires » (conteneurs opaques) : bin dat img nrg disc.
pub const EXTENSIONS_BINAIRES: &[&str] = &["bin", "dat", "img", "nrg", "disc"];

/// Motifs de nettoyage par défaut (expressions rationnelles sur le nom).
pub const MOTIFS_NETTOYAGE: &[&str] = &[r".txt$", r".url$", r".html$", r".sfv$"];

fn dans(liste: &[&str], ext: &str) -> bool {
    liste.iter().any(|e| e.trim_start_matches('.') == ext)
}

/// `ext` : extension en minuscules, sans point.
pub fn est_video(ext: &str) -> bool {
    dans(EXTENSIONS_VIDEO, ext)
}

pub fn est_audio(ext: &str) -> bool {
    dans(EXTENSIONS_AUDIO, ext)
}

pub fn est_sous_titres(ext: &str) -> bool {
    dans(EXTENSIONS_SOUS_TITRES, ext)
}

pub fn est_image(ext: &str) -> bool {
    EXTENSIONS_IMAGE.contains(&ext)
}

/// Extension connue comme extension de fichier (tous types confondus).
pub fn est_extension_connue(ext: &str) -> bool {
    est_video(ext)
        || est_audio(ext)
        || est_sous_titres(ext)
        || est_image(ext)
        || matches!(
            ext,
            "nfo" | "txt" | "xml" | "vsmeta" | "json" | "bif" | "idx"
        )
}
