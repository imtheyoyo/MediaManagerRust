//! Conversions de la fenêtre « Éditer un film » : film des services → [`DetailsFilm`] et report
//! de l'édition complète (champs de base + détails) dans le film des services. Fonctions
//! pures, testées par un aller-retour sans interface.

use crate::conversion::appliquer_edition;
use crate::dates;
use crate::details_film::{
    AudioEdit, BandeAnnonceEdit, DetailsFilm, FonctionPersonne, ImagesFilm, MediaEdit, NoteSource,
    PersonneFilm, SousTitresEdit, TypeArt,
};
use crate::modele::Film;
use mediamanager_core::modele::commun::BandeAnnonce;
use mediamanager_core::modele::{
    self as cm, DrapeauFlux, Edition, FichierMedia, FluxAudio, FluxSousTitres, Note, Personne,
    SourceMedia, TypeFichier as TypeCore, TypePersonne,
};
use std::path::{Path, PathBuf};

/// Clé de la note de l'utilisateur dans la table des notes.
const SOURCE_UTILISATEUR: &str = "user";

fn type_core(t: TypeArt) -> TypeCore {
    match t {
        TypeArt::Affiche => TypeCore::Poster,
        TypeArt::Fanart => TypeCore::Fanart,
        TypeArt::Clearlogo => TypeCore::Clearlogo,
        TypeArt::Banniere => TypeCore::Banner,
        TypeArt::ClearArt => TypeCore::Clearart,
        TypeArt::Miniature => TypeCore::Thumb,
        TypeArt::Disque => TypeCore::Disc,
        TypeArt::Keyart => TypeCore::Keyart,
        TypeArt::SquareArt => TypeCore::Squareart,
    }
}

/// Code (`SOURCES_MEDIA`) d'une source média ; vide si inconnue.
pub fn code_source_media(s: SourceMedia) -> &'static str {
    match s {
        SourceMedia::UhdBluray => "UHD_BLURAY",
        SourceMedia::Bluray => "BLURAY",
        SourceMedia::Dvd => "DVD",
        SourceMedia::HdDvd => "HDDVD",
        SourceMedia::Tv => "TV",
        SourceMedia::Vhs => "VHS",
        SourceMedia::Laserdisc => "LASERDISC",
        SourceMedia::DVhs => "D_VHS",
        SourceMedia::HdRip => "HDRIP",
        SourceMedia::Cam => "CAM",
        SourceMedia::Ts => "TS",
        SourceMedia::Tc => "TC",
        SourceMedia::DvdScr => "DVDSCR",
        SourceMedia::R5 => "R5",
        SourceMedia::WebRip => "WEBRIP",
        SourceMedia::WebDl => "WEB_DL",
        SourceMedia::Stream => "STREAM",
        SourceMedia::Inconnue => "",
    }
}

/// Source média désignée par son code ; `Inconnue` pour un code vide ou non reconnu.
pub fn source_media_depuis_code(code: &str) -> SourceMedia {
    [
        SourceMedia::UhdBluray,
        SourceMedia::Bluray,
        SourceMedia::Dvd,
        SourceMedia::HdDvd,
        SourceMedia::Tv,
        SourceMedia::Vhs,
        SourceMedia::Laserdisc,
        SourceMedia::DVhs,
        SourceMedia::HdRip,
        SourceMedia::Cam,
        SourceMedia::Ts,
        SourceMedia::Tc,
        SourceMedia::DvdScr,
        SourceMedia::R5,
        SourceMedia::WebRip,
        SourceMedia::WebDl,
        SourceMedia::Stream,
    ]
    .into_iter()
    .find(|s| code_source_media(*s) == code.trim())
    .unwrap_or(SourceMedia::Inconnue)
}

fn fonction(t: TypePersonne) -> FonctionPersonne {
    match t {
        TypePersonne::Acteur => FonctionPersonne::Acteur,
        TypePersonne::Realisateur => FonctionPersonne::Realisateur,
        TypePersonne::Scenariste => FonctionPersonne::Scenariste,
        TypePersonne::Producteur => FonctionPersonne::Producteur,
        TypePersonne::Invite => FonctionPersonne::Invite,
        TypePersonne::Compositeur => FonctionPersonne::Compositeur,
        TypePersonne::Monteur => FonctionPersonne::Monteur,
        TypePersonne::Camera => FonctionPersonne::Camera,
        TypePersonne::Autre => FonctionPersonne::Autre,
    }
}

fn type_personne(f: FonctionPersonne) -> TypePersonne {
    match f {
        FonctionPersonne::Acteur => TypePersonne::Acteur,
        FonctionPersonne::Realisateur => TypePersonne::Realisateur,
        FonctionPersonne::Scenariste => TypePersonne::Scenariste,
        FonctionPersonne::Producteur => TypePersonne::Producteur,
        FonctionPersonne::Invite => TypePersonne::Invite,
        FonctionPersonne::Compositeur => TypePersonne::Compositeur,
        FonctionPersonne::Monteur => TypePersonne::Monteur,
        FonctionPersonne::Camera => TypePersonne::Camera,
        FonctionPersonne::Autre => TypePersonne::Autre,
    }
}

fn personne_affichee(p: &Personne) -> PersonneFilm {
    PersonneFilm {
        nom: p.nom.clone(),
        role: p.role.clone(),
        url_vignette: p.url_vignette.clone(),
        url_profil: p.url_profil.clone(),
        fonction: fonction(p.type_personne),
    }
}

/// Fiche du modèle correspondant à une ligne de l'éditeur : l'existante (même nom et même
/// rôle, sinon même nom et même type) garde ses identifiants.
fn personne_modele(p: &PersonneFilm, existantes: &[Personne], t: TypePersonne) -> Personne {
    let nom = p.nom.trim();
    let base = existantes
        .iter()
        .find(|e| e.nom == nom && e.role == p.role)
        .or_else(|| {
            existantes
                .iter()
                .find(|e| e.nom == nom && e.type_personne == t)
        });
    Personne {
        type_personne: t,
        nom: nom.to_string(),
        role: p.role.clone(),
        url_vignette: p.url_vignette.trim().to_string(),
        url_profil: p.url_profil.trim().to_string(),
        ids: base.map(|b| b.ids.clone()).unwrap_or_default(),
    }
}

fn texte_chemin(p: &Path) -> String {
    p.display().to_string()
}

/// Illustrations existantes sur le disque, une par emplacement de [`TypeArt::TOUS`].
pub fn images_du_film(fichiers: &[FichierMedia]) -> ImagesFilm {
    let mut images = ImagesFilm::default();
    for t in TypeArt::TOUS {
        let chemin = fichiers
            .iter()
            .filter(|f| f.type_fichier == type_core(t))
            .map(FichierMedia::chemin_complet)
            .find(|p| p.is_file());
        images.dimensions[t.rang()] = chemin
            .as_deref()
            .and_then(|p| image::image_dimensions(p).ok());
        images.chemins[t.rang()] = chemin;
    }
    images
}

fn drapeau(drapeaux: &[DrapeauFlux], d: DrapeauFlux) -> bool {
    drapeaux.contains(&d)
}

fn definir_drapeau(drapeaux: &mut Vec<DrapeauFlux>, d: DrapeauFlux, actif: bool) {
    if actif {
        if !drapeaux.contains(&d) {
            drapeaux.push(d);
        }
    } else {
        drapeaux.retain(|x| *x != d);
    }
}

fn media_affiche(x: &FichierMedia) -> MediaEdit {
    let v = &x.video;
    MediaEdit {
        chemin: texte_chemin(&x.chemin_complet()),
        nom: x.nom.clone(),
        codec: v.codec.clone(),
        format_conteneur: x.format_conteneur.clone(),
        largeur: v.largeur,
        hauteur: v.hauteur,
        images_par_seconde: v.images_par_seconde,
        debit: v.debit,
        profondeur: v.profondeur_bits,
        duree_secondes: x.duree_secondes,
        format_3d: v.format_3d.clone(),
        format_hdr: v.format_hdr.clone(),
        ratio: v.ratio_aspect,
        ratio2: v.ratio_aspect2,
        audio: x
            .flux_audio
            .iter()
            .enumerate()
            .map(|(n, a)| AudioEdit {
                origine: Some(n),
                codec: a.codec.clone(),
                canaux: a.canaux,
                debit: a.debit,
                profondeur: a.profondeur_bits,
                langue: a.langue.clone(),
                defaut: drapeau(&a.drapeaux, DrapeauFlux::Defaut),
                force: drapeau(&a.drapeaux, DrapeauFlux::Force),
                titre: a.titre.clone(),
            })
            .collect(),
        sous_titres: x
            .sous_titres
            .iter()
            .enumerate()
            .map(|(n, s)| SousTitresEdit {
                origine: Some(n),
                langue: s.langue.clone(),
                defaut: drapeau(&s.drapeaux, DrapeauFlux::Defaut),
                force: drapeau(&s.drapeaux, DrapeauFlux::Force),
                malentendant: drapeau(&s.drapeaux, DrapeauFlux::Malentendants),
                format: s.codec.clone(),
                titre: s.titre.clone(),
            })
            .collect(),
    }
}

/// Identifiants triés : tmdb et imdb d'abord, puis l'ordre alphabétique.
fn identifiants(f: &cm::Film) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = f
        .ids
        .iter()
        .map(|(k, val)| (k.clone(), val.en_texte()))
        .collect();
    let rang = |k: &str| match k {
        "tmdb" => 0,
        "imdb" => 1,
        _ => 2,
    };
    v.sort_by(|a, b| rang(&a.0).cmp(&rang(&b.0)).then_with(|| a.0.cmp(&b.0)));
    v
}

/// Film des services → données de la fenêtre d'édition. `saga` n'y figure pas : le titre de la
/// saga est porté par le film d'affichage.
pub fn details(f: &cm::Film) -> DetailsFilm {
    let utilisateur = f
        .notes
        .get(SOURCE_UTILISATEUR)
        .filter(|n| n.est_valide())
        .map(Note::sur_dix)
        .unwrap_or(0.0);
    let mut urls: [String; 9] = Default::default();
    for t in TypeArt::TOUS {
        if let Some(u) = f.urls_artwork.get(&type_core(t)) {
            urls[t.rang()] = u.clone();
        }
    }
    DetailsFilm {
        titre_anglais: f.titre_anglais.clone(),
        contour: f.synopsis.clone(),
        note_libre: f.note_utilisateur.clone(),
        langues_parlees: f.langues_parlees.clone(),
        evaluation_utilisateur: utilisateur,
        top250: f.top250,
        notes: f
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
        date_ajout: dates::ms_vers_fr_heure(f.date_ajout),
        date_ajout_ms: f.date_ajout,
        date_vu: f
            .derniere_lecture
            .map(dates::ms_vers_fr_heure)
            .unwrap_or_default(),
        date_vu_ms: f.derniere_lecture,
        source_media: code_source_media(f.source_media).to_string(),
        video_3d: f.video_3d,
        edition: f.edition.libelle(),
        liens_series: f.liens_series.clone(),
        ids: identifiants(f),
        mots_cles: f.tags.clone(),
        acteurs: f.acteurs.iter().map(personne_affichee).collect(),
        equipe: f.equipe.iter().map(personne_affichee).collect(),
        images: images_du_film(&f.fichiers),
        urls,
        bandes_annonces: f
            .bandes_annonces
            .iter()
            .map(|b| BandeAnnonceEdit {
                id: b.id.clone(),
                nom: b.nom.clone(),
                source: b.fournisseur.clone(),
                qualite: b.qualite.clone(),
                langue: b.langue.clone(),
                url: b.url.clone(),
                dans_nfo: b.dans_nfo,
            })
            .collect(),
        medias: f
            .fichiers_de_type(TypeCore::Video)
            .map(media_affiche)
            .collect(),
    }
}

/// Notes du tableau reportées dans la table des notes (la note de l'utilisateur est conservée
/// ou mise à jour d'après l'évaluation saisie).
fn appliquer_notes(d: &DetailsFilm, f: &mut cm::Film) {
    let ancienne = f.notes.get(SOURCE_UTILISATEUR).cloned();
    f.notes.clear();
    for n in &d.notes {
        let source = n.source.trim();
        if source.is_empty() || source == SOURCE_UTILISATEUR {
            continue;
        }
        f.notes.insert(
            source.to_string(),
            Note::nouvelle(
                source,
                if n.valeur > 0.0 { n.valeur } else { -1.0 },
                n.votes,
                n.maximum,
            ),
        );
    }
    if d.evaluation_utilisateur > 0.0 {
        let mut n = ancienne.unwrap_or_else(|| Note::nouvelle(SOURCE_UTILISATEUR, 0.0, 0, 10));
        // L'évaluation est saisie sur 10 : elle est ramenée à l'échelle de la note d'origine.
        let maximum = if n.maximum == 0 { 10 } else { n.maximum };
        n.maximum = maximum;
        if (n.sur_dix() - d.evaluation_utilisateur).abs() > 0.005 {
            n.valeur = d.evaluation_utilisateur * maximum as f32 / 10.0;
        }
        f.notes.insert(SOURCE_UTILISATEUR.to_string(), n);
    }
}

fn appliquer_identifiants(d: &DetailsFilm, f: &mut cm::Film) {
    let cles: Vec<String> = d
        .ids
        .iter()
        .map(|(k, _)| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect();
    let anciennes: Vec<String> = f.ids.iter().map(|(k, _)| k.clone()).collect();
    for k in anciennes {
        if !cles.contains(&k) {
            f.ids.definir(&k, "");
        }
    }
    for (k, v) in &d.ids {
        let k = k.trim();
        if !k.is_empty() {
            f.ids.definir(k, v);
        }
    }
}

fn appliquer_media(m: &MediaEdit, f: &mut FichierMedia) {
    f.video.codec = m.codec.trim().to_string();
    f.format_conteneur = m.format_conteneur.trim().to_string();
    f.video.largeur = m.largeur;
    f.video.hauteur = m.hauteur;
    f.video.images_par_seconde = m.images_par_seconde;
    f.video.debit = m.debit;
    f.video.profondeur_bits = m.profondeur;
    f.duree_secondes = m.duree_secondes;
    f.video.format_3d = m.format_3d.trim().to_string();
    f.video.format_hdr = m.format_hdr.trim().to_string();
    f.video.ratio_aspect = m.ratio;
    f.video.ratio_aspect2 = m.ratio2;
    let audio_origine = std::mem::take(&mut f.flux_audio);
    f.flux_audio = m
        .audio
        .iter()
        .map(|a| {
            let mut x: FluxAudio = a
                .origine
                .and_then(|n| audio_origine.get(n))
                .cloned()
                .unwrap_or_default();
            x.codec = a.codec.trim().to_string();
            x.canaux = a.canaux;
            x.debit = a.debit;
            x.profondeur_bits = a.profondeur;
            x.langue = a.langue.trim().to_string();
            x.titre = a.titre.clone();
            definir_drapeau(&mut x.drapeaux, DrapeauFlux::Defaut, a.defaut);
            definir_drapeau(&mut x.drapeaux, DrapeauFlux::Force, a.force);
            x
        })
        .collect();
    let st_origine = std::mem::take(&mut f.sous_titres);
    f.sous_titres = m
        .sous_titres
        .iter()
        .map(|s| {
            let mut x: FluxSousTitres = s
                .origine
                .and_then(|n| st_origine.get(n))
                .cloned()
                .unwrap_or_default();
            x.codec = s.format.trim().to_string();
            x.langue = s.langue.trim().to_string();
            x.titre = s.titre.clone();
            definir_drapeau(&mut x.drapeaux, DrapeauFlux::Defaut, s.defaut);
            definir_drapeau(&mut x.drapeaux, DrapeauFlux::Force, s.force);
            definir_drapeau(&mut x.drapeaux, DrapeauFlux::Malentendants, s.malentendant);
            x
        })
        .collect();
}

/// Reporte les détails de l'éditeur dans le film des services. `acteurs` et `equipe` sont les
/// listes d'origine (leurs identifiants sont conservés). Renvoie les fichiers d'illustration
/// dont la suppression est demandée : ils sont retirés du film, leur mise à la corbeille est
/// faite par l'appelant.
pub fn appliquer_details(
    d: &DetailsFilm,
    f: &mut cm::Film,
    acteurs: &[Personne],
    equipe: &[Personne],
) -> Vec<PathBuf> {
    f.titre_anglais = d.titre_anglais.trim().to_string();
    f.synopsis = d.contour.clone();
    f.note_utilisateur = d.note_libre.clone();
    f.langues_parlees = d.langues_parlees.trim().to_string();
    f.top250 = d.top250;
    appliquer_notes(d, f);
    if let Some(ms) = dates::resoudre_horodatage(&d.date_ajout, Some(d.date_ajout_ms)) {
        f.date_ajout = ms;
    }
    f.derniere_lecture = dates::resoudre_horodatage(&d.date_vu, d.date_vu_ms);
    f.source_media = source_media_depuis_code(&d.source_media);
    f.video_3d = d.video_3d;
    f.edition = match Edition::depuis_chaine(&d.edition) {
        Edition::Aucune if !d.edition.trim().is_empty() => {
            Edition::Personnalisee(d.edition.trim().to_string())
        }
        e => e,
    };
    f.liens_series = d
        .liens_series
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    appliquer_identifiants(d, f);
    f.tags = d
        .mots_cles
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    f.acteurs = d
        .acteurs
        .iter()
        .filter(|p| !p.nom.trim().is_empty())
        .map(|p| personne_modele(p, acteurs, TypePersonne::Acteur))
        .collect();
    f.equipe = d
        .equipe
        .iter()
        .filter(|p| !p.nom.trim().is_empty())
        .map(|p| personne_modele(p, equipe, type_personne(p.fonction_effective())))
        .collect();
    for t in TypeArt::TOUS {
        let url = d.urls[t.rang()].trim();
        if url.is_empty() {
            f.urls_artwork.remove(&type_core(t));
        } else {
            f.urls_artwork.insert(type_core(t), url.to_string());
        }
    }
    let anciennes = std::mem::take(&mut f.bandes_annonces);
    f.bandes_annonces = d
        .bandes_annonces
        .iter()
        .map(|b| {
            let mut x: BandeAnnonce = anciennes
                .iter()
                .find(|a| !b.id.is_empty() && a.id == b.id)
                .cloned()
                .unwrap_or_default();
            x.id = b.id.clone();
            x.nom = b.nom.clone();
            x.fournisseur = b.source.clone();
            x.qualite = b.qualite.clone();
            x.langue = b.langue.clone();
            x.url = b.url.trim().to_string();
            x.dans_nfo = b.dans_nfo;
            x
        })
        .collect();
    for m in &d.medias {
        if let Some(x) = f
            .fichiers
            .iter_mut()
            .find(|x| texte_chemin(&x.chemin_complet()) == m.chemin)
        {
            appliquer_media(m, x);
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
        let avant = f.fichiers.len();
        f.fichiers
            .retain(|x| !(x.type_fichier == type_core(t) && x.chemin_complet() == *chemin));
        if f.fichiers.len() != avant {
            supprimes.push(chemin.clone());
        }
    }
    supprimes
}

/// Reporte l'édition complète (champs de base de `g`, détails `d`) dans le film des services.
/// Voir [`appliquer_details`] pour la valeur renvoyée.
pub fn appliquer_edition_detaillee(g: &Film, d: &DetailsFilm, f: &mut cm::Film) -> Vec<PathBuf> {
    let (acteurs, equipe) = (f.acteurs.clone(), f.equipe.clone());
    appliquer_edition(g, f);
    appliquer_details(d, f, &acteurs, &equipe)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversion::{self, ContexteConversion};
    use mediamanager_core::modele::{FluxVideo, Identifiants};
    use std::collections::BTreeMap;

    fn personne(nom: &str, role: &str, t: TypePersonne, tmdb: i64) -> Personne {
        let mut ids = Identifiants::default();
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

    fn film_riche(dossier: &Path) -> cm::Film {
        let mut f = cm::Film {
            id: "f1".into(),
            titre: "L'Odyssée".into(),
            titre_original: "The Odyssey".into(),
            titre_anglais: "The Odyssey".into(),
            synopsis: "Défiez les dieux.".into(),
            slogan: "Défiez les dieux.".into(),
            resume: "Ulysse...".into(),
            annee: 2026,
            date_sortie: Some("2026-07-08".into()),
            duree_minutes: 172,
            chemin: dossier.to_path_buf(),
            source_donnees: dossier.parent().unwrap().to_path_buf(),
            date_ajout: 1_794_000_060_000,
            derniere_lecture: Some(1_794_100_020_000),
            vu: true,
            langues_parlees: "en".into(),
            pays: "GB / US".into(),
            societe_production: "Universal Pictures / Syncopy".into(),
            note_utilisateur: "à revoir".into(),
            top250: 12,
            certification: "FR:-12".into(),
            source_media: SourceMedia::UhdBluray,
            video_3d: true,
            edition: Edition::Imax,
            genres: vec!["Aventure".into(), "Action".into()],
            tags: vec!["ship".into(), "trojan war".into()],
            liens_series: vec!["Albator 84".into()],
            ..cm::Film::default()
        };
        f.ids.definir("tmdb", "1368337");
        f.ids.definir("imdb", "tt33764258");
        f.ids.definir("twitter", "odysseymovie");
        f.notes = BTreeMap::from([
            ("tmdb".to_string(), Note::nouvelle("tmdb", 8.0, 4801, 10)),
            ("user".to_string(), Note::nouvelle("user", 7.5, 0, 10)),
        ]);
        f.acteurs = vec![
            personne("Matt Damon", "Odysseus", TypePersonne::Acteur, 1),
            personne("Tom Holland", "Telemachus", TypePersonne::Acteur, 2),
        ];
        f.equipe = vec![
            personne("Christopher Nolan", "Writer", TypePersonne::Scenariste, 3),
            personne(
                "Christopher Nolan",
                "Director",
                TypePersonne::Realisateur,
                3,
            ),
            personne("Mike Fay", "Additional Editor", TypePersonne::Monteur, 4),
        ];
        f.urls_artwork = BTreeMap::from([
            (TypeCore::Poster, "https://x/p.jpg".to_string()),
            (TypeCore::Clearlogo, "https://x/l.png".to_string()),
        ]);
        f.bandes_annonces = vec![BandeAnnonce {
            id: "b1".into(),
            nom: "Bande-annonce".into(),
            url: "https://y/1".into(),
            qualite: "1080p".into(),
            langue: "fra".into(),
            fournisseur: "YouTube".into(),
            dans_nfo: true,
            date: "2026-01-01".into(),
            scrape_par: "tmdb".into(),
        }];
        let mut video = FichierMedia {
            type_fichier: TypeCore::Video,
            chemin: dossier.to_path_buf(),
            nom: "film.mkv".into(),
            taille: 9_000_000_000,
            format_conteneur: "matroska".into(),
            duree_secondes: 10_327,
            video: FluxVideo {
                codec: "av1".into(),
                largeur: 3840,
                hauteur: 2160,
                images_par_seconde: 23.976,
                debit: 6986,
                profondeur_bits: 10,
                format_hdr: "Dolby Vision, HDR10+".into(),
                ratio_aspect: 1.78,
                ..FluxVideo::default()
            },
            ..FichierMedia::default()
        };
        video.flux_audio = vec![FluxAudio {
            codec: "eac3".into(),
            id_codec: "A_EAC3".into(),
            langue: "fra".into(),
            titre: "FR VFF : DDP 5.1".into(),
            drapeaux: vec![DrapeauFlux::Defaut],
            canaux: 6,
            debit: 640,
            configuration_canaux: "3/2/0.1".into(),
            ..FluxAudio::default()
        }];
        video.sous_titres = vec![
            FluxSousTitres {
                codec: "UTF8".into(),
                langue: "fra".into(),
                titre: "FR VFF Forcé : SRT".into(),
                drapeaux: vec![DrapeauFlux::Defaut, DrapeauFlux::Force],
            },
            FluxSousTitres {
                codec: "UTF8".into(),
                langue: "fra".into(),
                titre: "FR VFF Complet : SRT".into(),
                drapeaux: vec![],
            },
        ];
        f.fichiers = vec![video];
        f
    }

    fn contexte() -> ContexteConversion {
        ContexteConversion::default()
    }

    #[test]
    fn aller_retour_sans_modification() {
        let d0 = tempfile::tempdir().unwrap();
        let original = film_riche(&d0.path().join("L'Odyssée (2026)"));
        let g = conversion::film(&original, 1, &contexte());
        let det = details(&original);
        let mut relu = original.clone();
        let supprimes = appliquer_edition_detaillee(&g, &det, &mut relu);
        assert!(supprimes.is_empty());
        assert_eq!(relu, original);
    }

    #[test]
    fn lecture_des_details() {
        let d0 = tempfile::tempdir().unwrap();
        let f = film_riche(&d0.path().join("L'Odyssée (2026)"));
        let d = details(&f);
        assert_eq!(d.date_ajout, dates::ms_vers_fr_heure(1_794_000_060_000));
        assert_eq!(d.evaluation_utilisateur, 7.5);
        assert_eq!(d.notes.len(), 1);
        assert_eq!(d.notes[0].source, "tmdb");
        assert_eq!(d.source_media, "UHD_BLURAY");
        assert_eq!(d.edition, "IMAX");
        assert_eq!(d.ids[0], ("tmdb".to_string(), "1368337".to_string()));
        assert_eq!(d.ids[1].0, "imdb");
        assert_eq!(d.equipe.len(), 3);
        assert_eq!(d.urls[TypeArt::Affiche.rang()], "https://x/p.jpg");
        assert_eq!(d.urls[TypeArt::Clearlogo.rang()], "https://x/l.png");
        assert!(d.urls[TypeArt::Banniere.rang()].is_empty());
        assert_eq!(d.medias.len(), 1);
        assert_eq!(d.medias[0].audio.len(), 1);
        assert!(d.medias[0].audio[0].defaut && !d.medias[0].audio[0].force);
        assert!(d.medias[0].sous_titres[0].force && !d.medias[0].sous_titres[1].force);
    }

    #[test]
    fn modifications_reportees_dans_le_film() {
        let d0 = tempfile::tempdir().unwrap();
        let original = film_riche(&d0.path().join("L'Odyssée (2026)"));
        let mut g = conversion::film(&original, 1, &contexte());
        let mut det = details(&original);
        g.titre = "Odyssée".into();
        det.titre_anglais = "Odyssey".into();
        det.contour = "Court".into();
        det.note_libre = "".into();
        det.top250 = 99;
        det.langues_parlees = "en, fr".into();
        det.notes.push(NoteSource {
            source: "imdb".into(),
            valeur: 7.0,
            maximum: 10,
            votes: 10,
        });
        det.notes[0].votes = 5000;
        det.evaluation_utilisateur = 9.0;
        det.date_ajout = "15/11/2026 08:30".into();
        det.date_vu = String::new();
        det.source_media = "BLURAY".into();
        det.video_3d = false;
        det.edition = "Edition maison".into();
        det.liens_series = vec!["Autre série".into()];
        det.ids.retain(|(k, _)| k != "twitter");
        det.ids.push(("wikidata".into(), "Q131547207".into()));
        det.mots_cles = vec!["mythe".into()];
        // Retrait du premier acteur, ajout d'un nouveau, inversion de l'équipe.
        det.acteurs.remove(0);
        det.acteurs.push(PersonneFilm {
            nom: "Nouvel Acteur".into(),
            role: "Figurant".into(),
            fonction: FonctionPersonne::Acteur,
            ..PersonneFilm::default()
        });
        det.equipe.reverse();
        det.equipe.push(PersonneFilm {
            nom: "Hoyte van Hoytema".into(),
            role: "Director of Photography".into(),
            ..PersonneFilm::default()
        });
        det.urls[TypeArt::Banniere.rang()] = "https://x/b.jpg".into();
        det.urls[TypeArt::Clearlogo.rang()].clear();
        det.bandes_annonces[0].nom = "Renommée".into();
        det.bandes_annonces[0].dans_nfo = false;
        det.bandes_annonces.push(BandeAnnonceEdit {
            nom: "Autre".into(),
            url: "https://y/2".into(),
            ..BandeAnnonceEdit::default()
        });
        {
            let m = &mut det.medias[0];
            m.codec = "hevc".into();
            m.largeur = 1920;
            m.hauteur = 800;
            m.ratio = 2.40;
            m.ratio2 = Some(1.78);
            m.format_3d = "3D SBS".into();
            m.audio[0].canaux = 8;
            m.audio[0].force = true;
            m.audio.push(AudioEdit {
                codec: "ac3".into(),
                langue: "eng".into(),
                ..AudioEdit::default()
            });
            m.sous_titres.remove(0);
            m.sous_titres[0].defaut = true;
            m.sous_titres[0].malentendant = true;
        }
        let mut f = original.clone();
        appliquer_edition_detaillee(&g, &det, &mut f);

        assert_eq!(f.titre, "Odyssée");
        assert_eq!(f.titre_anglais, "Odyssey");
        assert_eq!(f.synopsis, "Court");
        assert_eq!(f.note_utilisateur, "");
        assert_eq!(f.top250, 99);
        assert_eq!(f.langues_parlees, "en, fr");
        assert_eq!(f.notes["tmdb"].votes, 5000);
        assert_eq!(f.notes["imdb"].valeur, 7.0);
        assert_eq!(f.notes["user"].valeur, 9.0);
        assert_eq!(
            f.date_ajout,
            dates::fr_heure_vers_ms("15/11/2026 08:30")
                .unwrap()
                .unwrap()
        );
        assert_eq!(f.derniere_lecture, None);
        assert_eq!(f.source_media, SourceMedia::Bluray);
        assert!(!f.video_3d);
        assert_eq!(f.edition, Edition::Personnalisee("Edition maison".into()));
        assert_eq!(f.liens_series, vec!["Autre série"]);
        assert!(!f.ids.contient("twitter"));
        assert_eq!(f.ids.obtenir("wikidata").unwrap().en_texte(), "Q131547207");
        assert_eq!(f.ids.tmdb(), 1_368_337);
        assert_eq!(f.tags, vec!["mythe"]);
        // Acteurs : « Tom Holland » conserve son identifiant, le nouveau n'en a pas.
        assert_eq!(f.acteurs.len(), 2);
        assert_eq!(f.acteurs[0].nom, "Tom Holland");
        assert_eq!(f.acteurs[0].ids.tmdb(), 2);
        assert_eq!(f.acteurs[1].ids.tmdb(), 0);
        // Équipe : ordre de l'éditeur, identifiants et types conservés, type déduit du rôle.
        let noms: Vec<(&str, TypePersonne)> = f
            .equipe
            .iter()
            .map(|p| (p.role.as_str(), p.type_personne))
            .collect();
        assert_eq!(
            noms,
            vec![
                ("Additional Editor", TypePersonne::Monteur),
                ("Director", TypePersonne::Realisateur),
                ("Writer", TypePersonne::Scenariste),
                ("Director of Photography", TypePersonne::Camera),
            ]
        );
        assert_eq!(f.equipe[1].ids.tmdb(), 3);
        assert_eq!(f.urls_artwork[&TypeCore::Banner], "https://x/b.jpg");
        assert!(!f.urls_artwork.contains_key(&TypeCore::Clearlogo));
        assert_eq!(f.urls_artwork[&TypeCore::Poster], "https://x/p.jpg");
        // Bandes-annonces : la ligne existante garde sa date, la nouvelle est ajoutée.
        assert_eq!(f.bandes_annonces.len(), 2);
        assert_eq!(f.bandes_annonces[0].nom, "Renommée");
        assert!(!f.bandes_annonces[0].dans_nfo);
        assert_eq!(f.bandes_annonces[0].date, "2026-01-01");
        assert_eq!(f.bandes_annonces[1].url, "https://y/2");
        // Fichier média.
        let v = &f.fichiers[0];
        assert_eq!(v.video.codec, "hevc");
        assert_eq!((v.video.largeur, v.video.hauteur), (1920, 800));
        assert_eq!(
            (v.video.ratio_aspect, v.video.ratio_aspect2),
            (2.40, Some(1.78))
        );
        assert_eq!(v.video.format_3d, "3D SBS");
        assert_eq!(v.flux_audio.len(), 2);
        assert_eq!(v.flux_audio[0].canaux, 8);
        assert_eq!(v.flux_audio[0].id_codec, "A_EAC3");
        assert!(v.flux_audio[0].drapeaux.contains(&DrapeauFlux::Force));
        assert_eq!(v.flux_audio[1].codec, "ac3");
        assert_eq!(v.sous_titres.len(), 1);
        assert_eq!(v.sous_titres[0].titre, "FR VFF Complet : SRT");
        assert!(
            v.sous_titres[0]
                .drapeaux
                .contains(&DrapeauFlux::Malentendants)
        );
        // Le film édité se relit à l'identique.
        assert_eq!(details(&f).evaluation_utilisateur, 9.0);
    }

    #[test]
    fn suppression_d_une_illustration() {
        let d0 = tempfile::tempdir().unwrap();
        let dossier = d0.path().join("Film (2000)");
        std::fs::create_dir_all(&dossier).unwrap();
        std::fs::write(dossier.join("film-clearlogo.png"), b"x").unwrap();
        let mut original = film_riche(&dossier);
        original.fichiers.push(FichierMedia {
            type_fichier: TypeCore::Clearlogo,
            chemin: dossier.clone(),
            nom: "film-clearlogo.png".into(),
            ..FichierMedia::default()
        });
        let g = conversion::film(&original, 1, &contexte());
        let mut det = details(&original);
        let rang = TypeArt::Clearlogo.rang();
        assert_eq!(
            det.images.chemins[rang].as_deref(),
            Some(dossier.join("film-clearlogo.png").as_path())
        );
        assert!(det.images.chemin(TypeArt::Clearlogo).is_some());
        det.images.supprimes[rang] = true;
        assert!(det.images.chemin(TypeArt::Clearlogo).is_none());
        let mut f = original.clone();
        let supprimes = appliquer_edition_detaillee(&g, &det, &mut f);
        assert_eq!(supprimes, vec![dossier.join("film-clearlogo.png")]);
        assert!(f.fichiers_de_type(TypeCore::Clearlogo).next().is_none());
    }

    #[test]
    fn codes_de_source_media() {
        assert_eq!(
            source_media_depuis_code("UHD_BLURAY"),
            SourceMedia::UhdBluray
        );
        assert_eq!(source_media_depuis_code(""), SourceMedia::Inconnue);
        assert_eq!(source_media_depuis_code("???"), SourceMedia::Inconnue);
        for (code, _) in crate::source_donnees::SOURCES_MEDIA {
            assert_eq!(code_source_media(source_media_depuis_code(code)), code);
        }
    }
}
