//! Éléments partagés par les NFO films, séries, épisodes et saisons :
//! connecteurs, options d'écriture, données conservées d'un ancien NFO,
//! lecture/écriture des notes, identifiants, personnes, artwork, fileinfo.

use std::collections::{BTreeMap, BTreeSet};

use crate::modele::commun::cles_id;
use crate::modele::{
    BandeAnnonce, Edition, FichierMedia, Identifiants, Note, Personne, SourceMedia, TypeFichier,
    TypePersonne,
};

use super::arbre::{Element, Noeud};
use super::outils::{
    FormatBandeAnnonce, booleen, decimal, format_note, format_note_courte,
    url_bande_annonce_depuis_nfo, url_bande_annonce_vers_nfo, votes,
};

/// Nom du logiciel inscrit dans les commentaires des NFO.
pub const NOM_LOGICIEL: &str = "MediaManagerRust";

// ---------------------------------------------------------------------------
// Connecteurs
// ---------------------------------------------------------------------------

/// Format cible d'écriture des NFO (lecteur multimédia visé).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Connecteur {
    /// Kodi ≥ 16 (défaut).
    #[default]
    Kodi,
    /// Kodi < 16.
    Xbmc,
    Emby,
    Jellyfin,
    /// Écrit comme Kodi.
    Plex,
    /// Écrit comme Kodi (films).
    Dvr3,
    /// MediaPortal : format « legacy » pour les films, Xbmc pour les séries.
    MediaPortal,
    /// MediaPortal Moving Pictures (films ; séries ⇒ Xbmc).
    MpMovingPictures,
    /// MediaPortal MyVideo (films ; séries ⇒ Xbmc).
    MpMyVideo,
}

impl Connecteur {
    /// Nom écrit dans le commentaire d'en-tête.
    pub fn nom(self) -> &'static str {
        match self {
            Connecteur::Kodi => "KODI",
            Connecteur::Xbmc => "XBMC",
            Connecteur::Emby => "EMBY",
            Connecteur::Jellyfin => "JELLYFIN",
            Connecteur::Plex => "PLEX",
            Connecteur::Dvr3 => "DVR_3",
            Connecteur::MediaPortal => "MEDIAPORTAL",
            Connecteur::MpMovingPictures => "MP_MP",
            Connecteur::MpMyVideo => "MP_MV",
        }
    }
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Réglages d'écriture des NFO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionsNfo {
    /// Sources de la note principale, par ordre de préférence.
    pub sources_notes: Vec<String>,
    /// Écrire `outline` à partir du résumé si le résumé court est vide.
    pub creer_resume_court: bool,
    /// Limiter ce résumé court aux premières phrases (≥ 20 caractères).
    pub resume_court_premiere_phrase: bool,
    /// Écrire les URL d'artwork (`thumb`, `fanart`, vignettes d'acteurs).
    pub ecrire_urls_artwork: bool,
    pub ecrire_bande_annonce: bool,
    pub ecrire_fileinfo: bool,
    pub ecrire_date_ajout: bool,
    pub ecrire_lockdata: bool,
    /// Un seul `studio` (le premier).
    pub studio_unique: bool,
    /// Séries : écrire `episodeguide` (JSON des identifiants).
    pub ecrire_guide_episodes: bool,
    /// Séries Emby/Jellyfin : écrire `enddate` si la série est terminée.
    pub ecrire_date_fin: bool,
    /// Épisodes : écrire aussi les acteurs de la série.
    pub ecrire_tous_acteurs: bool,
    /// Horodatage du commentaire d'en-tête (`None` : commentaire sans date,
    /// utile pour des sorties reproductibles).
    pub horodatage: Option<String>,
    /// Dossier `.actors` du film ou de la série : si la photo d'un acteur y
    /// existe (`<Nom_Prénom>.jpg`), le `thumb` de l'acteur la référence
    /// (chemin relatif `.actors/<fichier>`) à la place de l'URL distante.
    pub dossier_acteurs: Option<std::path::PathBuf>,
}

impl OptionsNfo {
    /// Défauts des films.
    pub fn films() -> Self {
        Self {
            sources_notes: vec!["imdb".to_string()],
            creer_resume_court: true,
            resume_court_premiere_phrase: false,
            ecrire_urls_artwork: true,
            ecrire_bande_annonce: true,
            ecrire_fileinfo: true,
            ecrire_date_ajout: true,
            ecrire_lockdata: false,
            studio_unique: false,
            ecrire_guide_episodes: true,
            ecrire_date_fin: false,
            ecrire_tous_acteurs: false,
            horodatage: None,
            dossier_acteurs: None,
        }
    }

    /// Défauts des séries (pas de résumé court automatique).
    pub fn series() -> Self {
        Self {
            creer_resume_court: false,
            ..Self::films()
        }
    }
}

impl Default for OptionsNfo {
    fn default() -> Self {
        Self::films()
    }
}

// ---------------------------------------------------------------------------
// Données conservées d'un ancien NFO
// ---------------------------------------------------------------------------

/// Données lues dans un NFO qui ne trouvent pas de place dans le modèle et
/// sont réinjectées telles quelles à la réécriture (sauf « NFO propre »).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DonneesConservees {
    pub epbookmark: Option<String>,
    pub status: Option<String>,
    pub code: Option<String>,
    /// Éléments racine non gérés, recopiés à l'identique.
    pub inconnues: Vec<Element>,
}

/// Sépare les enfants de la racine non gérés (`supportees` en minuscules).
pub fn elements_inconnus(racine: &Element, supportees: &[&str]) -> Vec<Element> {
    let ensemble: BTreeSet<String> = supportees.iter().map(|s| s.to_string()).collect();
    racine
        .elements()
        .filter(|e| !ensemble.contains(&e.nom.to_ascii_lowercase()))
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Document
// ---------------------------------------------------------------------------

/// Déclaration XML écrite en tête des NFO.
pub const DECLARATION: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;

/// Commentaire d'en-tête.
pub fn commentaire_entete(connecteur: Connecteur, options: &OptionsNfo) -> String {
    match &options.horodatage {
        Some(d) => format!("created on {d} by {NOM_LOGICIEL} for {}", connecteur.nom()),
        None => format!("created by {NOM_LOGICIEL} for {}", connecteur.nom()),
    }
}

/// Assemble un document : déclaration (si demandée), commentaire, racine ;
/// fins de ligne CRLF.
pub fn assembler(racines: &[Element], entete: &str) -> String {
    let mut s = String::new();
    for (i, r) in racines.iter().enumerate() {
        if i == 0 {
            s.push_str(DECLARATION);
            s.push('\n');
            s.push_str(&format!("<!--{entete}-->\n"));
        }
        r.serialiser(0, &mut s);
    }
    s.replace("\r\n", "\n").replace('\n', "\r\n")
}

/// Vrai si deux NFO ne diffèrent que par leurs commentaires et leurs fins de
/// ligne (pas de réécriture nécessaire).
pub fn contenu_equivalent(a: &str, b: &str) -> bool {
    fn normaliser(s: &str) -> String {
        let mut sortie = String::new();
        let mut reste = s;
        while let Some(i) = reste.find("<!--") {
            sortie.push_str(&reste[..i]);
            match reste[i..].find("-->") {
                Some(j) => reste = &reste[i + j + 3..],
                None => {
                    reste = "";
                }
            }
        }
        sortie.push_str(reste);
        sortie
            .lines()
            .map(str::trim_end)
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
    normaliser(a) == normaliser(b)
}

/// Écrit le NFO seulement si son contenu (hors commentaires) a changé.
/// Renvoie `true` si le fichier a été écrit.
pub fn ecrire_si_change(chemin: &std::path::Path, contenu: &str) -> std::io::Result<bool> {
    if let Ok(ancien) = std::fs::read(chemin) {
        let ancien = super::arbre::decoder_octets(&ancien);
        if contenu_equivalent(&ancien, contenu) {
            return Ok(false);
        }
    }
    std::fs::write(chemin, contenu.as_bytes())?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Notes
// ---------------------------------------------------------------------------

fn source_note_depuis_nfo(nom: &str) -> String {
    match nom.trim().to_ascii_lowercase().as_str() {
        "themoviedb" => "tmdb".to_string(),
        "rottentomatoes" => "tomatometerallcritics".to_string(),
        "metascore" => "metacritic".to_string(),
        "" => "default".to_string(),
        autre => autre.to_string(),
    }
}

fn source_note_vers_nfo(id: &str) -> &str {
    if id == "tmdb" { "themoviedb" } else { id }
}

/// Lit les notes : `ratings/rating`, `rating` + `votes` simples (source
/// `NFO`, seulement sans bloc `ratings`), `userrating`, `criticrating`.
pub fn lire_notes(racine: &Element) -> BTreeMap<String, Note> {
    let mut notes = BTreeMap::new();
    if let Some(bloc) = racine.enfant_unique("ratings") {
        for r in bloc.enfants_nommes("rating") {
            let source = source_note_depuis_nfo(r.attribut("name").unwrap_or(""));
            let maximum = r
                .attribut("max")
                .and_then(|m| m.trim().parse::<u32>().ok())
                .filter(|m| *m > 0)
                .unwrap_or(10);
            let valeur = r
                .texte_unique("value")
                .or_else(|| Some(r.texte_propre()))
                .and_then(|v| decimal(&v))
                .unwrap_or(0.0);
            let nb = r.texte_unique("votes").map_or(0, |v| votes(&v));
            if valeur > 0.0 {
                notes.insert(source.clone(), Note::nouvelle(&source, valeur, nb, maximum));
            }
        }
    }
    if notes.is_empty()
        && let Some(v) = racine.texte_unique("rating").and_then(|v| decimal(&v))
        && v > 0.0
    {
        let nb = racine.texte_unique("votes").map_or(0, |v| votes(&v));
        notes.insert("NFO".to_string(), Note::nouvelle("NFO", v, nb, 10));
    }
    if let Some(v) = racine.texte_unique("userrating").and_then(|v| decimal(&v))
        && v > 0.0
    {
        notes.insert("user".to_string(), Note::nouvelle("user", v, 0, 10));
    }
    if let Some(v) = racine
        .texte_unique("criticrating")
        .and_then(|v| decimal(&v))
        && v > 0.0
    {
        notes
            .entry("tomatometerallcritics".to_string())
            .or_insert_with(|| Note::nouvelle("tomatometerallcritics", v, 0, 100));
    }
    notes
}

/// Note principale : première source préférée présente, sinon la dernière
/// source autre que `user`.
pub fn note_principale<'a>(
    notes: &'a BTreeMap<String, Note>,
    sources: &[String],
) -> Option<&'a Note> {
    for s in sources {
        if let Some(n) = notes.get(s)
            && s != "user"
        {
            return Some(n);
        }
    }
    notes
        .iter()
        .filter(|(k, _)| k.as_str() != "user")
        .map(|(_, n)| n)
        .next_back()
}

/// Bloc Kodi `<ratings>` (la note `user` en est exclue). `None` si vide.
pub fn element_ratings(notes: &BTreeMap<String, Note>, sources: &[String]) -> Option<Element> {
    let principale = note_principale(notes, sources).map(|n| n.id.clone());
    let mut bloc = Element::nouveau("ratings");
    for (cle, n) in notes {
        if cle == "user" || !n.est_valide() {
            continue;
        }
        let mut r = Element::nouveau("rating")
            .attr("name", source_note_vers_nfo(cle))
            .attr("max", &n.maximum.to_string())
            .attr(
                "default",
                if principale.as_deref() == Some(n.id.as_str()) {
                    "true"
                } else {
                    "false"
                },
            );
        r.ajouter_texte("value", &format_note(n.valeur));
        r.ajouter_texte("votes", &n.votes.to_string());
        bloc.ajouter(r);
    }
    if bloc.enfants.is_empty() {
        None
    } else {
        Some(bloc)
    }
}

/// Valeur de `userrating` (`0` si absente).
pub fn texte_note_utilisateur(notes: &BTreeMap<String, Note>) -> String {
    notes
        .get("user")
        .filter(|n| n.est_valide())
        .map_or("0".to_string(), |n| format_note_courte(n.sur_dix()))
}

// ---------------------------------------------------------------------------
// Identifiants
// ---------------------------------------------------------------------------

/// Lit les balises `uniqueid` ; `type_par_defaut` sert quand l'attribut
/// `type` est absent (`None` : déduit, imdb si la valeur s'y prête).
pub fn lire_uniqueids(racine: &Element, ids: &mut Identifiants, type_par_defaut: Option<&str>) {
    for u in racine.enfants_nommes("uniqueid") {
        let valeur = u.texte_propre();
        let type_id = u
            .attribut("type")
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        let cle = match (type_id, type_par_defaut) {
            (Some(t), _) => normaliser_cle_id(&t),
            (None, Some(d)) => d.to_string(),
            (None, None) if crate::modele::est_id_imdb_valide(&valeur) => cles_id::IMDB.to_string(),
            _ => continue,
        };
        ids.definir(&cle, &valeur);
    }
}

/// Clés d'identifiants : minuscules, sauf la clé de saga qui garde sa casse.
fn normaliser_cle_id(cle: &str) -> String {
    if cle.eq_ignore_ascii_case(cles_id::TMDB_SAGA) || cle.eq_ignore_ascii_case("tmdbcollection") {
        cles_id::TMDB_SAGA.to_string()
    } else {
        cle.to_ascii_lowercase()
    }
}

/// Lit le bloc `ids` : `<entry><key/><value/></entry>` ou balises enfants.
pub fn lire_bloc_ids(racine: &Element, ids: &mut Identifiants) {
    let Some(bloc) = racine.enfant_unique("ids") else {
        return;
    };
    for e in bloc.elements() {
        if e.a_nom("entry") {
            if let (Some(k), Some(v)) = (e.texte_unique("key"), e.texte_unique("value")) {
                ids.definir(&normaliser_cle_id(&k), &v);
            }
        } else {
            ids.definir(&normaliser_cle_id(&e.nom), &e.texte_propre());
        }
    }
}

/// Balises `uniqueid` ; `default="true"` sur la première clé de `priorites`
/// présente, sinon sur la première clé.
pub fn elements_uniqueid(ids: &Identifiants, priorites: &[&str]) -> Vec<Element> {
    let defaut = priorites
        .iter()
        .find(|p| ids.contient(p))
        .map(|p| p.to_string())
        .or_else(|| ids.iter().next().map(|(k, _)| k.clone()));
    ids.iter()
        .map(|(k, v)| {
            let mut e = Element::avec_texte("uniqueid", &v.en_texte()).attr("type", k);
            if defaut.as_deref() == Some(k.as_str()) {
                e = e.attr("default", "true");
            }
            e
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Personnes
// ---------------------------------------------------------------------------

/// Libellé écrit pour un type de personne (`crew/role`).
pub fn libelle_type_personne(t: TypePersonne) -> &'static str {
    match t {
        TypePersonne::Acteur => "Actor",
        TypePersonne::Realisateur => "Director",
        TypePersonne::Scenariste => "Writer",
        TypePersonne::Producteur => "Producer",
        TypePersonne::Invite => "GuestStar",
        TypePersonne::Compositeur => "Composer",
        TypePersonne::Monteur => "Editor",
        TypePersonne::Camera => "Camera",
        TypePersonne::Autre => "Other",
    }
}

/// Type de personne déduit d'un libellé de rôle.
pub fn type_personne_depuis(libelle: &str) -> TypePersonne {
    let l = libelle.to_ascii_lowercase();
    if l.contains("guest") {
        TypePersonne::Invite
    } else if l.contains("direct") {
        TypePersonne::Realisateur
    } else if l.contains("writ") || l.contains("screenplay") || l.contains("author") {
        TypePersonne::Scenariste
    } else if l.contains("produc") {
        TypePersonne::Producteur
    } else if l.contains("compos") || l.contains("music") {
        TypePersonne::Compositeur
    } else if l.contains("edit") {
        TypePersonne::Monteur
    } else if l.contains("camera") || l.contains("photograph") {
        TypePersonne::Camera
    } else if l.contains("actor") || l.contains("actress") {
        TypePersonne::Acteur
    } else {
        TypePersonne::Autre
    }
}

fn lire_ids_personne_enfants(e: &Element, ids: &mut Identifiants) {
    for (balise, cle) in [
        ("tmdbid", cles_id::TMDB),
        ("imdbid", cles_id::IMDB),
        ("tvdbid", cles_id::TVDB),
    ] {
        if let Some(v) = e.texte_unique(balise) {
            ids.definir(cle, &v);
        }
    }
}

fn lire_ids_personne_attributs(e: &Element, ids: &mut Identifiants) {
    for (attr, cle) in [
        ("tmdbid", cles_id::TMDB),
        ("imdbid", cles_id::IMDB),
        ("tvdbid", cles_id::TVDB),
    ] {
        if let Some(v) = e.attribut(attr) {
            ids.definir(cle, v);
        }
    }
}

/// Lit une personne à sous-balises (`actor`, `producer`, `crew`).
pub fn lire_personne(e: &Element, type_personne: TypePersonne) -> Option<Personne> {
    let nom = e.texte_unique("name")?;
    let mut p = Personne {
        type_personne,
        nom,
        role: e.texte_unique("role").unwrap_or_default(),
        url_vignette: e.texte_unique("thumb").unwrap_or_default(),
        url_profil: e.texte_unique("profile").unwrap_or_default(),
        ids: Identifiants::new(),
    };
    lire_ids_personne_enfants(e, &mut p.ids);
    lire_ids_personne_attributs(e, &mut p.ids);
    Some(p)
}

/// Lit `crew` : `role` donne le type, son attribut `subrole` le rôle précis.
pub fn lire_crew(e: &Element) -> Option<Personne> {
    let mut p = lire_personne(e, TypePersonne::Autre)?;
    let role = e.enfant_unique("role");
    p.type_personne = type_personne_depuis(&role.map(Element::texte_propre).unwrap_or_default());
    p.role = role
        .and_then(|r| r.attribut("subrole"))
        .unwrap_or("")
        .to_string();
    Some(p)
}

/// Lit `director` / `credits` (texte, éventuellement plusieurs noms).
pub fn lire_personnes_texte(
    racine: &Element,
    balise: &str,
    type_personne: TypePersonne,
    role: &str,
) -> Vec<Personne> {
    let mut v = Vec::new();
    for e in racine.enfants_nommes(balise) {
        let noms = super::outils::decouper_liste(&e.texte_propre());
        let seul = noms.len() == 1;
        for nom in noms {
            let mut p = Personne {
                type_personne,
                nom,
                role: role.to_string(),
                ..Personne::default()
            };
            if seul {
                lire_ids_personne_attributs(e, &mut p.ids);
            }
            v.push(p);
        }
    }
    v
}

/// Équipe : `crew` s'il existe, sinon réalisateurs, scénaristes, producteurs.
pub fn lire_equipe(racine: &Element) -> Vec<Personne> {
    let crew: Vec<Personne> = racine
        .enfants_nommes("crew")
        .filter_map(lire_crew)
        .collect();
    if !crew.is_empty() {
        return crew;
    }
    let mut v = lire_personnes_texte(racine, "director", TypePersonne::Realisateur, "Director");
    v.extend(lire_personnes_texte(
        racine,
        "credits",
        TypePersonne::Scenariste,
        "Writer",
    ));
    v.extend(
        racine
            .enfants_nommes("producer")
            .filter_map(|e| lire_personne(e, TypePersonne::Producteur)),
    );
    v
}

fn ajouter_ids_personne_attributs(mut e: Element, p: &Personne) -> Element {
    if p.ids.tmdb() > 0 {
        e = e.attr("tmdbid", &p.ids.tmdb().to_string());
    }
    if !p.ids.imdb().is_empty() {
        e = e.attr("imdbid", &p.ids.imdb());
    }
    if p.ids.tvdb() > 0 {
        e = e.attr("tvdbid", &p.ids.tvdb().to_string());
    }
    e
}

fn ajouter_ids_personne_enfants(e: &mut Element, p: &Personne) {
    if p.ids.tmdb() > 0 {
        e.ajouter_texte("tmdbid", &p.ids.tmdb().to_string());
    }
    if !p.ids.imdb().is_empty() {
        e.ajouter_texte("imdbid", &p.ids.imdb());
    }
    if p.ids.tvdb() > 0 {
        e.ajouter_texte("tvdbid", &p.ids.tvdb().to_string());
    }
}

/// `<director tmdbid=…>Nom</director>` (idem `credits`).
pub fn element_personne_texte(balise: &str, p: &Personne) -> Element {
    ajouter_ids_personne_attributs(Element::avec_texte(balise, &p.nom), p)
}

/// `<actor>` : name, role, [type], thumb, profile, ids en sous-balises.
pub fn element_acteur(p: &Personne, type_ecrit: Option<&str>, urls: bool) -> Element {
    element_acteur_avec_thumb(
        p,
        type_ecrit,
        (urls && !p.url_vignette.is_empty()).then_some(p.url_vignette.as_str()),
    )
}

/// Nom de fichier de la photo d'un acteur (sans extension) : espaces
/// remplacés par `_`, caractères interdits retirés (`Keanu_Reeves`).
pub fn nom_fichier_acteur(nom: &str) -> String {
    let nettoye: String = nom
        .trim()
        .chars()
        .filter(|c| !c.is_control() && !"<>:\"/\\|?*".contains(*c))
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect();
    nettoye.trim_matches(['.', '_']).to_string()
}

/// Extensions reconnues pour la photo d'un acteur.
const EXTENSIONS_ACTEUR: [&str; 4] = ["jpg", "jpeg", "png", "webp"];

/// Photo locale de l'acteur dans `dossier` : `.actors/<fichier>` si elle existe.
fn thumb_local(p: &Personne, dossier: &std::path::Path) -> Option<String> {
    let base = nom_fichier_acteur(&p.nom);
    if base.is_empty() {
        return None;
    }
    let nom_dossier = dossier.file_name()?.to_string_lossy().into_owned();
    EXTENSIONS_ACTEUR.iter().find_map(|ext| {
        let nom = format!("{base}.{ext}");
        dossier
            .join(&nom)
            .is_file()
            .then(|| format!("{nom_dossier}/{nom}"))
    })
}

/// Comme [`element_acteur`], mais le `thumb` désigne la photo locale de
/// `.actors` quand elle existe (sinon l'URL, si les URL sont écrites).
pub fn element_acteur_options(p: &Personne, type_ecrit: Option<&str>, o: &OptionsNfo) -> Element {
    let local = o.dossier_acteurs.as_deref().and_then(|d| thumb_local(p, d));
    match local {
        Some(l) => element_acteur_avec_thumb(p, type_ecrit, Some(&l)),
        None => element_acteur(p, type_ecrit, o.ecrire_urls_artwork),
    }
}

fn element_acteur_avec_thumb(
    p: &Personne,
    type_ecrit: Option<&str>,
    thumb: Option<&str>,
) -> Element {
    let mut e = Element::nouveau("actor");
    e.ajouter_texte("name", &p.nom);
    e.ajouter_texte("role", &p.role);
    if let Some(t) = type_ecrit {
        e.ajouter_texte("type", t);
    }
    if let Some(t) = thumb {
        e.ajouter_texte("thumb", t);
    }
    if !p.url_profil.is_empty() {
        e.ajouter_texte("profile", &p.url_profil);
    }
    ajouter_ids_personne_enfants(&mut e, p);
    e
}

/// `<producer tmdbid=…>` : name, role, thumb, profile (ids en attributs).
pub fn element_producteur(p: &Personne, urls: bool) -> Element {
    let mut e = ajouter_ids_personne_attributs(Element::nouveau("producer"), p);
    e.ajouter_texte("name", &p.nom);
    e.ajouter_texte("role", &p.role);
    if urls && !p.url_vignette.is_empty() {
        e.ajouter_texte("thumb", &p.url_vignette);
    }
    if !p.url_profil.is_empty() {
        e.ajouter_texte("profile", &p.url_profil);
    }
    e
}

/// `<crew>` du bloc de métadonnées propres.
pub fn element_crew(p: &Personne) -> Element {
    let mut e = Element::nouveau("crew");
    e.ajouter_texte("name", &p.nom);
    let mut role = Element::avec_texte("role", libelle_type_personne(p.type_personne));
    if !p.role.is_empty() {
        role = role.attr("subrole", &p.role);
    }
    e.ajouter(role);
    if !p.url_vignette.is_empty() {
        e.ajouter_texte("thumb", &p.url_vignette);
    }
    if !p.url_profil.is_empty() {
        e.ajouter_texte("profile", &p.url_profil);
    }
    ajouter_ids_personne_enfants(&mut e, p);
    e
}

// ---------------------------------------------------------------------------
// Artwork
// ---------------------------------------------------------------------------

/// Correspondance attribut `aspect` ↔ type d'artwork, dans l'ordre d'écriture.
pub const ASPECTS_ARTWORK: [(&str, TypeFichier); 10] = [
    ("poster", TypeFichier::Poster),
    ("banner", TypeFichier::Banner),
    ("clearart", TypeFichier::Clearart),
    ("clearlogo", TypeFichier::Clearlogo),
    ("discart", TypeFichier::Disc),
    ("landscape", TypeFichier::Thumb),
    ("keyart", TypeFichier::Keyart),
    ("squareart", TypeFichier::Squareart),
    ("logo", TypeFichier::Logo),
    ("characterart", TypeFichier::Characterart),
];

fn est_url(s: &str) -> bool {
    let s = s.trim().to_ascii_lowercase();
    s.starts_with("http://") || s.starts_with("https://")
}

/// Lit les `thumb` (URL http seulement ; sans `aspect` ⇒ poster). Les
/// `thumb` de saison (`type="season"`) sont ignorés ici.
pub fn lire_thumbs(racine: &Element, urls: &mut BTreeMap<TypeFichier, String>) {
    for t in racine.enfants_nommes("thumb") {
        if t.attribut("type")
            .is_some_and(|v| v.eq_ignore_ascii_case("season"))
        {
            continue;
        }
        let url = t.texte_propre();
        if !est_url(&url) {
            continue;
        }
        let aspect = t
            .attribut("aspect")
            .unwrap_or("poster")
            .to_ascii_lowercase();
        let type_fichier = match aspect.as_str() {
            "thumb" => Some(TypeFichier::Thumb),
            a => ASPECTS_ARTWORK
                .iter()
                .find(|(n, _)| *n == a)
                .map(|(_, t)| *t),
        };
        if let Some(tf) = type_fichier {
            urls.entry(tf).or_insert(url);
        }
    }
}

/// Lit `fanart` : `<fanart><thumb>url</thumb>…</fanart>` ou texte direct.
/// Renvoie toutes les URL dans l'ordre.
pub fn lire_fanarts(racine: &Element) -> Vec<String> {
    let Some(f) = racine.enfant_unique("fanart") else {
        return Vec::new();
    };
    let mut v: Vec<String> = f
        .enfants_nommes("thumb")
        .map(Element::texte_propre)
        .filter(|u| est_url(u))
        .collect();
    if v.is_empty() {
        let t = f.texte_propre();
        if est_url(&t) {
            v.push(t);
        }
    }
    v
}

/// `thumb` avec attribut `aspect` pour chaque type d'artwork connu.
pub fn elements_thumbs_aspect(
    urls: &BTreeMap<TypeFichier, String>,
    aspects_admis: &[&str],
) -> Vec<Element> {
    ASPECTS_ARTWORK
        .iter()
        .filter(|(a, _)| aspects_admis.contains(a))
        .filter_map(|(a, t)| {
            urls.get(t)
                .filter(|u| !u.is_empty())
                .map(|u| Element::avec_texte("thumb", u).attr("aspect", a))
        })
        .collect()
}

/// `<fanart><thumb>…</thumb>…</fanart>` (`None` si aucune URL).
pub fn element_fanart(urls: &[&str]) -> Option<Element> {
    let urls: Vec<&&str> = urls.iter().filter(|u| !u.is_empty()).collect();
    if urls.is_empty() {
        return None;
    }
    let mut f = Element::nouveau("fanart");
    for u in urls {
        f.ajouter_texte("thumb", u);
    }
    Some(f)
}

// ---------------------------------------------------------------------------
// Bandes-annonces
// ---------------------------------------------------------------------------

/// Lit les `trailer` (convertis depuis les URL d'extension YouTube).
pub fn lire_bandes_annonces(racine: &Element) -> Vec<BandeAnnonce> {
    racine
        .enfants_nommes("trailer")
        .map(Element::texte_propre)
        .filter(|u| !u.is_empty())
        .map(|u| BandeAnnonce {
            url: url_bande_annonce_depuis_nfo(&u),
            nom: "fromNFO".to_string(),
            fournisseur: "from NFO".to_string(),
            qualite: "unknown".to_string(),
            dans_nfo: true,
            ..BandeAnnonce::default()
        })
        .collect()
}

/// URL de la bande-annonce à écrire : première marquée « dans le NFO », à
/// défaut la première tout court.
pub fn url_bande_annonce(bandes: &[BandeAnnonce], format: FormatBandeAnnonce) -> Option<String> {
    bandes
        .iter()
        .find(|b| b.dans_nfo && !b.url.is_empty())
        .or_else(|| bandes.iter().find(|b| !b.url.is_empty()))
        .map(|b| url_bande_annonce_vers_nfo(&b.url, format))
}

// ---------------------------------------------------------------------------
// Source média, édition
// ---------------------------------------------------------------------------

const NOMS_SOURCES: [(SourceMedia, &str); 18] = [
    (SourceMedia::UhdBluray, "UHD_BLURAY"),
    (SourceMedia::Bluray, "BLURAY"),
    (SourceMedia::Dvd, "DVD"),
    (SourceMedia::HdDvd, "HDDVD"),
    (SourceMedia::Tv, "TV"),
    (SourceMedia::Vhs, "VHS"),
    (SourceMedia::Laserdisc, "LASERDISC"),
    (SourceMedia::DVhs, "D_VHS"),
    (SourceMedia::HdRip, "HDRIP"),
    (SourceMedia::Cam, "CAM"),
    (SourceMedia::Ts, "TS"),
    (SourceMedia::Tc, "TC"),
    (SourceMedia::DvdScr, "DVDSCR"),
    (SourceMedia::R5, "R5"),
    (SourceMedia::WebRip, "WEBRIP"),
    (SourceMedia::WebDl, "WEB_DL"),
    (SourceMedia::Stream, "STREAM"),
    (SourceMedia::Inconnue, "UNKNOWN"),
];

/// Nom écrit dans `<source>`.
pub fn nom_source(s: SourceMedia) -> &'static str {
    NOMS_SOURCES
        .iter()
        .find(|(x, _)| *x == s)
        .map_or("UNKNOWN", |(_, n)| n)
}

/// Source lue dans `<source>` : nom d'énumération, sinon détection par motif.
pub fn source_depuis(texte: &str) -> SourceMedia {
    let t = texte.trim();
    if let Some((s, _)) = NOMS_SOURCES.iter().find(|(_, n)| n.eq_ignore_ascii_case(t)) {
        return *s;
    }
    SourceMedia::depuis_chemin(t)
}

const NOMS_EDITIONS: [(&str, Edition); 14] = [
    ("NONE", Edition::Aucune),
    ("DIRECTORS_CUT", Edition::DirectorsCut),
    ("EXTENDED_EDITION", Edition::Etendue),
    ("THEATRICAL_EDITION", Edition::Cinema),
    ("UNRATED", Edition::NonClassee),
    ("UNCUT", Edition::NonCoupee),
    ("IMAX", Edition::Imax),
    ("REMASTERED", Edition::Remasterisee),
    ("COLLECTORS_EDITION", Edition::Collector),
    ("ULTIMATE_EDITION", Edition::Ultime),
    ("FINAL_CUT", Edition::FinalCut),
    ("SPECIAL_EDITION", Edition::Speciale),
    ("CRITERION_COLLECTION", Edition::Criterion),
    ("OPEN_MATTE", Edition::OpenMatte),
];

/// Nom écrit dans `<edition>` (texte libre pour une édition personnalisée).
pub fn nom_edition(e: &Edition) -> String {
    if let Edition::Personnalisee(s) = e {
        return s.clone();
    }
    NOMS_EDITIONS
        .iter()
        .find(|(_, x)| x == e)
        .map_or(String::new(), |(n, _)| n.to_string())
}

/// Édition lue strictement : nom d'énumération ou libellé exact, sinon
/// édition personnalisée.
pub fn edition_depuis(texte: &str) -> Edition {
    let t = texte.trim();
    if t.is_empty() {
        return Edition::Aucune;
    }
    if let Some((_, e)) = NOMS_EDITIONS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(t))
    {
        return e.clone();
    }
    if let Some((_, e)) = NOMS_EDITIONS
        .iter()
        .find(|(_, e)| *e != Edition::Aucune && e.libelle().eq_ignore_ascii_case(t))
    {
        return e.clone();
    }
    Edition::Personnalisee(t.to_string())
}

// ---------------------------------------------------------------------------
// fileinfo / streamdetails
// ---------------------------------------------------------------------------

/// Codec vidéo au format NFO.
pub fn codec_video_nfo(codec: &str, ancien: bool) -> String {
    let c = codec.trim().to_ascii_lowercase();
    match c.as_str() {
        "avc" | "h.264" => "h264".to_string(),
        "h265" | "h.265" | "hevc" => if ancien { "HEVC" } else { "hevc" }.to_string(),
        _ if c.starts_with("mpeg-") => c.replace('-', ""),
        _ => c,
    }
}

/// Codec audio au format NFO.
pub fn codec_audio_nfo(codec: &str) -> String {
    let c = codec.trim().to_ascii_lowercase();
    match c.as_str() {
        "ac3ex" => "ac3",
        "adpcm_ms" => "pcm",
        "dts" | "dts-es" | "dts-96/24" => "dca",
        "truehd/atmos" => "truehd_atmos",
        "dtshd-hra" => "dtshd_hra",
        "dtshd-ma" => "dtshd_ma",
        "dts-x" => "dtshd_ma_x",
        "dts-x-imax" => "dtshd_ma_x_imax",
        "eac3/atmos" => "eac3_ddp_atmos",
        _ => return c,
    }
    .to_string()
}

/// Résolution au format NFO (`4K`, `8K`, sinon hauteur de format sans `p`).
pub fn resolution_nfo(largeur: u32, hauteur: u32) -> String {
    let (l, h) = (largeur, hauteur);
    if l >= 7680 || h >= 4320 {
        "8K".to_string()
    } else if l >= 3840 || h >= 2160 {
        "4K".to_string()
    } else if l >= 1920 || h >= 1080 {
        "1080".to_string()
    } else if l >= 1280 || h >= 720 {
        "720".to_string()
    } else if h >= 576 {
        "576".to_string()
    } else if h >= 540 {
        "540".to_string()
    } else if h >= 480 {
        "480".to_string()
    } else {
        h.to_string()
    }
}

fn hdr_nfo(hdr: &str) -> String {
    let h = hdr.trim().to_ascii_lowercase();
    if h.contains("dolby vision") {
        "dolbyvision".to_string()
    } else if h.contains("hdr10+") {
        "hdr10plus".to_string()
    } else {
        h.split_whitespace().next().unwrap_or("").to_string()
    }
}

fn stereo_nfo(format_3d: &str, ancien: bool) -> String {
    let f = format_3d.to_ascii_uppercase();
    let (sbs, tab) = if ancien {
        ("left_right", "top_bottom")
    } else {
        ("split_vertical", "split_horizontal")
    };
    if f.contains("SBS") {
        sbs.to_string()
    } else if f.contains("TAB") {
        tab.to_string()
    } else {
        String::new()
    }
}

/// `<fileinfo><streamdetails>…</streamdetails></fileinfo>` pour un fichier
/// vidéo et des fichiers de sous-titres externes. `None` sans vidéo connue.
pub fn element_fileinfo(
    video: &FichierMedia,
    sous_titres_externes: &[&FichierMedia],
    ancien: bool,
) -> Option<Element> {
    let v = &video.video;
    if v.codec.is_empty() && v.largeur == 0 && video.flux_audio.is_empty() {
        return None;
    }
    let mut sd = Element::nouveau("streamdetails");
    let mut ev = Element::nouveau("video");
    ev.ajouter_texte("codec", &codec_video_nfo(&v.codec, ancien));
    let aspect = if ancien {
        format!("{}", v.ratio_aspect)
    } else {
        format!("{:.2}", v.ratio_aspect)
    };
    ev.ajouter_texte("aspect", &aspect);
    ev.ajouter_texte("width", &v.largeur.to_string());
    ev.ajouter_texte("height", &v.hauteur.to_string());
    if !ancien {
        ev.ajouter_texte("resolution", &resolution_nfo(v.largeur, v.hauteur));
        let hdr = hdr_nfo(&v.format_hdr);
        if !hdr.is_empty() {
            ev.ajouter_texte("hdrtype", &hdr);
        }
    }
    let stereo = stereo_nfo(&v.format_3d, ancien);
    if !stereo.is_empty() {
        ev.ajouter_texte("stereomode", &stereo);
    }
    ev.ajouter_texte("durationinseconds", &video.duree_secondes.to_string());
    sd.ajouter(ev);
    for a in &video.flux_audio {
        let mut ea = Element::nouveau("audio");
        ea.ajouter_texte("codec", &codec_audio_nfo(&a.codec));
        ea.ajouter_texte("language", &a.langue);
        ea.ajouter_texte("channels", &a.canaux.to_string());
        sd.ajouter(ea);
    }
    let externes = sous_titres_externes
        .iter()
        .flat_map(|f| f.sous_titres.iter());
    for s in video.sous_titres.iter().chain(externes) {
        let mut es = Element::nouveau("subtitle");
        es.ajouter_texte("language", &s.langue);
        sd.ajouter(es);
    }
    let mut fi = Element::nouveau("fileinfo");
    fi.ajouter(sd);
    Some(fi)
}

/// Somme de contrôle CRC32 d'un fichier, si connue.
pub fn crc32(f: &FichierMedia) -> Option<&str> {
    f.sommes_controle
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("crc32"))
        .map(|(_, v)| v.as_str())
        .filter(|v| !v.is_empty())
}

/// Balise de verrou écrite par d'anciens NFO (lue seulement, jamais écrite).
pub const BALISE_VERROU_ANCIENNE: &str = "tmm_locked";

/// Booléen de verrou du bloc de métadonnées propres : `locked`, ou par
/// compatibilité avec d'anciens NFO la balise [`BALISE_VERROU_ANCIENNE`].
pub fn lire_verrou(racine: &Element) -> bool {
    racine.texte_unique("locked").is_some_and(|v| booleen(&v))
        || racine
            .texte_unique(BALISE_VERROU_ANCIENNE)
            .is_some_and(|v| booleen(&v))
}

/// Ajoute un nœud texte non vide (utilitaire d'écriture).
pub fn ajouter_si_non_vide(e: &mut Element, balise: &str, texte: &str) {
    if !texte.is_empty() {
        e.ajouter_texte(balise, texte);
    }
}

/// Ajoute le commentaire de séparation du bloc de métadonnées propres.
pub fn ajouter_commentaire_meta(e: &mut Element) {
    e.enfants
        .push(Noeud::Commentaire(format!("{NOM_LOGICIEL} meta data")));
}

#[cfg(test)]
mod tests_verrou_ancien {
    use crate::nfo::lire_nfo_film;

    #[test]
    fn verrou_ancien_relu() {
        let nfo =
            "<?xml version=\"1.0\"?><movie><title>A</title><tmm_locked>true</tmm_locked></movie>";
        assert!(lire_nfo_film(nfo).film.verrouille);
        let nfo2 = "<movie><title>A</title><locked>true</locked></movie>";
        assert!(lire_nfo_film(nfo2).film.verrouille);
        let nfo3 = "<movie><title>A</title></movie>";
        assert!(!lire_nfo_film(nfo3).film.verrouille);
    }
}
