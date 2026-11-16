//! Fournisseurs optionnels activés par la présence d'une clé d'API : état des
//! services, lecture, enregistrement et test des clés, fournisseurs de notes
//! (OMDb, Trakt, MDBList) et images complémentaires Fanart.tv.
//!
//! Les clés sont lues dans le coffre à secrets (jamais en dur, jamais
//! journalisées). Sans clé, aucun appel réseau n'est tenté pour le service
//! concerné.

use mediamanager_core::modele::{Film, TypeFichier};
use mediamanager_scrapers::{
    Artwork, ArtworkEtendu, ClientHttp, ConfigFanart, ConfigHttp, ConfigMdbList, ConfigOmdb,
    ConfigTmdb, ConfigTrakt, ErreurScraper, FanartTv, FournisseurEvaluations, MdbList, Omdb,
    OptionsRecherche, TmdbFilm, Trakt, TypeArtwork, TypeArtworkEtendu, tester_cle_service,
};
use mediamanager_store::reglages::noms_secrets;
use mediamanager_store::services::{self, SERVICES};

use crate::application::{Application, Noyau};
use crate::artwork::{
    ConfigArtwork, ImageATelecharger, destinations, extension_url, meilleures_images,
};
use crate::erreur::{ErreurApp, Resultat};
use crate::evenements::NiveauMessage;
use crate::taches::{ContexteTache, PoigneeTache, TypeTache};

/// État d'un service à clé d'API (affiché dans les réglages).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtatServiceCle {
    /// Identifiant stable : `tmdb`, `fanart`, `tvdb`, `omdb`, `trakt`,
    /// `mdblist`, `simkl`.
    pub id: String,
    pub nom: String,
    /// Nature de la clé (« clé d'API », « identifiant client »…).
    pub type_cle: String,
    /// À quoi sert le service dans l'application.
    pub usage: String,
    /// Une clé non vide est enregistrée (la valeur n'est jamais exposée).
    pub cle_presente: bool,
}

impl Noyau {
    /// Clé enregistrée pour un service (`None` si absente). La clé TMDB peut
    /// aussi provenir des réglages (compatibilité).
    pub(crate) fn cle_service(&self, service: &str) -> Resultat<Option<String>> {
        if service.trim().eq_ignore_ascii_case("tmdb") {
            return match self.cle_tmdb() {
                Ok(c) => Ok(Some(c)),
                Err(ErreurApp::CleTmdbAbsente) => Ok(None),
                Err(e) => Err(e),
            };
        }
        let s = self.verrou();
        Ok(services::lire_cle_api(
            s.gestionnaire_reglages.coffre(),
            service,
        )?)
    }

    /// URL de base imposée pour un service (tests, miroir).
    fn url_service(&self, service: &str) -> Option<String> {
        self.config.urls_services.get(service).cloned()
    }

    fn client_http(&self) -> Resultat<ClientHttp> {
        Ok(ClientHttp::new(ConfigHttp {
            dossier_cache: self.dossier_cache_http(),
            ..ConfigHttp::default()
        })?)
    }
}

/// État de tous les services connus, dans l'ordre d'affichage.
pub(crate) fn etat_services(n: &Noyau) -> Resultat<Vec<EtatServiceCle>> {
    SERVICES
        .iter()
        .map(|d| {
            Ok(EtatServiceCle {
                id: d.id.into(),
                nom: d.nom.into(),
                type_cle: d.type_cle.into(),
                usage: d.usage.into(),
                cle_presente: n.cle_service(d.id)?.is_some(),
            })
        })
        .collect()
}

/// Vrai si une clé est enregistrée pour ce service (erreur si inconnu).
pub(crate) fn cle_api_enregistree(n: &Noyau, service: &str) -> Resultat<bool> {
    if services::definition_service(service).is_none() {
        // Même message que le magasin de secrets.
        services::cle_api_enregistree(n.verrou().gestionnaire_reglages.coffre(), service)?;
    }
    Ok(n.cle_service(service)?.is_some())
}

/// Enregistre (ou, si vide, supprime) la clé d'un service.
pub(crate) fn enregistrer_cle(n: &Noyau, service: &str, secret: &str) -> Resultat<()> {
    let secret = secret.trim();
    let mut s = n.verrou();
    if service.trim().eq_ignore_ascii_case("tmdb") {
        // La clé TMDB est aussi un champ des réglages : on passe par eux.
        s.reglages.general.cle_api_tmdb = secret.to_string();
        let r = s.reglages.clone();
        s.gestionnaire_reglages.enregistrer_si_modifie(&r)?;
        if secret.is_empty() {
            s.gestionnaire_reglages
                .coffre()
                .supprimer(noms_secrets::CLE_API_TMDB)?;
        }
        return Ok(());
    }
    s.gestionnaire_reglages.enregistrer_cle(service, secret)?;
    Ok(())
}

/// Teste la clé enregistrée d'un service par une requête minimale. Sans clé :
/// [`ErreurScraper::CleApiManquante`]. Services sans test (TVDB) : erreur
/// explicite.
pub(crate) fn tester_cle(n: &Noyau, ctx: &ContexteTache, service: &str) -> Resultat<()> {
    let id = services::definition_service(service)
        .ok_or_else(|| ErreurApp::Interne(format!("service inconnu : {service}")))?
        .id;
    let cle = n.cle_service(id)?.ok_or(ErreurScraper::CleApiManquante)?;
    if id == "tmdb" {
        let mut cfg = ConfigTmdb::nouvelle(cle);
        cfg.url_api = n.config.url_api_tmdb.clone();
        let f = TmdbFilm::new(cfg, n.client_http()?)?;
        let o = OptionsRecherche::default().avec_id("tmdb", "603");
        ctx.bloquer(mediamanager_scrapers::FournisseurFilm::metadonnees(&f, &o))?;
        return Ok(());
    }
    if id == "tvdb" {
        return Err(ErreurApp::Interne(
            "le test de la clé TheTVDB n'est pas disponible (fournisseur non implémenté)".into(),
        ));
    }
    let http = n.client_http()?;
    ctx.bloquer(tester_cle_service(id, &cle, &http, n.url_service(id)))?;
    Ok(())
}

impl Application {
    /// État (clé présente ou non) de tous les services à clé d'API, dans
    /// l'ordre d'affichage : TMDB, Fanart.tv, TheTVDB, OMDb, Trakt, MDBList,
    /// Simkl. Les valeurs des clés ne sont jamais exposées.
    pub fn etat_services(&self) -> Resultat<Vec<EtatServiceCle>> {
        etat_services(&self.noyau)
    }

    /// Vrai si une clé d'API est enregistrée pour ce service (`tmdb`, `fanart`,
    /// `tvdb`, `omdb`, `trakt`, `mdblist`, `simkl`).
    pub fn cle_api_enregistree(&self, service: &str) -> Resultat<bool> {
        cle_api_enregistree(&self.noyau, service)
    }

    /// Enregistre la clé d'un service dans le coffre ; une valeur vide la
    /// supprime. Aucune valeur n'est journalisée.
    pub fn enregistrer_cle(&self, service: &str, secret: &str) -> Resultat<()> {
        enregistrer_cle(&self.noyau, service, secret)
    }

    /// Teste la clé enregistrée d'un service par une requête minimale
    /// (`CleApiInvalide`, `CleApiManquante`…). Service TheTVDB : non testable.
    pub fn tester_cle_api(&self, service: &str) -> PoigneeTache<()> {
        let service = service.to_string();
        self.lancer(
            "Test de la clé d'API",
            TypeTache::Principale,
            move |n, ctx| tester_cle(n, ctx, &service),
        )
    }
}

// ---------------------------------------------------------------- notes

/// Fournisseurs de notes dont la clé est présente, dans l'ordre de priorité
/// (OMDb, Trakt, MDBList). Un service sans clé est simplement absent.
pub(crate) fn fournisseurs_evaluations(
    n: &Noyau,
    serie: bool,
) -> Resultat<Vec<Box<dyn FournisseurEvaluations>>> {
    let mut liste: Vec<Box<dyn FournisseurEvaluations>> = Vec::new();
    let http = n.client_http()?;
    if let Some(cle) = n.cle_service("omdb")? {
        let mut c = ConfigOmdb::nouvelle(cle);
        c.url_api = n.url_service("omdb");
        liste.push(Box::new(Omdb::new(c, http.clone())?));
    }
    if let Some(cle) = n.cle_service("trakt")? {
        let mut c = ConfigTrakt::nouvelle(cle);
        c.url_api = n.url_service("trakt");
        c.serie = serie;
        liste.push(Box::new(Trakt::new(c, http.clone())?));
    }
    if let Some(cle) = n.cle_service("mdblist")? {
        let mut c = ConfigMdbList::nouvelle(cle);
        c.url_api = n.url_service("mdblist");
        c.serie = serie;
        liste.push(Box::new(MdbList::new(c, http)?));
    }
    Ok(liste)
}

/// Vrai si l'erreur doit désactiver le fournisseur pour le reste du
/// traitement (clé refusée, limitation de débit) : inutile d'insister.
pub(crate) fn erreur_bloquante_pour_fournisseur(e: &ErreurScraper) -> bool {
    matches!(
        e,
        ErreurScraper::CleApiInvalide
            | ErreurScraper::TropDeRequetes { .. }
            | ErreurScraper::Http { statut: 429, .. }
    )
}

// ---------------------------------------------------------------- Fanart.tv

/// Images obtenues de Fanart.tv pour un film.
#[derive(Debug, Default)]
pub(crate) struct ImagesFanart {
    /// Types de base (affiche, fond, vignette, logo) : fusionnés avec TMDB.
    pub base: Vec<Artwork>,
    /// Types supplémentaires (bannière, clearart, disque).
    pub supplementaires: Vec<ArtworkEtendu>,
}

fn fanart(n: &Noyau) -> Resultat<Option<FanartTv>> {
    let Some(cle) = n.cle_service("fanart")? else {
        return Ok(None);
    };
    let mut c = ConfigFanart::nouvelle(cle);
    c.url_api = n.url_service("fanart");
    Ok(Some(FanartTv::new(c, n.client_http()?)?))
}

/// Images Fanart.tv d'un film (`options.ids` doit contenir `imdb` ou `tmdb`).
/// Sans clé : `None`, aucun appel réseau. Un échec est signalé par un
/// avertissement et ne bloque pas le scraping (`None`).
pub(crate) fn images_fanart(
    n: &Noyau,
    ctx: &ContexteTache,
    options: &OptionsRecherche,
) -> Option<ImagesFanart> {
    let f = match fanart(n) {
        Ok(Some(f)) => f,
        Ok(None) => return None,
        Err(e) => {
            ctx.message(NiveauMessage::Avertissement, format!("Fanart.tv : {e}"));
            return None;
        }
    };
    match ctx.bloquer(f.artwork_film_etendu(options, None)) {
        Ok(v) => {
            let mut sortie = ImagesFanart::default();
            for a in v {
                match a.type_artwork {
                    TypeArtworkEtendu::Banniere
                    | TypeArtworkEtendu::ClearArt
                    | TypeArtworkEtendu::Disque => sortie.supplementaires.push(a),
                    _ => sortie.base.extend(a.en_artwork()),
                }
            }
            Some(sortie)
        }
        Err(ErreurScraper::AucunResultat) | Err(ErreurScraper::IdentifiantManquant(_)) => None,
        Err(e) => {
            ctx.message(
                NiveauMessage::Avertissement,
                format!("images Fanart.tv indisponibles : {e}"),
            );
            None
        }
    }
}

/// Fusionne les images TMDB et Fanart.tv. Pour le logo transparent et la
/// vignette, les images Fanart.tv (de meilleure qualité, cataloguées par la
/// communauté) remplacent celles de TMDB dès qu'il y en a ; les affiches et
/// fonds s'additionnent (le choix se fait ensuite par langue et votes).
pub(crate) fn fusionner_artwork(tmdb: Vec<Artwork>, fanart: Vec<Artwork>) -> Vec<Artwork> {
    let remplace = |t: TypeArtwork| fanart.iter().any(|a| a.type_artwork == t);
    let mut sortie: Vec<Artwork> = tmdb
        .into_iter()
        .filter(|a| {
            !(matches!(
                a.type_artwork,
                TypeArtwork::LogoTransparent | TypeArtwork::Vignette
            ) && remplace(a.type_artwork))
        })
        .collect();
    sortie.extend(fanart);
    sortie
}

/// Téléchargements des images supplémentaires Fanart.tv (bannière, clearart,
/// disque), avec les noms de fichiers des motifs configurés (`<b>-banner`,
/// `<b>-clearart`, `<b>-discart` par défaut). Un type absent de
/// `config.types`, ou sans motif, n'est pas téléchargé.
pub(crate) fn planifier_images_supplementaires(
    film: &Film,
    images: &[ArtworkEtendu],
    config: &ConfigArtwork,
    langues: &[String],
) -> Vec<ImageATelecharger> {
    const CORRESPONDANCE: [(TypeArtworkEtendu, TypeFichier); 3] = [
        (TypeArtworkEtendu::Banniere, TypeFichier::Banner),
        (TypeArtworkEtendu::ClearArt, TypeFichier::Clearart),
        (TypeArtworkEtendu::Disque, TypeFichier::Disc),
    ];
    let mut sortie = Vec::new();
    for (te, tf) in CORRESPONDANCE {
        // Configuration par défaut : ces types sont activés (ils ne servent
        // qu'avec Fanart.tv) ; sinon, seule la liste explicite compte.
        if !(config.types.contains(&tf) || config.types == ConfigArtwork::default().types) {
            continue;
        }
        // Le classement par langue/votes est celui des images de base.
        let candidates: Vec<Artwork> = images
            .iter()
            .filter(|a| a.type_artwork == te)
            .map(|a| Artwork {
                fournisseur: a.fournisseur.clone(),
                type_artwork: TypeArtwork::Affiche,
                url_apercu: a.url_apercu.clone(),
                url_originale: a.url_originale.clone(),
                langue: a.langue.clone(),
                votes: a.votes,
                tailles: a.tailles.clone(),
            })
            .collect();
        let Some(meilleure) = meilleures_images(&candidates, TypeArtwork::Affiche, langues, 1)
            .into_iter()
            .next()
        else {
            continue;
        };
        let url = if meilleure.url_originale.is_empty() {
            meilleure.url_apercu.clone()
        } else {
            meilleure.url_originale.clone()
        };
        if url.is_empty() {
            continue;
        }
        let dest = destinations(film, tf, &config.noms, &extension_url(&url));
        if !dest.is_empty() {
            sortie.push(ImageATelecharger {
                type_fichier: tf,
                url,
                destinations: dest,
                indice: 0,
            });
        }
    }
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;
    use mediamanager_core::modele::FichierMedia;
    use std::path::PathBuf;

    fn art(t: TypeArtwork, url: &str) -> Artwork {
        Artwork {
            fournisseur: "x".into(),
            type_artwork: t,
            url_apercu: String::new(),
            url_originale: url.into(),
            langue: String::new(),
            votes: 0,
            tailles: Vec::new(),
        }
    }

    #[test]
    fn fanart_remplace_logo_et_vignette_seulement() {
        let tmdb = vec![
            art(TypeArtwork::LogoTransparent, "tmdb-logo"),
            art(TypeArtwork::Vignette, "tmdb-thumb"),
            art(TypeArtwork::Affiche, "tmdb-poster"),
            art(TypeArtwork::Fond, "tmdb-fond"),
        ];
        let fa = vec![
            art(TypeArtwork::LogoTransparent, "fa-logo"),
            art(TypeArtwork::Fond, "fa-fond"),
        ];
        let m = fusionner_artwork(tmdb, fa);
        let urls: Vec<&str> = m.iter().map(|a| a.url_originale.as_str()).collect();
        assert_eq!(
            urls,
            [
                "tmdb-thumb",
                "tmdb-poster",
                "tmdb-fond",
                "fa-logo",
                "fa-fond"
            ]
        );
    }

    #[test]
    fn noms_des_images_supplementaires() {
        let dossier = PathBuf::from("/films/Matrix (1999)");
        let film = Film {
            chemin: dossier.clone(),
            fichiers: vec![FichierMedia::nouveau(
                &dossier.join("Matrix.mkv"),
                TypeFichier::Video,
            )],
            ..Film::default()
        };
        let ea = |t, url: &str, votes| ArtworkEtendu {
            fournisseur: "fanarttv".into(),
            type_artwork: t,
            url_apercu: String::new(),
            url_originale: url.into(),
            langue: String::new(),
            votes,
            saison: None,
            tailles: Vec::new(),
        };
        let images = vec![
            ea(TypeArtworkEtendu::Banniere, "https://x/b1.jpg", 1),
            ea(TypeArtworkEtendu::Banniere, "https://x/b2.png", 9),
            ea(TypeArtworkEtendu::ClearArt, "https://x/c.png", 1),
            ea(TypeArtworkEtendu::Disque, "https://x/d.png", 1),
        ];
        let plan = planifier_images_supplementaires(&film, &images, &ConfigArtwork::default(), &[]);
        let noms: Vec<(TypeFichier, String, String)> = plan
            .iter()
            .map(|p| {
                (
                    p.type_fichier,
                    p.url.clone(),
                    p.destinations[0]
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                )
            })
            .collect();
        assert_eq!(
            noms,
            vec![
                (
                    TypeFichier::Banner,
                    "https://x/b2.png".into(),
                    "Matrix-banner.png".into()
                ),
                (
                    TypeFichier::Clearart,
                    "https://x/c.png".into(),
                    "Matrix-clearart.png".into()
                ),
                (
                    TypeFichier::Disc,
                    "https://x/d.png".into(),
                    "Matrix-discart.png".into()
                ),
            ]
        );
    }
}
