//! Renommage des films : aperçu (plan du cœur), exécution, mise à jour des
//! chemins en base et réécriture des NFO.

use std::collections::BTreeMap;

use mediamanager_core::modele::{Film, Saga, TypeFichier};
use mediamanager_core::renommage::{
    NatureElement, OptionsExecution, PlanRenommage, ProfilRenommageFilm, SondeSystemeFichiers,
    StatutElement, appliquer_au_film, executer, planifier_films,
};
use mediamanager_store::Reglages;

use crate::application::Noyau;
use crate::artwork::ConfigArtwork;
use crate::ecriture_nfo::ecrire_nfo;
use crate::erreur::Resultat;
use crate::evenements::{Evenement, NiveauMessage};
use crate::taches::ContexteTache;

/// Bilan d'un renommage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RapportRenommage {
    /// Films déplacés ou renommés.
    pub renommes: Vec<String>,
    /// Films déjà correctement nommés.
    pub inchanges: Vec<String>,
    /// Films non traités ou en échec : (identifiant, raison).
    pub echecs: Vec<(String, String)>,
    /// Films verrouillés exclus de la sélection.
    pub verrouilles: Vec<String>,
}

/// Aperçu d'un renommage : plan (aucune écriture) et entités verrouillées
/// exclues de la sélection.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ApercuRenommage {
    pub plan: PlanRenommage,
    /// Identifiants des entités verrouillées, exclues du plan.
    pub verrouilles: Vec<String>,
}

/// Profil de renommage des films construit depuis les réglages.
pub fn profil_renommage_film(r: &Reglages, artwork: &ConfigArtwork) -> ProfilRenommageFilm {
    let rr = &r.renommeur;
    let defaut = ProfilRenommageFilm::default();
    ProfilRenommageFilm {
        dossier_actif: !rr.gabarit_dossier_film.trim().is_empty(),
        gabarit_dossier: rr.gabarit_dossier_film.clone(),
        fichier_actif: !rr.gabarit_fichier_film.trim().is_empty(),
        gabarit_fichier: rr.gabarit_fichier_film.clone(),
        remplacer_espaces_dossier: rr.remplacer_espaces,
        remplacement_espaces_dossier: rr.caractere_espace.clone(),
        remplacer_espaces_fichier: rr.remplacer_espaces,
        remplacement_espaces_fichier: rr.caractere_espace.clone(),
        remplacement_deux_points: if rr.caractere_remplacement.is_empty() {
            defaut.remplacement_deux_points.clone()
        } else {
            rr.caractere_remplacement.clone()
        },
        ascii: rr.ascii_seulement,
        prefixes_titre: r.general.prefixes_titre.clone(),
        noms: artwork.noms.clone(),
        ..defaut
    }
}

fn charger(n: &Noyau, ids: &[String]) -> Resultat<(Vec<Film>, Vec<String>, Vec<Saga>)> {
    let s = n.verrou();
    let mut films = Vec::new();
    let mut verrouilles = Vec::new();
    for id in ids {
        if let Some(f) = s.bibliotheque.obtenir::<Film>(id)? {
            if f.verrouille {
                verrouilles.push(f.id);
            } else {
                films.push(f);
            }
        }
    }
    let sagas = s.bibliotheque.lister::<Saga>()?;
    Ok((films, verrouilles, sagas))
}

/// Aperçu : plan de renommage d'une sélection (aucune écriture).
pub(crate) fn apercu_films(n: &Noyau, ids: &[String]) -> Resultat<PlanRenommage> {
    Ok(apercu_films_detaille(n, ids)?.plan)
}

/// Aperçu détaillé : plan et films verrouillés exclus.
pub(crate) fn apercu_films_detaille(n: &Noyau, ids: &[String]) -> Resultat<ApercuRenommage> {
    let (films, verrouilles, sagas) = charger(n, ids)?;
    let profil = profil_renommage_film(&n.reglages(), &n.config.artwork);
    let refs: Vec<&Film> = films.iter().collect();
    Ok(ApercuRenommage {
        plan: planifier_films(&refs, &sagas, &profil, &SondeSystemeFichiers),
        verrouilles,
    })
}

/// Tâche (ou étape) : renommage d'une sélection.
pub(crate) fn renommer_films(
    n: &Noyau,
    ctx: &ContexteTache,
    ids: &[String],
) -> Resultat<RapportRenommage> {
    ctx.verifier_annulation()?;
    let reglages = n.reglages();
    let (films, verrouilles, sagas) = charger(n, ids)?;
    let profil = profil_renommage_film(&reglages, &n.config.artwork);
    let refs: Vec<&Film> = films.iter().collect();
    let plan = planifier_films(&refs, &sagas, &profil, &SondeSystemeFichiers);
    for (i, c) in plan.conflits() {
        ctx.message(
            NiveauMessage::Avertissement,
            format!("{} : {c}", plan.elements[i].libelle),
        );
    }
    let total = plan.elements.len() as u64;
    ctx.progression(0, total, "Renommage");
    let options = OptionsExecution {
        corbeille: reglages.general.corbeille_activee,
        ..OptionsExecution::default()
    };
    let resultat = executer(&plan, &options);

    let mut par_id: BTreeMap<String, Film> = films.into_iter().map(|f| (f.id.clone(), f)).collect();
    let mut rapport = RapportRenommage {
        verrouilles,
        ..RapportRenommage::default()
    };
    for (k, r) in resultat.resultats.iter().enumerate() {
        let el = &plan.elements[r.indice];
        let NatureElement::Film { id } = &el.nature else {
            continue;
        };
        ctx.progression(k as u64 + 1, total, el.libelle.clone());
        match &r.statut {
            StatutElement::Reussi { .. } => {
                let Some(mut film) = par_id.remove(id) else {
                    continue;
                };
                let etait_partage = film.dossier_multi_films;
                appliquer_au_film(&mut film, el);
                // Dossier changé sans déplacement en bloc : `.actors` suit.
                if !etait_partage && !film.dossier_multi_films {
                    crate::acteurs::emporter_dossier_acteurs(
                        &el.ancien_dossier,
                        &el.nouveau_dossier,
                    );
                }
                if film.fichiers_de_type(TypeFichier::Nfo).next().is_some() {
                    let saga = film
                        .saga_id
                        .as_ref()
                        .and_then(|s| sagas.iter().find(|x| &x.id == s));
                    if let Err(e) = ecrire_nfo(
                        &mut film,
                        saga,
                        &reglages,
                        &n.config.nommage_nfo_film,
                        n.config.horodatage_nfo,
                    ) {
                        ctx.message(NiveauMessage::Avertissement, e.to_string());
                    }
                }
                n.verrou().bibliotheque.enregistrer(&mut film)?;
                ctx.emettre(Evenement::FilmModifie { id: id.clone() });
                rapport.renommes.push(id.clone());
            }
            StatutElement::Inchange => rapport.inchanges.push(id.clone()),
            StatutElement::Ignore { raison } => {
                rapport.echecs.push((id.clone(), raison.clone()));
            }
            StatutElement::Echoue {
                erreur,
                retour_complet,
                ..
            } => {
                let msg = if *retour_complet {
                    format!("{erreur} (état initial restauré)")
                } else {
                    format!("{erreur} (retour arrière incomplet)")
                };
                ctx.message(NiveauMessage::Erreur, format!("{} : {msg}", el.libelle));
                rapport.echecs.push((id.clone(), msg));
            }
        }
    }
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}
