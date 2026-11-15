//! Tests de la correspondance épisodes locaux ↔ épisodes scrapés.

use std::path::Path;

use crate::modele::{
    Episode, FichierMedia, GroupeEpisodes, NumeroEpisode, Saison, Serie, TypeEpisode, TypeFichier,
};
use crate::series::correspondance::*;
use crate::series::organisation::*;

fn num(groupe: GroupeEpisodes, saison: i32, episode: i32) -> NumeroEpisode {
    NumeroEpisode {
        groupe,
        saison,
        episode,
    }
}

/// Épisode local avec un fichier vidéo, numéroté dans l'ordre de diffusion.
fn local(saison: i32, episode: i32, nom: &str) -> Episode {
    Episode {
        numeros: vec![num(GroupeEpisodes::diffusion(), saison, episode)],
        nom_fichier_origine: format!("{nom}.mkv"),
        fichiers: vec![FichierMedia::nouveau(
            Path::new(&format!("/s/Show/{nom}.mkv")),
            TypeFichier::Video,
        )],
        ..Episode::default()
    }
}

/// Épisode scrapé : `(saison, épisode)` en diffusion, DVD, absolu.
fn scrape(titre: &str, diff: (i32, i32), dvd: (i32, i32), abs: i32, date: &str) -> EpisodeScrape {
    EpisodeScrape {
        numeros: vec![
            num(GroupeEpisodes::diffusion(), diff.0, diff.1),
            num(GroupeEpisodes::dvd(), dvd.0, dvd.1),
            num(GroupeEpisodes::absolu(), 1, abs),
        ],
        titre: titre.into(),
        resume: format!("Résumé de {titre}"),
        premiere_diffusion: Some(date.into()),
        ..EpisodeScrape::default()
    }
}

/// Série dont l'ordre DVD diffère de l'ordre de diffusion (épisodes 2 et 3
/// inversés en DVD), plus un spécial et un épisode futur.
fn liste_scrapee() -> Vec<EpisodeScrape> {
    vec![
        scrape("Pilot", (1, 1), (1, 1), 1, "2020-01-01"),
        scrape("Deux", (1, 2), (1, 3), 2, "2020-01-08"),
        scrape("Trois", (1, 3), (1, 2), 3, "2020-01-15"),
        scrape("Quatre", (1, 4), (1, 4), 4, "2020-01-22"),
        scrape("Cinq", (2, 1), (2, 1), 5, "2021-01-01"),
        scrape("Futur", (2, 2), (2, 2), 6, "2999-01-01"),
        EpisodeScrape {
            numeros: vec![num(GroupeEpisodes::diffusion(), 0, 1)],
            titre: "Spécial".into(),
            premiere_diffusion: Some("2020-12-25".into()),
            ..EpisodeScrape::default()
        },
    ]
}

#[test]
fn similarite_des_titres() {
    assert_eq!(similarite("Le Pilote", "le.pilote"), 1.0);
    assert_eq!(similarite("Pilot!", "pilot"), 1.0);
    assert!(similarite("Pilot", "Pilote") > 0.8);
    assert!(similarite("Pilot", "Finale") < 0.5);
    assert_eq!(similarite("", "x"), 0.0);
}

#[test]
fn choix_de_l_ordre_par_comptage() {
    let scrapes = liste_scrapee();
    // Fichiers en numérotation absolue : 1x05 et 1x06 n'existent qu'en absolu.
    let locaux: Vec<Episode> = (1..=6)
        .map(|n| local(1, n, &format!("Show {n:02}")))
        .collect();
    assert_eq!(
        choisir_meilleur_groupe(&locaux, &scrapes, &[]),
        Some(GroupeEpisodes::absolu())
    );
    // Fichiers en numérotation de diffusion : égalité de compte entre
    // diffusion et DVD, départagée par les titres puis par l'ordre (diffusion).
    let locaux = vec![
        local(1, 1, "Pilot"),
        local(1, 2, "Deux"),
        local(1, 3, "Trois"),
    ];
    assert_eq!(
        choisir_meilleur_groupe(&locaux, &scrapes, &[]),
        Some(GroupeEpisodes::diffusion())
    );
    // Les titres désignent l'ordre DVD (S01E02 = « Trois » en DVD).
    let mut locaux = vec![local(1, 2, "x"), local(1, 3, "y")];
    locaux[0].titre = "Trois".into();
    locaux[1].titre = "Deux".into();
    assert_eq!(
        choisir_meilleur_groupe(&locaux, &scrapes, &[]),
        Some(GroupeEpisodes::dvd())
    );
    // Aucun numéro commun.
    assert_eq!(
        choisir_meilleur_groupe(&[local(9, 9, "x")], &scrapes, &[]),
        None
    );
}

#[test]
fn appariement_et_manquants() {
    let scrapes = liste_scrapee();
    let mut non_numerote = local(-1, -1, "Bonus");
    non_numerote.numeros.clear();
    let locaux = vec![
        local(1, 1, "a"),
        local(1, 3, "b"),
        local(5, 5, "c"),
        non_numerote,
    ];
    let c = apparier_episodes(&locaux, &scrapes, &GroupeEpisodes::diffusion());
    assert_eq!(c.paires, vec![(0, 0), (1, 2)]);
    assert_eq!(c.locaux_orphelins, vec![2, 3]);
    assert_eq!(c.manquants, vec![1, 3, 4, 5, 6]);

    // En ordre DVD, le S01E03 local correspond à « Deux ».
    let c = apparier_episodes(&locaux, &scrapes, &GroupeEpisodes::dvd());
    assert_eq!(c.paires, vec![(0, 0), (1, 1)]);
    // Le spécial n'a pas de numéro DVD : ni apparié, ni manquant.
    assert!(!c.manquants.contains(&6));
}

#[test]
fn application_des_metadonnees() {
    let s = &liste_scrapee()[1];
    let mut e = local(1, 2, "x");
    e.titre = "Titre local".into();
    appliquer_scrape_episode(&mut e, s, false);
    assert_eq!(e.titre, "Titre local", "pas d'écrasement");
    assert_eq!(e.resume, "Résumé de Deux");
    assert_eq!(e.premiere_diffusion.as_deref(), Some("2020-01-08"));
    assert_eq!(e.numeros.len(), 3, "un numéro par ordre");
    assert_eq!(e.saison(&GroupeEpisodes::dvd()), 1);
    assert_eq!(e.episode(&GroupeEpisodes::dvd()), 3);
    appliquer_scrape_episode(&mut e, s, true);
    assert_eq!(e.titre, "Deux");
}

#[test]
fn mise_a_jour_complete_et_affichage() {
    let mut serie = Serie {
        titre: "Show".into(),
        groupe_actif: GroupeEpisodes::diffusion(),
        episodes: vec![
            local(1, 1, "Show - Pilot"),
            local(1, 2, "Show - Deux"),
            local(2, 1, "Show - Cinq"),
        ],
        saisons: vec![Saison {
            numero: 3,
            titre: "Saison à venir".into(),
            ..Saison::default()
        }],
        ..Serie::default()
    };
    let c = mettre_a_jour_depuis_scrape(&mut serie, &liste_scrapee(), &[], false);
    assert_eq!(serie.groupe_actif, GroupeEpisodes::diffusion());
    assert_eq!(c.paires.len(), 3);
    assert_eq!(serie.episodes_fictifs.len(), 7);
    assert!(
        serie
            .episodes_fictifs
            .iter()
            .all(|e| e.fictif && e.est_fictif())
    );
    assert_eq!(serie.episodes[0].titre, "Pilot");
    assert!(serie.groupes.contains(&GroupeEpisodes::dvd()));
    assert!(serie.groupes.contains(&GroupeEpisodes::absolu()));

    // Saisons : 0 (spécial fictif), 1, 2, et 3 conservée (titre).
    let saisons: Vec<i32> = serie.saisons.iter().map(|s| s.numero).collect();
    assert_eq!(saisons, vec![0, 1, 2, 3]);
    assert_eq!(
        serie.saisons[1].premiere_diffusion.as_deref(),
        Some("2020-01-01")
    );

    // Manquants : Trois, Quatre, Futur, Spécial.
    let manquants: Vec<&str> = episodes_manquants(&serie)
        .iter()
        .map(|e| e.titre.as_str())
        .collect();
    assert_eq!(manquants, vec!["Spécial", "Trois", "Quatre", "Futur"]);

    let aujourd_hui = Some("2026-11-15");
    let titres = |r: ReglagesFictifs| -> Vec<String> {
        episodes_a_afficher(&serie, r, aujourd_hui)
            .iter()
            .map(|e| e.titre.clone())
            .collect()
    };
    assert_eq!(
        titres(ReglagesFictifs::default()),
        vec!["Pilot", "Deux", "Cinq"]
    );
    assert_eq!(
        titres(ReglagesFictifs {
            afficher_manquants: true,
            ..Default::default()
        }),
        vec!["Pilot", "Deux", "Trois", "Quatre", "Cinq"]
    );
    assert_eq!(
        titres(ReglagesFictifs {
            afficher_manquants: true,
            afficher_speciaux_manquants: true,
            afficher_non_diffuses: true,
        }),
        vec![
            "Spécial", "Pilot", "Deux", "Trois", "Quatre", "Cinq", "Futur"
        ]
    );
    let futur = serie
        .episodes_fictifs
        .iter()
        .find(|e| e.titre == "Futur")
        .unwrap();
    assert_eq!(
        futur.type_episode(&serie.groupe_actif, aujourd_hui),
        TypeEpisode::FictifNonDiffuse
    );
}

#[test]
fn mise_a_jour_choisit_l_ordre_absolu() {
    let mut serie = Serie {
        episodes: (1..=6)
            .map(|n| local(1, n, &format!("Show {n:02}")))
            .collect(),
        ..Serie::default()
    };
    mettre_a_jour_depuis_scrape(&mut serie, &liste_scrapee(), &[], true);
    assert_eq!(serie.groupe_actif, GroupeEpisodes::absolu());
    // Les numéros de diffusion réels sont repris du scrape.
    let e5 = serie.episodes.iter().find(|e| e.titre == "Cinq").unwrap();
    assert_eq!(e5.saison(&GroupeEpisodes::diffusion()), 2);
    assert_eq!(e5.episode(&GroupeEpisodes::absolu()), 5);
    assert_eq!(
        serie.saisons.iter().map(|s| s.numero).collect::<Vec<_>>(),
        vec![1]
    );
    let manquants: Vec<&str> = episodes_manquants(&serie)
        .iter()
        .map(|e| e.titre.as_str())
        .collect();
    assert_eq!(
        manquants,
        vec!["Spécial"],
        "le spécial n'a pas de numéro absolu"
    );

    // Retour manuel à l'ordre de diffusion : saisons recalculées.
    definir_groupe_actif(&mut serie, GroupeEpisodes::diffusion());
    assert_eq!(
        serie.saisons.iter().map(|s| s.numero).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn choix_manuel() {
    let candidats = liste_scrapee();
    assert_eq!(choisir_episode_manuel("Quatre", "", &candidats), Some(3));
    assert_eq!(
        choisir_episode_manuel("Show S01E02", "trois", &candidats),
        Some(2)
    );
    assert_eq!(choisir_episode_manuel("x", "Pilote", &candidats), Some(0));
    assert_eq!(choisir_episode_manuel("x", "Rien à voir", &candidats), None);
}
