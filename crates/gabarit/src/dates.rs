//! Date/heure minimale et formatage à la manière de `SimpleDateFormat`.

use crate::erreur::{ErreurGabarit, Resultat};

const MOIS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const JOURS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// Date et heure civiles avec décalage horaire (en minutes par rapport à UTC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateHeure {
    pub annee: i32,
    pub mois: u32,
    pub jour: u32,
    pub heure: u32,
    pub minute: u32,
    pub seconde: u32,
    pub milliseconde: u32,
    pub decalage_minutes: i32,
}

impl DateHeure {
    /// Date seule (minuit, UTC). Renvoie `None` si la date n'existe pas.
    pub fn date(annee: i32, mois: u32, jour: u32) -> Option<Self> {
        Self::nouvelle(annee, mois, jour, 0, 0, 0)
    }

    /// Date et heure (UTC). Renvoie `None` si une composante est hors bornes.
    pub fn nouvelle(
        annee: i32,
        mois: u32,
        jour: u32,
        heure: u32,
        minute: u32,
        seconde: u32,
    ) -> Option<Self> {
        if !(1..=12).contains(&mois)
            || jour < 1
            || jour > jours_dans_mois(annee, mois)
            || heure > 23
            || minute > 59
            || seconde > 59
        {
            return None;
        }
        Some(DateHeure {
            annee,
            mois,
            jour,
            heure,
            minute,
            seconde,
            milliseconde: 0,
            decalage_minutes: 0,
        })
    }

    /// Construit depuis un horodatage Unix en millisecondes (UTC).
    pub fn depuis_epoch_ms(ms: i64) -> Self {
        let jours = ms.div_euclid(86_400_000);
        let reste = ms.rem_euclid(86_400_000);
        let (a, m, j) = civil_depuis_jours(jours);
        DateHeure {
            annee: a,
            mois: m,
            jour: j,
            heure: (reste / 3_600_000) as u32,
            minute: ((reste / 60_000) % 60) as u32,
            seconde: ((reste / 1000) % 60) as u32,
            milliseconde: (reste % 1000) as u32,
            decalage_minutes: 0,
        }
    }

    /// Lit `AAAA-MM-JJ`, `AAAA-MM-JJ HH:MM[:SS]` ou `AAAA-MM-JJTHH:MM[:SS][Z|±HH:MM]`.
    pub fn depuis_iso(texte: &str) -> Option<Self> {
        let t = texte.trim();
        let (date, heure) = match t.find(['T', ' ']) {
            Some(i) => (&t[..i], Some(&t[i + 1..])),
            None => (t, None),
        };
        let mut p = date.split('-');
        let a: i32 = p.next()?.parse().ok()?;
        let m: u32 = p.next()?.parse().ok()?;
        let j: u32 = p.next()?.parse().ok()?;
        if p.next().is_some() {
            return None;
        }
        let mut d = Self::date(a, m, j)?;
        if let Some(h) = heure {
            let (h, decalage) = if let Some(s) = h.strip_suffix('Z') {
                (s, 0)
            } else if let Some(i) = h.rfind(['+', '-']).filter(|i| *i > 0) {
                let signe = if h.as_bytes()[i] == b'-' { -1 } else { 1 };
                let z = h[i + 1..].replace(':', "");
                let (zh, zm) = if z.len() == 4 {
                    (z[..2].parse::<i32>().ok()?, z[2..].parse::<i32>().ok()?)
                } else if z.len() == 2 {
                    (z.parse::<i32>().ok()?, 0)
                } else {
                    return None;
                };
                (&h[..i], signe * (zh * 60 + zm))
            } else {
                (h, 0)
            };
            let mut q = h.split(':');
            d.heure = q.next()?.parse().ok()?;
            d.minute = q.next()?.parse().ok()?;
            let sec = q.next().unwrap_or("0");
            let (s, frac) = sec.split_once('.').unwrap_or((sec, "0"));
            d.seconde = s.parse().ok()?;
            let f: String = frac.chars().take(3).collect();
            d.milliseconde = format!("{f:0<3}").parse().ok()?;
            if d.heure > 23 || d.minute > 59 || d.seconde > 59 {
                return None;
            }
            d.decalage_minutes = decalage;
        }
        Some(d)
    }

    /// Jour de la semaine, 0 = lundi.
    fn jour_semaine(&self) -> usize {
        let j = jours_depuis_civil(self.annee, self.mois, self.jour);
        // 1970-01-01 était un jeudi (3).
        (j + 3).rem_euclid(7) as usize
    }

    /// Formate selon un motif de type `SimpleDateFormat` (`yyyy`, `MM`, `dd`,
    /// `HH`, `mm`, `ss`, `SSS`, `MMM`, `MMMM`, `EEE`, `EEEE`, `a`, `Z`, `z`,
    /// `X`, texte entre apostrophes ; noms en anglais).
    pub fn formater(&self, motif: &str) -> Resultat<String> {
        let err = |m: &str| ErreurGabarit::FormatDate {
            format: motif.to_string(),
            message: m.to_string(),
        };
        let c: Vec<char> = motif.chars().collect();
        let mut s = String::new();
        let mut i = 0;
        while i < c.len() {
            let ch = c[i];
            if ch == '\'' {
                if c.get(i + 1) == Some(&'\'') {
                    s.push('\'');
                    i += 2;
                    continue;
                }
                i += 1;
                loop {
                    match c.get(i) {
                        None => return Err(err("apostrophe non fermée")),
                        Some('\'') if c.get(i + 1) == Some(&'\'') => {
                            s.push('\'');
                            i += 2;
                        }
                        Some('\'') => {
                            i += 1;
                            break;
                        }
                        Some(x) => {
                            s.push(*x);
                            i += 1;
                        }
                    }
                }
                continue;
            }
            if !ch.is_ascii_alphabetic() {
                s.push(ch);
                i += 1;
                continue;
            }
            let mut n = 1;
            while c.get(i + n) == Some(&ch) {
                n += 1;
            }
            i += n;
            let h12 = match self.heure % 12 {
                0 => 12,
                h => h,
            };
            match ch {
                'y' => {
                    if n == 2 {
                        s.push_str(&format!("{:02}", self.annee.rem_euclid(100)));
                    } else {
                        s.push_str(&format!("{:0n$}", self.annee, n = n));
                    }
                }
                'M' => match n {
                    1 | 2 => s.push_str(&format!("{:0n$}", self.mois, n = n)),
                    3 => s.push_str(&MOIS[self.mois as usize - 1][..3]),
                    _ => s.push_str(MOIS[self.mois as usize - 1]),
                },
                'd' => s.push_str(&format!("{:0n$}", self.jour, n = n)),
                'H' => s.push_str(&format!("{:0n$}", self.heure, n = n)),
                'h' => s.push_str(&format!("{h12:0n$}", n = n)),
                'm' => s.push_str(&format!("{:0n$}", self.minute, n = n)),
                's' => s.push_str(&format!("{:0n$}", self.seconde, n = n)),
                'S' => s.push_str(&format!("{:0n$}", self.milliseconde, n = n)),
                'a' => s.push_str(if self.heure < 12 { "AM" } else { "PM" }),
                'E' => {
                    let nom = JOURS[self.jour_semaine()];
                    if n >= 4 {
                        s.push_str(nom);
                    } else {
                        s.push_str(&nom[..3]);
                    }
                }
                'Z' => {
                    let d = self.decalage_minutes;
                    s.push_str(&format!(
                        "{}{:02}{:02}",
                        if d < 0 { '-' } else { '+' },
                        d.abs() / 60,
                        d.abs() % 60
                    ));
                }
                'X' => {
                    let d = self.decalage_minutes;
                    if d == 0 {
                        s.push('Z');
                    } else {
                        s.push_str(&format!(
                            "{}{:02}",
                            if d < 0 { '-' } else { '+' },
                            d.abs() / 60
                        ));
                        if n >= 2 {
                            s.push_str(&format!("{:02}", d.abs() % 60));
                        }
                    }
                }
                'z' => {
                    let d = self.decalage_minutes;
                    if d == 0 {
                        s.push_str("UTC");
                    } else {
                        s.push_str(&format!(
                            "GMT{}{:02}:{:02}",
                            if d < 0 { '-' } else { '+' },
                            d.abs() / 60,
                            d.abs() % 60
                        ));
                    }
                }
                _ => return Err(err(&format!("lettre de motif inconnue `{ch}`"))),
            }
        }
        Ok(s)
    }
}

fn est_bissextile(a: i32) -> bool {
    (a % 4 == 0 && a % 100 != 0) || a % 400 == 0
}

fn jours_dans_mois(a: i32, m: u32) -> u32 {
    match m {
        2 if est_bissextile(a) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

// Algorithmes de H. Hinnant (calendrier grégorien proleptique).
fn jours_depuis_civil(a: i32, m: u32, j: u32) -> i64 {
    let a = i64::from(if m <= 2 { a - 1 } else { a });
    let ere = a.div_euclid(400);
    let aere = a.rem_euclid(400);
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(j) - 1;
    let doe = aere * 365 + aere / 4 - aere / 100 + doy;
    ere * 146_097 + doe - 719_468
}

fn civil_depuis_jours(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let ere = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let a = yoe + ere * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let j = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((if m <= 2 { a + 1 } else { a }) as i32, m, j)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_et_format() {
        let d = DateHeure::depuis_iso("2008-01-20").unwrap();
        assert_eq!(d.formater("yyyy-MM-dd").unwrap(), "2008-01-20");
        assert_eq!(d.formater("d/M/yy").unwrap(), "20/1/08");
        assert_eq!(
            d.formater("EEEE d MMMM yyyy").unwrap(),
            "Sunday 20 January 2008"
        );
        assert_eq!(
            d.formater("dd.MM.yyyy HH:mm:ss Z").unwrap(),
            "20.01.2008 00:00:00 +0000"
        );
    }

    #[test]
    fn epoch() {
        let d = DateHeure::depuis_epoch_ms(0);
        assert_eq!(d.formater("yyyy-MM-dd HH:mm").unwrap(), "1970-01-01 00:00");
        assert_eq!(d.formater("EEE").unwrap(), "Thu");
        let d = DateHeure::depuis_epoch_ms(1_700_000_000_000);
        assert_eq!(
            d.formater("yyyy-MM-dd HH:mm:ss").unwrap(),
            "2023-11-14 22:13:20"
        );
    }

    #[test]
    fn invalide() {
        assert!(DateHeure::depuis_iso("2008-13-01").is_none());
        assert!(DateHeure::depuis_iso("pas une date").is_none());
        let d = DateHeure::date(2000, 2, 29).unwrap();
        assert!(d.formater("yyyy-q").is_err());
        assert_eq!(d.formater("'le' dd ''").unwrap(), "le 29 '");
        assert!(d.formater("'abc").is_err());
    }

    #[test]
    fn heure_et_decalage() {
        let d = DateHeure::depuis_iso("2020-05-06T13:04:05+02:00").unwrap();
        assert_eq!(d.formater("hh:mm a Z").unwrap(), "01:04 PM +0200");
    }
}
