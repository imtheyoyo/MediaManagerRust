//! Exemples gabarit → résultat de la spécification du renommeur de séries.

use mediamanager_gabarit::*;

const FICHIER_DEFAUT: &str = "${showTitle} - S${seasonNr2}E${episodeNr2} - ${title}";

fn ep(saison: i64, episode: i64, titre: &str) -> EpisodeContexte {
    EpisodeContexte {
        saison,
        episode,
        contexte: Contexte::nouveau()
            .avec("showTitle", "Show")
            .avec("showYear", 2008)
            .avec("seasonNr", saison)
            .avec("seasonNr2", format!("{saison:02}"))
            .avec("episodeNr", episode)
            .avec("episodeNr2", format!("{episode:02}"))
            .avec("title", titre),
    }
}

fn opts() -> OptionsDestination {
    OptionsDestination {
        plateforme: Plateforme::Unix,
        remplacement_deux_points: String::new(),
        ..Default::default()
    }
}

fn multi(
    g: &str,
    eps: &[EpisodeContexte],
    style: StyleMultiEpisodes,
    o: &OptionsDestination,
) -> String {
    creer_destination_multi(
        g,
        eps,
        style,
        &ListesJetonsEpisode::default(),
        &Jetons::default(),
        FormeDestination::Fichier,
        o,
    )
    .unwrap()
}

fn simple(g: &str, e: &EpisodeContexte, forme: FormeDestination, o: &OptionsDestination) -> String {
    creer_destination(g, &e.contexte, &Jetons::default(), forme, o).unwrap()
}

#[test]
fn dossiers() {
    let e = ep(1, 1, "x");
    assert_eq!(
        simple(
            "${showTitle} (${showYear})",
            &e,
            FormeDestination::Dossier,
            &opts()
        ),
        "Show (2008)"
    );
    let e = ep(2, 1, "x");
    assert_eq!(
        simple("Season ${seasonNr}", &e, FormeDestination::Dossier, &opts()),
        "Season 2"
    );
}

#[test]
fn nombre_zero_avec_number() {
    // `seasonNr` brut vaut 0 ⇒ vide ; `;number(%d)` ⇒ "0".
    let e = ep(0, 1, "x");
    assert_eq!(
        simple("S${seasonNr}", &e, FormeDestination::Fichier, &opts()),
        "S"
    );
    assert_eq!(
        simple(
            "S${seasonNr;number(%d)}",
            &e,
            FormeDestination::Fichier,
            &opts()
        ),
        "S0"
    );
}

#[test]
fn fichier_simple() {
    assert_eq!(
        simple(
            FICHIER_DEFAUT,
            &ep(1, 2, "Pilot"),
            FormeDestination::Fichier,
            &opts()
        ),
        "Show - S01E02 - Pilot"
    );
}

#[test]
fn repeat() {
    let eps = [ep(1, 1, "A"), ep(1, 2, "B")];
    assert_eq!(
        multi(FICHIER_DEFAUT, &eps, StyleMultiEpisodes::Repeat, &opts()),
        "Show - S01E01 S01E02 - A - B"
    );
}

#[test]
fn range() {
    let eps = [ep(1, 1, "A"), ep(1, 2, "B")];
    assert_eq!(
        multi(FICHIER_DEFAUT, &eps, StyleMultiEpisodes::Range, &opts()),
        "Show - S01E01-E02 - A - B"
    );
    let eps3 = [ep(1, 1, "A"), ep(1, 2, "B"), ep(1, 3, "C")];
    assert_eq!(
        multi(FICHIER_DEFAUT, &eps3, StyleMultiEpisodes::Range, &opts()),
        "Show - S01E01-E03 - A - B - C"
    );
}

#[test]
fn range_format_x() {
    let eps = [ep(1, 1, "A"), ep(1, 2, "B")];
    assert_eq!(
        multi(
            "${showTitle} ${seasonNr}x${episodeNr2} ${title}",
            &eps,
            StyleMultiEpisodes::Range,
            &opts()
        ),
        "Show 1x01-02 A - B"
    );
}

#[test]
fn range_replie_sur_repeat() {
    let non_consecutifs = [ep(1, 1, "A"), ep(1, 3, "B")];
    assert_eq!(
        multi(
            FICHIER_DEFAUT,
            &non_consecutifs,
            StyleMultiEpisodes::Range,
            &opts()
        ),
        "Show - S01E01 S01E03 - A - B"
    );
    let saisons = [ep(1, 2, "A"), ep(2, 3, "B")];
    assert_eq!(
        multi(FICHIER_DEFAUT, &saisons, StyleMultiEpisodes::Range, &opts()),
        "Show - S01E02 S02E03 - A - B"
    );
}

#[test]
fn tri_doublons_et_pile() {
    let desordre = [ep(1, 2, "B"), ep(1, 1, "A")];
    assert_eq!(
        multi(
            FICHIER_DEFAUT,
            &desordre,
            StyleMultiEpisodes::Repeat,
            &opts()
        ),
        "Show - S01E01 S01E02 - A - B"
    );
    let meme_titre = [ep(1, 1, "Deux parties"), ep(1, 2, "Deux parties")];
    assert_eq!(
        multi(
            FICHIER_DEFAUT,
            &meme_titre,
            StyleMultiEpisodes::Repeat,
            &opts()
        ),
        "Show - S01E01 S01E02 - Deux parties"
    );
    let pile = [ep(1, 1, "A"), ep(1, 1, "A")];
    assert_eq!(
        multi(FICHIER_DEFAUT, &pile, StyleMultiEpisodes::Range, &opts()),
        "Show - S01E01 - A"
    );
    assert_eq!(
        multi(FICHIER_DEFAUT, &[], StyleMultiEpisodes::Range, &opts()),
        ""
    );
}

#[test]
fn remplacement_espaces() {
    let o = OptionsDestination {
        remplacement_espaces_fichier: Some("_".into()),
        ..opts()
    };
    assert_eq!(
        simple(
            FICHIER_DEFAUT,
            &ep(1, 2, "Pilot"),
            FormeDestination::Fichier,
            &o
        ),
        "Show_-_S01E02_-_Pilot"
    );
}

#[test]
fn deux_points_et_barre() {
    let o = OptionsDestination {
        remplacement_deux_points: "-".into(),
        ..opts()
    };
    let e = ep(1, 1, "Doctor Who: Rose");
    assert_eq!(
        simple("${title}", &e, FormeDestination::Fichier, &o),
        "Doctor Who - Rose"
    );
    let e = ep(1, 1, "AC/DC");
    assert_eq!(
        simple("${title}", &e, FormeDestination::Fichier, &o),
        "AC DC"
    );
}

#[test]
fn prefixe_sans_date() {
    let e = ep(1, 1, "Pilot");
    assert_eq!(
        simple(
            "${title}${ - ,airedDate,}",
            &e,
            FormeDestination::Fichier,
            &opts()
        ),
        "Pilot"
    );
    let mut e2 = ep(1, 1, "Pilot");
    e2.contexte.inserer("airedDate", "2008-01-20");
    assert_eq!(
        simple(
            "${title}${ - ,airedDate;date(yyyy-MM-dd),}",
            &e2,
            FormeDestination::Fichier,
            &opts()
        ),
        "Pilot - 2008-01-20"
    );
}

#[test]
fn dates_multi() {
    let mut a = ep(1, 1, "A");
    let mut b = ep(1, 2, "B");
    a.contexte.inserer("airedDate", "2008-01-20");
    b.contexte.inserer("airedDate", "2008-01-27");
    assert_eq!(
        multi(
            "S${seasonNr2}E${episodeNr2} ${airedDate;date(yyyy-MM-dd)}",
            &[a, b],
            StyleMultiEpisodes::Repeat,
            &opts()
        ),
        "S01E01 S01E02 2008-01-20 - 2008-01-27"
    );
}

#[test]
fn sans_partie_saison() {
    let eps = [ep(1, 1, "A"), ep(1, 2, "B")];
    assert_eq!(
        multi("E${episodeNr2}", &eps, StyleMultiEpisodes::Range, &opts()),
        "E01-E02"
    );
    // Sans jeton épisode : premier épisode seul.
    assert_eq!(
        multi("${title}", &eps, StyleMultiEpisodes::Range, &opts()),
        "A"
    );
}
