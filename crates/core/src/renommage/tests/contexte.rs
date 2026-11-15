//! Contexte des gabarits : exemples de la spécification.

use mediamanager_gabarit::{FormeDestination, creer_destination};

use super::aides::*;
use crate::modele::*;
use crate::renommage::*;

fn fichier_film(g: &str, f: &Film, saga: Option<&Saga>) -> String {
    let profil = ProfilRenommageFilm {
        gabarit_fichier: g.into(),
        ..Default::default()
    };
    nom_fichier_film(f, saga, &profil).unwrap()
}

fn dossier_film(g: &str, f: &Film) -> String {
    let profil = ProfilRenommageFilm {
        gabarit_dossier: g.into(),
        ..Default::default()
    };
    nom_dossier_film(f, None, &profil).unwrap()
}

#[test]
fn tables_de_jetons_valides() {
    assert_eq!(jetons_film().len(), JETONS_FILM.len());
    assert_eq!(jetons_serie().len(), JETONS_SERIE.len());
}

#[test]
fn gabarits_par_defaut() {
    let f = matrix();
    assert_eq!(
        fichier_film(profil::GABARIT_FICHIER_FILM, &f, None),
        BASE_MATRIX
    );
    assert_eq!(
        dossier_film(profil::GABARIT_DOSSIER_FILM, &f),
        "The Matrix (1999)"
    );
}

#[test]
fn star_wars_deux_points_et_edition() {
    let mut f = matrix();
    f.titre = "Star Wars: Episode IV – A New Hope".into();
    f.annee = 1977;
    assert_eq!(
        fichier_film("${title} (${year})", &f, None),
        "Star Wars - Episode IV – A New Hope (1977)"
    );
    f.edition = Edition::DirectorsCut;
    assert_eq!(
        fichier_film("${title}${ - ,edition,} (${year})", &f, None),
        "Star Wars - Episode IV – A New Hope - Director's Cut (1977)"
    );
}

#[test]
fn titre_triable_et_premiere_lettre() {
    let f = matrix();
    assert_eq!(
        fichier_film("${titleSortable} (${year})", &f, None),
        "Matrix, The (1999)"
    );
    assert_eq!(
        dossier_film("${title;first}/${title} (${year})", &f),
        "T/The Matrix (1999)"
    );
    let mut m = matrix();
    m.titre = "12 Monkeys".into();
    m.annee = 1995;
    assert_eq!(
        dossier_film("${title;first}/${title} (${year})", &m),
        "#/12 Monkeys (1995)"
    );
    assert_eq!(
        dossier_film("${decadeShort}/${title} (${year})", &f),
        "1990s/The Matrix (1999)"
    );
    let p = crate::scan::titre::prefixes_titre_defaut();
    assert_eq!(titre_triable("L'Aventure", &p), "Aventure, L'");
    assert_eq!(titre_triable("Die Hard", &p), "Die Hard");
    assert_eq!(titre_triable("LA Story", &p), "LA Story");
}

#[test]
fn identifiants_annee_nulle_et_majuscules() {
    let mut f = matrix();
    assert_eq!(
        fichier_film("${title} (${year}) [${imdb}]", &f, None),
        "The Matrix (1999)"
    );
    f.ids.definir("imdb", "tt0133093");
    f.ids.definir("tmdb", "603");
    assert_eq!(
        fichier_film("${title} (${year}) [${imdb}] ${tmdb}", &f, None),
        "The Matrix (1999) [tt0133093] 603"
    );
    assert_eq!(
        fichier_film("${title;upper} (${year})", &f, None),
        "THE MATRIX (1999)"
    );
    f.annee = 0;
    assert_eq!(fichier_film("${title} (${year})", &f, None), "The Matrix");
}

#[test]
fn listes_notes_et_dates() {
    let mut f = matrix();
    f.genres = vec!["Action".into(), "Sci-Fi".into()];
    f.notes
        .insert("imdb".into(), Note::nouvelle("imdb", 8.7, 1000, 10));
    f.date_sortie = Some("1999-03-31".into());
    f.equipe.push(Personne {
        type_personne: TypePersonne::Realisateur,
        nom: "Lana Wachowski".into(),
        ..Default::default()
    });
    assert_eq!(
        fichier_film("${title} ${genres;array( - )}", &f, None),
        "The Matrix Action - Sci-Fi"
    );
    assert_eq!(
        fichier_film("${title} ${rating}", &f, None),
        "The Matrix 8.7"
    );
    assert_eq!(
        fichier_film("${title} ${rating;number(%.2f)}", &f, None),
        "The Matrix 8.70"
    );
    assert_eq!(
        fichier_film("${title} ${releaseDate}", &f, None),
        "The Matrix 1999-03-31"
    );
    assert_eq!(
        fichier_film("${title} ${directors[0].name}", &f, None),
        "The Matrix Lana Wachowski"
    );
    assert_eq!(
        fichier_film("${title} ${genresAsString}", &f, None),
        "The Matrix Action, Sci-Fi"
    );
}

#[test]
fn technique() {
    let mut f = matrix();
    f.fichiers[0].video.format_hdr = "HDR10".into();
    f.fichiers[0].video.format_3d = "3D SBS".into();
    f.fichiers[0].video.images_par_seconde = 23.976;
    f.source_media = SourceMedia::Bluray;
    assert_eq!(
        fichier_film(
            "${title} ${videoResolution} ${videoCodec} ${audioChannels} ${audioChannelsDot} ${hdr} ${hdrformat} ${3Dformat} ${framerate} ${mediaSource}",
            &f,
            None
        ),
        "The Matrix 1920x1080 h264 6ch 5.1 HDR HDR10 3D SBS 23.976 Blu-ray"
    );
    assert_eq!(
        fichier_film("${title} ${filesize}", &f, None),
        "The Matrix 4.37 GB"
    );
    assert_eq!(
        fichier_film("${title} ${audioLanguage}", &f, None),
        "The Matrix eng"
    );
}

#[test]
fn saga_et_index() {
    let mut f = matrix();
    f.saga_id = Some("m".into());
    let mut s = Saga {
        id: "m".into(),
        titre: "Matrix Collection".into(),
        films: vec!["1".into(), "2".into()],
        ..Default::default()
    };
    let g = "${if movieSet.title}${movieSet.title}/${end}${title} (${year}) ${movieSetIndex}";
    let profil = ProfilRenommageFilm {
        gabarit_dossier: g.into(),
        ..Default::default()
    };
    assert_eq!(
        nom_dossier_film(&f, Some(&s), &profil).unwrap(),
        "Matrix Collection/The Matrix (1999) 1"
    );
    // Saga d'un seul film : `movieSet` absent sauf option.
    s.films = vec!["1".into()];
    assert_eq!(
        nom_dossier_film(&f, Some(&s), &profil).unwrap(),
        "The Matrix (1999) 1"
    );
    let profil = ProfilRenommageFilm {
        saga_pour_film_seul: true,
        ..profil
    };
    assert_eq!(
        nom_dossier_film(&f, Some(&s), &profil).unwrap(),
        "Matrix Collection/The Matrix (1999) 1"
    );
}

#[test]
fn separateurs_ascii_espaces_et_parent() {
    let mut f = matrix();
    f.titre = "AC/DC Léon".into();
    let mut profil = ProfilRenommageFilm {
        gabarit_fichier: "${title} (${year})".into(),
        ascii: true,
        ..Default::default()
    };
    assert_eq!(
        nom_fichier_film(&f, None, &profil).unwrap(),
        "AC DC Leon (1999)"
    );
    profil.remplacer_espaces_fichier = true;
    profil.remplacement_espaces_fichier = ".".into();
    assert_eq!(
        nom_fichier_film(&f, None, &profil).unwrap(),
        "AC.DC.Leon.(1999)"
    );
    let mut f = matrix();
    f.chemin = p("/films/A/B/Matrix");
    assert_eq!(dossier_film("${parent}/${title}", &f), "A/B/The Matrix");
}

#[test]
fn acces_direct_au_modele() {
    let f = matrix();
    let ctx = contexte_film(&f, None, false, &[]);
    let r = creer_destination(
        "${movie.title} ${movie.runtime}",
        &ctx,
        jetons_film(),
        FormeDestination::Fichier,
        &ProfilRenommageFilm::default().options_destination(),
    )
    .unwrap();
    assert_eq!(r, "The Matrix");
}

#[test]
fn contexte_episode_numeros() {
    let mut e = episode("1", 1, 2, "Pilot", vec![]);
    e.numeros.push(NumeroEpisode {
        groupe: GroupeEpisodes::absolu(),
        saison: 1,
        episode: 14,
    });
    e.premiere_diffusion = Some("2008-01-20".into());
    let s = serie(vec![e.clone()]);
    let profil = ProfilRenommageSerie {
        gabarit_fichier:
            "${showTitle} ${seasonNr}x${episodeNr2} #${absoluteNr2} ${airedDate} ${year} ${title}"
                .into(),
        ..Default::default()
    };
    assert_eq!(
        nom_fichier_episodes(&s, &[&e], &profil).unwrap(),
        "Show 1x02 #14 2008-01-20 2008 Pilot"
    );
}
