//! Structures d'affichage de la GUI (indépendantes du modèle des services : la conversion
//! est faite par `conversion.rs`).

use std::cmp::Ordering;

/// Type de fichier associé à un film (aperçu du renommage, onglet « Fichiers médias »).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TypeFichier {
    Video,
    Nfo,
    SousTitres,
    Affiche,
    Fanart,
    Banniere,
    Miniature,
    BandeAnnonce,
    #[default]
    Autre,
}

impl TypeFichier {
    pub fn libelle(self) -> &'static str {
        match self {
            TypeFichier::Video => "VIDEO",
            TypeFichier::Nfo => "NFO",
            TypeFichier::SousTitres => "SUBTITLE",
            TypeFichier::Affiche => "POSTER",
            TypeFichier::Fanart => "FANART",
            TypeFichier::Banniere => "BANNER",
            TypeFichier::Miniature => "THUMB",
            TypeFichier::BandeAnnonce => "TRAILER",
            TypeFichier::Autre => "OTHER",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FichierFilm {
    pub type_fichier: TypeFichier,
    pub nom: String,
    /// Suffixe conservé au renommage (`.mkv`, `.fr.srt`...). `None` : fichier non renommé.
    pub suffixe: Option<String>,
    pub taille_octets: u64,
    /// Chemin complet du fichier (vide si inconnu : il est alors déduit du dossier du film).
    pub chemin: String,
    /// Codec vidéo (vide si l'analyse des flux n'a pas eu lieu).
    pub codec: String,
    /// Résolution `LxH` (vide si inconnue).
    pub resolution: String,
    /// Pistes audio résumées (`AC3 6ch fra, ...`).
    pub audio: String,
    /// Langues des sous-titres internes.
    pub sous_titres: String,
}

/// Extensions reconnues comme images (onglet « Illustration »).
pub const EXTENSIONS_IMAGES: [&str; 7] = ["jpg", "jpeg", "png", "webp", "bmp", "gif", "tbn"];

impl FichierFilm {
    /// Vrai si le fichier est une image (d'après son extension).
    pub fn est_image(&self) -> bool {
        self.nom
            .rsplit_once('.')
            .is_some_and(|(_, ext)| EXTENSIONS_IMAGES.contains(&ext.to_lowercase().as_str()))
    }
}

/// Bande-annonce affichée (onglet « Bande annonce »).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BandeAnnonceFilm {
    pub nom: String,
    pub source: String,
    pub qualite: String,
    /// Adresse (web ou fichier local) ; vide si inconnue.
    pub url: String,
}

impl BandeAnnonceFilm {
    /// Libellé : nom (ou URL) suivi de la source et de la qualité entre parenthèses.
    pub fn libelle(&self) -> String {
        let nom = if self.nom.is_empty() {
            &self.url
        } else {
            &self.nom
        };
        let details: Vec<&str> = [self.source.as_str(), self.qualite.as_str()]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect();
        if details.is_empty() {
            nom.clone()
        } else {
            format!("{nom} ({})", details.join(", "))
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Film {
    pub id: u64,
    pub titre: String,
    pub titre_original: String,
    pub titre_triable: String,
    pub annee: u32,
    pub date_sortie: String,
    pub saga: Option<String>,
    pub note: f32,
    pub votes: u32,
    pub notes_sources: Vec<(String, f32)>,
    pub certification: String,
    pub duree_min: u32,
    pub genres: Vec<String>,
    pub realisateurs: Vec<String>,
    pub scenaristes: Vec<String>,
    pub pays: Vec<String>,
    pub production: Vec<String>,
    pub slogan: String,
    pub resume: String,
    pub imdb: String,
    pub tmdb: String,
    pub source: String,
    pub dossier: String,
    pub nom_fichier: String,
    pub hauteur_video: u32,
    pub codec_video: String,
    pub hdr: Option<String>,
    pub audio: String,
    pub taille_octets: u64,
    pub date_ajout: String,
    pub nouveau: bool,
    pub verrouille: bool,
    pub metadonnees_ok: bool,
    pub images_ok: bool,
    pub bande_annonce: bool,
    pub sous_titres: bool,
    pub vu: bool,
    pub acteurs: Vec<(String, String)>,
    pub fichiers: Vec<FichierFilm>,
    pub bandes_annonces: Vec<BandeAnnonceFilm>,
}

impl Film {
    /// Chemin affiché : source + dossier relatif.
    pub fn chemin(&self) -> String {
        if self.dossier.is_empty() {
            self.source.clone()
        } else if self.source.is_empty() {
            self.dossier.clone()
        } else {
            std::path::Path::new(&self.source)
                .join(&self.dossier)
                .display()
                .to_string()
        }
    }

    /// Chemin complet du fichier vidéo principal (`None` si le film n'a pas de vidéo).
    pub fn chemin_video(&self) -> Option<std::path::PathBuf> {
        if self.nom_fichier.is_empty() {
            return None;
        }
        self.fichiers
            .iter()
            .find(|f| f.type_fichier == TypeFichier::Video && f.nom == self.nom_fichier)
            .filter(|f| !f.chemin.is_empty())
            .map(|f| std::path::PathBuf::from(&f.chemin))
            .or_else(|| Some(std::path::Path::new(&self.chemin()).join(&self.nom_fichier)))
    }

    /// Chemin complet d'un fichier du film.
    pub fn chemin_fichier(&self, f: &FichierFilm) -> std::path::PathBuf {
        if f.chemin.is_empty() {
            std::path::Path::new(&self.chemin()).join(&f.nom)
        } else {
            std::path::PathBuf::from(&f.chemin)
        }
    }

    /// Libellé de format vidéo (480p, 720p, 1080p, 2160p...).
    pub fn format_video(&self) -> String {
        if self.hauteur_video == 0 {
            String::new()
        } else {
            format!("{}p", self.hauteur_video)
        }
    }

    /// Texte concaténé et normalisé sur lequel porte la recherche.
    pub fn texte_recherche(&self) -> String {
        let mut t = String::new();
        let mut ajouter = |s: &str| {
            t.push_str(s);
            t.push('\n');
        };
        ajouter(&self.titre);
        ajouter(&self.titre_original);
        ajouter(&self.slogan);
        ajouter(&self.resume);
        ajouter(&self.nom_fichier);
        for liste in [
            &self.genres,
            &self.realisateurs,
            &self.pays,
            &self.production,
        ] {
            for s in liste {
                ajouter(s);
            }
        }
        for (nom, _) in &self.acteurs {
            ajouter(nom);
        }
        normaliser(&t)
    }
}

/// Minuscules + suppression des accents latins usuels (équivalent de la normalisation de l'original).
pub fn normaliser(texte: &str) -> String {
    let mut s = String::with_capacity(texte.len());
    for c in texte.chars().flat_map(char::to_lowercase) {
        match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => s.push('a'),
            'ç' => s.push('c'),
            'è' | 'é' | 'ê' | 'ë' => s.push('e'),
            'ì' | 'í' | 'î' | 'ï' => s.push('i'),
            'ñ' => s.push('n'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => s.push('o'),
            'ù' | 'ú' | 'û' | 'ü' => s.push('u'),
            'ý' | 'ÿ' => s.push('y'),
            'œ' => s.push_str("oe"),
            'æ' => s.push_str("ae"),
            'ß' => s.push_str("ss"),
            autre => s.push(autre),
        }
    }
    s
}

/// Identifiants de colonnes du tableau Films (ceux de la spécification).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Colonne {
    Titre,
    TitreOriginal,
    Annee,
    DateSortie,
    Saga,
    Note,
    Votes,
    Certification,
    DateAjout,
    Duree,
    FormatVideo,
    CodecVideo,
    Audio,
    TailleFichier,
    Chemin,
    NomFichier,
    Nouveau,
    Metadonnees,
    Images,
    BandeAnnonce,
    SousTitres,
    Vu,
}

impl Colonne {
    pub const TOUTES: [Colonne; 22] = [
        Colonne::Titre,
        Colonne::TitreOriginal,
        Colonne::Annee,
        Colonne::DateSortie,
        Colonne::Saga,
        Colonne::Note,
        Colonne::Votes,
        Colonne::Certification,
        Colonne::DateAjout,
        Colonne::Duree,
        Colonne::FormatVideo,
        Colonne::CodecVideo,
        Colonne::Audio,
        Colonne::TailleFichier,
        Colonne::Chemin,
        Colonne::NomFichier,
        Colonne::Nouveau,
        Colonne::Metadonnees,
        Colonne::Images,
        Colonne::BandeAnnonce,
        Colonne::SousTitres,
        Colonne::Vu,
    ];

    /// Identifiant stable (clé de persistance), repris de la spécification.
    pub fn id(self) -> &'static str {
        match self {
            Colonne::Titre => "title",
            Colonne::TitreOriginal => "originalTitle",
            Colonne::Annee => "year",
            Colonne::DateSortie => "releaseDate",
            Colonne::Saga => "movieset",
            Colonne::Note => "rating",
            Colonne::Votes => "votes",
            Colonne::Certification => "certification",
            Colonne::DateAjout => "dateAdded",
            Colonne::Duree => "runtime",
            Colonne::FormatVideo => "videoFormat",
            Colonne::CodecVideo => "videoCodec",
            Colonne::Audio => "audio",
            Colonne::TailleFichier => "fileSize",
            Colonne::Chemin => "path",
            Colonne::NomFichier => "filename",
            Colonne::Nouveau => "new",
            Colonne::Metadonnees => "metadata",
            Colonne::Images => "images",
            Colonne::BandeAnnonce => "trailer",
            Colonne::SousTitres => "subtitles",
            Colonne::Vu => "watched",
        }
    }

    pub fn cle_i18n(self) -> &'static str {
        match self {
            Colonne::Titre => "metatag.title",
            Colonne::TitreOriginal => "metatag.originaltitle",
            Colonne::Annee => "metatag.year",
            Colonne::DateSortie => "metatag.releasedate",
            Colonne::Saga => "metatag.movieset",
            Colonne::Note => "metatag.rating",
            Colonne::Votes => "metatag.votes",
            Colonne::Certification => "metatag.certification",
            Colonne::DateAjout => "metatag.dateadded",
            Colonne::Duree => "metatag.runtime",
            Colonne::FormatVideo => "metatag.videoformat",
            Colonne::CodecVideo => "metatag.videocodec",
            Colonne::Audio => "metatag.audio",
            Colonne::TailleFichier => "metatag.filesize",
            Colonne::Chemin => "metatag.path",
            Colonne::NomFichier => "metatag.filename",
            Colonne::Nouveau => "column.new",
            Colonne::Metadonnees => "column.metadata",
            Colonne::Images => "column.images",
            Colonne::BandeAnnonce => "metatag.trailer",
            Colonne::SousTitres => "metatag.subtitles",
            Colonne::Vu => "metatag.watched",
        }
    }

    /// Visibles par défaut : title, year, rating, new, metadata, images, trailer, subtitles, watched.
    pub fn visible_par_defaut(self) -> bool {
        matches!(
            self,
            Colonne::Titre
                | Colonne::Annee
                | Colonne::Note
                | Colonne::Nouveau
                | Colonne::Metadonnees
                | Colonne::Images
                | Colonne::BandeAnnonce
                | Colonne::SousTitres
                | Colonne::Vu
        )
    }

    /// Colonnes à coche / icône (largeur fixe, en-tête abrégé).
    pub fn est_booleenne(self) -> bool {
        matches!(
            self,
            Colonne::Nouveau
                | Colonne::Metadonnees
                | Colonne::Images
                | Colonne::BandeAnnonce
                | Colonne::SousTitres
                | Colonne::Vu
        )
    }

    pub fn entete_court(self) -> Option<&'static str> {
        match self {
            Colonne::Nouveau => Some("N"),
            Colonne::Metadonnees => Some("M"),
            Colonne::Images => Some("I"),
            Colonne::BandeAnnonce => Some("B"),
            Colonne::SousTitres => Some("ST"),
            Colonne::Vu => Some("V"),
            _ => None,
        }
    }

    pub fn aligne_a_droite(self) -> bool {
        matches!(
            self,
            Colonne::Annee
                | Colonne::Note
                | Colonne::Votes
                | Colonne::Duree
                | Colonne::TailleFichier
        )
    }

    pub fn depuis_id(id: &str) -> Option<Colonne> {
        Colonne::TOUTES.into_iter().find(|c| c.id() == id)
    }

    pub fn largeur_initiale(self) -> f32 {
        match self {
            Colonne::Titre | Colonne::TitreOriginal => 240.0,
            Colonne::Chemin | Colonne::NomFichier => 260.0,
            Colonne::Saga | Colonne::Audio => 140.0,
            Colonne::DateSortie | Colonne::DateAjout => 110.0,
            c if c.est_booleenne() => 30.0,
            _ => 70.0,
        }
    }

    /// Comparateur de la colonne (titre triable, résolution numérique, taille numérique...).
    pub fn comparer(self, a: &Film, b: &Film) -> Ordering {
        match self {
            Colonne::Titre => normaliser(&a.titre_triable).cmp(&normaliser(&b.titre_triable)),
            Colonne::TitreOriginal => {
                normaliser(&a.titre_original).cmp(&normaliser(&b.titre_original))
            }
            Colonne::Annee => a.annee.cmp(&b.annee),
            Colonne::DateSortie => a.date_sortie.cmp(&b.date_sortie),
            Colonne::Saga => a.saga.cmp(&b.saga),
            Colonne::Note => a.note.total_cmp(&b.note),
            Colonne::Votes => a.votes.cmp(&b.votes),
            Colonne::Certification => a.certification.cmp(&b.certification),
            Colonne::DateAjout => a.date_ajout.cmp(&b.date_ajout),
            Colonne::Duree => a.duree_min.cmp(&b.duree_min),
            Colonne::FormatVideo => a.hauteur_video.cmp(&b.hauteur_video),
            Colonne::CodecVideo => a.codec_video.cmp(&b.codec_video),
            Colonne::Audio => a.audio.cmp(&b.audio),
            Colonne::TailleFichier => a.taille_octets.cmp(&b.taille_octets),
            Colonne::Chemin => normaliser(&a.chemin()).cmp(&normaliser(&b.chemin())),
            Colonne::NomFichier => normaliser(&a.nom_fichier).cmp(&normaliser(&b.nom_fichier)),
            Colonne::Nouveau => a.nouveau.cmp(&b.nouveau),
            Colonne::Metadonnees => a.metadonnees_ok.cmp(&b.metadonnees_ok),
            Colonne::Images => a.images_ok.cmp(&b.images_ok),
            Colonne::BandeAnnonce => a.bande_annonce.cmp(&b.bande_annonce),
            Colonne::SousTitres => a.sous_titres.cmp(&b.sous_titres),
            Colonne::Vu => a.vu.cmp(&b.vu),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ordre {
    Croissant,
    Decroissant,
}

impl Ordre {
    pub fn inverse(self) -> Self {
        match self {
            Ordre::Croissant => Ordre::Decroissant,
            Ordre::Decroissant => Ordre::Croissant,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColonneEtat {
    pub colonne: Colonne,
    pub visible: bool,
    /// Largeur mémorisée (redimensionnement par l'utilisateur) ; `None` : largeur initiale.
    pub largeur: Option<f32>,
}

/// État initial des colonnes : toutes présentes, visibilité par défaut de la spécification.
pub fn colonnes_par_defaut() -> Vec<ColonneEtat> {
    Colonne::TOUTES
        .iter()
        .map(|&c| ColonneEtat {
            colonne: c,
            visible: c.visible_par_defaut(),
            largeur: None,
        })
        .collect()
}

// ------------------------------------------------------------------ colonnes du tableau Séries

/// Colonnes du tableau Séries (arbre série > saison > épisode).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ColonneSerie {
    Titre,
    TitreOriginal,
    Annee,
    Statut,
    Note,
    Saisons,
    Episodes,
    Manquants,
    Chemin,
    Vu,
}

impl ColonneSerie {
    pub const TOUTES: [ColonneSerie; 10] = [
        ColonneSerie::Titre,
        ColonneSerie::TitreOriginal,
        ColonneSerie::Annee,
        ColonneSerie::Statut,
        ColonneSerie::Note,
        ColonneSerie::Saisons,
        ColonneSerie::Episodes,
        ColonneSerie::Manquants,
        ColonneSerie::Chemin,
        ColonneSerie::Vu,
    ];

    /// Identifiant stable (clé de persistance).
    pub fn id(self) -> &'static str {
        match self {
            ColonneSerie::Titre => "title",
            ColonneSerie::TitreOriginal => "originalTitle",
            ColonneSerie::Annee => "year",
            ColonneSerie::Statut => "status",
            ColonneSerie::Note => "rating",
            ColonneSerie::Saisons => "seasons",
            ColonneSerie::Episodes => "episodes",
            ColonneSerie::Manquants => "missing",
            ColonneSerie::Chemin => "path",
            ColonneSerie::Vu => "watched",
        }
    }

    pub fn depuis_id(id: &str) -> Option<ColonneSerie> {
        ColonneSerie::TOUTES.into_iter().find(|c| c.id() == id)
    }

    pub fn cle_i18n(self) -> &'static str {
        match self {
            ColonneSerie::Titre => "metatag.title",
            ColonneSerie::TitreOriginal => "metatag.originaltitle",
            ColonneSerie::Annee => "metatag.year",
            ColonneSerie::Statut => "gui.series.status",
            ColonneSerie::Note => "metatag.rating",
            ColonneSerie::Saisons => "gui.tvshows.seasons",
            ColonneSerie::Episodes => "gui.tvshows.episodecount",
            ColonneSerie::Manquants => "gui.tvshows.missingcount",
            ColonneSerie::Chemin => "metatag.path",
            ColonneSerie::Vu => "metatag.watched",
        }
    }

    /// En-tête abrégé (`S`, `E`) ; le libellé complet est l'info-bulle.
    pub fn entete_court(self) -> Option<&'static str> {
        match self {
            ColonneSerie::Saisons => Some("S"),
            ColonneSerie::Episodes => Some("E"),
            _ => None,
        }
    }

    /// Visibles par défaut : titre, année, saisons, épisodes, vu.
    pub fn visible_par_defaut(self) -> bool {
        matches!(
            self,
            ColonneSerie::Titre
                | ColonneSerie::Annee
                | ColonneSerie::Saisons
                | ColonneSerie::Episodes
                | ColonneSerie::Vu
        )
    }

    /// Colonne à pictogramme d'état (largeur fixe).
    pub fn est_booleenne(self) -> bool {
        self == ColonneSerie::Vu
    }

    pub fn aligne_a_droite(self) -> bool {
        matches!(
            self,
            ColonneSerie::Annee
                | ColonneSerie::Note
                | ColonneSerie::Saisons
                | ColonneSerie::Episodes
                | ColonneSerie::Manquants
        )
    }

    pub fn largeur_initiale(self) -> f32 {
        match self {
            ColonneSerie::Titre | ColonneSerie::TitreOriginal => 240.0,
            ColonneSerie::Chemin => 260.0,
            ColonneSerie::Statut => 90.0,
            ColonneSerie::Annee => 52.0,
            ColonneSerie::Saisons => 32.0,
            ColonneSerie::Episodes => 44.0,
            ColonneSerie::Vu => 27.0,
            _ => 70.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColonneSerieEtat {
    pub colonne: ColonneSerie,
    pub visible: bool,
    /// Largeur mémorisée ; `None` : largeur initiale.
    pub largeur: Option<f32>,
}

/// État initial des colonnes du tableau Séries.
pub fn colonnes_series_par_defaut() -> Vec<ColonneSerieEtat> {
    ColonneSerie::TOUTES
        .iter()
        .map(|&c| ColonneSerieEtat {
            colonne: c,
            visible: c.visible_par_defaut(),
            largeur: None,
        })
        .collect()
}

/// Taille en gigaoctets (base 10^9), comme l'original.
pub fn gigaoctets(octets: u64) -> f64 {
    octets as f64 / 1e9
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalisation_sans_accents() {
        assert_eq!(normaliser("Amélie Poulain ŒUVRE"), "amelie poulain oeuvre");
    }

    #[test]
    fn colonnes_visibles_par_defaut() {
        let visibles: Vec<&str> = colonnes_par_defaut()
            .iter()
            .filter(|c| c.visible)
            .map(|c| c.colonne.id())
            .collect();
        assert_eq!(
            visibles,
            [
                "title",
                "year",
                "rating",
                "new",
                "metadata",
                "images",
                "trailer",
                "subtitles",
                "watched"
            ]
        );
    }

    #[test]
    fn comparaison_par_resolution_et_taille() {
        let a = Film {
            hauteur_video: 720,
            taille_octets: 9_000_000_000,
            ..Film::default()
        };
        let b = Film {
            hauteur_video: 2160,
            taille_octets: 2_000_000_000,
            ..Film::default()
        };
        assert_eq!(Colonne::FormatVideo.comparer(&a, &b), Ordering::Less);
        assert_eq!(Colonne::TailleFichier.comparer(&a, &b), Ordering::Greater);
    }

    #[test]
    fn colonnes_des_series_par_defaut() {
        let visibles: Vec<&str> = colonnes_series_par_defaut()
            .iter()
            .filter(|c| c.visible)
            .map(|c| c.colonne.id())
            .collect();
        assert_eq!(
            visibles,
            ["title", "year", "seasons", "episodes", "watched"]
        );
        let mut ids: Vec<_> = ColonneSerie::TOUTES.iter().map(|c| c.id()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), ColonneSerie::TOUTES.len());
        assert_eq!(
            ColonneSerie::depuis_id("episodes"),
            Some(ColonneSerie::Episodes)
        );
        assert_eq!(ColonneSerie::depuis_id("x"), None);
    }

    #[test]
    fn identifiants_uniques() {
        let mut ids: Vec<_> = Colonne::TOUTES.iter().map(|c| c.id()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), Colonne::TOUTES.len());
    }
}
