//! Tests de rendu hors écran de la fenêtre « Éditer une série » : pour chaque onglet, les textes
//! attendus sont dessinés et la taille de la fenêtre ne bouge pas entre l'image 4 et l'image 20,
//! à plusieurs tailles d'écran (dont 1914×1026 et 1366×768).

use super::*;
use crate::details_film::{NoteSource, PersonneFilm};
use crate::details_serie::DetailsSerie;
use crate::etat::Dialogue;
use crate::source_donnees::{EpisodeAffichage, SaisonAffichage};
use crate::vue::essais::verifier_stable_sur;

/// Série de démonstration très remplie (listes longues dans tous les onglets).
fn etat_riche(onglet: OngletSerie, nombre: usize) -> (Etat, EtatEditeurSerie) {
    let mut etat = Etat::nouveau();
    let mut serie = etat.series[0].clone();
    serie.saisons = (1..=(nombre / 5 + 1) as i32)
        .map(|n| SaisonAffichage {
            numero: n,
            titre: String::new(),
            episodes: (1..=nombre.max(2) as i32)
                .map(|k| EpisodeAffichage {
                    episode: k,
                    titre: format!("Épisode {k}"),
                    fichier: format!("S{n:02}E{k:02}.mkv"),
                    vu: k % 2 == 0,
                    manquant: k % 7 == 0,
                    infos_media: "1080p H264 · AC3 6ch".into(),
                })
                .collect(),
        })
        .collect();
    etat.series[0] = serie.clone();
    let mut d = DetailsSerie::depuis_serie(&serie);
    d.acteurs = (0..nombre)
        .map(|k| PersonneFilm {
            nom: format!("Actrice numéro {k}"),
            role: format!("Rôle {k}"),
            url_vignette: if k % 3 == 0 {
                String::new()
            } else {
                "https://x/a.jpg".into()
            },
            url_profil: "https://x/p".into(),
            ..PersonneFilm::default()
        })
        .collect();
    d.equipe = (0..nombre / 2)
        .map(|k| PersonneFilm {
            nom: format!("Technicien {k}"),
            role: "Producer".into(),
            ..PersonneFilm::default()
        })
        .collect();
    d.notes = (0..nombre.min(8))
        .map(|k| NoteSource {
            source: format!("source{k}"),
            valeur: 7.5,
            maximum: 10,
            votes: 1000 + k as u32,
        })
        .collect();
    d.ids = (0..nombre.min(12))
        .map(|k| (format!("fournisseur{k}"), format!("{k}{k}{k}")))
        .collect();
    d.mots_cles = (0..nombre).map(|k| format!("mot clé {k}")).collect();
    let mut b = SerieEdite::depuis(&serie, d);
    b.genres = (0..nombre.min(9)).map(|k| format!("Genre {k}")).collect();
    let e = EtatEditeurSerie {
        id: serie.id.clone(),
        onglet,
        brouillon: b,
        options: Default::default(),
        erreurs: Vec::new(),
    };
    etat.dialogue = Some(Dialogue::EditeurSerie(Box::new(e.clone())));
    (etat, e)
}

/// `complets` : textes attendus à 1914×1026 (tout le formulaire tient) ; `hauts` : textes du
/// haut de l'onglet, attendus aussi à 1366×768 ; à plus petit, seule la stabilité est vérifiée.
fn verifier_onglet(onglet: OngletSerie, hauts: &[&str], complets: &[&str]) {
    for nombre in [0usize, 3, 40] {
        let (etat, e) = etat_riche(onglet, nombre);
        let mut communs: Vec<String> = vec![
            etat.i18n.t("gui.series.edit"),
            etat.i18n.t("Button.cancel"),
            etat.i18n.t("Button.ok"),
        ];
        communs.extend(OngletSerie::TOUS.iter().map(|o| etat.i18n.t(o.cle())));
        let mut images = GestionnaireImages::new(std::sync::Arc::new(|| {}));
        let mut attendus_hauts: Vec<String> = communs.clone();
        attendus_hauts.extend(hauts.iter().map(|s| s.to_string()));
        let mut attendus_complets = attendus_hauts.clone();
        attendus_complets.extend(complets.iter().map(|s| s.to_string()));
        verifier_stable_sur(
            &etat,
            "dlg_editeur_serie",
            &[vec2(1914.0, 1026.0)],
            &attendus_complets,
            |ctx, sortie| afficher(ctx, &etat, &mut images, &e, sortie),
        );
        verifier_stable_sur(
            &etat,
            "dlg_editeur_serie",
            &[vec2(1366.0, 768.0)],
            &attendus_hauts,
            |ctx, sortie| afficher(ctx, &etat, &mut images, &e, sortie),
        );
        verifier_stable_sur(
            &etat,
            "dlg_editeur_serie",
            &[vec2(1280.0, 800.0), vec2(1050.0, 700.0)],
            &[],
            |ctx, sortie| afficher(ctx, &etat, &mut images, &e, sortie),
        );
    }
}

#[test]
fn onglet_details_taille_stable() {
    verifier_onglet(
        OngletSerie::Details,
        &[
            "Titre",
            "Titre original",
            "Titre anglais",
            "Trier par titre",
            "Citation",
            "Résumé",
            "Année",
            "Première diffusion",
            "Statut",
            "Affiche",
        ],
        &[
            "Production",
            "Pays",
            "Langues parlées",
            "Certification",
            "Évaluation des utilisateurs",
            "Top 250",
            "Cote d'évaluation",
            "Source d'évaluation",
            "Note",
            "identifiants",
            "Fournisseur",
            "Genres",
            "Mots clés",
        ],
    );
}

#[test]
fn onglet_distribution_taille_stable() {
    verifier_onglet(
        OngletSerie::Distribution,
        &["Acteurs", "Équipe", "Nom", "Rôle"],
        &[],
    );
}

#[test]
fn onglet_illustrations_taille_stable() {
    verifier_onglet(
        OngletSerie::Illustrations,
        &["Clearlogo", "Bannière", "Miniature", "Square art"],
        &[],
    );
}

#[test]
fn onglet_urls_taille_stable() {
    verifier_onglet(
        OngletSerie::Urls,
        &["Affiche", "Fanart", "Clearlogo", "Square art"],
        &[],
    );
}

#[test]
fn onglet_episodes_taille_stable() {
    verifier_onglet(
        OngletSerie::Episodes,
        &["Titre", "Nom du fichier", "Saison 1", "Vu"],
        &[],
    );
}

#[test]
fn bascule_vu_depuis_l_arbre() {
    let (etat, e) = etat_riche(OngletSerie::Episodes, 3);
    let mut images = GestionnaireImages::new(std::sync::Arc::new(|| {}));
    // Un clic sur la cellule « vu » de la saison est envoyé comme action de l'éditeur : on
    // vérifie ici que l'affichage seul n'émet rien.
    let (_, _) = crate::vue::essais::rendre(
        &etat,
        "dlg_editeur_serie",
        vec2(1914.0, 1026.0),
        4,
        |ctx, sortie| {
            afficher(ctx, &etat, &mut images, &e, sortie);
            assert!(sortie.is_empty(), "{sortie:?}");
        },
    );
}
