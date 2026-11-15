//! Tests du modèle de données.

use super::*;
use std::path::Path;

#[test]
fn identifiants_regles() {
    let mut ids = Identifiants::new();
    assert!(ids.definir("tmdb", "603"));
    assert_eq!(ids.obtenir("tmdb"), Some(&ValeurId::Entier(603)));
    assert_eq!(ids.tmdb(), 603);
    assert!(!ids.definir("imdb", "abc"));
    assert!(ids.definir("imdb", "tt0133093"));
    assert_eq!(ids.imdb(), "tt0133093");
    assert!(ids.definir("tmdb", "0"));
    assert!(!ids.contient("tmdb"));
    ids.definir("tvdb", "null");
    assert!(!ids.contient("tvdb"));
    assert!(!ids.definir_si_absent("imdb", "tt9999999"));
    ids.definir("trakt", "  12 ");
    assert_eq!(ids.trakt(), 12);
    assert!(ids.definir("autre", "abc12"));
    assert_eq!(ids.obtenir("autre"), Some(&ValeurId::Texte("abc12".into())));
}

#[test]
fn editions() {
    assert_eq!(
        Edition::depuis_chaine("Alien Director's Cut"),
        Edition::DirectorsCut
    );
    assert_eq!(
        Edition::depuis_chaine("Film.Extended.Edition"),
        Edition::Etendue
    );
    assert_eq!(Edition::depuis_chaine("IMAX"), Edition::Imax);
    assert_eq!(
        Edition::depuis_chaine("Film {edition-Version Longue}"),
        Edition::Personnalisee("Version Longue".into())
    );
    assert_eq!(Edition::depuis_chaine("Film normal"), Edition::Aucune);
    assert_eq!(Edition::depuis_chaine("Unrated"), Edition::NonClassee);
    assert!(Edition::est_motif_edition("Director's.Cut"));
    assert!(!Edition::est_motif_edition("2009.3D.Extended"));
    assert_eq!(Edition::DirectorsCut.libelle(), "Director's Cut");
}

#[test]
fn sources_media() {
    assert_eq!(
        SourceMedia::depuis_chemin("Film (2000)/Film.2000.BluRay.x264.mkv"),
        SourceMedia::Bluray
    );
    assert_eq!(
        SourceMedia::depuis_chemin("Film/Film.UHD.BluRay.mkv"),
        SourceMedia::UhdBluray
    );
    assert_eq!(
        SourceMedia::depuis_chemin("Film WEB-DL/f.mkv"),
        SourceMedia::WebDl
    );
    assert_eq!(
        SourceMedia::depuis_chemin("Film/BDMV/index.bdmv"),
        SourceMedia::Bluray
    );
    assert_eq!(
        SourceMedia::depuis_chemin("Film/VIDEO_TS/VIDEO_TS.IFO"),
        SourceMedia::Dvd
    );
    assert_eq!(
        SourceMedia::depuis_chemin("Film/Film.HDTV.mkv"),
        SourceMedia::Tv
    );
    // L'extension `.ts` n'est pas la source « telesync ».
    assert_eq!(
        SourceMedia::depuis_chemin("Film/film.ts"),
        SourceMedia::Inconnue
    );
    assert_eq!(
        SourceMedia::depuis_chemin("Film/film.mkv"),
        SourceMedia::Inconnue
    );
}

#[test]
fn groupes_episodes_egalite() {
    let a = GroupeEpisodes {
        type_groupe: TypeGroupe::Diffusion,
        nom: "x".into(),
    };
    assert_eq!(a, GroupeEpisodes::diffusion());
    let b = GroupeEpisodes {
        type_groupe: TypeGroupe::Alternatif,
        nom: "x".into(),
    };
    let c = GroupeEpisodes {
        type_groupe: TypeGroupe::Alternatif,
        nom: "y".into(),
    };
    assert_ne!(b, c);
    assert_ne!(GroupeEpisodes::dvd(), GroupeEpisodes::diffusion());
}

#[test]
fn types_d_episode() {
    let g = GroupeEpisodes::diffusion();
    let mut e = Episode {
        numeros: vec![NumeroEpisode {
            groupe: g.clone(),
            saison: 1,
            episode: 2,
        }],
        ..Episode::default()
    };
    assert_eq!(
        e.type_episode(&g, Some("2026-01-01")),
        TypeEpisode::FictifNonDiffuse
    );
    e.premiere_diffusion = Some("2020-01-01".into());
    assert_eq!(
        e.type_episode(&g, Some("2026-01-01")),
        TypeEpisode::FictifNormal
    );
    e.fichiers.push(FichierMedia::nouveau(
        Path::new("/s/e.mkv"),
        TypeFichier::Video,
    ));
    assert_eq!(e.type_episode(&g, None), TypeEpisode::Normal);
    e.numeros[0].saison = 0;
    assert_eq!(e.type_episode(&g, None), TypeEpisode::Special);
    assert_eq!(e.saison(&GroupeEpisodes::dvd()), -1);
    assert_eq!(
        e.type_episode(&GroupeEpisodes::dvd(), None),
        TypeEpisode::NonCategorise
    );
}

#[test]
fn fichier_media_identite_et_tri() {
    let a = FichierMedia::nouveau(Path::new("/s/F/Film.mkv"), TypeFichier::Video);
    let mut b = FichierMedia::nouveau(Path::new("/s/F/Film.mkv"), TypeFichier::Nfo);
    b.taille = 5;
    assert_eq!(a, b);
    assert_eq!(a.extension(), "mkv");
    assert_eq!(a.nom_de_base(), "Film");
    assert!(FichierMedia::nouveau(Path::new("/s/x.ISO"), TypeFichier::Video).est_iso);
    assert!(FichierMedia::nouveau(Path::new("/s/x.r01"), TypeFichier::Inconnu).est_archive());
    assert!(FichierMedia::nouveau(Path::new("/s/x.mkv"), TypeFichier::Extra).est_video());
    assert!(!FichierMedia::nouveau(Path::new("/s/x.jpg"), TypeFichier::Extra).est_video());

    let mut v = [
        FichierMedia::nouveau(Path::new("/s/Film.srt"), TypeFichier::SousTitres),
        FichierMedia::nouveau(Path::new("/s/Film.fr.srt"), TypeFichier::SousTitres),
        FichierMedia::nouveau(Path::new("/s/b.mkv"), TypeFichier::Video),
        FichierMedia::nouveau(Path::new("/s/a.mkv"), TypeFichier::Video),
    ];
    v.sort_by(comparer_fichiers_film);
    let noms: Vec<&str> = v.iter().map(|f| f.nom.as_str()).collect();
    assert_eq!(noms, vec!["a.mkv", "b.mkv", "Film.fr.srt", "Film.srt"]);
    assert!(TypeFichier::Poster.est_graphique());
    assert!(!TypeFichier::Nfo.est_graphique());
}

#[test]
fn film_derives() {
    let mut f = Film::nouveau();
    assert_eq!(f.decennie_courte(), "");
    f.annee = 1979;
    assert_eq!(f.decennie_courte(), "1970s");
    assert_eq!(f.decennie_longue(), "1970-1979");
    assert!(f.fichier_principal().is_none());

    let mut gros = FichierMedia::nouveau(Path::new("/s/gros.mkv"), TypeFichier::Video);
    gros.taille = 100;
    let mut petit = FichierMedia::nouveau(Path::new("/s/petit.mkv"), TypeFichier::Video);
    petit.taille = 10;
    f.fichiers = vec![petit, gros];
    assert_eq!(f.fichier_principal().unwrap().nom, "gros.mkv");

    f.notes
        .insert("user".into(), Note::nouvelle("user", 8.0, 0, 10));
    assert!(f.note_principale().is_none());
    f.notes
        .insert("imdb".into(), Note::nouvelle("imdb", 7.5, 100, 10));
    f.notes
        .insert("tmdb".into(), Note::nouvelle("tmdb", 80.0, 10, 100));
    let n = f.note_principale().unwrap();
    assert_eq!(n.id, "tmdb");
    assert!((n.sur_dix() - 8.0).abs() < 1e-6);

    f.equipe.push(Personne {
        type_personne: TypePersonne::Realisateur,
        nom: "R".into(),
        ..Personne::default()
    });
    f.equipe.push(Personne {
        type_personne: TypePersonne::Scenariste,
        nom: "S".into(),
        ..Personne::default()
    });
    assert_eq!(f.realisateurs().len(), 1);
    assert_eq!(f.scenaristes().len(), 1);
    assert!(f.producteurs().is_empty());
}

#[test]
fn serde_aller_retour() {
    let mut f = Film::nouveau();
    f.titre = "The Matrix".into();
    f.annee = 1999;
    f.ids.definir("imdb", "tt0133093");
    f.ids.definir("tmdb", "603");
    f.edition = Edition::Personnalisee("Spéciale".into());
    f.source_media = SourceMedia::Bluray;
    f.urls_artwork
        .insert(TypeFichier::Poster, "https://exemple/p.jpg".into());
    f.notes
        .insert("imdb".into(), Note::nouvelle("imdb", 8.7, 1000, 10));
    let mut v = FichierMedia::nouveau(Path::new("/s/F/f.mkv"), TypeFichier::Video);
    v.flux_audio.push(FluxAudio {
        codec: "ac3".into(),
        canaux: 6,
        drapeaux: vec![DrapeauFlux::Defaut],
        ..FluxAudio::default()
    });
    v.sous_titres.push(FluxSousTitres {
        langue: "fra".into(),
        drapeaux: vec![DrapeauFlux::Force],
        ..FluxSousTitres::default()
    });
    f.fichiers.push(v);

    let json = serde_json::to_string(&f).unwrap();
    assert!(json.contains("\"imdb\":\"tt0133093\""));
    assert!(json.contains("\"tmdb\":603"));
    let g: Film = serde_json::from_str(&json).unwrap();
    assert_eq!(f, g);
    assert_eq!(g.fichiers[0].flux_audio[0].canaux, 6);

    // Champs absents : valeurs par défaut.
    let h: Film = serde_json::from_str(r#"{"titre":"X"}"#).unwrap();
    assert_eq!(h.titre, "X");
    assert_eq!(h.annee, 0);

    let s = Serie {
        titre: "S".into(),
        groupe_actif: GroupeEpisodes::dvd(),
        ..Serie::default()
    };
    let s2: Serie = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(s, s2);
    let sa: Saga = serde_json::from_str(r#"{"titre":"Saga"}"#).unwrap();
    assert_eq!(sa.titre, "Saga");
}
