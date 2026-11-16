//! NFO d'épisodes (racine `<episodedetails>`, un bloc par épisode : un
//! fichier multi-épisodes contient plusieurs blocs concaténés, la
//! déclaration XML n'étant écrite qu'avant le premier).

use std::path::Path;

use crate::modele::commun::cles_id;
use crate::modele::{
    Episode, FichierMedia, GroupeEpisodes, NumeroEpisode, Serie, TypeFichier, TypePersonne,
};

use super::arbre::{Element, decoder_octets, racines_nommees};
use super::commun::*;
use super::outils::{
    GenreMedia, booleen, completer_ids_depuis_texte, date_vers_ms, entier_en_tete,
    ms_vers_date_heure, normaliser_date, resume_court,
};
use super::serie::{ajouter_notes, nom_type_groupe, profil_serie, type_groupe_depuis};

/// Un bloc `<episodedetails>` lu.
#[derive(Debug, Clone, PartialEq)]
pub struct EpisodeLu {
    pub episode: Episode,
    pub conserves: DonneesConservees,
}

/// Résultat de la lecture d'un NFO d'épisode(s).
#[derive(Debug, Clone, PartialEq)]
pub struct EpisodesLus {
    /// Épisodes dans l'ordre du fichier.
    pub episodes: Vec<EpisodeLu>,
    /// Au moins un bloc avec titre ; s'il y a plusieurs blocs, le premier
    /// doit avoir un numéro d'épisode ≥ 0.
    pub valide: bool,
}

const BALISES_EPISODE: &[&str] = &[
    "title",
    "originaltitle",
    "showtitle",
    "season",
    "episode",
    "displayseason",
    "displayepisode",
    "id",
    "uniqueid",
    "imdb",
    "imdbid",
    "tmdbid",
    "tvdbid",
    "ids",
    "rating",
    "ratings",
    "userrating",
    "criticrating",
    "votes",
    "plot",
    "outline",
    "runtime",
    "thumb",
    "mpaa",
    "certification",
    "premiered",
    "aired",
    "watched",
    "playcount",
    "lastplayed",
    "studio",
    "tag",
    "credits",
    "director",
    "producer",
    "actor",
    "trailer",
    "dateadded",
    "lockdata",
    "fileinfo",
    "epbookmark",
    "code",
    "videoassettitle",
    "hasvideoversions",
    "source",
    "edition",
    "original_filename",
    "user_note",
    "episode_groups",
    "english_title",
    "crc32",
    "crew",
    "status",
    "set",
    "locked",
    "tmm_locked",
];

// ---------------------------------------------------------------------------
// Lecture
// ---------------------------------------------------------------------------

/// Lit un NFO d'épisode(s) depuis ses octets.
pub fn lire_nfo_episodes_octets(octets: &[u8]) -> EpisodesLus {
    lire_nfo_episodes(&decoder_octets(octets))
}

/// Lit un fichier NFO d'épisode(s).
pub fn lire_fichier_nfo_episodes(chemin: &Path) -> std::io::Result<EpisodesLus> {
    Ok(lire_nfo_episodes_octets(&std::fs::read(chemin)?))
}

/// Lit un NFO d'épisode(s) : blocs `<episodedetails>` (ou `<recording>`),
/// éventuellement entourés de texte. Sans aucun bloc, un épisode ne portant
/// que les identifiants trouvés dans le texte est renvoyé (s'il y en a).
pub fn lire_nfo_episodes(texte: &str) -> EpisodesLus {
    let racines = racines_nommees(texte, &["episodedetails", "recording"]);
    let mut episodes: Vec<EpisodeLu> = racines
        .iter()
        .map(|r| {
            if r.a_nom("recording") {
                lire_recording(r)
            } else {
                lire_bloc(r)
            }
        })
        .collect();
    if episodes.is_empty() {
        let ids = super::outils::ids_depuis_texte(texte, GenreMedia::Serie);
        if !ids.est_vide() {
            episodes.push(EpisodeLu {
                episode: Episode {
                    ids,
                    ..Episode::default()
                },
                conserves: DonneesConservees::default(),
            });
        }
    } else if episodes.len() == 1 {
        let ep = &mut episodes[0].episode;
        if ep.ids.imdb().is_empty() || ep.ids.tvdb() == 0 {
            completer_ids_depuis_texte(&mut ep.ids, texte, GenreMedia::Serie);
        }
    }
    let multi = episodes.len() > 1;
    for e in &mut episodes {
        e.episode.multi_episodes = multi;
    }
    let diffusion = GroupeEpisodes::diffusion();
    let valide = episodes.iter().any(|e| !e.episode.titre.is_empty())
        && (!multi || episodes[0].episode.episode(&diffusion) >= 0);
    EpisodesLus { episodes, valide }
}

fn lire_recording(r: &Element) -> EpisodeLu {
    let episode = Episode {
        titre: r
            .texte_unique("subtitle")
            .or_else(|| r.texte_unique("title"))
            .unwrap_or_default(),
        resume: r.texte_complet_unique("description").unwrap_or_default(),
        premiere_diffusion: r
            .texte_unique("original_air_date")
            .and_then(|t| normaliser_date(&t)),
        ..Episode::default()
    };
    EpisodeLu {
        episode,
        conserves: DonneesConservees::default(),
    }
}

fn entier(r: &Element, balise: &str) -> Option<i32> {
    r.texte_unique(balise)
        .and_then(|t| entier_en_tete(&t))
        .map(|n| n as i32)
}

fn lire_numeros(r: &Element) -> Vec<NumeroEpisode> {
    let mut numeros = Vec::new();
    if let Some(bloc) = r.enfant_unique("episode_groups") {
        for g in bloc.enfants_nommes("group") {
            let Some(t) = g.attribut("id").and_then(type_groupe_depuis) else {
                continue;
            };
            let saison = g.attribut("season").and_then(entier_en_tete).unwrap_or(-1) as i32;
            let episode = g.attribut("episode").and_then(entier_en_tete).unwrap_or(-1) as i32;
            numeros.push(NumeroEpisode {
                groupe: GroupeEpisodes {
                    type_groupe: t,
                    nom: g.attribut("name").unwrap_or("").to_string(),
                },
                saison,
                episode,
            });
        }
    }
    if !numeros.is_empty() {
        return numeros;
    }
    let (s, e) = (entier(r, "season"), entier(r, "episode"));
    if s.is_some() || e.is_some() {
        numeros.push(NumeroEpisode {
            groupe: GroupeEpisodes::diffusion(),
            saison: s.unwrap_or(-1),
            episode: e.unwrap_or(-1),
        });
    }
    if let (Some(s), Some(e)) = (entier(r, "displayseason"), entier(r, "displayepisode"))
        && s >= 0
        && e >= 0
    {
        numeros.push(NumeroEpisode {
            groupe: GroupeEpisodes::affichage(),
            saison: s,
            episode: e,
        });
    }
    numeros
}

fn lire_bloc(r: &Element) -> EpisodeLu {
    let mut e = Episode {
        numeros: lire_numeros(r),
        titre: r.texte_unique("title").unwrap_or_default(),
        titre_original: r.texte_unique("originaltitle").unwrap_or_default(),
        titre_anglais: r.texte_unique("english_title").unwrap_or_default(),
        resume: r
            .texte_complet_unique("plot")
            .or_else(|| r.texte_complet_unique("outline"))
            .unwrap_or_default(),
        note_utilisateur: r.texte_complet_unique("user_note").unwrap_or_default(),
        nom_fichier_origine: r.texte_unique("original_filename").unwrap_or_default(),
        ..Episode::default()
    };
    if let Some(n) = r.texte_unique("runtime").and_then(|t| entier_en_tete(&t)) {
        e.duree_minutes = n.max(0) as u32;
    }
    e.notes = lire_notes(r);

    // Identifiants : `id` = tvdb (ou imdb s'il en a la forme).
    if let Some(v) = r.texte_unique("id") {
        if crate::modele::est_id_imdb_valide(&v) {
            e.ids.definir(cles_id::IMDB, &v);
        } else {
            e.ids.definir(cles_id::TVDB, &v);
        }
    }
    for b in ["imdb", "imdbid"] {
        if let Some(v) = r.texte_unique(b) {
            e.ids.definir_si_absent(cles_id::IMDB, &v);
        }
    }
    if let Some(v) = r.texte_unique("tmdbid") {
        e.ids.definir(cles_id::TMDB, &v);
    }
    if let Some(v) = r.texte_unique("tvdbid") {
        e.ids.definir(cles_id::TVDB, &v);
    }
    lire_uniqueids(r, &mut e.ids, Some(cles_id::TVDB));
    lire_bloc_ids(r, &mut e.ids);

    // Date : `aired` puis `premiered`.
    e.premiere_diffusion = ["aired", "premiered"]
        .iter()
        .find_map(|b| r.texte_unique(b).and_then(|t| normaliser_date(&t)));

    e.vu = r.texte_unique("watched").is_some_and(|v| booleen(&v));
    if let Some(n) = r.texte_unique("playcount").and_then(|t| entier_en_tete(&t))
        && n > 0
    {
        e.nombre_lectures = n as u32;
        e.vu = true;
    }
    e.derniere_lecture = r.texte_unique("lastplayed").and_then(|t| date_vers_ms(&t));
    if let Some(ms) = r.texte_unique("dateadded").and_then(|t| date_vers_ms(&t)) {
        e.date_ajout = ms;
    }
    e.tags = r
        .enfants_nommes("tag")
        .map(Element::texte_propre)
        .filter(|t| !t.is_empty())
        .collect();
    // Invités : les acteurs explicitement typés « Actor » sont ceux de la
    // série (écrits avec l'option « tous les acteurs ») et sont ignorés.
    e.invites = r
        .enfants_nommes("actor")
        .filter(|a| {
            a.texte_unique("type")
                .is_none_or(|t| type_personne_depuis(&t) != TypePersonne::Acteur)
        })
        .filter_map(|a| lire_personne(a, TypePersonne::Invite))
        .collect();
    e.equipe = lire_equipe(r);
    if let Some(s) = r.texte_unique("source") {
        e.source_media = source_depuis(&s);
    }
    if let Some(x) = r.texte_unique("edition") {
        e.edition = edition_depuis(&x);
    }
    let conserves = DonneesConservees {
        epbookmark: r.texte_unique("epbookmark"),
        status: None,
        code: r.texte_unique("code"),
        inconnues: elements_inconnus(r, BALISES_EPISODE),
    };
    EpisodeLu {
        episode: e,
        conserves,
    }
}

// ---------------------------------------------------------------------------
// Écriture
// ---------------------------------------------------------------------------

fn video_principale(e: &Episode) -> Option<&FichierMedia> {
    e.fichiers
        .iter()
        .find(|f| f.type_fichier == TypeFichier::Video)
}

/// Construit un bloc `<episodedetails>`.
pub fn element_nfo_episode(
    serie: &Serie,
    episode: &Episode,
    conserves: &DonneesConservees,
    connecteur: Connecteur,
    options: &OptionsNfo,
) -> Element {
    let p = profil_serie(connecteur);
    let mut r = Element::nouveau("episodedetails");
    r.ajouter_texte("title", &episode.titre);
    r.ajouter_texte("originaltitle", &episode.titre_original);
    r.ajouter_texte("showtitle", &serie.titre);
    let groupe = &serie.groupe_actif;
    r.ajouter_texte("season", &episode.saison(groupe).to_string());
    r.ajouter_texte("episode", &episode.episode(groupe).to_string());
    if let Some(n) = episode.numero_dans(&GroupeEpisodes::affichage())
        && n.saison >= 0
        && n.episode >= 0
    {
        r.ajouter_texte("displayseason", &n.saison.to_string());
        r.ajouter_texte("displayepisode", &n.episode.to_string());
    }
    r.ajouter_texte(
        "id",
        &if episode.ids.tvdb() > 0 {
            episode.ids.tvdb().to_string()
        } else {
            String::new()
        },
    );
    for u in elements_uniqueid(&episode.ids, &[cles_id::TVDB, cles_id::IMDB, cles_id::TMDB]) {
        r.ajouter(u);
    }
    ajouter_notes(&mut r, &episode.notes, &p, options);
    r.ajouter_texte("plot", &episode.resume);
    if options.creer_resume_court {
        r.ajouter_texte(
            "outline",
            &resume_court(&episode.resume, options.resume_court_premiere_phrase),
        );
    }
    let video = video_principale(episode);
    let duree = video
        .map(|v| v.duree_secondes / 60)
        .filter(|m| *m > 0)
        .unwrap_or(episode.duree_minutes);
    r.ajouter_texte("runtime", &duree.to_string());
    r.ajouter_texte("mpaa", &serie.certification);
    let date = episode.premiere_diffusion.as_deref().unwrap_or("");
    r.ajouter_texte("premiered", date);
    r.ajouter_texte("aired", date);
    r.ajouter_texte("watched", if episode.vu { "true" } else { "false" });
    let lectures = if episode.vu {
        episode.nombre_lectures.max(1)
    } else {
        0
    };
    r.ajouter_texte("playcount", &lectures.to_string());
    if episode.vu
        && let Some(ms) = episode.derniere_lecture
    {
        r.ajouter_texte("lastplayed", &ms_vers_date_heure(ms));
    }
    let mut studios: Vec<&str> = serie
        .societe_production
        .split('/')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .collect();
    if options.studio_unique {
        studios.truncate(1);
    }
    for x in studios {
        r.ajouter_texte("studio", x);
    }
    for t in &episode.tags {
        r.ajouter_texte("tag", t);
    }
    for x in episode
        .equipe
        .iter()
        .filter(|x| x.type_personne == TypePersonne::Scenariste)
    {
        r.ajouter(element_personne_texte("credits", x));
    }
    for x in episode
        .equipe
        .iter()
        .filter(|x| x.type_personne == TypePersonne::Realisateur)
    {
        r.ajouter(element_personne_texte("director", x));
    }
    for a in &episode.invites {
        r.ajouter(element_acteur_options(a, Some("GuestStar"), options));
    }
    if options.ecrire_tous_acteurs {
        for a in &serie.acteurs {
            r.ajouter(element_acteur_options(a, Some("Actor"), options));
        }
    }
    if options.ecrire_date_ajout && episode.date_ajout > 0 {
        r.ajouter_texte("dateadded", &ms_vers_date_heure(episode.date_ajout));
    }
    if options.ecrire_lockdata {
        r.ajouter_texte("lockdata", "true");
    }
    if options.ecrire_fileinfo
        && let Some(v) = video
    {
        let sous_titres: Vec<&FichierMedia> = episode
            .fichiers
            .iter()
            .filter(|f| f.type_fichier == TypeFichier::SousTitres)
            .collect();
        if let Some(fi) = element_fileinfo(v, &sous_titres, p.fileinfo_ancien) {
            r.ajouter(fi);
        }
    }
    if p.balises_kodi {
        if let Some(b) = &conserves.epbookmark {
            r.ajouter_texte("epbookmark", b);
        }
        if let Some(c) = &conserves.code {
            r.ajouter_texte("code", c);
        }
        if episode.edition != crate::modele::Edition::Aucune {
            r.ajouter_texte("videoassettitle", &episode.edition.libelle());
            r.ajouter_texte("hasvideoversions", "true");
        }
    }
    for e in &conserves.inconnues {
        r.ajouter(e.clone());
    }

    ajouter_commentaire_meta(&mut r);
    r.ajouter_texte("source", nom_source(episode.source_media));
    r.ajouter_texte("edition", &nom_edition(&episode.edition));
    r.ajouter_texte("original_filename", &episode.nom_fichier_origine);
    r.ajouter_texte("user_note", &episode.note_utilisateur);
    let mut bloc = Element::nouveau("episode_groups");
    for n in &episode.numeros {
        bloc.ajouter(
            Element::nouveau("group")
                .attr("id", nom_type_groupe(n.groupe.type_groupe))
                .attr("name", &n.groupe.nom)
                .attr("season", &n.saison.to_string())
                .attr("episode", &n.episode.to_string()),
        );
    }
    r.ajouter(bloc);
    r.ajouter_texte("english_title", &episode.titre_anglais);
    if let Some(c) = video.and_then(crc32) {
        r.ajouter_texte("crc32", c);
    }
    for x in &episode.equipe {
        r.ajouter(element_crew(x));
    }
    r
}

/// Produit le NFO d'un fichier contenant un ou plusieurs épisodes.
/// `conserves[i]` correspond à `episodes[i]` (manquant = rien à conserver).
pub fn ecrire_nfo_episodes(
    serie: &Serie,
    episodes: &[&Episode],
    conserves: &[DonneesConservees],
    connecteur: Connecteur,
    options: &OptionsNfo,
) -> String {
    let vide = DonneesConservees::default();
    let racines: Vec<Element> = episodes
        .iter()
        .enumerate()
        .map(|(i, e)| {
            element_nfo_episode(
                serie,
                e,
                conserves.get(i).unwrap_or(&vide),
                connecteur,
                options,
            )
        })
        .collect();
    assembler(&racines, &commentaire_entete(connecteur, options))
}
