//! Analyse syntaxique d'un gabarit : texte → arbre de nœuds.

use crate::erreur::{ErreurGabarit, Resultat};

/// Indice d'accès `[...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Indice {
    /// `[last]`
    Dernier,
    /// `[n]` (base 0)
    Position(usize),
    /// `[début,compte]`
    Tranche(usize, usize),
}

/// Segment d'un chemin `a.b[0].c`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Segment {
    pub nom: String,
    pub indices: Vec<Indice>,
}

/// Appel d'un rendu nommé `;nom(args)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AppelRendu {
    pub nom: String,
    pub args: Option<String>,
}

/// Expression : chemin + rendus nommés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Expression {
    pub chemin: Vec<Segment>,
    pub rendus: Vec<AppelRendu>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Condition {
    Ou(Vec<Condition>),
    Et(Vec<Condition>),
    Atome {
        negation: bool,
        expr: Expression,
        /// `(egalite, valeur)` pour `=` / `!=`.
        comparaison: Option<(bool, String)>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Noeud {
    Texte(String),
    Sortie {
        prefixe: String,
        expr: Expression,
        suffixe: String,
    },
    Si {
        condition: Condition,
        alors: Vec<Noeud>,
        sinon: Vec<Noeud>,
    },
    Pour {
        liste: Expression,
        variable: String,
        separateur: String,
        corps: Vec<Noeud>,
    },
    /// `${@regexp "motif" source "gabarit"}` ; `None` si arguments invalides.
    Regexp(Option<(String, Expression, String)>),
}

enum Arret {
    Fichier,
    Sinon,
    Fermeture,
}

struct Analyseur<'a> {
    src: &'a str,
    pos: usize,
}

/// Analyse un gabarit complet.
pub(crate) fn analyser(src: &str) -> Resultat<Vec<Noeud>> {
    let mut a = Analyseur { src, pos: 0 };
    let (noeuds, fin) = a.lire_bloc(0)?;
    match fin {
        Arret::Fichier => Ok(noeuds),
        Arret::Sinon => Err(ErreurGabarit::MotInattendu {
            mot: "else".into(),
            position: a.pos,
        }),
        Arret::Fermeture => Err(ErreurGabarit::MotInattendu {
            mot: "end".into(),
            position: a.pos,
        }),
    }
}

const PROFONDEUR_MAX: usize = 64;

impl Analyseur<'_> {
    fn lire_bloc(&mut self, profondeur: usize) -> Resultat<(Vec<Noeud>, Arret)> {
        if profondeur > PROFONDEUR_MAX {
            return Err(ErreurGabarit::ProfondeurExcessive);
        }
        let mut noeuds = Vec::new();
        loop {
            let Some(rel) = self.src[self.pos..].find("${") else {
                if self.pos < self.src.len() {
                    noeuds.push(Noeud::Texte(self.src[self.pos..].to_string()));
                }
                self.pos = self.src.len();
                return Ok((noeuds, Arret::Fichier));
            };
            let debut = self.pos + rel;
            if rel > 0 {
                noeuds.push(Noeud::Texte(self.src[self.pos..debut].to_string()));
            }
            let fin = self.fin_balise(debut)?;
            let contenu = &self.src[debut + 2..fin];
            self.pos = fin + 1;
            let t = contenu.trim();
            if t == "else" {
                return Ok((noeuds, Arret::Sinon));
            }
            if t == "end" {
                return Ok((noeuds, Arret::Fermeture));
            }
            if let Some(reste) = mot_cle(t, "if") {
                let condition = analyser_condition(reste, debut)?;
                let (alors, f) = self.lire_bloc(profondeur + 1)?;
                let sinon = match f {
                    Arret::Fermeture => Vec::new(),
                    Arret::Sinon => {
                        let (s, f2) = self.lire_bloc(profondeur + 1)?;
                        match f2 {
                            Arret::Fermeture => s,
                            Arret::Sinon => {
                                return Err(ErreurGabarit::MotInattendu {
                                    mot: "else".into(),
                                    position: self.pos,
                                });
                            }
                            Arret::Fichier => {
                                return Err(ErreurGabarit::BlocNonTermine { bloc: "if".into() });
                            }
                        }
                    }
                    Arret::Fichier => {
                        return Err(ErreurGabarit::BlocNonTermine { bloc: "if".into() });
                    }
                };
                noeuds.push(Noeud::Si {
                    condition,
                    alors,
                    sinon,
                });
            } else if let Some(reste) = mot_cle(contenu.trim_start(), "foreach") {
                let (liste, variable, separateur) = analyser_foreach(reste, debut)?;
                let (corps, f) = self.lire_bloc(profondeur + 1)?;
                match f {
                    Arret::Fermeture => {}
                    Arret::Sinon => {
                        return Err(ErreurGabarit::MotInattendu {
                            mot: "else".into(),
                            position: self.pos,
                        });
                    }
                    Arret::Fichier => {
                        return Err(ErreurGabarit::BlocNonTermine {
                            bloc: "foreach".into(),
                        });
                    }
                }
                noeuds.push(Noeud::Pour {
                    liste,
                    variable,
                    separateur,
                    corps,
                });
            } else if t.starts_with('@') {
                noeuds.push(analyser_annotation(t, debut)?);
            } else {
                noeuds.push(analyser_sortie(contenu, debut)?);
            }
        }
    }

    /// Position de l'accolade fermante de la balise ouverte en `debut`.
    fn fin_balise(&self, debut: usize) -> Resultat<usize> {
        let corps = &self.src[debut + 2..];
        let annotation = corps.trim_start().starts_with('@');
        let mut profondeur = 1usize;
        let mut guillemets = false;
        let mut echappe = false;
        for (i, c) in corps.char_indices() {
            if annotation && guillemets {
                if echappe {
                    echappe = false;
                } else if c == '\\' {
                    echappe = true;
                } else if c == '"' {
                    guillemets = false;
                }
                continue;
            }
            match c {
                '"' if annotation => guillemets = true,
                '{' => profondeur += 1,
                '}' => {
                    profondeur -= 1;
                    if profondeur == 0 {
                        return Ok(debut + 2 + i);
                    }
                }
                _ => {}
            }
        }
        Err(ErreurGabarit::BaliseNonFermee { position: debut })
    }
}

/// Si `t` commence par le mot-clé suivi d'un blanc, renvoie le reste.
fn mot_cle<'a>(t: &'a str, mot: &str) -> Option<&'a str> {
    let reste = t.strip_prefix(mot)?;
    if reste.starts_with(char::is_whitespace) {
        Some(reste.trim_start())
    } else if reste.is_empty() {
        Some("")
    } else {
        None
    }
}

fn syntaxe(position: usize, message: &str) -> ErreurGabarit {
    ErreurGabarit::Syntaxe {
        position,
        message: message.to_string(),
    }
}

fn analyser_foreach(reste: &str, pos: usize) -> Resultat<(Expression, String, String)> {
    let (liste, apres) = reste
        .split_once(char::is_whitespace)
        .ok_or_else(|| syntaxe(pos, "`foreach` attend une liste et un nom de variable"))?;
    let apres = apres.trim_start();
    let (variable, separateur) = match apres.split_once(char::is_whitespace) {
        Some((v, sep)) => (v, sep),
        None => (apres, ""),
    };
    if variable.is_empty() || !nom_valide(variable) {
        return Err(syntaxe(pos, "nom de variable de boucle invalide"));
    }
    Ok((
        analyser_expression(liste, pos)?,
        variable.to_string(),
        separateur.to_string(),
    ))
}

fn analyser_condition(s: &str, pos: usize) -> Resultat<Condition> {
    if s.trim().is_empty() {
        return Err(syntaxe(pos, "condition vide"));
    }
    let ou: Vec<&str> = s.split("||").collect();
    let mut branches = Vec::new();
    for o in ou {
        let mut et = Vec::new();
        for a in o.split("&&") {
            et.push(analyser_atome(a, pos)?);
        }
        branches.push(if et.len() == 1 {
            et.remove(0)
        } else {
            Condition::Et(et)
        });
    }
    Ok(if branches.len() == 1 {
        branches.remove(0)
    } else {
        Condition::Ou(branches)
    })
}

fn analyser_atome(a: &str, pos: usize) -> Resultat<Condition> {
    let mut a = a.trim();
    let mut negation = false;
    if let Some(r) = a.strip_prefix('!') {
        negation = true;
        a = r.trim_start();
    }
    let (gauche, comparaison) = if let Some(i) = a.find("!=") {
        (&a[..i], Some((false, &a[i + 2..])))
    } else if let Some(i) = a.find('=') {
        (&a[..i], Some((true, &a[i + 1..])))
    } else {
        (a, None)
    };
    let comparaison = comparaison.map(|(eg, v)| (eg, retirer_guillemets(v.trim()).to_string()));
    Ok(Condition::Atome {
        negation,
        expr: analyser_expression(gauche.trim(), pos)?,
        comparaison,
    })
}

fn retirer_guillemets(v: &str) -> &str {
    for q in ['"', '\''] {
        if v.len() >= 2 && v.starts_with(q) && v.ends_with(q) {
            return &v[1..v.len() - 1];
        }
    }
    v
}

/// Découpe `s` sur `sep` hors parenthèses et crochets.
pub(crate) fn decouper_niveau(s: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut p, mut c) = (0i32, 0i32);
    let mut debut = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => p += 1,
            ')' => p -= 1,
            '[' => c += 1,
            ']' => c -= 1,
            _ if ch == sep && p <= 0 && c <= 0 => {
                parts.push(&s[debut..i]);
                debut = i + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&s[debut..]);
    parts
}

fn nom_valide(n: &str) -> bool {
    !n.is_empty()
        && n.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

/// Analyse `chemin[;rendu(args)]*`.
pub(crate) fn analyser_expression(s: &str, pos: usize) -> Resultat<Expression> {
    let s = s.trim();
    let mut parts = decouper_niveau(s, ';').into_iter();
    let chemin = analyser_chemin(parts.next().unwrap_or("").trim(), pos)?;
    let mut rendus = Vec::new();
    for r in parts {
        rendus.push(analyser_rendu(r.trim(), pos)?);
    }
    Ok(Expression { chemin, rendus })
}

/// Analyse un appel de rendu `nom` ou `nom(args)`.
pub(crate) fn analyser_rendu(r: &str, pos: usize) -> Resultat<AppelRendu> {
    if let Some(i) = r.find('(') {
        let nom = r[..i].trim();
        let Some(args) = r[i + 1..].strip_suffix(')') else {
            return Err(syntaxe(pos, "parenthèse fermante manquante dans un rendu"));
        };
        if !nom_valide(nom) {
            return Err(syntaxe(pos, "nom de rendu invalide"));
        }
        Ok(AppelRendu {
            nom: nom.to_string(),
            args: Some(args.to_string()),
        })
    } else {
        if !nom_valide(r) {
            return Err(syntaxe(pos, "nom de rendu invalide"));
        }
        Ok(AppelRendu {
            nom: r.to_string(),
            args: None,
        })
    }
}

fn analyser_chemin(s: &str, pos: usize) -> Resultat<Vec<Segment>> {
    if s.is_empty() {
        return Err(syntaxe(pos, "variable manquante"));
    }
    let mut segments = Vec::new();
    for seg in decouper_niveau_chemin(s) {
        let (nom, reste) = match seg.find('[') {
            Some(i) => (&seg[..i], &seg[i..]),
            None => (seg, ""),
        };
        if !nom_valide(nom) {
            return Err(syntaxe(pos, &format!("nom de variable invalide `{nom}`")));
        }
        let mut indices = Vec::new();
        let mut r = reste;
        while !r.is_empty() {
            let Some(fin) = r.find(']') else {
                return Err(syntaxe(pos, "crochet fermant manquant"));
            };
            if !r.starts_with('[') {
                return Err(syntaxe(pos, "caractère inattendu après un indice"));
            }
            indices.push(analyser_indice(&r[1..fin])?);
            r = &r[fin + 1..];
        }
        segments.push(Segment {
            nom: nom.to_string(),
            indices,
        });
    }
    Ok(segments)
}

fn decouper_niveau_chemin(s: &str) -> Vec<&str> {
    decouper_niveau(s, '.')
}

fn analyser_indice(s: &str) -> Resultat<Indice> {
    let t = s.trim();
    let invalide = || ErreurGabarit::IndiceInvalide {
        indice: s.to_string(),
    };
    if t == "last" {
        return Ok(Indice::Dernier);
    }
    if let Some((a, b)) = t.split_once(',') {
        let a = a.trim().parse::<usize>().map_err(|_| invalide())?;
        let b = b.trim().parse::<usize>().map_err(|_| invalide())?;
        return Ok(Indice::Tranche(a, b));
    }
    t.parse::<usize>()
        .map(Indice::Position)
        .map_err(|_| invalide())
}

/// Balise de sortie : forme simple ou forme `préfixe,expression,suffixe`.
fn analyser_sortie(contenu: &str, pos: usize) -> Resultat<Noeud> {
    if let Ok(expr) = analyser_expression(contenu, pos) {
        return Ok(Noeud::Sortie {
            prefixe: String::new(),
            expr,
            suffixe: String::new(),
        });
    }
    // Forme préfixe/suffixe : préfixe = avant la 1re virgule, suffixe = après
    // la dernière (les virgules des arguments de rendu restent au milieu).
    match (contenu.find(','), contenu.rfind(',')) {
        (Some(a), Some(b)) if a < b => Ok(Noeud::Sortie {
            prefixe: contenu[..a].to_string(),
            expr: analyser_expression(&contenu[a + 1..b], pos)?,
            suffixe: contenu[b + 1..].to_string(),
        }),
        _ => analyser_expression(contenu, pos).map(|expr| Noeud::Sortie {
            prefixe: String::new(),
            expr,
            suffixe: String::new(),
        }),
    }
}

/// `@regexp "motif" source "gabarit"`.
fn analyser_annotation(t: &str, pos: usize) -> Resultat<Noeud> {
    let Some(reste) = t.strip_prefix('@') else {
        return Err(syntaxe(pos, "annotation invalide"));
    };
    let (nom, args) = reste.split_once(char::is_whitespace).unwrap_or((reste, ""));
    if !nom.eq_ignore_ascii_case("regexp") {
        return Err(syntaxe(pos, &format!("annotation inconnue `@{nom}`")));
    }
    let args = lire_arguments(args);
    if args.len() != 3 {
        return Ok(Noeud::Regexp(None));
    }
    let expr = analyser_expression(&args[1], pos)?;
    Ok(Noeud::Regexp(Some((
        args[0].clone(),
        expr,
        args[2].clone(),
    ))))
}

/// Arguments séparés par des blancs ; `"..."` permet les espaces (`\"` échappé).
fn lire_arguments(s: &str) -> Vec<String> {
    let mut sortie = Vec::new();
    let mut it = s.chars().peekable();
    while let Some(&c) = it.peek() {
        if c.is_whitespace() {
            it.next();
        } else if c == '"' {
            it.next();
            let mut a = String::new();
            while let Some(c) = it.next() {
                match c {
                    '\\' if it.peek() == Some(&'"') => {
                        a.push('"');
                        it.next();
                    }
                    '"' => break,
                    _ => a.push(c),
                }
            }
            sortie.push(a);
        } else {
            let mut a = String::new();
            while let Some(&c) = it.peek() {
                if c.is_whitespace() {
                    break;
                }
                a.push(c);
                it.next();
            }
            sortie.push(a);
        }
    }
    sortie
}

/// Positions `(début, fin_exclusive)` de toutes les balises `${...}` du texte
/// (les blocs `if`/`foreach` ne sont pas imbriqués : balises à plat).
pub(crate) fn lister_balises(src: &str) -> Resultat<Vec<(usize, usize)>> {
    let a = Analyseur { src, pos: 0 };
    let mut sortie = Vec::new();
    let mut pos = 0;
    while let Some(rel) = src[pos..].find("${") {
        let debut = pos + rel;
        let fin = a.fin_balise(debut)?;
        sortie.push((debut, fin + 1));
        pos = fin + 1;
    }
    Ok(sortie)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texte_litteral() {
        assert_eq!(
            analyser("a $ b {x}").unwrap(),
            vec![Noeud::Texte("a $ b {x}".into())]
        );
    }

    #[test]
    fn prefixe_suffixe() {
        let n = analyser("${ - ,edition,}").unwrap();
        match &n[0] {
            Noeud::Sortie {
                prefixe, suffixe, ..
            } => {
                assert_eq!(prefixe, " - ");
                assert_eq!(suffixe, "");
            }
            _ => panic!(),
        }
        assert!(analyser("${(,year,)}").is_ok());
        assert!(analyser("${ {edition-,edition,}}").is_ok());
    }

    #[test]
    fn erreurs() {
        assert!(matches!(
            analyser("${titre"),
            Err(ErreurGabarit::BaliseNonFermee { position: 0 })
        ));
        assert!(analyser("${if a}x").is_err());
        assert!(analyser("${end}").is_err());
        assert!(analyser("${else}").is_err());
        assert!(analyser("${}").is_err());
        assert!(analyser("${a b}").is_err());
        assert!(analyser("${a[x]}").is_err());
        assert!(analyser("${a[1}").is_err());
        assert!(analyser("${a;b(}").is_err());
        assert!(analyser("${if a}${else}${else}${end}").is_err());
        assert!(analyser("${foreach a}${end}").is_err());
        assert!(analyser("${@inconnue x}").is_err());
    }

    #[test]
    fn profondeur() {
        let s = "${if a}".repeat(200);
        assert!(matches!(
            analyser(&s),
            Err(ErreurGabarit::ProfondeurExcessive)
        ));
    }
}
