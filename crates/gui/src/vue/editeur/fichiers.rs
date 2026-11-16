//! Onglet « Fichiers médias » : liste des fichiers vidéo à gauche ; à droite, les informations
//! techniques du fichier choisi (codec, conteneur, dimensions, cadence, débit, profondeur,
//! durée, 3D, HDR, rapports d'aspect) et les tableaux des flux audio et des sous-titres.

use super::Contexte;
use super::composants::{
    ColonneEdition, Entete, Glyphe, boutons_liste, cellule_case, cellule_crayon, cellule_entier,
    cellule_texte, champ_texte, etiquette, etiquette_haut, liste_deroulante, nombre_entier,
    tableau_edition,
};
use crate::details_film::{
    AudioEdit, FORMATS_3D, MediaEdit, RATIOS, SousTitresEdit, libelle_ratio,
};
use crate::dialogues::FilmEdite;
use crate::editeur_film::retirer;
use crate::vue::details::largeur_libelles;
use crate::vue::separateur_vertical;
use eframe::egui::{self, Align, Layout, RichText, ScrollArea, Ui, vec2};

/// Largeur de la liste des fichiers.
const LARGEUR_LISTE: f32 = 236.0;
const HAUTEUR_FLUX: f32 = 24.0 + 5.0 * 22.0 + 2.0;
const L_NUM: f32 = 120.0;

pub fn afficher(ui: &mut Ui, ct: &Contexte<'_>, b: &mut FilmEdite) {
    let i = ct.i;
    let hauteur = ui.available_height();
    let total = ui.available_width();
    let courant = b
        .selections
        .fichier
        .min(b.details.medias.len().saturating_sub(1));
    let mut choix = Some(courant);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        // Liste des fichiers.
        let noms: Vec<String> = b.details.medias.iter().map(|m| m.nom.clone()).collect();
        tableau_edition(
            ui,
            "ed_fichiers",
            LARGEUR_LISTE,
            hauteur,
            vec![ColonneEdition::reste(i.t("editor.media.filename"))],
            noms.len(),
            &mut choix,
            |row, k| {
                row.set_selected(k == courant);
                row.col(|ui| {
                    crate::vue::tableau::filets(ui);
                    ui.add(egui::Label::new(&noms[k]).truncate());
                });
                false
            },
        );
        separateur_vertical(ui, hauteur);
        let l_detail = total - LARGEUR_LISTE - 16.0;
        let Some(m) = b.details.medias.get_mut(courant) else {
            ui.label(RichText::new(i.t("editor.media.none")).weak());
            return;
        };
        let (sel_audio, sel_st) = (&mut b.selections.audio, &mut b.selections.sous_titre);
        // Le défilement vit dans une colonne : un ScrollArea hérite de la disposition de son
        // parent (ici horizontale).
        ui.vertical(|ui| {
            ScrollArea::vertical()
                .id_salt("ed_fichier_detail")
                .max_height(hauteur)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(l_detail - 12.0);
                    detail_fichier(ui, ct, m, l_detail - 12.0, sel_audio, sel_st);
                });
        });
    });
    // Un clic dans la liste change de fichier (et réinitialise les sélections de flux).
    if let Some(n) = choix
        && n != b.selections.fichier
    {
        b.selections.fichier = n;
        b.selections.audio = None;
        b.selections.sous_titre = None;
    }
}

fn detail_fichier(
    ui: &mut Ui,
    ct: &Contexte<'_>,
    m: &mut MediaEdit,
    largeur: f32,
    sel_audio: &mut Option<usize>,
    sel_st: &mut Option<usize>,
) {
    let i = ct.i;
    ui.spacing_mut().item_spacing = vec2(6.0, 7.0);
    ui.label(RichText::new(&m.nom).heading().strong());
    let colonne_g: Vec<String> = [
        "editor.media.codec",
        "editor.media.width",
        "editor.media.fps",
        "editor.media.depth",
        "editor.media.format3d",
        "editor.media.hdr",
        "editor.media.aspect",
        "editor.media.aspect2",
    ]
    .iter()
    .map(|c| i.t(c))
    .collect();
    let etiq = largeur_libelles(ui, &colonne_g) + 4.0;
    let etiq_d = largeur_libelles(
        ui,
        &[
            i.t("editor.media.container"),
            i.t("editor.media.height"),
            i.t("editor.media.bitrate"),
            i.t("editor.media.duration"),
        ],
    ) + 4.0;
    // Quatre lignes de deux champs.
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("editor.media.codec"));
        champ_texte(ui, &mut m.codec, L_NUM, false);
        ui.add_space(24.0);
        etiquette(ui, etiq_d, &i.t("editor.media.container"));
        champ_texte(ui, &mut m.format_conteneur, L_NUM, false);
    });
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("editor.media.width"));
        entier(ui, &mut m.largeur);
        ui.add_space(24.0);
        etiquette(ui, etiq_d, &i.t("editor.media.height"));
        entier(ui, &mut m.hauteur);
    });
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("editor.media.fps"));
        ui.add_sized(
            vec2(L_NUM, 22.0),
            egui::DragValue::new(&mut m.images_par_seconde)
                .range(0.0..=1000.0)
                .speed(0.01)
                .max_decimals(3),
        );
        ui.add_space(24.0);
        etiquette(ui, etiq_d, &i.t("editor.media.bitrate"));
        entier(ui, &mut m.debit);
    });
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("editor.media.depth"));
        entier(ui, &mut m.profondeur);
        ui.add_space(24.0);
        etiquette(ui, etiq_d, &i.t("editor.media.duration"));
        let mut minutes = i64::from(m.duree_minutes());
        if nombre_entier(ui, &mut minutes, L_NUM, 0, 1_000_000).changed() {
            m.definir_duree_minutes(minutes.max(0) as u32);
        }
    });
    ui.add_space(8.0);
    let l_liste = (largeur - etiq - 14.0 - 210.0).clamp(160.0, 420.0);
    // Format 3D et HDR.
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("editor.media.format3d"));
        let mut options: Vec<(String, String)> = vec![(String::new(), String::new())];
        options.extend(
            FORMATS_3D
                .iter()
                .map(|f| ((*f).to_string(), (*f).to_string())),
        );
        liste_deroulante(ui, "ed_format_3d", l_liste, &mut m.format_3d, &options);
    });
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("editor.media.hdr"));
        champ_texte(ui, &mut m.format_hdr, l_liste, false);
    });
    // Rapports d'aspect.
    let mut options: Vec<(String, String)> = vec![(String::new(), String::new())];
    options.extend(
        RATIOS
            .iter()
            .map(|(_, l)| ((*l).to_string(), (*l).to_string())),
    );
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("editor.media.aspect"));
        let mut choisi = libelle_ratio(m.ratio);
        if liste_deroulante(ui, "ed_ratio", l_liste, &mut choisi, &options) {
            m.ratio = ratio_depuis_libelle(&choisi).unwrap_or(0.0);
        }
        ui.add_space(24.0);
        let detecte = m.ratio_detecte();
        if ui
            .add_enabled(
                detecte.is_some(),
                egui::Button::new(i.t("editor.media.detect")).min_size(vec2(200.0, 24.0)),
            )
            .clicked()
            && let Some(r) = detecte
        {
            m.ratio = r;
        }
    });
    ui.horizontal(|ui| {
        etiquette(ui, etiq, &i.t("editor.media.aspect2"));
        let mut choisi = m.ratio2.map(libelle_ratio).unwrap_or_default();
        if liste_deroulante(ui, "ed_ratio2", l_liste, &mut choisi, &options) {
            m.ratio2 = ratio_depuis_libelle(&choisi);
        }
    });
    ui.add_space(10.0);
    flux_audio(ui, ct, m, largeur, etiq, sel_audio);
    ui.add_space(14.0);
    sous_titres(ui, ct, m, largeur, etiq, sel_st);
}

fn entier(ui: &mut Ui, valeur: &mut u32) {
    let mut v = i64::from(*valeur);
    if nombre_entier(ui, &mut v, L_NUM, 0, i64::from(u32::MAX)).changed() {
        *valeur = v.max(0) as u32;
    }
}

/// Valeur d'un rapport d'aspect désigné par son libellé de liste (`None` : aucun).
fn ratio_depuis_libelle(libelle: &str) -> Option<f32> {
    RATIOS.iter().find(|(_, l)| *l == libelle).map(|(r, _)| *r)
}

fn flux_audio(
    ui: &mut Ui,
    ct: &Contexte<'_>,
    m: &mut MediaEdit,
    largeur: f32,
    etiq: f32,
    sel: &mut Option<usize>,
) {
    let i = ct.i;
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            vec2(etiq, HAUTEUR_FLUX),
            Layout::top_down(Align::Max),
            |ui| {
                etiquette_haut(ui, etiq, &i.t("editor.media.audio"));
                let c = boutons_liste(ui, i, false, sel.is_some(), false, false);
                if c.ajouter {
                    m.audio.push(AudioEdit::default());
                    *sel = Some(m.audio.len() - 1);
                }
                if c.retirer {
                    retirer(&mut m.audio, sel);
                }
            },
        );
        let l_table = largeur - etiq - 10.0;
        let n = m.audio.len();
        let audio = &mut m.audio;
        tableau_edition(
            ui,
            "ed_audio",
            l_table,
            HAUTEUR_FLUX,
            vec![
                ColonneEdition::fixe(62.0, i.t("editor.media.codec")),
                ColonneEdition::fixe(62.0, i.t("editor.media.channels")),
                ColonneEdition::fixe(104.0, i.t("editor.media.bitrate")),
                ColonneEdition::fixe(118.0, i.t("editor.media.bitdepth")),
                ColonneEdition::fixe(60.0, i.t("editor.media.language")),
                ColonneEdition::fixe(56.0, i.t("editor.media.default")),
                ColonneEdition::fixe(56.0, i.t("editor.media.forced")),
                ColonneEdition::reste(i.t("editor.media.title")),
                ColonneEdition::pictogramme(Entete::Glyphe(Glyphe::Crayon)),
            ],
            n,
            sel,
            |row, k| {
                let a = &mut audio[k];
                let mut agi = cellule_texte(row, &mut a.codec);
                agi |= cellule_entier(row, &mut a.canaux);
                agi |= cellule_entier(row, &mut a.debit);
                agi |= cellule_entier(row, &mut a.profondeur);
                agi |= cellule_texte(row, &mut a.langue);
                agi |= cellule_case(row, &mut a.defaut);
                agi |= cellule_case(row, &mut a.force);
                agi |= cellule_texte(row, &mut a.titre);
                agi |= cellule_crayon(row, &i.t("editor.media.audio"));
                agi
            },
        );
    });
}

fn sous_titres(
    ui: &mut Ui,
    ct: &Contexte<'_>,
    m: &mut MediaEdit,
    largeur: f32,
    etiq: f32,
    sel: &mut Option<usize>,
) {
    let i = ct.i;
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            vec2(etiq, HAUTEUR_FLUX),
            Layout::top_down(Align::Max),
            |ui| {
                etiquette_haut(ui, etiq, &i.t("editor.media.subtitles"));
                let c = boutons_liste(ui, i, false, sel.is_some(), false, false);
                if c.ajouter {
                    m.sous_titres.push(SousTitresEdit::default());
                    *sel = Some(m.sous_titres.len() - 1);
                }
                if c.retirer {
                    retirer(&mut m.sous_titres, sel);
                }
            },
        );
        let l_table = largeur - etiq - 10.0;
        let n = m.sous_titres.len();
        let st = &mut m.sous_titres;
        tableau_edition(
            ui,
            "ed_sous_titres",
            l_table,
            HAUTEUR_FLUX,
            vec![
                ColonneEdition::fixe(60.0, i.t("editor.media.language")),
                ColonneEdition::fixe(56.0, i.t("editor.media.default")),
                ColonneEdition::fixe(56.0, i.t("editor.media.forced")),
                ColonneEdition::fixe(98.0, i.t("editor.media.hearing")),
                ColonneEdition::fixe(64.0, i.t("editor.media.format")),
                ColonneEdition::reste(i.t("editor.media.title")),
                ColonneEdition::pictogramme(Entete::Glyphe(Glyphe::Crayon)),
            ],
            n,
            sel,
            |row, k| {
                let s = &mut st[k];
                let mut agi = cellule_texte(row, &mut s.langue);
                agi |= cellule_case(row, &mut s.defaut);
                agi |= cellule_case(row, &mut s.force);
                agi |= cellule_case(row, &mut s.malentendant);
                agi |= cellule_texte(row, &mut s.format);
                agi |= cellule_texte(row, &mut s.titre);
                agi |= cellule_crayon(row, &i.t("editor.media.subtitles"));
                agi
            },
        );
    });
}
