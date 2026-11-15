//! Report des informations analysées dans le modèle [`FichierMedia`].

use super::InfosMedia;
use crate::modele::FichierMedia;

/// Remplit les champs techniques du fichier. Les flux audio et sous-titres
/// sont remplacés ; la vidéo l'est s'il y a un flux vidéo. Une valeur
/// inconnue (vide ou nulle) ne remplace pas une valeur déjà présente pour
/// le conteneur, la durée, le débit global et le titre.
pub fn appliquer_au_fichier(fichier: &mut FichierMedia, infos: &InfosMedia) {
    if !infos.conteneur.is_empty() {
        fichier.format_conteneur = infos.conteneur.clone();
    }
    if infos.duree_secondes > 0 {
        fichier.duree_secondes = infos.duree_secondes;
    }
    if infos.debit_global > 0 {
        fichier.debit_global = infos.debit_global;
    }
    if !infos.titre.is_empty() {
        fichier.titre = infos.titre.clone();
    }
    if let Some(video) = infos.video() {
        fichier.video = video.clone();
        // Le modèle ne garde qu'un format HDR : le plus riche.
        if let Some(hdr) = infos.formats_hdr.first() {
            fichier.video.format_hdr = hdr.clone();
        }
    }
    // Animation (GIF, WebP animé…) : une seule « vidéo » d'un codec d'image.
    fichier.est_graphique_anime = infos
        .video()
        .is_some_and(|v| matches!(v.codec.as_str(), "gif" | "webp" | "apng"))
        && infos.audio.is_empty();
    fichier.flux_audio = infos.audio.clone();
    fichier.sous_titres = infos.sous_titres.clone();
}
