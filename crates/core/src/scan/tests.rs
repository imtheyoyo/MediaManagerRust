//! Tests d'intégration du scan sur des arborescences temporaires.

use std::fs;
use std::path::Path;

use super::*;
use crate::modele::{Edition, SourceMedia, TypeFichier};
use tempfile::TempDir;

/// Crée les fichiers (et dossiers parents) ; un chemin finissant par `/` crée un dossier.
fn creer(racine: &Path, chemins: &[&str]) {
    for c in chemins {
        let p = racine.join(c);
        if c.ends_with('/') {
            fs::create_dir_all(&p).unwrap();
        } else {
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, b"contenu").unwrap();
        }
    }
}

fn ecrire(racine: &Path, chemin: &str, contenu: &str) {
    let p = racine.join(chemin);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, contenu).unwrap();
}

fn scanner(racine: &Path) -> ResultatScan {
    scanner_source(racine, &OptionsScan::default()).unwrap()
}

fn types(film: &Film) -> Vec<TypeFichier> {
    film.fichiers.iter().map(|f| f.type_fichier).collect()
}

fn nombre(film: &Film, t: TypeFichier) -> usize {
    film.fichiers.iter().filter(|f| f.type_fichier == t).count()
}

fn noms(film: &Film, t: TypeFichier) -> Vec<String> {
    film.fichiers
        .iter()
        .filter(|f| f.type_fichier == t)
        .map(|f| f.nom.clone())
        .collect()
}

fn film_titre<'a>(r: &'a ResultatScan, titre: &str) -> &'a Film {
    r.films
        .iter()
        .find(|f| f.titre == titre)
        .unwrap_or_else(|| {
            panic!(
                "film « {titre} » introuvable parmi {:?}",
                r.films.iter().map(|f| &f.titre).collect::<Vec<_>>()
            )
        })
}

// ---------------------------------------------------------------------------
// Erreurs de source
// ---------------------------------------------------------------------------

#[test]
fn source_introuvable_vide_ou_fichier() {
    let t = TempDir::new().unwrap();
    let o = OptionsScan::default();
    assert!(matches!(
        scanner_source(&t.path().join("absent"), &o),
        Err(ErreurScan::SourceIntrouvable(_))
    ));
    // Hors Windows, une source vide est considérée comme indisponible (disque
    // réseau non monté) ; sous Windows elle est simplement vide.
    #[cfg(not(windows))]
    assert!(matches!(
        scanner_source(t.path(), &o),
        Err(ErreurScan::SourceVide(_))
    ));
    creer(t.path(), &["x.txt"]);
    assert!(matches!(
        scanner_source(&t.path().join("x.txt"), &o),
        Err(ErreurScan::PasUnDossier(_))
    ));
}

// ---------------------------------------------------------------------------
// Un dossier = un film
// ---------------------------------------------------------------------------

#[test]
fn dossier_par_film_complet() {
    let t = TempDir::new().unwrap();
    let d = "The Matrix (1999) {imdb-tt0133093}";
    creer(
        t.path(),
        &[
            &format!("{d}/The Matrix (1999).mkv"),
            &format!("{d}/The Matrix (1999).nfo"),
            &format!("{d}/The Matrix (1999).fr.srt"),
            &format!("{d}/The Matrix (1999).en.forced.srt"),
            &format!("{d}/poster.jpg"),
            &format!("{d}/fanart.jpg"),
            &format!("{d}/banner.jpg"),
            &format!("{d}/clearlogo.png"),
            &format!("{d}/extrafanart/fanart1.jpg"),
            &format!("{d}/The Matrix (1999)-trailer.mkv"),
            &format!("{d}/Extras/making of.mkv"),
            &format!("{d}/theme.mp3"),
            &format!("{d}/notes.txt"),
            &format!("{d}/.cache"),
            &format!("{d}/inconnu.xyz"),
        ],
    );
    creer(t.path(), &["Autre (2000)/Autre (2000).avi"]);
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 2);
    let f = film_titre(&r, "The Matrix");
    assert_eq!(f.annee, 1999);
    assert_eq!(f.chemin, t.path().join(d));
    assert_eq!(f.source_donnees, t.path());
    assert!(!f.dossier_multi_films);
    assert!(!f.est_disque);
    assert_eq!(f.ids.imdb(), "tt0133093");
    assert_eq!(f.nom_fichier_origine, "The Matrix (1999).mkv");
    assert_eq!(nombre(f, TypeFichier::Video), 1);
    assert_eq!(nombre(f, TypeFichier::Nfo), 1);
    assert_eq!(nombre(f, TypeFichier::SousTitres), 2);
    assert_eq!(nombre(f, TypeFichier::Poster), 1);
    assert_eq!(nombre(f, TypeFichier::Fanart), 1);
    assert_eq!(nombre(f, TypeFichier::Banner), 1);
    assert_eq!(nombre(f, TypeFichier::Clearlogo), 1);
    assert_eq!(nombre(f, TypeFichier::ExtraFanart), 1);
    assert_eq!(nombre(f, TypeFichier::Trailer), 1);
    assert_eq!(nombre(f, TypeFichier::Extra), 1);
    assert_eq!(nombre(f, TypeFichier::Theme), 1);
    assert_eq!(nombre(f, TypeFichier::Texte), 1);
    // Fichier caché et fichier inconnu : non rattachés.
    assert!(
        f.fichiers
            .iter()
            .all(|x| x.nom != ".cache" && x.nom != "inconnu.xyz")
    );
    // Tri : vidéo d'abord ; sous-titres du nom le plus long d'abord.
    assert_eq!(f.fichiers[0].type_fichier, TypeFichier::Video);
    assert_eq!(
        noms(f, TypeFichier::SousTitres),
        vec![
            "The Matrix (1999).en.forced.srt".to_string(),
            "The Matrix (1999).fr.srt".to_string()
        ]
    );
    let video = f.fichier_principal().unwrap();
    assert_eq!(video.taille, 7);
    assert!(video.date_fichier > 0);
    assert_eq!(video.format_conteneur, "mkv");
    assert!(!f.empile);
}

#[test]
fn identifiants_du_nom_de_dossier() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Fight Club (1999) [tmdbid=550]/f.mkv",
            "Autre (2001) tmdb-77 tvdb_88/a.mkv",
            "Via Url (2002)/Via Url.mkv",
        ],
    );
    let r = scanner(t.path());
    assert_eq!(film_titre(&r, "Fight Club").ids.tmdb(), 550);
    let a = film_titre(&r, "Autre");
    assert_eq!((a.ids.tmdb(), a.ids.tvdb()), (77, 88));
}

#[test]
fn identifiants_depuis_le_nfo_et_nom_de_fichier() {
    let t = TempDir::new().unwrap();
    creer(t.path(), &["Film (2000)/Film (2000).mkv"]);
    ecrire(
        t.path(),
        "Film (2000)/Film (2000).nfo",
        "<movie><title>Film</title><id>tt0111161</id>https://www.themoviedb.org/movie/278</movie>",
    );
    creer(t.path(), &["Autre (2001)/Autre tt7654321.mkv"]);
    let r = scanner(t.path());
    let f = film_titre(&r, "Film");
    assert_eq!(f.ids.imdb(), "tt0111161");
    assert_eq!(f.ids.tmdb(), 278);
    assert_eq!(film_titre(&r, "Autre").ids.imdb(), "tt7654321");
}

#[test]
fn edition_3d_et_source() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &["Avatar (2009) 3D Extended Edition/Avatar.2009.BluRay.mkv"],
    );
    let r = scanner(t.path());
    let f = film_titre(&r, "Avatar");
    assert_eq!(f.annee, 2009);
    assert_eq!(f.edition, Edition::Etendue);
    assert!(f.video_3d);
    assert_eq!(f.source_media, SourceMedia::Bluray);
}

#[test]
fn source_media_depuis_un_dossier_parent() {
    let t = TempDir::new().unwrap();
    creer(t.path(), &["Film (2000) DVDRip/film.avi"]);
    let r = scanner(t.path());
    assert_eq!(r.films[0].source_media, SourceMedia::Dvd);
}

#[test]
fn titre_triable_et_dossier_sans_annee() {
    let t = TempDir::new().unwrap();
    creer(t.path(), &["Matrix, The (1999)/m.mkv", "Sans annee/s.mkv"]);
    let r = scanner(t.path());
    assert_eq!(film_titre(&r, "The Matrix").annee, 1999);
    assert_eq!(film_titre(&r, "Sans annee").annee, 0);
}

#[test]
fn trailer_et_sample_ne_font_pas_un_film() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Seul/Seul-trailer.mkv",
            "Seul/Seul-sample.mkv",
            "Seul/Extras/x.mkv",
        ],
    );
    let r = scanner(t.path());
    assert!(r.films.is_empty());
}

#[test]
fn sample_et_extras_rattaches_au_film() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Film (2000)/Film.mkv",
            "Film (2000)/Film-sample.mkv",
            "Film (2000)/Sample/s.mkv",
            "Film (2000)/Trailers/t.mp4",
        ],
    );
    let r = scanner(t.path());
    let f = &r.films[0];
    assert_eq!(nombre(f, TypeFichier::Video), 1);
    assert_eq!(nombre(f, TypeFichier::Sample), 2);
    assert_eq!(nombre(f, TypeFichier::Trailer), 1);
}

#[test]
fn image_d_un_autre_nom_est_promue_si_absente() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Film (2000)/Film.mkv",
            "Film (2000)/Autre-poster.jpg",
            "Film (2000)/divers.jpg",
            "Film (2000)/Film-fanart.jpg",
        ],
    );
    let r = scanner(t.path());
    let f = &r.films[0];
    assert_eq!(nombre(f, TypeFichier::Poster), 1);
    assert_eq!(nombre(f, TypeFichier::Fanart), 1);
    // `divers.jpg` reste un graphique non rattaché.
    assert!(f.fichiers.iter().all(|x| x.nom != "divers.jpg"));
}

#[test]
fn fichiers_inconnus_en_double_extension() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Film (2000)/Film.mkv",
            "Film (2000)/Film.mkv.foo",
            "Film (2000)/autre.foo",
        ],
    );
    let r = scanner(t.path());
    let f = &r.films[0];
    assert_eq!(
        noms(f, TypeFichier::DoubleExt),
        vec!["Film.mkv.foo".to_string()]
    );
}

#[test]
fn skiptro_et_scenepass_seulement_avec_video_homonyme() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Film (2000)/Film.mkv",
            "Film (2000)/Film.skiptro.json",
            "Film (2000)/Film.sp.json",
            "Film (2000)/Autre.skiptro.json",
            "Film (2000)/Autre.sp.hash.json",
        ],
    );
    let r = scanner(t.path());
    let f = &r.films[0];
    assert_eq!(nombre(f, TypeFichier::Skiptro), 1);
    assert_eq!(nombre(f, TypeFichier::Scenepass), 1);
}

// ---------------------------------------------------------------------------
// Empilement
// ---------------------------------------------------------------------------

#[test]
fn fichiers_empiles_a_plat_dans_le_dossier_du_film() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Film (2000)/Film (2000) CD1.mkv",
            "Film (2000)/Film (2000) CD2.mkv",
            "Film (2000)/Film (2000) CD1.fr.srt",
        ],
    );
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 1);
    let f = &r.films[0];
    assert!(f.empile);
    let videos: Vec<_> = f
        .fichiers_de_type(TypeFichier::Video)
        .map(|v| (v.nom.clone(), v.empilement, v.marqueur_empilement.clone()))
        .collect();
    assert_eq!(
        videos,
        vec![
            ("Film (2000) CD1.mkv".to_string(), 1, "CD1".to_string()),
            ("Film (2000) CD2.mkv".to_string(), 2, "CD2".to_string())
        ]
    );
    assert_eq!(f.fichier_principal().unwrap().nom, "Film (2000) CD1.mkv");
    assert_eq!(f.nom_video_sans_pile(), "Film (2000)");
    assert_eq!(f.titre, "Film");
    assert_eq!(f.annee, 2000);
}

#[test]
fn dossiers_cd1_cd2_traites_au_niveau_parent() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Film (2000)/CD1/film cd1.avi",
            "Film (2000)/CD2/film cd2.avi",
            "Film (2000)/Film (2000).nfo",
        ],
    );
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 1);
    let f = &r.films[0];
    assert_eq!(f.chemin, t.path().join("Film (2000)"));
    assert_eq!(f.titre, "Film");
    assert_eq!(nombre(f, TypeFichier::Video), 2);
    assert!(f.empile);
    assert_eq!(nombre(f, TypeFichier::Nfo), 1);
}

#[test]
fn dossiers_de_pile_depuis_le_dossier_pour_les_fichiers_sans_marqueur() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &["Film (2000)/CD1/a.avi", "Film (2000)/CD2/b.avi"],
    );
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 1);
    let nums: Vec<u32> = r.films[0]
        .fichiers_de_type(TypeFichier::Video)
        .map(|v| v.empilement)
        .collect();
    assert_eq!(nums, vec![1, 2]);
}

#[test]
fn deux_videos_de_meme_nom_apres_retrait_de_pile_sont_un_seul_film() {
    let t = TempDir::new().unwrap();
    creer(t.path(), &["Divers/Film-a.mkv", "Divers/Film-b.mkv"]);
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 1);
    assert_eq!(nombre(&r.films[0], TypeFichier::Video), 2);
    assert!(r.films[0].empile);
}

// ---------------------------------------------------------------------------
// Multi-films
// ---------------------------------------------------------------------------

#[test]
fn dossier_multi_films_rattache_les_fichiers_au_bon_film() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Divers/Alpha (2001).mkv",
            "Divers/Alpha (2001).nfo",
            "Divers/Alpha (2001)-poster.jpg",
            "Divers/Alpha (2001).fr.srt",
            "Divers/Alpha (2001)-trailer.mkv",
            "Divers/Alpha II (2003).mkv",
            "Divers/Alpha II (2003).nfo",
            "Divers/Alpha II (2003)-fanart.jpg",
            "Divers/Beta (2002).avi",
            "Divers/Beta (2002).jpg",
            "Divers/orphelin.srt",
        ],
    );
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 3);
    for f in &r.films {
        assert!(f.dossier_multi_films);
        assert_eq!(f.chemin, t.path().join("Divers"));
    }
    let a = film_titre(&r, "Alpha");
    assert_eq!(a.annee, 2001);
    assert_eq!(nombre(a, TypeFichier::Nfo), 1);
    assert_eq!(
        a.fichiers_de_type(TypeFichier::Nfo).next().unwrap().nom,
        "Alpha (2001).nfo"
    );
    assert_eq!(nombre(a, TypeFichier::Poster), 1);
    assert_eq!(nombre(a, TypeFichier::SousTitres), 1);
    assert_eq!(nombre(a, TypeFichier::Trailer), 1);
    assert_eq!(nombre(a, TypeFichier::Video), 1);
    assert_eq!(nombre(a, TypeFichier::Fanart), 0);

    let a2 = film_titre(&r, "Alpha II");
    assert_eq!(a2.annee, 2003);
    assert_eq!(
        a2.fichiers_de_type(TypeFichier::Nfo).next().unwrap().nom,
        "Alpha II (2003).nfo"
    );
    assert_eq!(nombre(a2, TypeFichier::Fanart), 1);
    assert_eq!(nombre(a2, TypeFichier::Poster), 0);

    // `Beta (2002).jpg` porte le nom de la vidéo : affiche.
    let b = film_titre(&r, "Beta");
    assert_eq!(nombre(b, TypeFichier::Poster), 1);
    // Le sous-titre orphelin n'est rattaché à personne.
    assert!(
        r.films
            .iter()
            .all(|f| f.fichiers.iter().all(|x| x.nom != "orphelin.srt"))
    );
}

#[test]
fn multi_films_avec_empilement() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Divers/Alpha CD1.mkv",
            "Divers/Alpha CD2.mkv",
            "Divers/Beta.mkv",
        ],
    );
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 2);
    let a = film_titre(&r, "Alpha");
    assert!(a.empile);
    assert_eq!(nombre(a, TypeFichier::Video), 2);
    assert!(!film_titre(&r, "Beta").empile);
}

#[test]
fn films_a_plat_a_la_racine_de_la_source() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Zeta (2010).mkv",
            "Zeta (2010).nfo",
            "Eta.2011.1080p.mkv",
            "Eta.2011.1080p.fr.srt",
            "Dossier (2012)/d.mkv",
        ],
    );
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 3);
    let z = film_titre(&r, "Zeta");
    assert!(z.dossier_multi_films);
    assert_eq!(z.chemin, t.path());
    assert_eq!(nombre(z, TypeFichier::Nfo), 1);
    let e = film_titre(&r, "Eta");
    assert_eq!(e.annee, 2011);
    assert_eq!(nombre(e, TypeFichier::SousTitres), 1);
    assert!(!film_titre(&r, "Dossier").dossier_multi_films);
}

#[test]
fn dossier_contenant_des_films_et_des_sous_dossiers_de_films() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Collection/Direct (2000).mkv",
            "Collection/Sous Film (2001)/sf.mkv",
        ],
    );
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 2);
    assert!(film_titre(&r, "Direct").dossier_multi_films);
    let sf = film_titre(&r, "Sous Film");
    assert!(!sf.dossier_multi_films);
    assert_eq!(sf.chemin, t.path().join("Collection/Sous Film (2001)"));
}

#[test]
fn dossiers_de_regroupement_lettre_annee_genre() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "2019/Un (2019).mkv",
            "Action/Deux (2005).mkv",
            "1980-1989/Trois (1985).mkv",
            "A/Alien (1979)/alien.mkv",
        ],
    );
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 4);
    for (titre, multi) in [
        ("Un", true),
        ("Deux", true),
        ("Trois", true),
        ("Alien", false),
    ] {
        assert_eq!(film_titre(&r, titre).dossier_multi_films, multi, "{titre}");
    }
    assert_eq!(film_titre(&r, "Un").chemin, t.path().join("2019"));
}

// ---------------------------------------------------------------------------
// Dossiers ignorés
// ---------------------------------------------------------------------------

#[test]
fn dossiers_et_fichiers_ignores() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Bon (2000)/bon.mkv",
            ".cache/Cache (2000)/c.mkv",
            "@eaDir/Ea (2000)/e.mkv",
            "$RECYCLE.BIN/Corbeille (2000)/r.mkv",
            "System Volume Information/S (2000)/s.mkv",
            "MAKEMKV/Make (2000)/m.mkv",
            "Bon (2000)/.cache/x.mkv",
            "Lost.dir/L (2000)/l.mkv",
            "Plex Versions/P (2000)/p.mkv",
            ".45 Calibre (2015)/c.mkv",
        ],
    );
    let r = scanner(t.path());
    let mut titres: Vec<&str> = r.films.iter().map(|f| f.titre.as_str()).collect();
    titres.sort();
    assert_eq!(titres, vec!["45 Calibre", "Bon"]);
    assert_eq!(nombre(&r.films[1], TypeFichier::Video), 1);
}

#[test]
fn marqueurs_d_exclusion() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Bon (2000)/bon.mkv",
            "A Ignorer (2000)/a.mkv",
            "A Ignorer (2000)/.mmignore",
            "Media (2001)/m.mkv",
            "Media (2001)/.nomedia",
            "Sous/Profond (2002)/p.mkv",
            "Sous/mmignore",
        ],
    );
    let r = scanner(t.path());
    let titres: Vec<&str> = r.films.iter().map(|f| f.titre.as_str()).collect();
    assert_eq!(titres, vec!["Bon"]);

    let o = OptionsScan {
        ignorer_nomedia: false,
        ..OptionsScan::default()
    };
    let r = scanner_source(t.path(), &o).unwrap();
    let titres: Vec<&str> = r.films.iter().map(|f| f.titre.as_str()).collect();
    assert_eq!(titres, vec!["Bon", "Media"]);
}

#[test]
fn motifs_d_exclusion_personnalises_et_sources_imbriquees() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Bon (2000)/b.mkv",
            "Brouillon42/Brou (2000)/b.mkv",
            "Autre source/Imbriquee (2001)/i.mkv",
            "MAKEMKV/M (2000)/m.mkv",
        ],
    );
    let o = OptionsScan {
        dossiers_ignores: vec!["Brouillon\\d+".to_string()],
        sources_exclues: vec![t.path().join("Autre source")],
        ..OptionsScan::default()
    };
    let r = scanner_source(t.path(), &o).unwrap();
    let mut titres: Vec<&str> = r.films.iter().map(|f| f.titre.as_str()).collect();
    titres.sort();
    // Le motif par défaut est remplacé : MAKEMKV n'est plus ignoré.
    assert_eq!(titres, vec!["Bon", "M"]);
}

// ---------------------------------------------------------------------------
// Disques
// ---------------------------------------------------------------------------

#[test]
fn dvd_video_ts() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Film DVD (2005)/VIDEO_TS/VIDEO_TS.IFO",
            "Film DVD (2005)/VIDEO_TS/VTS_01_1.VOB",
            "Film DVD (2005)/VIDEO_TS/VTS_01_0.IFO",
            "Film DVD (2005)/VIDEO_TS/VIDEO_TS.nfo",
            "Film DVD (2005)/poster.jpg",
        ],
    );
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 1);
    let f = &r.films[0];
    assert!(f.est_disque);
    assert_eq!(f.chemin, t.path().join("Film DVD (2005)"));
    assert_eq!(f.annee, 2005);
    assert_eq!(f.source_media, SourceMedia::Dvd);
    // Seuls le fichier identifiant, le nfo et l'artwork sont conservés.
    assert_eq!(
        noms(f, TypeFichier::Video),
        vec!["VIDEO_TS.IFO".to_string()]
    );
    assert_eq!(nombre(f, TypeFichier::Nfo), 1);
    assert_eq!(nombre(f, TypeFichier::Poster), 1);
    assert!(!f.empile);
}

#[test]
fn bluray_bdmv_avec_titre_de_disque() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Disque_BD_2/BDMV/index.bdmv",
            "Disque_BD_2/BDMV/STREAM/00000.m2ts",
            "Disque_BD_2/BDMV/PLAYLIST/00000.mpls",
        ],
    );
    ecrire(
        t.path(),
        "Disque_BD_2/BDMV/META/DL/bdmt_eng.xml",
        "<disclib><di:discinfo><di:title><di:name>Mon Super Film</di:name></di:title></di:discinfo></disclib>",
    );
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 1);
    let f = &r.films[0];
    assert!(f.est_disque);
    assert_eq!(f.titre, "Mon Super Film");
    assert_eq!(f.source_media, SourceMedia::Bluray);
    assert_eq!(noms(f, TypeFichier::Video), vec!["index.bdmv".to_string()]);
}

#[test]
fn bluray_titre_bdinfo_en_dernier_recours() {
    let t = TempDir::new().unwrap();
    creer(t.path(), &["2019/BDMV/index.bdmv"]);
    // « 2019 » est un nom de regroupement, mais un disque reste un film.
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 1);
    assert!(r.films[0].est_disque);
    // Titre de repli : le nom du dossier, faute de mieux.
    assert!(!r.films[0].titre.is_empty());
}

#[test]
fn dossier_disque_a_la_racine_avertit() {
    let t = TempDir::new().unwrap();
    creer(t.path(), &["BDMV/index.bdmv", "Film (2000)/f.mkv"]);
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 1);
    assert!(
        r.avertissements
            .iter()
            .any(|a| matches!(a, Avertissement::DossierDisqueALaRacine(_)))
    );
}

#[test]
fn film_iso() {
    let t = TempDir::new().unwrap();
    creer(t.path(), &["Film (2010)/Film (2010).iso"]);
    let r = scanner(t.path());
    let f = &r.films[0];
    let v = f.fichier_principal().unwrap();
    assert!(v.est_iso);
    assert!(!f.est_disque);
}

#[test]
fn film_hors_ligne_disc() {
    let t = TempDir::new().unwrap();
    creer(t.path(), &["Film (2010)/Film (2010).disc"]);
    let r = scanner(t.path());
    assert!(r.films[0].hors_ligne);
}

#[test]
fn sous_films_d_un_film_disque_ignores() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Coffret (2000)/VIDEO_TS/VIDEO_TS.IFO",
            "Coffret (2000)/Bonus (2001)/b.mkv",
        ],
    );
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 1);
    assert!(r.films[0].est_disque);
}

// ---------------------------------------------------------------------------
// Divers
// ---------------------------------------------------------------------------

#[test]
fn resultat_trie_et_deterministe() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &["B (2000)/b.mkv", "A (2000)/a.mkv", "C (2000)/c.mkv"],
    );
    let a = scanner(t.path());
    let b = scanner(t.path());
    let titres = |r: &ResultatScan| r.films.iter().map(|f| f.titre.clone()).collect::<Vec<_>>();
    assert_eq!(titres(&a), vec!["A", "B", "C"]);
    assert_eq!(titres(&a), titres(&b));
}

#[test]
fn dossier_sans_video_ignore() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Vide (2000)/Vide.nfo",
            "Vide (2000)/poster.jpg",
            "Bon (2000)/b.mkv",
        ],
    );
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 1);
}

#[cfg(unix)]
#[test]
fn liens_symboliques_suivis() {
    let t = TempDir::new().unwrap();
    let ailleurs = TempDir::new().unwrap();
    creer(ailleurs.path(), &["Lie (2000)/lie.mkv"]);
    creer(t.path(), &["Bon (2000)/b.mkv"]);
    std::os::unix::fs::symlink(
        ailleurs.path().join("Lie (2000)"),
        t.path().join("Lie (2000)"),
    )
    .unwrap();
    let r = scanner(t.path());
    assert_eq!(r.films.len(), 2);
}

#[test]
fn types_de_fichiers_du_film_sont_tries() {
    let t = TempDir::new().unwrap();
    creer(
        t.path(),
        &[
            "Film (2000)/zz.nfo",
            "Film (2000)/Film.mkv",
            "Film (2000)/poster.jpg",
            "Film (2000)/Film.srt",
        ],
    );
    let r = scanner(t.path());
    assert_eq!(
        types(&r.films[0]),
        vec![
            TypeFichier::Video,
            TypeFichier::SousTitres,
            TypeFichier::Nfo,
            TypeFichier::Poster
        ]
    );
}
