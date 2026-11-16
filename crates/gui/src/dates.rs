//! Dates de l'éditeur de film : conversions entre le format de saisie (`jj/mm/aaaa` et
//! `jj/mm/aaaa hh:mm`), le format de stockage (`aaaa-mm-jj`) et les horodatages en
//! millisecondes (UTC). Calendrier grégorien proleptique, sans dépendance externe.

/// Vrai si l'année est bissextile.
pub fn est_bissextile(annee: i32) -> bool {
    (annee % 4 == 0 && annee % 100 != 0) || annee % 400 == 0
}

/// Nombre de jours du mois (1 à 12) ; 0 pour un mois invalide.
pub fn jours_dans_mois(annee: i32, mois: u32) -> u32 {
    match mois {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if est_bissextile(annee) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Jours écoulés depuis le 1er janvier 1970 (algorithme « days from civil »).
pub fn jours_depuis_epoque(annee: i32, mois: u32, jour: u32) -> i64 {
    let a = i64::from(annee) - i64::from(mois <= 2);
    let ere = a.div_euclid(400);
    let yoe = a - ere * 400;
    let m = i64::from(mois);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(jour) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    ere * 146_097 + doe - 719_468
}

/// Date civile (année, mois, jour) d'un nombre de jours depuis 1970.
pub fn civil_depuis_jours(jours: i64) -> (i32, u32, u32) {
    let z = jours + 719_468;
    let ere = z.div_euclid(146_097);
    let doe = z - ere * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let jour = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let mois = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let annee = (yoe + ere * 400 + i64::from(mois <= 2)) as i32;
    (annee, mois, jour)
}

/// Jour de la semaine : 0 = lundi … 6 = dimanche.
pub fn jour_semaine(annee: i32, mois: u32, jour: u32) -> u32 {
    // Le 1er janvier 1970 était un jeudi (3).
    (jours_depuis_epoque(annee, mois, jour) + 3).rem_euclid(7) as u32
}

/// Date `jj/mm/aaaa` → (année, mois, jour) si elle existe au calendrier.
pub fn composants_fr(texte: &str) -> Option<(i32, u32, u32)> {
    let p: Vec<&str> = texte.trim().split('/').collect();
    if p.len() != 3 || p[0].len() != 2 || p[1].len() != 2 || p[2].len() != 4 {
        return None;
    }
    let jour: u32 = p[0].parse().ok()?;
    let mois: u32 = p[1].parse().ok()?;
    let annee: i32 = p[2].parse().ok()?;
    (jour >= 1 && jour <= jours_dans_mois(annee, mois)).then_some((annee, mois, jour))
}

/// `jj/mm/aaaa` valide, ou chaîne vide (date absente).
pub fn date_fr_valide(texte: &str) -> bool {
    texte.trim().is_empty() || composants_fr(texte).is_some()
}

/// `aaaa-mm-jj` → `jj/mm/aaaa` ; une valeur non reconnue est renvoyée telle quelle.
pub fn iso_vers_fr(iso: &str) -> String {
    let p: Vec<&str> = iso.trim().split('-').collect();
    if p.len() == 3 && p[0].len() == 4 && p[1].len() == 2 && p[2].len() == 2 {
        format!("{}/{}/{}", p[2], p[1], p[0])
    } else {
        iso.to_string()
    }
}

/// `jj/mm/aaaa` → `aaaa-mm-jj` ; `None` si la date est invalide (chaîne vide : `Some("")`).
pub fn fr_vers_iso(texte: &str) -> Option<String> {
    if texte.trim().is_empty() {
        return Some(String::new());
    }
    let (a, m, j) = composants_fr(texte)?;
    Some(format!("{a:04}-{m:02}-{j:02}"))
}

/// Horodatage (ms, UTC) → `jj/mm/aaaa hh:mm` ; chaîne vide si l'horodatage est absent.
pub fn ms_vers_fr_heure(ms: i64) -> String {
    if ms <= 0 {
        return String::new();
    }
    let secondes = ms.div_euclid(1000);
    let (a, m, j) = civil_depuis_jours(secondes.div_euclid(86_400));
    let dans_jour = secondes.rem_euclid(86_400);
    format!(
        "{j:02}/{m:02}/{a:04} {:02}:{:02}",
        dans_jour / 3600,
        dans_jour % 3600 / 60
    )
}

/// `jj/mm/aaaa hh:mm` (l'heure est facultative) → horodatage (ms, UTC).
/// `Ok(None)` pour une chaîne vide, `Err(())` pour une saisie invalide.
#[allow(clippy::result_unit_err)]
pub fn fr_heure_vers_ms(texte: &str) -> Result<Option<i64>, ()> {
    let t = texte.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let (date, heure) = match t.split_once(' ') {
        Some((d, h)) => (d, h.trim()),
        None => (t, "00:00"),
    };
    let (a, m, j) = composants_fr(date).ok_or(())?;
    let (h, mn) = heure.split_once(':').ok_or(())?;
    let (h, mn): (i64, i64) = (h.parse().map_err(|_| ())?, mn.parse().map_err(|_| ())?);
    if !(0..24).contains(&h) || !(0..60).contains(&mn) {
        return Err(());
    }
    Ok(Some(
        (jours_depuis_epoque(a, m, j) * 86_400 + h * 3600 + mn * 60) * 1000,
    ))
}

/// Horodatage à écrire : l'original si le texte n'a pas changé par rapport à son affichage
/// (les secondes de l'original ne sont pas perdues), sinon le texte relu.
pub fn resoudre_horodatage(texte: &str, original_ms: Option<i64>) -> Option<i64> {
    if let Some(o) = original_ms
        && ms_vers_fr_heure(o) == texte.trim()
    {
        return Some(o);
    }
    fr_heure_vers_ms(texte).ok().flatten()
}

/// Mois suivant (`delta` = 1) ou précédent (`delta` = -1).
pub fn decaler_mois(annee: i32, mois: u32, delta: i32) -> (i32, u32) {
    let n = annee * 12 + (mois as i32 - 1) + delta;
    (n.div_euclid(12), (n.rem_euclid(12) + 1) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendrier() {
        assert_eq!(jours_dans_mois(2024, 2), 29);
        assert_eq!(jours_dans_mois(2026, 2), 28);
        assert_eq!(jours_dans_mois(1900, 2), 28);
        assert_eq!(jours_dans_mois(2000, 2), 29);
        assert_eq!(jours_dans_mois(2026, 13), 0);
        // Le 8 juillet 2026 est un mercredi (2) ; le 1er janvier 1970, un jeudi (3).
        assert_eq!(jour_semaine(2026, 7, 8), 2);
        assert_eq!(jour_semaine(1970, 1, 1), 3);
        assert_eq!(
            civil_depuis_jours(jours_depuis_epoque(2026, 11, 14)),
            (2026, 11, 14)
        );
        assert_eq!(civil_depuis_jours(0), (1970, 1, 1));
    }

    #[test]
    fn conversions_de_dates() {
        assert_eq!(iso_vers_fr("2026-07-08"), "08/07/2026");
        assert_eq!(iso_vers_fr(""), "");
        assert_eq!(iso_vers_fr("n'importe quoi"), "n'importe quoi");
        assert_eq!(fr_vers_iso("08/07/2026").as_deref(), Some("2026-07-08"));
        assert_eq!(fr_vers_iso("").as_deref(), Some(""));
        assert_eq!(fr_vers_iso("31/02/2026"), None);
        assert_eq!(fr_vers_iso("2026-07-08"), None);
        assert!(date_fr_valide("") && date_fr_valide("29/02/2024"));
        assert!(!date_fr_valide("29/02/2026") && !date_fr_valide("1/1/2026"));
    }

    #[test]
    fn horodatages() {
        let ms = fr_heure_vers_ms("14/11/2026 21:01").unwrap().unwrap();
        assert_eq!(ms_vers_fr_heure(ms), "14/11/2026 21:01");
        assert_eq!(fr_heure_vers_ms(""), Ok(None));
        assert_eq!(fr_heure_vers_ms("14/11/2026 25:00"), Err(()));
        assert_eq!(fr_heure_vers_ms("n/a"), Err(()));
        assert_eq!(
            fr_heure_vers_ms("14/11/2026").unwrap(),
            Some(ms - (21 * 3600 + 60) * 1000)
        );
        // Texte inchangé : l'original (avec ses secondes) est conservé.
        let avec_secondes = ms + 37_000;
        assert_eq!(
            resoudre_horodatage("14/11/2026 21:01", Some(avec_secondes)),
            Some(avec_secondes)
        );
        assert_eq!(
            resoudre_horodatage("14/11/2026 21:02", Some(avec_secondes)),
            Some(ms + 60_000)
        );
        assert_eq!(resoudre_horodatage("", Some(avec_secondes)), None);
        assert_eq!(ms_vers_fr_heure(0), "");
    }

    #[test]
    fn navigation_de_mois() {
        assert_eq!(decaler_mois(2026, 1, -1), (2025, 12));
        assert_eq!(decaler_mois(2026, 12, 1), (2027, 1));
        assert_eq!(decaler_mois(2026, 6, 1), (2026, 7));
    }
}
