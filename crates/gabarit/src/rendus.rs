//! Rendus nommés (`${expr;nom(args)}`).

use crate::dates::DateHeure;
use crate::erreur::{ErreurGabarit, Resultat};
use crate::format_nombre;
use crate::options::OptionsRendu;
use crate::syntaxe::{analyser_rendu, decouper_niveau};
use crate::texte;
use crate::valeur::Valeur;

const PROFONDEUR_CHAIN: usize = 16;

/// Lit une table CSV à deux colonnes (séparateur `;`, `,` ou tabulation ;
/// les lignes vides et celles commençant par `#` sont ignorées).
pub fn parser_table_csv(contenu: &str) -> Vec<(String, String)> {
    contenu
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let sep = [';', '\t', ','].into_iter().find(|s| l.contains(*s))?;
            let (a, b) = l.split_once(sep)?;
            let nettoyer = |x: &str| {
                let x = x.trim();
                if x.len() >= 2 && x.starts_with('"') && x.ends_with('"') {
                    x[1..x.len() - 1].to_string()
                } else {
                    x.to_string()
                }
            };
            Some((nettoyer(a), nettoyer(b)))
        })
        .collect()
}

fn erreur_arg(rendu: &str, message: &str) -> ErreurGabarit {
    ErreurGabarit::ArgumentRendu {
        rendu: rendu.to_string(),
        message: message.to_string(),
    }
}

/// Applique un rendu nommé à une valeur.
pub(crate) fn appliquer(
    nom: &str,
    args: Option<&str>,
    v: Valeur,
    opts: &OptionsRendu,
    profondeur: usize,
) -> Resultat<Valeur> {
    let nom_min = nom.to_lowercase();
    match nom_min.as_str() {
        "lower" | "lowercase" => Ok(Valeur::Texte(v.en_texte().to_lowercase())),
        "upper" | "uppercase" => Ok(Valeur::Texte(v.en_texte().to_uppercase())),
        "title" | "titlecase" => Ok(Valeur::Texte(texte::capitaliser_titre(&v.en_texte()))),
        "nfd" => Ok(Valeur::Texte(texte::normaliser_nfd(&v.en_texte()))),
        "first" | "firstcharacter" => Ok(Valeur::Texte(match &v {
            Valeur::Entier(_) | Valeur::Decimal(_) | Valeur::Date(_) => {
                opts.remplacement_premier_caractere.clone()
            }
            autre => {
                texte::premier_caractere(&autre.en_texte(), &opts.remplacement_premier_caractere)
            }
        })),
        "array" | "uniquearray" | "arrayunique" => {
            let sep = args.unwrap_or(", ");
            match v {
                Valeur::Liste(l) => {
                    let mut elements: Vec<String> = l.iter().map(Valeur::en_texte_brut).collect();
                    if nom_min != "array" {
                        let mut vus = std::collections::HashSet::new();
                        elements.retain(|e| vus.insert(e.clone()));
                    }
                    Ok(Valeur::Texte(elements.join(sep)))
                }
                autre => Ok(autre),
            }
        }
        "split" => rendu_split(&v.en_texte(), args).map(Valeur::Texte),
        "replace" | "replacement" => rendu_replace(&v.en_texte(), args, opts).map(Valeur::Texte),
        "number" => {
            if v.est_vide() {
                return Ok(Valeur::Vide);
            }
            let fmt = args.ok_or_else(|| erreur_arg("number", "format manquant"))?;
            format_nombre::formater(fmt, &v).map(Valeur::Texte)
        }
        "date" => rendu_date(&v, args.unwrap_or("dd.MM.yyyy HH:mm:ss Z")),
        "bitrate" => Ok(rendu_bitrate(&v, args)),
        "filesize" => Ok(rendu_filesize(&v, args)),
        "framerate" => Ok(rendu_framerate(&v, args)),
        "chain" => {
            if profondeur >= PROFONDEUR_CHAIN {
                return Err(ErreurGabarit::ProfondeurExcessive);
            }
            let mut v = v;
            for r in decouper_niveau(args.unwrap_or(""), ';') {
                let r = r.trim();
                if r.is_empty() {
                    continue;
                }
                let appel = analyser_rendu(r, 0)?;
                v = appliquer(&appel.nom, appel.args.as_deref(), v, opts, profondeur + 1)?;
            }
            Ok(v)
        }
        _ => Err(ErreurGabarit::RenduInconnu {
            nom: nom.to_string(),
        }),
    }
}

fn rendu_split(texte: &str, args: Option<&str>) -> Resultat<String> {
    let args = args.ok_or_else(|| erreur_arg("split", "indice manquant"))?;
    if !texte.contains(',') {
        return Ok(texte.to_string());
    }
    let elements: Vec<&str> = texte.split(',').map(str::trim).collect();
    let nombre = |s: &str| {
        s.trim()
            .parse::<usize>()
            .map_err(|_| erreur_arg("split", &format!("indice invalide `{s}`")))
    };
    match args.split_once(',') {
        None => Ok(elements
            .get(nombre(args)?)
            .copied()
            .unwrap_or("")
            .to_string()),
        Some((a, b)) => {
            let (a, b) = (nombre(a)?, nombre(b)?);
            if a > b || a >= elements.len() {
                return Ok(String::new());
            }
            let fin = b.min(elements.len() - 1);
            Ok(elements[a..=fin].join(", "))
        }
    }
}

fn rendu_replace(texte: &str, args: Option<&str>, opts: &OptionsRendu) -> Resultat<String> {
    let args = args.ok_or_else(|| erreur_arg("replace", "argument manquant"))?;
    if let Some(reste) = args.strip_prefix(",,") {
        return Ok(texte.replace(',', reste));
    }
    match args.split_once(',') {
        Some((a, b)) => Ok(if a.is_empty() {
            texte.to_string()
        } else {
            texte.replace(a, b)
        }),
        None if args.to_lowercase().ends_with(".csv") => {
            let resolveur = opts
                .resolveur_table
                .as_ref()
                .ok_or_else(|| erreur_arg("replace", "aucun résolveur de table configuré"))?;
            let contenu = resolveur(args)
                .ok_or_else(|| erreur_arg("replace", &format!("table `{args}` introuvable")))?;
            let mut s = texte.to_string();
            for (a, b) in parser_table_csv(&contenu) {
                if !a.is_empty() {
                    s = s.replace(&a, &b);
                }
            }
            Ok(s)
        }
        None => Ok(texte.replace(args, "")),
    }
}

fn rendu_date(v: &Valeur, fmt: &str) -> Resultat<Valeur> {
    let d = match v {
        Valeur::Date(d) => *d,
        Valeur::Entier(ms) => DateHeure::depuis_epoch_ms(*ms),
        Valeur::Texte(t) => match DateHeure::depuis_iso(t) {
            Some(d) => d,
            None => return Ok(v.clone()),
        },
        _ => return Ok(Valeur::Vide),
    };
    d.formater(fmt).map(Valeur::Texte)
}

fn nombre_non_nul(v: &Valeur) -> Option<f64> {
    v.en_nombre().filter(|x| *x != 0.0 && x.is_finite())
}

fn rendu_bitrate(v: &Valeur, unite: Option<&str>) -> Valeur {
    let Some(x) = nombre_non_nul(v) else {
        return Valeur::Vide;
    };
    let u = unite.unwrap_or("Mbps").trim().to_lowercase();
    let (val, etiquette) = if matches!(u.as_str(), "k" | "kb" | "kbps") {
        (x, "kbps")
    } else {
        (x / 1000.0, "Mbps")
    };
    Valeur::Texte(format!(
        "{} {etiquette}",
        format_nombre::arrondir_demi_haut(val.abs(), 1)
    ))
}

fn rendu_filesize(v: &Valeur, unite: Option<&str>) -> Valeur {
    let Some(x) = nombre_non_nul(v) else {
        return Valeur::Vide;
    };
    let u = unite
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .unwrap_or("GB");
    let (diviseur, etiquette) = match u {
        "K" | "kB" | "KB" => (1e3, u),
        "kiB" | "KiB" => (1024.0, u),
        "M" | "MB" => (1e6, u),
        "MiB" => (1_048_576.0, u),
        "G" | "GB" => (1e9, u),
        "GiB" => (1_073_741_824.0, u),
        "T" | "TB" => (1e12, u),
        "TiB" => (1_099_511_627_776.0, u),
        _ => (1e9, "GB"),
    };
    Valeur::Texte(format!(
        "{} {etiquette}",
        format_nombre::arrondir_demi_haut((x / diviseur).abs(), 2)
    ))
}

fn rendu_framerate(v: &Valeur, arrondi: Option<&str>) -> Valeur {
    let Some(x) = nombre_non_nul(v) else {
        return Valeur::Vide;
    };
    if arrondi.is_some_and(|a| a.trim().eq_ignore_ascii_case("round")) {
        return Valeur::Texte(format!("{}", x.round() as i64));
    }
    let s = format_nombre::arrondir_demi_haut(x.abs(), 3);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    Valeur::Texte(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(nom: &str, args: Option<&str>, v: impl Into<Valeur>) -> String {
        appliquer(nom, args, v.into(), &OptionsRendu::default(), 0)
            .unwrap()
            .en_texte()
    }

    #[test]
    fn rendus_texte() {
        assert_eq!(r("lower", None, "ABC"), "abc");
        assert_eq!(r("upper", None, "abc"), "ABC");
        assert_eq!(r("title", None, "the matrix"), "The Matrix");
        assert_eq!(r("first", None, "the"), "T");
        assert_eq!(r("first", None, 12), "#");
        assert_eq!(r("replace", Some("a,XX"), "banana"), "bXXnXXnXX");
        assert_eq!(r("replace", Some(",,x"), "a,b"), "axb");
        assert_eq!(r("replace", Some("a"), "banana"), "bnn");
        assert_eq!(r("split", Some("1"), "A, B, C"), "B");
        assert_eq!(r("split", Some("0,1"), "A, B, C"), "A, B");
        assert_eq!(r("split", Some("0,9"), "A, B, C"), "A, B, C");
        assert_eq!(r("split", Some("5"), "A, B"), "");
        assert_eq!(r("split", Some("1"), "Seul"), "Seul");
        assert_eq!(
            r("chain", Some("replace(a,XX);lower"), "BAnana"),
            "banxxnxx"
        );
    }

    #[test]
    fn rendus_listes() {
        let l = Valeur::from(vec!["Action", "Sci-Fi", "Action"]);
        assert_eq!(
            r("array", Some(" / "), l.clone()),
            "Action / Sci-Fi / Action"
        );
        assert_eq!(r("uniqueArray", None, l.clone()), "Action, Sci-Fi");
        assert_eq!(r("array", None, "texte"), "texte");
    }

    #[test]
    fn rendus_nombres() {
        assert_eq!(r("number", Some("%03d"), 7), "007");
        assert_eq!(r("number", Some("%d"), 0), "0");
        assert_eq!(r("number", Some("%.1f"), 7.84), "7.8");
        assert_eq!(r("bitrate", None, 8500), "8.5 Mbps");
        assert_eq!(r("bitrate", Some("kbps"), 8500), "8500.0 kbps");
        assert_eq!(r("filesize", None, 4_370_000_000i64), "4.37 GB");
        assert_eq!(r("filesize", Some("MiB"), 1_048_576), "1.00 MiB");
        assert_eq!(r("framerate", None, 23.976), "23.976");
        assert_eq!(r("framerate", None, 24.0), "24");
        assert_eq!(r("framerate", Some("round"), 23.976), "24");
    }

    #[test]
    fn rendu_date_et_erreurs() {
        assert_eq!(r("date", Some("yyyy-MM-dd"), "2008-01-20"), "2008-01-20");
        assert_eq!(r("date", Some("yyyy"), "pas une date"), "pas une date");
        assert!(appliquer("inconnu", None, Valeur::Vide, &OptionsRendu::default(), 0).is_err());
        assert!(
            appliquer(
                "number",
                None,
                Valeur::Entier(1),
                &OptionsRendu::default(),
                0
            )
            .is_err()
        );
        assert!(
            appliquer(
                "split",
                Some("x"),
                Valeur::Texte("a,b".into()),
                &OptionsRendu::default(),
                0
            )
            .is_err()
        );
    }

    #[test]
    fn table_csv() {
        let t = parser_table_csv("a;b\n# c\n\n\"x\";\"y\"\n");
        assert_eq!(t, vec![("a".into(), "b".into()), ("x".into(), "y".into())]);
        let o = OptionsRendu {
            resolveur_table: Some(std::sync::Arc::new(|n| {
                (n == "t.csv").then(|| "a;1\nb;2".to_string())
            })),
            ..Default::default()
        };
        let v = appliquer("replace", Some("t.csv"), "aabb".into(), &o, 0).unwrap();
        assert_eq!(v.en_texte(), "1122");
        assert!(appliquer("replace", Some("z.csv"), "a".into(), &o, 0).is_err());
    }
}
