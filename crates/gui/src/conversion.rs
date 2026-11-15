//! Conversions entre le modèle des services (`mediamanager_core`, `mediamanager_store`) et les
//! structures d'affichage de l'interface. Fonctions pures, testées sans interface.

use crate::modele::{BandeAnnonceFilm, FichierFilm, Film, TypeFichier};
use crate::renommage::{ApercuFilm, LigneFichier};
use crate::source_donnees::{
    ElementManquant, EpisodeAffichage, GenreImage, ReglagesServices, SagaAffichage,
    SaisonAffichage, SerieAffichage, StatutSerie,
};
use mediamanager_core::flux::normalisation::format_video;
use mediamanager_core::modele::commun::cles_id;
use mediamanager_core::modele::{
    self as cm, Episode, FichierMedia, Note, Personne, Serie, StatutDiffusion,
    TypeFichier as TypeCore, TypePersonne,
};
use mediamanager_core::renommage::{ElementPlan, NatureElement, PlanRenommage, titre_triable};
use mediamanager_store::Reglages;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Données annexes nécessaires à la conversion d'un film.
#[derive(Clone, Debug, Default)]
pub struct ContexteConversion {
    /// Identifiants des films marqués « nouveau ».
    pub nouveaux: HashSet<String>,
    /// Titre des sagas par identifiant.
    pub sagas: HashMap<String, String>,
    /// Articles déplacés pour le titre triable.
    pub prefixes: Vec<String>,
}

/// Date `aaaa-mm-jj hh:mm` (UTC) d'un horodatage en millisecondes.
pub fn date_heure(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let secondes = ms.div_euclid(1000);
    let jours = secondes.div_euclid(86_400);
    let dans_jour = secondes.rem_euclid(86_400);
    // Algorithme « civil from days » (calendrier grégorien proleptique).
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let doe = z - ere * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let jour = doy - (153 * mp + 2) / 5 + 1;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 };
    let annee = yoe + ere * 400 + i64::from(mois <= 2);
    format!(
        "{annee:04}-{mois:02}-{jour:02} {:02}:{:02}",
        dans_jour / 3600,
        dans_jour % 3600 / 60
    )
}

/// Type de fichier affiché.
pub fn type_fichier(t: TypeCore) -> TypeFichier {
    match t {
        TypeCore::Video => TypeFichier::Video,
        TypeCore::Nfo => TypeFichier::Nfo,
        TypeCore::SousTitres => TypeFichier::SousTitres,
        TypeCore::Poster => TypeFichier::Affiche,
        TypeCore::Fanart => TypeFichier::Fanart,
        TypeCore::Banner => TypeFichier::Banniere,
        TypeCore::Thumb => TypeFichier::Miniature,
        TypeCore::Trailer => TypeFichier::BandeAnnonce,
        _ => TypeFichier::Autre,
    }
}

/// Libellé d'un type de fichier du modèle (aperçu du renommage).
pub fn libelle_type(t: TypeCore) -> String {
    match type_fichier(t) {
        TypeFichier::Autre => format!("{t:?}").to_uppercase(),
        connu => connu.libelle().to_string(),
    }
}

fn type_image(genre: GenreImage) -> TypeCore {
    match genre {
        GenreImage::Affiche => TypeCore::Poster,
        GenreImage::Fanart => TypeCore::Fanart,
        GenreImage::Banniere => TypeCore::Banner,
        GenreImage::Miniature => TypeCore::Thumb,
    }
}

/// Fichiers d'illustration existants sur le disque, dans l'ordre de [`GenreImage::TOUS`].
pub fn chemins_images(fichiers: &[FichierMedia]) -> [Option<PathBuf>; 4] {
    GenreImage::TOUS.map(|g| {
        fichiers
            .iter()
            .filter(|f| f.type_fichier == type_image(g))
            .map(FichierMedia::chemin_complet)
            .find(|p| p.is_file())
    })
}

fn liste(texte: &str, separateur: char) -> Vec<String> {
    texte
        .split(separateur)
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

fn texte_chemin(p: &Path) -> String {
    p.display().to_string()
}

fn suffixe(nom: &str, base_video: &str) -> Option<String> {
    (!base_video.is_empty() && nom.starts_with(base_video) && nom.len() > base_video.len())
        .then(|| nom[base_video.len()..].to_string())
}

/// Film du modèle → film affiché.
pub fn film(f: &cm::Film, id: u64, ctx: &ContexteConversion) -> Film {
    let principal = f.fichier_principal();
    let base_video = principal.map(FichierMedia::nom_de_base).unwrap_or_default();
    let videos: Vec<&FichierMedia> = f.fichiers_de_type(TypeCore::Video).collect();
    let note = f.note_principale();
    let titre_tri = if f.titre_tri.trim().is_empty() {
        titre_triable(&f.titre, &ctx.prefixes)
    } else {
        f.titre_tri.clone()
    };
    let dossier = match f.chemin.strip_prefix(&f.source_donnees) {
        Ok(r) if !f.source_donnees.as_os_str().is_empty() => texte_chemin(r),
        _ => texte_chemin(&f.chemin),
    };
    let audio = principal
        .and_then(|v| v.flux_audio.first())
        .map(|a| {
            if a.canaux > 0 {
                format!("{} {}ch", a.codec.to_uppercase(), a.canaux)
            } else {
                a.codec.to_uppercase()
            }
        })
        .unwrap_or_default();
    let duree = if f.duree_minutes > 0 {
        f.duree_minutes
    } else {
        videos.iter().map(|v| v.duree_secondes).sum::<u32>() / 60
    };
    let a_type = |t: TypeCore| f.fichiers_de_type(t).next().is_some();
    Film {
        id,
        titre: f.titre.clone(),
        titre_original: f.titre_original.clone(),
        titre_triable: titre_tri,
        annee: f.annee,
        date_sortie: f.date_sortie.clone().unwrap_or_default(),
        saga: f.saga_id.as_ref().and_then(|s| ctx.sagas.get(s).cloned()),
        note: note
            .map(|n| n.sur_dix())
            .filter(|n| *n > 0.0)
            .unwrap_or(0.0),
        votes: note.map(|n| n.votes).unwrap_or(0),
        notes_sources: f
            .notes
            .values()
            .filter(|n| n.est_valide())
            .map(|n| (n.id.clone(), n.sur_dix()))
            .collect(),
        certification: f.certification.clone(),
        duree_min: duree,
        genres: f.genres.clone(),
        realisateurs: f.realisateurs().iter().map(|p| p.nom.clone()).collect(),
        scenaristes: f.scenaristes().iter().map(|p| p.nom.clone()).collect(),
        pays: liste(&f.pays, '/'),
        production: liste(&f.societe_production, '/'),
        slogan: f.slogan.clone(),
        resume: f.resume.clone(),
        imdb: f.ids.imdb(),
        tmdb: match f.ids.tmdb() {
            n if n > 0 => n.to_string(),
            _ => String::new(),
        },
        source: texte_chemin(&f.source_donnees),
        dossier,
        nom_fichier: principal.map(|v| v.nom.clone()).unwrap_or_default(),
        hauteur_video: principal.map(|v| v.video.hauteur).unwrap_or(0),
        codec_video: principal
            .map(|v| v.video.codec.to_uppercase())
            .unwrap_or_default(),
        hdr: principal
            .map(|v| v.video.format_hdr.clone())
            .filter(|h| !h.is_empty()),
        audio,
        taille_octets: videos.iter().map(|v| v.taille).sum(),
        date_ajout: date_heure(f.date_ajout),
        nouveau: ctx.nouveaux.contains(&f.id),
        verrouille: f.verrouille,
        metadonnees_ok: !f.ids.est_vide() || !f.resume.trim().is_empty(),
        images_ok: a_type(TypeCore::Poster),
        bande_annonce: a_type(TypeCore::Trailer) || !f.bandes_annonces.is_empty(),
        sous_titres: a_type(TypeCore::SousTitres)
            || videos.iter().any(|v| !v.sous_titres.is_empty()),
        vu: f.vu,
        acteurs: f
            .acteurs
            .iter()
            .map(|p| (p.nom.clone(), p.role.clone()))
            .collect(),
        fichiers: f
            .fichiers
            .iter()
            .map(|x| fichier_affiche(x, &base_video))
            .collect(),
        bandes_annonces: f
            .bandes_annonces
            .iter()
            .map(|b| BandeAnnonceFilm {
                nom: b.nom.clone(),
                source: b.fournisseur.clone(),
                qualite: b.qualite.clone(),
                url: b.url.clone(),
            })
            .collect(),
    }
}

/// Fichier du modèle → ligne de l'onglet « Fichiers médias » (colonnes de flux vides si
/// l'analyse n'a pas eu lieu).
pub fn fichier_affiche(x: &FichierMedia, base_video: &str) -> FichierFilm {
    let v = &x.video;
    let audio = x
        .flux_audio
        .iter()
        .map(|a| {
            let mut t = a.codec.to_uppercase();
            if a.canaux > 0 {
                t.push_str(&format!(" {}ch", a.canaux));
            }
            if !a.langue.is_empty() {
                t.push(' ');
                t.push_str(&a.langue);
            }
            t
        })
        .collect::<Vec<_>>()
        .join(", ");
    let sous_titres = x
        .sous_titres
        .iter()
        .map(|s| {
            if s.langue.is_empty() {
                s.codec.clone()
            } else {
                s.langue.clone()
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    FichierFilm {
        type_fichier: type_fichier(x.type_fichier),
        suffixe: suffixe(&x.nom, base_video),
        nom: x.nom.clone(),
        taille_octets: x.taille,
        chemin: texte_chemin(&x.chemin_complet()),
        codec: v.codec.to_uppercase(),
        resolution: if v.largeur > 0 && v.hauteur > 0 {
            format!("{}x{}", v.largeur, v.hauteur)
        } else {
            String::new()
        },
        audio,
        sous_titres,
    }
}

/// Saga du modèle → saga affichée (`id_de` : identifiant affiché d'un film, `None` si le film
/// est inconnu de la bibliothèque).
pub fn saga(s: &cm::Saga, id_de: impl Fn(&str) -> Option<u64>) -> SagaAffichage {
    let images = chemins_images(&s.fichiers);
    SagaAffichage {
        id: s.id.clone(),
        titre: s.titre.clone(),
        titre_original: s.titre_original.clone(),
        resume: s.resume.clone(),
        films: s.films.iter().filter_map(|id| id_de(id)).collect(),
        affiche: images[GenreImage::Affiche.rang()].clone(),
        fanart: images[GenreImage::Fanart.rang()].clone(),
        banniere: images[GenreImage::Banniere.rang()].clone(),
    }
}

/// Remplace les personnes d'un type par la liste de noms, en conservant les fiches existantes.
fn fusionner_personnes(
    existantes: &[Personne],
    noms: &[(String, String)],
    t: TypePersonne,
) -> Vec<Personne> {
    noms.iter()
        .map(|(nom, role)| {
            let mut p = existantes
                .iter()
                .find(|p| p.type_personne == t && p.nom == *nom)
                .cloned()
                .unwrap_or_else(|| Personne {
                    type_personne: t,
                    nom: nom.clone(),
                    ..Personne::default()
                });
            p.role = role.clone();
            p
        })
        .collect()
}

/// Reporte les champs éditables d'un film affiché dans le film du modèle.
pub fn appliquer_edition(g: &Film, f: &mut cm::Film) {
    f.titre = g.titre.clone();
    f.titre_original = g.titre_original.clone();
    f.titre_tri = if g.titre_triable == g.titre {
        String::new()
    } else {
        g.titre_triable.clone()
    };
    f.annee = g.annee;
    f.date_sortie = Some(g.date_sortie.clone()).filter(|d| !d.is_empty());
    f.duree_minutes = g.duree_min;
    f.certification = g.certification.clone();
    f.vu = g.vu;
    f.verrouille = g.verrouille;
    f.slogan = g.slogan.clone();
    f.resume = g.resume.clone();
    f.genres = g.genres.clone();
    f.pays = g.pays.join(" / ");
    f.societe_production = g.production.join(" / ");
    if !g.imdb.is_empty() {
        f.ids.definir(cles_id::IMDB, &g.imdb);
    }
    if !g.tmdb.is_empty() {
        f.ids.definir(cles_id::TMDB, &g.tmdb);
    }
    f.acteurs = fusionner_personnes(&f.acteurs, &g.acteurs, TypePersonne::Acteur);
    let realisateurs: Vec<(String, String)> = g
        .realisateurs
        .iter()
        .map(|n| (n.clone(), String::new()))
        .collect();
    let mut equipe = fusionner_personnes(&f.equipe, &realisateurs, TypePersonne::Realisateur);
    for (p, r) in equipe.iter_mut().zip(&realisateurs) {
        // Le rôle (fonction précise) d'un réalisateur existant est conservé.
        if r.1.is_empty()
            && let Some(x) = f
                .equipe
                .iter()
                .find(|x| x.type_personne == TypePersonne::Realisateur && x.nom == p.nom)
        {
            p.role = x.role.clone();
        }
    }
    equipe.extend(
        f.equipe
            .iter()
            .filter(|p| p.type_personne != TypePersonne::Realisateur)
            .cloned(),
    );
    f.equipe = equipe;
}

/// Statut de diffusion du modèle → statut affiché.
pub fn statut_serie(s: StatutDiffusion) -> StatutSerie {
    match s {
        StatutDiffusion::Inconnu => StatutSerie::Inconnu,
        StatutDiffusion::EnCours => StatutSerie::EnCours,
        StatutDiffusion::Terminee => StatutSerie::Terminee,
        StatutDiffusion::Annulee => StatutSerie::Annulee,
    }
}

/// Statut affiché → statut du modèle.
pub fn statut_diffusion(s: StatutSerie) -> StatutDiffusion {
    match s {
        StatutSerie::Inconnu => StatutDiffusion::Inconnu,
        StatutSerie::EnCours => StatutDiffusion::EnCours,
        StatutSerie::Terminee => StatutDiffusion::Terminee,
        StatutSerie::Annulee => StatutDiffusion::Annulee,
    }
}

/// Résumé technique d'un fichier analysé (« 1080p H264 · AC3 6ch · 44 min ») ; vide si le
/// fichier n'a pas été analysé.
pub fn resume_media(f: &FichierMedia) -> String {
    let mut parties: Vec<String> = Vec::new();
    let v = &f.video;
    let mut video = Vec::new();
    if v.hauteur > 0 {
        video.push(format_video(v.largeur, v.hauteur));
    }
    if !v.codec.is_empty() {
        video.push(v.codec.to_uppercase());
    }
    if !video.is_empty() {
        parties.push(video.join(" "));
    }
    if let Some(a) = f.flux_audio.first() {
        let mut t = a.codec.to_uppercase();
        if a.canaux > 0 {
            t.push_str(&format!(" {}ch", a.canaux));
        }
        if !t.trim().is_empty() {
            parties.push(t);
        }
    }
    if f.duree_secondes > 0 && !parties.is_empty() {
        parties.push(format!("{} min", f.duree_secondes.div_ceil(60)));
    }
    parties.join(" · ")
}

/// Note affichée d'une série : celle de l'utilisateur, sinon la première exploitable.
fn note_serie(s: &Serie) -> Option<&Note> {
    s.notes
        .get("user")
        .filter(|n| n.est_valide())
        .or_else(|| s.notes.get("tmdb").filter(|n| n.est_valide()))
        .or_else(|| s.notes.values().find(|n| n.est_valide()))
}

fn episode_affiche(e: &Episode, groupe: &cm::GroupeEpisodes) -> EpisodeAffichage {
    let video = e
        .fichiers
        .iter()
        .find(|f| f.type_fichier == TypeCore::Video);
    EpisodeAffichage {
        episode: e.episode(groupe),
        titre: e.titre.clone(),
        fichier: video.map(|f| f.nom.clone()).unwrap_or_default(),
        vu: e.vu,
        manquant: e.est_fictif(),
        infos_media: video.map(resume_media).unwrap_or_default(),
    }
}

/// Série du modèle → arbre affiché (épisodes réels, plus les épisodes manquants autorisés par
/// les réglages ; triés par saison puis numéro).
pub fn serie(s: &Serie, r: &Reglages) -> SerieAffichage {
    let mut saisons: BTreeMap<i32, SaisonAffichage> = BTreeMap::new();
    for sa in &s.saisons {
        saisons.insert(
            sa.numero,
            SaisonAffichage {
                numero: sa.numero,
                titre: sa.titre.clone(),
                episodes: Vec::new(),
            },
        );
    }
    for e in &mediamanager_app::series::episodes_affiches(s, r) {
        let n = e.saison(&s.groupe_actif);
        let saison = saisons.entry(n).or_insert_with(|| SaisonAffichage {
            numero: n,
            titre: String::new(),
            episodes: Vec::new(),
        });
        saison.episodes.push(episode_affiche(e, &s.groupe_actif));
    }
    let mut saisons: Vec<SaisonAffichage> = saisons.into_values().collect();
    for sa in &mut saisons {
        sa.episodes
            .sort_by(|a, b| a.episode.cmp(&b.episode).then(a.titre.cmp(&b.titre)));
    }
    // Une saison sans aucun épisode affiché (manquants masqués) n'apparaît pas.
    saisons.retain(|sa| !sa.episodes.is_empty());
    SerieAffichage {
        id: s.id.clone(),
        titre: s.titre.clone(),
        titre_original: s.titre_original.clone(),
        annee: s.annee,
        chemin: texte_chemin(&s.chemin),
        resume: s.resume.clone(),
        genres: s.genres.clone(),
        statut: statut_serie(s.statut),
        note: note_serie(s).map(Note::sur_dix).unwrap_or(0.0),
        imdb: s.ids.imdb(),
        tmdb: match s.ids.tmdb() {
            n if n > 0 => n.to_string(),
            _ => String::new(),
        },
        tvdb: match s.ids.tvdb() {
            n if n > 0 => n.to_string(),
            _ => String::new(),
        },
        verrouillee: s.verrouille,
        saisons,
    }
}

/// Reporte les champs éditables d'une série affichée dans la série du modèle. Une note
/// inchangée n'est pas recopiée comme note de l'utilisateur.
pub fn appliquer_edition_serie(g: &SerieAffichage, s: &mut Serie) {
    s.titre = g.titre.trim().to_string();
    s.titre_original = g.titre_original.trim().to_string();
    s.annee = g.annee;
    s.resume = g.resume.clone();
    s.genres = g
        .genres
        .iter()
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect();
    s.statut = statut_diffusion(g.statut);
    s.verrouille = g.verrouillee;
    if !g.imdb.is_empty() {
        s.ids.definir(cles_id::IMDB, &g.imdb);
    }
    if !g.tmdb.is_empty() {
        s.ids.definir(cles_id::TMDB, &g.tmdb);
    }
    if !g.tvdb.is_empty() {
        s.ids.definir(cles_id::TVDB, &g.tvdb);
    }
    let actuelle = note_serie(s).map(Note::sur_dix).unwrap_or(0.0);
    if g.note > 0.0 {
        if (g.note - actuelle).abs() > 0.05 {
            s.notes
                .insert("user".into(), Note::nouvelle("user", g.note, 0, 10));
        }
    } else {
        s.notes.remove("user");
    }
}

/// Épisode manquant du modèle → ligne de la liste des éléments manquants.
pub fn episode_manquant(e: &Episode, groupe: &cm::GroupeEpisodes) -> ElementManquant {
    ElementManquant {
        titre: e.titre.clone(),
        titre_original: e.titre_original.clone(),
        detail: format!("S{:02}E{:02}", e.saison(groupe), e.episode(groupe)),
        resume: e.resume.clone(),
    }
}

/// Partie de collection absente de la bibliothèque → ligne de la liste des éléments manquants.
pub fn partie_manquante(p: &mediamanager_scrapers::PartieCollection) -> ElementManquant {
    ElementManquant {
        titre: p.titre.clone(),
        titre_original: p.titre_original.clone(),
        detail: if p.annee > 0 {
            p.annee.to_string()
        } else {
            String::new()
        },
        resume: p.resume.clone(),
    }
}

fn relatif(p: &Path, base: &Path) -> String {
    match p.strip_prefix(base) {
        Ok(r) if !base.as_os_str().is_empty() => texte_chemin(r),
        _ => texte_chemin(p),
    }
}

/// Élément de plan → aperçu d'un film (`id` = identifiant affiché).
pub fn element_apercu(e: &ElementPlan, id: u64) -> ApercuFilm {
    let fichiers = e
        .lignes
        .iter()
        .map(|l| LigneFichier {
            type_fichier: libelle_type(l.type_fichier),
            ancien: relatif(&l.ancien, &e.ancien_dossier),
            nouveau: l
                .nouveaux
                .iter()
                .map(|n| relatif(n, &e.nouveau_dossier))
                .collect::<Vec<_>>()
                .join(", "),
            inchange: l.est_inchange(),
        })
        .collect();
    ApercuFilm {
        id,
        titre: e.libelle.clone(),
        source: texte_chemin(&e.source_donnees),
        ancien_dossier: relatif(&e.ancien_dossier, &e.source_donnees),
        nouveau_dossier: relatif(&e.nouveau_dossier, &e.source_donnees),
        fichiers,
        conflits: e.conflits.iter().map(ToString::to_string).collect(),
        avertissements: e.avertissements.clone(),
        inchange: e.est_inchange(),
    }
}

/// Plan de renommage → aperçus des films (les éléments non-films sont ignorés).
pub fn plan_apercu(plan: &PlanRenommage, id_de: impl Fn(&str) -> u64) -> Vec<ApercuFilm> {
    plan.elements
        .iter()
        .filter_map(|e| match &e.nature {
            NatureElement::Film { id } => Some(element_apercu(e, id_de(id))),
            _ => None,
        })
        .collect()
}

/// Plan de renommage de séries → aperçus (série et épisodes), numérotés dans l'ordre du plan.
pub fn plan_apercu_series(plan: &PlanRenommage) -> Vec<ApercuFilm> {
    plan.elements
        .iter()
        .enumerate()
        .map(|(n, e)| element_apercu(e, n as u64 + 1))
        .collect()
}

fn chemins(v: &[PathBuf]) -> Vec<String> {
    v.iter().map(|p| texte_chemin(p)).collect()
}

/// Réglages du stockage → réglages affichés.
pub fn services(r: &Reglages) -> ReglagesServices {
    let (g, f, rr) = (&r.general, &r.films, &r.renommeur);
    ReglagesServices {
        sources_films: chemins(&f.sources),
        sources_series: chemins(&r.series.sources),
        dossiers_ignores: f.dossiers_ignores.clone(),
        langue_scraping: f.langue_scraping.clone(),
        gabarit_dossier_film: rr.gabarit_dossier_film.clone(),
        gabarit_fichier_film: rr.gabarit_fichier_film.clone(),
        remplacer_espaces: rr.remplacer_espaces,
        caractere_espace: rr.caractere_espace.clone(),
        ascii_seulement: rr.ascii_seulement,
        caractere_remplacement: rr.caractere_remplacement.clone(),
        supprimer_dossiers_vides: rr.supprimer_dossiers_vides,
        renommer_apres_scraping: f.renommer_apres_scraping,
        proxy_hote: g.proxy_hote.clone(),
        proxy_port: if g.proxy_port == 0 {
            String::new()
        } else {
            g.proxy_port.to_string()
        },
        proxy_utilisateur: g.proxy_utilisateur.clone(),
        proxy_mot_de_passe: g.mot_de_passe_proxy.clone(),
        ignorer_erreurs_ssl: g.ignorer_erreurs_ssl,
        threads_telechargement: g.threads_telechargement,
        corbeille: g.corbeille_activee,
        cache_images: g.cache_images,
        chemin_outils: g.chemin_ffmpeg.clone(),
        gabarit_dossier_serie: rr.gabarit_dossier_serie.clone(),
        gabarit_dossier_saison: rr.gabarit_dossier_saison.clone(),
        gabarit_fichier_episode: rr.gabarit_fichier_episode.clone(),
        afficher_episodes_manquants: r.series.afficher_episodes_manquants,
        afficher_speciaux_manquants: r.series.afficher_speciaux_manquants,
        afficher_non_diffuses: r.series.afficher_non_diffuses,
    }
}

/// Reporte les réglages affichés dans les réglages du stockage.
pub fn appliquer_services(s: &ReglagesServices, r: &mut Reglages) {
    let vers_chemins = |v: &[String]| -> Vec<PathBuf> {
        v.iter()
            .map(|x| x.trim())
            .filter(|x| !x.is_empty())
            .map(PathBuf::from)
            .collect()
    };
    r.films.sources = vers_chemins(&s.sources_films);
    r.series.sources = vers_chemins(&s.sources_series);
    r.films.dossiers_ignores = s.dossiers_ignores.clone();
    if !s.langue_scraping.trim().is_empty() {
        r.films.langue_scraping = s.langue_scraping.trim().to_string();
    }
    r.films.renommer_apres_scraping = s.renommer_apres_scraping;
    let rr = &mut r.renommeur;
    rr.gabarit_dossier_film = s.gabarit_dossier_film.clone();
    rr.gabarit_fichier_film = s.gabarit_fichier_film.clone();
    rr.remplacer_espaces = s.remplacer_espaces;
    rr.caractere_espace = s.caractere_espace.clone();
    rr.ascii_seulement = s.ascii_seulement;
    rr.caractere_remplacement = s.caractere_remplacement.clone();
    rr.supprimer_dossiers_vides = s.supprimer_dossiers_vides;
    let g = &mut r.general;
    g.proxy_hote = s.proxy_hote.trim().to_string();
    g.proxy_port = s.proxy_port.trim().parse().unwrap_or(0);
    g.proxy_utilisateur = s.proxy_utilisateur.clone();
    g.mot_de_passe_proxy = s.proxy_mot_de_passe.clone();
    g.ignorer_erreurs_ssl = s.ignorer_erreurs_ssl;
    g.threads_telechargement = s.threads_telechargement.max(1);
    g.corbeille_activee = s.corbeille;
    g.cache_images = s.cache_images;
    g.chemin_ffmpeg = s.chemin_outils.trim().to_string();
    rr.gabarit_dossier_serie = s.gabarit_dossier_serie.clone();
    rr.gabarit_dossier_saison = s.gabarit_dossier_saison.clone();
    rr.gabarit_fichier_episode = s.gabarit_fichier_episode.clone();
    r.series.afficher_episodes_manquants = s.afficher_episodes_manquants;
    r.series.afficher_speciaux_manquants = s.afficher_speciaux_manquants;
    r.series.afficher_non_diffuses = s.afficher_non_diffuses;
}

#[cfg(test)]
mod tests {
    use super::*;
    use mediamanager_core::modele::{BandeAnnonce, Note, Saison};

    #[test]
    fn dates_lisibles() {
        assert_eq!(date_heure(0), "");
        assert_eq!(date_heure(86_400_000), "1970-01-02 00:00");
        // 2024-02-29 13:45 UTC
        assert_eq!(date_heure(1_709_214_300_000), "2024-02-29 13:45");
    }

    fn film_modele(racine: &Path) -> cm::Film {
        let dossier = racine.join("Alien (1979)");
        let mut f = cm::Film {
            id: "abc".into(),
            source_donnees: racine.to_path_buf(),
            chemin: dossier.clone(),
            titre: "Le Retour".into(),
            annee: 1979,
            resume: "Résumé".into(),
            pays: "États-Unis / Royaume-Uni".into(),
            date_ajout: 86_400_000,
            ..cm::Film::default()
        };
        f.ids.definir(cles_id::TMDB, "348");
        f.notes
            .insert("tmdb".into(), Note::nouvelle("tmdb", 8.1, 1200, 10));
        let mut v = FichierMedia::nouveau(&dossier.join("Alien.mkv"), TypeCore::Video);
        v.taille = 1000;
        v.video.hauteur = 1080;
        v.video.codec = "h264".into();
        f.fichiers.push(v);
        f.fichiers.push(FichierMedia::nouveau(
            &dossier.join("Alien.fr.srt"),
            TypeCore::SousTitres,
        ));
        f.fichiers.push(FichierMedia::nouveau(
            &dossier.join("poster.jpg"),
            TypeCore::Poster,
        ));
        f.bandes_annonces.push(BandeAnnonce {
            nom: "Officielle".into(),
            fournisseur: "youtube".into(),
            ..BandeAnnonce::default()
        });
        f.equipe.push(Personne {
            type_personne: TypePersonne::Realisateur,
            nom: "Ridley Scott".into(),
            url_profil: "http://exemple".into(),
            ..Personne::default()
        });
        f
    }

    #[test]
    fn conversion_d_un_film() {
        let d = tempfile::tempdir().unwrap();
        let f = film_modele(d.path());
        let ctx = ContexteConversion {
            nouveaux: HashSet::from(["abc".to_string()]),
            prefixes: vec!["Le".into()],
            ..Default::default()
        };
        let g = film(&f, 7, &ctx);
        assert_eq!(g.id, 7);
        assert_eq!(g.titre_triable, "Retour, Le");
        assert_eq!(g.dossier, "Alien (1979)");
        assert_eq!(g.nom_fichier, "Alien.mkv");
        assert_eq!(g.hauteur_video, 1080);
        assert_eq!(g.codec_video, "H264");
        assert_eq!(g.taille_octets, 1000);
        assert_eq!(g.tmdb, "348");
        assert!((g.note - 8.1).abs() < 1e-4);
        assert_eq!(g.pays, ["États-Unis", "Royaume-Uni"]);
        assert_eq!(g.realisateurs, ["Ridley Scott"]);
        assert_eq!(g.date_ajout, "1970-01-02 00:00");
        assert!(g.nouveau && g.metadonnees_ok && g.images_ok && g.sous_titres);
        assert!(g.bande_annonce);
        assert_eq!(g.bandes_annonces[0].libelle(), "Officielle (youtube)");
        let video = &g.fichiers[0];
        assert_eq!(video.codec, "H264");
        assert!(video.chemin.ends_with("Alien.mkv"));
        assert_eq!(g.chemin_video(), Some(PathBuf::from(&video.chemin)));
        // Analyse des flux absente : colonnes vides.
        assert!(g.fichiers[1].codec.is_empty() && g.fichiers[1].resolution.is_empty());
        let st = g
            .fichiers
            .iter()
            .find(|x| x.type_fichier == TypeFichier::SousTitres)
            .unwrap();
        assert_eq!(st.suffixe.as_deref(), Some(".fr.srt"));
    }

    #[test]
    fn images_existantes_seulement() {
        let d = tempfile::tempdir().unwrap();
        let f = film_modele(d.path());
        assert_eq!(chemins_images(&f.fichiers), [None, None, None, None]);
        std::fs::create_dir_all(d.path().join("Alien (1979)")).unwrap();
        std::fs::write(d.path().join("Alien (1979)/poster.jpg"), b"x").unwrap();
        let c = chemins_images(&f.fichiers);
        assert!(c[GenreImage::Affiche.rang()].as_ref().unwrap().is_file());
        assert!(c[GenreImage::Fanart.rang()].is_none());
    }

    #[test]
    fn edition_reportee_sans_perdre_les_fiches() {
        let d = tempfile::tempdir().unwrap();
        let mut f = film_modele(d.path());
        let mut g = film(&f, 1, &ContexteConversion::default());
        g.titre = "Alien".into();
        g.titre_triable = "Alien".into();
        g.vu = true;
        g.genres = vec!["Horreur".into()];
        g.realisateurs = vec!["Ridley Scott".into(), "Autre".into()];
        g.acteurs = vec![("Sigourney Weaver".into(), "Ripley".into())];
        appliquer_edition(&g, &mut f);
        assert_eq!(f.titre, "Alien");
        assert!(f.titre_tri.is_empty());
        assert!(f.vu);
        assert_eq!(f.genres, ["Horreur"]);
        assert_eq!(f.realisateurs().len(), 2);
        assert_eq!(f.realisateurs()[0].url_profil, "http://exemple");
        assert_eq!(f.acteurs[0].role, "Ripley");
        assert_eq!(f.pays, "États-Unis / Royaume-Uni");
    }

    #[test]
    fn arbre_d_une_serie() {
        let mut s = Serie {
            id: "s1".into(),
            titre: "Série".into(),
            ..Serie::default()
        };
        s.saisons.push(Saison {
            numero: 1,
            titre: "Première".into(),
            ..Saison::default()
        });
        for (sa, ep) in [(2, 1), (1, 2), (1, 1)] {
            let mut e = cm::Episode {
                titre: format!("S{sa}E{ep}"),
                ..cm::Episode::default()
            };
            e.numeros.push(cm::NumeroEpisode {
                groupe: s.groupe_actif.clone(),
                saison: sa,
                episode: ep,
            });
            e.fichiers.push(cm::FichierMedia {
                nom: format!("s{sa}e{ep}.mkv"),
                ..cm::FichierMedia::default()
            });
            s.episodes.push(e);
        }
        let a = serie(&s, &Reglages::default());
        assert_eq!(a.saisons.len(), 2);
        assert_eq!(a.saisons[0].titre, "Première");
        assert_eq!(a.saisons[0].episodes[0].titre, "S1E1");
        assert_eq!(a.saisons[1].numero, 2);
        assert_eq!(a.nombre_episodes(), 3);
    }

    #[test]
    fn conversion_d_une_saga() {
        let s = cm::Saga {
            id: "s".into(),
            titre: "Alien".into(),
            films: vec!["a".into(), "inconnu".into(), "b".into()],
            ..cm::Saga::default()
        };
        let a = saga(&s, |id| match id {
            "a" => Some(1),
            "b" => Some(2),
            _ => None,
        });
        assert_eq!(a.films, [1, 2]);
        assert_eq!(a.titre, "Alien");
        assert!(a.affiche.is_none());
    }

    #[test]
    fn reglages_aller_retour() {
        let mut r = Reglages::default();
        r.films.sources = vec![PathBuf::from("/films")];
        r.general.proxy_port = 8080;
        let mut s = services(&r);
        assert_eq!(s.sources_films, ["/films"]);
        assert_eq!(s.proxy_port, "8080");
        s.sources_series.push("/series".into());
        s.gabarit_dossier_film = "${title}".into();
        s.proxy_port = "pas un port".into();
        appliquer_services(&s, &mut r);
        assert_eq!(r.series.sources, [PathBuf::from("/series")]);
        assert_eq!(r.renommeur.gabarit_dossier_film, "${title}");
        assert_eq!(r.general.proxy_port, 0);
        assert_eq!(services(&r).sources_series, ["/series"]);
    }
}
