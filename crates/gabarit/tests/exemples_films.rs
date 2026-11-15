//! Exemples gabarit → résultat de la spécification du renommeur de films.

use mediamanager_gabarit::*;

fn film(titre: &str, annee: i64) -> Contexte {
    Contexte::nouveau()
        .avec("title", titre)
        .avec("year", annee)
        .avec("videoFormat", "1080p")
        .avec("audioCodec", "AC3")
        .avec("imdb", "tt0133093")
}

fn dossier(g: &str, c: &Contexte) -> String {
    creer_destination(g, c, &Jetons::default(), FormeDestination::Dossier, &opts()).unwrap()
}

fn fichier(g: &str, c: &Contexte, o: &OptionsDestination) -> String {
    creer_destination(g, c, &Jetons::default(), FormeDestination::Fichier, o).unwrap()
}

fn opts() -> OptionsDestination {
    OptionsDestination {
        plateforme: Plateforme::Unix,
        ..Default::default()
    }
}

fn star_wars() -> Contexte {
    film("Star Wars: Episode IV – A New Hope", 1977).avec("edition", "")
}

#[test]
fn titre_annee_deux_points() {
    assert_eq!(
        fichier("${title} (${year})", &star_wars(), &opts()),
        "Star Wars - Episode IV – A New Hope (1977)"
    );
}

#[test]
fn edition_prefixe_suffixe() {
    let g = "${title}${ - ,edition,} (${year})";
    assert_eq!(
        fichier(g, &star_wars(), &opts()),
        "Star Wars - Episode IV – A New Hope (1977)"
    );
    let c = star_wars().avec("edition", "Director's Cut");
    assert_eq!(
        fichier(g, &c, &opts()),
        "Star Wars - Episode IV – A New Hope - Director's Cut (1977)"
    );
    // Édition absente du contexte.
    let c = film("Alien", 1979);
    assert_eq!(fichier(g, &c, &opts()), "Alien (1979)");
}

#[test]
fn gabarit_par_defaut() {
    assert_eq!(
        fichier(
            "${title} (${year}) ${videoFormat} ${audioCodec}",
            &star_wars(),
            &opts()
        ),
        "Star Wars - Episode IV – A New Hope (1977) 1080p AC3"
    );
}

#[test]
fn titre_triable() {
    let c = film("The Matrix", 1999).avec("titleSortable", "Matrix, The");
    assert_eq!(
        fichier("${titleSortable} (${year})", &c, &opts()),
        "Matrix, The (1999)"
    );
}

#[test]
fn premiere_lettre() {
    let g = "${title;first}/${title} (${year})";
    assert_eq!(dossier(g, &film("The Matrix", 1999)), "T/The Matrix (1999)");
    assert_eq!(dossier(g, &film("12 Monkeys", 1995)), "#/12 Monkeys (1995)");
    assert_eq!(
        dossier("${title[0]}/${title} (${year})", &film("The Matrix", 1999)),
        "T/The Matrix (1999)"
    );
}

#[test]
fn decennie() {
    let c = film("The Matrix", 1999).avec("decadeShort", "1990s");
    assert_eq!(
        dossier("${decadeShort}/${title} (${year})", &c),
        "1990s/The Matrix (1999)"
    );
}

#[test]
fn imdb_absent_crochets_retires() {
    let g = "${title} (${year}) [${imdb}]";
    assert_eq!(
        fichier(g, &film("The Matrix", 1999), &opts()),
        "The Matrix (1999) [tt0133093]"
    );
    let c = Contexte::nouveau()
        .avec("title", "The Matrix")
        .avec("year", 1999);
    assert_eq!(fichier(g, &c, &opts()), "The Matrix (1999)");
}

#[test]
fn annee_zero_parentheses_vides() {
    assert_eq!(
        fichier("${title} (${year})", &film("Titre", 0), &opts()),
        "Titre"
    );
}

#[test]
fn majuscules_et_listes() {
    let c = film("The Matrix", 1999).avec("genres", vec!["Action", "Sci-Fi"]);
    assert_eq!(
        fichier("${title;upper} (${year})", &c, &opts()),
        "THE MATRIX (1999)"
    );
    // Moteur seul : le séparateur d'`array` est conservé.
    assert_eq!(
        rendre("${title} (${year}) ${genres;array( / )}", &c).unwrap(),
        "The Matrix (1999) Action / Sci-Fi"
    );
    // Avec le nettoyage fichier, `/` est (comme dans le programme d'origine)
    // remplacé par une espace.
    assert_eq!(
        fichier("${title} (${year}) ${genres;array( - )}", &c, &opts()),
        "The Matrix (1999) Action - Sci-Fi"
    );
}

#[test]
fn ascii() {
    let o = OptionsDestination {
        ascii: true,
        ..opts()
    };
    let c = Contexte::nouveau().avec("originalTitle", "Léon");
    assert_eq!(fichier("${originalTitle}", &c, &o), "Leon");
}

#[test]
fn substitution_espaces() {
    let o = OptionsDestination {
        remplacement_espaces_fichier: Some(".".into()),
        ..opts()
    };
    assert_eq!(
        fichier(
            "${title} (${year}) ${videoFormat}",
            &film("The Matrix", 1999),
            &o
        ),
        "The.Matrix.(1999).1080p"
    );
}

#[test]
fn ensemble_de_films() {
    let g = "${if movieSet.title}${movieSet.title}/${end}${title} (${year})";
    let c = film("The Matrix", 1999).avec(
        "movieSet",
        Contexte::nouveau().avec("title", "Matrix Collection"),
    );
    assert_eq!(dossier(g, &c), "Matrix Collection/The Matrix (1999)");
    assert_eq!(dossier(g, &film("The Matrix", 1999)), "The Matrix (1999)");
}

#[test]
fn nombres_et_taille() {
    let c = Contexte::nouveau()
        .avec("rating", 8.7)
        .avec("size", 4_370_000_000i64);
    assert_eq!(rendre("${rating;number(%.1f)}", &c).unwrap(), "8.7");
    assert_eq!(rendre("${size;filesize}", &c).unwrap(), "4.37 GB");
    // Note nulle : vide.
    let c = Contexte::nouveau().avec("rating", 0.0);
    assert_eq!(rendre("[${rating}]", &c).unwrap(), "[]");
}

#[test]
fn separateurs_dans_les_valeurs() {
    assert_eq!(fichier("${title}", &film("AC/DC", 2000), &opts()), "AC DC");
    let o = OptionsDestination {
        unicode: true,
        ..opts()
    };
    assert_eq!(
        fichier("${title}", &film("AC/DC", 2000), &o),
        "AC\u{2044}DC"
    );
    // Le jeton parent n'est pas filtré.
    let c = Contexte::nouveau().avec("parent", "a/b").avec("title", "T");
    assert_eq!(dossier("${parent}/${title}", &c), "a/b/T");
}

#[test]
fn dossier_multiniveaux_et_nettoyage() {
    let c = film("Titre.", 1999);
    assert_eq!(
        dossier("Films / ${title} .../${title} (${year})", &c),
        "Films/Titre/Titre. (1999)"
    );
    assert_eq!(dossier("/${title}//x", &c), "Titre/x");
    assert_eq!(fichier("  - ${title} -  ", &c, &opts()), "Titre");
}

#[test]
fn trois_points_unicode() {
    let o = OptionsDestination {
        unicode: true,
        ..opts()
    };
    assert_eq!(
        fichier("${title}", &film("Et alors...", 0), &o),
        "Et alors\u{2026}"
    );
    assert_eq!(
        fichier("${title}", &film("Et alors...", 0), &opts()),
        "Et alors"
    );
    assert_eq!(fichier("${title}...x", &film("Et", 0), &opts()), "Et...x");
}

#[test]
fn unicode_caracteres_interdits() {
    let o = OptionsDestination {
        unicode: true,
        ..opts()
    };
    let r = fichier("${title}", &film("A: B?", 0), &o);
    assert_eq!(r, "A\u{A789} B\u{FF1F}");
}

#[test]
fn jetons_et_chemin_movie() {
    let mut j = Jetons::nouveau();
    j.ajouter("title", "movie.title").unwrap();
    j.ajouter("year", "movie.year").unwrap();
    j.ajouter("releaseDate", "movie.releaseDate;date(yyyy-MM-dd)")
        .unwrap();
    j.ajouter("genres", "movie.genres").unwrap();
    let c = Contexte::nouveau().avec(
        "movie",
        Contexte::nouveau()
            .avec("title", "The Matrix")
            .avec("year", 1999)
            .avec("releaseDate", "1999-03-31")
            .avec("genres", vec!["Action", "Sci-Fi"]),
    );
    let o = OptionsRendu::default();
    let r = |g: &str| Gabarit::analyser(g).unwrap().rendre(&c, &j, &o).unwrap();
    assert_eq!(r("${TITLE} (${year})"), "The Matrix (1999)");
    assert_eq!(r("${releaseDate}"), "1999-03-31");
    assert_eq!(r("${title[0,3]}"), "The");
    assert_eq!(r("${genres[last]}"), "Sci-Fi");
    assert_eq!(r("${if year=1999}oui${else}non${end}"), "oui");
    assert_eq!(r("${movie.title}"), "The Matrix");
    assert_eq!(r("${title;first}"), "T");
}

#[test]
fn validite_des_gabarits() {
    assert!(gabarit_dossier_unique("${title} (${year})"));
    assert!(gabarit_dossier_unique("${imdb}"));
    assert!(!gabarit_dossier_unique("${title}"));
    assert!(!gabarit_dossier_unique("Films"));
    assert!(gabarit_fichier_valide("${originalFilename}"));
    assert!(gabarit_fichier_valide("${TITLE}"));
    assert!(!gabarit_fichier_valide("${year}"));
}

#[test]
fn erreur_et_repli() {
    let c = film("A", 1);
    assert!(
        creer_destination(
            "${title",
            &c,
            &Jetons::default(),
            FormeDestination::Fichier,
            &opts()
        )
        .is_err()
    );
    assert_eq!(
        creer_destination_ou_brut(
            "${title",
            &c,
            &Jetons::default(),
            FormeDestination::Fichier,
            &opts()
        ),
        "${title"
    );
    assert_eq!(fichier("   ", &c, &opts()), "");
}

#[test]
fn nom_reserve_windows() {
    let o = OptionsDestination {
        plateforme: Plateforme::Windows,
        ..Default::default()
    };
    let c = Contexte::nouveau().avec("title", "CON");
    assert_eq!(fichier("${title}", &c, &o), "CON_");
    assert_eq!(fichier("${title}", &c, &opts()), "CON");
}
