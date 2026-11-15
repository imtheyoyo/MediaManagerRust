//! Tests du module NFO : allers-retours écriture → lecture, NFO réels
//! abrégés, NFO bruités, multi-épisodes, variantes de connecteurs, noms.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::*;
use crate::modele::commun::cles_id;
use crate::modele::{
    BandeAnnonce, Edition, Episode, FichierMedia, Film, FluxAudio, FluxSousTitres, GroupeEpisodes,
    Note, NumeroEpisode, Personne, Saga, Saison, Serie, SourceMedia, StatutDiffusion, TypeFichier,
    TypeGroupe, TypePersonne,
};

const DATE_AJOUT: i64 = 1_600_000_000_000;

fn options() -> OptionsNfo {
    OptionsNfo {
        creer_resume_court: false,
        ..OptionsNfo::films()
    }
}

fn personne(t: TypePersonne, nom: &str, role: &str, tmdb: &str) -> Personne {
    let mut p = Personne {
        type_personne: t,
        nom: nom.to_string(),
        role: role.to_string(),
        ..Personne::default()
    };
    p.ids.definir(cles_id::TMDB, tmdb);
    p
}

fn video(chemin: &str) -> FichierMedia {
    let mut f = FichierMedia::nouveau(&PathBuf::from(chemin), TypeFichier::Video);
    f.video.codec = "h265".into();
    f.video.largeur = 3840;
    f.video.hauteur = 2160;
    f.video.ratio_aspect = 1.78;
    f.video.format_hdr = "Dolby Vision".into();
    f.duree_secondes = 8160;
    f.flux_audio.push(FluxAudio {
        codec: "TrueHD/Atmos".into(),
        langue: "eng".into(),
        canaux: 8,
        ..FluxAudio::default()
    });
    f.sous_titres.push(FluxSousTitres {
        langue: "fre".into(),
        ..FluxSousTitres::default()
    });
    f.sommes_controle.insert("crc32".into(), "ABCD1234".into());
    f
}

fn film_complet() -> Film {
    let mut f = Film {
        titre: "Matrix & Co <test>".into(),
        titre_original: "The Matrix".into(),
        titre_tri: "Matrix".into(),
        titre_anglais: "The Matrix".into(),
        annee: 1999,
        resume: "Ligne 1.\nLigne 2.".into(),
        synopsis: "Court.".into(),
        slogan: "Bienvenue dans le réel.".into(),
        date_ajout: DATE_AJOUT,
        societe_production: "Warner Bros. / Village Roadshow".into(),
        note_utilisateur: "à revoir".into(),
        duree_minutes: 136,
        vu: true,
        nombre_lectures: 3,
        derniere_lecture: Some(DATE_AJOUT + 86_400_000),
        langues_parlees: "en, fr".into(),
        pays: "United States/Australia".into(),
        date_sortie: Some("1999-03-31".into()),
        top250: 16,
        source_media: SourceMedia::UhdBluray,
        certification: "US:R".into(),
        edition: Edition::DirectorsCut,
        genres: vec!["Action".into(), "Science Fiction".into()],
        tags: vec!["cyberpunk".into()],
        nom_fichier_origine: "matrix.mkv".into(),
        verrouille: true,
        liens_series: vec!["Animatrix".into()],
        chemin: PathBuf::from("/films/Matrix (1999)"),
        ..Film::default()
    };
    f.ids.definir(cles_id::IMDB, "tt0133093");
    f.ids.definir(cles_id::TMDB, "603");
    f.ids.definir(cles_id::TMDB_SAGA, "2344");
    f.notes
        .insert("imdb".into(), Note::nouvelle("imdb", 8.7, 1_900_000, 10));
    f.notes
        .insert("tmdb".into(), Note::nouvelle("tmdb", 8.2, 24_000, 10));
    f.notes
        .insert("user".into(), Note::nouvelle("user", 9.0, 0, 10));
    f.urls_artwork
        .insert(TypeFichier::Poster, "https://img/poster.jpg".into());
    f.urls_artwork
        .insert(TypeFichier::Thumb, "https://img/landscape.jpg".into());
    f.urls_artwork
        .insert(TypeFichier::Fanart, "https://img/fanart.jpg".into());
    f.fanarts_extra = vec!["https://img/fanart2.jpg".into()];
    let mut a = personne(TypePersonne::Acteur, "Keanu Reeves", "Neo", "6384");
    a.url_vignette = "https://img/keanu.jpg".into();
    a.ids.definir(cles_id::IMDB, "tt0000206");
    f.acteurs.push(a);
    f.equipe.push(personne(
        TypePersonne::Realisateur,
        "Lana Wachowski",
        "Director",
        "9340",
    ));
    f.equipe.push(personne(
        TypePersonne::Scenariste,
        "Lilly Wachowski",
        "Screenplay",
        "9339",
    ));
    f.equipe.push(personne(
        TypePersonne::Producteur,
        "Joel Silver",
        "Producer",
        "1100",
    ));
    f.bandes_annonces.push(BandeAnnonce {
        url: "https://www.youtube.com/watch?v=vKQi3bBA1y8".into(),
        dans_nfo: true,
        ..BandeAnnonce::default()
    });
    f.fichiers.push(video("/films/Matrix (1999)/Matrix.mkv"));
    f
}

fn saga() -> Saga {
    let mut s = Saga {
        titre: "Matrix - Saga".into(),
        resume: "Trilogie.".into(),
        ..Saga::default()
    };
    s.ids.definir(cles_id::TMDB, "2344");
    s
}

// ---------------------------------------------------------------------------
// Films
// ---------------------------------------------------------------------------

#[test]
fn film_aller_retour_kodi() {
    let f = film_complet();
    let xml = ecrire_nfo_film(
        &f,
        Some(&saga()),
        &DonneesConservees::default(),
        Connecteur::Kodi,
        &options(),
    );
    let lu = lire_nfo_film(&xml);
    assert!(lu.valide);
    let g = lu.film;
    assert_eq!(g.titre, f.titre);
    assert_eq!(g.titre_original, f.titre_original);
    assert_eq!(g.titre_tri, f.titre_tri);
    assert_eq!(g.titre_anglais, f.titre_anglais);
    assert_eq!(g.annee, 1999);
    assert_eq!(g.resume, f.resume);
    assert_eq!(g.synopsis, f.synopsis);
    assert_eq!(g.slogan, f.slogan);
    assert_eq!(g.date_ajout, DATE_AJOUT);
    assert_eq!(g.societe_production, f.societe_production);
    assert_eq!(g.note_utilisateur, f.note_utilisateur);
    assert_eq!(g.duree_minutes, 136);
    assert!(g.vu);
    assert_eq!(g.nombre_lectures, 3);
    assert_eq!(g.derniere_lecture, f.derniere_lecture);
    assert_eq!(g.langues_parlees, "en, fr");
    assert_eq!(g.pays, f.pays);
    assert_eq!(g.date_sortie, f.date_sortie);
    assert_eq!(g.top250, 16);
    assert_eq!(g.source_media, SourceMedia::UhdBluray);
    assert_eq!(g.certification, "US:R");
    assert_eq!(g.edition, Edition::DirectorsCut);
    assert_eq!(g.genres, f.genres);
    assert_eq!(g.tags, f.tags);
    assert_eq!(g.nom_fichier_origine, f.nom_fichier_origine);
    assert!(g.verrouille);
    assert_eq!(g.liens_series, f.liens_series);
    assert_eq!(g.ids, f.ids);
    assert_eq!(g.notes, f.notes);
    assert_eq!(g.urls_artwork, f.urls_artwork);
    assert_eq!(g.fanarts_extra, f.fanarts_extra);
    assert_eq!(g.acteurs, f.acteurs);
    assert_eq!(g.equipe, f.equipe);
    assert_eq!(g.bandes_annonces.len(), 1);
    assert_eq!(g.bandes_annonces[0].url, f.bandes_annonces[0].url);
    let s = lu.saga.unwrap();
    assert_eq!(s.titre, "Matrix - Saga");
    assert_eq!(s.resume, "Trilogie.");
}

#[test]
fn film_format_kodi() {
    let xml = ecrire_nfo_film(
        &film_complet(),
        Some(&saga()),
        &DonneesConservees::default(),
        Connecteur::Kodi,
        &options(),
    );
    assert!(xml.starts_with(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<!--created by MediaManagerRust for KODI-->\r\n<movie>\r\n  <title>Matrix &amp; Co &lt;test&gt;</title>\r\n"
    ));
    // Toutes les fins de ligne sont CRLF.
    assert!(!xml.replace("\r\n", "").contains('\n'));
    assert!(xml.contains("<rating name=\"imdb\" max=\"10\" default=\"true\">"));
    assert!(xml.contains("<rating name=\"themoviedb\" max=\"10\" default=\"false\">"));
    assert!(xml.contains("<value>8.7</value>"));
    assert!(!xml.contains("name=\"user\""));
    assert!(xml.contains("<userrating>9</userrating>"));
    assert!(
        !xml.contains("<votes>1900000</votes>\r\n  <"),
        "pas de votes simples en Kodi"
    );
    assert!(xml.contains(
        "<set>\r\n    <name>Matrix - Saga</name>\r\n    <overview>Trilogie.</overview>\r\n  </set>"
    ));
    assert!(xml.contains("<thumb aspect=\"poster\">https://img/poster.jpg</thumb>"));
    assert!(xml.contains("<thumb aspect=\"landscape\">https://img/landscape.jpg</thumb>"));
    assert!(xml.contains("<fanart>\r\n    <thumb>https://img/fanart.jpg</thumb>\r\n    <thumb>https://img/fanart2.jpg</thumb>\r\n  </fanart>"));
    assert!(xml.contains("<uniqueid type=\"imdb\" default=\"true\">tt0133093</uniqueid>"));
    assert!(xml.contains("<uniqueid type=\"tmdb\">603</uniqueid>"));
    assert!(xml.contains("<id>tt0133093</id>"));
    assert!(xml.contains("<tmdbid>603</tmdbid>"));
    assert!(xml.contains("<country>United States</country>\r\n  <country>Australia</country>"));
    assert!(xml.contains("<director tmdbid=\"9340\">Lana Wachowski</director>"));
    assert!(xml.contains("<credits tmdbid=\"9339\">Lilly Wachowski</credits>"));
    assert!(xml.contains("<producer tmdbid=\"1100\">"));
    assert!(
        xml.contains("<trailer>plugin://plugin.video.youtube/play/?video_id=vKQi3bBA1y8</trailer>")
    );
    assert!(xml.contains("<dateadded>2020-09-13 12:26:40</dateadded>"));
    assert!(xml.contains("<codec>hevc</codec>"));
    assert!(xml.contains("<aspect>1.78</aspect>"));
    assert!(xml.contains("<resolution>4K</resolution>"));
    assert!(xml.contains("<hdrtype>dolbyvision</hdrtype>"));
    assert!(xml.contains("<codec>truehd_atmos</codec>"));
    assert!(xml.contains("<durationinseconds>8160</durationinseconds>"));
    assert!(xml.contains("<videoassettitle>Director's Cut</videoassettitle>"));
    assert!(xml.contains("<!--MediaManagerRust meta data-->"));
    assert!(xml.contains("<source>UHD_BLURAY</source>"));
    assert!(xml.contains("<edition>DIRECTORS_CUT</edition>"));
    assert!(xml.contains("<crc32>ABCD1234</crc32>"));
    assert!(xml.contains("<role subrole=\"Director\">Director</role>"));
    assert!(xml.contains("<locked>true</locked>"));

    // Ordre des balises principales.
    let ordre = [
        "<title>",
        "<originaltitle>",
        "<sorttitle>",
        "<year>",
        "<ratings>",
        "<userrating>",
        "<top250>",
        "<set>",
        "<plot>",
        "<outline>",
        "<tagline>",
        "<runtime>",
        "<thumb ",
        "<fanart>",
        "<mpaa>",
        "<certification>",
        "<id>",
        "<tmdbid>",
        "<uniqueid ",
        "<country>",
        "<premiered>",
        "<watched>",
        "<playcount>",
        "<lastplayed>",
        "<genre>",
        "<studio>",
        "<credits ",
        "<director ",
        "<tag>",
        "<actor>",
        "<producer ",
        "<trailer>",
        "<languages>",
        "<showlink>",
        "<dateadded>",
        "<fileinfo>",
        "<videoassettitle>",
        "<source>",
        "<edition>",
        "<original_filename>",
        "<user_note>",
        "<english_title>",
        "<crc32>",
        "<crew>",
        "<locked>",
    ];
    let mut pos = 0;
    for b in ordre {
        let i = xml[pos..]
            .find(b)
            .unwrap_or_else(|| panic!("balise {b} absente ou mal placée"));
        pos += i;
    }
}

#[test]
fn film_variantes_connecteurs() {
    let f = film_complet();
    let o = options();
    let vide = DonneesConservees::default();

    let emby = ecrire_nfo_film(&f, Some(&saga()), &vide, Connecteur::Emby, &o);
    assert!(emby.contains("<set tmdbcolid=\"2344\">"));
    assert!(emby.find("<actor>").unwrap() < emby.find("<director ").unwrap());

    let jelly = ecrire_nfo_film(&f, Some(&saga()), &vide, Connecteur::Jellyfin, &o);
    assert!(jelly.contains("<collectionnumber>2344</collectionnumber>"));
    assert!(!jelly.contains("<videoassettitle>"));
    assert!(!jelly.contains("<top250>"));

    let xbmc = ecrire_nfo_film(&f, Some(&saga()), &vide, Connecteur::Xbmc, &o);
    assert!(xbmc.contains("<rating>8.7</rating>"));
    assert!(xbmc.contains("<votes>1900000</votes>"));
    assert!(xbmc.contains("<set>Matrix - Saga</set>"));
    assert!(xbmc.contains("<thumb>https://img/poster.jpg</thumb>"));
    assert!(xbmc.contains("action=play_video&amp;videoid=vKQi3bBA1y8"));
    assert!(xbmc.contains("<codec>HEVC</codec>"));
    assert!(!xbmc.contains("<resolution>"));
    // Relu, le XBMC garde l'essentiel.
    let lu = lire_nfo_film(&xbmc);
    assert_eq!(lu.film.notes.get("NFO").map(|n| n.valeur), Some(8.7));
    assert_eq!(lu.saga.unwrap().titre, "Matrix - Saga");
    assert_eq!(lu.film.bandes_annonces[0].url, f.bandes_annonces[0].url);

    let mp = ecrire_nfo_film(&f, None, &vide, Connecteur::MediaPortal, &o);
    assert!(mp.contains("<genres>\r\n    <genre>Action</genre>"));
    assert!(!mp.contains("<mpaa>"));
    assert!(mp.contains("<country>United States/Australia</country>"));
    assert!(mp.contains("<languages>en|fr</languages>"));
    assert!(mp.contains("<director>Lana Wachowski</director>"));
    let lu = lire_nfo_film(&mp);
    assert_eq!(lu.film.genres, f.genres);
    assert_eq!(lu.film.pays, f.pays);

    let mv = ecrire_nfo_film(&f, None, &vide, Connecteur::MpMyVideo, &o);
    assert!(mv.contains("<imdb>tt0133093</imdb>"));
    assert!(mv.contains("<language>en|fr</language>"));

    let plex = ecrire_nfo_film(&f, None, &vide, Connecteur::Plex, &o);
    let kodi = ecrire_nfo_film(&f, None, &vide, Connecteur::Kodi, &o);
    assert_eq!(plex.replace("PLEX", "KODI"), kodi);
}

#[test]
fn film_resume_court_et_options() {
    let mut f = film_complet();
    f.synopsis.clear();
    f.resume = "Court. Une deuxième phrase longue. Trois.".into();
    let mut o = OptionsNfo::films();
    o.resume_court_premiere_phrase = true;
    o.ecrire_urls_artwork = false;
    o.ecrire_fileinfo = false;
    o.ecrire_bande_annonce = false;
    o.ecrire_date_ajout = false;
    o.ecrire_lockdata = true;
    o.studio_unique = true;
    o.horodatage = Some("2026-01-02 03:04:05".into());
    let xml = ecrire_nfo_film(
        &f,
        None,
        &DonneesConservees::default(),
        Connecteur::Kodi,
        &o,
    );
    assert!(xml.contains("<!--created on 2026-01-02 03:04:05 by MediaManagerRust for KODI-->"));
    assert!(xml.contains("<outline>Court. Une deuxième phrase longue.</outline>"));
    assert!(!xml.contains("<thumb"));
    assert!(!xml.contains("<fanart>"));
    assert!(!xml.contains("<fileinfo>"));
    assert!(!xml.contains("<trailer>"));
    assert!(!xml.contains("<dateadded>"));
    assert!(xml.contains("<lockdata>true</lockdata>"));
    assert_eq!(xml.matches("<studio>").count(), 1);
}

#[test]
fn film_balises_inconnues_conservees() {
    let ancien = r#"<?xml version="1.0"?>
<movie>
  <title>Ancien</title>
  <epbookmark>12.5</epbookmark>
  <status>Released</status>
  <code>ABC-123</code>
  <maBalise attr="x">valeur <b>gras</b></maBalise>
  <resume><position>10</position><total>100</total></resume>
</movie>"#;
    let lu = lire_nfo_film(ancien);
    assert_eq!(lu.conserves.epbookmark.as_deref(), Some("12.5"));
    assert_eq!(lu.conserves.status.as_deref(), Some("Released"));
    assert_eq!(lu.conserves.inconnues.len(), 2);
    let xml = ecrire_nfo_film(&lu.film, None, &lu.conserves, Connecteur::Kodi, &options());
    assert!(xml.contains("<maBalise attr=\"x\">valeur <b>gras</b></maBalise>"));
    assert!(xml.contains(
        "<resume>\r\n    <position>10</position>\r\n    <total>100</total>\r\n  </resume>"
    ));
    assert!(xml.find("<epbookmark>").unwrap() < xml.find("<year").unwrap());
    assert!(xml.find("<status>").unwrap() < xml.find("<premiered").unwrap());
    // Relecture : toujours conservées, une seule fois.
    let relu = lire_nfo_film(&xml);
    assert_eq!(relu.conserves, lu.conserves);
}

#[test]
fn film_nfo_reel_abrege() {
    let nfo = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes" ?>
<!--created on 2020-01-01 - un autre outil-->
<movie>
    <title>Le Fabuleux Destin d'Amélie Poulain</title>
    <originaltitle>Le Fabuleux Destin d'Amélie Poulain</originaltitle>
    <ratings>
        <rating name="imdb" max="10" default="true">
            <value>8.300000</value>
            <votes>750000</votes>
        </rating>
        <rating name="metascore" max="100"><value>69</value></rating>
    </ratings>
    <userrating>0</userrating>
    <top250>0</top250>
    <outline></outline>
    <plot>Amélie, une jeune serveuse dans un bar de Montmartre,
passe son temps à observer les gens.</plot>
    <runtime>122</runtime>
    <thumb spoof="" cache="" aspect="poster" preview="">https://image.example/p.jpg</thumb>
    <thumb aspect="clearlogo">https://image.example/logo.png</thumb>
    <thumb>/chemin/local.jpg</thumb>
    <fanart><thumb colors="" preview="">https://image.example/f.jpg</thumb></fanart>
    <mpaa>Tous publics</mpaa>
    <playcount>2</playcount>
    <lastplayed>2021-05-06</lastplayed>
    <id>tt0211915</id>
    <uniqueid type="imdb" default="true">tt0211915</uniqueid>
    <uniqueid type="tmdb">194</uniqueid>
    <genre>Comédie / Romance</genre>
    <country>France</country>
    <country>Allemagne</country>
    <set><name>Collection</name><overview></overview></set>
    <credits>Guillaume Laurant, Jean-Pierre Jeunet</credits>
    <director tmdbid="2419">Jean-Pierre Jeunet</director>
    <premiered>2001-04-25</premiered>
    <studio>UGC</studio>
    <trailer>plugin://plugin.video.youtube/?action=play_video&amp;videoid=HUECWi5pX7o</trailer>
    <actor>
        <name>Audrey Tautou</name>
        <role>Amélie Poulain</role>
        <order>0</order>
        <thumb>https://image.example/a.jpg</thumb>
    </actor>
    <tag>paris</tag>
    <dateadded>2019-11-30 18:00:00</dateadded>
</movie>
"#;
    let lu = lire_nfo_film(nfo);
    assert!(lu.valide);
    let f = &lu.film;
    assert_eq!(f.titre, "Le Fabuleux Destin d'Amélie Poulain");
    assert_eq!(f.notes["imdb"].valeur, 8.3);
    assert_eq!(f.notes["imdb"].votes, 750_000);
    assert_eq!(f.notes["metacritic"].maximum, 100);
    assert!(!f.notes.contains_key("user"));
    assert!(f.resume.contains("Montmartre,\npasse"));
    assert_eq!(f.synopsis, "");
    assert_eq!(f.duree_minutes, 122);
    assert_eq!(
        f.urls_artwork[&TypeFichier::Poster],
        "https://image.example/p.jpg"
    );
    assert_eq!(
        f.urls_artwork[&TypeFichier::Clearlogo],
        "https://image.example/logo.png"
    );
    assert_eq!(
        f.urls_artwork[&TypeFichier::Fanart],
        "https://image.example/f.jpg"
    );
    assert_eq!(f.certification, "Tous publics");
    assert!(f.vu);
    assert_eq!(f.nombre_lectures, 2);
    assert_eq!(f.ids.imdb(), "tt0211915");
    assert_eq!(f.ids.tmdb(), 194);
    assert_eq!(f.genres, vec!["Comédie", "Romance"]);
    assert_eq!(f.pays, "France/Allemagne");
    assert_eq!(lu.saga.as_ref().unwrap().titre, "Collection");
    // Pas de `crew` : l'équipe vient de credits / director.
    let noms: Vec<_> = f
        .equipe
        .iter()
        .map(|p| (p.type_personne, p.nom.as_str()))
        .collect();
    assert_eq!(
        noms,
        vec![
            (TypePersonne::Realisateur, "Jean-Pierre Jeunet"),
            (TypePersonne::Scenariste, "Guillaume Laurant"),
            (TypePersonne::Scenariste, "Jean-Pierre Jeunet"),
        ]
    );
    assert_eq!(f.equipe[0].ids.tmdb(), 2419);
    assert_eq!(f.equipe[1].ids.tmdb(), 0);
    assert_eq!(f.date_sortie.as_deref(), Some("2001-04-25"));
    assert_eq!(
        f.bandes_annonces[0].url,
        "https://www.youtube.com/watch?v=HUECWi5pX7o"
    );
    assert_eq!(f.acteurs[0].role, "Amélie Poulain");
    assert_eq!(f.date_ajout, utils_date("2019-11-30 18:00:00"));
    // `order` n'est pas une balise racine : rien à conserver.
    assert!(lu.conserves.inconnues.is_empty());
}

fn utils_date(s: &str) -> i64 {
    super::outils::date_vers_ms(s).unwrap()
}

#[test]
fn film_nfo_bruite() {
    // Texte avant/après, BOM, Windows-1252, & non échappé, balise non fermée,
    // titre en double (ignoré), casse mélangée.
    let mut octets = vec![0xEF, 0xBB, 0xBF];
    octets.extend_from_slice(
        b"Release info - ripped by someone\r\n<b>gras</b>\r\n<Movie>\r\n<Title>Tom & Jerry</Title>\r\n<TMDBID>12345</TMDBID>\r\n<plot>Un chat et une souris\r\n<year>1992</year>\r\n</movie>\r\nhttps://www.imdb.com/title/tt0105622/\r\n",
    );
    let lu = lire_nfo_film_octets(&octets);
    assert!(lu.valide);
    assert_eq!(lu.film.titre, "Tom & Jerry");
    assert_eq!(lu.film.ids.tmdb(), 12345);
    assert_eq!(lu.film.ids.imdb(), "tt0105622");
    assert!(lu.film.resume.starts_with("Un chat et une souris"));

    let latin1 = b"<movie><title>Am\xe9lie \x80</title></movie>";
    assert_eq!(lire_nfo_film_octets(latin1).film.titre, "Amélie €");

    let mut utf16 = vec![0xFF, 0xFE];
    for u in "<movie><title>Zoé</title></movie>".encode_utf16() {
        utf16.extend_from_slice(&u.to_le_bytes());
    }
    assert_eq!(lire_nfo_film_octets(&utf16).film.titre, "Zoé");

    let double = "<movie><title>A</title><title>B</title><year>2000</year></movie>";
    let lu = lire_nfo_film(double);
    assert!(!lu.valide);
    assert_eq!(lu.film.annee, 2000);
}

#[test]
fn film_nfo_url_ou_id() {
    let lu = lire_nfo_film("https://www.imdb.com/title/tt0133093/\n");
    assert!(!lu.valide);
    assert_eq!(lu.film.ids.imdb(), "tt0133093");
    let lu = lire_nfo_film("https://www.themoviedb.org/movie/603-the-matrix?language=fr");
    assert_eq!(lu.film.ids.tmdb(), 603);
    let lu = lire_nfo_film("tt0133093");
    assert_eq!(lu.film.ids.imdb(), "tt0133093");
    let lu = lire_nfo_film("");
    assert!(lu.film.ids.est_vide());
}

#[test]
fn film_recording() {
    let lu = lire_nfo_film(
        "<recording><title>Docu</title><description>Résumé</description><rating>TV-PG</rating><genre>Documentary</genre></recording>",
    );
    assert!(lu.valide);
    assert_eq!(lu.film.resume, "Résumé");
    assert_eq!(lu.film.certification, "TV-PG");
    assert_eq!(lu.film.genres, vec!["Documentary"]);
}

#[test]
fn film_ids_bloc_et_saga() {
    let nfo = r#"<movie><title>X</title>
<ids><entry><key>imdb</key><value>tt1234567</value></entry><trakt>42</trakt></ids>
<sets><set>Ma saga</set></sets><tmdbCollectionId>99</tmdbCollectionId>
<rating>6.5</rating><votes>1,200</votes><criticrating>87</criticrating>
<edition>Version spéciale maison</edition><source>bluray</source>
<languages>English, Français</languages>
</movie>"#;
    let lu = lire_nfo_film(nfo);
    let f = &lu.film;
    assert_eq!(f.ids.imdb(), "tt1234567");
    assert_eq!(f.ids.trakt(), 42);
    assert_eq!(
        f.ids
            .obtenir(cles_id::TMDB_SAGA)
            .and_then(|v| v.en_entier()),
        Some(99)
    );
    let s = lu.saga.unwrap();
    assert_eq!((s.titre.as_str(), s.tmdb_id), ("Ma saga", 99));
    assert_eq!(f.notes["NFO"].valeur, 6.5);
    assert_eq!(f.notes["NFO"].votes, 1200);
    assert_eq!(f.notes["tomatometerallcritics"].maximum, 100);
    assert_eq!(
        f.edition,
        Edition::Personnalisee("Version spéciale maison".into())
    );
    assert_eq!(f.source_media, SourceMedia::Bluray);
    assert_eq!(f.langues_parlees, "English, Français");
}

#[test]
fn contenu_equivalent_ignore_commentaires() {
    let f = film_complet();
    let mut o = options();
    let a = ecrire_nfo_film(
        &f,
        None,
        &DonneesConservees::default(),
        Connecteur::Kodi,
        &o,
    );
    o.horodatage = Some("2030-01-01 00:00:00".into());
    let b = ecrire_nfo_film(
        &f,
        None,
        &DonneesConservees::default(),
        Connecteur::Kodi,
        &o,
    );
    assert_ne!(a, b);
    assert!(contenu_equivalent(&a, &b));
    assert!(contenu_equivalent(&a, &a.replace("\r\n", "\n")));
    let mut g = f.clone();
    g.titre = "Autre".into();
    let c = ecrire_nfo_film(
        &g,
        None,
        &DonneesConservees::default(),
        Connecteur::Kodi,
        &o,
    );
    assert!(!contenu_equivalent(&a, &c));
}

#[test]
fn ecriture_disque_si_change() {
    let dir = tempfile::tempdir().unwrap();
    let chemin = dir.path().join("movie.nfo");
    let f = film_complet();
    let mut o = options();
    let a = ecrire_nfo_film(
        &f,
        None,
        &DonneesConservees::default(),
        Connecteur::Kodi,
        &o,
    );
    assert!(ecrire_si_change(&chemin, &a).unwrap());
    o.horodatage = Some("2030-01-01 00:00:00".into());
    let b = ecrire_nfo_film(
        &f,
        None,
        &DonneesConservees::default(),
        Connecteur::Kodi,
        &o,
    );
    assert!(!ecrire_si_change(&chemin, &b).unwrap());
    let lu = lire_fichier_nfo_film(&chemin).unwrap();
    assert_eq!(lu.film.titre, f.titre);
}

// ---------------------------------------------------------------------------
// Séries
// ---------------------------------------------------------------------------

fn serie_complete() -> Serie {
    let mut s = Serie {
        titre: "Breaking Bad".into(),
        titre_original: "Breaking Bad".into(),
        titre_tri: "Breaking Bad".into(),
        titre_anglais: "Breaking Bad".into(),
        slogan: "Remember my name".into(),
        resume: "Un professeur de chimie.".into(),
        annee: 2008,
        premiere_diffusion: Some("2008-01-20".into()),
        statut: StatutDiffusion::Terminee,
        duree_minutes: 47,
        certification: "US:TV-MA".into(),
        pays: "US".into(),
        societe_production: "Sony Pictures Television / High Bridge".into(),
        genres: vec!["Drama".into(), "Crime".into()],
        tags: vec!["chimie".into()],
        top250: 3,
        note_utilisateur: "culte".into(),
        date_ajout: DATE_AJOUT,
        verrouille: true,
        groupe_actif: GroupeEpisodes::diffusion(),
        groupes: vec![GroupeEpisodes::diffusion(), GroupeEpisodes::dvd()],
        ..Serie::default()
    };
    s.ids.definir(cles_id::TVDB, "81189");
    s.ids.definir(cles_id::IMDB, "tt0903747");
    s.ids.definir(cles_id::TMDB, "1396");
    s.notes
        .insert("imdb".into(), Note::nouvelle("imdb", 9.5, 2_000_000, 10));
    s.urls_artwork
        .insert(TypeFichier::Poster, "https://img/p.jpg".into());
    s.urls_artwork
        .insert(TypeFichier::Characterart, "https://img/c.png".into());
    s.urls_artwork
        .insert(TypeFichier::Fanart, "https://img/f.jpg".into());
    s.acteurs.push(personne(
        TypePersonne::Acteur,
        "Bryan Cranston",
        "Walter White",
        "17419",
    ));
    s.equipe.push(personne(
        TypePersonne::Realisateur,
        "Vince Gilligan",
        "Creator",
        "66633",
    ));
    s.saisons.push(Saison {
        numero: 1,
        titre: "Première saison".into(),
        resume: "Walter commence.".into(),
        ..Saison::default()
    });
    s.bandes_annonces.push(BandeAnnonce {
        url: "https://youtu.be/HhesaQXLuRY".into(),
        dans_nfo: true,
        ..BandeAnnonce::default()
    });
    s
}

#[test]
fn serie_aller_retour() {
    let s = serie_complete();
    let o = OptionsNfo::series();
    let xml = ecrire_nfo_serie(&s, &DonneesConservees::default(), Connecteur::Kodi, &o);
    let lu = lire_nfo_serie(&xml);
    assert!(lu.valide);
    let t = lu.serie;
    assert_eq!(t.titre, s.titre);
    assert_eq!(t.titre_original, s.titre_original);
    assert_eq!(t.titre_tri, s.titre_tri);
    assert_eq!(t.titre_anglais, s.titre_anglais);
    assert_eq!(t.slogan, s.slogan);
    assert_eq!(t.resume, s.resume);
    assert_eq!(t.annee, 2008);
    assert_eq!(t.premiere_diffusion, s.premiere_diffusion);
    assert_eq!(t.statut, StatutDiffusion::Terminee);
    assert_eq!(t.duree_minutes, 47);
    assert_eq!(t.certification, s.certification);
    assert_eq!(t.pays, s.pays);
    assert_eq!(t.societe_production, s.societe_production);
    assert_eq!(t.genres, s.genres);
    assert_eq!(t.tags, s.tags);
    assert_eq!(t.top250, 3);
    assert_eq!(t.note_utilisateur, s.note_utilisateur);
    assert_eq!(t.date_ajout, DATE_AJOUT);
    assert!(t.verrouille);
    assert_eq!(t.ids, s.ids);
    assert_eq!(t.notes, s.notes);
    assert_eq!(t.urls_artwork, s.urls_artwork);
    assert_eq!(t.acteurs, s.acteurs);
    assert_eq!(t.equipe, s.equipe);
    assert_eq!(t.saisons, s.saisons);
    assert_eq!(t.groupe_actif, s.groupe_actif);
    assert_eq!(t.groupes, s.groupes);
    assert_eq!(
        t.bandes_annonces[0].url,
        "https://www.youtube.com/watch?v=HhesaQXLuRY"
    );
    assert!(xml.contains(
        "<episodeguide>{\"imdb\":\"tt0903747\",\"tmdb\":\"1396\",\"tvdb\":\"81189\"}</episodeguide>"
    ));
    assert!(xml.contains("<uniqueid type=\"tvdb\" default=\"true\">81189</uniqueid>"));
    assert!(xml.contains("<namedseason number=\"1\">Première saison</namedseason>"));
    assert!(xml.contains("<thumb aspect=\"characterart\">"));
    assert!(xml.contains("<group id=\"AIRED\" name=\"\" active=\"true\"/>"));
    assert!(xml.contains("<id>81189</id>"));
    assert!(!xml.contains("<enddate>"));
}

#[test]
fn serie_date_fin_emby() {
    let mut s = serie_complete();
    let mut e = Episode {
        numeros: vec![NumeroEpisode {
            groupe: GroupeEpisodes::diffusion(),
            saison: 5,
            episode: 16,
        }],
        premiere_diffusion: Some("2013-09-29".into()),
        ..Episode::default()
    };
    s.episodes.push(e.clone());
    e.numeros[0].saison = 0;
    e.premiere_diffusion = Some("2015-01-01".into());
    s.episodes.push(e);
    let mut o = OptionsNfo::series();
    o.ecrire_date_fin = true;
    let xml = ecrire_nfo_serie(&s, &DonneesConservees::default(), Connecteur::Emby, &o);
    assert!(xml.contains("<enddate>2013-09-29</enddate>"));
    let kodi = ecrire_nfo_serie(&s, &DonneesConservees::default(), Connecteur::Kodi, &o);
    assert!(!kodi.contains("<enddate>"));
}

#[test]
fn serie_nfo_reel_abrege() {
    let nfo = r#"<?xml version="1.0" encoding="UTF-8"?>
<tvshow>
  <title>Kaamelott</title>
  <showtitle>Kaamelott</showtitle>
  <rating>8.4</rating>
  <votes>12000</votes>
  <plot>Les chevaliers de la Table ronde.</plot>
  <thumb aspect="poster">https://img/k.jpg</thumb>
  <thumb aspect="poster" type="season" season="1">https://img/s1.jpg</thumb>
  <episodeguide><url cache="auth.json">https://api.thetvdb.com/login</url><url>https://thetvdb.com/?tab=series&amp;id=79175</url></episodeguide>
  <uniqueid>79175</uniqueid>
  <uniqueid type="imdb">tt0441005</uniqueid>
  <premiered>2005-01-03</premiered>
  <status>Ended</status>
  <studio>M6</studio>
  <genre>Comédie</genre>
  <actor><name>Alexandre Astier</name><role>Arthur</role></actor>
  <set>ignoré</set>
  <perso>à garder</perso>
</tvshow>"#;
    let lu = lire_nfo_serie(nfo);
    assert!(lu.valide);
    let s = &lu.serie;
    assert_eq!(s.ids.tvdb(), 79175);
    assert_eq!(s.ids.imdb(), "tt0441005");
    assert_eq!(s.annee, 2005);
    assert_eq!(s.statut, StatutDiffusion::Terminee);
    assert_eq!(s.notes["NFO"].valeur, 8.4);
    assert_eq!(s.urls_artwork[&TypeFichier::Poster], "https://img/k.jpg");
    // La vignette de saison et la balise inconnue sont conservées, pas `set`.
    let noms: Vec<_> = lu
        .conserves
        .inconnues
        .iter()
        .map(|e| e.nom.as_str())
        .collect();
    assert_eq!(noms, vec!["perso", "thumb"]);
    let xml = ecrire_nfo_serie(s, &lu.conserves, Connecteur::Kodi, &OptionsNfo::series());
    assert!(xml.contains(
        "<thumb aspect=\"poster\" type=\"season\" season=\"1\">https://img/s1.jpg</thumb>"
    ));
    assert!(xml.contains("<perso>à garder</perso>"));
    assert!(!xml.contains("<set>"));
}

#[test]
fn serie_variante_xbmc() {
    let s = serie_complete();
    let xml = ecrire_nfo_serie(
        &s,
        &DonneesConservees::default(),
        Connecteur::Xbmc,
        &OptionsNfo::series(),
    );
    assert!(xml.contains("<rating>9.5</rating>"));
    assert!(xml.contains("<votes>2000000</votes>"));
    assert!(!xml.contains("<ratings>"));
    assert!(xml.contains("<thumb>https://img/p.jpg</thumb>"));
    assert!(xml.contains("action=play_video"));
}

#[test]
fn serie_url_seule() {
    let lu = lire_nfo_serie(
        "https://thetvdb.com/?tab=series&id=81189\nhttps://www.themoviedb.org/tv/1396",
    );
    assert!(!lu.valide);
    assert_eq!(lu.serie.ids.tvdb(), 81189);
    assert_eq!(lu.serie.ids.tmdb(), 1396);
}

// ---------------------------------------------------------------------------
// Épisodes
// ---------------------------------------------------------------------------

fn episode(s: i32, e: i32, titre: &str) -> Episode {
    let mut ep = Episode {
        numeros: vec![
            NumeroEpisode {
                groupe: GroupeEpisodes::diffusion(),
                saison: s,
                episode: e,
            },
            NumeroEpisode {
                groupe: GroupeEpisodes::dvd(),
                saison: s,
                episode: e + 10,
            },
        ],
        titre: titre.into(),
        titre_original: format!("{titre} VO"),
        titre_anglais: format!("{titre} EN"),
        resume: format!("Résumé de {titre}."),
        premiere_diffusion: Some("2008-01-20".into()),
        duree_minutes: 58,
        vu: true,
        nombre_lectures: 1,
        derniere_lecture: Some(DATE_AJOUT),
        date_ajout: DATE_AJOUT,
        source_media: SourceMedia::WebDl,
        edition: Edition::NonCoupee,
        nom_fichier_origine: "bb.s01e01e02.mkv".into(),
        note_utilisateur: "top".into(),
        tags: vec!["pilote".into()],
        ..Episode::default()
    };
    ep.ids.definir(cles_id::TVDB, &format!("{}", 349_000 + e));
    ep.notes
        .insert("imdb".into(), Note::nouvelle("imdb", 9.0, 30_000, 10));
    ep.invites
        .push(personne(TypePersonne::Invite, "Invité", "Lui-même", "1"));
    ep.equipe.push(personne(
        TypePersonne::Scenariste,
        "Vince Gilligan",
        "Writer",
        "66633",
    ));
    ep
}

#[test]
fn episodes_multi_aller_retour() {
    let s = serie_complete();
    let mut e1 = episode(1, 1, "Pilot");
    let mut e2 = episode(1, 2, "Cat's in the Bag");
    e2.numeros.push(NumeroEpisode {
        groupe: GroupeEpisodes::affichage(),
        saison: 1,
        episode: 3,
    });
    let mut fichier = video("/series/BB/S01/bb.s01e01e02.mkv");
    fichier.duree_secondes = 0; // durée prise dans l'épisode
    e1.fichiers.push(fichier.clone());
    e2.fichiers.push(fichier);
    let conserves = vec![
        DonneesConservees {
            epbookmark: Some("100".into()),
            ..DonneesConservees::default()
        },
        DonneesConservees::default(),
    ];
    let xml = ecrire_nfo_episodes(
        &s,
        &[&e1, &e2],
        &conserves,
        Connecteur::Kodi,
        &OptionsNfo::series(),
    );
    assert_eq!(xml.matches("<?xml").count(), 1);
    assert_eq!(xml.matches("<episodedetails>").count(), 2);
    assert!(xml.contains("<showtitle>Breaking Bad</showtitle>"));
    assert!(
        xml.contains("<displayseason>1</displayseason>\r\n  <displayepisode>3</displayepisode>")
    );
    assert!(xml.contains("<type>GuestStar</type>"));
    assert!(xml.contains("<videoassettitle>Uncut</videoassettitle>"));
    assert!(xml.contains("<group id=\"DVD\" name=\"\" season=\"1\" episode=\"11\"/>"));
    assert!(xml.contains("<epbookmark>100</epbookmark>"));

    let lus = lire_nfo_episodes(&xml);
    assert!(lus.valide);
    assert_eq!(lus.episodes.len(), 2);
    assert_eq!(lus.episodes[0].conserves.epbookmark.as_deref(), Some("100"));
    for (lu, orig) in lus.episodes.iter().zip([&e1, &e2]) {
        let e = &lu.episode;
        assert!(e.multi_episodes);
        assert_eq!(e.numeros, orig.numeros);
        assert_eq!(e.titre, orig.titre);
        assert_eq!(e.titre_original, orig.titre_original);
        assert_eq!(e.titre_anglais, orig.titre_anglais);
        assert_eq!(e.resume, orig.resume);
        assert_eq!(e.premiere_diffusion, orig.premiere_diffusion);
        assert_eq!(e.duree_minutes, 58);
        assert_eq!(e.notes, orig.notes);
        assert_eq!(e.ids, orig.ids);
        assert_eq!(e.tags, orig.tags);
        assert_eq!(e.invites, orig.invites);
        assert_eq!(e.equipe, orig.equipe);
        assert!(e.vu);
        assert_eq!(e.nombre_lectures, 1);
        assert_eq!(e.derniere_lecture, orig.derniere_lecture);
        assert_eq!(e.date_ajout, DATE_AJOUT);
        assert_eq!(e.source_media, SourceMedia::WebDl);
        assert_eq!(e.edition, Edition::NonCoupee);
        assert_eq!(e.nom_fichier_origine, orig.nom_fichier_origine);
        assert_eq!(e.note_utilisateur, orig.note_utilisateur);
    }
}

#[test]
fn episodes_tous_acteurs_non_relus_comme_invites() {
    let s = serie_complete();
    let e = episode(1, 1, "Pilot");
    let mut o = OptionsNfo::series();
    o.ecrire_tous_acteurs = true;
    let xml = ecrire_nfo_episodes(&s, &[&e], &[], Connecteur::Kodi, &o);
    assert!(xml.contains("<name>Bryan Cranston</name>"));
    let lus = lire_nfo_episodes(&xml);
    assert_eq!(lus.episodes[0].episode.invites, e.invites);
}

#[test]
fn episodes_nfo_reel_multi_bruite() {
    // Deux blocs sans épisode_groups, texte parasite entre et après, pas de
    // racine commune.
    let nfo = "\u{FEFF}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<episodedetails>\n  <title>Partie 1</title>\n  <season>2</season>\n  <episode>5</episode>\n\
  <displayseason>0</displayseason><displayepisode>1</displayepisode>\n\
  <aired>2010-10-10</aired><premiered>2010-10-11</premiered>\n\
  <uniqueid type=\"tvdb\" default=\"true\">111</uniqueid>\n\
  <actor><name>A</name><role>B</role></actor>\n\
  <status>ignoré</status><inconnue>x</inconnue>\n</episodedetails>\n\
du bruit ici & là\n\
<episodedetails>\n  <title>Partie 2</title>\n  <season>2</season>\n  <episode>6</episode>\n  <id>222</id>\n</episodedetails>\n\
fin";
    let lus = lire_nfo_episodes(nfo);
    assert!(lus.valide);
    assert_eq!(lus.episodes.len(), 2);
    let a = &lus.episodes[0].episode;
    let diff = GroupeEpisodes::diffusion();
    assert_eq!((a.saison(&diff), a.episode(&diff)), (2, 5));
    let aff = a.numero_dans(&GroupeEpisodes::affichage()).unwrap();
    assert_eq!((aff.saison, aff.episode), (0, 1));
    assert_eq!(a.premiere_diffusion.as_deref(), Some("2010-10-10"));
    assert_eq!(a.ids.tvdb(), 111);
    assert_eq!(a.invites[0].type_personne, TypePersonne::Invite);
    let noms: Vec<_> = lus.episodes[0]
        .conserves
        .inconnues
        .iter()
        .map(|e| e.nom.as_str())
        .collect();
    assert_eq!(noms, vec!["inconnue"]);
    let b = &lus.episodes[1].episode;
    assert_eq!(b.episode(&diff), 6);
    assert_eq!(b.ids.tvdb(), 222);
}

#[test]
fn episodes_validite() {
    // Plusieurs blocs : le premier doit avoir un numéro d'épisode.
    let nfo = "<episodedetails><title>A</title></episodedetails><episodedetails><title>B</title><episode>2</episode></episodedetails>";
    assert!(!lire_nfo_episodes(nfo).valide);
    let nfo = "<episodedetails><title>A</title></episodedetails>";
    assert!(lire_nfo_episodes(nfo).valide);
    let nfo = "<episodedetails><season>1</season></episodedetails>";
    assert!(!lire_nfo_episodes(nfo).valide);
    // URL seule.
    let lus = lire_nfo_episodes("https://www.imdb.com/title/tt0959621/");
    assert!(!lus.valide);
    assert_eq!(lus.episodes[0].episode.ids.imdb(), "tt0959621");
    // Variante enregistrement.
    let lus = lire_nfo_episodes(
        "<recording><title>Série</title><subtitle>Épisode</subtitle><original_air_date>2020-02-02</original_air_date></recording>",
    );
    assert_eq!(lus.episodes[0].episode.titre, "Épisode");
    assert_eq!(
        lus.episodes[0].episode.premiere_diffusion.as_deref(),
        Some("2020-02-02")
    );
}

#[test]
fn episode_groupe_actif_dvd() {
    let mut s = serie_complete();
    s.groupe_actif = GroupeEpisodes::dvd();
    let e = episode(1, 1, "Pilot");
    let xml = ecrire_nfo_episodes(&s, &[&e], &[], Connecteur::Kodi, &OptionsNfo::series());
    assert!(xml.contains("<season>1</season>\r\n  <episode>11</episode>"));
}

// ---------------------------------------------------------------------------
// Saisons
// ---------------------------------------------------------------------------

#[test]
fn saison_aller_retour() {
    let s = serie_complete();
    let saison = Saison {
        numero: 2,
        titre: "Deuxième".into(),
        resume: "Suite.\nEt fin.".into(),
        premiere_diffusion: Some("2009-03-08".into()),
        fichiers: Vec::new(),
    };
    for c in [Connecteur::Kodi, Connecteur::Emby, Connecteur::Jellyfin] {
        let xml = ecrire_nfo_saison(
            &s,
            &saison,
            &DonneesConservees::default(),
            c,
            &OptionsNfo::series(),
        );
        assert!(xml.contains("<seasonnumber>2</seasonnumber>"));
        assert!(xml.contains("<year>2009</year>"));
        assert!(xml.contains(&format!("for {}-->", c.nom())));
        let lu = lire_nfo_saison(&xml);
        assert!(lu.valide);
        assert_eq!(lu.saison, saison);
    }
    let lu = lire_nfo_saison(
        "<season><title>X</title><resume>1</resume><epbookmark>2</epbookmark><autre/></season>",
    );
    assert!(!lu.valide);
    assert_eq!(lu.conserves.inconnues.len(), 1);
    assert!(!lire_nfo_saison("pas de xml").valide);
}

// ---------------------------------------------------------------------------
// Noms de fichiers
// ---------------------------------------------------------------------------

fn film_avec_videos(noms: &[&str]) -> Film {
    let mut f = Film {
        chemin: PathBuf::from("/films/Film"),
        ..Film::default()
    };
    for (i, n) in noms.iter().enumerate() {
        let mut v =
            FichierMedia::nouveau(&PathBuf::from("/films/Film").join(n), TypeFichier::Video);
        v.taille = 100 + i as u64;
        f.fichiers.push(v);
    }
    f
}

#[test]
fn noms_films() {
    use NommageNfoFilm::*;
    let f = film_avec_videos(&["Film (2000).mkv"]);
    assert_eq!(
        noms_nfo_film(&f, &[NomFichier], true),
        vec!["Film (2000).nfo"]
    );
    assert_eq!(
        noms_nfo_film(&f, &[NomFichier, Film, NomFichier], true),
        vec!["Film (2000).nfo", "movie.nfo"]
    );
    assert!(noms_nfo_film(&f, &[], true).is_empty());

    let mut multi = film_avec_videos(&["Autre.mkv"]);
    multi.dossier_multi_films = true;
    assert_eq!(noms_nfo_film(&multi, &[Film], true), vec!["Autre.nfo"]);

    let mut disque = film_avec_videos(&["BDMV/index.bdmv"]);
    disque.est_disque = true;
    assert_eq!(
        noms_nfo_film(&disque, &[NomFichier, Film], true),
        vec!["BDMV/index.nfo"]
    );
    assert_eq!(
        noms_nfo_film(&disque, &[NomFichier], false),
        vec!["movie.nfo"]
    );
}

#[test]
fn noms_series_episodes_saisons() {
    assert_eq!(nom_nfo_serie(NommageNfoSerie::TvShow), "tvshow.nfo");
    let mut e = Episode::default();
    assert_eq!(nom_nfo_episode(&e, NommageNfoEpisode::NomFichier), None);
    e.fichiers.push(FichierMedia::nouveau(
        &PathBuf::from("/s/S01/Show.S01E01.mkv"),
        TypeFichier::Video,
    ));
    assert_eq!(
        nom_nfo_episode(&e, NommageNfoEpisode::NomFichier).as_deref(),
        Some("Show.S01E01.nfo")
    );
    assert_eq!(
        nom_nfo_saison(1, NommageNfoSaison::Saison, ""),
        "season01.nfo"
    );
    assert_eq!(
        nom_nfo_saison(0, NommageNfoSaison::Saison, ""),
        "season00.nfo"
    );
    assert_eq!(
        nom_nfo_saison(-1, NommageNfoSaison::Saison, ""),
        "season-all.nfo"
    );
    assert_eq!(
        nom_nfo_saison(3, NommageNfoSaison::DossierSaison, "Saison 03/"),
        "Saison 03/season.nfo"
    );
    assert_eq!(
        nom_nfo_saison(3, NommageNfoSaison::DossierSaison, ""),
        "season.nfo"
    );
}

#[test]
fn notes_principales_et_groupes() {
    let mut notes = BTreeMap::new();
    notes.insert("tmdb".to_string(), Note::nouvelle("tmdb", 7.0, 10, 10));
    notes.insert(
        "metacritic".to_string(),
        Note::nouvelle("metacritic", 70.0, 0, 100),
    );
    // Source préférée absente : dernière source non `user`.
    let p = commun::note_principale(&notes, &["imdb".to_string()]).unwrap();
    assert_eq!(p.id, "tmdb");
    assert_eq!(
        serie::type_groupe_depuis("absolute"),
        Some(TypeGroupe::Absolu)
    );
    assert_eq!(serie::nom_type_groupe(TypeGroupe::Alternatif), "ALTERNATE");
}
