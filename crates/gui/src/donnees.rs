//! Données factices pour montrer l'écran (à remplacer par la bibliothèque réelle).

use crate::modele::{BandeAnnonceFilm, FichierFilm, Film, TypeFichier};

fn s(v: &str) -> String {
    v.to_string()
}

fn liste(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// Construit les fichiers standard d'un film : vidéo, NFO, affiche, fanart (+ sous-titres).
fn fichiers(nom_fichier: &str, avec_st: bool, taille: u64) -> Vec<FichierFilm> {
    let (base, ext) = nom_fichier.rsplit_once('.').unwrap_or((nom_fichier, "mkv"));
    let mut v = vec![
        FichierFilm {
            type_fichier: TypeFichier::Video,
            nom: nom_fichier.to_string(),
            suffixe: Some(format!(".{ext}")),
            taille_octets: taille,
            codec: "H.264".into(),
            resolution: "1920x1080".into(),
            audio: "DTS 6ch fra, AC3 2ch eng".into(),
            sous_titres: "fra".into(),
            ..FichierFilm::default()
        },
        FichierFilm {
            type_fichier: TypeFichier::Nfo,
            nom: format!("{base}.nfo"),
            suffixe: Some(".nfo".into()),
            taille_octets: 4_200,
            ..FichierFilm::default()
        },
        FichierFilm {
            type_fichier: TypeFichier::Affiche,
            nom: s("poster.jpg"),
            suffixe: None,
            taille_octets: 310_000,
            ..FichierFilm::default()
        },
        FichierFilm {
            type_fichier: TypeFichier::Fanart,
            nom: s("fanart.jpg"),
            suffixe: None,
            taille_octets: 820_000,
            ..FichierFilm::default()
        },
    ];
    if avec_st {
        v.push(FichierFilm {
            type_fichier: TypeFichier::SousTitres,
            nom: format!("{base}.fr.srt"),
            suffixe: Some(".fr.srt".into()),
            taille_octets: 68_000,
            ..FichierFilm::default()
        });
    }
    v
}

#[allow(clippy::too_many_arguments)]
fn film(
    id: u64,
    titre: &str,
    original: &str,
    annee: u32,
    note: f32,
    duree: u32,
    hauteur: u32,
    dossier: &str,
    fichier: &str,
    taille_go: f64,
    drapeaux: &str,
) -> Film {
    let a = |c: char| drapeaux.contains(c);
    let titre_triable = match titre.split_once(' ') {
        Some(("Le", reste)) | Some(("La", reste)) | Some(("Les", reste)) => {
            format!("{reste}, {}", &titre[..titre.len() - reste.len() - 1])
        }
        _ => s(titre),
    };
    Film {
        id,
        titre: s(titre),
        titre_original: s(original),
        titre_triable,
        annee,
        date_sortie: format!("{annee}-0{}-1{}", 1 + id % 9, id % 10),
        note,
        votes: 1200 + (id as u32) * 3170,
        notes_sources: vec![
            (s("IMDb"), note),
            (s("TMDB"), (note - 0.4).max(0.0)),
            (s("RT"), note * 10.0 + 4.0),
        ],
        certification: s(if id.is_multiple_of(3) { "PG-13" } else { "R" }),
        duree_min: duree,
        genres: liste(&["Science-fiction", "Drame"]),
        realisateurs: liste(&["Réalisateur Exemple"]),
        scenaristes: liste(&["Scénariste Exemple"]),
        pays: liste(&["États-Unis"]),
        production: liste(&["Studio Exemple"]),
        slogan: format!("Le slogan de « {titre} »."),
        resume: format!(
            "Résumé factice de « {titre} » : un texte assez long pour montrer le retour à la ligne automatique du panneau de détails, avec plusieurs phrases afin de tester le défilement vertical."
        ),
        imdb: format!("tt{:07}", 1_000_000 + id * 7919),
        tmdb: format!("{}", 100_000 + id * 313),
        source: s("D:\\Films"),
        dossier: dossier.to_string(),
        nom_fichier: fichier.to_string(),
        hauteur_video: hauteur,
        codec_video: s(if hauteur >= 2160 { "HEVC" } else { "H.264" }),
        hdr: if hauteur >= 2160 {
            Some(s("HDR10"))
        } else {
            None
        },
        audio: s(if hauteur >= 2160 {
            "TrueHD Atmos 7.1"
        } else {
            "DTS 5.1"
        }),
        taille_octets: (taille_go * 1e9) as u64,
        date_ajout: format!("2026-10-{:02} 18:{:02}", 1 + id % 28, id % 60),
        nouveau: a('n'),
        verrouille: a('l'),
        metadonnees_ok: a('m'),
        images_ok: a('i'),
        bande_annonce: a('b'),
        sous_titres: a('s'),
        vu: a('v'),
        acteurs: vec![
            (s("Actrice Exemple"), s("Rôle principal")),
            (s("Acteur Exemple"), s("Second rôle")),
            (s("Comédien Exemple"), s("Antagoniste")),
        ],
        fichiers: fichiers(fichier, a('s'), (taille_go * 1e9) as u64),
        bandes_annonces: if a('b') {
            vec![BandeAnnonceFilm {
                nom: s("Bande-annonce officielle"),
                source: s("YouTube"),
                qualite: s("1080p"),
                url: format!("https://www.youtube.com/watch?v=demo{id}"),
            }]
        } else {
            vec![]
        },
        saga: None,
    }
}

/// Sagas de démonstration (titre).
pub const SAGA_DUNE: &str = "Dune - La saga";
pub const SAGA_MATRIX: &str = "Matrix - La trilogie";

/// Quelques films factices (suffisants pour tester tri, filtre et sélection).
pub fn films_factices() -> Vec<Film> {
    let mut v = vec![
        film(
            1,
            "Alien",
            "Alien",
            1979,
            8.5,
            117,
            1080,
            "Alien (1979)",
            "Alien.1979.1080p.mkv",
            9.8,
            "mibsv",
        ),
        film(
            2,
            "Blade Runner 2049",
            "Blade Runner 2049",
            2017,
            8.0,
            164,
            2160,
            "Blade.Runner.2049.2017.1080p",
            "br2049.1080p.mkv",
            21.4,
            "nmibs",
        ),
        film(
            3,
            "Dune",
            "Dune",
            2021,
            8.0,
            155,
            2160,
            "Dune (2021)",
            "Dune.2021.2160p.mkv",
            31.2,
            "nmb",
        ),
        film(
            4,
            "Dune",
            "Dune",
            1984,
            6.3,
            137,
            1080,
            "Dune (1984)",
            "Dune.1984.1080p.mkv",
            7.5,
            "mi",
        ),
        film(
            5,
            "Le Fabuleux Destin d'Amélie Poulain",
            "Amélie",
            2001,
            8.3,
            122,
            1080,
            "Amelie.2001",
            "Amelie.2001.mkv",
            8.1,
            "mibsv",
        ),
        film(
            6,
            "Les Évadés",
            "The Shawshank Redemption",
            1994,
            9.3,
            142,
            1080,
            "Les Evades (1994)",
            "Les.Evades.1994.mkv",
            10.2,
            "mibsvl",
        ),
        film(
            7,
            "Interstellar",
            "Interstellar",
            2014,
            8.7,
            169,
            2160,
            "Interstellar (2014)",
            "Interstellar.2014.2160p.mkv",
            38.0,
            "mibsv",
        ),
        film(
            8,
            "Mad Max: Fury Road",
            "Mad Max: Fury Road",
            2015,
            8.1,
            120,
            1080,
            "Mad Max Fury Road",
            "mad.max.fury.road.mkv",
            11.3,
            "ni",
        ),
        film(
            9,
            "Matrix",
            "The Matrix",
            1999,
            8.7,
            136,
            1080,
            "Matrix (1999)",
            "Matrix.1999.mkv",
            12.0,
            "mibsv",
        ),
        film(
            10,
            "Parasite",
            "기생충",
            2019,
            8.5,
            132,
            1080,
            "Parasite (2019)",
            "Parasite.2019.mkv",
            14.7,
            "mibs",
        ),
        film(
            11,
            "Pulp Fiction",
            "Pulp Fiction",
            1994,
            8.9,
            154,
            720,
            "Pulp Fiction",
            "pulp.fiction.720p.mkv",
            5.6,
            "mv",
        ),
        film(
            12,
            "Tenet",
            "Tenet",
            2020,
            7.3,
            150,
            2160,
            "Tenet (2020)",
            "Tenet.2020.2160p.mkv",
            29.9,
            "nmib",
        ),
        film(
            13,
            "Gladiator",
            "Gladiator",
            2000,
            8.5,
            155,
            1080,
            "Gladiator (2000)",
            "Gladiator.2000.mkv",
            13.4,
            "mibsv",
        ),
        film(
            14,
            "Hérédité",
            "Hereditary",
            2018,
            7.3,
            127,
            1080,
            "Heredite (2018)",
            "Hereditary.2018.mkv",
            9.0,
            "ns",
        ),
    ];
    for f in v.iter_mut() {
        f.saga = match f.id {
            3 | 4 => Some(s(SAGA_DUNE)),
            9 => Some(s(SAGA_MATRIX)),
            _ => None,
        };
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiants_uniques() {
        let mut ids: Vec<u64> = films_factices().iter().map(|f| f.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), films_factices().len());
    }
}
