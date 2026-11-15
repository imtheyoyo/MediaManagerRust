//! Tests de la syntaxe et de la sémantique du moteur.

use mediamanager_gabarit::*;

fn r(g: &str, c: &Contexte) -> String {
    rendre(g, c).unwrap()
}

fn ctx() -> Contexte {
    Contexte::nouveau()
        .avec("a", "x")
        .avec("vide", "")
        .avec("zero", 0)
        .avec("n", 5)
        .avec("vrai", true)
        .avec("faux", false)
        .avec("liste", vec!["un", "deux", "trois"])
        .avec("listevide", Vec::<String>::new())
        .avec(
            "personne",
            Contexte::nouveau().avec("nom", "Ada").avec("age", 36),
        )
        .avec(
            "acteurs",
            vec![
                Contexte::nouveau().avec("name", "A1"),
                Contexte::nouveau().avec("name", "A2"),
            ],
        )
}

#[test]
fn variables() {
    let c = ctx();
    assert_eq!(r("${a}", &c), "x");
    assert_eq!(r("[${absente}]", &c), "[]");
    assert_eq!(r("${personne.nom} ${personne.age}", &c), "Ada 36");
    assert_eq!(r("${personne.inconnu.x}", &c), "");
    assert_eq!(r("${ a }", &c), "x");
    assert_eq!(r("prix $5 {a} $", &c), "prix $5 {a} $");
    assert_eq!(r("", &c), "");
    assert_eq!(r("${zero}|${n}|${vrai}|${faux}", &c), "|5|true|false");
    assert_eq!(r("${liste}", &c), "un, deux, trois");
    assert_eq!(r("é${a}ü", &c), "éxü");
}

#[test]
fn acces_indices() {
    let c = ctx();
    assert_eq!(r("${liste[0]}", &c), "un");
    assert_eq!(r("${liste[last]}", &c), "trois");
    assert_eq!(r("${liste[1,2]}", &c), "deux, trois");
    assert_eq!(r("${liste[9]}", &c), "");
    assert_eq!(r("${acteurs[0].name}", &c), "A1");
    assert_eq!(r("${acteurs[last].name}", &c), "A2");
    assert_eq!(r("${personne.nom[0]}", &c), "A");
    assert_eq!(r("${personne.nom[0,2]}", &c), "Ad");
    assert_eq!(r("${personne.nom[7]}", &c), "");
    assert_eq!(r("${vide[0]}", &c), "");
    assert!(rendre("${liste[x]}", &c).is_err());
    assert!(rendre("${liste[-1]}", &c).is_err());
}

#[test]
fn prefixe_suffixe() {
    let c = ctx();
    assert_eq!(r("t${ - ,a,}", &c), "t - x");
    assert_eq!(r("t${ - ,vide,}", &c), "t");
    assert_eq!(r("t${ - ,absente,}", &c), "t");
    assert_eq!(r("${(,a,)}", &c), "(x)");
    assert_eq!(r("${[,a,]}", &c), "[x]");
    assert_eq!(r("${<,a;upper,>}", &c), "<X>");
    assert_eq!(r("${ {ed-,a,}}", &c), " {ed-x}");
    assert_eq!(r("${ {ed-,vide,}}", &c), "");
    assert_eq!(r("${> ,liste;replace(un,1),!}", &c), "> 1, deux, trois!");
    assert_eq!(r("${>,a,}", &c), ">x");
    assert_eq!(r("${,zero,}", &c), "");
}

#[test]
fn rendus_chaines() {
    let c = Contexte::nouveau()
        .avec("t", "the MATRIX")
        .avec("g", "A, B, C");
    assert_eq!(r("${t;title}", &c), "The Matrix");
    assert_eq!(r("${t;upper;replace(THE,A)}", &c), "A MATRIX");
    assert_eq!(r("${t;chain(replace(the,XX);lower)}", &c), "xx matrix");
    assert_eq!(r("${g;split(0,1)}", &c), "A, B");
    assert_eq!(r("${g;split(2)}", &c), "C");
    assert_eq!(r("${g;replace(,,+)}", &c), "A+ B+ C");
    assert_eq!(r("${t;lower;first}", &c), "T");
    assert!(rendre("${t;inconnu}", &c).is_err());
    assert!(rendre("${t;chain(inconnu)}", &c).is_err());
    assert!(rendre("${t;number}", &c).is_err());
}

#[test]
fn conditions() {
    let c = ctx();
    assert_eq!(r("${if a}oui${end}", &c), "oui");
    assert_eq!(r("${if vide}oui${else}non${end}", &c), "non");
    assert_eq!(r("${if absente}oui${else}non${end}", &c), "non");
    assert_eq!(r("${if zero}oui${else}non${end}", &c), "non");
    assert_eq!(r("${if faux}oui${else}non${end}", &c), "non");
    assert_eq!(r("${if listevide}oui${else}non${end}", &c), "non");
    assert_eq!(r("${if liste}oui${else}non${end}", &c), "oui");
    assert_eq!(r("${if !vide}oui${else}non${end}", &c), "oui");
    assert_eq!(r("${if n=5}oui${else}non${end}", &c), "oui");
    assert_eq!(r("${if n!=5}oui${else}non${end}", &c), "non");
    assert_eq!(r("${if a=x}oui${else}non${end}", &c), "oui");
    assert_eq!(r("${if a=\"x\"}oui${else}non${end}", &c), "oui");
    assert_eq!(r("${if a=y}oui${else}non${end}", &c), "non");
    assert_eq!(r("${if a && vide}oui${else}non${end}", &c), "non");
    assert_eq!(r("${if a && n}oui${else}non${end}", &c), "oui");
    assert_eq!(r("${if vide || n}oui${else}non${end}", &c), "oui");
    assert_eq!(r("${if vide || zero && a}oui${else}non${end}", &c), "non");
    assert_eq!(r("${if personne.nom=Ada}oui${end}", &c), "oui");
    assert_eq!(r("${if a}${if vide}1${else}2${end}${else}3${end}", &c), "2");
    assert_eq!(r("${if absente}x${else}${if a}y${end}${end}", &c), "y");
}

#[test]
fn boucles() {
    let c = ctx();
    assert_eq!(r("${foreach liste i}[${i}]${end}", &c), "[un][deux][trois]");
    assert_eq!(r("${foreach liste i , }${i}${end}", &c), "un, deux, trois");
    assert_eq!(r("${foreach acteurs p /}${p.name}${end}", &c), "A1/A2");
    assert_eq!(
        r(
            "${foreach liste i}${index_i}${if last_i}.${else},${end}${end}",
            &c
        ),
        "0,1,2."
    );
    assert_eq!(r("${foreach listevide i}x${end}", &c), "");
    assert_eq!(r("${foreach absente i}x${end}", &c), "");
    assert_eq!(
        r(
            "${foreach liste i}${foreach liste j}${j[0]}${end}|${end}",
            &c
        ),
        "udt|udt|udt|"
    );
    // La variable de boucle ne fuit pas.
    assert_eq!(r("${foreach liste i}${end}[${i}]", &c), "[]");
}

#[test]
fn boucle_sur_dictionnaire() {
    let c = Contexte::nouveau().avec("d", Contexte::nouveau().avec("k1", "v1").avec("k2", "v2"));
    assert_eq!(
        r("${foreach d e ;}${e.key}=${e.value}${end}", &c),
        "k1=v1;k2=v2"
    );
}

#[test]
fn annotation_regexp() {
    let c = Contexte::nouveau().avec("t", "Show S02E05 titre");
    assert_eq!(r("${@regexp \"s(\\d+)e(\\d+)\" t \"$1x$2\"}", &c), "02x05");
    assert_eq!(r("${@regexp \"zzz\" t \"$1\"}", &c), "");
    assert_eq!(r("${@regexp \"x\" t}", &c), "");
    assert!(rendre("${@regexp \"(\" t \"a\"}", &c).is_err());
    assert_eq!(r("${@regexp \"a{2}\" t \"ok\"}|", &c), "|");
}

#[test]
fn dates_dans_gabarit() {
    let c = Contexte::nouveau()
        .avec("d", DateHeure::date(2024, 3, 9).unwrap())
        .avec("s", "2008-01-20")
        .avec("ms", 0);
    assert_eq!(r("${d;date(yyyy-MM-dd)}", &c), "2024-03-09");
    assert_eq!(r("${d;date(d MMM yy)}", &c), "9 Mar 24");
    assert_eq!(r("${s;date(dd/MM/yyyy)}", &c), "20/01/2008");
    assert_eq!(r("${d}", &c), "09.03.2024 00:00:00 +0000");
    assert!(rendre("${d;date(Q)}", &c).is_err());
}

#[test]
fn erreurs_de_syntaxe() {
    let c = ctx();
    for g in [
        "${a",
        "${if a}x",
        "${end}",
        "${else}",
        "${if}",
        "${foreach liste}x${end}",
        "${a b}",
        "${;x}",
        "${a;}",
        "${@x}",
        "${if a}${else}${else}${end}",
    ] {
        assert!(rendre(g, &c).is_err(), "devrait échouer : {g}");
    }
}

#[test]
fn jamais_de_panique() {
    let c = ctx();
    let entrees = [
        "${",
        "$",
        "${}",
        "}",
        "{${",
        "${${a}}",
        "${if ${a}}",
        "${foreach",
        "${a[}",
        "${a]}",
        "${[0]}",
        "${a;replace(}",
        "${a;)}",
        "${,}",
        "${,,}",
        "${,a,}",
        "${@regexp}",
        "${@regexp \"}",
        "\u{0}${a}",
        "${é}",
        "${a;number(%)}",
        "${a;number(%9999999999d)}",
        "${a;date('}",
        "${a[99999999999999999999]}",
    ];
    for g in entrees {
        let _ = rendre(g, &c);
        let _ = creer_destination_ou_brut(
            g,
            &c,
            &Jetons::default(),
            FormeDestination::Dossier,
            &OptionsDestination::default(),
        );
    }
    // Profondeur excessive.
    let g = "${if a}".repeat(1000);
    assert!(rendre(&g, &c).is_err());
    let g = format!("{}{}", "${if a}".repeat(10), "${end}".repeat(10));
    assert_eq!(r(&g, &c), "");
}

#[test]
fn gabarit_reutilisable() {
    let g = Gabarit::analyser("${a}-${n}").unwrap();
    let j = Jetons::default();
    let o = OptionsRendu::default();
    assert_eq!(g.rendre(&ctx(), &j, &o).unwrap(), "x-5");
    assert_eq!(
        g.rendre(&Contexte::nouveau().avec("a", "y"), &j, &o)
            .unwrap(),
        "y-"
    );
}

#[test]
fn jeton_invalide() {
    let mut j = Jetons::nouveau();
    assert!(j.ajouter("x", "a b").is_err());
    assert!(j.ajouter("x", "a;(").is_err());
    assert!(j.ajouter("x", "a;upper").is_ok());
    assert_eq!(j.len(), 1);
    assert!(!j.is_empty());
}
