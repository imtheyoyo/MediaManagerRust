//! Conversion du modèle (films, séries, épisodes) en contexte de gabarit et
//! tables de jetons (raccourcis `${title}` → `movie.title`).

use std::path::Path;
use std::sync::LazyLock;

use mediamanager_gabarit::{Contexte, DateHeure, Jetons, Valeur};

use super::technique::{InfosTechniques, decimal_propre, libelle_source};
use crate::modele::{
    Episode, FichierMedia, Film, GroupeEpisodes, Note, Personne, Saga, Serie, StatutDiffusion,
    TypeFichier, TypeGroupe, TypePersonne,
};

/// Jetons des films : (jeton, expression).
pub const JETONS_FILM: &[(&str, &str)] = &[
    ("title", "movie.title"),
    ("originalTitle", "movie.originalTitle"),
    ("englishTitle", "movie.englishTitle"),
    ("originalFilename", "movie.originalFilename"),
    ("originalBasename", "movie.originalBasename"),
    ("sorttitle", "movie.sortTitle"),
    ("titleSortable", "movie.titleSortable"),
    ("year", "movie.year"),
    ("releaseDate", "movie.releaseDate;date(yyyy-MM-dd)"),
    ("decadeShort", "movie.decadeShort"),
    ("decadeLong", "movie.decadeLong"),
    ("rating", "movie.rating.rating"),
    ("imdb", "movie.imdbId"),
    ("tmdb", "movie.tmdbId"),
    ("certification", "movie.certification"),
    ("language", "movie.spokenLanguages"),
    ("genres", "movie.genres"),
    ("genresAsString", "movie.genresAsString"),
    ("tags", "movie.tags"),
    ("tagsAsString", "movie.tagsAsString"),
    ("actors", "movie.actors"),
    ("producers", "movie.producers"),
    ("directors", "movie.directors"),
    ("writers", "movie.writers"),
    ("productionCompany", "movie.productionCompany"),
    ("productionCompanyAsArray", "movie.productionCompanyAsArray"),
    ("videoCodec", "movie.mediaInfoVideoCodec"),
    ("videoFormat", "movie.mediaInfoVideoFormat"),
    ("aspectRatio", "movie.mediaInfoAspectRatioAsString"),
    ("aspectRatio2", "movie.mediaInfoAspectRatio2AsString"),
    ("videoResolution", "movie.mediaInfoVideoResolution"),
    ("videoBitDepth", "movie.mediaInfoVideoBitDepth"),
    ("videoBitRate", "movie.mediaInfoVideoBitrate;bitrate"),
    ("framerate", "movie.mediaInfoFrameRate;framerate"),
    ("audioCodec", "movie.mediaInfoAudioCodec"),
    ("audioCodecList", "movie.mediaInfoAudioCodecList"),
    ("audioCodecsAsString", "movie.mediaInfoAudioCodecList;array"),
    ("audioChannels", "movie.mediaInfoAudioChannels"),
    ("audioChannelList", "movie.mediaInfoAudioChannelList"),
    (
        "audioChannelsAsString",
        "movie.mediaInfoAudioChannelList;array",
    ),
    ("audioChannelsDot", "movie.mediaInfoAudioChannelsDot"),
    ("audioChannelDotList", "movie.mediaInfoAudioChannelDotList"),
    (
        "audioChannelsDotAsString",
        "movie.mediaInfoAudioChannelDotList;array",
    ),
    ("audioLanguage", "movie.mediaInfoAudioLanguage"),
    ("audioLanguageList", "movie.mediaInfoAudioLanguageList"),
    (
        "audioLanguagesAsString",
        "movie.mediaInfoAudioLanguageList;array",
    ),
    (
        "subtitleLanguageList",
        "movie.mediaInfoSubtitleLanguageList",
    ),
    (
        "subtitleLanguagesAsString",
        "movie.mediaInfoSubtitleLanguageList;array",
    ),
    ("3Dformat", "movie.video3DFormat"),
    ("3Dformat2", "movie.video3DFormat2"),
    ("hdr", "movie.videoHDR"),
    ("hdrformat", "movie.videoHDRFormat"),
    ("filesize", "movie.videoFilesize;filesize"),
    ("mediaSource", "movie.mediaSource"),
    ("edition", "movie.edition"),
    ("parent", "movie.parent"),
    ("note", "movie.note"),
    ("movieSetIndex", "movie.movieSetIndex"),
    ("movieSetIndex2", "movie.movieSetIndex2"),
    ("crc32", "movie.CRC32"),
];

/// Jetons des séries et épisodes : (jeton, expression).
pub const JETONS_SERIE: &[(&str, &str)] = &[
    ("showTitle", "tvShow.title"),
    ("showOriginalTitle", "tvShow.originalTitle"),
    ("showEnglishTitle", "tvShow.englishTitle"),
    ("showTitleSortable", "tvShow.titleSortable"),
    ("showTagline", "tvShow.tagline"),
    ("showYear", "tvShow.year"),
    ("showYearEnd", "tvShow.yearEnd"),
    ("parent", "tvShow.parent"),
    ("showNote", "tvShow.note"),
    ("showStatus", "tvShow.status"),
    ("showRating", "tvShow.rating.rating"),
    ("showImdb", "tvShow.imdbId"),
    ("showTmdb", "tvShow.tmdbId"),
    ("showTvdb", "tvShow.tvdbId"),
    ("showCertification", "tvShow.certification"),
    ("showTags", "tvShow.tags"),
    ("showGenres", "tvShow.genres"),
    ("showGenresAsString", "tvShow.genresAsString"),
    ("showProductionCompany", "tvShow.productionCompany"),
    (
        "showProductionCompanyAsArray",
        "tvShow.productionCompanyAsArray",
    ),
    ("seasonName", "season.title"),
    ("seasonNr", "episode.season;number(%d)"),
    ("seasonNr2", "episode.season;number(%02d)"),
    ("seasonNrAired", "episode.airedSeason;number(%d)"),
    ("seasonNrAired2", "episode.airedSeason;number(%02d)"),
    ("seasonNrDvd", "episode.dvdSeason;number(%d)"),
    ("seasonNrDvd2", "episode.dvdSeason;number(%02d)"),
    ("episodeNr", "episode.episode;number(%d)"),
    ("episodeNr2", "episode.episode;number(%02d)"),
    ("episodeNrAired", "episode.airedEpisode;number(%d)"),
    ("episodeNrAired2", "episode.airedEpisode;number(%02d)"),
    ("episodeNrDvd", "episode.dvdEpisode;number(%d)"),
    ("episodeNrDvd2", "episode.dvdEpisode;number(%02d)"),
    ("absoluteNr", "episode.absoluteNumber;number(%d)"),
    ("absoluteNr2", "episode.absoluteNumber;number(%02d)"),
    ("title", "episode.title"),
    ("originalTitle", "episode.originalTitle"),
    ("englishTitle", "episode.englishTitle"),
    ("titleSortable", "episode.titleSortable"),
    ("year", "episode.year"),
    ("airedDate", "episode.firstAired;date(yyyy-MM-dd)"),
    ("episodeRating", "episode.rating.rating"),
    ("episodeImdb", "episode.imdbId"),
    ("episodeTmdb", "episode.tmdbId"),
    ("episodeTvdb", "episode.tvdbId"),
    ("episodeTags", "episode.tags"),
    ("note", "episode.note"),
    ("originalFilename", "episode.originalFilename"),
    ("originalBasename", "episode.originalBasename"),
    ("folderName", "episode.folderName"),
    ("filesize", "episode.videoFilesize;filesize"),
    ("mediaSource", "episode.mediaSource"),
    ("crc32", "episode.CRC32"),
    ("videoCodec", "episode.mediaInfoVideoCodec"),
    ("videoFormat", "episode.mediaInfoVideoFormat"),
    ("videoResolution", "episode.mediaInfoVideoResolution"),
    ("aspectRatio", "episode.mediaInfoAspectRatioAsString"),
    ("aspectRatio2", "episode.mediaInfoAspectRatio2AsString"),
    ("videoBitDepth", "episode.mediaInfoVideoBitDepth"),
    ("videoBitRate", "episode.mediaInfoVideoBitrate;bitrate"),
    ("framerate", "episode.mediaInfoFrameRate;framerate"),
    ("3Dformat", "episode.video3DFormat"),
    ("hdr", "episode.videoHDR"),
    ("hdrformat", "episode.videoHDRFormat"),
    ("audioCodec", "episode.mediaInfoAudioCodec"),
    ("audioCodecList", "episode.mediaInfoAudioCodecList"),
    (
        "audioCodecAsString",
        "episode.mediaInfoAudioCodecList;array",
    ),
    (
        "audioCodecsAsString",
        "episode.mediaInfoAudioCodecList;array",
    ),
    ("audioChannels", "episode.mediaInfoAudioChannels"),
    ("audioChannelList", "episode.mediaInfoAudioChannelList"),
    (
        "audioChannelsAsString",
        "episode.mediaInfoAudioChannelList;array",
    ),
    ("audioChannelsDot", "episode.mediaInfoAudioChannelsDot"),
    (
        "audioChannelDotList",
        "episode.mediaInfoAudioChannelDotList",
    ),
    (
        "audioChannelsDotAsString",
        "episode.mediaInfoAudioChannelDotList;array",
    ),
    ("audioLanguage", "episode.mediaInfoAudioLanguage"),
    ("audioLanguageList", "episode.mediaInfoAudioLanguageList"),
    (
        "audioLanguageAsString",
        "episode.mediaInfoAudioLanguageList;array",
    ),
    (
        "audioLanguagesAsString",
        "episode.mediaInfoAudioLanguageList;array",
    ),
    (
        "subtitleLanguageList",
        "episode.mediaInfoSubtitleLanguageList",
    ),
    (
        "subtitleLanguageAsString",
        "episode.mediaInfoSubtitleLanguageList;array",
    ),
    (
        "subtitleLanguagesAsString",
        "episode.mediaInfoSubtitleLanguageList;array",
    ),
];

static TABLE_JETONS_FILM: LazyLock<Jetons> = LazyLock::new(|| {
    Jetons::depuis_paires(JETONS_FILM.iter().copied()).expect("jetons de films valides")
});
static TABLE_JETONS_SERIE: LazyLock<Jetons> = LazyLock::new(|| {
    Jetons::depuis_paires(JETONS_SERIE.iter().copied()).expect("jetons de séries valides")
});

/// Table des jetons des films.
pub fn jetons_film() -> &'static Jetons {
    &TABLE_JETONS_FILM
}

/// Table des jetons des séries.
pub fn jetons_serie() -> &'static Jetons {
    &TABLE_JETONS_SERIE
}

/// Titre triable : « The Matrix » ⇒ « Matrix, The » (préfixes sensibles à la
/// casse ; exceptions « Die Hard » et « Die Another Day »).
pub fn titre_triable(titre: &str, prefixes: &[String]) -> String {
    let t = titre.trim();
    if t.starts_with("Die Hard") || t.starts_with("Die Another Day") {
        return t.to_string();
    }
    for p in prefixes {
        if p.is_empty() {
            continue;
        }
        let colle = p.ends_with(['\'', '´', '`']);
        if colle {
            if let Some(reste) = t.strip_prefix(p.as_str())
                && !reste.trim().is_empty()
            {
                return format!("{}, {}", reste.trim(), p);
            }
        } else if let Some(reste) = t.strip_prefix(&format!("{p} "))
            && !reste.trim().is_empty()
        {
            return format!("{}, {}", reste.trim(), p);
        }
    }
    t.to_string()
}

fn personnes(liste: &[&Personne]) -> Valeur {
    Valeur::Liste(
        liste
            .iter()
            .map(|p| {
                Contexte::nouveau()
                    .avec("name", p.nom.as_str())
                    .avec("role", p.role.as_str())
                    .into()
            })
            .collect(),
    )
}

fn note(n: Option<&Note>) -> Valeur {
    let n = n.cloned().unwrap_or_default();
    let valeur = if n.valeur > 0.0 {
        decimal_propre(n.valeur)
    } else {
        0.0
    };
    Contexte::nouveau()
        .avec("rating", valeur)
        .avec("votes", n.votes)
        .avec("maxValue", n.maximum)
        .into()
}

fn date(texte: Option<&String>) -> Valeur {
    match texte {
        Some(t) if !t.trim().is_empty() => DateHeure::depuis_iso(t)
            .map(Valeur::Date)
            .unwrap_or_else(|| Valeur::Texte(t.clone())),
        _ => Valeur::Vide,
    }
}

fn liste_textes(l: &[String]) -> Valeur {
    Valeur::Liste(l.iter().map(|s| Valeur::Texte(s.clone())).collect())
}

fn societes(s: &str) -> Valeur {
    Valeur::Liste(
        s.split(" / ")
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(|x| Valeur::Texte(x.to_string()))
            .collect(),
    )
}

/// Chemin parent relatif à la source (séparateur `/`).
fn parent_relatif(chemin: &Path, source: &Path) -> String {
    chemin
        .parent()
        .and_then(|p| p.strip_prefix(source).ok())
        .map(|p| {
            p.components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_default()
}

fn inserer_technique(c: &mut Contexte, t: &InfosTechniques) {
    c.inserer("mediaInfoVideoCodec", t.codec_video.as_str());
    c.inserer("mediaInfoVideoFormat", t.format_video.as_str());
    c.inserer("mediaInfoVideoResolution", t.resolution.as_str());
    c.inserer("mediaInfoAspectRatioAsString", t.ratio.as_str());
    c.inserer("mediaInfoAspectRatio2AsString", t.ratio2.as_str());
    c.inserer("mediaInfoVideoBitDepth", t.profondeur_bits);
    c.inserer("mediaInfoVideoBitrate", t.debit_video);
    c.inserer("mediaInfoFrameRate", t.images_par_seconde);
    c.inserer("video3DFormat", t.format_3d.as_str());
    c.inserer("video3DFormat2", t.format_3d.replace(' ', ""));
    c.inserer("videoHDR", t.hdr.as_str());
    c.inserer("videoHDRFormat", t.format_hdr.as_str());
    c.inserer("mediaInfoAudioCodec", t.codec_audio.as_str());
    c.inserer("mediaInfoAudioCodecList", liste_textes(&t.codecs_audio));
    c.inserer("mediaInfoAudioChannels", t.canaux.as_str());
    c.inserer("mediaInfoAudioChannelList", liste_textes(&t.liste_canaux));
    c.inserer("mediaInfoAudioChannelsDot", t.canaux_point.as_str());
    c.inserer(
        "mediaInfoAudioChannelDotList",
        liste_textes(&t.liste_canaux_point),
    );
    c.inserer("mediaInfoAudioLanguage", t.langue_audio.as_str());
    c.inserer("mediaInfoAudioLanguageList", liste_textes(&t.langues_audio));
    c.inserer(
        "mediaInfoSubtitleLanguageList",
        liste_textes(&t.langues_sous_titres),
    );
    c.inserer(
        "videoFilesize",
        i64::try_from(t.taille_videos).unwrap_or(i64::MAX),
    );
    c.inserer("CRC32", t.crc32.as_str());
}

fn nom_fichier_origine(origine: &str, principal: Option<&FichierMedia>) -> (String, String) {
    let complet = if origine.is_empty() {
        principal.map(|f| f.nom.clone()).unwrap_or_default()
    } else {
        origine.to_string()
    };
    let base = crate::modele::fichier::nom_de_base(&complet);
    (complet, base)
}

/// Contexte `movie` (et `movieSet`) d'un film.
///
/// `movieSet` n'est présent que si la saga existe et compte plus d'un film
/// (ou si `saga_pour_film_seul`).
pub fn contexte_film(
    film: &Film,
    saga: Option<&Saga>,
    saga_pour_film_seul: bool,
    prefixes_titre: &[String],
) -> Contexte {
    let principal = film.fichier_principal();
    let tech = InfosTechniques::depuis(principal, &film.fichiers);
    let (origine, origine_base) = nom_fichier_origine(&film.nom_fichier_origine, principal);
    let mut m = Contexte::nouveau();
    m.inserer("title", film.titre.as_str());
    m.inserer("originalTitle", film.titre_original.as_str());
    m.inserer("englishTitle", film.titre_anglais.as_str());
    m.inserer("originalFilename", origine);
    m.inserer("originalBasename", origine_base);
    m.inserer("sortTitle", film.titre_tri.as_str());
    m.inserer("titleSortable", titre_triable(&film.titre, prefixes_titre));
    m.inserer("year", film.annee);
    m.inserer("releaseDate", date(film.date_sortie.as_ref()));
    m.inserer("decadeShort", film.decennie_courte());
    m.inserer("decadeLong", film.decennie_longue());
    m.inserer("rating", note(film.note_principale()));
    m.inserer("imdbId", film.ids.imdb());
    m.inserer("tmdbId", film.ids.tmdb());
    m.inserer("certification", film.certification.as_str());
    m.inserer("spokenLanguages", film.langues_parlees.as_str());
    m.inserer("country", film.pays.as_str());
    m.inserer("genres", liste_textes(&film.genres));
    m.inserer("genresAsString", film.genres.join(", "));
    m.inserer("tags", liste_textes(&film.tags));
    m.inserer("tagsAsString", film.tags.join(", "));
    let acteurs: Vec<&Personne> = film.acteurs.iter().collect();
    m.inserer("actors", personnes(&acteurs));
    m.inserer("producers", personnes(&film.producteurs()));
    m.inserer("directors", personnes(&film.realisateurs()));
    m.inserer("writers", personnes(&film.scenaristes()));
    m.inserer("productionCompany", film.societe_production.as_str());
    m.inserer(
        "productionCompanyAsArray",
        societes(&film.societe_production),
    );
    inserer_technique(&mut m, &tech);
    m.inserer("mediaSource", libelle_source(film.source_media));
    m.inserer("edition", film.edition.libelle());
    m.inserer("parent", parent_relatif(&film.chemin, &film.source_donnees));
    m.inserer("note", film.note_utilisateur.as_str());
    m.inserer("plot", film.resume.as_str());
    m.inserer("tagline", film.slogan.as_str());
    m.inserer("runtime", film.duree_minutes);
    m.inserer("top250", film.top250);
    m.inserer("watched", film.vu);
    m.inserer("video3D", film.video_3d);
    m.inserer("disc", film.est_disque);
    m.inserer("stacked", film.empile);
    m.inserer("multiMovieDir", film.dossier_multi_films);

    let mut c = Contexte::nouveau();
    if let Some(s) = saga {
        let position = s.films.iter().position(|id| id == &film.id);
        if let Some(p) = position {
            m.inserer("movieSetIndex", p + 1);
            m.inserer("movieSetIndex2", p + 1);
        }
        if s.films.len() > 1 || saga_pour_film_seul {
            c.inserer(
                "movieSet",
                Contexte::nouveau()
                    .avec("title", s.titre.as_str())
                    .avec("originalTitle", s.titre_original.as_str())
                    .avec("titleSortable", titre_triable(&s.titre, prefixes_titre))
                    .avec("plot", s.resume.as_str())
                    .avec("tmdbId", s.ids.tmdb()),
            );
        }
    }
    c.inserer("movie", m);
    c
}

fn libelle_statut(s: StatutDiffusion) -> &'static str {
    match s {
        StatutDiffusion::Inconnu => "",
        StatutDiffusion::EnCours => "Continuing",
        StatutDiffusion::Terminee => "Ended",
        StatutDiffusion::Annulee => "Canceled",
    }
}

/// Contexte `tvShow` d'une série.
pub fn contexte_serie(serie: &Serie, prefixes_titre: &[String]) -> Contexte {
    let mut s = Contexte::nouveau();
    s.inserer("title", serie.titre.as_str());
    s.inserer("originalTitle", serie.titre_original.as_str());
    s.inserer("englishTitle", serie.titre_anglais.as_str());
    s.inserer("titleSortable", titre_triable(&serie.titre, prefixes_titre));
    s.inserer("tagline", serie.slogan.as_str());
    s.inserer("year", serie.annee);
    s.inserer("yearEnd", serie.annee_fin);
    s.inserer(
        "parent",
        parent_relatif(&serie.chemin, &serie.source_donnees),
    );
    s.inserer("note", serie.note_utilisateur.as_str());
    s.inserer("status", libelle_statut(serie.statut));
    let n = ["tmdb", "imdb", "tvdb", "trakt"]
        .iter()
        .find_map(|k| serie.notes.get(*k))
        .or_else(|| serie.notes.values().next());
    s.inserer("rating", note(n));
    s.inserer("imdbId", serie.ids.imdb());
    s.inserer("tmdbId", serie.ids.tmdb());
    s.inserer("tvdbId", serie.ids.tvdb());
    s.inserer("certification", serie.certification.as_str());
    s.inserer("tags", liste_textes(&serie.tags));
    s.inserer("genres", liste_textes(&serie.genres));
    s.inserer("genresAsString", serie.genres.join(", "));
    s.inserer("productionCompany", serie.societe_production.as_str());
    s.inserer(
        "productionCompanyAsArray",
        societes(&serie.societe_production),
    );
    Contexte::nouveau().avec("tvShow", s)
}

fn numero(ep: &Episode, groupe: &GroupeEpisodes) -> Option<(i32, i32)> {
    ep.numero_dans(groupe).map(|n| (n.saison, n.episode))
}

fn numero_type(ep: &Episode, t: TypeGroupe) -> Option<(i32, i32)> {
    ep.numeros
        .iter()
        .find(|n| n.groupe.type_groupe == t)
        .map(|n| (n.saison, n.episode))
}

/// Contexte complet (`tvShow`, `season`, `episode`) d'un épisode.
pub fn contexte_episode(serie: &Serie, ep: &Episode, prefixes_titre: &[String]) -> Contexte {
    let mut c = contexte_serie(serie, prefixes_titre);
    let (saison, episode) = numero(ep, &serie.groupe_actif).unwrap_or((-1, -1));
    let principal = principal_episode(ep);
    let tech = InfosTechniques::depuis(principal, &ep.fichiers);
    let (origine, origine_base) = nom_fichier_origine(&ep.nom_fichier_origine, principal);
    let mut e = Contexte::nouveau();
    e.inserer("title", ep.titre.as_str());
    e.inserer("originalTitle", ep.titre_original.as_str());
    e.inserer("englishTitle", ep.titre_anglais.as_str());
    e.inserer("titleSortable", titre_triable(&ep.titre, prefixes_titre));
    e.inserer("season", saison);
    e.inserer("episode", episode);
    if let Some((s, n)) = numero_type(ep, TypeGroupe::Diffusion) {
        e.inserer("airedSeason", s);
        e.inserer("airedEpisode", n);
    }
    if let Some((s, n)) = numero_type(ep, TypeGroupe::Dvd) {
        e.inserer("dvdSeason", s);
        e.inserer("dvdEpisode", n);
    }
    if let Some((_, n)) = numero_type(ep, TypeGroupe::Absolu) {
        e.inserer("absoluteNumber", n);
    }
    let annee = ep
        .premiere_diffusion
        .as_deref()
        .and_then(|d| d.get(0..4))
        .and_then(|a| a.parse::<u32>().ok())
        .unwrap_or(0);
    e.inserer("year", annee);
    e.inserer("firstAired", date(ep.premiere_diffusion.as_ref()));
    let n = ["tmdb", "imdb", "tvdb", "trakt"]
        .iter()
        .find_map(|k| ep.notes.get(*k))
        .or_else(|| ep.notes.values().next());
    e.inserer("rating", note(n));
    e.inserer("imdbId", ep.ids.imdb());
    e.inserer("tmdbId", ep.ids.tmdb());
    e.inserer("tvdbId", ep.ids.tvdb());
    e.inserer("tags", liste_textes(&ep.tags));
    e.inserer("note", ep.note_utilisateur.as_str());
    e.inserer("originalFilename", origine);
    e.inserer("originalBasename", origine_base);
    e.inserer(
        "folderName",
        serie
            .chemin
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    );
    e.inserer("mediaSource", libelle_source(ep.source_media));
    e.inserer("edition", ep.edition.libelle());
    e.inserer("plot", ep.resume.as_str());
    let invites: Vec<&Personne> = ep.invites.iter().collect();
    e.inserer("guests", personnes(&invites));
    let realisateurs: Vec<&Personne> = ep
        .equipe
        .iter()
        .filter(|p| p.type_personne == TypePersonne::Realisateur)
        .collect();
    e.inserer("directors", personnes(&realisateurs));
    inserer_technique(&mut e, &tech);
    c.inserer("episode", e);
    let titre_saison = serie
        .saisons
        .iter()
        .find(|s| s.numero == saison)
        .map(|s| s.titre.clone())
        .unwrap_or_default();
    c.inserer(
        "season",
        Contexte::nouveau()
            .avec("title", titre_saison)
            .avec("season", saison),
    );
    c
}

/// Fichier vidéo principal d'un épisode (plus petite pile, sinon le plus gros).
pub fn principal_episode(ep: &Episode) -> Option<&FichierMedia> {
    let videos: Vec<&FichierMedia> = ep
        .fichiers
        .iter()
        .filter(|f| f.type_fichier == TypeFichier::Video)
        .collect();
    if videos.iter().any(|v| v.empilement > 0) {
        return videos
            .iter()
            .filter(|v| v.empilement > 0)
            .min_by_key(|v| v.empilement)
            .copied();
    }
    videos.into_iter().max_by_key(|v| v.taille)
}
