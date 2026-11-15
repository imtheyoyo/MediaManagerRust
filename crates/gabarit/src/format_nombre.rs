//! Mini-formateur de nombres à la manière de `String.format` (`%d`, `%03d`,
//! `%.1f`, `%s`, `%x`, `%%`).

use crate::erreur::{ErreurGabarit, Resultat};
use crate::valeur::Valeur;

/// Applique un format `printf` à une valeur numérique ou textuelle.
pub fn formater(format: &str, valeur: &Valeur) -> Resultat<String> {
    let err = |m: &str| ErreurGabarit::FormatNombre {
        format: format.to_string(),
        message: m.to_string(),
    };
    let c: Vec<char> = format.chars().collect();
    let mut sortie = String::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] != '%' {
            sortie.push(c[i]);
            i += 1;
            continue;
        }
        i += 1;
        let (mut gauche, mut zero, mut plus, mut espace, mut groupes) =
            (false, false, false, false, false);
        while let Some(f) = c.get(i) {
            match f {
                '-' => gauche = true,
                '0' => zero = true,
                '+' => plus = true,
                ' ' => espace = true,
                ',' => groupes = true,
                _ => break,
            }
            i += 1;
        }
        let mut largeur = 0usize;
        while let Some(d) = c.get(i).and_then(|x| x.to_digit(10)) {
            largeur = largeur.saturating_mul(10).saturating_add(d as usize);
            i += 1;
        }
        let mut precision: Option<usize> = None;
        if c.get(i) == Some(&'.') {
            i += 1;
            let mut p = 0usize;
            let mut vu = false;
            while let Some(d) = c.get(i).and_then(|x| x.to_digit(10)) {
                p = p.saturating_mul(10).saturating_add(d as usize);
                i += 1;
                vu = true;
            }
            if !vu {
                return Err(err("précision manquante"));
            }
            precision = Some(p);
        }
        if largeur > 1000 || precision.is_some_and(|p| p > 100) {
            return Err(err("largeur ou précision excessive"));
        }
        let conv = *c.get(i).ok_or_else(|| err("conversion manquante"))?;
        i += 1;
        if conv == '%' {
            sortie.push('%');
            continue;
        }
        let (signe, corps, numerique) = match conv {
            'd' => {
                let n = entier(valeur).ok_or_else(|| err("%d demande un entier"))?;
                let mut abs = n.unsigned_abs().to_string();
                if groupes {
                    abs = grouper(&abs);
                }
                (signe_de(n < 0, plus, espace), abs, true)
            }
            'x' | 'X' => {
                let n = entier(valeur).ok_or_else(|| err("%x demande un entier"))?;
                let t = if conv == 'x' {
                    format!("{n:x}")
                } else {
                    format!("{n:X}")
                };
                (String::new(), t, true)
            }
            'f' => {
                let x = valeur
                    .en_nombre()
                    .filter(|x| x.is_finite())
                    .ok_or_else(|| err("%f demande un nombre"))?;
                let t = arrondir_demi_haut(x.abs(), precision.unwrap_or(6));
                let t = if groupes {
                    match t.split_once('.') {
                        Some((e, f)) => format!("{}.{f}", grouper(e)),
                        None => grouper(&t),
                    }
                } else {
                    t
                };
                (
                    signe_de(
                        x < 0.0 && t.chars().any(|d| d != '0' && d != '.' && d != ','),
                        plus,
                        espace,
                    ),
                    t,
                    true,
                )
            }
            's' | 'S' => {
                let mut t = valeur.en_texte_brut();
                if let Some(p) = precision {
                    t = t.chars().take(p).collect();
                }
                if conv == 'S' {
                    t = t.to_uppercase();
                }
                (String::new(), t, false)
            }
            autre => return Err(err(&format!("conversion `%{autre}` non prise en charge"))),
        };
        let longueur = signe.chars().count() + corps.chars().count();
        let bourrage = largeur.saturating_sub(longueur);
        if gauche {
            sortie.push_str(&signe);
            sortie.push_str(&corps);
            sortie.extend(std::iter::repeat_n(' ', bourrage));
        } else if zero && numerique {
            sortie.push_str(&signe);
            sortie.extend(std::iter::repeat_n('0', bourrage));
            sortie.push_str(&corps);
        } else {
            sortie.extend(std::iter::repeat_n(' ', bourrage));
            sortie.push_str(&signe);
            sortie.push_str(&corps);
        }
    }
    Ok(sortie)
}

fn signe_de(negatif: bool, plus: bool, espace: bool) -> String {
    if negatif {
        "-".into()
    } else if plus {
        "+".into()
    } else if espace {
        " ".into()
    } else {
        String::new()
    }
}

fn entier(v: &Valeur) -> Option<i64> {
    match v {
        Valeur::Entier(n) => Some(*n),
        Valeur::Booleen(_) => None,
        autre => {
            let x = autre.en_nombre()?;
            if x.fract() == 0.0 && x.abs() < 9e18 {
                Some(x as i64)
            } else {
                None
            }
        }
    }
}

fn grouper(chiffres: &str) -> String {
    let n = chiffres.len();
    let mut s = String::new();
    for (i, c) in chiffres.chars().enumerate() {
        if i > 0 && (n - i).is_multiple_of(3) {
            s.push(',');
        }
        s.push(c);
    }
    s
}

/// Arrondi « demi supérieur » (comme `String.format`) d'un flottant positif.
pub(crate) fn arrondir_demi_haut(x: f64, precision: usize) -> String {
    if x >= 1e17 {
        return format!("{x:.precision$}");
    }
    // Développement décimal exact (assez long pour détecter l'égalité).
    let exact = format!("{x:.60}");
    let (ent, frac) = exact.split_once('.').unwrap_or((&exact, ""));
    let mut chiffres: Vec<u8> = ent.bytes().chain(frac.bytes().take(precision)).collect();
    let suivant = frac.as_bytes().get(precision).copied().unwrap_or(b'0');
    if suivant >= b'5' {
        let mut k = chiffres.len();
        loop {
            if k == 0 {
                chiffres.insert(0, b'1');
                break;
            }
            k -= 1;
            if chiffres[k] == b'9' {
                chiffres[k] = b'0';
            } else {
                chiffres[k] += 1;
                break;
            }
        }
    }
    let nb_ent = chiffres.len() - precision;
    let mut s = String::from_utf8(chiffres[..nb_ent].to_vec()).unwrap_or_default();
    if precision > 0 {
        s.push('.');
        s.push_str(&String::from_utf8(chiffres[nb_ent..].to_vec()).unwrap_or_default());
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(fmt: &str, v: impl Into<Valeur>) -> String {
        formater(fmt, &v.into()).unwrap()
    }

    #[test]
    fn entiers() {
        assert_eq!(f("%d", 1), "1");
        assert_eq!(f("%02d", 1), "01");
        assert_eq!(f("%03d", 12), "012");
        assert_eq!(f("%d", 0), "0");
        assert_eq!(f("%+d", 5), "+5");
        assert_eq!(f("%05d", -42), "-0042");
        assert_eq!(f("%-4d|", 7), "7   |");
        assert_eq!(f("%,d", 1234567), "1,234,567");
        assert_eq!(f("S%02d", "3"), "S03");
        assert_eq!(f("%x", 255), "ff");
    }

    #[test]
    fn decimaux() {
        assert_eq!(f("%.1f", 8.65), "8.7");
        assert_eq!(f("%.1f", 8.7), "8.7");
        assert_eq!(f("%.0f", 2.5), "3");
        assert_eq!(f("%.2f", 4.3749), "4.37");
        assert_eq!(f("%.2f", 9.999), "10.00");
        assert_eq!(f("%f", 1.5), "1.500000");
        assert_eq!(f("%6.2f", 3.14259), "  3.14");
        assert_eq!(f("%.1f", -0.04), "0.0");
        assert_eq!(f("%.1f", 7), "7.0");
    }

    #[test]
    fn textes_et_erreurs() {
        assert_eq!(f("%s!", "a"), "a!");
        assert_eq!(f("100%%", 1), "100%");
        assert!(formater("%d", &Valeur::Decimal(1.5)).is_err());
        assert!(formater("%d", &Valeur::Texte("abc".into())).is_err());
        assert!(formater("%", &Valeur::Entier(1)).is_err());
        assert!(formater("%q", &Valeur::Entier(1)).is_err());
        assert!(formater("%.f", &Valeur::Entier(1)).is_err());
        assert!(formater("%99999d", &Valeur::Entier(1)).is_err());
    }
}
