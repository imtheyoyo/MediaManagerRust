//! Base SQLite de la médiathèque.
//!
//! # Schéma
//!
//! Chaque entité racine (film, saga, série) est stockée comme un **document
//! JSON** (sérialisation serde du modèle) sous une clé UUID, avec quelques
//! colonnes dupliquées et indexées pour les recherches courantes (`titre`,
//! `annee`, `chemin`). Les identifiants externes (tmdb, imdb, tvdb…) de toutes
//! les entités, épisodes compris, sont indexés dans `ids_externes`.
//!
//! # Épisodes dans le document de la série
//!
//! Le modèle (`Serie::episodes`, `Serie::saisons`) fait de la série un agrégat :
//! on la charge et l'enregistre toujours entière. Les épisodes restent donc
//! **dans le JSON de la série** plutôt que dans une table séparée :
//! - écriture atomique de l'agrégat, aucun épisode orphelin possible (le
//!   rattachement parent/enfant n'est plus à reconstruire au chargement) ;
//! - correspondance 1:1 avec les types serde, pas de double source de vérité ;
//! - coût : réécrire toute la série quand un épisode change (quelques centaines
//!   de Kio au pire, dans une transaction : acceptable).
//!
//! La recherche d'un épisode par identifiant externe passe par `ids_externes`
//! (colonne `racine_id` = série) : voir
//! [`Bibliotheque::chercher_episodes_par_id_externe`].
//!
//! # Versions du schéma
//!
//! `PRAGMA user_version` porte la version ; [`MIGRATIONS`] est appliqué dans
//! l'ordre, chaque palier dans sa propre transaction (idempotent par
//! construction : un palier n'est jamais rejoué).

use std::path::Path;

use mediamanager_core::modele::{Film, Saga, Serie};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::erreur::{ErreurStockage, Resultat};

/// Nom du fichier de base dans le dossier de données.
pub const NOM_BASE: &str = "bibliotheque.db";

/// Palier de migration : reçoit la transaction en cours.
type Migration = fn(&Transaction<'_>) -> Resultat<()>;

/// Migrations du schéma, dans l'ordre ; l'indice `i` amène à la version `i+1`.
const MIGRATIONS: &[Migration] = &[migration_1_tables, migration_2_ids_externes];

/// Version de schéma produite par cette version du code.
pub const VERSION_SCHEMA: i64 = MIGRATIONS.len() as i64;

fn migration_1_tables(tx: &Transaction<'_>) -> Resultat<()> {
    tx.execute_batch(
        "CREATE TABLE films (
             id TEXT PRIMARY KEY NOT NULL,
             titre TEXT NOT NULL DEFAULT '',
             annee INTEGER NOT NULL DEFAULT 0,
             chemin TEXT NOT NULL DEFAULT '',
             json TEXT NOT NULL,
             modifie_le INTEGER NOT NULL DEFAULT 0
         );
         CREATE INDEX films_titre ON films(titre COLLATE NOCASE);
         CREATE INDEX films_chemin ON films(chemin);
         CREATE TABLE sagas (
             id TEXT PRIMARY KEY NOT NULL,
             titre TEXT NOT NULL DEFAULT '',
             annee INTEGER NOT NULL DEFAULT 0,
             chemin TEXT NOT NULL DEFAULT '',
             json TEXT NOT NULL,
             modifie_le INTEGER NOT NULL DEFAULT 0
         );
         CREATE INDEX sagas_titre ON sagas(titre COLLATE NOCASE);
         CREATE TABLE series (
             id TEXT PRIMARY KEY NOT NULL,
             titre TEXT NOT NULL DEFAULT '',
             annee INTEGER NOT NULL DEFAULT 0,
             chemin TEXT NOT NULL DEFAULT '',
             json TEXT NOT NULL,
             modifie_le INTEGER NOT NULL DEFAULT 0
         );
         CREATE INDEX series_titre ON series(titre COLLATE NOCASE);
         CREATE INDEX series_chemin ON series(chemin);
         CREATE TABLE valeurs (
             cle TEXT PRIMARY KEY NOT NULL,
             valeur TEXT NOT NULL
         );",
    )?;
    Ok(())
}

fn migration_2_ids_externes(tx: &Transaction<'_>) -> Resultat<()> {
    tx.execute_batch(
        "CREATE TABLE ids_externes (
             table_racine TEXT NOT NULL,
             racine_id TEXT NOT NULL,
             type_entite TEXT NOT NULL,
             entite_id TEXT NOT NULL,
             cle TEXT NOT NULL,
             valeur TEXT NOT NULL,
             PRIMARY KEY (type_entite, entite_id, cle)
         );
         CREATE INDEX ids_externes_valeur ON ids_externes(cle, valeur);
         CREATE INDEX ids_externes_racine ON ids_externes(table_racine, racine_id);",
    )?;
    // Les documents existants sont indexés.
    reindexer::<Film>(tx)?;
    reindexer::<Saga>(tx)?;
    reindexer::<Serie>(tx)?;
    Ok(())
}

fn reindexer<E: Entite>(conn: &Connection) -> Resultat<()> {
    let docs: Vec<(String, String)> = {
        let mut st = conn.prepare(&format!("SELECT id, json FROM {}", E::TABLE))?;
        st.query_map([], |l| Ok((l.get(0)?, l.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    for (id, json) in docs {
        match serde_json::from_str::<E>(&json) {
            Ok(mut e) => {
                e.definir_id(id);
                indexer_ids(conn, &e)?;
            }
            Err(err) => tracing::warn!("{} {id} illisible, non indexé : {err}", E::TABLE),
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Entités
// ---------------------------------------------------------------------------

mod scelle {
    pub trait Scelle {}
    impl Scelle for mediamanager_core::modele::Film {}
    impl Scelle for mediamanager_core::modele::Saga {}
    impl Scelle for mediamanager_core::modele::Serie {}
}

/// Identifiant externe indexé : (type d'entité, id de l'entité, clé, valeur).
#[doc(hidden)]
pub struct IdIndexe {
    pub type_entite: &'static str,
    pub entite_id: String,
    pub cle: String,
    pub valeur: String,
}

/// Entité racine stockable (film, saga, série). Trait scellé.
pub trait Entite: Serialize + DeserializeOwned + scelle::Scelle {
    /// Table SQL.
    const TABLE: &'static str;
    /// Identifiant interne (vide = pas encore enregistré).
    fn id(&self) -> &str;
    #[doc(hidden)]
    fn definir_id(&mut self, id: String);
    #[doc(hidden)]
    fn titre(&self) -> &str;
    #[doc(hidden)]
    fn annee(&self) -> i64 {
        0
    }
    #[doc(hidden)]
    fn chemin(&self) -> String {
        String::new()
    }
    /// Complète l'entité avant écriture (identifiants des sous-entités…).
    #[doc(hidden)]
    fn preparer(&mut self) {}
    #[doc(hidden)]
    fn ids_indexes(&self) -> Vec<IdIndexe>;
}

fn ids_de(
    type_entite: &'static str,
    entite_id: &str,
    ids: &mediamanager_core::modele::Identifiants,
) -> impl Iterator<Item = IdIndexe> {
    ids.iter()
        .map(move |(cle, v)| IdIndexe {
            type_entite,
            entite_id: entite_id.to_string(),
            cle: cle.clone(),
            valeur: v.en_texte(),
        })
        .collect::<Vec<_>>()
        .into_iter()
}

impl Entite for Film {
    const TABLE: &'static str = "films";
    fn id(&self) -> &str {
        &self.id
    }
    fn definir_id(&mut self, id: String) {
        self.id = id;
    }
    fn titre(&self) -> &str {
        &self.titre
    }
    fn annee(&self) -> i64 {
        i64::from(self.annee)
    }
    fn chemin(&self) -> String {
        self.chemin.to_string_lossy().into_owned()
    }
    fn ids_indexes(&self) -> Vec<IdIndexe> {
        ids_de("film", &self.id, &self.ids).collect()
    }
}

impl Entite for Saga {
    const TABLE: &'static str = "sagas";
    fn id(&self) -> &str {
        &self.id
    }
    fn definir_id(&mut self, id: String) {
        self.id = id;
    }
    fn titre(&self) -> &str {
        &self.titre
    }
    fn ids_indexes(&self) -> Vec<IdIndexe> {
        ids_de("saga", &self.id, &self.ids).collect()
    }
}

impl Entite for Serie {
    const TABLE: &'static str = "series";
    fn id(&self) -> &str {
        &self.id
    }
    fn definir_id(&mut self, id: String) {
        self.id = id;
    }
    fn titre(&self) -> &str {
        &self.titre
    }
    fn annee(&self) -> i64 {
        i64::from(self.annee)
    }
    fn chemin(&self) -> String {
        self.chemin.to_string_lossy().into_owned()
    }
    fn preparer(&mut self) {
        for ep in self
            .episodes
            .iter_mut()
            .chain(self.episodes_fictifs.iter_mut())
        {
            if ep.id.is_empty() {
                ep.id = nouvel_id();
            }
            ep.id_serie = self.id.clone();
        }
    }
    fn ids_indexes(&self) -> Vec<IdIndexe> {
        let mut v: Vec<IdIndexe> = ids_de("serie", &self.id, &self.ids).collect();
        for ep in &self.episodes {
            v.extend(ids_de("episode", &ep.id, &ep.ids));
        }
        v
    }
}

/// Nouvel identifiant interne (UUID v4, forme canonique).
pub fn nouvel_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn maintenant_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// ---------------------------------------------------------------------------
// Opérations communes (sur une connexion ou une transaction)
// ---------------------------------------------------------------------------

fn indexer_ids<E: Entite>(conn: &Connection, e: &E) -> Resultat<()> {
    conn.execute(
        "DELETE FROM ids_externes WHERE table_racine = ?1 AND racine_id = ?2",
        params![E::TABLE, e.id()],
    )?;
    let mut st = conn.prepare_cached(
        "INSERT OR REPLACE INTO ids_externes
             (table_racine, racine_id, type_entite, entite_id, cle, valeur)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for i in e.ids_indexes() {
        st.execute(params![
            E::TABLE,
            e.id(),
            i.type_entite,
            i.entite_id,
            i.cle,
            i.valeur
        ])?;
    }
    Ok(())
}

/// Insère ou met à jour ; renvoie `false` si le document stocké était
/// identique (aucune écriture).
fn enregistrer_dans<E: Entite>(conn: &Connection, e: &mut E) -> Resultat<bool> {
    if e.id().is_empty() {
        e.definir_id(nouvel_id());
    }
    e.preparer();
    let json = serde_json::to_string(e)?;
    let existant: Option<String> = conn
        .prepare_cached(&format!("SELECT json FROM {} WHERE id = ?1", E::TABLE))?
        .query_row([e.id()], |l| l.get(0))
        .optional()?;
    if existant.as_deref() == Some(json.as_str()) {
        return Ok(false);
    }
    conn.prepare_cached(&format!(
        "INSERT INTO {} (id, titre, annee, chemin, json, modifie_le)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET titre = excluded.titre, annee = excluded.annee,
             chemin = excluded.chemin, json = excluded.json, modifie_le = excluded.modifie_le",
        E::TABLE
    ))?
    .execute(params![
        e.id(),
        e.titre(),
        e.annee(),
        e.chemin(),
        json,
        maintenant_ms()
    ])?;
    indexer_ids(conn, e)?;
    Ok(true)
}

fn supprimer_dans<E: Entite>(conn: &Connection, id: &str) -> Resultat<bool> {
    let n = conn.execute(&format!("DELETE FROM {} WHERE id = ?1", E::TABLE), [id])?;
    conn.execute(
        "DELETE FROM ids_externes WHERE table_racine = ?1 AND racine_id = ?2",
        params![E::TABLE, id],
    )?;
    Ok(n > 0)
}

fn decoder<E: Entite>(id: String, json: &str) -> Resultat<E> {
    let mut e: E = serde_json::from_str(json)?;
    e.definir_id(id);
    Ok(e)
}

/// Exécute une requête renvoyant `(id, json)` et décode les documents. Les
/// documents illisibles sont journalisés et ignorés (ils restent en base).
fn requete_docs<E: Entite>(
    conn: &Connection,
    sql: &str,
    p: impl rusqlite::Params,
) -> Resultat<Vec<E>> {
    let mut st = conn.prepare_cached(sql)?;
    let lignes = st.query_map(p, |l| Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?)))?;
    let mut sortie = Vec::new();
    for l in lignes {
        let (id, json) = l?;
        match decoder(id.clone(), &json) {
            Ok(e) => sortie.push(e),
            Err(err) => tracing::warn!("{} {id} illisible, ignoré : {err}", E::TABLE),
        }
    }
    Ok(sortie)
}

// ---------------------------------------------------------------------------
// Bibliothèque
// ---------------------------------------------------------------------------

/// Résultat d'une recherche d'épisode par identifiant externe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceEpisode {
    pub serie_id: String,
    pub episode_id: String,
}

/// Accès à la base de la médiathèque. Non `Sync` : à partager derrière un
/// `Mutex` si plusieurs fils d'exécution y accèdent.
pub struct Bibliotheque {
    conn: Connection,
}

impl std::fmt::Debug for Bibliotheque {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bibliotheque")
            .field("chemin", &self.conn.path())
            .finish()
    }
}

impl Bibliotheque {
    /// Ouvre (ou crée) la base, vérifie son intégrité et applique les
    /// migrations. Une base illisible donne [`ErreurStockage::BaseCorrompue`]
    /// (voir [`crate::sauvegarde::ouvrir_ou_restaurer`] pour la reprise).
    pub fn ouvrir(chemin: &Path) -> Resultat<Self> {
        if let Some(parent) = chemin.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ErreurStockage::io(parent, e))?;
        }
        let conn = Connection::open(chemin)?;
        Self::initialiser(conn).map_err(qualifier_corruption)
    }

    /// Base en mémoire (tests, essais).
    pub fn ouvrir_en_memoire() -> Resultat<Self> {
        Self::initialiser(Connection::open_in_memory()?)
    }

    fn initialiser(conn: Connection) -> Resultat<Self> {
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        verifier_integrite(&conn)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let mut b = Self { conn };
        b.migrer_jusqua(VERSION_SCHEMA)?;
        Ok(b)
    }

    /// Applique les migrations jusqu'à `cible` incluse.
    pub(crate) fn migrer_jusqua(&mut self, cible: i64) -> Resultat<()> {
        let actuelle = self.version_schema()?;
        if actuelle > VERSION_SCHEMA {
            return Err(ErreurStockage::SchemaTropRecent {
                trouvee: actuelle,
                maximale: VERSION_SCHEMA,
            });
        }
        for version in (actuelle + 1)..=cible.min(VERSION_SCHEMA) {
            let tx = self.conn.transaction()?;
            MIGRATIONS[(version - 1) as usize](&tx)?;
            tx.pragma_update(None, "user_version", version)?;
            tx.commit()?;
            tracing::info!("schéma de la base migré en version {version}");
        }
        Ok(())
    }

    /// Ouvre sans migrer (tests de migration).
    #[cfg(test)]
    pub(crate) fn ouvrir_brut(chemin: &Path) -> Resultat<Self> {
        Ok(Self {
            conn: Connection::open(chemin)?,
        })
    }

    /// Connexion brute (tests).
    #[cfg(test)]
    pub(crate) fn connexion(&self) -> &Connection {
        &self.conn
    }

    /// Version du schéma (`PRAGMA user_version`).
    pub fn version_schema(&self) -> Resultat<i64> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |l| l.get(0))?)
    }

    /// Insère ou met à jour une entité ; attribue un UUID si `id` est vide.
    /// Renvoie `false` si rien n'a changé (document identique).
    pub fn enregistrer<E: Entite>(&mut self, e: &mut E) -> Resultat<bool> {
        let tx = self.conn.transaction()?;
        let modifie = enregistrer_dans(&tx, e)?;
        tx.commit()?;
        Ok(modifie)
    }

    /// Enregistre un lot d'entités dans **une seule transaction** (tout ou
    /// rien). Renvoie le nombre d'entités réellement modifiées.
    pub fn enregistrer_lot<E: Entite>(&mut self, entites: &mut [E]) -> Resultat<usize> {
        let tx = self.conn.transaction()?;
        let mut n = 0;
        for e in entites.iter_mut() {
            if enregistrer_dans(&tx, e)? {
                n += 1;
            }
        }
        tx.commit()?;
        Ok(n)
    }

    /// Ouvre un lot mixte (films, sagas, séries) validé par [`Lot::valider`] ;
    /// abandonné s'il est détruit sans validation.
    pub fn lot(&mut self) -> Resultat<Lot<'_>> {
        Ok(Lot {
            tx: self.conn.transaction()?,
        })
    }

    /// Entité par identifiant interne.
    pub fn obtenir<E: Entite>(&self, id: &str) -> Resultat<Option<E>> {
        let ligne: Option<String> = self
            .conn
            .prepare_cached(&format!("SELECT json FROM {} WHERE id = ?1", E::TABLE))?
            .query_row([id], |l| l.get(0))
            .optional()?;
        ligne.map(|json| decoder(id.to_string(), &json)).transpose()
    }

    /// Supprime une entité ; renvoie `true` si elle existait.
    pub fn supprimer<E: Entite>(&mut self, id: &str) -> Resultat<bool> {
        let tx = self.conn.transaction()?;
        let r = supprimer_dans::<E>(&tx, id)?;
        tx.commit()?;
        Ok(r)
    }

    /// Toutes les entités d'un type, triées par titre.
    pub fn lister<E: Entite>(&self) -> Resultat<Vec<E>> {
        requete_docs(
            &self.conn,
            &format!(
                "SELECT id, json FROM {} ORDER BY titre COLLATE NOCASE, id",
                E::TABLE
            ),
            [],
        )
    }

    /// Nombre d'entités d'un type.
    pub fn compter<E: Entite>(&self) -> Resultat<u64> {
        Ok(self
            .conn
            .query_row(&format!("SELECT COUNT(*) FROM {}", E::TABLE), [], |l| {
                l.get::<_, i64>(0)
            })? as u64)
    }

    /// Entités dont le dossier est exactement `chemin` (plusieurs films
    /// peuvent partager un dossier).
    pub fn chercher_par_chemin<E: Entite>(&self, chemin: &Path) -> Resultat<Vec<E>> {
        requete_docs(
            &self.conn,
            &format!(
                "SELECT id, json FROM {} WHERE chemin = ?1 ORDER BY id",
                E::TABLE
            ),
            [chemin.to_string_lossy()],
        )
    }

    /// Entités (racines) portant l'identifiant externe `cle` = `valeur`
    /// (comparaison textuelle : `603` et `"603"` sont équivalents).
    pub fn chercher_par_id_externe<E: Entite>(&self, cle: &str, valeur: &str) -> Resultat<Vec<E>> {
        requete_docs(
            &self.conn,
            &format!(
                "SELECT t.id, t.json FROM {t} t
                 WHERE t.id IN (SELECT racine_id FROM ids_externes
                                WHERE table_racine = ?1 AND entite_id = racine_id
                                  AND cle = ?2 AND valeur = ?3)
                 ORDER BY t.id",
                t = E::TABLE
            ),
            params![E::TABLE, cle, valeur.trim()],
        )
    }

    /// Épisodes portant l'identifiant externe `cle` = `valeur`.
    pub fn chercher_episodes_par_id_externe(
        &self,
        cle: &str,
        valeur: &str,
    ) -> Resultat<Vec<ReferenceEpisode>> {
        let mut st = self.conn.prepare_cached(
            "SELECT racine_id, entite_id FROM ids_externes
             WHERE type_entite = 'episode' AND cle = ?1 AND valeur = ?2
             ORDER BY racine_id, entite_id",
        )?;
        let v = st
            .query_map(params![cle, valeur.trim()], |l| {
                Ok(ReferenceEpisode {
                    serie_id: l.get(0)?,
                    episode_id: l.get(1)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(v)
    }

    /// Recherche par titre (sous-chaîne, insensible à la casse ASCII).
    pub fn chercher_par_titre<E: Entite>(&self, motif: &str) -> Resultat<Vec<E>> {
        let echappe = motif
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        requete_docs(
            &self.conn,
            &format!(
                "SELECT id, json FROM {} WHERE titre LIKE ?1 ESCAPE '\\'
                 ORDER BY titre COLLATE NOCASE, id",
                E::TABLE
            ),
            [format!("%{echappe}%")],
        )
    }

    /// Valeur du petit magasin clé/valeur (horodatages, états divers ; pas
    /// de secrets : voir [`crate::secrets`]).
    pub fn lire_valeur(&self, cle: &str) -> Resultat<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT valeur FROM valeurs WHERE cle = ?1", [cle], |l| {
                l.get(0)
            })
            .optional()?)
    }

    /// Écrit une valeur du magasin clé/valeur.
    pub fn ecrire_valeur(&mut self, cle: &str, valeur: &str) -> Resultat<()> {
        self.conn.execute(
            "INSERT INTO valeurs (cle, valeur) VALUES (?1, ?2)
             ON CONFLICT(cle) DO UPDATE SET valeur = excluded.valeur",
            params![cle, valeur],
        )?;
        Ok(())
    }

    /// Supprime une valeur du magasin clé/valeur.
    pub fn supprimer_valeur(&mut self, cle: &str) -> Resultat<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM valeurs WHERE cle = ?1", [cle])?
            > 0)
    }

    /// Copie cohérente de la base vers `cible` (`VACUUM INTO`), utilisable
    /// pendant que la base est ouverte.
    pub fn copier_vers(&self, cible: &Path) -> Resultat<()> {
        copier_base(&self.conn, cible)
    }

    /// Ferme la base proprement (point de contrôle du journal WAL).
    pub fn fermer(self) -> Resultat<()> {
        let _ = self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        self.conn.close().map_err(|(_, e)| e.into())
    }
}

/// Lot d'opérations dans une transaction unique.
pub struct Lot<'a> {
    tx: Transaction<'a>,
}

impl Lot<'_> {
    /// Voir [`Bibliotheque::enregistrer`].
    pub fn enregistrer<E: Entite>(&mut self, e: &mut E) -> Resultat<bool> {
        enregistrer_dans(&self.tx, e)
    }

    /// Voir [`Bibliotheque::supprimer`].
    pub fn supprimer<E: Entite>(&mut self, id: &str) -> Resultat<bool> {
        supprimer_dans::<E>(&self.tx, id)
    }

    /// Valide toutes les opérations du lot.
    pub fn valider(self) -> Resultat<()> {
        self.tx.commit()?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Intégrité et copies
// ---------------------------------------------------------------------------

/// `PRAGMA quick_check` ; erreur [`ErreurStockage::BaseCorrompue`] si KO.
pub(crate) fn verifier_integrite(conn: &Connection) -> Resultat<()> {
    let r: String = conn
        .query_row("PRAGMA quick_check", [], |l| l.get(0))
        .map_err(|e| qualifier_corruption(e.into()))?;
    if r == "ok" {
        Ok(())
    } else {
        Err(ErreurStockage::BaseCorrompue(r))
    }
}

/// Convertit les erreurs SQLite « pas une base » / « corrompue » en
/// [`ErreurStockage::BaseCorrompue`].
pub(crate) fn qualifier_corruption(e: ErreurStockage) -> ErreurStockage {
    use rusqlite::ffi::ErrorCode;
    match &e {
        ErreurStockage::Sqlite(rusqlite::Error::SqliteFailure(f, msg))
            if matches!(f.code, ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt) =>
        {
            ErreurStockage::BaseCorrompue(msg.clone().unwrap_or_else(|| f.to_string()))
        }
        _ => e,
    }
}

pub(crate) fn copier_base(conn: &Connection, cible: &Path) -> Resultat<()> {
    if cible.exists() {
        std::fs::remove_file(cible).map_err(|e| ErreurStockage::io(cible, e))?;
    }
    conn.execute("VACUUM INTO ?1", [cible.to_string_lossy()])?;
    Ok(())
}
