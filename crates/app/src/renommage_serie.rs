//! Renommage des séries et de leurs épisodes : aperçu (plan du cœur),
//! exécution, mise à jour des chemins en base et réécriture des NFO.

use mediamanager_core::modele::{Serie, TypeFichier};
use mediamanager_core::renommage::{
    Conflit, OptionsExecution, PlanRenommage, ProfilRenommageSerie, SondeSystemeFichiers,
    StatutElement, appliquer_aux_episodes, elements_reussis, executer, planifier_serie,
};
use mediamanager_store::Reglages;

use crate::application::Noyau;
use crate::erreur::Resultat;
use crate::evenements::{Evenement, NiveauMessage};
use crate::renommage::{ApercuRenommage, RapportRenommage};
use crate::series::ecrire_nfo_serie_complete;
use crate::taches::ContexteTache;

/// Profil de renommage des séries construit depuis les réglages.
pub fn profil_renommage_serie(r: &Reglages) -> ProfilRenommageSerie {
    let rr = &r.renommeur;
    ProfilRenommageSerie {
        dossier_serie_actif: !rr.gabarit_dossier_serie.trim().is_empty(),
        gabarit_dossier_serie: rr.gabarit_dossier_serie.clone(),
        dossier_saison_actif: !rr.gabarit_dossier_saison.trim().is_empty(),
        gabarit_dossier_saison: rr.gabarit_dossier_saison.clone(),
        fichier_actif: !rr.gabarit_fichier_episode.trim().is_empty(),
        gabarit_fichier: rr.gabarit_fichier_episode.clone(),
        remplacer_espaces_serie: rr.remplacer_espaces,
        remplacement_espaces_serie: rr.caractere_espace.clone(),
        remplacer_espaces_saison: rr.remplacer_espaces,
        remplacement_espaces_saison: rr.caractere_espace.clone(),
        remplacer_espaces_fichier: rr.remplacer_espaces,
        remplacement_espaces_fichier: rr.caractere_espace.clone(),
        remplacement_deux_points: rr.caractere_remplacement.clone(),
        ascii: rr.ascii_seulement,
        prefixes_titre: r.general.prefixes_titre.clone(),
        ..ProfilRenommageSerie::default()
    }
}

fn charger(n: &Noyau, ids: &[String]) -> Resultat<(Vec<Serie>, Vec<String>)> {
    let s = n.verrou();
    let mut series = Vec::new();
    let mut verrouillees = Vec::new();
    for id in ids {
        if let Some(x) = s.bibliotheque.obtenir::<Serie>(id)? {
            if x.verrouille {
                verrouillees.push(x.id);
            } else {
                series.push(x);
            }
        }
    }
    Ok((series, verrouillees))
}

/// Ajoute les éléments d'un plan à un autre en décalant les dépendances.
fn concatener(cible: &mut PlanRenommage, plan: PlanRenommage) {
    let decalage = cible.elements.len();
    for mut el in plan.elements {
        el.depend_de = el.depend_de.map(|d| d + decalage);
        for c in el.conflits.iter_mut() {
            if let Conflit::Dependance(i) = c {
                *i += decalage;
            }
        }
        cible.elements.push(el);
    }
}

/// Aperçu : plan de renommage d'une sélection de séries (aucune écriture),
/// séries verrouillées exclues et signalées.
pub(crate) fn apercu_series(n: &Noyau, ids: &[String]) -> Resultat<ApercuRenommage> {
    let (series, verrouilles) = charger(n, ids)?;
    let profil = profil_renommage_serie(&n.reglages());
    let mut plan = PlanRenommage::default();
    for s in &series {
        concatener(
            &mut plan,
            planifier_serie(s, &profil, &SondeSystemeFichiers),
        );
    }
    Ok(ApercuRenommage { plan, verrouilles })
}

/// Tâche (ou étape) : renommage d'une sélection de séries.
pub(crate) fn renommer_series(
    n: &Noyau,
    ctx: &ContexteTache,
    ids: &[String],
) -> Resultat<RapportRenommage> {
    ctx.verifier_annulation()?;
    let reglages = n.reglages();
    let (series, verrouilles) = charger(n, ids)?;
    let profil = profil_renommage_serie(&reglages);
    let options = OptionsExecution {
        corbeille: reglages.general.corbeille_activee,
        ..OptionsExecution::default()
    };
    let mut rapport = RapportRenommage {
        verrouilles,
        ..RapportRenommage::default()
    };
    let total = series.len() as u64;
    for (k, mut serie) in series.into_iter().enumerate() {
        ctx.verifier_annulation()?;
        ctx.progression(k as u64, total, serie.titre.clone());
        let plan = planifier_serie(&serie, &profil, &SondeSystemeFichiers);
        for (i, c) in plan.conflits() {
            ctx.message(
                NiveauMessage::Avertissement,
                format!("{} : {c}", plan.elements[i].libelle),
            );
        }
        let resultat = executer(&plan, &options);
        let reussis = elements_reussis(&resultat, plan.elements.len());
        let mut change = false;
        let mut echec: Option<String> = None;
        for r in &resultat.resultats {
            match &r.statut {
                StatutElement::Reussi { .. } => change = true,
                StatutElement::Inchange => {}
                StatutElement::Ignore { raison } => {
                    echec.get_or_insert_with(|| {
                        format!("{} : {raison}", plan.elements[r.indice].libelle)
                    });
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
                    ctx.message(
                        NiveauMessage::Erreur,
                        format!("{} : {msg}", plan.elements[r.indice].libelle),
                    );
                    echec.get_or_insert(msg);
                }
            }
        }
        if change {
            let ancien = serie.chemin.clone();
            appliquer_aux_episodes(&mut serie, &plan, &reussis);
            if serie.chemin != ancien {
                let nouveau = serie.chemin.clone();
                for s in serie.saisons.iter_mut() {
                    for f in s.fichiers.iter_mut() {
                        if let Ok(rel) = f.chemin.strip_prefix(&ancien) {
                            f.chemin = nouveau.join(rel);
                        }
                    }
                }
            }
            let avait_nfo = serie
                .fichiers
                .iter()
                .any(|f| f.type_fichier == TypeFichier::Nfo);
            if avait_nfo
                && let Err(e) = ecrire_nfo_serie_complete(
                    &mut serie,
                    &reglages,
                    n.config.horodatage_nfo,
                    n.config.nommage_nfo_saison,
                )
            {
                ctx.message(NiveauMessage::Avertissement, e.to_string());
            }
            n.verrou().bibliotheque.enregistrer(&mut serie)?;
            ctx.emettre(Evenement::SerieModifiee {
                id: serie.id.clone(),
            });
            rapport.renommes.push(serie.id.clone());
        }
        match echec {
            Some(m) => rapport.echecs.push((serie.id.clone(), m)),
            None if !change => rapport.inchanges.push(serie.id.clone()),
            None => {}
        }
    }
    ctx.progression(total, total, "Terminé");
    Ok(rapport)
}
