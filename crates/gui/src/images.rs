//! Images : décodage asynchrone de fichiers locaux (crate `image`) sur des threads dédiés,
//! textures egui conservées dans deux caches LRU bornés en octets (miniatures et pleine taille).

use crate::taches::Repaint;
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

/// Côté maximal d'une miniature (au-delà : cache « pleine taille », visionneuse).
pub const TAILLE_MINIATURE: u32 = 512;
/// Côté maximal d'une image pleine taille (visionneuse).
pub const TAILLE_PLEINE: u32 = 2048;
const PLAFOND_MINIATURES: usize = 96 * 1024 * 1024;
const PLAFOND_PLEINES: usize = 192 * 1024 * 1024;
// Les adresses distantes bloquent un fil pendant le téléchargement : quelques fils de plus.
const NB_DECODEURS: usize = 4;

/// Cache LRU générique borné par un coût total (octets). Éviction en O(n) : suffisant pour
/// quelques centaines d'entrées.
#[derive(Debug)]
pub struct CacheLru<K, V> {
    plafond: usize,
    cout_total: usize,
    horloge: u64,
    entrees: HashMap<K, (V, usize, u64)>,
}

impl<K: Hash + Eq + Clone, V> CacheLru<K, V> {
    pub fn new(plafond: usize) -> Self {
        Self {
            plafond,
            cout_total: 0,
            horloge: 0,
            entrees: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entrees.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entrees.is_empty()
    }

    pub fn cout_total(&self) -> usize {
        self.cout_total
    }

    pub fn contient(&self, cle: &K) -> bool {
        self.entrees.contains_key(cle)
    }

    /// Lecture qui rafraîchit l'ancienneté de l'entrée.
    pub fn obtenir(&mut self, cle: &K) -> Option<&V> {
        self.horloge += 1;
        let h = self.horloge;
        self.entrees.get_mut(cle).map(|e| {
            e.2 = h;
            &e.0
        })
    }

    /// Insère (ou remplace) et renvoie les clés évincées. Une entrée plus grosse que le
    /// plafond reste seule dans le cache.
    pub fn inserer(&mut self, cle: K, valeur: V, cout: usize) -> Vec<K> {
        self.horloge += 1;
        if let Some((_, ancien, _)) = self.entrees.remove(&cle) {
            self.cout_total -= ancien;
        }
        let mut evinces = Vec::new();
        while !self.entrees.is_empty() && self.cout_total + cout > self.plafond {
            let plus_vieille = self
                .entrees
                .iter()
                .min_by_key(|(_, (_, _, h))| *h)
                .map(|(k, _)| k.clone());
            if let Some(k) = plus_vieille {
                if let Some((_, c, _)) = self.entrees.remove(&k) {
                    self.cout_total -= c;
                }
                evinces.push(k);
            }
        }
        self.cout_total += cout;
        self.entrees.insert(cle, (valeur, cout, self.horloge));
        evinces
    }

    pub fn vider(&mut self) {
        self.entrees.clear();
        self.cout_total = 0;
    }
}

/// Clé d'une image : fichier + côté maximal demandé.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CleImage {
    pub chemin: PathBuf,
    pub taille_max: u32,
}

impl CleImage {
    pub fn miniature(chemin: impl Into<PathBuf>) -> Self {
        Self {
            chemin: chemin.into(),
            taille_max: TAILLE_MINIATURE,
        }
    }

    pub fn pleine(chemin: impl Into<PathBuf>) -> Self {
        Self {
            chemin: chemin.into(),
            taille_max: TAILLE_PLEINE,
        }
    }
}

/// Taille maximale acceptée pour une image téléchargée (octets).
const PLAFOND_TELECHARGEMENT: u64 = 20 * 1024 * 1024;

/// `true` si le « chemin » est en réalité une adresse `http(s)://`.
pub fn est_url(chemin: &Path) -> bool {
    let t = chemin.to_string_lossy();
    t.starts_with("http://") || t.starts_with("https://")
}

/// Dossier de cache disque des images téléchargées.
pub fn dossier_cache_distant() -> PathBuf {
    std::env::temp_dir().join("mediamanager-cache-images")
}

/// Nom de fichier de cache (hachage FNV-1a 64 bits de l'adresse, stable d'une exécution à l'autre).
pub fn nom_cache(url: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for o in url.bytes() {
        h ^= u64::from(o);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}.img")
}

fn client_http() -> Result<&'static reqwest::blocking::Client, String> {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<Result<reqwest::blocking::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::blocking::Client::builder()
                .user_agent(concat!("MediaManagerRust/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(std::time::Duration::from_secs(8))
                .timeout(std::time::Duration::from_secs(25))
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| e.clone())
}

/// Télécharge `url` dans le cache disque `dossier` (sans re-télécharger si déjà présent) et
/// renvoie le chemin du fichier mis en cache.
pub fn telecharger_dans_cache(url: &str, dossier: &Path) -> Result<PathBuf, String> {
    use std::io::Read;
    let cible = dossier.join(nom_cache(url));
    if std::fs::metadata(&cible).is_ok_and(|m| m.len() > 0) {
        return Ok(cible);
    }
    std::fs::create_dir_all(dossier).map_err(|e| format!("{}: {e}", dossier.display()))?;
    let reponse = client_http()?
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| format!("{url}: {e}"))?;
    if reponse
        .content_length()
        .is_some_and(|l| l > PLAFOND_TELECHARGEMENT)
    {
        return Err(format!("{url}: image trop volumineuse"));
    }
    let mut octets = Vec::new();
    reponse
        .take(PLAFOND_TELECHARGEMENT + 1)
        .read_to_end(&mut octets)
        .map_err(|e| format!("{url}: {e}"))?;
    if octets.is_empty() || octets.len() as u64 > PLAFOND_TELECHARGEMENT {
        return Err(format!("{url}: taille de réponse invalide"));
    }
    // Vérifie que c'est bien une image avant de la garder en cache.
    image::guess_format(&octets).map_err(|e| format!("{url}: {e}"))?;
    let temporaire = cible.with_extension(format!("part{}", std::process::id()));
    std::fs::write(&temporaire, &octets).map_err(|e| e.to_string())?;
    std::fs::rename(&temporaire, &cible).map_err(|e| e.to_string())?;
    Ok(cible)
}

/// Décode un fichier image (ou une adresse `http(s)://`, téléchargée dans le cache disque) et
/// le réduit (proportions conservées) à `taille_max` de côté.
pub fn decoder(chemin: &Path, taille_max: u32) -> Result<ColorImage, String> {
    if est_url(chemin) {
        let local = telecharger_dans_cache(&chemin.to_string_lossy(), &dossier_cache_distant())?;
        return decoder_fichier(&local, taille_max);
    }
    decoder_fichier(chemin, taille_max)
}

fn decoder_fichier(chemin: &Path, taille_max: u32) -> Result<ColorImage, String> {
    let octets = std::fs::read(chemin).map_err(|e| format!("{}: {e}", chemin.display()))?;
    let img = image::load_from_memory(&octets).map_err(|e| format!("{}: {e}", chemin.display()))?;
    let img = if img.width() > taille_max || img.height() > taille_max {
        img.thumbnail(taille_max, taille_max)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    let taille = [rgba.width() as usize, rgba.height() as usize];
    Ok(ColorImage::from_rgba_unmultiplied(taille, rgba.as_raw()))
}

pub type ResultatDecodage = (CleImage, Result<ColorImage, String>);

/// Pool de threads de décodage : reçoit des clés, renvoie des `ColorImage`.
pub struct Chargeur {
    tx_requetes: Sender<CleImage>,
    rx_resultats: Receiver<ResultatDecodage>,
}

impl Chargeur {
    pub fn new(nb: usize, repaint: Repaint) -> Self {
        let (tx_requetes, rx_requetes) = channel::<CleImage>();
        let (tx_resultats, rx_resultats) = channel();
        let partage = Arc::new(Mutex::new(rx_requetes));
        for n in 0..nb.max(1) {
            let rx = partage.clone();
            let tx = tx_resultats.clone();
            let repaint = repaint.clone();
            let _ = thread::Builder::new()
                .name(format!("decodeur-images-{n}"))
                .spawn(move || {
                    loop {
                        let cle = {
                            let Ok(verrou) = rx.lock() else { return };
                            match verrou.recv() {
                                Ok(c) => c,
                                Err(_) => return,
                            }
                        };
                        let resultat = decoder(&cle.chemin, cle.taille_max);
                        if tx.send((cle, resultat)).is_err() {
                            return;
                        }
                        repaint();
                    }
                });
        }
        Self {
            tx_requetes,
            rx_resultats,
        }
    }

    pub fn demander(&self, cle: CleImage) {
        let _ = self.tx_requetes.send(cle);
    }

    pub fn recevoir(&self) -> Option<ResultatDecodage> {
        self.rx_resultats.try_recv().ok()
    }

    pub fn recevoir_bloquant(&self, delai: std::time::Duration) -> Option<ResultatDecodage> {
        self.rx_resultats.recv_timeout(delai).ok()
    }
}

/// État d'une image demandée à la vue.
pub enum EtatImage {
    Prete(TextureHandle),
    EnCours,
    Erreur,
}

/// Gestionnaire possédé par l'application (hors de `Etat` : les textures exigent un `Context`).
pub struct GestionnaireImages {
    chargeur: Chargeur,
    miniatures: CacheLru<CleImage, TextureHandle>,
    pleines: CacheLru<CleImage, TextureHandle>,
    en_attente: HashSet<CleImage>,
    echecs: HashSet<CleImage>,
}

impl GestionnaireImages {
    pub fn new(repaint: Repaint) -> Self {
        Self {
            chargeur: Chargeur::new(NB_DECODEURS, repaint),
            miniatures: CacheLru::new(PLAFOND_MINIATURES),
            pleines: CacheLru::new(PLAFOND_PLEINES),
            en_attente: HashSet::new(),
            echecs: HashSet::new(),
        }
    }

    fn cache(&mut self, cle: &CleImage) -> &mut CacheLru<CleImage, TextureHandle> {
        if cle.taille_max <= TAILLE_MINIATURE {
            &mut self.miniatures
        } else {
            &mut self.pleines
        }
    }

    /// Transforme les images décodées en textures (à appeler une fois par image).
    pub fn recevoir(&mut self, ctx: &egui::Context) {
        while let Some((cle, resultat)) = self.chargeur.recevoir() {
            self.en_attente.remove(&cle);
            match resultat {
                Ok(img) => {
                    let cout = img.width() * img.height() * 4;
                    let nom = format!("img:{}:{}", cle.taille_max, cle.chemin.display());
                    let tex = ctx.load_texture(nom, img, TextureOptions::LINEAR);
                    self.cache(&cle).inserer(cle, tex, cout);
                }
                Err(_) => {
                    self.echecs.insert(cle);
                }
            }
        }
    }

    /// Texture prête, ou demande de chargement (placeholder immédiat côté vue).
    pub fn image(&mut self, cle: &CleImage) -> EtatImage {
        if let Some(t) = self.cache(cle).obtenir(cle) {
            return EtatImage::Prete(t.clone());
        }
        if self.echecs.contains(cle) {
            return EtatImage::Erreur;
        }
        if self.en_attente.insert(cle.clone()) {
            self.chargeur.demander(cle.clone());
        }
        EtatImage::EnCours
    }

    /// Vide les caches (commande « Vider le cache image »).
    pub fn vider(&mut self) {
        self.miniatures.vider();
        self.pleines.vider();
        self.echecs.clear();
    }

    pub fn statistiques(&self) -> (usize, usize) {
        (
            self.miniatures.len() + self.pleines.len(),
            self.miniatures.cout_total() + self.pleines.cout_total(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn lru_evince_la_plus_ancienne() {
        let mut c: CacheLru<&str, u8> = CacheLru::new(30);
        assert!(c.inserer("a", 1, 10).is_empty());
        c.inserer("b", 2, 10);
        c.inserer("c", 3, 10);
        // « a » est relu : « b » devient la plus ancienne
        assert_eq!(c.obtenir(&"a"), Some(&1));
        assert_eq!(c.inserer("d", 4, 10), vec!["b"]);
        assert!(c.contient(&"a") && !c.contient(&"b"));
        assert_eq!(c.cout_total(), 30);
    }

    #[test]
    fn lru_remplacement_et_entree_geante() {
        let mut c: CacheLru<u32, ()> = CacheLru::new(100);
        c.inserer(1, (), 40);
        c.inserer(1, (), 60);
        assert_eq!((c.len(), c.cout_total()), (1, 60));
        let evinces = c.inserer(2, (), 500);
        assert_eq!(evinces, vec![1]);
        assert_eq!((c.len(), c.cout_total()), (1, 500));
        c.vider();
        assert!(c.is_empty() && c.cout_total() == 0);
    }

    fn png_temporaire(nom: &str, l: u32, h: u32) -> PathBuf {
        let p = std::env::temp_dir().join(format!("mmr-img-{}-{nom}.png", std::process::id()));
        crate::source_donnees::generer_image_demo(&p, l, h, 7).unwrap();
        p
    }

    #[test]
    fn decodage_avec_reduction_proportionnelle() {
        let p = png_temporaire("reduc", 400, 600);
        let img = decoder(&p, 300).unwrap();
        assert_eq!(img.size, [200, 300]);
        let img = decoder(&p, 2048).unwrap();
        assert_eq!(img.size, [400, 600]);
        assert!(decoder(Path::new("/n/existe/pas.png"), 100).is_err());
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn url_reconnue_et_nom_de_cache_stable() {
        assert!(est_url(Path::new("https://exemple.org/a.jpg")));
        assert!(est_url(Path::new("http://exemple.org/a.jpg")));
        assert!(!est_url(Path::new("/tmp/a.jpg")));
        assert_eq!(nom_cache("https://a/b"), nom_cache("https://a/b"));
        assert_ne!(nom_cache("https://a/b"), nom_cache("https://a/c"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn telechargement_http_avec_cache_disque() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let src = png_temporaire("http", 80, 120);
        let octets = std::fs::read(&src).unwrap();
        let serveur = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/affiche.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(octets))
            .expect(1)
            .mount(&serveur)
            .await;
        Mock::given(method("GET"))
            .and(path("/absente.jpg"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&serveur)
            .await;
        let url = format!("{}/affiche.jpg", serveur.uri());
        let mauvaise = format!("{}/absente.jpg", serveur.uri());
        let cache = std::env::temp_dir().join(format!("mmr-cache-{}", std::process::id()));
        let (c2, u2, m2) = (cache.clone(), url.clone(), mauvaise.clone());
        tokio::task::spawn_blocking(move || {
            let f1 = telecharger_dans_cache(&u2, &c2).unwrap();
            // Deuxième appel : servi par le disque (le serveur n'attend qu'une requête).
            let f2 = telecharger_dans_cache(&u2, &c2).unwrap();
            assert_eq!(f1, f2);
            assert_eq!(decoder_fichier(&f1, 512).unwrap().size, [80, 120]);
            assert!(telecharger_dans_cache(&m2, &c2).is_err());
        })
        .await
        .unwrap();
        let _ = std::fs::remove_dir_all(cache);
        let _ = std::fs::remove_file(src);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn gestionnaire_charge_une_url_http_sans_bloquer() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let src = png_temporaire("gest", 50, 75);
        let octets = std::fs::read(&src).unwrap();
        let serveur = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/a.png"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_bytes(octets)
                    .set_delay(Duration::from_millis(300)),
            )
            .mount(&serveur)
            .await;
        let url = PathBuf::from(format!("{}/a.png", serveur.uri()));
        let ctx = egui::Context::default();
        let rendu = tokio::task::spawn_blocking(move || {
            let mut g = GestionnaireImages::new(Arc::new(|| {}));
            let cle = CleImage::miniature(&url);
            // Le premier appel rend la main immédiatement (chargement en tâche de fond).
            let debut = std::time::Instant::now();
            assert!(matches!(g.image(&cle), EtatImage::EnCours));
            assert!(debut.elapsed() < Duration::from_millis(250));
            for _ in 0..200 {
                g.recevoir(&ctx);
                if let EtatImage::Prete(t) = g.image(&cle) {
                    return t.size();
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            panic!("image distante jamais chargée");
        })
        .await
        .unwrap();
        assert_eq!(rendu, [50, 75]);
        let _ = std::fs::remove_file(src);
    }

    #[test]
    fn chargeur_asynchrone() {
        let p = png_temporaire("async", 64, 32);
        let ch = Chargeur::new(2, Arc::new(|| {}));
        ch.demander(CleImage::miniature(&p));
        ch.demander(CleImage::miniature("/n/existe/pas.png"));
        let mut ok = 0;
        let mut ko = 0;
        for _ in 0..2 {
            let (_, r) = ch.recevoir_bloquant(Duration::from_secs(10)).unwrap();
            match r {
                Ok(img) => {
                    assert_eq!(img.size, [64, 32]);
                    ok += 1
                }
                Err(_) => ko += 1,
            }
        }
        assert_eq!((ok, ko), (1, 1));
        let _ = std::fs::remove_file(p);
    }
}
