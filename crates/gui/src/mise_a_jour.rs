//! Mise à jour du programme (aucune dépendance egui : entièrement testable).
//!
//! 1. [`rechercher`] interroge l'API des publications GitHub (dernière publication) ;
//! 2. [`comparer`] confronte le build courant (`MMR_BUILD`, SHA du commit fixé par la CI, et la
//!    version Cargo) à la publication (ligne `build: <sha>` de ses notes, étiquette de version) ;
//! 3. [`installer_publication`] télécharge l'archive Windows, l'extrait dans un dossier
//!    temporaire à côté de l'exécutable, renomme les fichiers remplacés en `.old` (Windows
//!    autorise le renommage d'un exécutable en cours d'exécution) et met les nouveaux en place ;
//! 4. l'interface relance le programme ([`relancer`]) et se ferme ; au démarrage suivant,
//!    [`nettoyer_anciens`] supprime les `.old`.

use serde::Deserialize;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

/// Adresse de la dernière publication du dépôt.
pub const URL_DERNIERE_PUBLICATION: &str =
    "https://api.github.com/repos/imtheyoyo/MediaManagerRust/releases/latest";
/// Archive de la publication contenant les exécutables Windows.
pub const NOM_ARCHIVE: &str = "MediaManagerRust-windows-x64.zip";
/// Build courant : SHA du commit, fixé par la CI (absent en compilation locale).
pub const BUILD: Option<&str> = option_env!("MMR_BUILD");
/// Version Cargo du programme.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Extension des fichiers remplacés, supprimés au démarrage suivant.
pub const EXTENSION_ANCIEN: &str = "old";
/// Argument ajouté à la relance : le nouveau programme attend la fin de l'ancien.
pub const ARGUMENT_RELANCE: &str = "--apres-mise-a-jour";
const PREFIXE_TEMPORAIRE: &str = ".mise-a-jour-";
const DELAI_RESEAU: Duration = Duration::from_secs(20);
const DELAI_TELECHARGEMENT: Duration = Duration::from_secs(600);

/// Identité du programme en cours (injectée pour les tests).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Courant {
    pub build: Option<String>,
    pub version: String,
}

impl Courant {
    /// Identité de l'exécutable compilé.
    pub fn compile() -> Self {
        Self {
            build: BUILD
                .map(str::trim)
                .filter(|b| !b.is_empty())
                .map(String::from),
            version: VERSION.to_string(),
        }
    }

    /// Texte court affiché (« 0.1.0 (abc1234) » ou « 0.1.0 (développement) »).
    pub fn libelle(&self) -> String {
        match &self.build {
            Some(b) => format!("{} ({})", self.version, court(b)),
            None => format!("{} (développement)", self.version),
        }
    }
}

/// Configuration d'une recherche / installation (adresse remplaçable dans les tests).
#[derive(Clone, Debug)]
pub struct ConfigMiseAJour {
    pub url_api: String,
    pub courant: Courant,
    /// Dossier de l'exécutable à remplacer (`None` : celui de l'exécutable courant).
    pub dossier_programme: Option<PathBuf>,
}

impl Default for ConfigMiseAJour {
    fn default() -> Self {
        Self {
            url_api: URL_DERNIERE_PUBLICATION.to_string(),
            courant: Courant::compile(),
            dossier_programme: None,
        }
    }
}

/// Publication analysée.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Publication {
    pub etiquette: String,
    pub nom: String,
    /// Notes de version (corps de la publication).
    pub notes: String,
    /// SHA indiqué par la ligne `build: <sha>` des notes.
    pub build: Option<String>,
    /// Adresse de téléchargement de [`NOM_ARCHIVE`].
    pub url_archive: Option<String>,
    pub taille_archive: Option<u64>,
    pub page: Option<String>,
}

impl Publication {
    /// Version déduite de l'étiquette (« v0.2.0 » → 0.2.0).
    pub fn version(&self) -> Option<semver::Version> {
        version_depuis(&self.etiquette).or_else(|| version_depuis(&self.nom))
    }

    /// Libellé affiché : étiquette, puis build abrégé.
    pub fn libelle(&self) -> String {
        let base = if self.etiquette.is_empty() {
            self.nom.clone()
        } else {
            self.etiquette.clone()
        };
        match &self.build {
            Some(b) => format!("{base} ({})", court(b)),
            None => base,
        }
    }
}

/// Résultat de la comparaison avec le programme courant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    AJour,
    Disponible,
    /// Compilation locale (pas de `MMR_BUILD`) : aucune installation proposée.
    Developpement,
}

#[derive(Debug)]
pub enum ErreurMiseAJour {
    /// Échec de connexion, délai dépassé…
    Reseau(String),
    /// Réponse HTTP inattendue (code, message éventuel).
    Http(u16, String),
    AucunePublication,
    LimiteAtteinte,
    ReponseInvalide(String),
    ArchiveAbsente,
    Archive(String),
    Fichiers(String),
}

impl fmt::Display for ErreurMiseAJour {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reseau(e) => write!(f, "serveur injoignable ({e})"),
            Self::Http(code, m) if m.is_empty() => write!(f, "réponse HTTP {code} du serveur"),
            Self::Http(code, m) => write!(f, "réponse HTTP {code} du serveur : {m}"),
            Self::AucunePublication => write!(f, "aucune publication disponible"),
            Self::LimiteAtteinte => write!(
                f,
                "limite de requêtes du serveur atteinte, réessayez dans une heure"
            ),
            Self::ReponseInvalide(e) => write!(f, "réponse du serveur illisible ({e})"),
            Self::ArchiveAbsente => {
                write!(f, "la publication ne contient pas l'archive {NOM_ARCHIVE}")
            }
            Self::Archive(e) => write!(f, "archive invalide ({e})"),
            Self::Fichiers(e) => write!(f, "remplacement des fichiers impossible ({e})"),
        }
    }
}

impl std::error::Error for ErreurMiseAJour {}

fn erreur_fichiers(contexte: &str, chemin: &Path, e: io::Error) -> ErreurMiseAJour {
    ErreurMiseAJour::Fichiers(format!("{contexte} {} : {e}", chemin.display()))
}

// ---------------------------------------------------------------- analyse et comparaison

#[derive(Deserialize)]
struct PublicationJson {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    assets: Vec<FichierJson>,
}

#[derive(Deserialize)]
struct FichierJson {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: Option<u64>,
}

/// Analyse la réponse JSON de l'API des publications.
pub fn analyser_publication(json: &str) -> Result<Publication, ErreurMiseAJour> {
    let p: PublicationJson =
        serde_json::from_str(json).map_err(|e| ErreurMiseAJour::ReponseInvalide(e.to_string()))?;
    let notes = p.body.unwrap_or_default();
    let archive = p
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(NOM_ARCHIVE));
    Ok(Publication {
        build: build_depuis_notes(&notes),
        etiquette: p.tag_name,
        nom: p.name.unwrap_or_default(),
        notes,
        url_archive: archive.map(|a| a.browser_download_url.clone()),
        taille_archive: archive.and_then(|a| a.size),
        page: p.html_url,
    })
}

/// Cherche la ligne `build: <sha>` (casse, puces, gras et accents graves tolérés).
pub fn build_depuis_notes(notes: &str) -> Option<String> {
    notes.lines().find_map(|ligne| {
        let l = ligne
            .trim()
            .trim_start_matches(['-', '*', '>', ' '])
            .replace(['`', '*'], "");
        let l = l.trim();
        let (cle, valeur) = l.split_once(':')?;
        if !cle.trim().eq_ignore_ascii_case("build") {
            return None;
        }
        let sha = valeur.split_whitespace().next()?.to_ascii_lowercase();
        (sha.len() >= 7 && sha.chars().all(|c| c.is_ascii_hexdigit())).then_some(sha)
    })
}

fn version_depuis(texte: &str) -> Option<semver::Version> {
    let t = texte.trim();
    let t = t
        .strip_prefix('v')
        .or_else(|| t.strip_prefix('V'))
        .unwrap_or(t);
    semver::Version::parse(t).ok()
}

/// Deux SHA désignent le même commit (forme abrégée acceptée, 7 caractères au moins).
pub fn meme_build(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim().to_ascii_lowercase(), b.trim().to_ascii_lowercase());
    let n = a.len().min(b.len());
    n >= 7 && a[..n] == b[..n]
}

/// SHA abrégé (7 caractères).
pub fn court(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

/// Compare le programme courant à la publication.
pub fn comparer(courant: &Courant, publication: &Publication) -> Verdict {
    let Some(build) = &courant.build else {
        return Verdict::Developpement;
    };
    if let Some(b) = &publication.build {
        return if meme_build(build, b) {
            Verdict::AJour
        } else {
            Verdict::Disponible
        };
    }
    // Pas de build dans les notes : la version décide.
    match (publication.version(), version_depuis(&courant.version)) {
        (Some(p), Some(c)) if p > c => Verdict::Disponible,
        _ => Verdict::AJour,
    }
}

// ---------------------------------------------------------------- réseau

fn agent_utilisateur(courant: &Courant) -> String {
    format!(
        "MediaManagerRust/{} (verification-mise-a-jour)",
        courant.version
    )
}

fn client(
    courant: &Courant,
    delai: Duration,
) -> Result<reqwest::blocking::Client, ErreurMiseAJour> {
    reqwest::blocking::Client::builder()
        .user_agent(agent_utilisateur(courant))
        .connect_timeout(DELAI_RESEAU)
        .timeout(delai)
        .build()
        .map_err(|e| ErreurMiseAJour::Reseau(e.to_string()))
}

fn erreur_reseau(e: reqwest::Error) -> ErreurMiseAJour {
    let mut texte = e.to_string();
    let mut source = std::error::Error::source(&e);
    while let Some(s) = source {
        texte = format!("{texte} : {s}");
        source = s.source();
    }
    ErreurMiseAJour::Reseau(texte)
}

/// Interroge l'API et analyse la dernière publication (appel bloquant : thread dédié).
pub fn rechercher(config: &ConfigMiseAJour) -> Result<Publication, ErreurMiseAJour> {
    let reponse = client(&config.courant, DELAI_RESEAU)?
        .get(&config.url_api)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .map_err(erreur_reseau)?;
    let statut = reponse.status();
    let limite_epuisee = reponse
        .headers()
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        == Some("0");
    let texte = reponse.text().map_err(erreur_reseau)?;
    match statut.as_u16() {
        200..=299 => analyser_publication(&texte),
        404 => Err(ErreurMiseAJour::AucunePublication),
        403 | 429 if limite_epuisee || statut.as_u16() == 429 => {
            Err(ErreurMiseAJour::LimiteAtteinte)
        }
        code => {
            let message = serde_json::from_str::<serde_json::Value>(&texte)
                .ok()
                .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(String::from))
                .unwrap_or_default();
            Err(ErreurMiseAJour::Http(code, message))
        }
    }
}

/// Recherche + comparaison.
pub fn verifier(config: &ConfigMiseAJour) -> Result<(Verdict, Publication), ErreurMiseAJour> {
    let p = rechercher(config)?;
    Ok((comparer(&config.courant, &p), p))
}

/// Télécharge `url` dans `destination` ; `progression(fait, total)` est appelé par blocs.
pub fn telecharger(
    config: &ConfigMiseAJour,
    url: &str,
    destination: &Path,
    progression: &mut dyn FnMut(u64, Option<u64>),
) -> Result<(), ErreurMiseAJour> {
    let mut reponse = client(&config.courant, DELAI_TELECHARGEMENT)?
        .get(url)
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .send()
        .map_err(erreur_reseau)?;
    if !reponse.status().is_success() {
        return Err(ErreurMiseAJour::Http(
            reponse.status().as_u16(),
            String::new(),
        ));
    }
    let total = reponse.content_length();
    let mut fichier = fs::File::create(destination)
        .map_err(|e| erreur_fichiers("création de", destination, e))?;
    let mut tampon = vec![0u8; 64 * 1024];
    let mut fait = 0u64;
    progression(0, total);
    loop {
        let n = reponse
            .read(&mut tampon)
            .map_err(|e| ErreurMiseAJour::Reseau(e.to_string()))?;
        if n == 0 {
            break;
        }
        fichier
            .write_all(&tampon[..n])
            .map_err(|e| erreur_fichiers("écriture de", destination, e))?;
        fait += n as u64;
        progression(fait, total);
    }
    fichier
        .sync_all()
        .map_err(|e| erreur_fichiers("écriture de", destination, e))?;
    if let Some(t) = total
        && t != fait
    {
        return Err(ErreurMiseAJour::Reseau(format!(
            "téléchargement incomplet ({fait} octets sur {t})"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------- archive et fichiers

/// Extrait l'archive dans `dossier` (chemins hors du dossier refusés). Si toutes les entrées
/// partagent un même dossier racine, celui-ci est retiré. Renvoie les fichiers extraits
/// (chemins relatifs).
pub fn extraire_archive(archive: &Path, dossier: &Path) -> Result<Vec<PathBuf>, ErreurMiseAJour> {
    let erreur = |e: zip::result::ZipError| ErreurMiseAJour::Archive(e.to_string());
    let f = fs::File::open(archive).map_err(|e| erreur_fichiers("ouverture de", archive, e))?;
    let mut zip = zip::ZipArchive::new(f).map_err(erreur)?;
    let mut entrees: Vec<(usize, PathBuf)> = Vec::new();
    for n in 0..zip.len() {
        let e = zip.by_index(n).map_err(erreur)?;
        if e.is_dir() {
            continue;
        }
        let chemin = e
            .enclosed_name()
            .ok_or_else(|| ErreurMiseAJour::Archive(format!("chemin refusé : {}", e.name())))?;
        entrees.push((n, chemin));
    }
    if entrees.is_empty() {
        return Err(ErreurMiseAJour::Archive("archive vide".into()));
    }
    let racine_commune = {
        let premiere = |p: &Path| match p.components().next() {
            Some(Component::Normal(c)) if p.components().count() > 1 => Some(c.to_owned()),
            _ => None,
        };
        let r = premiere(&entrees[0].1);
        r.filter(|r| entrees.iter().all(|(_, p)| premiere(p).as_ref() == Some(r)))
    };
    let mut extraits = Vec::new();
    for (n, chemin) in entrees {
        let relatif = match &racine_commune {
            Some(r) => chemin
                .strip_prefix(r)
                .map(Path::to_path_buf)
                .unwrap_or(chemin),
            None => chemin,
        };
        let cible = dossier.join(&relatif);
        if let Some(parent) = cible.parent() {
            fs::create_dir_all(parent).map_err(|e| erreur_fichiers("création de", parent, e))?;
        }
        let mut e = zip.by_index(n).map_err(erreur)?;
        let mut sortie =
            fs::File::create(&cible).map_err(|e| erreur_fichiers("création de", &cible, e))?;
        io::copy(&mut e, &mut sortie).map_err(|e| erreur_fichiers("extraction de", &cible, e))?;
        extraits.push(relatif);
    }
    Ok(extraits)
}

/// Chemin `fichier.ext.old`.
pub fn chemin_ancien(fichier: &Path) -> PathBuf {
    let mut nom = fichier.as_os_str().to_owned();
    nom.push(".");
    nom.push(EXTENSION_ANCIEN);
    PathBuf::from(nom)
}

/// Déplace (renommage, sinon copie puis suppression : autre volume).
fn deplacer(source: &Path, cible: &Path) -> io::Result<()> {
    match fs::rename(source, cible) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::copy(source, cible)?;
            fs::remove_file(source)
        }
    }
}

/// Met en place les fichiers `relatifs` de `extrait` dans `programme` : chaque fichier
/// existant est d'abord renommé en `.old`. En cas d'échec, tout est remis en état.
pub fn echanger_fichiers(
    extrait: &Path,
    relatifs: &[PathBuf],
    programme: &Path,
) -> Result<(), ErreurMiseAJour> {
    // (cible, ancien renommé ?)
    let mut faits: Vec<(PathBuf, bool)> = Vec::new();
    let mut resultat = Ok(());
    for relatif in relatifs {
        let source = extrait.join(relatif);
        let cible = programme.join(relatif);
        let ancien = chemin_ancien(&cible);
        let etape = (|| -> Result<bool, ErreurMiseAJour> {
            if let Some(parent) = cible.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| erreur_fichiers("création de", parent, e))?;
            }
            let existe = cible.exists();
            if existe {
                if ancien.exists() {
                    fs::remove_file(&ancien)
                        .map_err(|e| erreur_fichiers("suppression de", &ancien, e))?;
                }
                fs::rename(&cible, &ancien)
                    .map_err(|e| erreur_fichiers("renommage de", &cible, e))?;
            }
            if let Err(e) = deplacer(&source, &cible) {
                if existe {
                    let _ = fs::rename(&ancien, &cible);
                }
                return Err(erreur_fichiers("mise en place de", &cible, e));
            }
            Ok(existe)
        })();
        match etape {
            Ok(existe) => faits.push((cible, existe)),
            Err(e) => {
                resultat = Err(e);
                break;
            }
        }
    }
    if resultat.is_err() {
        for (cible, existe) in faits.into_iter().rev() {
            let _ = fs::remove_file(&cible);
            if existe {
                let _ = fs::rename(chemin_ancien(&cible), &cible);
            }
        }
    }
    resultat
}

/// Supprime les fichiers `.old` laissés par une mise à jour et les dossiers temporaires
/// abandonnés. Renvoie le nombre de fichiers supprimés.
pub fn nettoyer_anciens(programme: &Path) -> usize {
    let mut n = 0;
    let Ok(entrees) = fs::read_dir(programme) else {
        return 0;
    };
    for e in entrees.flatten() {
        let chemin = e.path();
        let nom = e.file_name().to_string_lossy().to_string();
        if chemin.is_file()
            && chemin.extension().is_some_and(|x| x == EXTENSION_ANCIEN)
            && fs::remove_file(&chemin).is_ok()
        {
            n += 1;
        } else if chemin.is_dir() && nom.starts_with(PREFIXE_TEMPORAIRE) {
            let _ = fs::remove_dir_all(&chemin);
        }
    }
    n
}

/// Nettoyage au démarrage, dans le dossier de l'exécutable courant.
pub fn nettoyer_au_demarrage() {
    if let Some(d) = dossier_exe_courant() {
        nettoyer_anciens(&d);
    }
}

fn dossier_exe_courant() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
}

/// Exécutable à relancer après installation.
pub fn exe_courant() -> Option<PathBuf> {
    std::env::current_exe().ok()
}

/// Étapes de l'installation (progression transmise à l'interface).
#[derive(Clone, Debug, PartialEq)]
pub enum EtapeInstallation {
    Telechargement { fait: u64, total: Option<u64> },
    Extraction,
    Remplacement,
}

/// Télécharge l'archive de la publication, l'extrait à côté du programme et échange les
/// fichiers. Le dossier temporaire est supprimé dans tous les cas.
pub fn installer_publication(
    config: &ConfigMiseAJour,
    publication: &Publication,
    etape: &mut dyn FnMut(EtapeInstallation),
) -> Result<PathBuf, ErreurMiseAJour> {
    if config.courant.build.is_none() {
        return Err(ErreurMiseAJour::Fichiers(
            "version de développement : installation désactivée".into(),
        ));
    }
    let url = publication
        .url_archive
        .as_deref()
        .ok_or(ErreurMiseAJour::ArchiveAbsente)?;
    let programme = config
        .dossier_programme
        .clone()
        .or_else(dossier_exe_courant)
        .ok_or_else(|| ErreurMiseAJour::Fichiers("dossier du programme inconnu".into()))?;
    let temporaire = programme.join(format!("{PREFIXE_TEMPORAIRE}{}", std::process::id()));
    let _ = fs::remove_dir_all(&temporaire);
    fs::create_dir_all(&temporaire).map_err(|e| erreur_fichiers("création de", &temporaire, e))?;
    let resultat = (|| {
        let archive = temporaire.join(NOM_ARCHIVE);
        telecharger(config, url, &archive, &mut |fait, total| {
            etape(EtapeInstallation::Telechargement { fait, total })
        })?;
        etape(EtapeInstallation::Extraction);
        let extrait = temporaire.join("extrait");
        let fichiers = extraire_archive(&archive, &extrait)?;
        if !fichiers
            .iter()
            .any(|f| f.extension().is_some_and(|x| x.eq_ignore_ascii_case("exe")))
        {
            return Err(ErreurMiseAJour::Archive(
                "aucun exécutable dans l'archive".into(),
            ));
        }
        etape(EtapeInstallation::Remplacement);
        echanger_fichiers(&extrait, &fichiers, &programme)?;
        Ok(programme.clone())
    })();
    let _ = fs::remove_dir_all(&temporaire);
    resultat
}

/// Relance le programme (mêmes arguments + [`ARGUMENT_RELANCE`]) ; l'appelant ferme ensuite
/// l'interface.
pub fn relancer(exe: &Path) -> io::Result<()> {
    std::process::Command::new(exe)
        .args(
            std::env::args_os()
                .skip(1)
                .filter(|a| a != ARGUMENT_RELANCE),
        )
        .arg(ARGUMENT_RELANCE)
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    fn courant(build: Option<&str>, version: &str) -> Courant {
        Courant {
            build: build.map(String::from),
            version: version.into(),
        }
    }

    fn json_publication(url_archive: &str, build: &str) -> String {
        serde_json::json!({
            "tag_name": "v0.2.0",
            "name": "MediaManagerRust 0.2.0",
            "html_url": "https://exemple.invalid/publication",
            "body": format!("## Nouveautés\n- sélecteur de dossiers natif\n\nbuild: {build}\n"),
            "assets": [
                {"name": "MediaManagerRust-linux-x64.tar.gz", "browser_download_url": "https://exemple.invalid/a", "size": 3},
                {"name": NOM_ARCHIVE, "browser_download_url": url_archive, "size": 1234}
            ]
        })
        .to_string()
    }

    /// Archive de test : un dossier racine commun avec deux exécutables et une notice.
    fn archive_test(fichiers: &[(&str, &[u8])]) -> Vec<u8> {
        let mut tampon = io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut tampon);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            for (nom, contenu) in fichiers {
                z.start_file(*nom, options).unwrap();
                z.write_all(contenu).unwrap();
            }
            z.finish().unwrap();
        }
        tampon.into_inner()
    }

    #[test]
    fn analyse_de_la_reponse() {
        let p = analyser_publication(&json_publication("https://exemple.invalid/z", SHA)).unwrap();
        assert_eq!(p.etiquette, "v0.2.0");
        assert_eq!(p.build.as_deref(), Some(SHA));
        assert_eq!(p.url_archive.as_deref(), Some("https://exemple.invalid/z"));
        assert_eq!(p.taille_archive, Some(1234));
        assert_eq!(p.version(), Some(semver::Version::new(0, 2, 0)));
        assert!(p.notes.contains("sélecteur"));
        assert!(matches!(
            analyser_publication("pas du json"),
            Err(ErreurMiseAJour::ReponseInvalide(_))
        ));
        // Champs facultatifs absents.
        let p = analyser_publication(r#"{"tag_name":"v1.0.0","body":null}"#).unwrap();
        assert_eq!(p.build, None);
        assert_eq!(p.url_archive, None);
    }

    #[test]
    fn ligne_build_toleree() {
        for notes in [
            "build: ABCDEF1234567",
            "  - **build:** `abcdef1234567`",
            "Notes\nBuild : abcdef1234567 (CI)",
        ] {
            assert_eq!(
                build_depuis_notes(notes).as_deref(),
                Some("abcdef1234567"),
                "{notes}"
            );
        }
        assert_eq!(build_depuis_notes("build: xyz"), None);
        assert_eq!(build_depuis_notes("rebuild: abcdef1234567"), None);
        assert_eq!(build_depuis_notes(""), None);
    }

    #[test]
    fn comparaison_des_builds() {
        let pub_sha = Publication {
            etiquette: "v0.1.0".into(),
            build: Some(SHA.into()),
            ..Default::default()
        };
        // Même commit (forme abrégée ou complète, casse indifférente).
        assert_eq!(
            comparer(&courant(Some(SHA), "0.1.0"), &pub_sha),
            Verdict::AJour
        );
        assert_eq!(
            comparer(&courant(Some("0123456"), "0.1.0"), &pub_sha),
            Verdict::AJour
        );
        assert_eq!(
            comparer(&courant(Some(&SHA.to_uppercase()), "0.1.0"), &pub_sha),
            Verdict::AJour
        );
        // Commit différent à version égale : nouvelle version.
        assert_eq!(
            comparer(&courant(Some("fedcba9876543"), "0.1.0"), &pub_sha),
            Verdict::Disponible
        );
        // Compilation locale.
        assert_eq!(
            comparer(&courant(None, "0.1.0"), &pub_sha),
            Verdict::Developpement
        );
        // Sans build dans les notes : la version décide.
        let sans_build = Publication {
            etiquette: "v0.2.0".into(),
            ..Default::default()
        };
        assert_eq!(
            comparer(&courant(Some(SHA), "0.1.0"), &sans_build),
            Verdict::Disponible
        );
        assert_eq!(
            comparer(&courant(Some(SHA), "0.2.0"), &sans_build),
            Verdict::AJour
        );
        assert!(!meme_build("abc", "abc"), "SHA trop court");
        assert_eq!(court(SHA), "0123456");
    }

    #[test]
    fn libelles() {
        assert_eq!(courant(Some(SHA), "0.1.0").libelle(), "0.1.0 (0123456)");
        assert_eq!(courant(None, "0.1.0").libelle(), "0.1.0 (développement)");
    }

    fn config(serveur: &MockServer, programme: Option<PathBuf>) -> ConfigMiseAJour {
        ConfigMiseAJour {
            url_api: format!("{}/repos/x/y/releases/latest", serveur.uri()),
            courant: courant(Some("fedcba9876543"), "0.1.0"),
            dossier_programme: programme,
        }
    }

    async fn bloquant<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        tokio::task::spawn_blocking(f).await.unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn recherche_sur_faux_serveur() {
        let serveur = MockServer::start().await;
        let url_archive = format!("{}/telechargement/{NOM_ARCHIVE}", serveur.uri());
        Mock::given(method("GET"))
            .and(path("/repos/x/y/releases/latest"))
            .and(header("accept", "application/vnd.github+json"))
            .respond_with(
                ResponseTemplate::new(200).set_body_string(json_publication(&url_archive, SHA)),
            )
            .expect(1)
            .mount(&serveur)
            .await;
        let c = config(&serveur, None);
        let (verdict, p) = bloquant(move || verifier(&c)).await.unwrap();
        assert_eq!(verdict, Verdict::Disponible);
        assert_eq!(p.url_archive.as_deref(), Some(url_archive.as_str()));
        // Agent utilisateur explicite.
        let requetes = serveur.received_requests().await.unwrap();
        let agent = requetes[0]
            .headers
            .get("user-agent")
            .unwrap()
            .to_str()
            .unwrap();
        assert!(agent.starts_with("MediaManagerRust/0.1.0"), "{agent}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn erreurs_http_signalees_proprement() {
        let serveur = MockServer::start().await;
        Mock::given(path("/repos/x/y/releases/latest"))
            .respond_with(ResponseTemplate::new(404).set_body_string(r#"{"message":"Not Found"}"#))
            .up_to_n_times(1)
            .mount(&serveur)
            .await;
        Mock::given(path("/repos/x/y/releases/latest"))
            .respond_with(
                ResponseTemplate::new(403)
                    .insert_header("x-ratelimit-remaining", "0")
                    .set_body_string(r#"{"message":"API rate limit exceeded"}"#),
            )
            .up_to_n_times(1)
            .mount(&serveur)
            .await;
        Mock::given(path("/repos/x/y/releases/latest"))
            .respond_with(ResponseTemplate::new(500).set_body_string(r#"{"message":"panne"}"#))
            .mount(&serveur)
            .await;
        let c = config(&serveur, None);
        let (a, b, d) = bloquant(move || (rechercher(&c), rechercher(&c), rechercher(&c))).await;
        assert!(matches!(a, Err(ErreurMiseAJour::AucunePublication)));
        assert!(matches!(b, Err(ErreurMiseAJour::LimiteAtteinte)));
        let d = d.unwrap_err();
        assert!(matches!(d, ErreurMiseAJour::Http(500, _)));
        assert_eq!(d.to_string(), "réponse HTTP 500 du serveur : panne");
        // Serveur injoignable (port fermé) : erreur réseau, pas de panique.
        let injoignable = ConfigMiseAJour {
            url_api: "http://127.0.0.1:9/releases/latest".into(),
            ..ConfigMiseAJour::default()
        };
        let e = bloquant(move || rechercher(&injoignable))
            .await
            .unwrap_err();
        assert!(matches!(e, ErreurMiseAJour::Reseau(_)), "{e}");
        assert!(e.to_string().starts_with("serveur injoignable"));
    }

    #[test]
    fn extraction_et_echange_des_fichiers() {
        let d = tempfile::tempdir().unwrap();
        let programme = d.path().join("programme");
        fs::create_dir_all(&programme).unwrap();
        fs::write(programme.join("mediamanager-gui.exe"), b"ancien gui").unwrap();
        fs::write(programme.join("mediamanager-cli.exe"), b"ancien cli").unwrap();
        fs::write(programme.join("ui.json"), b"{}").unwrap();
        let archive = d.path().join("a.zip");
        fs::write(
            &archive,
            archive_test(&[
                ("MediaManagerRust/mediamanager-gui.exe", b"nouveau gui"),
                ("MediaManagerRust/mediamanager-cli.exe", b"nouveau cli"),
                ("MediaManagerRust/LISEZMOI.txt", b"notice"),
            ]),
        )
        .unwrap();
        let extrait = d.path().join("extrait");
        let mut fichiers = extraire_archive(&archive, &extrait).unwrap();
        fichiers.sort();
        // Dossier racine commun retiré.
        assert_eq!(
            fichiers,
            [
                PathBuf::from("LISEZMOI.txt"),
                PathBuf::from("mediamanager-cli.exe"),
                PathBuf::from("mediamanager-gui.exe")
            ]
        );
        echanger_fichiers(&extrait, &fichiers, &programme).unwrap();
        let lire = |n: &str| fs::read_to_string(programme.join(n)).unwrap();
        assert_eq!(lire("mediamanager-gui.exe"), "nouveau gui");
        assert_eq!(lire("mediamanager-gui.exe.old"), "ancien gui");
        assert_eq!(lire("mediamanager-cli.exe.old"), "ancien cli");
        assert_eq!(lire("LISEZMOI.txt"), "notice");
        assert_eq!(lire("ui.json"), "{}", "données de l'utilisateur intactes");
        // Démarrage suivant : les .old disparaissent.
        fs::create_dir_all(programme.join(".mise-a-jour-42")).unwrap();
        assert_eq!(nettoyer_anciens(&programme), 2);
        assert!(!programme.join("mediamanager-gui.exe.old").exists());
        assert!(!programme.join(".mise-a-jour-42").exists());
        assert_eq!(lire("mediamanager-gui.exe"), "nouveau gui");
    }

    #[test]
    fn echange_annule_en_cas_d_echec() {
        let d = tempfile::tempdir().unwrap();
        let programme = d.path().join("programme");
        let extrait = d.path().join("extrait");
        fs::create_dir_all(&programme).unwrap();
        fs::create_dir_all(&extrait).unwrap();
        fs::write(programme.join("a.exe"), b"ancien a").unwrap();
        fs::write(extrait.join("a.exe"), b"nouveau a").unwrap();
        // Le second fichier est absent de l'extraction : échec, retour à l'état initial.
        let relatifs = [PathBuf::from("a.exe"), PathBuf::from("absent.exe")];
        assert!(echanger_fichiers(&extrait, &relatifs, &programme).is_err());
        assert_eq!(
            fs::read_to_string(programme.join("a.exe")).unwrap(),
            "ancien a"
        );
        assert!(!programme.join("a.exe.old").exists());
    }

    #[test]
    fn archive_malveillante_refusee() {
        let d = tempfile::tempdir().unwrap();
        let archive = d.path().join("a.zip");
        fs::write(&archive, archive_test(&[("../evasion.exe", b"x")])).unwrap();
        assert!(matches!(
            extraire_archive(&archive, &d.path().join("x")),
            Err(ErreurMiseAJour::Archive(_))
        ));
        assert!(!d.path().join("evasion.exe").exists());
        fs::write(&archive, b"pas une archive").unwrap();
        assert!(extraire_archive(&archive, &d.path().join("x")).is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn installation_complete_sur_faux_serveur() {
        let serveur = MockServer::start().await;
        let contenu = archive_test(&[
            ("mediamanager-gui.exe", b"nouveau gui"),
            ("mediamanager-cli.exe", b"nouveau cli"),
        ]);
        Mock::given(method("GET"))
            .and(path(format!("/telechargement/{NOM_ARCHIVE}")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(contenu))
            .mount(&serveur)
            .await;
        let d = tempfile::tempdir().unwrap();
        let programme = d.path().to_path_buf();
        fs::write(programme.join("mediamanager-gui.exe"), b"ancien").unwrap();
        let publication = Publication {
            etiquette: "v0.2.0".into(),
            build: Some(SHA.into()),
            url_archive: Some(format!("{}/telechargement/{NOM_ARCHIVE}", serveur.uri())),
            ..Default::default()
        };
        let c = config(&serveur, Some(programme.clone()));
        let (resultat, etapes) = bloquant(move || {
            let mut etapes = Vec::new();
            let r = installer_publication(&c, &publication, &mut |e| etapes.push(e));
            (r, etapes)
        })
        .await;
        assert_eq!(resultat.unwrap(), programme);
        assert!(etapes.contains(&EtapeInstallation::Extraction));
        assert!(etapes.iter().any(|e| matches!(
            e,
            EtapeInstallation::Telechargement { fait, .. } if *fait > 0
        )));
        assert_eq!(
            fs::read_to_string(programme.join("mediamanager-gui.exe")).unwrap(),
            "nouveau gui"
        );
        assert!(programme.join("mediamanager-gui.exe.old").exists());
        assert!(programme.join("mediamanager-cli.exe").exists());
        // Dossier temporaire supprimé.
        let restes: Vec<_> = fs::read_dir(&programme)
            .unwrap()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(PREFIXE_TEMPORAIRE)
            })
            .collect();
        assert!(restes.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn installation_refusee_sans_archive_ou_en_developpement() {
        let serveur = MockServer::start().await;
        let d = tempfile::tempdir().unwrap();
        let mut c = config(&serveur, Some(d.path().to_path_buf()));
        let sans_archive = Publication::default();
        assert!(matches!(
            installer_publication(&c, &sans_archive, &mut |_| {}),
            Err(ErreurMiseAJour::ArchiveAbsente)
        ));
        c.courant.build = None;
        let avec_archive = Publication {
            url_archive: Some(format!("{}/x.zip", serveur.uri())),
            ..Default::default()
        };
        assert!(installer_publication(&c, &avec_archive, &mut |_| {}).is_err());
        // Archive introuvable (404) : erreur HTTP, programme intact.
        c.courant.build = Some("fedcba9876543".into());
        fs::write(d.path().join("a.exe"), b"ancien").unwrap();
        let r = bloquant(move || installer_publication(&c, &avec_archive, &mut |_| {})).await;
        assert!(matches!(r, Err(ErreurMiseAJour::Http(404, _))));
        assert_eq!(
            fs::read_to_string(d.path().join("a.exe")).unwrap(),
            "ancien"
        );
    }
}
