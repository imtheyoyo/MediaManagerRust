//! Pages de la fenêtre des réglages. Chaque contrôle modifie directement la copie de travail
//! (`Reglages` de l'interface ou `ReglagesServices`) ; le dialogue compare ensuite cette copie à
//! l'état et émet `AppliquerReglages` / `AppliquerServices` (enregistrés à la fermeture).
//!
//! Règles de mise en page (fenêtre modale à taille stable) : aucune zone ne remplit « l'espace
//! disponible » sans hauteur bornée, aucun `ui.separator()` dans une ligne horizontale, listes
//! longues dans des zones à hauteur maximale.

use super::Sortie;
use crate::conversion::{NB_AFFICHES_MAX, NB_FANARTS_MAX};
use crate::etat::{ActionDialogue, Etat, EtatDialogueReglages, Message, Secret};
use crate::gabarits::{self, ChampGabarit, Exemples};
use crate::i18n::{I18n, Langue};
use crate::polices;
use crate::reglages::{HauteurLigne, Page, Reglages, Theme};
use crate::selecteur::{CibleDossier, CibleFichier};
use crate::source_donnees::{
    OutilAffiche, ReglagesServices, ServiceCle, cle_testable, est_identifiant_client,
    lien_obtention_cle,
};
use crate::validation::{self, ErreurChamp};
use eframe::egui::{
    self, Id, Response, RichText, ScrollArea, TextEdit, Ui,
    text::{CCursor, CCursorRange},
};
use mediamanager_core::renommage::titre_triable;
use mediamanager_store::reglages::{
    FormatNfo, QualiteBandeAnnonce, QualiteCacheImages, ReglagesFilms, ReglagesSeries,
    TailleCacheImages, TailleImage,
};

/// Fournisseurs de métadonnées pris en charge (identifiant stocké).
const FOURNISSEURS: &[&str] = &["tmdb"];
const MARGE_LABELS: f32 = 240.0;
const HAUTEUR_VARIABLES: f32 = 150.0;

// ---------------------------------------------------------------------------
// Briques communes
// ---------------------------------------------------------------------------

fn infobulle(i: &I18n, cle: &str) -> String {
    i.t(&format!("{cle}.tip"))
}

/// Case à cocher avec infobulle (clé du libellé + `.tip`).
fn case(ui: &mut Ui, i: &I18n, valeur: &mut bool, cle: &str) -> Response {
    ui.checkbox(valeur, i.t(cle))
        .on_hover_text(infobulle(i, cle))
}

/// Libellé d'une ligne de grille, avec infobulle.
fn etiquette(ui: &mut Ui, i: &I18n, cle: &str) -> Response {
    ui.label(i.t(cle)).on_hover_text(infobulle(i, cle))
}

fn section(ui: &mut Ui, i: &I18n, cle: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(i.t(cle)).strong());
    ui.add_space(2.0);
}

fn note(ui: &mut Ui, texte: impl Into<String>) {
    ui.label(RichText::new(texte.into()).weak().small());
}

fn erreur(ui: &mut Ui, i: &I18n, e: ErreurChamp) {
    ui.label(
        RichText::new(format!("{} {}", polices::AVERTISSEMENT, i.t(e.cle())))
            .color(crate::theme::ROUGE)
            .small(),
    );
}

/// Ligne « libellé + nombre borné » (champ validé : la saisie est ramenée dans `[min, max]`).
fn nombre_borne(ui: &mut Ui, i: &I18n, cle: &str, valeur: &mut u32, min: u32, max: u32) {
    etiquette(ui, i, cle);
    ui.add(egui::DragValue::new(valeur).range(min..=max).speed(0.1))
        .on_hover_text(infobulle(i, cle));
    *valeur = (*valeur).clamp(min, max);
    ui.end_row();
}

fn grille(id: impl std::hash::Hash + std::fmt::Debug) -> egui::Grid {
    egui::Grid::new(id).num_columns(2).spacing([14.0, 8.0])
}

/// Largeur d'un champ de saisie : s'adapte à la largeur de la page.
fn largeur_champ(ui: &Ui) -> f32 {
    (ui.available_width() - MARGE_LABELS).clamp(140.0, 420.0)
}

/// Liste déroulante d'un type énuméré ; les libellés sont des clés de traduction.
fn choix<T: Copy + PartialEq>(
    ui: &mut Ui,
    i: &I18n,
    id: &str,
    valeur: &mut T,
    options: &[(T, &str)],
) -> Response {
    let courant = options
        .iter()
        .find(|(v, _)| v == valeur)
        .map(|(_, c)| i.t(c))
        .unwrap_or_default();
    egui::ComboBox::from_id_salt(id)
        .selected_text(courant)
        .show_ui(ui, |ui| {
            for (v, cle) in options {
                ui.selectable_value(valeur, *v, i.t(cle));
            }
        })
        .response
}

/// Champ texte dont la valeur est validée : l'erreur éventuelle s'affiche sous le champ (la
/// valeur n'est enregistrée par les services que si elle est valide).
fn champ_valide(
    ui: &mut Ui,
    i: &I18n,
    valeur: &mut String,
    largeur: f32,
    valider: fn(&str) -> Result<String, ErreurChamp>,
) {
    ui.vertical(|ui| {
        ui.add(TextEdit::singleline(valeur).desired_width(largeur));
        if let Err(e) = valider(valeur) {
            erreur(ui, i, e);
        }
    });
}

/// Liste de valeurs éditable : retrait par ✖, ajout validé (sans doublon) par le bouton ou
/// Entrée, valeurs par défaut en option. La saisie en cours vit dans la mémoire d'egui.
fn liste(
    ui: &mut Ui,
    i: &I18n,
    id_salt: &str,
    valeurs: &mut Vec<String>,
    valider: fn(&str) -> Result<String, ErreurChamp>,
    defauts: Option<Vec<String>>,
) {
    let id = Id::unique(("liste_reglages", id_salt));
    let mut saisie: String = ui.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    if valeurs.is_empty() {
        ui.label(RichText::new(i.t("gui.settings.list.empty")).weak());
    }
    let mut a_retirer = None;
    ui.horizontal_wrapped(|ui| {
        for (n, v) in valeurs.iter().enumerate() {
            ui.label(RichText::new(v).monospace());
            if ui
                .small_button("✖")
                .on_hover_text(i.t("gui.settings.list.remove"))
                .clicked()
            {
                a_retirer = Some(n);
            }
            ui.add_space(6.0);
        }
    });
    if let Some(n) = a_retirer {
        valeurs.remove(n);
    }
    let mut essai = valeurs.clone();
    let verdict = validation::ajouter(&mut essai, &saisie, valider);
    ui.horizontal(|ui| {
        let champ = ui.add(TextEdit::singleline(&mut saisie).desired_width(160.0));
        let entree = champ.lost_focus() && ui.input(|e| e.key_pressed(egui::Key::Enter));
        let ajouter = ui
            .add_enabled(
                verdict.is_ok(),
                egui::Button::new(format!("+ {}", i.t("gui.settings.list.add"))),
            )
            .clicked();
        if (ajouter || entree) && verdict.is_ok() {
            *valeurs = essai.clone();
            saisie.clear();
        }
        if let Some(d) = &defauts
            && ui
                .button(i.t("gui.settings.list.reset"))
                .on_hover_text(i.t("gui.settings.list.reset.tip"))
                .clicked()
        {
            *valeurs = d.clone();
        }
    });
    if let Err(e) = verdict
        && e != ErreurChamp::Vide
    {
        erreur(ui, i, e);
    }
    ui.data_mut(|d| d.insert_temp(id, saisie));
}

/// Page dont les réglages ne sont pas encore pris en charge : contrôles désactivés.
fn bientot(ui: &mut Ui, i: &I18n, cles: &[&str]) {
    ui.label(RichText::new(i.t("gui.settings.soon")).strong());
    note(ui, i.t("gui.settings.soon.note"));
    ui.add_space(6.0);
    ui.add_enabled_ui(false, |ui| {
        for cle in cles {
            let mut v = false;
            ui.checkbox(&mut v, i.t(cle))
                .on_disabled_hover_text(i.t("gui.settings.soon"));
        }
    });
}

/// Hauteur maximale de la liste des colonnes (la liste défile au-delà).
const HAUTEUR_COLONNES: f32 = 270.0;

/// Une ligne de la liste des colonnes : ce que l'utilisateur a changé.
#[derive(Default)]
struct ChangeColonne {
    visible: Option<bool>,
    /// -1 : monter, 1 : descendre.
    deplacement: i32,
    largeur: Option<f32>,
}

/// Ligne de la liste des colonnes : case de visibilité, boutons monter / descendre, largeur par
/// défaut (`None` : colonne à largeur automatique ou fixe).
fn ligne_colonne(
    ui: &mut Ui,
    i: &I18n,
    libelle: &str,
    (visible, peut_masquer): (bool, bool),
    (rang, total): (usize, usize),
    largeur: Option<f32>,
) -> ChangeColonne {
    use crate::vue::editeur::composants::{Glyphe, bouton};
    let mut c = ChangeColonne::default();
    let mut v = visible;
    if ui
        .add_enabled(peut_masquer, egui::Checkbox::new(&mut v, libelle))
        .changed()
    {
        c.visible = Some(v);
    }
    if bouton(ui, Glyphe::Haut, rang > 0, &i.t("Button.moveup")).clicked() {
        c.deplacement = -1;
    }
    if bouton(ui, Glyphe::Bas, rang + 1 < total, &i.t("Button.movedown")).clicked() {
        c.deplacement = 1;
    }
    match largeur {
        Some(mut l) => {
            if ui
                .add(
                    egui::DragValue::new(&mut l)
                        .range(30.0..=900.0)
                        .speed(1.0)
                        .max_decimals(0)
                        .suffix(" px"),
                )
                .changed()
            {
                c.largeur = Some(l.round());
            }
        }
        None => {
            ui.add_enabled(false, egui::Label::new(i.t("gui.settings.table.widthauto")));
        }
    }
    ui.end_row();
    c
}

/// Page « Interface » des films et des séries : colonnes du tableau (affichage, ordre, largeur
/// par défaut), options d'affichage. Les colonnes sont appliquées tout de suite par messages ;
/// les options d'affichage modifient la copie de travail des réglages.
fn page_interface(ui: &mut Ui, etat: &Etat, m: Module, r: &mut Reglages, sortie: &mut Sortie) {
    let i = &etat.i18n;
    let series = m == Module::Series;
    section(ui, i, "gui.settings.table.columns");
    note(ui, i.t("gui.settings.table.columns.note"));
    ui.add_space(4.0);
    ScrollArea::vertical()
        .id_salt(("colonnes_interface", series))
        .max_height(HAUTEUR_COLONNES)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new(("grille_colonnes_interface", series))
                .num_columns(4)
                .spacing([10.0, 4.0])
                .show(ui, |ui| {
                    if series {
                        let total = etat.colonnes_series.len();
                        for (rang, c) in etat.colonnes_series.iter().enumerate() {
                            let col = c.colonne;
                            let libelle = i.t(col.cle_i18n());
                            let largeur = (!col.est_booleenne()
                                && col != crate::modele::ColonneSerie::Titre)
                                .then(|| etat.largeur_colonne_serie(col));
                            let ch = ligne_colonne(
                                ui,
                                i,
                                &libelle,
                                (c.visible, col != crate::modele::ColonneSerie::Titre),
                                (rang, total),
                                largeur,
                            );
                            if let Some(v) = ch.visible {
                                sortie.push(Message::ColonneSerieVisible(col, v));
                            }
                            if ch.deplacement != 0 {
                                sortie.push(Message::OrdreColonneSerie(col, ch.deplacement));
                            }
                            if let Some(l) = ch.largeur {
                                sortie.push(Message::LargeurColonneSerie(col, Some(l)));
                            }
                        }
                    } else {
                        let total = etat.colonnes.len();
                        for (rang, c) in etat.colonnes.iter().enumerate() {
                            let col = c.colonne;
                            let libelle = i.t(col.cle_i18n());
                            let largeur = (!col.est_booleenne()
                                && col != crate::modele::Colonne::Titre)
                                .then(|| etat.largeur_colonne(col));
                            let ch = ligne_colonne(
                                ui,
                                i,
                                &libelle,
                                (c.visible, col != crate::modele::Colonne::Titre),
                                (rang, total),
                                largeur,
                            );
                            if let Some(v) = ch.visible {
                                sortie.push(Message::ColonneVisible(col, v));
                            }
                            if ch.deplacement != 0 {
                                sortie.push(Message::OrdreColonne(col, ch.deplacement));
                            }
                            if let Some(l) = ch.largeur {
                                sortie.push(Message::LargeurColonne(col, Some(l)));
                            }
                        }
                    }
                });
        });
    ui.add_space(4.0);
    if ui.button(i.t("gui.settings.table.reset")).clicked() {
        sortie.push(Message::ReinitialiserColonnes(series));
    }
    section(ui, i, "gui.settings.table.display");
    let aff = if series {
        &mut r.affichage_series
    } else {
        &mut r.affichage_films
    };
    case(ui, i, &mut aff.lignes_rayees, "gui.settings.table.striped");
    case(
        ui,
        i,
        &mut aff.icones_etat,
        "gui.settings.table.statusicons",
    );
    ui.add_space(4.0);
    grille(("grille_affichage_interface", series)).show(ui, |ui| {
        etiquette(ui, i, "gui.settings.table.rowheight");
        let options: Vec<(HauteurLigne, &str)> =
            HauteurLigne::TOUTES.iter().map(|h| (*h, h.cle())).collect();
        choix(
            ui,
            i,
            if series {
                "hauteur_ligne_series"
            } else {
                "hauteur_ligne_films"
            },
            &mut aff.hauteur_ligne,
            &options,
        );
        ui.end_row();
    });
    ui.add_space(8.0);
    bientot(
        ui,
        i,
        &[
            "gui.settings.soon.missing",
            "gui.settings.soon.searchfields",
            "gui.settings.soon.newindicator",
        ],
    );
}

const TAILLES_IMAGE: [(TailleImage, &str); 4] = [
    (TailleImage::Petite, "gui.settings.size.small"),
    (TailleImage::Moyenne, "gui.settings.size.medium"),
    (TailleImage::Grande, "gui.settings.size.large"),
    (TailleImage::Originale, "gui.settings.size.original"),
];
const FORMATS_NFO: [(FormatNfo, &str); 4] = [
    (FormatNfo::Kodi, "gui.settings.nfo.format.kodi"),
    (FormatNfo::Jellyfin, "gui.settings.nfo.format.jellyfin"),
    (FormatNfo::Emby, "gui.settings.nfo.format.emby"),
    (FormatNfo::Plex, "gui.settings.nfo.format.plex"),
];
const QUALITES_BANDE: [(QualiteBandeAnnonce, &str); 4] = [
    (QualiteBandeAnnonce::Sd, "gui.settings.trailer.sd"),
    (QualiteBandeAnnonce::Hd720, "gui.settings.trailer.hd720"),
    (QualiteBandeAnnonce::Hd1080, "gui.settings.trailer.hd1080"),
    (QualiteBandeAnnonce::Uhd, "gui.settings.trailer.uhd"),
];

/// Champs communs aux réglages des films et des séries (emprunts disjoints).
struct Commun<'a> {
    ignorer_nomedia: &'a mut bool,
    mots_exclus: &'a mut Vec<String>,
    mise_a_jour_au_demarrage: &'a mut bool,
    cache_a_l_import: &'a mut bool,
    scraper: &'a mut String,
    pays: &'a mut String,
    toutes_notes: &'a mut bool,
    meilleure_image: &'a mut bool,
    taille_affiche: &'a mut TailleImage,
    taille_fanart: &'a mut TailleImage,
    images_acteurs: &'a mut bool,
    nombre_affiches: &'a mut u32,
    nombre_fanarts: &'a mut u32,
    /// Vignettes d'épisodes (séries seulement).
    taille_vignette: Option<&'a mut TailleImage>,
    format_nfo: &'a mut FormatNfo,
    bandes_auto: &'a mut bool,
    qualite_bande: &'a mut QualiteBandeAnnonce,
    langue_sous_titres: &'a mut String,
    duree_depuis_analyse: &'a mut bool,
}

fn commun_films(f: &mut ReglagesFilms) -> Commun<'_> {
    Commun {
        ignorer_nomedia: &mut f.ignorer_dossiers_nomedia,
        mots_exclus: &mut f.mots_exclus,
        mise_a_jour_au_demarrage: &mut f.mise_a_jour_au_demarrage,
        cache_a_l_import: &mut f.cache_images_a_l_import,
        scraper: &mut f.scraper,
        pays: &mut f.pays_certification,
        toutes_notes: &mut f.recuperer_toutes_notes,
        meilleure_image: &mut f.meilleure_image_auto,
        taille_affiche: &mut f.taille_affiche,
        taille_fanart: &mut f.taille_fanart,
        images_acteurs: &mut f.images_acteurs,
        nombre_affiches: &mut f.nombre_affiches,
        nombre_fanarts: &mut f.nombre_fanarts,
        taille_vignette: None,
        format_nfo: &mut f.format_nfo,
        bandes_auto: &mut f.bandes_annonces_auto,
        qualite_bande: &mut f.qualite_bande_annonce,
        langue_sous_titres: &mut f.langue_sous_titres,
        duree_depuis_analyse: &mut f.duree_depuis_analyse,
    }
}

fn commun_series(f: &mut ReglagesSeries) -> Commun<'_> {
    Commun {
        ignorer_nomedia: &mut f.ignorer_dossiers_nomedia,
        mots_exclus: &mut f.mots_exclus,
        mise_a_jour_au_demarrage: &mut f.mise_a_jour_au_demarrage,
        cache_a_l_import: &mut f.cache_images_a_l_import,
        scraper: &mut f.scraper,
        pays: &mut f.pays_certification,
        toutes_notes: &mut f.recuperer_toutes_notes,
        meilleure_image: &mut f.meilleure_image_auto,
        taille_affiche: &mut f.taille_affiche,
        taille_fanart: &mut f.taille_fanart,
        images_acteurs: &mut f.images_acteurs,
        nombre_affiches: &mut f.nombre_affiches,
        nombre_fanarts: &mut f.nombre_fanarts,
        taille_vignette: Some(&mut f.taille_vignette),
        format_nfo: &mut f.format_nfo,
        bandes_auto: &mut f.bandes_annonces_auto,
        qualite_bande: &mut f.qualite_bande_annonce,
        langue_sous_titres: &mut f.langue_sous_titres,
        duree_depuis_analyse: &mut f.duree_depuis_analyse,
    }
}

/// Module concerné par une page commune.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Module {
    Films,
    Series,
}

impl Module {
    fn sel(self, films: &'static str, series: &'static str) -> &'static str {
        match self {
            Module::Films => films,
            Module::Series => series,
        }
    }
}

// ---------------------------------------------------------------------------
// Aiguillage
// ---------------------------------------------------------------------------

pub fn page(
    ui: &mut Ui,
    etat: &Etat,
    d: &EtatDialogueReglages,
    r: &mut Reglages,
    s: &mut ReglagesServices,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    match d.page {
        Page::General => page_general(ui, etat, r, sortie),
        Page::TypesFichiers => page_types_fichiers(ui, i, s),
        Page::TriTitre => page_tri_titre(ui, i, s),
        Page::OutilsExternes => page_outils(ui, etat, d, s, sortie),
        Page::MaterielsExternes => bientot(ui, i, &["gui.settings.soon.devices"]),
        Page::ServicesExternes => page_services(ui, etat, d, s, sortie),
        Page::Systeme => page_systeme(ui, i, s),
        Page::Ard => bientot(ui, i, &["gui.settings.soon.ard"]),
        Page::Divers => page_divers(ui, i, s),
        Page::Films => page_racine(ui, i, Module::Films, s),
        Page::Series => page_racine(ui, i, Module::Series, s),
        Page::FilmsInterface => page_interface(ui, etat, Module::Films, r, sortie),
        Page::SeriesInterface => page_interface(ui, etat, Module::Series, r, sortie),
        Page::FilmsSource => page_source(ui, etat, d, Module::Films, s, sortie),
        Page::SeriesSource => page_source(ui, etat, d, Module::Series, s, sortie),
        Page::FilmsNfo => page_nfo(ui, i, Module::Films, s),
        Page::SeriesNfo => page_nfo(ui, i, Module::Series, s),
        Page::FilmsExtracteur => page_extracteur(ui, i, Module::Films, s),
        Page::SeriesExtracteur => page_extracteur(ui, i, Module::Series, s),
        Page::FilmsIllustrations => page_illustrations(ui, i, Module::Films, s),
        Page::SeriesIllustrations => page_illustrations(ui, i, Module::Series, s),
        Page::FilmsBandesAnnonces => page_bandes_annonces(ui, etat, d, Module::Films, s, sortie),
        Page::SeriesBandesAnnonces => page_bandes_annonces(ui, etat, d, Module::Series, s, sortie),
        Page::FilmsSousTitres => page_sous_titres(ui, i, Module::Films, s),
        Page::SeriesSousTitres => page_sous_titres(ui, i, Module::Series, s),
        Page::FilmsRenommeur => page_renommeur(ui, etat, Module::Films, s),
        Page::SeriesRenommeur => page_renommeur(ui, etat, Module::Series, s),
        Page::FilmsPostTraitement | Page::SeriesPostTraitement => {
            bientot(ui, i, &["gui.settings.soon.scripts"])
        }
        Page::Collections => page_collections(ui, i, s),
    }
}

/// Champs communs du module ; n'emprunte que `films` ou `series`, le reste de `s` reste libre.
fn commun<'a>(
    m: Module,
    films: &'a mut ReglagesFilms,
    series: &'a mut ReglagesSeries,
) -> Commun<'a> {
    match m {
        Module::Films => commun_films(films),
        Module::Series => commun_series(series),
    }
}

// ---------------------------------------------------------------------------
// Général
// ---------------------------------------------------------------------------

fn page_general(ui: &mut Ui, etat: &Etat, r: &mut Reglages, sortie: &mut Sortie) {
    let i = &etat.i18n;
    grille("g_general").show(ui, |ui| {
        etiquette(ui, i, "Settings.language");
        egui::ComboBox::from_id_salt("langue")
            .selected_text(r.langue.nom_natif())
            .show_ui(ui, |ui| {
                for l in Langue::TOUTES {
                    ui.selectable_value(&mut r.langue, l, l.nom_natif());
                }
            });
        ui.end_row();
        etiquette(ui, i, "Settings.uitheme");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut r.theme, Theme::Clair, i.t("Settings.theme.light"));
            ui.selectable_value(&mut r.theme, Theme::Sombre, i.t("Settings.theme.dark"));
        });
        ui.end_row();
    });
    section(ui, i, "gui.settings.section.options");
    case(
        ui,
        i,
        &mut r.memoriser_module,
        "Settings.rememberlastmodule",
    );
    case(ui, i, &mut r.afficher_memoire, "Settings.showmemory");
    section(ui, i, "gui.update.section");
    case(ui, i, &mut r.verifier_mises_a_jour, "gui.update.atstartup");
    ui.horizontal(|ui| {
        if ui.button(format!("{}…", i.t("gui.update.check"))).clicked() {
            sortie.push(Message::RechercherMiseAJour { manuel: true });
        }
        ui.label(
            RichText::new(etat.config_mise_a_jour.courant.libelle())
                .weak()
                .small(),
        );
    });
    ui.add_space(8.0);
    note(ui, i.t("Settings.restartrequired"));
}

// ---------------------------------------------------------------------------
// Types de fichiers, tri par titre
// ---------------------------------------------------------------------------

fn page_types_fichiers(ui: &mut Ui, i: &I18n, s: &mut ReglagesServices) {
    let defauts = crate::source_donnees::OptionsGenerales::default();
    let g = &mut s.generales;
    note(ui, i.t("gui.settings.ext.hint"));
    for (cle, id, valeurs, defaut) in [
        (
            "gui.settings.ext.video",
            "ext_video",
            &mut g.extensions_video,
            defauts.extensions_video,
        ),
        (
            "gui.settings.ext.audio",
            "ext_audio",
            &mut g.extensions_audio,
            defauts.extensions_audio,
        ),
        (
            "gui.settings.ext.subtitles",
            "ext_st",
            &mut g.extensions_sous_titres,
            defauts.extensions_sous_titres,
        ),
    ] {
        ui.add_space(8.0);
        etiquette(ui, i, cle);
        liste(ui, i, id, valeurs, validation::extension, Some(defaut));
    }
    ui.add_space(8.0);
    etiquette(ui, i, "gui.settings.ext.cleanup");
    liste(
        ui,
        i,
        "motifs",
        &mut g.motifs_nettoyage,
        validation::motif,
        Some(defauts.motifs_nettoyage),
    );
}

fn page_tri_titre(ui: &mut Ui, i: &I18n, s: &mut ReglagesServices) {
    etiquette(ui, i, "gui.settings.sort.prefixes");
    liste(
        ui,
        i,
        "prefixes",
        &mut s.generales.prefixes_titre,
        validation::mot,
        Some(crate::source_donnees::OptionsGenerales::default().prefixes_titre),
    );
    section(ui, i, "gui.settings.sort.example");
    for titre in ["Le Parrain", "The Godfather", "Casablanca"] {
        let trie = titre_triable(titre, &s.generales.prefixes_titre);
        ui.label(format!("{titre}  →  {trie}"));
    }
}

// ---------------------------------------------------------------------------
// Outils externes
// ---------------------------------------------------------------------------

/// Page « Outils externes » : ffprobe / MediaInfo (sélecteur de fichier et test), yt-dlp et
/// lecteur multimédia.
fn page_outils(
    ui: &mut Ui,
    etat: &Etat,
    d: &EtatDialogueReglages,
    s: &mut ReglagesServices,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    ui.label(RichText::new(i.t("gui.tools.path")).strong());
    ui.horizontal(|ui| {
        ui.add(
            TextEdit::singleline(&mut s.chemin_outils)
                .hint_text(i.t("gui.tools.auto"))
                .desired_width(320.0),
        );
        if ui
            .button(format!("{}…", i.t("gui.folder.browse")))
            .on_hover_text(i.t("gui.tools.choose"))
            .clicked()
        {
            sortie.push(Message::ChoisirFichier(CibleFichier::OutilsAnalyse));
        }
        if ui
            .add_enabled(
                !d.test_outils_en_cours,
                egui::Button::new(i.t("gui.tools.test")),
            )
            .clicked()
        {
            sortie.push(Message::TesterOutils);
        }
    });
    ui.add_space(4.0);
    if d.test_outils_en_cours {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(i.t("gui.tools.testing"));
        });
    }
    match &d.test_outils {
        Some(Ok(diag)) if diag.disponible() => {
            for (nom, outil) in [("ffprobe", &diag.ffprobe), ("MediaInfo", &diag.mediainfo)] {
                let texte = match outil {
                    Some(o) => {
                        let version = o
                            .version
                            .clone()
                            .unwrap_or_else(|| i.t("gui.tools.noversion"));
                        format!("{} {nom} : {version}\n    {}", polices::COCHE, o.chemin)
                    }
                    None => format!("{nom} : {}", i.t("gui.tools.notfound")),
                };
                ui.label(texte);
            }
        }
        Some(Ok(_)) => {
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    polices::AVERTISSEMENT,
                    i.t("gui.tools.none")
                ))
                .color(crate::theme::ROUGE),
            );
        }
        Some(Err(e)) => {
            ui.label(
                RichText::new(format!("{} {e}", polices::AVERTISSEMENT)).color(crate::theme::ROUGE),
            );
        }
        None => {}
    }
    ui.add_space(8.0);
    note(ui, i.t("gui.tools.hint"));
    section(ui, i, "gui.settings.section.options");
    grille("g_outils").show(ui, |ui| {
        etiquette(ui, i, "gui.settings.tools.ytdlp");
        ui.add(
            TextEdit::singleline(&mut s.generales.chemin_yt_dlp)
                .hint_text(i.t("gui.tools.auto"))
                .desired_width(largeur_champ(ui)),
        );
        ui.end_row();
        etiquette(ui, i, "gui.settings.tools.player");
        ui.add(
            TextEdit::singleline(&mut s.generales.lecteur_media).desired_width(largeur_champ(ui)),
        );
        ui.end_row();
    });
}

// ---------------------------------------------------------------------------
// Services externes (clés d'API)
// ---------------------------------------------------------------------------

/// Texte court d'usage d'un service : traduction propre à l'interface, à défaut celui des
/// services.
fn usage_service(i: &I18n, c: &ServiceCle) -> String {
    let cle = format!("gui.settings.service.{}.usage", c.id);
    i.brut(&cle).map_or_else(|| c.usage.clone(), str::to_string)
}

/// Services dont une partie reste à venir : mention « Bientôt disponible » à côté du nom
/// (TheTVDB : fournisseur et test absents ; Simkl : aucun fournisseur de notes).
fn service_partiel(id: &str) -> bool {
    matches!(id, "tvdb" | "simkl")
}

/// Page « Services externes » : une ligne par service à clé d'API (état, saisie masquée,
/// enregistrement, suppression, test). Les valeurs des clés ne sont jamais affichées : le
/// champ de saisie est masqué et la saisie vit dans un type dont le `Debug` est expurgé.
pub(crate) fn page_services(
    ui: &mut Ui,
    etat: &Etat,
    d: &EtatDialogueReglages,
    _s: &ReglagesServices,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    ui.label(RichText::new(i.t("gui.settings.keys.title")).strong());
    note(ui, i.t("gui.settings.keys.note"));
    for c in &d.cles {
        ui.add_space(10.0);
        ligne_service(ui, i, d, c, sortie);
    }
}

fn ligne_service(
    ui: &mut Ui,
    i: &I18n,
    d: &EtatDialogueReglages,
    c: &ServiceCle,
    sortie: &mut Sortie,
) {
    let identifiant = est_identifiant_client(&c.id);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(&c.nom).strong());
        let (statut, couleur) = if c.cle_presente {
            (
                format!("{} {}", polices::COCHE, i.t("gui.settings.keys.stored")),
                None,
            )
        } else {
            (
                i.t("gui.settings.keys.missing"),
                Some(ui.visuals().weak_text_color()),
            )
        };
        let mut t = RichText::new(statut).small();
        if let Some(col) = couleur {
            t = t.color(col);
        }
        ui.label(t);
        if service_partiel(&c.id) {
            ui.label(RichText::new(i.t("gui.settings.soon")).weak().small());
        }
    });
    note(ui, usage_service(i, c));
    let saisie = d.saisies_cles.get(&c.id).cloned().unwrap_or_default();
    ui.horizontal_wrapped(|ui| {
        let mut valeur = saisie.valeur().to_string();
        let tip = if identifiant {
            i.t("gui.settings.keys.field.id.tip")
        } else {
            i.t("gui.settings.keys.field.key.tip")
        };
        let champ = ui
            .add(
                TextEdit::singleline(&mut valeur)
                    .password(true)
                    .hint_text(if identifiant {
                        i.t("gui.settings.keys.field.id")
                    } else {
                        i.t("gui.settings.keys.field.key")
                    })
                    .desired_width(240.0),
            )
            .on_hover_text(tip);
        if champ.changed() {
            sortie.push(Message::Dialogue(ActionDialogue::CleSaisie(
                c.id.clone(),
                Secret::new(valeur),
            )));
        }
        if ui
            .add_enabled(!saisie.est_vide(), egui::Button::new(i.t("Button.save")))
            .clicked()
        {
            sortie.push(Message::Dialogue(ActionDialogue::CleEnregistrer(
                c.id.clone(),
            )));
        }
        if ui
            .add_enabled(c.cle_presente, egui::Button::new(i.t("Button.remove")))
            .clicked()
        {
            sortie.push(Message::Dialogue(ActionDialogue::CleSupprimer(
                c.id.clone(),
            )));
        }
        let testable = cle_testable(&c.id);
        let test = ui.add_enabled(
            c.cle_presente && testable,
            egui::Button::new(i.t("gui.tools.test")),
        );
        let test = if testable {
            test.on_hover_text(i.t("gui.settings.keys.test.tip"))
        } else {
            test.on_disabled_hover_text(i.t("gui.settings.keys.test.unavailable.tip"))
        };
        if test.clicked() {
            sortie.push(Message::Dialogue(ActionDialogue::CleTester(c.id.clone())));
        }
    });
    if !cle_testable(&c.id) {
        note(ui, i.t("gui.settings.keys.test.unavailable"));
    }
    let lien = lien_obtention_cle(&c.id);
    if !lien.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(i.t("gui.settings.keys.getkey"))
                    .weak()
                    .small(),
            );
            ui.add(egui::Label::new(RichText::new(lien).monospace().small()).selectable(true));
        });
    }
}

// ---------------------------------------------------------------------------
// Système, divers
// ---------------------------------------------------------------------------

fn page_systeme(ui: &mut Ui, i: &I18n, s: &mut ReglagesServices) {
    ui.label(RichText::new(i.t("Settings.proxy")).strong());
    note(ui, i.t("gui.settings.system.proxy.hint"));
    ui.add_space(4.0);
    let largeur = largeur_champ(ui);
    grille("g_proxy").show(ui, |ui| {
        etiquette(ui, i, "Settings.proxy.host");
        ui.add(TextEdit::singleline(&mut s.proxy_hote).desired_width(largeur));
        ui.end_row();
        etiquette(ui, i, "Settings.proxy.port");
        ui.vertical(|ui| {
            ui.add(TextEdit::singleline(&mut s.proxy_port).desired_width(80.0));
            if let Err(e) = validation::port(&s.proxy_port) {
                erreur(ui, i, e);
            }
        });
        ui.end_row();
        etiquette(ui, i, "Settings.proxy.user");
        ui.add(TextEdit::singleline(&mut s.proxy_utilisateur).desired_width(largeur));
        ui.end_row();
        etiquette(ui, i, "Settings.proxy.password");
        ui.add(
            TextEdit::singleline(&mut s.proxy_mot_de_passe)
                .password(true)
                .desired_width(largeur),
        );
        ui.end_row();
    });
    section(ui, i, "gui.settings.section.options");
    case(
        ui,
        i,
        &mut s.ignorer_erreurs_ssl,
        "Settings.ignoresslerrors",
    );
    ui.horizontal(|ui| {
        etiquette(ui, i, "Settings.parallel");
        ui.add(egui::DragValue::new(&mut s.threads_telechargement).range(1..=16));
    });
}

fn page_divers(ui: &mut Ui, i: &I18n, s: &mut ReglagesServices) {
    case(ui, i, &mut s.cache_images, "Settings.imagecache");
    ui.add_enabled_ui(s.cache_images, |ui| {
        grille("g_cache").show(ui, |ui| {
            etiquette(ui, i, "gui.settings.misc.cachesize");
            choix(
                ui,
                i,
                "taille_cache",
                &mut s.generales.taille_cache_images,
                &[
                    (
                        TailleCacheImages::Petite,
                        "gui.settings.misc.cachesize.small",
                    ),
                    (
                        TailleCacheImages::Grande,
                        "gui.settings.misc.cachesize.large",
                    ),
                    (
                        TailleCacheImages::Originale,
                        "gui.settings.misc.cachesize.original",
                    ),
                ],
            )
            .on_hover_text(infobulle(i, "gui.settings.misc.cachesize"));
            ui.end_row();
            etiquette(ui, i, "gui.settings.misc.cachequality");
            choix(
                ui,
                i,
                "qualite_cache",
                &mut s.generales.qualite_cache_images,
                &[
                    (
                        QualiteCacheImages::Equilibree,
                        "gui.settings.misc.quality.balanced",
                    ),
                    (QualiteCacheImages::Haute, "gui.settings.misc.quality.high"),
                    (
                        QualiteCacheImages::Maximale,
                        "gui.settings.misc.quality.max",
                    ),
                ],
            )
            .on_hover_text(infobulle(i, "gui.settings.misc.cachequality"));
            ui.end_row();
        });
    });
    section(ui, i, "gui.settings.section.options");
    case(ui, i, &mut s.corbeille, "Settings.trash");
    ui.add_enabled_ui(s.corbeille, |ui| {
        case(
            ui,
            i,
            &mut s.generales.vider_corbeille_a_la_sortie,
            "gui.settings.misc.emptytrash",
        );
    });
}

// ---------------------------------------------------------------------------
// Films et séries : pages communes
// ---------------------------------------------------------------------------

/// Page racine d'un module : tâches automatiques et affichage.
fn page_racine(ui: &mut Ui, i: &I18n, m: Module, s: &mut ReglagesServices) {
    section(ui, i, "gui.settings.section.tasks");
    if m == Module::Films {
        case(
            ui,
            i,
            &mut s.renommer_apres_scraping,
            "Settings.renamer.afterscrape",
        );
    } else {
        case(
            ui,
            i,
            &mut s.series.renommer_apres_scraping,
            "Settings.renamer.afterscrape",
        );
    }
    let c = commun(m, &mut s.films, &mut s.series);
    case(
        ui,
        i,
        c.mise_a_jour_au_demarrage,
        "gui.settings.updateatstart",
    );
    case(ui, i, c.cache_a_l_import, "gui.settings.cacheatimport");
    case(
        ui,
        i,
        c.duree_depuis_analyse,
        "gui.settings.durationfromfile",
    );
    if m == Module::Series {
        section(ui, i, "gui.series.display");
        case(
            ui,
            i,
            &mut s.afficher_episodes_manquants,
            "gui.series.showmissing",
        );
        ui.add_enabled_ui(s.afficher_episodes_manquants, |ui| {
            case(
                ui,
                i,
                &mut s.afficher_speciaux_manquants,
                "gui.series.showmissingspecials",
            );
            case(
                ui,
                i,
                &mut s.afficher_non_diffuses,
                "gui.series.showunaired",
            );
        });
    }
}

/// Dossiers racines, dossiers ignorés, mots à retirer.
fn page_source(
    ui: &mut Ui,
    etat: &Etat,
    d: &EtatDialogueReglages,
    m: Module,
    s: &mut ReglagesServices,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    ui.label(RichText::new(i.t("Settings.folders")).strong());
    match m {
        Module::Films => liste_dossiers(
            ui,
            etat,
            &mut s.sources_films,
            &d.saisie_source,
            ActionDialogue::ReglagesSaisieSource,
            CibleDossier::SourceFilms,
            sortie,
        ),
        Module::Series => liste_dossiers(
            ui,
            etat,
            &mut s.sources_series,
            &d.saisie_source_serie,
            ActionDialogue::ReglagesSaisieSourceSerie,
            CibleDossier::SourceSeries,
            sortie,
        ),
    }
    section(ui, i, "gui.settings.section.scan");
    etiquette(ui, i, "gui.settings.excluded");
    match m {
        Module::Films => liste(
            ui,
            i,
            "ignores_films",
            &mut s.dossiers_ignores,
            validation::mot,
            None,
        ),
        Module::Series => liste(
            ui,
            i,
            "ignores_series",
            &mut s.series.dossiers_ignores,
            validation::mot,
            None,
        ),
    }
    ui.add_space(8.0);
    let c = commun(m, &mut s.films, &mut s.series);
    case(ui, i, c.ignorer_nomedia, "gui.settings.nomedia");
    ui.add_space(8.0);
    etiquette(ui, i, "gui.settings.badwords");
    liste(
        ui,
        i,
        m.sel("mots_films", "mots_series"),
        c.mots_exclus,
        validation::mot,
        None,
    );
}

/// Liste de dossiers éditable : retrait par ✖, ajout par le sélecteur natif (« Parcourir… »)
/// ou, en repli, par saisie manuelle.
fn liste_dossiers(
    ui: &mut Ui,
    etat: &Etat,
    dossiers: &mut Vec<String>,
    saisie: &str,
    message: fn(String) -> ActionDialogue,
    cible: CibleDossier,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    let mut a_retirer = None;
    if dossiers.is_empty() {
        ui.label(RichText::new(i.t("gui.folder.none")).weak());
    }
    for (n, dossier) in dossiers.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(dossier);
            if ui
                .small_button("✖")
                .on_hover_text(i.t("Button.remove"))
                .clicked()
            {
                a_retirer = Some(n);
            }
        });
    }
    if let Some(n) = a_retirer {
        dossiers.remove(n);
    }
    ui.add_space(4.0);
    if ui
        .button(format!("{}…", i.t("gui.folder.browse")))
        .on_hover_text(i.t("gui.folder.choose"))
        .clicked()
    {
        sortie.push(Message::ChoisirDossier(cible));
    }
    ui.horizontal(|ui| {
        let mut texte = saisie.to_string();
        let champ = ui.add(
            TextEdit::singleline(&mut texte)
                .hint_text(i.t("wizard.sources.hint"))
                .desired_width(320.0),
        );
        if champ.changed() {
            sortie.push(Message::Dialogue(message(texte.clone())));
        }
        let valide = champ.lost_focus() && ui.input(|e| e.key_pressed(egui::Key::Enter));
        let nouveau = texte.trim().to_string();
        if (ui.button(format!("+ {}", i.t("Button.add"))).clicked() || valide)
            && !nouveau.is_empty()
            && !dossiers.contains(&nouveau)
        {
            dossiers.push(nouveau);
            sortie.push(Message::Dialogue(message(String::new())));
        }
    });
}

fn page_nfo(ui: &mut Ui, i: &I18n, m: Module, s: &mut ReglagesServices) {
    {
        let c = commun(m, &mut s.films, &mut s.series);
        grille("g_nfo").show(ui, |ui| {
            etiquette(ui, i, "gui.settings.nfo.format");
            choix(ui, i, "format_nfo", c.format_nfo, &FORMATS_NFO)
                .on_hover_text(infobulle(i, "gui.settings.nfo.format"));
            ui.end_row();
        });
    }
    section(ui, i, "gui.settings.section.options");
    match m {
        Module::Films => {
            let f = &mut s.films;
            case(ui, i, &mut f.nfo_propre, "gui.settings.nfo.clean");
            ui.add_enabled_ui(!f.nfo_propre, |ui| {
                case(ui, i, &mut f.nfo_date_ajout, "gui.settings.nfo.dateadded");
                case(ui, i, &mut f.creer_synopsis, "gui.settings.nfo.outline");
                case(ui, i, &mut f.nfo_infos_fichier, "gui.settings.nfo.fileinfo");
                case(ui, i, &mut f.nfo_urls_artwork, "gui.settings.nfo.arturls");
            });
        }
        Module::Series => {
            let f = &mut s.series;
            case(ui, i, &mut f.nfo_guide_episodes, "gui.settings.nfo.guide");
            case(ui, i, &mut f.nfo_date_fin, "gui.settings.nfo.enddate");
            case(ui, i, &mut f.nfo_tous_acteurs, "gui.settings.nfo.allactors");
        }
    }
}

fn page_extracteur(ui: &mut Ui, i: &I18n, m: Module, s: &mut ReglagesServices) {
    let largeur = largeur_champ(ui).min(140.0);
    // Champs propres à un module, lus avant d'emprunter les champs communs.
    let (langue, seuil) = match m {
        Module::Films => (&mut s.langue_scraping, Some(&mut s.films.seuil_scraper)),
        Module::Series => (&mut s.series.langue_scraping, None),
    };
    grille("g_extracteur").show(ui, |ui| {
        etiquette(ui, i, "gui.settings.scraper.language");
        champ_valide(ui, i, langue, largeur, validation::langue);
        ui.end_row();
        if let Some(seuil) = seuil {
            etiquette(ui, i, "gui.settings.scraper.threshold");
            ui.add(
                egui::Slider::new(seuil, 0.0..=1.0)
                    .step_by(0.05)
                    .fixed_decimals(2),
            )
            .on_hover_text(infobulle(i, "gui.settings.scraper.threshold"));
            ui.end_row();
        }
    });
    // Le fournisseur et le pays sont des champs communs.
    let c = commun(m, &mut s.films, &mut s.series);
    grille("g_extracteur2").show(ui, |ui| {
        etiquette(ui, i, "gui.settings.scraper.provider");
        ui.vertical(|ui| {
            egui::ComboBox::from_id_salt(m.sel("fournisseur_films", "fournisseur_series"))
                .selected_text(c.scraper.to_uppercase())
                .show_ui(ui, |ui| {
                    for f in FOURNISSEURS {
                        ui.selectable_value(c.scraper, f.to_string(), f.to_uppercase());
                    }
                })
                .response
                .on_hover_text(infobulle(i, "gui.settings.scraper.provider"));
            note(ui, i.t("gui.settings.scraper.others"));
        });
        ui.end_row();
        etiquette(ui, i, "gui.settings.scraper.country");
        champ_valide(ui, i, c.pays, largeur.min(60.0), validation::pays);
        ui.end_row();
    });
    section(ui, i, "gui.settings.section.options");
    case(ui, i, c.toutes_notes, "gui.settings.scraper.allratings");
    if m == Module::Films {
        case(
            ui,
            i,
            &mut s.films.repli_scraper,
            "gui.settings.scraper.fallback",
        );
        case(
            ui,
            i,
            &mut s.films.ne_pas_ecraser,
            "gui.settings.scraper.nooverwrite",
        );
        case(
            ui,
            i,
            &mut s.renommer_apres_scraping,
            "Settings.renamer.afterscrape",
        );
    } else {
        case(
            ui,
            i,
            &mut s.series.renommer_apres_scraping,
            "Settings.renamer.afterscrape",
        );
    }
}

fn page_illustrations(ui: &mut Ui, i: &I18n, m: Module, s: &mut ReglagesServices) {
    {
        let c = commun(m, &mut s.films, &mut s.series);
        case(ui, i, c.meilleure_image, "gui.settings.art.best");
        section(ui, i, "gui.settings.section.sizes");
        grille("g_tailles").show(ui, |ui| {
            etiquette(ui, i, "gui.settings.art.poster");
            choix(ui, i, "taille_affiche", c.taille_affiche, &TAILLES_IMAGE)
                .on_hover_text(infobulle(i, "gui.settings.art.poster"));
            ui.end_row();
            etiquette(ui, i, "gui.settings.art.fanart");
            choix(ui, i, "taille_fanart", c.taille_fanart, &TAILLES_IMAGE)
                .on_hover_text(infobulle(i, "gui.settings.art.fanart"));
            ui.end_row();
            if let Some(vignette) = c.taille_vignette {
                etiquette(ui, i, "gui.settings.art.thumb");
                choix(ui, i, "taille_vignette", vignette, &TAILLES_IMAGE)
                    .on_hover_text(infobulle(i, "gui.settings.art.thumb"));
                ui.end_row();
            }
        });
    }
    section(ui, i, "gui.settings.section.counts");
    {
        let c = commun(m, &mut s.films, &mut s.series);
        grille("g_nombres").show(ui, |ui| {
            nombre_borne(
                ui,
                i,
                "gui.settings.art.postercount",
                c.nombre_affiches,
                1,
                NB_AFFICHES_MAX,
            );
            nombre_borne(
                ui,
                i,
                "gui.settings.art.fanartcount",
                c.nombre_fanarts,
                1,
                NB_FANARTS_MAX,
            );
        });
        case(ui, i, c.images_acteurs, "gui.settings.art.actors");
    }
    section(ui, i, "gui.settings.section.options");
    match m {
        Module::Films => {
            let f = &mut s.films;
            etiquette(ui, i, "gui.settings.art.languages");
            liste(
                ui,
                i,
                "langues_images",
                &mut f.langues_images,
                validation::langue,
                None,
            );
            ui.add_space(6.0);
            case(ui, i, &mut f.fanarts_extra, "gui.settings.art.extra");
            ui.add_enabled_ui(f.fanarts_extra, |ui| {
                ui.horizontal(|ui| {
                    etiquette(ui, i, "gui.settings.art.extracount");
                    ui.add(egui::DragValue::new(&mut f.nombre_fanarts_extra).range(0..=50));
                });
            });
        }
        Module::Series => {
            case(
                ui,
                i,
                &mut s.series.repli_artwork_saison,
                "gui.settings.art.seasonfallback",
            );
        }
    }
}

fn page_bandes_annonces(
    ui: &mut Ui,
    etat: &Etat,
    d: &EtatDialogueReglages,
    m: Module,
    s: &mut ReglagesServices,
    sortie: &mut Sortie,
) {
    let i = &etat.i18n;
    let c = commun(m, &mut s.films, &mut s.series);
    case(ui, i, c.bandes_auto, "gui.settings.trailer.auto");
    ui.add_space(6.0);
    let qualite = c.qualite_bande;
    grille("g_bandes").show(ui, |ui| {
        etiquette(ui, i, "gui.settings.trailer.quality");
        choix(ui, i, "qualite_bande", qualite, &QUALITES_BANDE)
            .on_hover_text(infobulle(i, "gui.settings.trailer.quality"));
        ui.end_row();
    });
    if m == Module::Films {
        let libelle = QUALITES_BANDE
            .iter()
            .find(|(q, _)| *q == *qualite)
            .map(|(_, cle)| i.t(cle))
            .unwrap_or_default();
        section(ui, i, "gui.settings.trailer.ytdlp");
        bloc_yt_dlp(ui, i, d, &libelle, sortie);
    }
}

/// Texte d'état de yt-dlp : version et chemin s'ils sont connus, sinon « introuvable ».
pub(crate) fn etat_yt_dlp(
    i: &I18n,
    detecte: bool,
    test: Option<&Result<Option<OutilAffiche>, String>>,
) -> (bool, String) {
    match test {
        Some(Ok(Some(o))) => {
            let version = o
                .version
                .clone()
                .unwrap_or_else(|| i.t("gui.tools.noversion"));
            (
                true,
                format!("{} yt-dlp : {version}\n    {}", polices::COCHE, o.chemin),
            )
        }
        Some(Ok(None)) => (false, i.t("gui.settings.trailer.ytdlp.notfound")),
        Some(Err(e)) => (false, e.clone()),
        None if detecte => (true, i.t("gui.settings.trailer.ytdlp.found")),
        None => (false, i.t("gui.settings.trailer.ytdlp.notfound")),
    }
}

/// État de yt-dlp (détecté ou non), bouton de test et rappel de la qualité visée.
fn bloc_yt_dlp(
    ui: &mut Ui,
    i: &I18n,
    d: &EtatDialogueReglages,
    qualite: &str,
    sortie: &mut Sortie,
) {
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !d.test_yt_dlp_en_cours,
                egui::Button::new(i.t("gui.tools.test")),
            )
            .on_hover_text(i.t("gui.settings.trailer.ytdlp.test.tip"))
            .clicked()
        {
            sortie.push(Message::TesterYtDlp);
        }
        if d.test_yt_dlp_en_cours {
            ui.spinner();
            ui.label(i.t("gui.tools.testing"));
        }
    });
    if !d.test_yt_dlp_en_cours {
        let (ok, texte) = etat_yt_dlp(i, d.yt_dlp_detecte, d.test_yt_dlp.as_ref());
        if ok {
            ui.label(texte);
        } else {
            ui.label(
                RichText::new(format!("{} {texte}", polices::AVERTISSEMENT))
                    .color(crate::theme::ROUGE),
            );
            note(ui, i.t("gui.trailer.noytdlp"));
        }
    }
    note(
        ui,
        i.t_args(
            "gui.settings.trailer.ytdlp.quality",
            &[("quality", qualite)],
        ),
    );
}

fn page_sous_titres(ui: &mut Ui, i: &I18n, m: Module, s: &mut ReglagesServices) {
    let largeur = largeur_champ(ui).min(140.0);
    let c = commun(m, &mut s.films, &mut s.series);
    grille("g_st").show(ui, |ui| {
        etiquette(ui, i, "gui.settings.subtitle.language");
        champ_valide(ui, i, c.langue_sous_titres, largeur, validation::langue);
        ui.end_row();
    });
}

fn page_collections(ui: &mut Ui, i: &I18n, s: &mut ReglagesServices) {
    let largeur = largeur_champ(ui);
    grille("g_collections").show(ui, |ui| {
        etiquette(ui, i, "gui.settings.sets.datafolder");
        ui.add(
            TextEdit::singleline(&mut s.films.dossier_donnees_sagas)
                .hint_text(i.t("gui.tools.auto"))
                .desired_width(largeur),
        );
        ui.end_row();
    });
    ui.add_space(6.0);
    case(
        ui,
        i,
        &mut s.films.afficher_films_manquants_sagas,
        "gui.settings.sets.showmissing",
    );
}

// ---------------------------------------------------------------------------
// Renommeur
// ---------------------------------------------------------------------------

/// Champ de modèle et position du curseur du dernier champ édité (mémoire d'egui : le clic
/// sur une variable retire le focus au champ, on retient donc où insérer).
#[derive(Clone, Copy)]
struct CibleInsertion {
    champ: ChampGabarit,
    debut: usize,
    fin: usize,
}

fn id_cible() -> Id {
    Id::unique("reglages_gabarit_cible")
}

fn id_champ(champ: ChampGabarit) -> Id {
    Id::unique(("reglages_gabarit_champ", champ))
}

fn page_renommeur(ui: &mut Ui, etat: &Etat, m: Module, s: &mut ReglagesServices) {
    let i = &etat.i18n;
    let champs: &[ChampGabarit] = match m {
        Module::Films => &ChampGabarit::FILMS,
        Module::Series => &ChampGabarit::SERIES,
    };
    let largeur = largeur_champ(ui);
    let ctx = ui.ctx().clone();
    let mut cible: Option<CibleInsertion> = ui.data_mut(|d| d.get_temp(id_cible()));
    if cible.is_none_or(|c| !champs.contains(&c.champ)) {
        cible = None;
    }

    // 1. Champs de modèle.
    grille(m.sel("g_renamer", "g_renamer_series")).show(ui, |ui| {
        for champ in champs {
            etiquette(ui, i, champ.cle());
            let id = id_champ(*champ);
            let sortie = TextEdit::singleline(champ.valeur_mut(s))
                .id(id)
                .font(egui::TextStyle::Monospace)
                .desired_width(largeur)
                .show(ui);
            sortie
                .response
                .response
                .clone()
                .on_hover_text(infobulle(i, champ.cle()));
            if let Some(plage) = sortie.cursor_range.filter(|_| sortie.response.has_focus()) {
                cible = Some(CibleInsertion {
                    champ: *champ,
                    debut: usize::from(plage.secondary.index),
                    fin: usize::from(plage.primary.index),
                });
            }
            ui.end_row();
        }
    });
    ui.horizontal(|ui| {
        if ui
            .button(i.t("gui.renamer.reset"))
            .on_hover_text(i.t("gui.renamer.reset.tip"))
            .clicked()
        {
            gabarits::retablir_defauts(s, champs);
        }
    });

    // 2. Aperçu.
    let selection = etat
        .selection_ordonnee()
        .first()
        .and_then(|id| etat.films.iter().find(|f| f.id == *id));
    let exemples = match m {
        Module::Films => Exemples::avec_film(selection),
        Module::Series => Exemples::default(),
    };
    section(ui, i, "gui.renamer.preview");
    let titre_exemple = match m {
        Module::Films if exemples.film_reel => i.t_args(
            "gui.renamer.preview.selected",
            &[("title", exemples.titre_film())],
        ),
        Module::Films => i.t_args(
            "gui.renamer.preview.demo",
            &[("title", exemples.titre_film())],
        ),
        Module::Series => i.t_args(
            "gui.renamer.preview.demo",
            &[("title", &exemples.serie.titre)],
        ),
    };
    note(ui, titre_exemple);
    grille(m.sel("g_apercu", "g_apercu_series")).show(ui, |ui| {
        for champ in champs {
            ui.label(i.t(champ.cle_apercu()));
            match gabarits::apercu(*champ, s, &exemples) {
                Ok(Some(nom)) if nom.is_empty() => {
                    ui.label(RichText::new(i.t("gui.renamer.preview.empty")).weak());
                }
                Ok(Some(nom)) => {
                    ui.label(RichText::new(nom).monospace());
                }
                Ok(None) => {
                    ui.label(RichText::new(i.t("gui.renamer.preview.unchanged")).weak());
                }
                Err(e) => {
                    ui.label(
                        RichText::new(format!(
                            "{} {}",
                            polices::AVERTISSEMENT,
                            i.t_args("gui.renamer.invalid", &[("error", &e)])
                        ))
                        .color(crate::theme::ROUGE),
                    );
                }
            }
            ui.end_row();
        }
    });
    note(ui, i.t("gui.renamer.hint.syntax"));

    // 3. Options de renommage.
    section(ui, i, "gui.settings.section.options");
    grille(m.sel("g_ren_options", "g_ren_options_series")).show(ui, |ui| {
        etiquette(ui, i, "Settings.renamer.replacement");
        ui.add(TextEdit::singleline(&mut s.caractere_remplacement).desired_width(40.0));
        ui.end_row();
    });
    ui.horizontal(|ui| {
        case(ui, i, &mut s.remplacer_espaces, "Settings.renamer.spaces");
        ui.add_enabled(
            s.remplacer_espaces,
            TextEdit::singleline(&mut s.caractere_espace).desired_width(30.0),
        );
    });
    case(ui, i, &mut s.ascii_seulement, "Settings.renamer.ascii");
    case(
        ui,
        i,
        &mut s.supprimer_dossiers_vides,
        "Settings.renamer.cleanup",
    );

    // 4. Variables disponibles : clic = insertion à la position du curseur.
    section(ui, i, "gui.renamer.variables");
    note(ui, i.t("gui.renamer.variables.hint"));
    let id_filtre = Id::unique(("reglages_gabarit_filtre", m.sel("f", "s")));
    let mut filtre: String = ui.data_mut(|d| d.get_temp(id_filtre)).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.add(
            TextEdit::singleline(&mut filtre)
                .hint_text(i.t("gui.renamer.variables.filter"))
                .desired_width(200.0),
        );
        match cible {
            Some(c) => note(
                ui,
                i.t_args(
                    "gui.renamer.variables.target",
                    &[("field", &i.t(c.champ.cle()))],
                ),
            ),
            None => note(ui, i.t("gui.renamer.variables.notarget")),
        }
    });
    ui.data_mut(|d| d.insert_temp(id_filtre, filtre.clone()));
    let toutes = champs[0].variables();
    let visibles = gabarits::filtrer(&toutes, &filtre);
    let mut choisie = None;
    ScrollArea::vertical()
        .id_salt(m.sel("variables_films", "variables_series"))
        .max_height(HAUTEUR_VARIABLES)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            if visibles.is_empty() {
                note(ui, i.t("gui.renamer.variables.none"));
            }
            ui.horizontal_wrapped(|ui| {
                for v in &visibles {
                    let description = i
                        .brut(&format!("gui.var.{}", v.nom))
                        .map(|_| i.t(&format!("gui.var.{}", v.nom)));
                    let aide = match description {
                        Some(d) => format!("{d}\n{}  →  {}", v.jeton(), v.expression),
                        None => format!("{}  →  {}", v.jeton(), v.expression),
                    };
                    if ui.small_button(v.nom).on_hover_text(aide).clicked() {
                        choisie = Some(**v);
                    }
                }
            });
        });
    if let Some(v) = choisie {
        // Sans champ actif, insertion à la fin du premier champ.
        let c = cible.unwrap_or_else(|| {
            let n = champs[0].valeur(s).chars().count();
            CibleInsertion {
                champ: champs[0],
                debut: n,
                fin: n,
            }
        });
        let (texte, curseur) = gabarits::inserer_variable(c.champ.valeur(s), c.debut, c.fin, v.nom);
        *c.champ.valeur_mut(s) = texte;
        // Curseur juste après la variable, focus rendu au champ.
        let id = id_champ(c.champ);
        let mut etat_champ = TextEdit::load_state(&ctx, id).unwrap_or_default();
        etat_champ
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(curseur))));
        etat_champ.store(&ctx, id);
        ctx.memory_mut(|mem| mem.request_focus(id));
        cible = Some(CibleInsertion {
            champ: c.champ,
            debut: curseur,
            fin: curseur,
        });
    }
    if let Some(c) = cible {
        ui.data_mut(|d| d.insert_temp(id_cible(), c));
    }
}
