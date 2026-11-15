//! NFO de séries (`tvshow.nfo`, racine `<tvshow>`).

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::modele::commun::cles_id;
use crate::modele::{
    GroupeEpisodes, Saison, Serie, StatutDiffusion, TypeFichier, TypeGroupe, TypePersonne,
};

use super::arbre::{Element, decoder_octets, racines_nommees};
use super::commun::*;
use super::outils::{
    FormatBandeAnnonce, GenreMedia, annee_de_date, completer_ids_depuis_texte, date_vers_ms,
    decouper_liste, entier_en_tete, ms_vers_date_heure, normaliser_date, resume_court,
};

/// Résultat de la lecture d'un `tvshow.nfo`.
#[derive(Debug, Clone, PartialEq)]
pub struct SerieLue {
    pub serie: Serie,
    /// NFO valide = titre non vide.
    pub valide: bool,
    pub conserves: DonneesConservees,
}

const BALISES_SERIE: &[&str] = &[
    "title",
    "originaltitle",
    "showtitle",
    "sorttitle",
    "year",
    "top250",
    "ratings",
    "rating",
    "votes",
    "userrating",
    "criticrating",
    "outline",
    "plot",
    "tagline",
    "runtime",
    "thumb",
    "namedseason",
    "seasonplot",
    "fanart",
    "mpaa",
    "certification",
    "episodeguide",
    "id",
    "imdb",
    "imdbid",
    "tmdbid",
    "tvdbid",
    "uniqueid",
    "ids",
    "premiered",
    "aired",
    "status",
    "watched",
    "playcount",
    "genre",
    "studio",
    "country",
    "tag",
    "credits",
    "director",
    "producer",
    "actor",
    "trailer",
    "dateadded",
    "lockdata",
    "enddate",
    "user_note",
    "episode_groups",
    "english_title",
    "crew",
    "locked",
    "tmm_locked",
    "set",
    "fileinfo",
];

// ---------------------------------------------------------------------------
// Groupes d'épisodes
// ---------------------------------------------------------------------------

/// Identifiant écrit pour un type de groupe.
pub fn nom_type_groupe(t: TypeGroupe) -> &'static str {
    match t {
        TypeGroupe::Diffusion => "AIRED",
        TypeGroupe::Dvd => "DVD",
        TypeGroupe::Absolu => "ABSOLUTE",
        TypeGroupe::Alternatif => "ALTERNATE",
        TypeGroupe::Affichage => "DISPLAY",
    }
}

/// Type de groupe lu (`None` si inconnu).
pub fn type_groupe_depuis(s: &str) -> Option<TypeGroupe> {
    match s.trim().to_ascii_uppercase().as_str() {
        "AIRED" => Some(TypeGroupe::Diffusion),
        "DVD" => Some(TypeGroupe::Dvd),
        "ABSOLUTE" => Some(TypeGroupe::Absolu),
        "ALTERNATE" => Some(TypeGroupe::Alternatif),
        "DISPLAY" => Some(TypeGroupe::Affichage),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Statut
// ---------------------------------------------------------------------------

fn nom_statut(s: StatutDiffusion) -> &'static str {
    match s {
        StatutDiffusion::Inconnu => "",
        StatutDiffusion::EnCours => "Continuing",
        StatutDiffusion::Terminee => "Ended",
        StatutDiffusion::Annulee => "Canceled",
    }
}

fn statut_depuis(s: &str) -> StatutDiffusion {
    let s = s.to_ascii_lowercase();
    if s.contains("continu")
        || s.contains("returning")
        || s.contains("production")
        || s.contains("airing")
    {
        StatutDiffusion::EnCours
    } else if s.contains("cancel") {
        StatutDiffusion::Annulee
    } else if s.contains("ended") || s.contains("termin") {
        StatutDiffusion::Terminee
    } else {
        StatutDiffusion::Inconnu
    }
}

// ---------------------------------------------------------------------------
// Lecture
// ---------------------------------------------------------------------------

/// Lit un `tvshow.nfo` depuis ses octets.
pub fn lire_nfo_serie_octets(octets: &[u8]) -> SerieLue {
    lire_nfo_serie(&decoder_octets(octets))
}

/// Lit un fichier `tvshow.nfo`.
pub fn lire_fichier_nfo_serie(chemin: &Path) -> std::io::Result<SerieLue> {
    Ok(lire_nfo_serie_octets(&std::fs::read(chemin)?))
}

static RE_JSON_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""(\w+)"\s*:\s*"?([^",}\s]+)"?"#).unwrap());

fn lire_guide_episodes(r: &Element, serie: &mut Serie) {
    let Some(g) = r.enfant_unique("episodeguide") else {
        return;
    };
    let texte = g.texte_complet();
    if texte.trim_start().starts_with('{') {
        for c in RE_JSON_ID.captures_iter(&texte) {
            let cle = c[1].to_ascii_lowercase();
            if matches!(cle.as_str(), "imdb" | "tmdb" | "tvdb" | "trakt" | "tvmaze") {
                serie.ids.definir_si_absent(&cle, &c[2]);
            }
        }
    } else {
        completer_ids_depuis_texte(&mut serie.ids, &texte, GenreMedia::Serie);
    }
}

fn saison_mut(serie: &mut Serie, numero: i32) -> &mut Saison {
    if let Some(i) = serie.saisons.iter().position(|s| s.numero == numero) {
        return &mut serie.saisons[i];
    }
    serie.saisons.push(Saison {
        numero,
        ..Saison::default()
    });
    serie.saisons.last_mut().expect("saison ajoutée")
}

/// Lit un `tvshow.nfo` (XML éventuellement bruité, ou simple URL / ID).
pub fn lire_nfo_serie(texte: &str) -> SerieLue {
    let racines = racines_nommees(texte, &["tvshow"]);
    let (mut serie, conserves) = match racines.first() {
        Some(r) => lire_tvshow(r),
        None => (Serie::default(), DonneesConservees::default()),
    };
    if serie.ids.tvdb() == 0 || serie.ids.imdb().is_empty() || serie.ids.tmdb() == 0 {
        completer_ids_depuis_texte(&mut serie.ids, texte, GenreMedia::Serie);
    }
    let valide = !serie.titre.is_empty();
    SerieLue {
        serie,
        valide,
        conserves,
    }
}

fn lire_tvshow(r: &Element) -> (Serie, DonneesConservees) {
    let mut s = Serie {
        titre: r.texte_unique("title").unwrap_or_default(),
        titre_original: r.texte_unique("originaltitle").unwrap_or_default(),
        titre_tri: r.texte_unique("sorttitle").unwrap_or_default(),
        titre_anglais: r.texte_unique("english_title").unwrap_or_default(),
        resume: r
            .texte_complet_unique("plot")
            .or_else(|| r.texte_complet_unique("outline"))
            .unwrap_or_default(),
        slogan: r.texte_complet_unique("tagline").unwrap_or_default(),
        note_utilisateur: r.texte_complet_unique("user_note").unwrap_or_default(),
        verrouille: lire_verrou(r),
        ..Serie::default()
    };
    if s.titre.is_empty() {
        s.titre = r.texte_unique("showtitle").unwrap_or_default();
    }
    if let Some(a) = r.texte_unique("year").and_then(|t| entier_en_tete(&t))
        && a > 0
    {
        s.annee = a as u32;
    }
    if let Some(n) = r.texte_unique("top250").and_then(|t| entier_en_tete(&t)) {
        s.top250 = n.max(0) as u32;
    }
    if let Some(n) = r.texte_unique("runtime").and_then(|t| entier_en_tete(&t)) {
        s.duree_minutes = n.max(0) as u32;
    }
    s.notes = lire_notes(r);
    lire_thumbs(r, &mut s.urls_artwork);
    if let Some(u) = lire_fanarts(r).into_iter().next() {
        s.urls_artwork.insert(TypeFichier::Fanart, u);
    }

    // Noms et résumés de saisons
    for n in r.enfants_nommes("namedseason") {
        if let Some(num) = n.attribut("number").and_then(entier_en_tete) {
            let t = n.texte_propre();
            if !t.is_empty() {
                saison_mut(&mut s, num as i32).titre = t;
            }
        }
    }
    for n in r.enfants_nommes("seasonplot") {
        if let Some(num) = n.attribut("number").and_then(entier_en_tete) {
            let t = n.texte_complet();
            if !t.is_empty() {
                saison_mut(&mut s, num as i32).resume = t;
            }
        }
    }
    s.saisons.sort_by_key(|x| x.numero);

    s.certification = r
        .texte_unique("certification")
        .or_else(|| r.texte_unique("mpaa"))
        .unwrap_or_default();

    // Identifiants
    if let Some(v) = r.texte_unique("id") {
        if crate::modele::est_id_imdb_valide(&v) {
            s.ids.definir(cles_id::IMDB, &v);
        } else {
            s.ids.definir(cles_id::TVDB, &v);
        }
    }
    for b in ["imdb", "imdbid"] {
        if let Some(v) = r.texte_unique(b) {
            s.ids.definir_si_absent(cles_id::IMDB, &v);
        }
    }
    if let Some(v) = r.texte_unique("tmdbid") {
        s.ids.definir(cles_id::TMDB, &v);
    }
    if let Some(v) = r.texte_unique("tvdbid") {
        s.ids.definir(cles_id::TVDB, &v);
    }
    lire_uniqueids(r, &mut s.ids, Some(cles_id::TVDB));
    lire_bloc_ids(r, &mut s.ids);
    lire_guide_episodes(r, &mut s);

    s.premiere_diffusion = ["premiered", "aired"]
        .iter()
        .find_map(|b| r.texte_unique(b).and_then(|t| normaliser_date(&t)));
    if s.annee == 0
        && let Some(d) = &s.premiere_diffusion
    {
        s.annee = annee_de_date(d);
    }
    if let Some(d) = r.texte_unique("enddate") {
        s.annee_fin = annee_de_date(&d);
    }
    s.statut = r
        .texte_unique("status")
        .map_or(StatutDiffusion::Inconnu, |t| statut_depuis(&t));

    for g in r.enfants_nommes("genre") {
        s.genres.extend(decouper_liste(&g.texte_propre()));
    }
    let studios: Vec<&Element> = r.enfants_nommes("studio").collect();
    s.societe_production = if studios.len() == 1 {
        decouper_liste(&studios[0].texte_propre()).join(" / ")
    } else {
        studios
            .iter()
            .map(|e| e.texte_propre())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(" / ")
    };
    let pays: Vec<&Element> = r.enfants_nommes("country").collect();
    s.pays = if pays.len() == 1 {
        decouper_liste(&pays[0].texte_propre()).join("/")
    } else {
        pays.iter()
            .map(|e| e.texte_propre())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join("/")
    };
    s.tags = r
        .enfants_nommes("tag")
        .map(Element::texte_propre)
        .filter(|t| !t.is_empty())
        .collect();
    s.acteurs = r
        .enfants_nommes("actor")
        .filter_map(|e| lire_personne(e, TypePersonne::Acteur))
        .collect();
    s.equipe = lire_equipe(r);
    s.bandes_annonces = lire_bandes_annonces(r);
    if let Some(ms) = r.texte_unique("dateadded").and_then(|t| date_vers_ms(&t)) {
        s.date_ajout = ms;
    }

    // Ordres d'épisodes
    if let Some(bloc) = r.enfant_unique("episode_groups") {
        for g in bloc.enfants_nommes("group") {
            let Some(t) = g.attribut("id").and_then(type_groupe_depuis) else {
                continue;
            };
            let groupe = GroupeEpisodes {
                type_groupe: t,
                nom: g.attribut("name").unwrap_or("").to_string(),
            };
            if g.attribut("active").is_some_and(super::outils::booleen) {
                s.groupe_actif = groupe.clone();
            }
            if !s.groupes.contains(&groupe) {
                s.groupes.push(groupe);
            }
        }
    }

    // Les vignettes de saison n'ont pas de place dans le modèle : elles sont
    // conservées telles quelles pour la réécriture.
    let mut inconnues = elements_inconnus(r, BALISES_SERIE);
    inconnues.extend(
        r.enfants_nommes("thumb")
            .filter(|t| {
                t.attribut("type")
                    .is_some_and(|v| v.eq_ignore_ascii_case("season"))
            })
            .cloned(),
    );
    let conserves = DonneesConservees {
        inconnues,
        ..DonneesConservees::default()
    };
    (s, conserves)
}

// ---------------------------------------------------------------------------
// Écriture
// ---------------------------------------------------------------------------

/// Format d'écriture d'une série / d'un épisode selon le connecteur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProfilSerie {
    pub notes_kodi: bool,
    pub thumbs_aspect: bool,
    pub date_fin: bool,
    pub balises_kodi: bool,
    pub format_bande_annonce: FormatBandeAnnonce,
    pub fileinfo_ancien: bool,
}

pub(crate) fn profil_serie(c: Connecteur) -> ProfilSerie {
    let kodi = ProfilSerie {
        notes_kodi: true,
        thumbs_aspect: true,
        date_fin: false,
        balises_kodi: true,
        format_bande_annonce: FormatBandeAnnonce::Moderne,
        fileinfo_ancien: false,
    };
    match c {
        Connecteur::Kodi | Connecteur::Plex | Connecteur::Dvr3 => kodi,
        Connecteur::Emby | Connecteur::Jellyfin => ProfilSerie {
            date_fin: true,
            balises_kodi: false,
            ..kodi
        },
        Connecteur::Xbmc
        | Connecteur::MediaPortal
        | Connecteur::MpMovingPictures
        | Connecteur::MpMyVideo => ProfilSerie {
            notes_kodi: false,
            thumbs_aspect: false,
            date_fin: false,
            balises_kodi: false,
            format_bande_annonce: FormatBandeAnnonce::Ancien,
            fileinfo_ancien: true,
        },
    }
}

/// Notes : bloc `ratings` (Kodi) ou `rating` simple ; puis `userrating`, puis
/// `votes` (formats anciens).
pub(crate) fn ajouter_notes(
    r: &mut Element,
    notes: &std::collections::BTreeMap<String, crate::modele::Note>,
    p: &ProfilSerie,
    options: &OptionsNfo,
) {
    let principale = note_principale(notes, &options.sources_notes);
    if p.notes_kodi {
        if let Some(b) = element_ratings(notes, &options.sources_notes) {
            r.ajouter(b);
        }
    } else if let Some(n) = principale {
        r.ajouter_texte("rating", &super::outils::format_note(n.sur_dix()));
    }
    r.ajouter_texte("userrating", &texte_note_utilisateur(notes));
    if !p.notes_kodi
        && let Some(n) = principale
    {
        r.ajouter_texte("votes", &n.votes.to_string());
    }
}

/// `episodeguide` au format JSON des identifiants (aucune clé d'API).
fn texte_guide_episodes(serie: &Serie) -> Option<String> {
    let mut parties = Vec::new();
    for cle in [cles_id::IMDB, cles_id::TMDB, cles_id::TVDB] {
        if let Some(v) = serie.ids.obtenir(cle) {
            parties.push(format!("\"{cle}\":\"{}\"", v.en_texte()));
        }
    }
    if parties.is_empty() {
        None
    } else {
        Some(format!("{{{}}}", parties.join(",")))
    }
}

/// Date de fin : dernière diffusion connue hors épisodes spéciaux.
fn date_fin(serie: &Serie) -> Option<String> {
    serie
        .episodes
        .iter()
        .chain(serie.episodes_fictifs.iter())
        .filter(|e| e.saison(&serie.groupe_actif) > 0)
        .filter_map(|e| e.premiere_diffusion.clone())
        .max()
}

/// Construit l'élément racine `<tvshow>`.
pub fn element_nfo_serie(
    serie: &Serie,
    conserves: &DonneesConservees,
    connecteur: Connecteur,
    options: &OptionsNfo,
) -> Element {
    let p = profil_serie(connecteur);
    let mut r = Element::nouveau("tvshow");
    r.ajouter_texte("title", &serie.titre);
    r.ajouter_texte("originaltitle", &serie.titre_original);
    r.ajouter_texte("showtitle", &serie.titre);
    r.ajouter_texte("sorttitle", &serie.titre_tri);
    r.ajouter_texte(
        "year",
        &if serie.annee > 0 {
            serie.annee.to_string()
        } else {
            String::new()
        },
    );
    if p.balises_kodi {
        r.ajouter_texte("top250", &serie.top250.to_string());
    }
    ajouter_notes(&mut r, &serie.notes, &p, options);
    let outline = if options.creer_resume_court {
        resume_court(&serie.resume, options.resume_court_premiere_phrase)
    } else {
        String::new()
    };
    r.ajouter_texte("outline", &outline);
    r.ajouter_texte("plot", &serie.resume);
    r.ajouter_texte("tagline", &serie.slogan);
    r.ajouter_texte("runtime", &serie.duree_minutes.to_string());
    if options.ecrire_urls_artwork {
        if p.thumbs_aspect {
            let aspects = [
                "poster",
                "banner",
                "clearart",
                "clearlogo",
                "landscape",
                "keyart",
                "squareart",
                "logo",
                "characterart",
                "discart",
            ];
            for t in elements_thumbs_aspect(&serie.urls_artwork, &aspects) {
                r.ajouter(t);
            }
        } else if let Some(u) = serie.urls_artwork.get(&TypeFichier::Poster) {
            r.ajouter_texte("thumb", u);
        }
    }
    for s in &serie.saisons {
        if !s.titre.is_empty() {
            r.ajouter(
                Element::avec_texte("namedseason", &s.titre).attr("number", &s.numero.to_string()),
            );
        }
    }
    for s in &serie.saisons {
        if !s.resume.is_empty() {
            r.ajouter(
                Element::avec_texte("seasonplot", &s.resume).attr("number", &s.numero.to_string()),
            );
        }
    }
    if options.ecrire_urls_artwork
        && let Some(f) = serie
            .urls_artwork
            .get(&TypeFichier::Fanart)
            .and_then(|u| element_fanart(&[u.as_str()]))
    {
        r.ajouter(f);
    }
    r.ajouter_texte("mpaa", &serie.certification);
    r.ajouter_texte("certification", &serie.certification);
    if options.ecrire_guide_episodes
        && let Some(g) = texte_guide_episodes(serie)
    {
        r.ajouter_texte("episodeguide", &g);
    }
    r.ajouter_texte(
        "id",
        &if serie.ids.tvdb() > 0 {
            serie.ids.tvdb().to_string()
        } else {
            String::new()
        },
    );
    r.ajouter_texte("imdbid", &serie.ids.imdb());
    if serie.ids.tmdb() > 0 {
        r.ajouter_texte("tmdbid", &serie.ids.tmdb().to_string());
    }
    for u in elements_uniqueid(&serie.ids, &[cles_id::TVDB, cles_id::IMDB, cles_id::TMDB]) {
        r.ajouter(u);
    }
    r.ajouter_texte(
        "premiered",
        serie.premiere_diffusion.as_deref().unwrap_or(""),
    );
    r.ajouter_texte("status", nom_statut(serie.statut));
    for g in &serie.genres {
        r.ajouter_texte("genre", g);
    }
    let mut studios: Vec<String> = serie
        .societe_production
        .split('/')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(str::to_string)
        .collect();
    if options.studio_unique {
        studios.truncate(1);
    }
    for x in &studios {
        r.ajouter_texte("studio", x);
    }
    for x in serie
        .pays
        .split('/')
        .map(str::trim)
        .filter(|x| !x.is_empty())
    {
        r.ajouter_texte("country", x);
    }
    for t in &serie.tags {
        r.ajouter_texte("tag", t);
    }
    for x in serie
        .equipe
        .iter()
        .filter(|x| x.type_personne == TypePersonne::Scenariste)
    {
        r.ajouter(element_personne_texte("credits", x));
    }
    for x in serie
        .equipe
        .iter()
        .filter(|x| x.type_personne == TypePersonne::Realisateur)
    {
        r.ajouter(element_personne_texte("director", x));
    }
    for a in &serie.acteurs {
        r.ajouter(element_acteur(a, None, options.ecrire_urls_artwork));
    }
    if options.ecrire_bande_annonce
        && let Some(u) = url_bande_annonce(&serie.bandes_annonces, p.format_bande_annonce)
    {
        r.ajouter_texte("trailer", &u);
    }
    if options.ecrire_date_ajout && serie.date_ajout > 0 {
        r.ajouter_texte("dateadded", &ms_vers_date_heure(serie.date_ajout));
    }
    if options.ecrire_lockdata {
        r.ajouter_texte("lockdata", "true");
    }
    if p.date_fin
        && options.ecrire_date_fin
        && serie.statut == StatutDiffusion::Terminee
        && let Some(d) = date_fin(serie)
    {
        r.ajouter_texte("enddate", &d);
    }
    for e in &conserves.inconnues {
        r.ajouter(e.clone());
    }

    ajouter_commentaire_meta(&mut r);
    r.ajouter_texte("user_note", &serie.note_utilisateur);
    let mut groupes = serie.groupes.clone();
    if !groupes.contains(&serie.groupe_actif) {
        groupes.insert(0, serie.groupe_actif.clone());
    }
    let mut bloc = Element::nouveau("episode_groups");
    for g in &groupes {
        let mut e = Element::nouveau("group")
            .attr("id", nom_type_groupe(g.type_groupe))
            .attr("name", &g.nom);
        if *g == serie.groupe_actif {
            e = e.attr("active", "true");
        }
        bloc.ajouter(e);
    }
    r.ajouter(bloc);
    r.ajouter_texte("english_title", &serie.titre_anglais);
    for x in &serie.equipe {
        r.ajouter(element_crew(x));
    }
    r.ajouter_texte("locked", if serie.verrouille { "true" } else { "" });
    r
}

/// Produit le contenu complet d'un `tvshow.nfo` (UTF-8, CRLF).
pub fn ecrire_nfo_serie(
    serie: &Serie,
    conserves: &DonneesConservees,
    connecteur: Connecteur,
    options: &OptionsNfo,
) -> String {
    let racine = element_nfo_serie(serie, conserves, connecteur, options);
    assembler(&[racine], &commentaire_entete(connecteur, options))
}
