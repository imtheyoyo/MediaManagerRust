//! Correspondance propre aux séries : choix du meilleur résultat (seuil 0.75)
//! et appariement d'un épisode dans une liste (identifiant, titre, numéro, date).

use crate::erreur::{ErreurScraper, Resultat};
use crate::types::{MetadonneesEpisode, OptionsEpisode, ResultatRecherche};

/// Seuil de score pour accepter automatiquement une série (valeur fixe).
pub const SEUIL_SERIE: f32 = 0.75;

/// Longueur minimale (en caractères) d'un titre pour l'appariement par titre exact.
pub const LONGUEUR_MIN_TITRE: usize = 16;

/// Choisit la série d'une liste triée : un résultat unique est accepté quel que
/// soit son score ; sinon `None` si ex æquo en tête ou score sous `seuil`.
pub fn choisir_meilleure_serie(
    resultats: &[ResultatRecherche],
    seuil: f32,
) -> Option<&ResultatRecherche> {
    match resultats {
        [] => None,
        [seul] => Some(seul),
        _ => crate::correspondance::choisir_meilleur(resultats, seuil),
    }
}

/// Règles d'appariement d'un épisode (varient selon le fournisseur).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReglesAppariement {
    /// Clé de l'identifiant d'épisode propre au fournisseur (`tmdb`, `tvmaze`...).
    pub cle_id: &'static str,
    /// Essai d'un titre identique (titres de plus de 15 caractères) avant les numéros.
    pub titre_avant_numeros: bool,
    /// La date de diffusion n'est retenue que si un seul épisode correspond.
    pub date_unique: bool,
}

impl ReglesAppariement {
    /// Règles TMDB.
    pub const TMDB: Self = Self {
        cle_id: "tmdb",
        titre_avant_numeros: false,
        date_unique: false,
    };
    /// Règles TVmaze.
    pub const TVMAZE: Self = Self {
        cle_id: "tvmaze",
        titre_avant_numeros: true,
        date_unique: true,
    };
}

/// Apparie l'épisode demandé dans `episodes`.
///
/// Ordre : identifiant du fournisseur ; (selon les règles) titre exact ;
/// numéro dans le groupe demandé ; date de diffusion.
/// `IdentifiantManquant` si rien n'est exploitable, `AucunResultat` sinon.
pub fn apparier_episode<'a>(
    episodes: &'a [MetadonneesEpisode],
    cible: &OptionsEpisode,
    regles: ReglesAppariement,
) -> Resultat<&'a MetadonneesEpisode> {
    let id = cible
        .ids_episode
        .get(regles.cle_id)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != "0");
    let numeros = cible.saison.zip(cible.episode);
    let date = cible
        .date_diffusion
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty());
    let titre = cible.titre.trim();
    let titre_utilisable =
        regles.titre_avant_numeros && titre.chars().count() >= LONGUEUR_MIN_TITRE;
    if id.is_none() && numeros.is_none() && date.is_none() && !titre_utilisable {
        return Err(ErreurScraper::IdentifiantManquant(format!(
            "{}, saison/épisode, date de diffusion",
            regles.cle_id
        )));
    }
    if let Some(id) = id
        && let Some(e) = episodes
            .iter()
            .find(|e| e.ids.get(regles.cle_id).map(String::as_str) == Some(id))
    {
        return Ok(e);
    }
    if titre_utilisable && let Some(e) = episodes.iter().find(|e| e.titre.trim() == titre) {
        return Ok(e);
    }
    if let Some((s, n)) = numeros
        && let Some(e) = episodes
            .iter()
            .find(|e| e.numero(&cible.groupe) == Some((s, n)))
    {
        return Ok(e);
    }
    if let Some(d) = date {
        let mut candidats = episodes
            .iter()
            .filter(|e| e.date_diffusion.as_deref() == Some(d));
        if let Some(premier) = candidats.next()
            && (!regles.date_unique || candidats.next().is_none())
        {
            return Ok(premier);
        }
    }
    Err(ErreurScraper::AucunResultat)
}

/// Traductions du mot « épisode » servant à repérer un titre générique.
const MOTS_EPISODE: [&str; 24] = [
    "episode",
    "épisode",
    "episodio",
    "episódio",
    "folge",
    "aflevering",
    "odcinek",
    "epizoda",
    "epizod",
    "epizód",
    "epizodas",
    "epizode",
    "avsnitt",
    "jakso",
    "afsnit",
    "episod",
    "bölüm",
    "серия",
    "эпизод",
    "епізод",
    "επεισόδιο",
    "エピソード",
    "에피소드",
    "第",
];

/// Vrai si le titre est générique (« Episode 5 », « Épisode 12 »...) : vide
/// une fois les chiffres retirés, ou contenant une traduction de « épisode ».
pub fn est_titre_generique(titre: &str) -> bool {
    let sans_chiffres: String = titre
        .chars()
        .filter(|c| !c.is_ascii_digit())
        .collect::<String>()
        .to_lowercase();
    let t = sans_chiffres.trim();
    if t.is_empty() {
        return true;
    }
    MOTS_EPISODE.iter().any(|m| t.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GroupeEpisodes, TypeGroupeEpisodes};

    fn ep(id: &str, titre: &str, s: u32, e: u32, date: &str) -> MetadonneesEpisode {
        let mut m = MetadonneesEpisode {
            titre: titre.into(),
            date_diffusion: Some(date.into()),
            ..Default::default()
        };
        m.ids.insert("tmdb".into(), id.into());
        m.ids.insert("tvmaze".into(), id.into());
        m.definir_numero(GroupeEpisodes::diffusion(), s, e);
        m
    }

    fn liste() -> Vec<MetadonneesEpisode> {
        let mut v = vec![
            ep("10", "Pilote", 1, 1, "2020-01-01"),
            ep("11", "Un titre suffisamment long", 1, 2, "2020-01-08"),
            ep("12", "Double A", 1, 3, "2020-01-15"),
            ep("13", "Double B", 1, 4, "2020-01-15"),
        ];
        v[2].definir_numero(GroupeEpisodes::dvd(), 2, 1);
        v
    }

    #[test]
    fn seuil_serie() {
        let mut a = ResultatRecherche {
            score: 0.2,
            ..Default::default()
        };
        assert!(choisir_meilleure_serie(std::slice::from_ref(&a), SEUIL_SERIE).is_some());
        let mut b = a.clone();
        b.score = 0.1;
        assert!(choisir_meilleure_serie(&[a.clone(), b.clone()], SEUIL_SERIE).is_none());
        a.score = 0.8;
        assert!(choisir_meilleure_serie(&[a.clone(), b], SEUIL_SERIE).is_some());
        assert!(choisir_meilleure_serie(&[], SEUIL_SERIE).is_none());
    }

    #[test]
    fn appariement_par_id_puis_numero_puis_date() {
        let l = liste();
        let mut c = OptionsEpisode::default();
        c.ids_episode.insert("tmdb".into(), "11".into());
        c.saison = Some(1);
        c.episode = Some(1);
        assert_eq!(
            apparier_episode(&l, &c, ReglesAppariement::TMDB)
                .unwrap()
                .titre,
            "Un titre suffisamment long"
        );
        c.ids_episode.clear();
        assert_eq!(
            apparier_episode(&l, &c, ReglesAppariement::TMDB)
                .unwrap()
                .titre,
            "Pilote"
        );
        let c = OptionsEpisode {
            date_diffusion: Some("2020-01-08".into()),
            ..Default::default()
        };
        assert_eq!(
            apparier_episode(&l, &c, ReglesAppariement::TMDB)
                .unwrap()
                .titre,
            "Un titre suffisamment long"
        );
    }

    #[test]
    fn appariement_groupe_dvd() {
        let l = liste();
        let c = OptionsEpisode {
            groupe: GroupeEpisodes::nouveau(TypeGroupeEpisodes::Dvd, ""),
            saison: Some(2),
            episode: Some(1),
            ..Default::default()
        };
        assert_eq!(
            apparier_episode(&l, &c, ReglesAppariement::TMDB)
                .unwrap()
                .titre,
            "Double A"
        );
    }

    #[test]
    fn date_ambigue_selon_regles() {
        let l = liste();
        let c = OptionsEpisode {
            date_diffusion: Some("2020-01-15".into()),
            ..Default::default()
        };
        assert_eq!(
            apparier_episode(&l, &c, ReglesAppariement::TMDB)
                .unwrap()
                .titre,
            "Double A"
        );
        assert!(matches!(
            apparier_episode(&l, &c, ReglesAppariement::TVMAZE),
            Err(ErreurScraper::AucunResultat)
        ));
    }

    #[test]
    fn titre_prioritaire_tvmaze() {
        let l = liste();
        let c = OptionsEpisode {
            titre: "Un titre suffisamment long".into(),
            saison: Some(1),
            episode: Some(1),
            ..Default::default()
        };
        assert_eq!(
            apparier_episode(&l, &c, ReglesAppariement::TVMAZE)
                .unwrap()
                .titre,
            "Un titre suffisamment long"
        );
        assert_eq!(
            apparier_episode(&l, &c, ReglesAppariement::TMDB)
                .unwrap()
                .titre,
            "Pilote"
        );
        // Titre court : ignoré.
        let c = OptionsEpisode {
            titre: "Pilote".into(),
            ..Default::default()
        };
        assert!(matches!(
            apparier_episode(&l, &c, ReglesAppariement::TVMAZE),
            Err(ErreurScraper::IdentifiantManquant(_))
        ));
    }

    #[test]
    fn rien_d_exploitable_ou_rien_trouve() {
        let l = liste();
        assert!(matches!(
            apparier_episode(&l, &OptionsEpisode::default(), ReglesAppariement::TMDB),
            Err(ErreurScraper::IdentifiantManquant(_))
        ));
        let c = OptionsEpisode::diffusion(Default::default(), 9, 9);
        assert!(matches!(
            apparier_episode(&l, &c, ReglesAppariement::TMDB),
            Err(ErreurScraper::AucunResultat)
        ));
    }

    #[test]
    fn titres_generiques() {
        assert!(est_titre_generique("Episode 5"));
        assert!(est_titre_generique("Épisode 12"));
        assert!(est_titre_generique("Folge 3"));
        assert!(est_titre_generique("12"));
        assert!(!est_titre_generique("Pilote"));
    }

    #[test]
    fn egalite_des_groupes() {
        assert_eq!(
            GroupeEpisodes::nouveau(TypeGroupeEpisodes::Diffusion, "x"),
            GroupeEpisodes::diffusion()
        );
        assert_ne!(
            GroupeEpisodes::nouveau(TypeGroupeEpisodes::Alternatif, "a"),
            GroupeEpisodes::nouveau(TypeGroupeEpisodes::Alternatif, "b")
        );
    }
}
