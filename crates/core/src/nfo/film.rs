//! NFO de films : lecture tolérante (racine `<movie>`, variante `<recording>`,
//! NFO réduit à une URL ou un identifiant) et écriture par connecteur.

use std::path::Path;

use crate::modele::commun::cles_id;
use crate::modele::{Film, Personne, Saga, TypeFichier, TypePersonne};

use super::arbre::{Element, decoder_octets, racines_nommees};
use super::commun::*;
use super::outils::{
    FormatBandeAnnonce, GenreMedia, booleen, completer_ids_depuis_texte, date_vers_ms,
    decouper_liste, entier_en_tete, ms_vers_date_heure, normaliser_date, resume_court,
};

/// Saga décrite dans un NFO de film (le rattachement à une [`Saga`] de la
/// médiathèque est du ressort de l'appelant).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SagaLue {
    pub titre: String,
    pub resume: String,
    /// Identifiant TMDB de la collection (0 si inconnu).
    pub tmdb_id: i64,
}

/// Résultat de la lecture d'un NFO de film.
#[derive(Debug, Clone, PartialEq)]
pub struct FilmLu {
    pub film: Film,
    pub saga: Option<SagaLue>,
    /// NFO valide = titre non vide.
    pub valide: bool,
    /// À repasser à [`ecrire_nfo_film`] pour conserver les balises non gérées.
    pub conserves: DonneesConservees,
}

/// Balises racine gérées (les autres sont conservées telles quelles).
const BALISES_FILM: &[&str] = &[
    "title",
    "originaltitle",
    "sorttitle",
    "english_title",
    "year",
    "top250",
    "plot",
    "outline",
    "tagline",
    "runtime",
    "thumb",
    "fanart",
    "certification",
    "mpaa",
    "id",
    "imdb",
    "imdbid",
    "tmdbid",
    "uniqueid",
    "ids",
    "tmdbcollectionid",
    "country",
    "premiered",
    "aired",
    "releasedate",
    "watched",
    "playcount",
    "genre",
    "genres",
    "studio",
    "credits",
    "director",
    "crew",
    "tag",
    "actor",
    "producer",
    "fileinfo",
    "languages",
    "language",
    "source",
    "edition",
    "trailer",
    "showlink",
    "epbookmark",
    "lastplayed",
    "status",
    "code",
    "dateadded",
    "original_filename",
    "user_note",
    "locked",
    "tmm_locked",
    "set",
    "sets",
    "collectionnumber",
    "rating",
    "ratings",
    "userrating",
    "votes",
    "criticrating",
    "lockdata",
    "videoassettitle",
    "hasvideoversions",
    "crc32",
];

// ---------------------------------------------------------------------------
// Lecture
// ---------------------------------------------------------------------------

/// Lit un NFO de film depuis ses octets (BOM et encodage gérés).
pub fn lire_nfo_film_octets(octets: &[u8]) -> FilmLu {
    lire_nfo_film(&decoder_octets(octets))
}

/// Lit un fichier NFO de film.
pub fn lire_fichier_nfo_film(chemin: &Path) -> std::io::Result<FilmLu> {
    Ok(lire_nfo_film_octets(&std::fs::read(chemin)?))
}

/// Lit un NFO de film (XML éventuellement bruité, ou simple URL / ID).
pub fn lire_nfo_film(texte: &str) -> FilmLu {
    let racines = racines_nommees(texte, &["movie", "recording"]);
    let mut lu = match racines.first() {
        Some(r) if r.a_nom("movie") => lire_movie(r),
        Some(r) => lire_recording(r),
        None => FilmLu {
            film: Film::default(),
            saga: None,
            valide: false,
            conserves: DonneesConservees::default(),
        },
    };
    // Identifiants manquants cherchés dans le texte brut (NFO = URL, liens…).
    if lu.film.ids.imdb().is_empty() || lu.film.ids.tmdb() == 0 {
        completer_ids_depuis_texte(&mut lu.film.ids, texte, GenreMedia::Film);
    }
    lu.valide = !lu.film.titre.is_empty();
    lu
}

fn lire_recording(r: &Element) -> FilmLu {
    let mut film = Film {
        titre: r.texte_unique("title").unwrap_or_default(),
        resume: r.texte_complet_unique("description").unwrap_or_default(),
        certification: r.texte_unique("rating").unwrap_or_default(),
        ..Film::default()
    };
    for g in r.enfants_nommes("genre") {
        film.genres.extend(decouper_liste(&g.texte_propre()));
    }
    FilmLu {
        film,
        saga: None,
        valide: false,
        conserves: DonneesConservees::default(),
    }
}

/// Valeurs de toutes les balises `nom`, une balise unique étant découpée.
fn valeurs_liste(r: &Element, nom: &str) -> Vec<String> {
    let elements: Vec<&Element> = r.enfants_nommes(nom).collect();
    match elements.len() {
        0 => Vec::new(),
        1 => decouper_liste(&elements[0].texte_propre()),
        _ => elements
            .iter()
            .map(|e| e.texte_propre())
            .filter(|t| !t.is_empty())
            .collect(),
    }
}

fn lire_saga(r: &Element) -> Option<SagaLue> {
    let mut saga = None;
    if let Some(sets) = r.enfant_unique("sets")
        && let Some(s) = sets.enfants_nommes("set").next()
    {
        let t = s.texte_propre();
        if !t.is_empty() {
            saga = Some(SagaLue {
                titre: t,
                ..SagaLue::default()
            });
        }
    }
    if saga.is_none()
        && let Some(s) = r.enfant_unique("set")
    {
        let titre = s
            .texte_unique("name")
            .or_else(|| s.texte_unique("setname"))
            .unwrap_or_else(|| s.texte_propre());
        let resume = s
            .texte_complet_unique("overview")
            .or_else(|| s.texte_complet_unique("setdescription"))
            .unwrap_or_default();
        let tmdb_id = s
            .attribut("tmdbcolid")
            .and_then(entier_en_tete)
            .unwrap_or(0);
        if !titre.is_empty() || tmdb_id > 0 {
            saga = Some(SagaLue {
                titre,
                resume,
                tmdb_id,
            });
        }
    }
    let numero = r
        .texte_unique("collectionnumber")
        .or_else(|| r.texte_unique("tmdbcollectionid"))
        .and_then(|t| entier_en_tete(&t))
        .unwrap_or(0);
    if numero > 0 {
        let s = saga.get_or_insert_with(SagaLue::default);
        if s.tmdb_id <= 0 {
            s.tmdb_id = numero;
        }
    }
    saga
}

fn lire_movie(r: &Element) -> FilmLu {
    let mut f = Film {
        titre: r.texte_unique("title").unwrap_or_default(),
        titre_original: r.texte_unique("originaltitle").unwrap_or_default(),
        titre_tri: r.texte_unique("sorttitle").unwrap_or_default(),
        titre_anglais: r.texte_unique("english_title").unwrap_or_default(),
        resume: r.texte_complet_unique("plot").unwrap_or_default(),
        synopsis: r.texte_complet_unique("outline").unwrap_or_default(),
        slogan: r.texte_complet_unique("tagline").unwrap_or_default(),
        note_utilisateur: r.texte_complet_unique("user_note").unwrap_or_default(),
        nom_fichier_origine: r.texte_unique("original_filename").unwrap_or_default(),
        ..Film::default()
    };
    if let Some(a) = r.texte_unique("year").and_then(|t| entier_en_tete(&t))
        && a > 0
    {
        f.annee = a as u32;
    }
    if let Some(n) = r.texte_unique("top250").and_then(|t| entier_en_tete(&t)) {
        f.top250 = n.max(0) as u32;
    }
    if let Some(n) = r.texte_unique("runtime").and_then(|t| entier_en_tete(&t)) {
        f.duree_minutes = n.max(0) as u32;
    }
    f.notes = lire_notes(r);

    // Artwork
    lire_thumbs(r, &mut f.urls_artwork);
    let fanarts = lire_fanarts(r);
    if let Some((premier, reste)) = fanarts.split_first() {
        f.urls_artwork.insert(TypeFichier::Fanart, premier.clone());
        f.fanarts_extra = reste.to_vec();
    }

    f.certification = r
        .texte_unique("certification")
        .or_else(|| r.texte_unique("mpaa"))
        .unwrap_or_default();

    // Identifiants (ordre d'écrasement de la spécification)
    if let Some(id) = r.texte_unique("id")
        && crate::modele::est_id_imdb_valide(&id)
    {
        f.ids.definir(cles_id::IMDB, &id);
    }
    for b in ["imdb", "imdbid"] {
        if let Some(v) = r.texte_unique(b) {
            f.ids.definir_si_absent(cles_id::IMDB, &v);
        }
    }
    if let Some(v) = r.texte_unique("tmdbid") {
        f.ids.definir(cles_id::TMDB, &v);
    }
    lire_uniqueids(r, &mut f.ids, None);
    lire_bloc_ids(r, &mut f.ids);
    let saga = lire_saga(r);
    if let Some(s) = &saga
        && s.tmdb_id > 0
    {
        f.ids.definir(cles_id::TMDB_SAGA, &s.tmdb_id.to_string());
    }

    f.pays = valeurs_liste(r, "country").join("/");
    f.date_sortie = ["premiered", "aired", "releasedate"]
        .iter()
        .find_map(|b| r.texte_unique(b).and_then(|t| normaliser_date(&t)));

    // Lecture
    f.vu = r.texte_unique("watched").is_some_and(|v| booleen(&v));
    if let Some(n) = r.texte_unique("playcount").and_then(|t| entier_en_tete(&t))
        && n > 0
    {
        f.nombre_lectures = n as u32;
        f.vu = true;
    }
    f.derniere_lecture = r.texte_unique("lastplayed").and_then(|t| date_vers_ms(&t));
    if let Some(ms) = r.texte_unique("dateadded").and_then(|t| date_vers_ms(&t)) {
        f.date_ajout = ms;
    }

    // Genres : `genre` multiples ou `<genres><genre/></genres>`.
    let mut genres = Vec::new();
    for g in r.enfants_nommes("genre") {
        genres.extend(decouper_liste(&g.texte_propre()));
    }
    if let Some(bloc) = r.enfant_unique("genres") {
        for g in bloc.enfants_nommes("genre") {
            genres.extend(decouper_liste(&g.texte_propre()));
        }
        if genres.is_empty() {
            genres.extend(decouper_liste(&bloc.texte_propre()));
        }
    }
    dedoublonner(&mut genres);
    f.genres = genres;

    f.societe_production = valeurs_liste(r, "studio").join(" / ");
    f.tags = r
        .enfants_nommes("tag")
        .map(Element::texte_propre)
        .filter(|t| !t.is_empty())
        .collect();
    dedoublonner(&mut f.tags);
    f.acteurs = r
        .enfants_nommes("actor")
        .filter_map(|e| lire_personne(e, TypePersonne::Acteur))
        .collect();
    f.equipe = lire_equipe(r);

    let langues = r
        .texte_unique("languages")
        .or_else(|| r.texte_unique("language"))
        .unwrap_or_default();
    f.langues_parlees = decouper_liste(&langues).join(", ");

    if let Some(s) = r.texte_unique("source") {
        f.source_media = source_depuis(&s);
    }
    if let Some(e) = r.texte_unique("edition") {
        f.edition = edition_depuis(&e);
    }
    f.bandes_annonces = lire_bandes_annonces(r);
    f.liens_series = r
        .enfants_nommes("showlink")
        .map(Element::texte_propre)
        .filter(|t| !t.is_empty())
        .collect();
    f.verrouille = lire_verrou(r);

    let conserves = DonneesConservees {
        epbookmark: r.texte_unique("epbookmark"),
        status: r.texte_unique("status"),
        code: r.texte_unique("code"),
        inconnues: elements_inconnus(r, BALISES_FILM),
    };
    FilmLu {
        film: f,
        saga,
        valide: false,
        conserves,
    }
}

fn dedoublonner(v: &mut Vec<String>) {
    let mut vus = std::collections::BTreeSet::new();
    v.retain(|x| vus.insert(x.to_lowercase()));
}

// ---------------------------------------------------------------------------
// Écriture
// ---------------------------------------------------------------------------

/// Écriture d'une liste dans une ou plusieurs balises.
#[derive(Clone, Copy)]
enum Liste {
    /// Une balise par valeur.
    Multiple,
    /// Une seule balise, valeurs jointes par ce séparateur.
    Jointe(&'static str),
}

/// Différences d'écriture entre connecteurs.
struct Profil {
    notes_kodi: bool,
    set_structure: bool,
    set_tmdbcolid: bool,
    thumbs_aspect: bool,
    fanart_multiple: bool,
    equipe_apres_acteurs: bool,
    /// epbookmark / top250 / status / code.
    balises_kodi: bool,
    videoasset: bool,
    collectionnumber: bool,
    format_bande_annonce: FormatBandeAnnonce,
    mpaa: bool,
    certification: bool,
    genres_englobes: bool,
    pays: Liste,
    studio: Liste,
    personnes_jointes: Option<&'static str>,
    balise_langues: &'static str,
    separateur_langues: &'static str,
    balise_imdb: bool,
    fileinfo_ancien: bool,
}

const PROFIL_BASE: Profil = Profil {
    notes_kodi: false,
    set_structure: false,
    set_tmdbcolid: false,
    thumbs_aspect: false,
    fanart_multiple: true,
    equipe_apres_acteurs: false,
    balises_kodi: false,
    videoasset: false,
    collectionnumber: false,
    format_bande_annonce: FormatBandeAnnonce::Moderne,
    mpaa: true,
    certification: true,
    genres_englobes: false,
    pays: Liste::Multiple,
    studio: Liste::Multiple,
    personnes_jointes: None,
    balise_langues: "languages",
    separateur_langues: ", ",
    balise_imdb: false,
    fileinfo_ancien: false,
};

const PROFIL_KODI: Profil = Profil {
    notes_kodi: true,
    set_structure: true,
    thumbs_aspect: true,
    balises_kodi: true,
    videoasset: true,
    ..PROFIL_BASE
};

fn profil_film(c: Connecteur) -> Profil {
    match c {
        Connecteur::Kodi | Connecteur::Plex | Connecteur::Dvr3 => PROFIL_KODI,
        Connecteur::Emby => Profil {
            set_tmdbcolid: true,
            equipe_apres_acteurs: true,
            ..PROFIL_KODI
        },
        Connecteur::Jellyfin => Profil {
            balises_kodi: false,
            videoasset: false,
            collectionnumber: true,
            ..PROFIL_KODI
        },
        Connecteur::Xbmc => Profil {
            balises_kodi: true,
            format_bande_annonce: FormatBandeAnnonce::Ancien,
            fileinfo_ancien: true,
            ..PROFIL_BASE
        },
        Connecteur::MediaPortal => Profil {
            fanart_multiple: false,
            mpaa: false,
            genres_englobes: true,
            pays: Liste::Jointe("/"),
            studio: Liste::Jointe(" / "),
            personnes_jointes: Some(" / "),
            separateur_langues: "|",
            ..PROFIL_BASE
        },
        Connecteur::MpMovingPictures => Profil {
            certification: false,
            pays: Liste::Jointe(", "),
            studio: Liste::Jointe(", "),
            personnes_jointes: Some(", "),
            balise_langues: "language",
            ..PROFIL_BASE
        },
        Connecteur::MpMyVideo => Profil {
            fanart_multiple: false,
            balise_imdb: true,
            pays: Liste::Jointe(" / "),
            studio: Liste::Jointe(" / "),
            personnes_jointes: Some(" / "),
            balise_langues: "language",
            separateur_langues: "|",
            ..PROFIL_BASE
        },
    }
}

fn ecrire_liste(racine: &mut Element, balise: &str, valeurs: &[String], mode: Liste) {
    match mode {
        Liste::Multiple => {
            for v in valeurs {
                racine.ajouter_texte(balise, v);
            }
        }
        Liste::Jointe(sep) => {
            if !valeurs.is_empty() {
                racine.ajouter_texte(balise, &valeurs.join(sep));
            }
        }
    }
}

fn ecrire_personnes_texte(racine: &mut Element, balise: &str, personnes: &[&Personne], p: &Profil) {
    match p.personnes_jointes {
        Some(sep) => {
            if !personnes.is_empty() {
                let noms: Vec<&str> = personnes.iter().map(|x| x.nom.as_str()).collect();
                racine.ajouter_texte(balise, &noms.join(sep));
            }
        }
        None => {
            for x in personnes {
                racine.ajouter(element_personne_texte(balise, x));
            }
        }
    }
}

/// Valeurs d'un champ joint par `/` (pays, sociétés).
fn decouper_barres(s: &str) -> Vec<String> {
    s.split('/')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(str::to_string)
        .collect()
}

/// Construit l'élément racine `<movie>`.
pub fn element_nfo_film(
    film: &Film,
    saga: Option<&Saga>,
    conserves: &DonneesConservees,
    connecteur: Connecteur,
    options: &OptionsNfo,
) -> Element {
    let p = profil_film(connecteur);
    let mut r = Element::nouveau("movie");
    r.ajouter_texte("title", &film.titre);
    r.ajouter_texte("originaltitle", &film.titre_original);
    r.ajouter_texte("sorttitle", &film.titre_tri);
    if p.balises_kodi
        && let Some(e) = &conserves.epbookmark
    {
        r.ajouter_texte("epbookmark", e);
    }
    r.ajouter_texte(
        "year",
        &if film.annee > 0 {
            film.annee.to_string()
        } else {
            String::new()
        },
    );

    // Notes
    let principale = note_principale(&film.notes, &options.sources_notes);
    if p.notes_kodi {
        if let Some(b) = element_ratings(&film.notes, &options.sources_notes) {
            r.ajouter(b);
        }
    } else if let Some(n) = principale {
        r.ajouter_texte("rating", &super::outils::format_note(n.sur_dix()));
    }
    r.ajouter_texte("userrating", &texte_note_utilisateur(&film.notes));
    if !p.notes_kodi
        && let Some(n) = principale
    {
        r.ajouter_texte("votes", &n.votes.to_string());
    }
    if p.balises_kodi {
        r.ajouter_texte("top250", &film.top250.to_string());
    }

    // Saga
    if let Some(s) = saga.filter(|s| !s.titre.is_empty()) {
        if p.set_structure {
            let mut e = Element::nouveau("set");
            if p.set_tmdbcolid {
                let id = film
                    .ids
                    .obtenir(cles_id::TMDB_SAGA)
                    .and_then(|v| v.en_entier());
                let id = id
                    .filter(|i| *i > 0)
                    .or(Some(s.ids.tmdb()).filter(|i| *i > 0));
                if let Some(i) = id {
                    e = e.attr("tmdbcolid", &i.to_string());
                }
            }
            e.ajouter_texte("name", &s.titre);
            e.ajouter_texte("overview", &s.resume);
            r.ajouter(e);
        } else {
            r.ajouter_texte("set", &s.titre);
        }
    }

    r.ajouter_texte("plot", &film.resume);
    let outline = if !film.synopsis.is_empty() {
        film.synopsis.clone()
    } else if options.creer_resume_court {
        resume_court(&film.resume, options.resume_court_premiere_phrase)
    } else {
        String::new()
    };
    r.ajouter_texte("outline", &outline);
    r.ajouter_texte("tagline", &film.slogan);
    r.ajouter_texte("runtime", &film.duree_minutes.to_string());

    // Artwork
    if options.ecrire_urls_artwork {
        if p.thumbs_aspect {
            let aspects = [
                "poster",
                "banner",
                "clearart",
                "clearlogo",
                "discart",
                "landscape",
                "keyart",
                "squareart",
                "logo",
            ];
            for t in elements_thumbs_aspect(&film.urls_artwork, &aspects) {
                r.ajouter(t);
            }
        } else if let Some(u) = film.urls_artwork.get(&TypeFichier::Poster) {
            r.ajouter_texte("thumb", u);
        }
        let mut fanarts: Vec<&str> = Vec::new();
        if let Some(u) = film.urls_artwork.get(&TypeFichier::Fanart) {
            fanarts.push(u);
        }
        if p.fanart_multiple {
            fanarts.extend(film.fanarts_extra.iter().map(String::as_str));
        }
        if let Some(f) = element_fanart(&fanarts) {
            r.ajouter(f);
        }
    }

    if p.mpaa {
        r.ajouter_texte("mpaa", &film.certification);
    }
    if p.certification {
        r.ajouter_texte("certification", &film.certification);
    }
    r.ajouter_texte("id", &film.ids.imdb());
    if p.balise_imdb {
        r.ajouter_texte("imdb", &film.ids.imdb());
    }
    if film.ids.tmdb() > 0 {
        r.ajouter_texte("tmdbid", &film.ids.tmdb().to_string());
    }
    for u in elements_uniqueid(&film.ids, &[cles_id::IMDB, cles_id::TVDB, cles_id::TMDB]) {
        r.ajouter(u);
    }
    ecrire_liste(&mut r, "country", &decouper_barres(&film.pays), p.pays);
    if p.balises_kodi {
        if let Some(s) = &conserves.status {
            r.ajouter_texte("status", s);
        }
        if let Some(c) = &conserves.code {
            r.ajouter_texte("code", c);
        }
    }
    r.ajouter_texte("premiered", film.date_sortie.as_deref().unwrap_or(""));

    // Lecture
    r.ajouter_texte("watched", if film.vu { "true" } else { "false" });
    let lectures = if film.vu {
        film.nombre_lectures.max(1)
    } else {
        0
    };
    r.ajouter_texte("playcount", &lectures.to_string());
    if film.vu
        && let Some(ms) = film.derniere_lecture
    {
        r.ajouter_texte("lastplayed", &ms_vers_date_heure(ms));
    }

    if p.genres_englobes {
        let mut g = Element::nouveau("genres");
        for x in &film.genres {
            g.ajouter_texte("genre", x);
        }
        r.ajouter(g);
    } else {
        for x in &film.genres {
            r.ajouter_texte("genre", x);
        }
    }
    let mut studios = decouper_barres(&film.societe_production);
    if options.studio_unique {
        studios.truncate(1);
    }
    ecrire_liste(&mut r, "studio", &studios, p.studio);

    let scenaristes = film.scenaristes();
    let realisateurs = film.realisateurs();
    if !p.equipe_apres_acteurs {
        ecrire_personnes_texte(&mut r, "credits", &scenaristes, &p);
        ecrire_personnes_texte(&mut r, "director", &realisateurs, &p);
    }
    for t in &film.tags {
        r.ajouter_texte("tag", t);
    }
    for a in &film.acteurs {
        r.ajouter(element_acteur_options(a, None, options));
    }
    if p.equipe_apres_acteurs {
        ecrire_personnes_texte(&mut r, "credits", &scenaristes, &p);
        ecrire_personnes_texte(&mut r, "director", &realisateurs, &p);
    }
    for x in film.producteurs() {
        r.ajouter(element_producteur(x, options.ecrire_urls_artwork));
    }
    if options.ecrire_bande_annonce
        && let Some(u) = url_bande_annonce(&film.bandes_annonces, p.format_bande_annonce)
    {
        r.ajouter_texte("trailer", &u);
    }
    let langues = decouper_liste(&film.langues_parlees);
    r.ajouter_texte(p.balise_langues, &langues.join(p.separateur_langues));
    for l in &film.liens_series {
        r.ajouter_texte("showlink", l);
    }
    if options.ecrire_date_ajout {
        r.ajouter_texte("dateadded", &ms_vers_date_heure(film.date_ajout));
    }
    if options.ecrire_lockdata {
        r.ajouter_texte("lockdata", "true");
    }
    let video = film.fichier_principal();
    if options.ecrire_fileinfo
        && let Some(v) = video
    {
        let sous_titres: Vec<_> = film.fichiers_de_type(TypeFichier::SousTitres).collect();
        if let Some(fi) = element_fileinfo(v, &sous_titres, p.fileinfo_ancien) {
            r.ajouter(fi);
        }
    }
    if p.videoasset && film.edition != crate::modele::Edition::Aucune {
        r.ajouter_texte("videoassettitle", &film.edition.libelle());
        r.ajouter_texte("hasvideoversions", "true");
    }
    if p.collectionnumber
        && let Some(id) = film
            .ids
            .obtenir(cles_id::TMDB_SAGA)
            .and_then(|v| v.en_entier())
    {
        r.ajouter_texte("collectionnumber", &id.to_string());
    }
    for e in &conserves.inconnues {
        r.ajouter(e.clone());
    }

    // Bloc de métadonnées propres
    ajouter_commentaire_meta(&mut r);
    r.ajouter_texte("source", nom_source(film.source_media));
    r.ajouter_texte("edition", &nom_edition(&film.edition));
    r.ajouter_texte("original_filename", &film.nom_fichier_origine);
    r.ajouter_texte("user_note", &film.note_utilisateur);
    r.ajouter_texte("english_title", &film.titre_anglais);
    if let Some(c) = video.and_then(crc32) {
        r.ajouter_texte("crc32", c);
    }
    for x in &film.equipe {
        r.ajouter(element_crew(x));
    }
    r.ajouter_texte("locked", if film.verrouille { "true" } else { "" });
    r
}

/// Produit le contenu complet d'un NFO de film (UTF-8, CRLF).
pub fn ecrire_nfo_film(
    film: &Film,
    saga: Option<&Saga>,
    conserves: &DonneesConservees,
    connecteur: Connecteur,
    options: &OptionsNfo,
) -> String {
    let racine = element_nfo_film(film, saga, conserves, connecteur, options);
    assembler(&[racine], &commentaire_entete(connecteur, options))
}
