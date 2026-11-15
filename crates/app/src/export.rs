//! Export de la bibliothèque par modèles : liste des films ou des séries
//! rendue par le moteur de gabarits (`gabarit`), avec modèles intégrés (liste
//! HTML détaillée, CSV, texte), modèles de l'utilisateur et copie optionnelle
//! des affiches en vignettes.
//!
//! # Modèles de l'utilisateur
//!
//! Chaque sous-dossier de `<données>/modeles_export/` est un modèle s'il
//! contient un fichier `template.conf` (lignes `clé=valeur`, `#` pour les
//! commentaires) :
//!
//! | Clé | Rôle |
//! |---|---|
//! | `name` | Nom affiché (défaut : nom du dossier) |
//! | `type` | `MOVIE` ou `TV_SHOW` (obligatoire ; les autres sont ignorés) |
//! | `list` | Fichier gabarit de la liste (obligatoire) |
//! | `detail` | Fichier gabarit d'une page de détail (facultatif) |
//! | `filename` | Nom du fichier de liste produit, sans extension |
//! | `extension` | Extension produite (`html` par défaut) ; elle choisit l'échappement : `html`, `xml`, `json`, `csv` ; autre = aucun |
//! | `description` | Description |
//!
//! Les autres fichiers du dossier (feuilles de style, images, scripts) sont
//! copiés tels quels dans le dossier de sortie.
//!
//! # Variables des gabarits
//!
//! - liste : `title` (titre de la liste), `count`, `generated` (date),
//!   `movies` (films) ou `tvShows` (séries) ;
//! - détail : `movie` ou `tvShow`, `title`, `generated`.
//!
//! Un film (`movie.*`) expose les variables du renommeur (`title`, `year`,
//! `rating.rating`, `genres`, `actors[].name`…) et, en plus : `poster`
//! (chemin relatif de la vignette, vide sans affiche), `detailFile` (page de
//! détail), `dateAdded`, `path`, `fileName`, `directorsAsString`,
//! `writersAsString`, `actorsAsString`, `watched`, `plot`, `runtime`,
//! `movieSetTitle`. Une série (`tvShow.*`) expose en plus `seasons[]`
//! (`number`, `title`, `episodes[]`), `episodes[]`, `episodeCount`,
//! `watchedCount`, `poster`, `detailFile`, `path`. Un épisode : `season`,
//! `episode`, `title`, `plot`, `firstAired`, `watched`, `rating.rating`,
//! `fileName`.
//!
//! Toutes les valeurs textuelles sont échappées selon l'extension (HTML,
//! XML, JSON ou CSV) avant le rendu.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use mediamanager_core::echappement::{Echappement, echapper_valeur};
use mediamanager_core::modele::{Film, Saga, Serie, TypeFichier};
use mediamanager_core::renommage::{contexte_episode, contexte_film, contexte_serie};
use mediamanager_gabarit::{Contexte, DateHeure, Gabarit, Jetons, OptionsRendu, Valeur};

use crate::application::Noyau;
use crate::artwork::ecrire_atomiquement;
use crate::erreur::{ErreurApp, Resultat};
use crate::evenements::NiveauMessage;
use crate::taches::ContexteTache;

/// Dossier des modèles de l'utilisateur, dans le dossier de données.
pub const DOSSIER_MODELES_EXPORT: &str = "modeles_export";
/// Nom du fichier de description d'un modèle de l'utilisateur.
pub const FICHIER_CONFIG_MODELE: &str = "template.conf";
/// Taille maximale d'un fichier gabarit.
const TAILLE_MAX_GABARIT: u64 = 5 * 1024 * 1024;

/// Ce que le modèle exporte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypeExport {
    Films,
    Series,
}

/// Modèle d'export.
#[derive(Debug, Clone, PartialEq)]
pub struct ModeleExport {
    /// Identifiant : `integre:<nom>` ou `utilisateur:<dossier>`.
    pub id: String,
    pub nom: String,
    pub description: String,
    pub type_export: TypeExport,
    /// Nom du fichier de liste, sans extension.
    pub nom_fichier: String,
    /// Extension produite (sans point) ; fixe aussi l'échappement.
    pub extension: String,
    pub gabarit_liste: String,
    /// Gabarit d'une page de détail par élément (facultatif).
    pub gabarit_detail: Option<String>,
    /// Dossier d'un modèle de l'utilisateur (fichiers annexes copiés).
    pub dossier: Option<PathBuf>,
    /// Fichiers du dossier à ne pas copier (gabarits).
    pub fichiers_exclus: Vec<PathBuf>,
    pub integre: bool,
}

impl ModeleExport {
    /// Échappement appliqué aux valeurs.
    pub fn echappement(&self) -> Echappement {
        Echappement::depuis_extension(&self.extension)
    }
}

/// Options d'un export.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptionsExport {
    /// Copie des affiches dans `images/` : `Some(largeur)` = vignettes d'au
    /// plus `largeur` pixels de large (JPEG) ; `Some(0)` = copie sans
    /// redimensionnement ; `None` = pas de copie.
    pub vignettes: Option<u32>,
    /// Titre de la liste (défaut : « Films » / « Séries »).
    pub titre: Option<String>,
}

/// Bilan d'un export.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapportExport {
    /// Fichier de liste produit.
    pub fichier_liste: PathBuf,
    /// Pages de détail produites.
    pub pages_detail: Vec<PathBuf>,
    /// Éléments exportés.
    pub exportes: usize,
    /// Vignettes ou affiches copiées.
    pub vignettes: Vec<PathBuf>,
    /// Fichiers annexes du modèle copiés.
    pub annexes: Vec<PathBuf>,
    /// Identifiants demandés mais introuvables.
    pub introuvables: Vec<String>,
    pub avertissements: Vec<String>,
}

// ---------------------------------------------------------------------------
// Modèles intégrés
// ---------------------------------------------------------------------------

const CSS_HTML: &str = "body{font-family:system-ui,sans-serif;margin:2rem auto;max-width:60rem;color:#222;background:#fafafa}\
h1{margin-bottom:.2rem}.meta{color:#666;margin-top:0}\
article{display:flex;gap:1rem;padding:1rem 0;border-bottom:1px solid #ddd}\
article img{width:120px;height:auto;align-self:flex-start;border-radius:4px}\
article h2{margin:.1rem 0;font-size:1.2rem}.infos p{margin:.3rem 0}\
.vu{color:#2a7a2a;font-weight:600}.etiquette{color:#666}\
table{border-collapse:collapse}td,th{padding:.2rem .6rem;border:1px solid #ddd;text-align:left}";

fn gabarit_html_films() -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"fr\"><head><meta charset=\"utf-8\">\
<title>${{title}}</title><style>{CSS_HTML}</style></head><body>\n\
<h1>${{title}}</h1>\n<p class=\"meta\">${{count}} film(s) — ${{generated;date(yyyy-MM-dd)}}</p>\n\
${{foreach movies movie}}<article id=\"film-${{index_movie}}\">\
${{if movie.poster}}<img src=\"${{movie.poster}}\" alt=\"\">${{end}}\
<div class=\"infos\"><h2>${{if movie.detailFile}}<a href=\"${{movie.detailFile}}\">${{movie.title}}</a>${{else}}${{movie.title}}${{end}}\
${{if movie.year}} (${{movie.year}})${{end}}</h2>\n\
${{if movie.originalTitle}}<p class=\"etiquette\">${{movie.originalTitle}}</p>\n${{end}}\
<p>${{if movie.rating.rating}}Note : ${{movie.rating.rating}}/10 · ${{end}}\
${{if movie.runtime}}${{movie.runtime}} min · ${{end}}\
${{if movie.certification}}${{movie.certification}} · ${{end}}${{movie.genresAsString}}\
${{if movie.watched}} · <span class=\"vu\">vu</span>${{end}}</p>\n\
${{if movie.tagline}}<p><em>${{movie.tagline}}</em></p>\n${{end}}\
${{if movie.plot}}<p>${{movie.plot}}</p>\n${{end}}\
${{if movie.directorsAsString}}<p><span class=\"etiquette\">Réalisation :</span> ${{movie.directorsAsString}}</p>\n${{end}}\
${{if movie.actorsAsString}}<p><span class=\"etiquette\">Avec :</span> ${{movie.actorsAsString}}</p>\n${{end}}\
</div></article>\n${{end}}</body></html>\n"
    )
}

const GABARIT_DETAIL_HTML_FILMS: &str = "<!DOCTYPE html>\n<html lang=\"fr\"><head><meta charset=\"utf-8\">\
<title>${movie.title}</title></head><body>\n<h1>${movie.title}${if movie.year} (${movie.year})${end}</h1>\n<p>${movie.plot}</p>\n</body></html>\n";

const GABARIT_CSV_FILMS: &str = "\u{FEFF}Titre;Titre original;Année;Durée (min);Note;Votes;Genres;Certification;Vu;Réalisateurs;Acteurs;Résumé\r\n\
${foreach movies movie}${movie.title};${movie.originalTitle};${movie.year};${movie.runtime};${movie.rating.rating};${movie.rating.votes};${movie.genresAsString};${movie.certification};${if movie.watched}oui${else}non${end};${movie.directorsAsString};${movie.actorsAsString};${movie.plot}\r\n${end}";

const GABARIT_TEXTE_FILMS: &str = "${title} (${count})\n\n\
${foreach movies movie}${movie.title}${if movie.year} (${movie.year})${end}${if movie.genresAsString} — ${movie.genresAsString}${end}${if movie.watched} [vu]${end}\n${end}";

fn gabarit_html_series() -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"fr\"><head><meta charset=\"utf-8\">\
<title>${{title}}</title><style>{CSS_HTML}</style></head><body>\n\
<h1>${{title}}</h1>\n<p class=\"meta\">${{count}} série(s) — ${{generated;date(yyyy-MM-dd)}}</p>\n\
${{foreach tvShows tvShow}}<article id=\"serie-${{index_tvShow}}\">\
${{if tvShow.poster}}<img src=\"${{tvShow.poster}}\" alt=\"\">${{end}}\
<div class=\"infos\"><h2>${{tvShow.title}}${{if tvShow.year}} (${{tvShow.year}})${{end}}</h2>\n\
<p>${{if tvShow.rating.rating}}Note : ${{tvShow.rating.rating}}/10 · ${{end}}\
${{if tvShow.status}}${{tvShow.status}} · ${{end}}${{tvShow.genresAsString}} · \
${{tvShow.watchedCount}}/${{tvShow.episodeCount}} épisode(s) vu(s)</p>\n\
${{foreach tvShow.seasons season}}<details><summary>${{if season.title}}${{season.title}}${{else}}Saison ${{season.number}}${{end}}</summary><table>\n\
${{foreach season.episodes episode}}<tr><td>${{episode.episode}}</td><td>${{episode.title}}</td>\
<td>${{episode.firstAired;date(yyyy-MM-dd)}}</td><td>${{if episode.watched}}vu${{end}}</td></tr>\n${{end}}\
</table></details>\n${{end}}</div></article>\n${{end}}</body></html>\n"
    )
}

const GABARIT_CSV_SERIES: &str = "\u{FEFF}Titre;Titre original;Année;Statut;Note;Genres;Certification;Saisons;Épisodes;Épisodes vus;Résumé\r\n\
${foreach tvShows tvShow}${tvShow.title};${tvShow.originalTitle};${tvShow.year};${tvShow.status};${tvShow.rating.rating};${tvShow.genresAsString};${tvShow.certification};${tvShow.seasonCount};${tvShow.episodeCount};${tvShow.watchedCount};${tvShow.plot}\r\n${end}";

const GABARIT_TEXTE_SERIES: &str = "${title} (${count})\n\n\
${foreach tvShows tvShow}${tvShow.title}${if tvShow.year} (${tvShow.year})${end} — ${tvShow.watchedCount}/${tvShow.episodeCount} épisode(s) vu(s)\n${foreach tvShow.seasons season}  ${if season.title}${season.title}${else}Saison ${season.number}${end}\n${foreach season.episodes episode}    ${episode.episode}. ${episode.title}${if episode.watched} [vu]${end}\n${end}${end}${end}";

#[allow(clippy::too_many_arguments)]
fn integre(
    id: &str,
    nom: &str,
    description: &str,
    type_export: TypeExport,
    nom_fichier: &str,
    extension: &str,
    liste: String,
    detail: Option<&str>,
) -> ModeleExport {
    ModeleExport {
        id: format!("integre:{id}"),
        nom: nom.to_string(),
        description: description.to_string(),
        type_export,
        nom_fichier: nom_fichier.to_string(),
        extension: extension.to_string(),
        gabarit_liste: liste,
        gabarit_detail: detail.map(str::to_string),
        dossier: None,
        fichiers_exclus: Vec::new(),
        integre: true,
    }
}

/// Modèles intégrés d'un type d'export.
pub fn modeles_integres(type_export: TypeExport) -> Vec<ModeleExport> {
    match type_export {
        TypeExport::Films => vec![
            integre(
                "films-html",
                "Liste HTML détaillée",
                "Page HTML avec affiche, note, genres, résumé et distribution",
                TypeExport::Films,
                "films",
                "html",
                gabarit_html_films(),
                None,
            ),
            integre(
                "films-html-pages",
                "Liste HTML avec pages de détail",
                "Comme la liste détaillée, avec une page par film",
                TypeExport::Films,
                "films",
                "html",
                gabarit_html_films(),
                Some(GABARIT_DETAIL_HTML_FILMS),
            ),
            integre(
                "films-csv",
                "Liste CSV",
                "Tableau CSV (séparateur « ; », UTF-8 avec BOM) pour tableur",
                TypeExport::Films,
                "films",
                "csv",
                GABARIT_CSV_FILMS.to_string(),
                None,
            ),
            integre(
                "films-texte",
                "Liste texte",
                "Un film par ligne",
                TypeExport::Films,
                "films",
                "txt",
                GABARIT_TEXTE_FILMS.to_string(),
                None,
            ),
        ],
        TypeExport::Series => vec![
            integre(
                "series-html",
                "Liste HTML détaillée",
                "Page HTML avec saisons et épisodes repliables",
                TypeExport::Series,
                "series",
                "html",
                gabarit_html_series(),
                None,
            ),
            integre(
                "series-csv",
                "Liste CSV",
                "Tableau CSV (séparateur « ; », UTF-8 avec BOM) pour tableur",
                TypeExport::Series,
                "series",
                "csv",
                GABARIT_CSV_SERIES.to_string(),
                None,
            ),
            integre(
                "series-texte",
                "Liste texte",
                "Séries, saisons et épisodes en texte indenté",
                TypeExport::Series,
                "series",
                "txt",
                GABARIT_TEXTE_SERIES.to_string(),
                None,
            ),
        ],
    }
}

// ---------------------------------------------------------------------------
// Modèles de l'utilisateur
// ---------------------------------------------------------------------------

/// Lit un fichier `clé=valeur` (`#` et `!` : commentaires).
pub fn lire_proprietes(texte: &str) -> BTreeMap<String, String> {
    texte
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('!'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
        .collect()
}

/// Nom de fichier relatif sûr : ni absolu, ni `..`.
fn chemin_relatif_sur(nom: &str) -> Option<PathBuf> {
    let p = Path::new(nom);
    if nom.is_empty() || p.is_absolute() {
        return None;
    }
    p.components()
        .all(|c| matches!(c, Component::Normal(_)))
        .then(|| p.to_path_buf())
}

fn lire_gabarit(dossier: &Path, nom: &str) -> Result<(String, PathBuf), String> {
    let rel =
        chemin_relatif_sur(nom).ok_or_else(|| format!("nom de fichier invalide « {nom} »"))?;
    let chemin = dossier.join(&rel);
    let meta = std::fs::symlink_metadata(&chemin).map_err(|e| format!("{nom} : {e}"))?;
    if !meta.is_file() {
        return Err(format!("{nom} n'est pas un fichier"));
    }
    if meta.len() > TAILLE_MAX_GABARIT {
        return Err(format!("{nom} est trop volumineux"));
    }
    let texte = std::fs::read_to_string(&chemin).map_err(|e| format!("{nom} : {e}"))?;
    Ok((texte, chemin))
}

fn charger_un_modele(dossier: &Path) -> Result<Option<ModeleExport>, String> {
    let conf = dossier.join(FICHIER_CONFIG_MODELE);
    if !conf.is_file() {
        return Ok(None);
    }
    let props = lire_proprietes(&std::fs::read_to_string(&conf).map_err(|e| e.to_string())?);
    let type_export = match props.get("type").map(|t| t.to_uppercase()).as_deref() {
        Some("MOVIE") => TypeExport::Films,
        Some("TV_SHOW") => TypeExport::Series,
        // Autres types (sagas…) non pris en charge : modèle ignoré.
        _ => return Ok(None),
    };
    let nom_dossier = dossier
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let liste = props.get("list").ok_or("clé `list` absente")?;
    let (gabarit_liste, chemin_liste) = lire_gabarit(dossier, liste)?;
    let mut exclus = vec![conf, chemin_liste];
    let gabarit_detail = match props.get("detail").filter(|d| !d.is_empty()) {
        Some(d) => {
            let (t, c) = lire_gabarit(dossier, d)?;
            exclus.push(c);
            Some(t)
        }
        None => None,
    };
    let extension = props
        .get("extension")
        .map(|e| e.trim_start_matches('.').to_lowercase())
        .filter(|e| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or_else(|| "html".to_string());
    let nom_fichier = props
        .get("filename")
        .and_then(|f| chemin_relatif_sur(f))
        .filter(|f| f.components().count() == 1)
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| match type_export {
            TypeExport::Films => "films".into(),
            TypeExport::Series => "series".into(),
        });
    Ok(Some(ModeleExport {
        id: format!("utilisateur:{nom_dossier}"),
        nom: props.get("name").cloned().unwrap_or(nom_dossier),
        description: props.get("description").cloned().unwrap_or_default(),
        type_export,
        nom_fichier,
        extension,
        gabarit_liste,
        gabarit_detail,
        dossier: Some(dossier.to_path_buf()),
        fichiers_exclus: exclus,
        integre: false,
    }))
}

/// Modèles de l'utilisateur d'un dossier `modeles_export/` pour un type
/// d'export. Les modèles invalides sont ignorés (journalisés).
pub fn modeles_utilisateur(racine: &Path, type_export: TypeExport) -> Vec<ModeleExport> {
    let Ok(entrees) = std::fs::read_dir(racine) else {
        return Vec::new();
    };
    let mut dossiers: Vec<PathBuf> = entrees
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dossiers.sort();
    let mut v = Vec::new();
    for d in dossiers {
        match charger_un_modele(&d) {
            Ok(Some(m)) if m.type_export == type_export => v.push(m),
            Ok(_) => {}
            Err(e) => tracing::warn!("modèle d'export {d:?} ignoré : {e}"),
        }
    }
    v
}

/// Modèles disponibles (intégrés, puis ceux de l'utilisateur).
pub fn modeles_disponibles(
    racine_utilisateur: &Path,
    type_export: TypeExport,
) -> Vec<ModeleExport> {
    let mut v = modeles_integres(type_export);
    v.extend(modeles_utilisateur(racine_utilisateur, type_export));
    v
}

// ---------------------------------------------------------------------------
// Contextes
// ---------------------------------------------------------------------------

/// Nom de fichier sûr pour un identifiant interne.
fn nom_sur(id: &str) -> String {
    let s: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() { "sans-id".into() } else { s }
}

fn dictionnaire(v: Option<&Valeur>) -> BTreeMap<String, Valeur> {
    match v {
        Some(Valeur::Dictionnaire(d)) => d.clone(),
        _ => BTreeMap::new(),
    }
}

fn texte_personnes(l: &[&mediamanager_core::modele::Personne]) -> String {
    l.iter()
        .map(|p| p.nom.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn nom_fichier_principal(film: &Film) -> String {
    film.fichier_principal()
        .map(|f| f.nom.clone())
        .unwrap_or_default()
}

fn titre_de_tri(titre_tri: &str, titre: &str) -> String {
    if titre_tri.is_empty() {
        titre
    } else {
        titre_tri
    }
    .to_lowercase()
}

/// Éléments communs aux contextes d'un film.
pub(crate) struct ExtrasFilm {
    pub poster: String,
    pub detail: String,
}

/// Dictionnaire `movie` d'un film (non échappé).
pub(crate) fn dictionnaire_film(
    film: &Film,
    saga: Option<&Saga>,
    prefixes: &[String],
    extras: &ExtrasFilm,
) -> BTreeMap<String, Valeur> {
    let c = contexte_film(film, saga, false, prefixes);
    let mut m = dictionnaire(c.obtenir("movie"));
    m.insert("poster".into(), extras.poster.as_str().into());
    m.insert("detailFile".into(), extras.detail.as_str().into());
    m.insert(
        "dateAdded".into(),
        Valeur::Date(DateHeure::depuis_epoch_ms(film.date_ajout)),
    );
    m.insert(
        "path".into(),
        film.chemin.to_string_lossy().into_owned().into(),
    );
    m.insert("fileName".into(), nom_fichier_principal(film).into());
    m.insert(
        "directorsAsString".into(),
        texte_personnes(&film.realisateurs()).into(),
    );
    m.insert(
        "writersAsString".into(),
        texte_personnes(&film.scenaristes()).into(),
    );
    let acteurs: Vec<_> = film.acteurs.iter().collect();
    m.insert("actorsAsString".into(), texte_personnes(&acteurs).into());
    m.insert(
        "movieSetTitle".into(),
        saga.map(|s| s.titre.clone()).unwrap_or_default().into(),
    );
    m
}

fn dictionnaire_episode(
    serie: &Serie,
    ep: &mediamanager_core::modele::Episode,
    prefixes: &[String],
) -> BTreeMap<String, Valeur> {
    let c = contexte_episode(serie, ep, prefixes);
    let mut e = dictionnaire(c.obtenir("episode"));
    e.insert("watched".into(), ep.vu.into());
    e.insert(
        "fileName".into(),
        ep.fichiers
            .iter()
            .find(|f| f.type_fichier == TypeFichier::Video)
            .map(|f| f.nom.clone())
            .unwrap_or_default()
            .into(),
    );
    e
}

/// Dictionnaire `tvShow` d'une série (non échappé).
pub(crate) fn dictionnaire_serie(
    serie: &Serie,
    prefixes: &[String],
    extras: &ExtrasFilm,
) -> BTreeMap<String, Valeur> {
    let c = contexte_serie(serie, prefixes);
    let mut s = dictionnaire(c.obtenir("tvShow"));
    let groupe = &serie.groupe_actif;
    let mut eps: Vec<&mediamanager_core::modele::Episode> =
        serie.episodes.iter().filter(|e| !e.est_fictif()).collect();
    eps.sort_by_key(|e| (e.saison(groupe), e.episode(groupe)));
    let mut saisons: BTreeMap<i32, Vec<Valeur>> = BTreeMap::new();
    let mut tous = Vec::new();
    for e in &eps {
        let d = Valeur::Dictionnaire(dictionnaire_episode(serie, e, prefixes));
        saisons.entry(e.saison(groupe)).or_default().push(d.clone());
        tous.push(d);
    }
    let liste_saisons: Vec<Valeur> = saisons
        .into_iter()
        .map(|(numero, episodes)| {
            let titre = serie
                .saisons
                .iter()
                .find(|x| x.numero == numero)
                .map(|x| x.titre.clone())
                .unwrap_or_default();
            Contexte::nouveau()
                .avec("number", numero)
                .avec("title", titre)
                .avec("episodes", Valeur::Liste(episodes))
                .into()
        })
        .collect();
    s.insert("seasonCount".into(), liste_saisons.len().into());
    s.insert("seasons".into(), Valeur::Liste(liste_saisons));
    s.insert("episodeCount".into(), eps.len().into());
    s.insert(
        "watchedCount".into(),
        eps.iter().filter(|e| e.vu).count().into(),
    );
    s.insert("episodes".into(), Valeur::Liste(tous));
    s.insert("plot".into(), serie.resume.as_str().into());
    s.insert("genresAsString".into(), serie.genres.join(", ").into());
    s.insert("poster".into(), extras.poster.as_str().into());
    s.insert("detailFile".into(), extras.detail.as_str().into());
    s.insert(
        "path".into(),
        serie.chemin.to_string_lossy().into_owned().into(),
    );
    s
}

fn contexte_depuis(d: BTreeMap<String, Valeur>) -> Contexte {
    d.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Rendu et écriture
// ---------------------------------------------------------------------------

fn rendre(
    modele: &ModeleExport,
    source: &str,
    racine: &Contexte,
    mode: Echappement,
) -> Resultat<String> {
    let echappe = contexte_depuis(match echapper_valeur(&racine.en_valeur(), mode) {
        Valeur::Dictionnaire(d) => d,
        _ => BTreeMap::new(),
    });
    Gabarit::analyser(source)
        .and_then(|g| g.rendre(&echappe, &Jetons::default(), &OptionsRendu::default()))
        .map_err(|e| ErreurApp::Interne(format!("modèle d'export « {} » : {e}", modele.nom)))
}

fn copier_vignette(origine: &Path, destination_sans_ext: &Path, largeur: u32) -> Option<PathBuf> {
    if largeur > 0
        && let Ok(img) = image::open(origine)
    {
        let img = if img.width() > largeur {
            img.thumbnail(largeur, largeur.saturating_mul(4))
        } else {
            img
        };
        let dest = destination_sans_ext.with_extension("jpg");
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p).ok()?;
        }
        if img
            .to_rgb8()
            .save_with_format(&dest, image::ImageFormat::Jpeg)
            .is_ok()
        {
            return Some(dest);
        }
        let _ = std::fs::remove_file(&dest);
    }
    // Copie telle quelle (image illisible ou pas de redimensionnement).
    let ext = origine
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .filter(|e| e.chars().all(|c| c.is_ascii_alphanumeric()) && !e.is_empty())
        .unwrap_or_else(|| "jpg".into());
    let dest = destination_sans_ext.with_extension(ext);
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p).ok()?;
    }
    std::fs::copy(origine, &dest).ok().map(|_| dest)
}

fn copier_annexes(
    modele: &ModeleExport,
    sortie: &Path,
    rapport: &mut RapportExport,
) -> Resultat<()> {
    let Some(dossier) = &modele.dossier else {
        return Ok(());
    };
    let sortie_canon = sortie
        .canonicalize()
        .unwrap_or_else(|_| sortie.to_path_buf());
    if sortie_canon.starts_with(dossier.canonicalize().unwrap_or_else(|_| dossier.clone())) {
        return Err(ErreurApp::io(
            sortie,
            "le dossier de sortie est dans le dossier du modèle",
        ));
    }
    for e in walkdir::WalkDir::new(dossier)
        .follow_links(false)
        .sort_by_file_name()
    {
        let Ok(e) = e else { continue };
        if !e.file_type().is_file()
            || modele.fichiers_exclus.iter().any(|x| x == e.path())
            || e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("jmte"))
        {
            continue;
        }
        let Ok(rel) = e.path().strip_prefix(dossier) else {
            continue;
        };
        let dest = sortie.join(rel);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p).map_err(|x| ErreurApp::io(p, x))?;
        }
        std::fs::copy(e.path(), &dest).map_err(|x| ErreurApp::io(&dest, x))?;
        rapport.annexes.push(dest);
    }
    Ok(())
}

/// Dossier de sortie : créé, jamais un fichier existant.
fn preparer_sortie(sortie: &Path) -> Resultat<()> {
    if sortie.exists() && !sortie.is_dir() {
        return Err(ErreurApp::io(sortie, "le dossier de sortie est un fichier"));
    }
    std::fs::create_dir_all(sortie).map_err(|e| ErreurApp::io(sortie, e))
}

struct Element {
    id: String,
    dict: BTreeMap<String, Valeur>,
}

/// Noms des variables et du sous-dossier de détail d'un type d'export.
struct Noms {
    /// Variable d'un élément (`movie`, `tvShow`).
    element: &'static str,
    /// Variable de la liste (`movies`, `tvShows`).
    liste: &'static str,
    sous_dossier_detail: &'static str,
}

/// Rend la liste et les pages de détail, copie les annexes.
fn ecrire_export(
    modele: &ModeleExport,
    elements: Vec<Element>,
    noms: &Noms,
    sortie: &Path,
    titre: &str,
    rapport: &mut RapportExport,
) -> Resultat<()> {
    let mode = modele.echappement();
    let generated = Valeur::Date(DateHeure::depuis_epoch_ms(
        mediamanager_core::modele::film::maintenant_ms(),
    ));
    let liste: Vec<Valeur> = elements
        .iter()
        .map(|e| Valeur::Dictionnaire(e.dict.clone()))
        .collect();
    let racine = Contexte::nouveau()
        .avec("title", titre)
        .avec("count", elements.len())
        .avec("generated", generated.clone())
        .avec(noms.liste, Valeur::Liste(liste));
    let contenu = rendre(modele, &modele.gabarit_liste, &racine, mode)?;
    let fichier = sortie.join(format!("{}.{}", modele.nom_fichier, modele.extension));
    ecrire_atomiquement(&fichier, contenu.as_bytes())?;
    rapport.fichier_liste = fichier;

    if let Some(detail) = &modele.gabarit_detail {
        for e in &elements {
            let racine = Contexte::nouveau()
                .avec("title", titre)
                .avec("generated", generated.clone())
                .avec(noms.element, Valeur::Dictionnaire(e.dict.clone()));
            let contenu = rendre(modele, detail, &racine, mode)?;
            let f = sortie.join(noms.sous_dossier_detail).join(format!(
                "{}.{}",
                nom_sur(&e.id),
                modele.extension
            ));
            ecrire_atomiquement(&f, contenu.as_bytes())?;
            rapport.pages_detail.push(f);
        }
    }
    copier_annexes(modele, sortie, rapport)?;
    rapport.exportes = elements.len();
    Ok(())
}

fn titre_liste(options: &OptionsExport, defaut: &str) -> String {
    options
        .titre
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| defaut.to_string())
}

/// Tâche : exporte des films (`None` = tous).
pub(crate) fn exporter_films(
    n: &Noyau,
    ctx: &ContexteTache,
    ids: Option<&[String]>,
    modele: &ModeleExport,
    sortie: &Path,
    options: &OptionsExport,
) -> Resultat<RapportExport> {
    if modele.type_export != TypeExport::Films {
        return Err(ErreurApp::io(
            sortie,
            format!("le modèle « {} » n'exporte pas des films", modele.nom),
        ));
    }
    let reglages = n.reglages();
    let prefixes = reglages.general.prefixes_titre.clone();
    let (mut films, sagas, mut rapport) = {
        let s = n.verrou();
        let tous = s.bibliotheque.lister::<Film>()?;
        let sagas = s.bibliotheque.lister::<Saga>()?;
        let mut rapport = RapportExport::default();
        let films: Vec<Film> = match ids {
            None => tous,
            Some(ids) => {
                let mut v = Vec::new();
                for id in ids {
                    match tous.iter().find(|f| &f.id == id) {
                        Some(f) if !v.iter().any(|x: &Film| x.id == f.id) => v.push(f.clone()),
                        Some(_) => {}
                        None => rapport.introuvables.push(id.clone()),
                    }
                }
                v
            }
        };
        (films, sagas, rapport)
    };
    films.sort_by_key(|f| titre_de_tri(&f.titre_tri, &f.titre));
    preparer_sortie(sortie)?;
    let total = films.len() as u64;
    let mut elements = Vec::new();
    for (i, film) in films.iter().enumerate() {
        ctx.verifier_annulation()?;
        ctx.progression(i as u64, total, film.titre.clone());
        let poster = match options.vignettes {
            Some(l) => film
                .fichiers
                .iter()
                .find(|f| f.type_fichier == TypeFichier::Poster)
                .map(|f| f.chemin_complet())
                .filter(|p| p.is_file())
                .and_then(|p| {
                    copier_vignette(&p, &sortie.join("images").join(nom_sur(&film.id)), l)
                }),
            None => None,
        };
        let poster_rel = poster
            .as_ref()
            .and_then(|p| p.strip_prefix(sortie).ok())
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if let Some(p) = poster {
            rapport.vignettes.push(p);
        }
        let detail = if modele.gabarit_detail.is_some() {
            format!("films/{}.{}", nom_sur(&film.id), modele.extension)
        } else {
            String::new()
        };
        let saga = film
            .saga_id
            .as_ref()
            .and_then(|id| sagas.iter().find(|s| &s.id == id));
        elements.push(Element {
            id: film.id.clone(),
            dict: dictionnaire_film(
                film,
                saga,
                &prefixes,
                &ExtrasFilm {
                    poster: poster_rel,
                    detail,
                },
            ),
        });
    }
    ecrire_export(
        modele,
        elements,
        &Noms {
            element: "movie",
            liste: "movies",
            sous_dossier_detail: "films",
        },
        sortie,
        &titre_liste(options, "Films"),
        &mut rapport,
    )?;
    if !rapport.introuvables.is_empty() {
        ctx.message(
            NiveauMessage::Avertissement,
            format!(
                "{} film(s) introuvable(s) ignoré(s)",
                rapport.introuvables.len()
            ),
        );
    }
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}

/// Tâche : exporte des séries (`None` = toutes).
pub(crate) fn exporter_series(
    n: &Noyau,
    ctx: &ContexteTache,
    ids: Option<&[String]>,
    modele: &ModeleExport,
    sortie: &Path,
    options: &OptionsExport,
) -> Resultat<RapportExport> {
    if modele.type_export != TypeExport::Series {
        return Err(ErreurApp::io(
            sortie,
            format!("le modèle « {} » n'exporte pas des séries", modele.nom),
        ));
    }
    let prefixes = n.reglages().general.prefixes_titre.clone();
    let mut rapport = RapportExport::default();
    let mut series: Vec<Serie> = {
        let s = n.verrou();
        let toutes = s.bibliotheque.lister::<Serie>()?;
        match ids {
            None => toutes,
            Some(ids) => {
                let mut v: Vec<Serie> = Vec::new();
                for id in ids {
                    match toutes.iter().find(|x| &x.id == id) {
                        Some(x) if !v.iter().any(|y| y.id == x.id) => v.push(x.clone()),
                        Some(_) => {}
                        None => rapport.introuvables.push(id.clone()),
                    }
                }
                v
            }
        }
    };
    series.sort_by_key(|s| titre_de_tri(&s.titre_tri, &s.titre));
    preparer_sortie(sortie)?;
    let total = series.len() as u64;
    let mut elements = Vec::new();
    for (i, serie) in series.iter().enumerate() {
        ctx.verifier_annulation()?;
        ctx.progression(i as u64, total, serie.titre.clone());
        let poster = match options.vignettes {
            Some(l) => serie
                .fichiers
                .iter()
                .find(|f| f.type_fichier == TypeFichier::Poster)
                .map(|f| f.chemin_complet())
                .filter(|p| p.is_file())
                .and_then(|p| {
                    copier_vignette(&p, &sortie.join("images").join(nom_sur(&serie.id)), l)
                }),
            None => None,
        };
        let poster_rel = poster
            .as_ref()
            .and_then(|p| p.strip_prefix(sortie).ok())
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if let Some(p) = poster {
            rapport.vignettes.push(p);
        }
        let detail = if modele.gabarit_detail.is_some() {
            format!("series/{}.{}", nom_sur(&serie.id), modele.extension)
        } else {
            String::new()
        };
        elements.push(Element {
            id: serie.id.clone(),
            dict: dictionnaire_serie(
                serie,
                &prefixes,
                &ExtrasFilm {
                    poster: poster_rel,
                    detail,
                },
            ),
        });
    }
    ecrire_export(
        modele,
        elements,
        &Noms {
            element: "tvShow",
            liste: "tvShows",
            sous_dossier_detail: "series",
        },
        sortie,
        &titre_liste(options, "Séries"),
        &mut rapport,
    )?;
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proprietes() {
        let p = lire_proprietes("# c\nName = Mon modèle\ntype=MOVIE\n\nlist=l.jmte\n! x\n");
        assert_eq!(p["name"], "Mon modèle");
        assert_eq!(p["type"], "MOVIE");
        assert_eq!(p["list"], "l.jmte");
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn chemins_relatifs() {
        assert!(chemin_relatif_sur("a/b.txt").is_some());
        assert!(chemin_relatif_sur("../a").is_none());
        assert!(chemin_relatif_sur("/etc/passwd").is_none());
        assert!(chemin_relatif_sur("").is_none());
    }

    #[test]
    fn noms_surs() {
        assert_eq!(nom_sur("a/b c"), "a_b_c");
        assert_eq!(nom_sur(""), "sans-id");
    }

    #[test]
    fn les_modeles_integres_se_compilent() {
        for t in [TypeExport::Films, TypeExport::Series] {
            for m in modeles_integres(t) {
                Gabarit::analyser(&m.gabarit_liste).unwrap_or_else(|e| panic!("{} : {e}", m.id));
                if let Some(d) = &m.gabarit_detail {
                    Gabarit::analyser(d).unwrap();
                }
            }
        }
    }
}
