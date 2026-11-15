//! Filtres avancés (spécification §2.3) : six onglets, une ligne par filtre avec une case à
//! trois états (inactif / inclure / exclure), combinaison en ET, préréglages nommés.
//! Logique pure, sans egui.

use crate::modele::{Film, gigaoctets, normaliser};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// Case à trois états : `[ ]` inactif, `[X]` inclure, `[-]` exclure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EtatFiltre {
    #[default]
    Inactif,
    Inclure,
    Exclure,
}

impl EtatFiltre {
    /// Cycle d'un clic : inactif -> inclure -> exclure -> inactif.
    pub fn suivant(self) -> Self {
        match self {
            EtatFiltre::Inactif => EtatFiltre::Inclure,
            EtatFiltre::Inclure => EtatFiltre::Exclure,
            EtatFiltre::Exclure => EtatFiltre::Inactif,
        }
    }

    pub fn glyphe(self) -> &'static str {
        match self {
            EtatFiltre::Inactif => " ",
            EtatFiltre::Inclure => "✔",
            EtatFiltre::Exclure => "−",
        }
    }
}

/// Opérateurs numériques (et de date) : LT, LE, EQ, GE, GT, BT (entre).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OperateurNombre {
    Inf,
    InfEgal,
    #[default]
    Egal,
    SupEgal,
    Sup,
    Entre,
}

impl OperateurNombre {
    pub const TOUS: [OperateurNombre; 6] = [
        OperateurNombre::Inf,
        OperateurNombre::InfEgal,
        OperateurNombre::Egal,
        OperateurNombre::SupEgal,
        OperateurNombre::Sup,
        OperateurNombre::Entre,
    ];

    pub fn symbole(self) -> &'static str {
        match self {
            OperateurNombre::Inf => "<",
            OperateurNombre::InfEgal => "≤",
            OperateurNombre::Egal => "=",
            OperateurNombre::SupEgal => "≥",
            OperateurNombre::Sup => ">",
            OperateurNombre::Entre => "↔",
        }
    }

    fn compare<T: PartialOrd>(self, v: T, a: T, b: T) -> bool {
        match self {
            OperateurNombre::Inf => v < a,
            OperateurNombre::InfEgal => v <= a,
            OperateurNombre::Egal => v == a,
            OperateurNombre::SupEgal => v >= a,
            OperateurNombre::Sup => v > a,
            OperateurNombre::Entre => {
                let (min, max) = if a <= b { (a, b) } else { (b, a) };
                v >= min && v <= max
            }
        }
    }
}

/// Opérateurs d'ensemble : au moins un (ANY) / tous (ALL).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OperateurEnsemble {
    #[default]
    AuMoinsUn,
    Tous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OngletFiltre {
    Details,
    Metadonnee,
    Video,
    Audio,
    Illustration,
    Autres,
}

impl OngletFiltre {
    pub const TOUS: [OngletFiltre; 6] = [
        OngletFiltre::Details,
        OngletFiltre::Metadonnee,
        OngletFiltre::Video,
        OngletFiltre::Audio,
        OngletFiltre::Illustration,
        OngletFiltre::Autres,
    ];

    pub fn cle(self) -> &'static str {
        match self {
            OngletFiltre::Details => "metatag.details",
            OngletFiltre::Metadonnee => "filter.tab.metadata",
            OngletFiltre::Video => "filter.tab.video",
            OngletFiltre::Audio => "metatag.audio",
            OngletFiltre::Illustration => "metatag.artwork",
            OngletFiltre::Autres => "filter.tab.other",
        }
    }
}

/// Composant de valeur d'un filtre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeValeur {
    /// Simple prédicat (la case suffit).
    Aucune,
    Texte,
    Nombre,
    /// Date `AAAA-MM-JJ` (préfixe accepté), comparée avec les opérateurs numériques.
    Date,
    /// Liste à cases multiples ; `ensemble` : champ multivalué (ANY / ALL).
    Choix {
        ensemble: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IdFiltre {
    Nouveaux,
    SourceDonnees,
    Vu,
    Verrouille,
    DateAjout,
    Doublons,
    DansSaga,
    Universel,
    Annee,
    DateSortie,
    Decennie,
    Genres,
    Certification,
    Distribution,
    Realisateur,
    Pays,
    Production,
    Note,
    IdImdb,
    IdTmdb,
    FormatVideo,
    CodecVideo,
    FormatHdr,
    CodecAudio,
    ArtworkManquant,
    NomFichier,
    TailleVideo,
    Chemin,
    Duree,
    MetadonneeManquante,
    SousTitresManquants,
    BandeAnnonceManquante,
}

impl IdFiltre {
    pub const TOUS: [IdFiltre; 32] = [
        IdFiltre::Nouveaux,
        IdFiltre::SourceDonnees,
        IdFiltre::Vu,
        IdFiltre::Verrouille,
        IdFiltre::DateAjout,
        IdFiltre::Doublons,
        IdFiltre::DansSaga,
        IdFiltre::Universel,
        IdFiltre::Annee,
        IdFiltre::DateSortie,
        IdFiltre::Decennie,
        IdFiltre::Genres,
        IdFiltre::Certification,
        IdFiltre::Distribution,
        IdFiltre::Realisateur,
        IdFiltre::Pays,
        IdFiltre::Production,
        IdFiltre::Note,
        IdFiltre::IdImdb,
        IdFiltre::IdTmdb,
        IdFiltre::FormatVideo,
        IdFiltre::CodecVideo,
        IdFiltre::FormatHdr,
        IdFiltre::CodecAudio,
        IdFiltre::ArtworkManquant,
        IdFiltre::NomFichier,
        IdFiltre::TailleVideo,
        IdFiltre::Chemin,
        IdFiltre::Duree,
        IdFiltre::MetadonneeManquante,
        IdFiltre::SousTitresManquants,
        IdFiltre::BandeAnnonceManquante,
    ];

    pub fn onglet(self) -> OngletFiltre {
        use IdFiltre::*;
        match self {
            Nouveaux | SourceDonnees | Vu | Verrouille | DateAjout | Doublons | DansSaga
            | Universel => OngletFiltre::Details,
            Annee | DateSortie | Decennie | Genres | Certification | Distribution | Realisateur
            | Pays | Production | Note | IdImdb | IdTmdb => OngletFiltre::Metadonnee,
            FormatVideo | CodecVideo | FormatHdr => OngletFiltre::Video,
            CodecAudio => OngletFiltre::Audio,
            ArtworkManquant => OngletFiltre::Illustration,
            NomFichier
            | TailleVideo
            | Chemin
            | Duree
            | MetadonneeManquante
            | SousTitresManquants
            | BandeAnnonceManquante => OngletFiltre::Autres,
        }
    }

    pub fn type_valeur(self) -> TypeValeur {
        use IdFiltre::*;
        match self {
            Nouveaux
            | Vu
            | Verrouille
            | Doublons
            | DansSaga
            | ArtworkManquant
            | MetadonneeManquante
            | SousTitresManquants
            | BandeAnnonceManquante => TypeValeur::Aucune,
            Universel | Distribution | Realisateur | Production | IdImdb | IdTmdb | NomFichier
            | Chemin => TypeValeur::Texte,
            Annee | Note | TailleVideo | Duree => TypeValeur::Nombre,
            DateAjout | DateSortie => TypeValeur::Date,
            Genres | Pays => TypeValeur::Choix { ensemble: true },
            SourceDonnees | Decennie | Certification | FormatVideo | CodecVideo | FormatHdr
            | CodecAudio => TypeValeur::Choix { ensemble: false },
        }
    }

    pub fn cle(self) -> &'static str {
        use IdFiltre::*;
        match self {
            Nouveaux => "filter.new",
            SourceDonnees => "metatag.datasource",
            Vu => "metatag.watched",
            Verrouille => "filter.locked",
            DateAjout => "metatag.dateadded",
            Doublons => "filter.duplicates",
            DansSaga => "filter.inmovieset",
            Universel => "filter.universal",
            Annee => "metatag.year",
            DateSortie => "metatag.releasedate",
            Decennie => "filter.decade",
            Genres => "metatag.genre",
            Certification => "metatag.certification",
            Distribution => "metatag.cast",
            Realisateur => "metatag.director",
            Pays => "metatag.country",
            Production => "metatag.production",
            Note => "metatag.rating",
            IdImdb => "filter.id.imdb",
            IdTmdb => "filter.id.tmdb",
            FormatVideo => "metatag.videoformat",
            CodecVideo => "metatag.videocodec",
            FormatHdr => "filter.hdrformat",
            CodecAudio => "filter.audiocodec",
            ArtworkManquant => "filter.missingartwork",
            NomFichier => "metatag.filename",
            TailleVideo => "filter.videosize",
            Chemin => "metatag.path",
            Duree => "metatag.runtime",
            MetadonneeManquante => "filter.missingmetadata",
            SousTitresManquants => "filter.missingsubtitles",
            BandeAnnonceManquante => "filter.missingtrailer",
        }
    }

    /// Valeur(s) du film pour les filtres à choix.
    fn valeurs(self, f: &Film) -> Vec<String> {
        use IdFiltre::*;
        match self {
            SourceDonnees => vec![f.source.clone()],
            Decennie => vec![format!("{}s", f.annee / 10 * 10)],
            Genres => f.genres.clone(),
            Pays => f.pays.clone(),
            Certification => vec![f.certification.clone()],
            FormatVideo => vec![f.format_video()],
            CodecVideo => vec![f.codec_video.clone()],
            FormatHdr => vec![f.hdr.clone().unwrap_or_else(|| "SDR".into())],
            CodecAudio => vec![f.audio.clone()],
            _ => Vec::new(),
        }
    }

    fn texte(self, f: &Film) -> String {
        use IdFiltre::*;
        match self {
            Universel => f.texte_recherche(),
            Distribution => f
                .acteurs
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            Realisateur => f.realisateurs.join("\n"),
            Production => f.production.join("\n"),
            IdImdb => f.imdb.clone(),
            IdTmdb => f.tmdb.clone(),
            NomFichier => f.nom_fichier.clone(),
            Chemin => f.chemin(),
            DateAjout => f.date_ajout.clone(),
            DateSortie => f.date_sortie.clone(),
            _ => String::new(),
        }
    }

    fn nombre(self, f: &Film) -> f64 {
        match self {
            IdFiltre::Annee => f.annee as f64,
            IdFiltre::Note => f.note as f64,
            IdFiltre::TailleVideo => gigaoctets(f.taille_octets),
            IdFiltre::Duree => f.duree_min as f64,
            _ => 0.0,
        }
    }
}

/// Contexte de calcul partagé par toutes les lignes (ensembles précalculés).
#[derive(Clone, Debug, Default)]
pub struct ContexteFiltre {
    pub doublons: HashSet<u64>,
}

impl ContexteFiltre {
    pub fn calculer(films: &[Film]) -> Self {
        Self {
            doublons: doublons(films),
        }
    }
}

/// Films en double : même titre normalisé et même année, ou même identifiant IMDb.
pub fn doublons(films: &[Film]) -> HashSet<u64> {
    let mut par_titre: HashMap<(String, u32), Vec<u64>> = HashMap::new();
    let mut par_imdb: HashMap<&str, Vec<u64>> = HashMap::new();
    for f in films {
        par_titre
            .entry((normaliser(&f.titre), f.annee))
            .or_default()
            .push(f.id);
        if !f.imdb.is_empty() {
            par_imdb.entry(f.imdb.as_str()).or_default().push(f.id);
        }
    }
    par_titre
        .into_values()
        .chain(par_imdb.into_values())
        .filter(|v| v.len() > 1)
        .flatten()
        .collect()
}

/// Une ligne du dialogue (persistée telle quelle dans les préréglages).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LigneFiltre {
    pub id: IdFiltre,
    #[serde(default)]
    pub etat: EtatFiltre,
    #[serde(default)]
    pub texte: String,
    #[serde(default)]
    pub texte2: String,
    #[serde(default)]
    pub a: f64,
    #[serde(default)]
    pub b: f64,
    #[serde(default)]
    pub op_nombre: OperateurNombre,
    #[serde(default)]
    pub choix: BTreeSet<String>,
    #[serde(default)]
    pub op_ensemble: OperateurEnsemble,
}

impl LigneFiltre {
    pub fn nouvelle(id: IdFiltre) -> Self {
        Self {
            id,
            etat: EtatFiltre::Inactif,
            texte: String::new(),
            texte2: String::new(),
            a: 0.0,
            b: 0.0,
            op_nombre: OperateurNombre::default(),
            choix: BTreeSet::new(),
            op_ensemble: OperateurEnsemble::default(),
        }
    }

    /// Vrai si la ligne participe au filtrage (état actif et valeur renseignée).
    pub fn effective(&self) -> bool {
        self.etat != EtatFiltre::Inactif
            && match self.id.type_valeur() {
                TypeValeur::Texte | TypeValeur::Date => !self.texte.trim().is_empty(),
                TypeValeur::Choix { .. } => !self.choix.is_empty(),
                TypeValeur::Aucune | TypeValeur::Nombre => true,
            }
    }

    /// Le film correspond-il au critère (avant application de l'état inclure / exclure) ?
    pub fn correspond(&self, f: &Film, ctx: &ContexteFiltre) -> bool {
        use IdFiltre::*;
        match self.id.type_valeur() {
            TypeValeur::Aucune => match self.id {
                Nouveaux => f.nouveau,
                Vu => f.vu,
                Verrouille => f.verrouille,
                Doublons => ctx.doublons.contains(&f.id),
                DansSaga => f.saga.is_some(),
                ArtworkManquant => !f.images_ok,
                MetadonneeManquante => !f.metadonnees_ok,
                SousTitresManquants => !f.sous_titres,
                BandeAnnonceManquante => !f.bande_annonce,
                _ => true,
            },
            TypeValeur::Texte => {
                normaliser(&self.id.texte(f)).contains(&normaliser(self.texte.trim()))
            }
            TypeValeur::Nombre => self.op_nombre.compare(self.id.nombre(f), self.a, self.b),
            TypeValeur::Date => {
                // Comparaison lexicographique sur la longueur de la saisie (préfixe de date).
                let v = self.id.texte(f);
                let (a, b) = (self.texte.trim(), self.texte2.trim());
                let n = a.len().max(b.len()).min(v.len());
                let tronque = |s: &str| s.chars().take(n).collect::<String>();
                self.op_nombre.compare(tronque(&v), tronque(a), tronque(b))
            }
            TypeValeur::Choix { ensemble } => {
                let valeurs = self.id.valeurs(f);
                if ensemble && self.op_ensemble == OperateurEnsemble::Tous {
                    self.choix.iter().all(|c| valeurs.contains(c))
                } else {
                    valeurs.iter().any(|v| self.choix.contains(v))
                }
            }
        }
    }

    pub fn accepte(&self, f: &Film, ctx: &ContexteFiltre) -> bool {
        if !self.effective() {
            return true;
        }
        let ok = self.correspond(f, ctx);
        match self.etat {
            EtatFiltre::Exclure => !ok,
            _ => ok,
        }
    }
}

/// Actions du dialogue de filtres (appliquées par `FiltresAvances::appliquer`).
#[derive(Clone, Debug)]
pub enum ActionFiltre {
    Ligne(Box<LigneFiltre>),
    Activer(bool),
    ReinitialiserOnglet(OngletFiltre),
    Reinitialiser,
    EnregistrerPrereglage(String),
    AppliquerPrereglage(String),
    SupprimerPrereglage(String),
}

/// État complet des filtres avancés.
#[derive(Clone, Debug, PartialEq)]
pub struct FiltresAvances {
    /// Une ligne par filtre, dans l'ordre de `IdFiltre::TOUS`.
    pub lignes: Vec<LigneFiltre>,
    /// « Activer le filtrage » : coupe tous les filtres sans les perdre.
    pub actif: bool,
    pub prereglages: BTreeMap<String, Vec<LigneFiltre>>,
}

impl Default for FiltresAvances {
    fn default() -> Self {
        Self {
            lignes: IdFiltre::TOUS
                .iter()
                .map(|&id| LigneFiltre::nouvelle(id))
                .collect(),
            actif: true,
            prereglages: BTreeMap::new(),
        }
    }
}

impl FiltresAvances {
    pub fn ligne(&self, id: IdFiltre) -> &LigneFiltre {
        self.lignes
            .iter()
            .find(|l| l.id == id)
            .expect("une ligne par filtre")
    }

    pub fn lignes_onglet(&self, onglet: OngletFiltre) -> impl Iterator<Item = &LigneFiltre> {
        self.lignes.iter().filter(move |l| l.id.onglet() == onglet)
    }

    /// Au moins un filtre effectif et filtrage activé.
    pub fn est_actif(&self) -> bool {
        self.actif && self.lignes.iter().any(LigneFiltre::effective)
    }

    /// Pastille d'entonnoir sur l'onglet.
    pub fn onglet_actif(&self, onglet: OngletFiltre) -> bool {
        self.lignes_onglet(onglet).any(LigneFiltre::effective)
    }

    pub fn utilise_doublons(&self) -> bool {
        self.ligne(IdFiltre::Doublons).effective()
    }

    /// Combinaison en ET de toutes les lignes effectives.
    pub fn accepte(&self, f: &Film, ctx: &ContexteFiltre) -> bool {
        !self.actif || self.lignes.iter().all(|l| l.accepte(f, ctx))
    }

    /// Lignes non inactives (contenu d'un préréglage / de l'état mémorisé).
    pub fn lignes_actives(&self) -> Vec<LigneFiltre> {
        self.lignes
            .iter()
            .filter(|l| l.etat != EtatFiltre::Inactif)
            .cloned()
            .collect()
    }

    /// Remplace l'état courant par une liste de lignes (les autres redeviennent inactives).
    pub fn charger_lignes(&mut self, lignes: &[LigneFiltre]) {
        for l in self.lignes.iter_mut() {
            *l = lignes
                .iter()
                .find(|x| x.id == l.id)
                .cloned()
                .unwrap_or_else(|| LigneFiltre::nouvelle(l.id));
        }
    }

    pub fn appliquer(&mut self, action: ActionFiltre) {
        match action {
            ActionFiltre::Ligne(ligne) => {
                if let Some(l) = self.lignes.iter_mut().find(|l| l.id == ligne.id) {
                    *l = *ligne;
                }
            }
            ActionFiltre::Activer(a) => self.actif = a,
            ActionFiltre::ReinitialiserOnglet(o) => {
                for l in self.lignes.iter_mut().filter(|l| l.id.onglet() == o) {
                    *l = LigneFiltre::nouvelle(l.id);
                }
            }
            ActionFiltre::Reinitialiser => {
                for l in self.lignes.iter_mut() {
                    *l = LigneFiltre::nouvelle(l.id);
                }
            }
            ActionFiltre::EnregistrerPrereglage(nom) => {
                let nom = nom.trim().to_string();
                if !nom.is_empty() {
                    self.prereglages.insert(nom, self.lignes_actives());
                }
            }
            ActionFiltre::AppliquerPrereglage(nom) => {
                if let Some(lignes) = self.prereglages.get(&nom).cloned() {
                    self.charger_lignes(&lignes);
                    self.actif = true;
                }
            }
            ActionFiltre::SupprimerPrereglage(nom) => {
                self.prereglages.remove(&nom);
            }
        }
    }
}

/// Options proposées par un filtre à choix (valeurs distinctes de la bibliothèque, triées).
pub fn options(id: IdFiltre, films: &[Film]) -> Vec<String> {
    let mut v: BTreeSet<String> = BTreeSet::new();
    for f in films {
        for x in id.valeurs(f) {
            if !x.is_empty() {
                v.insert(x);
            }
        }
    }
    v.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::donnees::films_factices;

    fn titres(fa: &FiltresAvances) -> Vec<String> {
        let films = films_factices();
        let ctx = ContexteFiltre::calculer(&films);
        let mut v: Vec<String> = films
            .iter()
            .filter(|f| fa.accepte(f, &ctx))
            .map(|f| format!("{} {}", f.titre, f.annee))
            .collect();
        v.sort();
        v
    }

    fn avec(fa: &mut FiltresAvances, id: IdFiltre, regler: impl FnOnce(&mut LigneFiltre)) {
        let mut l = fa.ligne(id).clone();
        regler(&mut l);
        fa.appliquer(ActionFiltre::Ligne(Box::new(l)));
    }

    #[test]
    fn cycle_trois_etats() {
        let e = EtatFiltre::Inactif;
        assert_eq!(e.suivant(), EtatFiltre::Inclure);
        assert_eq!(e.suivant().suivant(), EtatFiltre::Exclure);
        assert_eq!(e.suivant().suivant().suivant(), EtatFiltre::Inactif);
    }

    #[test]
    fn inclure_exclure_et_combinaison_et() {
        let mut fa = FiltresAvances::default();
        assert!(!fa.est_actif());
        assert_eq!(titres(&fa).len(), films_factices().len());
        avec(&mut fa, IdFiltre::Nouveaux, |l| {
            l.etat = EtatFiltre::Inclure
        });
        let nouveaux = titres(&fa);
        assert!(fa.est_actif() && fa.onglet_actif(OngletFiltre::Details));
        assert!(!fa.onglet_actif(OngletFiltre::Video));
        // ET : nouveaux ET format 2160p
        avec(&mut fa, IdFiltre::FormatVideo, |l| {
            l.etat = EtatFiltre::Inclure;
            l.choix.insert("2160p".into());
        });
        let t = titres(&fa);
        assert_eq!(t, ["Blade Runner 2049 2017", "Dune 2021", "Tenet 2020"]);
        // exclure le 2160p : le complément parmi les nouveaux
        avec(&mut fa, IdFiltre::FormatVideo, |l| {
            l.etat = EtatFiltre::Exclure
        });
        let t2 = titres(&fa);
        assert_eq!(t.len() + t2.len(), nouveaux.len());
        assert!(t2.iter().all(|x| !t.contains(x)));
        // « Activer le filtrage » décoché : tout passe, sans perdre les lignes
        fa.appliquer(ActionFiltre::Activer(false));
        assert_eq!(titres(&fa).len(), films_factices().len());
        assert!(fa.ligne(IdFiltre::Nouveaux).effective());
    }

    #[test]
    fn nombres_dates_et_textes() {
        let mut fa = FiltresAvances::default();
        avec(&mut fa, IdFiltre::Annee, |l| {
            l.etat = EtatFiltre::Inclure;
            l.op_nombre = OperateurNombre::Entre;
            l.a = 2000.0;
            l.b = 1994.0;
        });
        avec(&mut fa, IdFiltre::Note, |l| {
            l.etat = EtatFiltre::Inclure;
            l.op_nombre = OperateurNombre::SupEgal;
            l.a = 8.85;
        });
        assert_eq!(titres(&fa), ["Les Évadés 1994", "Pulp Fiction 1994"]);
        fa.appliquer(ActionFiltre::Reinitialiser);
        avec(&mut fa, IdFiltre::DateSortie, |l| {
            l.etat = EtatFiltre::Inclure;
            l.op_nombre = OperateurNombre::Inf;
            l.texte = "1980".into();
        });
        assert_eq!(titres(&fa), ["Alien 1979"]);
        fa.appliquer(ActionFiltre::ReinitialiserOnglet(OngletFiltre::Metadonnee));
        assert!(!fa.est_actif());
        // texte vide : ligne non effective même cochée
        avec(&mut fa, IdFiltre::NomFichier, |l| {
            l.etat = EtatFiltre::Inclure
        });
        assert!(!fa.est_actif());
        avec(&mut fa, IdFiltre::NomFichier, |l| l.texte = "2160P".into());
        assert_eq!(titres(&fa).len(), 3);
    }

    #[test]
    fn ensembles_au_moins_un_et_tous() {
        let mut films = films_factices();
        films[0].genres = vec!["Horreur".into(), "Science-fiction".into()];
        let ctx = ContexteFiltre::calculer(&films);
        let mut l = LigneFiltre::nouvelle(IdFiltre::Genres);
        l.etat = EtatFiltre::Inclure;
        l.choix = ["Horreur".to_string(), "Drame".to_string()].into();
        assert!(l.accepte(&films[0], &ctx) && l.accepte(&films[1], &ctx));
        l.op_ensemble = OperateurEnsemble::Tous;
        assert!(!l.accepte(&films[0], &ctx) && !l.accepte(&films[1], &ctx));
        l.choix = ["Horreur".to_string(), "Science-fiction".to_string()].into();
        assert!(l.accepte(&films[0], &ctx) && !l.accepte(&films[1], &ctx));
        assert!(options(IdFiltre::Genres, &films).contains(&"Horreur".to_string()));
    }

    #[test]
    fn doublons_par_titre_annee_ou_imdb() {
        let mut films = films_factices();
        assert!(doublons(&films).is_empty()); // les deux « Dune » ont des années différentes
        films[3].annee = 2021;
        let d = doublons(&films);
        assert!(d.contains(&3) && d.contains(&4) && d.len() == 2);
        films[3].annee = 1984;
        films[1].imdb = films[0].imdb.clone();
        assert_eq!(doublons(&films).len(), 2);
    }

    #[test]
    fn prereglages_enregistrer_appliquer_supprimer() {
        let mut fa = FiltresAvances::default();
        avec(&mut fa, IdFiltre::Vu, |l| l.etat = EtatFiltre::Exclure);
        fa.appliquer(ActionFiltre::EnregistrerPrereglage("  À voir ".into()));
        fa.appliquer(ActionFiltre::EnregistrerPrereglage("   ".into()));
        assert_eq!(fa.prereglages.keys().collect::<Vec<_>>(), ["À voir"]);
        assert_eq!(fa.prereglages["À voir"].len(), 1);
        fa.appliquer(ActionFiltre::Reinitialiser);
        fa.appliquer(ActionFiltre::Activer(false));
        fa.appliquer(ActionFiltre::AppliquerPrereglage("À voir".into()));
        assert!(fa.actif && fa.ligne(IdFiltre::Vu).etat == EtatFiltre::Exclure);
        fa.appliquer(ActionFiltre::SupprimerPrereglage("À voir".into()));
        assert!(fa.prereglages.is_empty());
    }
}
