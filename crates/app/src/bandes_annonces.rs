//! Bandes-annonces : récupération chez le fournisseur (TMDB, vidéos YouTube)
//! et téléchargement avec `yt-dlp` dans le dossier du film.
//!
//! - [`Application::recuperer_bandes_annonces`] ajoute (ou met à jour) la
//!   liste des bandes-annonces des films ; le scraping complet le fait aussi ;
//! - [`Application::telecharger_bande_annonce`] télécharge une bande-annonce
//!   sous le nom `<base du film>-trailer.<ext>` (jamais d'écrasement : un
//!   numéro est ajouté, `-trailer.2.<ext>`), l'enregistre dans le film
//!   (type bande-annonce) et met à jour le NFO si c'est la bande-annonce
//!   principale ;
//! - [`Application::telecharger_bandes_annonces_auto`] choisit la meilleure
//!   bande-annonce de chaque film (langue, qualité, YouTube) et la télécharge.
//!
//! `yt-dlp` est cherché dans l'ordre : chemin des réglages (« Outils
//! externes »), dossier `outils/` ou `addons/` à côté de l'exécutable, `PATH`.
//! Il est lancé sans fenêtre console, avec un délai maximal, et tué si la
//! tâche est annulée.

use std::path::{Path, PathBuf};
use std::time::Duration;

use mediamanager_core::bandes_annonces::{
    FormatVideoBandeAnnonce, arguments_yt_dlp, chemin_bande_annonce_libre, choisir_bandes_annonces,
    cle_bande_annonce, est_extension_video, fusionner_bandes_annonces, identifiant_youtube,
    progression_yt_dlp, url_telechargement,
};
use mediamanager_core::flux::{ErreurFlux, executer_en_continu, trouver_yt_dlp, version_yt_dlp};
use mediamanager_core::modele::{BandeAnnonce, Film, TypeFichier};
use mediamanager_core::nfo::commun::url_bande_annonce;
use mediamanager_core::nfo::outils::{FormatBandeAnnonce, url_bande_annonce_vers_nfo};
use mediamanager_scrapers as sc;
use mediamanager_scrapers::{FournisseurBandesAnnonces, OptionsRecherche, TmdbFilm};
use mediamanager_store::Reglages;
use mediamanager_store::reglages::QualiteBandeAnnonce;

use crate::analyse::OutilTrouve;
use crate::application::{Application, Noyau};
use crate::artwork::ajouter_fichier;
use crate::conversion::options_recherche;
use crate::ecriture_nfo::{ecrire_nfo, saga_du_film};
use crate::erreur::{ErreurApp, Resultat};
use crate::evenements::{Evenement, NiveauMessage};
use crate::metadonnees::{charger_film, fournisseur};
use crate::taches::{ContexteTache, PoigneeTache, TypeTache};

/// Valeur de `scrape_par` des bandes-annonces venant de TMDB.
const SCRAPE_PAR_TMDB: &str = "tmdb";
/// Nom du fichier de cookies de `yt-dlp` dans le dossier des données.
const FICHIER_COOKIES: &str = "yt-dlp-cookies.txt";

/// Options d'un téléchargement de bande-annonce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionsTelechargement {
    /// Qualité préférée : hauteur maximale téléchargée.
    pub qualite: QualiteBandeAnnonce,
    /// Conteneur du fichier.
    pub format: FormatVideoBandeAnnonce,
    /// Nombre de bandes-annonces par film (mode automatique ; au moins 1).
    pub nombre: u32,
    /// Langue préférée (`fr`, `fr-FR`) ; vide = langue de scraping des réglages.
    pub langue: String,
    /// Délai maximal d'un téléchargement.
    pub delai: Duration,
    /// Réécrire le NFO si la bande-annonce téléchargée est la principale.
    pub mettre_a_jour_nfo: bool,
}

impl Default for OptionsTelechargement {
    fn default() -> Self {
        Self {
            qualite: QualiteBandeAnnonce::Hd720,
            format: FormatVideoBandeAnnonce::Mp4,
            nombre: 1,
            langue: String::new(),
            delai: Duration::from_secs(30 * 60),
            mettre_a_jour_nfo: true,
        }
    }
}

impl OptionsTelechargement {
    /// Options d'après les réglages films (qualité préférée, langue).
    pub fn depuis_reglages(r: &Reglages) -> Self {
        Self {
            qualite: r.films.qualite_bande_annonce,
            langue: r.films.langue_scraping.clone(),
            ..Self::default()
        }
    }

    fn hauteur(&self) -> u32 {
        match self.qualite {
            QualiteBandeAnnonce::Sd => 480,
            QualiteBandeAnnonce::Hd720 => 720,
            QualiteBandeAnnonce::Hd1080 => 1080,
            QualiteBandeAnnonce::Uhd => 2160,
        }
    }
}

/// Un fichier de bande-annonce téléchargé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FichierBandeAnnonce {
    pub film_id: String,
    /// Chemin du fichier créé (`<base>-trailer.<ext>`).
    pub chemin: PathBuf,
    /// URL d'origine.
    pub url: String,
}

/// Bilan d'un téléchargement de bandes-annonces.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapportTelechargement {
    pub telecharges: Vec<FichierBandeAnnonce>,
    /// Films déjà pourvus du nombre voulu de fichiers de bande-annonce.
    pub deja_presents: Vec<String>,
    /// Films sans bande-annonce YouTube exploitable.
    pub sans_bande_annonce: Vec<String>,
    /// Films verrouillés, laissés intacts.
    pub verrouilles: Vec<String>,
    /// (identifiant du film, message d'erreur).
    pub echecs: Vec<(String, String)>,
}

/// Bilan de la récupération des bandes-annonces chez le fournisseur.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapportBandesAnnonces {
    /// Films dont la liste a été mise à jour (nombre de bandes-annonces
    /// du fournisseur).
    pub mis_a_jour: Vec<(String, usize)>,
    /// Films pour lesquels le fournisseur n'a rien renvoyé.
    pub sans_resultat: Vec<String>,
    pub verrouilles: Vec<String>,
    /// (identifiant du film, message d'erreur).
    pub echecs: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// Outil yt-dlp
// ---------------------------------------------------------------------------

/// Chemin de `yt-dlp` d'après les réglages et la configuration des outils.
pub(crate) fn chemin_yt_dlp(n: &Noyau) -> Option<PathBuf> {
    let regle = n.verrou().reglages.general.chemin_yt_dlp.clone();
    trouver_yt_dlp(&n.config.outils, &regle)
}

fn exiger_yt_dlp(n: &Noyau) -> Resultat<PathBuf> {
    chemin_yt_dlp(n).ok_or_else(|| ErreurApp::OutilIntrouvable("yt-dlp".into()))
}

// ---------------------------------------------------------------------------
// Récupération chez le fournisseur
// ---------------------------------------------------------------------------

/// Bande-annonce du fournisseur → modèle partagé.
pub fn vers_bande_annonce(b: &sc::BandeAnnonce) -> BandeAnnonce {
    BandeAnnonce {
        id: identifiant_youtube(&b.url).unwrap_or_default(),
        nom: b.nom.clone(),
        url: b.url.clone(),
        qualite: b.qualite.clone(),
        langue: b.langue.clone(),
        fournisseur: b.hebergeur.clone(),
        dans_nfo: false,
        date: b.date.clone(),
        scrape_par: SCRAPE_PAR_TMDB.into(),
    }
}

/// Désigne comme bande-annonce du NFO la meilleure (langue de scraping,
/// qualité préférée) si aucune ne l'est encore.
fn designer_principale(film: &mut Film, r: &Reglages) {
    if film.bandes_annonces.iter().any(|b| b.dans_nfo) {
        return;
    }
    let hauteur = OptionsTelechargement {
        qualite: r.films.qualite_bande_annonce,
        ..OptionsTelechargement::default()
    }
    .hauteur();
    let cle = choisir_bandes_annonces(&film.bandes_annonces, &r.films.langue_scraping, hauteur, 1)
        .first()
        .map(|b| cle_bande_annonce(b));
    if let Some(cle) = cle
        && let Some(b) = film
            .bandes_annonces
            .iter_mut()
            .find(|b| cle_bande_annonce(b) == cle)
    {
        b.dans_nfo = true;
    }
}

/// Interroge le fournisseur et fusionne ses bandes-annonces dans le film
/// (celles du NFO ou saisies à la main sont conservées). Renvoie le nombre
/// de bandes-annonces du fournisseur.
fn recuperer_pour_film(
    ctx: &ContexteTache,
    f: &TmdbFilm,
    film: &mut Film,
    options: &OptionsRecherche,
    r: &Reglages,
) -> Resultat<usize> {
    let bandes = ctx.bloquer(f.bandes_annonces(options))?;
    let nouvelles: Vec<BandeAnnonce> = bandes.iter().map(vers_bande_annonce).collect();
    let nombre = nouvelles.len();
    fusionner_bandes_annonces(&mut film.bandes_annonces, nouvelles, SCRAPE_PAR_TMDB);
    designer_principale(film, r);
    Ok(nombre)
}

/// Appelée par le scraping complet d'un film : récupère les bandes-annonces
/// avant l'écriture du NFO. Un échec n'interrompt pas le scraping (simple
/// avertissement). Avec `ne_pas_ecraser`, une liste déjà remplie est gardée.
pub(crate) fn recuperer_au_scrap(
    ctx: &ContexteTache,
    f: &TmdbFilm,
    film: &mut Film,
    options: &OptionsRecherche,
    r: &Reglages,
    ne_pas_ecraser: bool,
) {
    if ne_pas_ecraser && !film.bandes_annonces.is_empty() {
        return;
    }
    if let Err(e) = recuperer_pour_film(ctx, f, film, options, r) {
        ctx.message(
            NiveauMessage::Avertissement,
            format!("bandes-annonces de « {} » indisponibles : {e}", film.titre),
        );
    }
}

fn recuperer(n: &Noyau, ctx: &ContexteTache, ids: &[String]) -> Resultat<RapportBandesAnnonces> {
    let f = fournisseur(n)?;
    let reglages = n.reglages();
    let mut rapport = RapportBandesAnnonces::default();
    let total = ids.len() as u64;
    for (i, id) in ids.iter().enumerate() {
        ctx.verifier_annulation()?;
        let mut film = match charger_film(n, id) {
            Ok(f) => f,
            Err(e) => {
                rapport.echecs.push((id.clone(), e.to_string()));
                continue;
            }
        };
        ctx.progression(i as u64, total, film.titre.clone());
        if film.verrouille {
            rapport.verrouilles.push(id.clone());
            continue;
        }
        let o = options_recherche(&film, &reglages.films);
        match recuperer_pour_film(ctx, &f, &mut film, &o, &reglages) {
            Ok(0) => rapport.sans_resultat.push(id.clone()),
            Ok(nombre) => {
                n.verrou().bibliotheque.enregistrer(&mut film)?;
                ctx.emettre(Evenement::FilmModifie { id: id.clone() });
                rapport.mis_a_jour.push((id.clone(), nombre));
            }
            Err(ErreurApp::Annulee) => return Err(ErreurApp::Annulee),
            Err(e) => {
                ctx.message(
                    NiveauMessage::Avertissement,
                    format!("{} : {e}", film.titre),
                );
                rapport.echecs.push((id.clone(), e.to_string()));
            }
        }
    }
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}

// ---------------------------------------------------------------------------
// Téléchargement
// ---------------------------------------------------------------------------

/// Dossier temporaire du téléchargement, supprimé à la sortie.
struct DossierTemporaire(PathBuf);

impl Drop for DossierTemporaire {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Fichier vidéo le plus volumineux produit par `yt-dlp` dans `dossier`.
fn fichier_produit(dossier: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dossier)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| {
            let p = e.path();
            p.is_file()
                && p.extension()
                    .is_some_and(|x| est_extension_video(&x.to_string_lossy()))
        })
        .max_by_key(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .map(|e| e.path())
}

/// Déplace `source` vers `cible` (copie puis suppression entre volumes).
fn deplacer(source: &Path, cible: &Path) -> Resultat<()> {
    if std::fs::rename(source, cible).is_ok() {
        return Ok(());
    }
    std::fs::copy(source, cible).map_err(|e| ErreurApp::io(cible, e))?;
    let _ = std::fs::remove_file(source);
    Ok(())
}

/// Télécharge une bande-annonce dans le dossier du film, l'ajoute au film
/// et met à jour le NFO si besoin.
fn telecharger_une(
    n: &Noyau,
    ctx: &ContexteTache,
    yt_dlp: &Path,
    film_id: &str,
    bande: &BandeAnnonce,
    options: &OptionsTelechargement,
    avec_progression: bool,
) -> Resultat<FichierBandeAnnonce> {
    let film = charger_film(n, film_id)?;
    if film.verrouille {
        return Err(ErreurApp::Verrouille(film.titre));
    }
    let url = url_telechargement(bande);
    if url.is_empty() {
        return Err(ErreurApp::Telechargement {
            url,
            message: "adresse de bande-annonce vide".into(),
        });
    }
    if !film.chemin.is_dir() {
        return Err(ErreurApp::Introuvable(format!(
            "dossier du film « {} »",
            film.titre
        )));
    }
    let base = film.nom_video_sans_pile();
    if base.is_empty() {
        return Err(ErreurApp::Introuvable(format!(
            "fichier vidéo du film « {} »",
            film.titre
        )));
    }

    let jour = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temporaire = DossierTemporaire(
        film.chemin
            .join(format!(".bande-annonce-{}-{jour}", ctx.id())),
    );
    std::fs::create_dir_all(&temporaire.0).map_err(|e| ErreurApp::io(&temporaire.0, e))?;
    let cookies = n.verrou().dossiers.donnees.join(FICHIER_COOKIES);
    let args = arguments_yt_dlp(
        &url,
        &temporaire.0.join("trailer.%(ext)s"),
        options.hauteur(),
        options.format,
        cookies.is_file().then_some(cookies.as_path()),
    );
    let titre = film.titre.clone();
    let annule = || ctx.est_annulee();
    let mut dernier = -1i64;
    let mut suivre = |ligne: &str| {
        if avec_progression
            && let Some(p) = progression_yt_dlp(ligne)
            && p as i64 != dernier
        {
            dernier = p as i64;
            ctx.progression(p as u64, 100, format!("Bande-annonce de « {titre} »"));
        }
    };
    if avec_progression {
        ctx.progression(0, 100, format!("Bande-annonce de « {titre} »"));
    }
    executer_en_continu(yt_dlp, &args, options.delai, &annule, &mut suivre).map_err(
        |e| match e {
            ErreurFlux::Annule => ErreurApp::Annulee,
            autre => ErreurApp::Telechargement {
                url: url.clone(),
                message: autre.to_string(),
            },
        },
    )?;

    let produit = fichier_produit(&temporaire.0).ok_or_else(|| ErreurApp::Telechargement {
        url: url.clone(),
        message: "yt-dlp n'a produit aucun fichier vidéo".into(),
    })?;
    let ext = produit
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "mp4".into());
    let cible = chemin_bande_annonce_libre(&film.chemin, &base, &ext);
    deplacer(&produit, &cible)?;

    // Le film peut avoir changé pendant le téléchargement : relecture.
    let mut film = charger_film(n, film_id)?;
    ajouter_fichier(&mut film, &cible, TypeFichier::Trailer);
    let cle = cle_bande_annonce(bande);
    if !film
        .bandes_annonces
        .iter()
        .any(|b| cle_bande_annonce(b) == cle)
    {
        let mut b = bande.clone();
        if b.scrape_par.is_empty() {
            b.scrape_par = "manuel".into();
        }
        film.bandes_annonces.push(b);
    }
    let principale = url_bande_annonce(&film.bandes_annonces, FormatBandeAnnonce::Moderne);
    let est_principale = principale.as_deref()
        == Some(url_bande_annonce_vers_nfo(&bande.url, FormatBandeAnnonce::Moderne).as_str());
    if options.mettre_a_jour_nfo && est_principale {
        let reglages = n.reglages();
        let saga = saga_du_film(n, &film)?;
        ecrire_nfo(
            &mut film,
            saga.as_ref(),
            &reglages,
            &n.config.nommage_nfo_film,
            n.config.horodatage_nfo,
        )?;
    }
    n.verrou().bibliotheque.enregistrer(&mut film)?;
    ctx.emettre(Evenement::FilmModifie {
        id: film_id.to_string(),
    });
    ctx.message(
        NiveauMessage::Info,
        format!("bande-annonce de « {} » : {}", film.titre, cible.display()),
    );
    Ok(FichierBandeAnnonce {
        film_id: film_id.to_string(),
        chemin: cible,
        url,
    })
}

fn telecharger_unique(
    n: &Noyau,
    ctx: &ContexteTache,
    film_id: &str,
    bande: &BandeAnnonce,
    options: &OptionsTelechargement,
) -> Resultat<RapportTelechargement> {
    let yt_dlp = exiger_yt_dlp(n)?;
    let fichier = telecharger_une(n, ctx, &yt_dlp, film_id, bande, options, true)?;
    ctx.progression(100, 100, "Terminé");
    Ok(RapportTelechargement {
        telecharges: vec![fichier],
        ..RapportTelechargement::default()
    })
}

fn telecharger_auto(
    n: &Noyau,
    ctx: &ContexteTache,
    ids: &[String],
    options: &OptionsTelechargement,
) -> Resultat<RapportTelechargement> {
    let yt_dlp = exiger_yt_dlp(n)?;
    let reglages = n.reglages();
    let langue = if options.langue.trim().is_empty() {
        reglages.films.langue_scraping.clone()
    } else {
        options.langue.clone()
    };
    let voulu = options.nombre.max(1) as usize;
    let mut rapport = RapportTelechargement::default();
    let total = ids.len() as u64;
    for (i, id) in ids.iter().enumerate() {
        ctx.verifier_annulation()?;
        let mut film = match charger_film(n, id) {
            Ok(f) => f,
            Err(e) => {
                rapport.echecs.push((id.clone(), e.to_string()));
                continue;
            }
        };
        ctx.progression(i as u64, total, film.titre.clone());
        if film.verrouille {
            rapport.verrouilles.push(id.clone());
            continue;
        }
        let presents = film.fichiers_de_type(TypeFichier::Trailer).count();
        if presents >= voulu {
            rapport.deja_presents.push(id.clone());
            continue;
        }
        // Aucune bande-annonce connue : essai chez le fournisseur (sans clé
        // d'API ou en cas d'échec, on continue avec ce que l'on a).
        if film.bandes_annonces.is_empty()
            && let Ok(f) = fournisseur(n)
        {
            let o = options_recherche(&film, &reglages.films);
            match recuperer_pour_film(ctx, &f, &mut film, &o, &reglages) {
                Ok(0) => {}
                Ok(_) => {
                    n.verrou().bibliotheque.enregistrer(&mut film)?;
                }
                Err(ErreurApp::Annulee) => return Err(ErreurApp::Annulee),
                Err(e) => ctx.message(
                    NiveauMessage::Avertissement,
                    format!("{} : {e}", film.titre),
                ),
            }
        }
        let choisies: Vec<BandeAnnonce> = choisir_bandes_annonces(
            &film.bandes_annonces,
            &langue,
            options.hauteur(),
            voulu - presents,
        )
        .into_iter()
        .cloned()
        .collect();
        if choisies.is_empty() {
            rapport.sans_bande_annonce.push(id.clone());
            continue;
        }
        for bande in &choisies {
            match telecharger_une(n, ctx, &yt_dlp, id, bande, options, false) {
                Ok(fichier) => rapport.telecharges.push(fichier),
                Err(ErreurApp::Annulee) => return Err(ErreurApp::Annulee),
                Err(e) => {
                    ctx.message(
                        NiveauMessage::Avertissement,
                        format!("{} : {e}", film.titre),
                    );
                    rapport.echecs.push((id.clone(), e.to_string()));
                }
            }
        }
    }
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}

// ---------------------------------------------------------------------------
// API publique
// ---------------------------------------------------------------------------

impl Application {
    /// Chemin de `yt-dlp` avec les réglages courants (réglage, `outils/` ou
    /// `addons/` à côté de l'exécutable, `PATH`) ; aucun lancement.
    pub fn chemin_yt_dlp(&self) -> Option<PathBuf> {
        chemin_yt_dlp(&self.noyau)
    }

    /// Vrai si `yt-dlp` est localisable (rapide : aucun lancement).
    pub fn yt_dlp_disponible(&self) -> bool {
        self.chemin_yt_dlp().is_some()
    }

    /// Localise `yt-dlp` pour un chemin donné (`""` = recherche automatique)
    /// et l'interroge pour sa version (quelques secondes au plus : à appeler
    /// hors du fil d'interface).
    pub fn diagnostiquer_yt_dlp(&self, chemin: &str) -> Option<OutilTrouve> {
        let chemin = trouver_yt_dlp(&self.noyau.config.outils, chemin)?;
        Some(OutilTrouve {
            version: version_yt_dlp(&chemin),
            chemin,
        })
    }

    /// Récupère chez TMDB les bandes-annonces (vidéos YouTube) des films et
    /// les fusionne dans leur liste ; celles du NFO ou saisies à la main sont
    /// conservées. Les films verrouillés sont ignorés (tâche principale).
    pub fn recuperer_bandes_annonces(
        &self,
        ids: Vec<String>,
    ) -> PoigneeTache<RapportBandesAnnonces> {
        self.lancer(
            "Récupération des bandes-annonces",
            TypeTache::Principale,
            move |n, ctx| recuperer(n, ctx, &ids),
        )
    }

    /// Télécharge une bande-annonce dans le dossier du film sous le nom
    /// `<base du film>-trailer.<ext>` (jamais d'écrasement : `-trailer.2`,
    /// `-trailer.3`…), l'enregistre dans le film et met à jour le NFO si
    /// elle en est la bande-annonce principale (tâche principale, annulable,
    /// avec progression en pourcentage).
    ///
    /// Erreurs : [`ErreurApp::OutilIntrouvable`] sans `yt-dlp`,
    /// [`ErreurApp::Verrouille`] pour un film verrouillé,
    /// [`ErreurApp::Telechargement`] si `yt-dlp` échoue ou dépasse le délai.
    pub fn telecharger_bande_annonce(
        &self,
        film_id: &str,
        bande: &BandeAnnonce,
        options: OptionsTelechargement,
    ) -> PoigneeTache<RapportTelechargement> {
        let id = film_id.to_string();
        let bande = bande.clone();
        self.lancer(
            "Téléchargement d'une bande-annonce",
            TypeTache::Principale,
            move |n, ctx| telecharger_unique(n, ctx, &id, &bande, &options),
        )
    }

    /// Choisit la meilleure bande-annonce YouTube de chaque film (langue de
    /// scraping, qualité préférée des réglages films) et la télécharge
    /// (options issues des réglages). Voir
    /// [`telecharger_bandes_annonces_auto_avec`](Self::telecharger_bandes_annonces_auto_avec).
    pub fn telecharger_bandes_annonces_auto(
        &self,
        ids: Vec<String>,
    ) -> PoigneeTache<RapportTelechargement> {
        let options = OptionsTelechargement::depuis_reglages(&self.noyau.reglages());
        self.telecharger_bandes_annonces_auto_avec(ids, options)
    }

    /// Comme [`telecharger_bandes_annonces_auto`](Self::telecharger_bandes_annonces_auto)
    /// avec des options explicites. Les films verrouillés sont exclus ; ceux
    /// qui ont déjà `options.nombre` fichiers de bande-annonce sont laissés
    /// tels quels ; sans bande-annonce connue, TMDB est interrogé (si une clé
    /// est configurée). Sans `yt-dlp` : [`ErreurApp::OutilIntrouvable`].
    pub fn telecharger_bandes_annonces_auto_avec(
        &self,
        ids: Vec<String>,
        options: OptionsTelechargement,
    ) -> PoigneeTache<RapportTelechargement> {
        self.lancer(
            "Téléchargement des bandes-annonces",
            TypeTache::Principale,
            move |n, ctx| telecharger_auto(n, ctx, &ids, &options),
        )
    }
}
