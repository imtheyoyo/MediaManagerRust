//! Client HTTP asynchrone : timeouts, user-agent, cache disque avec TTL,
//! gestion du 429 (`Retry-After`). La validation TLS n'est jamais désactivée.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

use crate::erreur::{ErreurScraper, Resultat};

/// Configuration du client HTTP.
#[derive(Debug, Clone)]
pub struct ConfigHttp {
    pub user_agent: String,
    pub delai_connexion: Duration,
    pub delai_total: Duration,
    /// Dossier du cache disque (`None` = pas de cache).
    pub dossier_cache: Option<PathBuf>,
    /// Durée de validité des entrées du cache (15 min par défaut).
    pub ttl_cache: Duration,
    /// Nombre total de tentatives en cas de 429.
    pub essais_429: u32,
    /// Attente par défaut si `Retry-After` est absent.
    pub delai_429_defaut: Duration,
    /// Marge ajoutée à l'attente.
    pub marge_429: Duration,
    /// Au-delà, abandon immédiat.
    pub retry_after_max: Duration,
}

impl Default for ConfigHttp {
    fn default() -> Self {
        Self {
            user_agent: format!("MediaManagerRust/{}", env!("CARGO_PKG_VERSION")),
            delai_connexion: Duration::from_secs(30),
            delai_total: Duration::from_secs(30),
            dossier_cache: None,
            ttl_cache: Duration::from_secs(15 * 60),
            essais_429: 3,
            delai_429_defaut: Duration::from_secs(10),
            marge_429: Duration::from_secs(1),
            retry_after_max: Duration::from_secs(30),
        }
    }
}

/// Client HTTP partageable (clonable à faible coût).
#[derive(Debug, Clone)]
pub struct ClientHttp {
    client: reqwest::Client,
    config: ConfigHttp,
}

/// Noms de paramètres considérés comme secrets (exclus des clés de cache et des erreurs).
const PARAMS_SECRETS: [&str; 4] = ["api_key", "apikey", "token", "key"];

impl ClientHttp {
    /// Construit le client (rustls, validation TLS active).
    pub fn new(config: ConfigHttp) -> Resultat<Self> {
        let client = reqwest::Client::builder()
            .user_agent(config.user_agent.clone())
            .connect_timeout(config.delai_connexion)
            .timeout(config.delai_total)
            .build()
            .map_err(|e| ErreurScraper::Reseau(e.without_url().to_string()))?;
        Ok(Self { client, config })
    }

    /// Configuration courante.
    pub fn config(&self) -> &ConfigHttp {
        &self.config
    }

    /// GET d'un texte (JSON, HTML...). `params` : paramètres de requête
    /// (valeurs `None` omises) ; `secrets` : paramètres jamais mis en cache ni journalisés.
    pub async fn obtenir_texte(
        &self,
        url: &str,
        params: &[(&str, Option<&str>)],
        secrets: &[(&str, &str)],
    ) -> Resultat<String> {
        self.obtenir_texte_avec_entetes(url, params, secrets, &[])
            .await
    }

    /// Comme [`Self::obtenir_texte`], avec des en-têtes secrets (clés d'API
    /// transmises par en-tête) : jamais mis en cache ni journalisés.
    pub async fn obtenir_texte_avec_entetes(
        &self,
        url: &str,
        params: &[(&str, Option<&str>)],
        secrets: &[(&str, &str)],
        entetes: &[(&str, &str)],
    ) -> Resultat<String> {
        let mut u = url::Url::parse(url)
            .map_err(|e| ErreurScraper::Reseau(format!("URL invalide : {e}")))?;
        {
            let mut q = u.query_pairs_mut();
            for (k, v) in params {
                if let Some(v) = v {
                    q.append_pair(k, v);
                }
            }
        }
        let cle = self.cle_cache(&u);
        if let Some(texte) = self.lire_cache(&cle).await {
            return Ok(texte);
        }
        {
            let mut q = u.query_pairs_mut();
            for (k, v) in secrets {
                q.append_pair(k, v);
            }
        }
        let url_publique = Self::url_sans_secrets(&u);
        let texte = self
            .requete_avec_reprises(&u, &url_publique, entetes)
            .await?;
        self.ecrire_cache(&cle, &texte).await;
        Ok(texte)
    }

    /// GET d'un JSON désérialisé.
    pub async fn obtenir_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        params: &[(&str, Option<&str>)],
        secrets: &[(&str, &str)],
    ) -> Resultat<T> {
        let texte = self.obtenir_texte(url, params, secrets).await?;
        serde_json::from_str(&texte).map_err(|e| ErreurScraper::Analyse(e.to_string()))
    }

    /// GET d'un JSON désérialisé, avec des en-têtes secrets.
    pub async fn obtenir_json_avec_entetes<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        params: &[(&str, Option<&str>)],
        entetes: &[(&str, &str)],
    ) -> Resultat<T> {
        let texte = self
            .obtenir_texte_avec_entetes(url, params, &[], entetes)
            .await?;
        serde_json::from_str(&texte).map_err(|e| ErreurScraper::Analyse(e.to_string()))
    }

    async fn requete_avec_reprises(
        &self,
        u: &url::Url,
        url_publique: &str,
        entetes: &[(&str, &str)],
    ) -> Resultat<String> {
        let max = self.config.essais_429.max(1);
        let mut essai = 1;
        loop {
            let mut requete = self.client.get(u.clone());
            for (k, v) in entetes {
                requete = requete.header(*k, *v);
            }
            let rep = requete
                .send()
                .await
                .map_err(|e| ErreurScraper::Reseau(e.without_url().to_string()))?;
            let statut = rep.status();
            if statut.as_u16() == 429 {
                let attente = rep
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.trim().parse::<u64>().ok())
                    .map(Duration::from_secs)
                    .unwrap_or(self.config.delai_429_defaut);
                if attente > self.config.retry_after_max {
                    return Err(ErreurScraper::TropDeRequetes {
                        secondes: attente.as_secs(),
                    });
                }
                if essai >= max {
                    return Err(ErreurScraper::Http {
                        statut: 429,
                        url: url_publique.to_string(),
                    });
                }
                tracing::warn!(
                    "429 reçu, nouvelle tentative dans {:?}",
                    attente + self.config.marge_429
                );
                tokio::time::sleep(attente + self.config.marge_429).await;
                essai += 1;
                continue;
            }
            if statut.as_u16() == 401 {
                return Err(ErreurScraper::CleApiInvalide);
            }
            if statut.as_u16() >= 400 {
                return Err(ErreurScraper::Http {
                    statut: statut.as_u16(),
                    url: url_publique.to_string(),
                });
            }
            return rep
                .text()
                .await
                .map_err(|e| ErreurScraper::Reseau(e.without_url().to_string()));
        }
    }

    /// URL sans les paramètres secrets.
    fn url_sans_secrets(u: &url::Url) -> String {
        let mut copie = u.clone();
        let gardes: Vec<(String, String)> = u
            .query_pairs()
            .filter(|(k, _)| !PARAMS_SECRETS.contains(&k.to_lowercase().as_str()))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        copie.set_query(None);
        if !gardes.is_empty() {
            copie.query_pairs_mut().extend_pairs(gardes);
        }
        copie.to_string()
    }

    fn cle_cache(&self, u: &url::Url) -> Option<String> {
        self.config.dossier_cache.as_ref()?;
        let mut h = Sha256::new();
        h.update(Self::url_sans_secrets(u).as_bytes());
        Some(h.finalize().iter().map(|o| format!("{o:02x}")).collect())
    }

    fn chemin_cache(&self, cle: &str) -> Option<PathBuf> {
        self.config
            .dossier_cache
            .as_ref()
            .map(|d| d.join(format!("{cle}.json")))
    }

    async fn lire_cache(&self, cle: &Option<String>) -> Option<String> {
        let chemin = self.chemin_cache(cle.as_deref()?)?;
        let meta = tokio::fs::metadata(&chemin).await.ok()?;
        let age = SystemTime::now()
            .duration_since(meta.modified().ok()?)
            .unwrap_or(Duration::MAX);
        if age > self.config.ttl_cache {
            return None;
        }
        tokio::fs::read_to_string(&chemin).await.ok()
    }

    async fn ecrire_cache(&self, cle: &Option<String>, texte: &str) {
        let Some(chemin) = cle.as_deref().and_then(|c| self.chemin_cache(c)) else {
            return;
        };
        if let Some(parent) = chemin.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        if let Err(e) = tokio::fs::write(&chemin, texte).await {
            tracing::debug!("écriture du cache impossible : {e}");
        }
    }
}
