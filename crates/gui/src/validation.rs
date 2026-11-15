//! Validation et normalisation des champs de la fenêtre des réglages (fonctions pures).
//! Chaque fonction renvoie la valeur normalisée à enregistrer, ou l'erreur à afficher.

use regex::Regex;

/// Erreur de saisie ; [`cle`](Self::cle) donne le message traduit à afficher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErreurChamp {
    Vide,
    Langue,
    Pays,
    Port,
    Extension,
    Motif,
    Doublon,
}

impl ErreurChamp {
    pub fn cle(self) -> &'static str {
        match self {
            ErreurChamp::Vide => "gui.settings.err.empty",
            ErreurChamp::Langue => "gui.settings.err.language",
            ErreurChamp::Pays => "gui.settings.err.country",
            ErreurChamp::Port => "gui.settings.err.port",
            ErreurChamp::Extension => "gui.settings.err.extension",
            ErreurChamp::Motif => "gui.settings.err.pattern",
            ErreurChamp::Doublon => "gui.settings.err.duplicate",
        }
    }
}

/// Code de langue : `fr`, `en`, `pt_BR` (le tiret `pt-BR` est accepté et normalisé).
pub fn langue(saisie: &str) -> Result<String, ErreurChamp> {
    let s = saisie.trim().replace('-', "_");
    if s.is_empty() {
        return Err(ErreurChamp::Vide);
    }
    let (base, region) = match s.split_once('_') {
        Some((b, r)) => (b, Some(r)),
        None => (s.as_str(), None),
    };
    let base_ok = (2..=3).contains(&base.len()) && base.chars().all(|c| c.is_ascii_alphabetic());
    let region_ok =
        region.is_none_or(|r| r.len() == 2 && r.chars().all(|c| c.is_ascii_alphabetic()));
    if !base_ok || !region_ok {
        return Err(ErreurChamp::Langue);
    }
    Ok(match region {
        Some(r) => format!("{}_{}", base.to_lowercase(), r.to_uppercase()),
        None => base.to_lowercase(),
    })
}

/// Code de pays sur deux lettres, mis en majuscules.
pub fn pays(saisie: &str) -> Result<String, ErreurChamp> {
    let s = saisie.trim();
    if s.is_empty() {
        return Err(ErreurChamp::Vide);
    }
    if s.len() != 2 || !s.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(ErreurChamp::Pays);
    }
    Ok(s.to_uppercase())
}

/// Port réseau : vide = aucun (`None`) ; sinon entier de 1 à 65535.
pub fn port(saisie: &str) -> Result<Option<u16>, ErreurChamp> {
    let s = saisie.trim();
    if s.is_empty() {
        return Ok(None);
    }
    match s.parse::<u16>() {
        Ok(p) if p >= 1 => Ok(Some(p)),
        _ => Err(ErreurChamp::Port),
    }
}

/// Extension de fichier : `mkv` ou `.MKV` donnent `.mkv`.
pub fn extension(saisie: &str) -> Result<String, ErreurChamp> {
    let s = saisie.trim().trim_start_matches('.').to_lowercase();
    if s.is_empty() {
        return Err(ErreurChamp::Vide);
    }
    if s.len() > 12 || !s.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(ErreurChamp::Extension);
    }
    Ok(format!(".{s}"))
}

/// Expression régulière (doit se compiler).
pub fn motif(saisie: &str) -> Result<String, ErreurChamp> {
    let s = saisie.trim();
    if s.is_empty() {
        return Err(ErreurChamp::Vide);
    }
    Regex::new(s).map_err(|_| ErreurChamp::Motif)?;
    Ok(s.to_string())
}

/// Texte libre non vide (mot à retirer, article…).
pub fn mot(saisie: &str) -> Result<String, ErreurChamp> {
    let s = saisie.trim();
    if s.is_empty() {
        Err(ErreurChamp::Vide)
    } else {
        Ok(s.to_string())
    }
}

/// Ajoute `saisie` à `liste` après validation par `valider` ; refuse les doublons
/// (sans tenir compte de la casse).
pub fn ajouter(
    liste: &mut Vec<String>,
    saisie: &str,
    valider: fn(&str) -> Result<String, ErreurChamp>,
) -> Result<(), ErreurChamp> {
    let valeur = valider(saisie)?;
    if liste.iter().any(|x| x.eq_ignore_ascii_case(&valeur)) {
        return Err(ErreurChamp::Doublon);
    }
    liste.push(valeur);
    Ok(())
}

/// Liste de langues séparées par des virgules (`fr, en`) : les valeurs invalides sont
/// écartées, les doublons supprimés.
pub fn langues(saisie: &str) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for part in saisie.split([',', ';', ' ']) {
        if let Ok(l) = langue(part)
            && !v.contains(&l)
        {
            v.push(l);
        }
    }
    v
}

/// Seuil de concordance ramené dans [0 ; 1].
pub fn seuil(v: f64) -> f64 {
    if v.is_nan() { 0.75 } else { v.clamp(0.0, 1.0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn langues_normalisees() {
        assert_eq!(langue(" FR "), Ok("fr".into()));
        assert_eq!(langue("pt-br"), Ok("pt_BR".into()));
        assert_eq!(langue(""), Err(ErreurChamp::Vide));
        assert_eq!(langue("français"), Err(ErreurChamp::Langue));
        assert_eq!(langue("f"), Err(ErreurChamp::Langue));
        assert_eq!(langue("fr_FRA"), Err(ErreurChamp::Langue));
    }

    #[test]
    fn pays_sur_deux_lettres() {
        assert_eq!(pays("fr"), Ok("FR".into()));
        assert_eq!(pays("FRA"), Err(ErreurChamp::Pays));
        assert_eq!(pays("1a"), Err(ErreurChamp::Pays));
        assert_eq!(pays(" "), Err(ErreurChamp::Vide));
    }

    #[test]
    fn ports() {
        assert_eq!(port(""), Ok(None));
        assert_eq!(port(" 8080 "), Ok(Some(8080)));
        assert_eq!(port("65535"), Ok(Some(65535)));
        assert_eq!(port("0"), Err(ErreurChamp::Port));
        assert_eq!(port("65536"), Err(ErreurChamp::Port));
        assert_eq!(port("abc"), Err(ErreurChamp::Port));
        assert_eq!(port("-1"), Err(ErreurChamp::Port));
    }

    #[test]
    fn extensions() {
        assert_eq!(extension("MKV"), Ok(".mkv".into()));
        assert_eq!(extension(".mp4"), Ok(".mp4".into()));
        assert_eq!(extension("."), Err(ErreurChamp::Vide));
        assert_eq!(extension("m kv"), Err(ErreurChamp::Extension));
        assert_eq!(extension(".a/b"), Err(ErreurChamp::Extension));
    }

    #[test]
    fn motifs_regex() {
        assert_eq!(motif(r"\.txt$"), Ok(r"\.txt$".into()));
        assert_eq!(motif("(abc"), Err(ErreurChamp::Motif));
        assert_eq!(motif(""), Err(ErreurChamp::Vide));
    }

    #[test]
    fn ajout_sans_doublon() {
        let mut l = vec![".mkv".to_string()];
        assert_eq!(ajouter(&mut l, "AVI", extension), Ok(()));
        assert_eq!(
            ajouter(&mut l, ".MKV", extension),
            Err(ErreurChamp::Doublon)
        );
        assert_eq!(ajouter(&mut l, "", extension), Err(ErreurChamp::Vide));
        assert_eq!(l, vec![".mkv", ".avi"]);
    }

    #[test]
    fn listes_de_langues() {
        assert_eq!(langues("fr, EN; pt-br xx1 fr"), vec!["fr", "en", "pt_BR"]);
        assert!(langues("").is_empty());
    }

    #[test]
    fn seuil_borne() {
        assert_eq!(seuil(1.7), 1.0);
        assert_eq!(seuil(-1.0), 0.0);
        assert_eq!(seuil(0.5), 0.5);
        assert_eq!(seuil(f64::NAN), 0.75);
    }
}
