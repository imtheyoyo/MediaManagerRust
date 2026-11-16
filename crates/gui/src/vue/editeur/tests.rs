//! Tests de rendu hors écran de la fenêtre « Éditer un film » : pour chaque onglet, les textes
//! attendus sont dessinés et la taille de la fenêtre ne bouge pas entre l'image 4 et l'image 20,
//! à plusieurs tailles d'écran (dont 1914×1026 et 1366×768).

use super::*;
use crate::details_film::{
    AudioEdit, BandeAnnonceEdit, MediaEdit, NoteSource, PersonneFilm, SousTitresEdit,
};
use crate::etat::Dialogue;
use crate::vue::essais::{ECRANS, verifier_stable_sur};

/// Film de démonstration très rempli (listes longues dans tous les onglets).
fn etat_riche(onglet: OngletEditeur, nombre: usize) -> (Etat, EtatEditeur) {
    let mut etat = Etat::nouveau();
    let film = etat.films[0].clone();
    let mut b = FilmEdite::depuis(&film, etat.source.details_film(&film));
    let d = &mut b.details;
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
    d.bandes_annonces = (0..nombre.min(14))
        .map(|k| BandeAnnonceEdit {
            nom: format!("Bande-annonce {k}"),
            source: "YouTube".into(),
            qualite: "1080p".into(),
            langue: "fra".into(),
            url: format!("https://www.youtube.com/watch?v={k}"),
            dans_nfo: k == 0,
            ..BandeAnnonceEdit::default()
        })
        .collect();
    d.medias = (0..nombre.min(6))
        .map(|k| MediaEdit {
            nom: format!("Fichier numéro {k} (2160p).mkv"),
            codec: "AV1".into(),
            format_conteneur: "matroska".into(),
            largeur: 3840,
            hauteur: 2160,
            images_par_seconde: 23.976,
            debit: 6986,
            profondeur: 10,
            duree_secondes: 10_320,
            format_hdr: "Dolby Vision".into(),
            ratio: 1.78,
            audio: (0..nombre.min(4))
                .map(|a| AudioEdit {
                    codec: "EAC3".into(),
                    canaux: 6,
                    debit: 640,
                    langue: "fra".into(),
                    defaut: a == 0,
                    titre: "FR VFF : DDP 5.1".into(),
                    ..AudioEdit::default()
                })
                .collect(),
            sous_titres: (0..nombre.min(3))
                .map(|_| SousTitresEdit {
                    langue: "fra".into(),
                    format: "UTF8".into(),
                    titre: "FR VFF Forcé : SRT".into(),
                    ..SousTitresEdit::default()
                })
                .collect(),
            ..MediaEdit::default()
        })
        .collect();
    b.genres = (0..nombre.min(5)).map(|k| format!("Genre {k}")).collect();
    let e = EtatEditeur {
        id: film.id,
        onglet,
        brouillon: b,
        options: Default::default(),
        erreurs: Vec::new(),
        file: Vec::new(),
        total: 1,
    };
    etat.dialogue = Some(Dialogue::Editeur(Box::new(e.clone())));
    (etat, e)
}

fn verifier_onglet(onglet: OngletEditeur, attendus: &[&str]) {
    verifier_onglet_avec(onglet, attendus, &[0, 3, 40]);
}

fn verifier_onglet_avec(onglet: OngletEditeur, attendus: &[&str], nombres: &[usize]) {
    // Le titre de la fenêtre et les six onglets sont toujours présents.
    for &nombre in nombres {
        let (etat, e) = etat_riche(onglet, nombre);
        let mut attendus: Vec<String> = attendus.iter().map(|s| s.to_string()).collect();
        attendus.push(etat.i18n.t("movie.edit"));
        attendus.extend(OngletEditeur::TOUS.iter().map(|o| etat.i18n.t(o.cle())));
        attendus.push(etat.i18n.t("Button.cancel"));
        attendus.push(etat.i18n.t("Button.ok"));
        let mut images = GestionnaireImages::new(std::sync::Arc::new(|| {}));
        // Grands écrans : tous les textes sont visibles ; petits écrans : la fenêtre reste stable
        // (le contenu défile).
        let grands = [vec2(1914.0, 1026.0), vec2(1366.0, 768.0)];
        let petits = [vec2(1280.0, 800.0), vec2(1050.0, 700.0)];
        verifier_stable_sur(&etat, "dlg_editeur", &grands, &attendus, |ctx, sortie| {
            afficher(ctx, &etat, &mut images, &e, sortie)
        });
        verifier_stable_sur(&etat, "dlg_editeur", &petits, &[], |ctx, sortie| {
            afficher(ctx, &etat, &mut images, &e, sortie)
        });
    }
}

#[test]
fn onglet_details_taille_stable() {
    verifier_onglet(
        OngletEditeur::Details,
        &[
            "Titre",
            "Titre original",
            "Titre anglais",
            "Trier par titre",
            "Citation",
            "Résumé",
            "Contour",
            "Année",
            "Date de sortie",
            "Production",
            "Pays",
            "Langues parlées",
            "Certification",
            "Évaluation des utilisateurs",
            "Top 250",
            "Cote d'évaluation",
            "Source d'évaluation",
            "Valeur maximale",
            "Nombre de votes",
            "Note",
            "Affiche",
        ],
    );
}

#[test]
fn onglet_details2_taille_stable() {
    verifier_onglet(
        OngletEditeur::Details2,
        &[
            "Date de l'ajout",
            "Vu",
            "Source",
            "3D",
            "Modifier",
            "Durée",
            "mn(s)",
            "Saga",
            "Lien vers la série",
            "identifiants",
            "Genres",
            "Mots clés",
        ],
    );
}

#[test]
fn onglet_distribution_taille_stable() {
    verifier_onglet(
        OngletEditeur::Distribution,
        &["Acteurs", "Équipe", "Nom", "Rôle"],
    );
}

#[test]
fn onglet_illustrations_taille_stable() {
    verifier_onglet(
        OngletEditeur::Illustrations,
        &[
            "Clearlogo",
            "Illustrations \"ClearArt\"",
            "Keyart (affiche)",
            "Bannière",
            "Miniature",
            "Illustration de disque",
            "Square art",
        ],
    );
}

#[test]
fn onglet_urls_taille_stable() {
    verifier_onglet(
        OngletEditeur::Urls,
        &[
            "Affiche",
            "Fanart (illustrations de fond)",
            "Clearlogo",
            "Bannière",
            "Illustrations \"ClearArt\"",
            "Miniature",
            "Illustration de disque",
            "Keyart (affiche)",
            "Square art",
            "Bande annonce",
            "NFO",
            "Qualité",
            "Langue",
            "URL",
        ],
    );
}

#[test]
fn onglet_fichiers_taille_stable() {
    verifier_onglet_avec(
        OngletEditeur::Fichiers,
        &[
            "Nom du fichier",
            "Codec",
            "Format du conteneur",
            "largeur",
            "Hauteur",
            "Fréquence d'images",
            "Débit binaire",
            "Profondeur de couleur vidéo",
            "Durée",
            "Format 3D",
            "format HDR",
            "Rapport hauteur/largeur",
            "2ème rapport d'aspect",
            "Détecter le rapport d'aspect",
            "Flux audio",
            "Canaux",
            "Profondeur de bit",
            "Sous-titres",
            "Malentendant",
        ],
        &[1, 3, 40],
    );
}

#[test]
fn fenetre_90_pour_cent_de_l_ecran() {
    for ecran in ECRANS {
        let t = taille_fenetre(ecran);
        assert!(
            t.x <= ecran.x - 24.0 && t.y <= ecran.y - 24.0,
            "{t:?} / {ecran:?}"
        );
        assert!(t.x >= ecran.x * 0.85 || t.x == 720.0, "{t:?} / {ecran:?}");
    }
    assert_eq!(taille_fenetre(vec2(1914.0, 1026.0)).x, (1914.0f32 * 0.9));
    // Écran immense : fenêtre bornée.
    assert_eq!(taille_fenetre(vec2(8000.0, 5000.0)), vec2(2000.0, 1300.0));
    // Écran minuscule : la fenêtre tient quand même.
    let t = taille_fenetre(vec2(600.0, 400.0));
    assert!(t.x <= 576.0 && t.y <= 376.0);
}
