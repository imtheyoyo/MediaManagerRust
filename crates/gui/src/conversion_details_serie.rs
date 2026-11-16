//! Conversions de la fenêtre « Éditer une série » : série des services → [`DetailsSerie`] et
//! report de l'édition (détails) dans la série des services. Fonctions pures, testées par un
//! aller-retour sans interface.

use crate::conversion_details::{
    appliquer_table_identifiants, appliquer_table_notes, images_du_film, personne_affichee,
    personne_modele, table_identifiants, type_core, type_personne,
};
use crate::dates;
use crate::details_film::{NoteSource, TypeArt};
use crate::details_serie::DetailsSerie;
use mediamanager_core::modele::{Note, Serie, TypePersonne};
use std::path::PathBuf;

/// Clé de la note de l'utilisateur dans la table des notes.
const SOURCE_UTILISATEUR: &str = "user";

/// Série des services → données de la fenêtre d'édition.
pub fn details(s: &Serie) -> DetailsSerie {
    let utilisateur = s
        .notes
        .get(SOURCE_UTILISATEUR)
        .filter(|n| n.est_valide())
        .map(Note::sur_dix)
        .unwrap_or(0.0);
    let mut urls: [String; 9] = Default::default();
    for t in TypeArt::TOUS {
        if let Some(u) = s.urls_artwork.get(&type_core(t)) {
            urls[t.rang()] = u.clone();
        }
    }
    DetailsSerie {
        titre_anglais: s.titre_anglais.clone(),
        titre_triable: if s.titre_tri == s.titre {
            String::new()
        } else {
            s.titre_tri.clone()
        },
        slogan: s.slogan.clone(),
        note_libre: s.note_utilisateur.clone(),
        premiere_diffusion: s
            .premiere_diffusion
            .as_deref()
            .map(dates::iso_vers_fr)
            .unwrap_or_default(),
        duree: s.duree_minutes,
        certification: s.certification.clone(),
        pays: s.pays.clone(),
        production: s.societe_production.clone(),
        evaluation_utilisateur: utilisateur,
        top250: s.top250,
        notes: s
            .notes
            .iter()
            .filter(|(k, _)| k.as_str() != SOURCE_UTILISATEUR)
            .map(|(k, n)| NoteSource {
                source: k.clone(),
                valeur: n.valeur.max(0.0),
                maximum: n.maximum,
                votes: n.votes,
            })
            .collect(),
        ids: table_identifiants(&s.ids),
        mots_cles: s.tags.clone(),
        acteurs: s.acteurs.iter().map(personne_affichee).collect(),
        equipe: s.equipe.iter().map(personne_affichee).collect(),
        images: images_du_film(&s.fichiers),
        urls,
    }
}

/// Reporte les détails de l'éditeur dans la série des services. Renvoie les fichiers
/// d'illustration dont la suppression est demandée : ils sont retirés de la série, leur mise à
/// la corbeille est faite par l'appelant.
pub fn appliquer_details(d: &DetailsSerie, s: &mut Serie) -> Vec<PathBuf> {
    s.titre_anglais = d.titre_anglais.trim().to_string();
    s.titre_tri = if d.titre_triable.trim().is_empty() {
        s.titre.clone()
    } else {
        d.titre_triable.trim().to_string()
    };
    s.slogan = d.slogan.clone();
    s.note_utilisateur = d.note_libre.clone();
    s.premiere_diffusion = dates::fr_vers_iso(&d.premiere_diffusion);
    s.duree_minutes = d.duree;
    s.certification = d.certification.trim().to_string();
    s.pays = d.pays.trim().to_string();
    s.societe_production = d.production.trim().to_string();
    s.top250 = d.top250;
    appliquer_table_notes(&d.notes, d.evaluation_utilisateur, &mut s.notes);
    appliquer_table_identifiants(&d.ids, &mut s.ids);
    s.tags = d
        .mots_cles
        .iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    let (acteurs, equipe) = (s.acteurs.clone(), s.equipe.clone());
    s.acteurs = d
        .acteurs
        .iter()
        .filter(|p| !p.nom.trim().is_empty())
        .map(|p| personne_modele(p, &acteurs, TypePersonne::Acteur))
        .collect();
    s.equipe = d
        .equipe
        .iter()
        .filter(|p| !p.nom.trim().is_empty())
        .map(|p| personne_modele(p, &equipe, type_personne(p.fonction_effective())))
        .collect();
    for t in TypeArt::TOUS {
        let url = d.urls[t.rang()].trim();
        if url.is_empty() {
            s.urls_artwork.remove(&type_core(t));
        } else {
            s.urls_artwork.insert(type_core(t), url.to_string());
        }
    }
    let mut supprimes = Vec::new();
    for t in TypeArt::TOUS {
        if !d.images.supprimes[t.rang()] {
            continue;
        }
        let Some(chemin) = &d.images.chemins[t.rang()] else {
            continue;
        };
        let avant = s.fichiers.len();
        s.fichiers
            .retain(|x| !(x.type_fichier == type_core(t) && x.chemin_complet() == *chemin));
        if s.fichiers.len() != avant {
            supprimes.push(chemin.clone());
        }
    }
    supprimes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversion;
    use crate::details_film::{FonctionPersonne, PersonneFilm};
    use mediamanager_core::modele::{FichierMedia, Personne, TypeFichier as TypeCore};
    use std::collections::BTreeMap;
    use std::path::Path;

    fn personne(nom: &str, role: &str, t: TypePersonne, tmdb: i64) -> Personne {
        let mut ids = mediamanager_core::modele::Identifiants::default();
        ids.definir("tmdb", &tmdb.to_string());
        Personne {
            type_personne: t,
            nom: nom.into(),
            role: role.into(),
            url_vignette: format!("https://x/{nom}.jpg"),
            url_profil: String::new(),
            ids,
        }
    }

    fn serie_riche(dossier: &Path) -> Serie {
        let mut s = Serie {
            id: "s1".into(),
            titre: "Les Disparus".into(),
            titre_tri: "Disparus (Les)".into(),
            titre_original: "The Vanished".into(),
            titre_anglais: "The Vanished".into(),
            slogan: "Où sont-ils ?".into(),
            resume: "Ils ont disparu.".into(),
            annee: 2019,
            premiere_diffusion: Some("2019-03-04".into()),
            duree_minutes: 52,
            certification: "FR:-12".into(),
            pays: "FR / BE".into(),
            societe_production: "Arte / Canal+".into(),
            genres: vec!["Drame".into()],
            tags: vec!["mystère".into(), "île".into()],
            note_utilisateur: "à finir".into(),
            top250: 7,
            chemin: dossier.to_path_buf(),
            ..Serie::default()
        };
        s.ids.definir("tmdb", "1399");
        s.ids.definir("tvdb", "121361");
        s.ids.definir("imdb", "tt0944947");
        s.notes = BTreeMap::from([
            ("tmdb".to_string(), Note::nouvelle("tmdb", 8.0, 4801, 10)),
            ("user".to_string(), Note::nouvelle("user", 7.5, 0, 10)),
        ]);
        s.acteurs = vec![personne("Anna Durand", "Léa", TypePersonne::Acteur, 1)];
        s.equipe = vec![personne(
            "Marc Petit",
            "Director",
            TypePersonne::Realisateur,
            2,
        )];
        s.urls_artwork = BTreeMap::from([(TypeCore::Poster, "https://x/p.jpg".to_string())]);
        s
    }

    #[test]
    fn aller_retour_sans_modification() {
        let d = std::env::temp_dir().join(format!("mmr-det-serie-{}", std::process::id()));
        let s = serie_riche(&d);
        let det = details(&s);
        assert_eq!(det.titre_triable, "Disparus (Les)");
        assert_eq!(det.premiere_diffusion, "04/03/2019");
        assert_eq!(det.evaluation_utilisateur, 7.5);
        assert_eq!(det.notes.len(), 1);
        assert_eq!(det.ids[0].0, "tmdb");
        assert_eq!(det.ids[2].0, "tvdb");
        let mut t = s.clone();
        let supprimes = appliquer_details(&det, &mut t);
        assert!(supprimes.is_empty());
        assert_eq!(t, s);
    }

    #[test]
    fn modifications_reportees() {
        let d = std::env::temp_dir().join(format!("mmr-det-serie2-{}", std::process::id()));
        let s = serie_riche(&d);
        let mut det = details(&s);
        det.pays = "France".into();
        det.premiere_diffusion = "05/06/2020".into();
        det.titre_triable = String::new();
        det.ids = vec![("tmdb".into(), "42".into())];
        det.mots_cles = vec!["  nouveau ".into(), String::new()];
        det.evaluation_utilisateur = 9.0;
        det.urls[TypeArt::Fanart.rang()] = "https://x/f.jpg".into();
        det.urls[TypeArt::Affiche.rang()] = String::new();
        det.equipe.push(PersonneFilm {
            nom: "Zoé Martin".into(),
            role: "Writer".into(),
            fonction: FonctionPersonne::Scenariste,
            ..PersonneFilm::default()
        });
        let mut t = s.clone();
        appliquer_details(&det, &mut t);
        assert_eq!(t.pays, "France");
        assert_eq!(t.premiere_diffusion.as_deref(), Some("2020-06-05"));
        assert_eq!(t.titre_tri, t.titre);
        assert_eq!(t.ids.tmdb(), 42);
        assert_eq!(t.ids.tvdb(), 0);
        assert_eq!(t.tags, ["nouveau"]);
        assert_eq!(t.notes["user"].sur_dix(), 9.0);
        assert_eq!(t.urls_artwork.len(), 1);
        assert_eq!(t.equipe.len(), 2);
        assert_eq!(t.equipe[1].type_personne, TypePersonne::Scenariste);
        // Le reste de la série, et son affichage, restent cohérents.
        let r = mediamanager_store::Reglages::default();
        let a = conversion::serie(&t, &r);
        assert_eq!(a.tmdb, "42");
    }

    #[test]
    fn illustration_supprimee_retiree_de_la_serie() {
        let dossier = std::env::temp_dir().join(format!("mmr-det-serie3-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dossier);
        std::fs::create_dir_all(&dossier).unwrap();
        std::fs::write(dossier.join("poster.jpg"), b"x").unwrap();
        let mut s = serie_riche(&dossier);
        s.fichiers = vec![FichierMedia {
            nom: "poster.jpg".into(),
            chemin: dossier.clone(),
            type_fichier: TypeCore::Poster,
            ..FichierMedia::default()
        }];
        let mut det = details(&s);
        assert!(det.images.chemin(TypeArt::Affiche).is_some());
        det.images.supprimes[TypeArt::Affiche.rang()] = true;
        let mut t = s.clone();
        let supprimes = appliquer_details(&det, &mut t);
        assert_eq!(supprimes, vec![dossier.join("poster.jpg")]);
        assert!(t.fichiers.is_empty());
        let _ = std::fs::remove_dir_all(&dossier);
    }
}
