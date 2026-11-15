//! Évaluation d'un gabarit analysé contre un contexte.

use std::collections::HashMap;

use regex::RegexBuilder;

use crate::erreur::{ErreurGabarit, Resultat};
use crate::options::{OptionsRendu, SubstitutionSeparateurs};
use crate::rendus;
use crate::syntaxe::{self, Condition, Expression, Indice, Noeud};
use crate::valeur::{Contexte, Valeur};

/// Table de jetons (raccourcis) : nom de jeton → expression complète.
///
/// Exemple : `title` → `movie.title`, `releaseDate` →
/// `movie.releaseDate;date(yyyy-MM-dd)`. La résolution est insensible à la
/// casse et s'applique au premier segment de chaque chemin (valeur,
/// condition, liste de `foreach`, source d'un `@regexp`), sauf aux variables
/// de boucle. Les rendus et indices écrits dans le gabarit s'ajoutent après
/// ceux du jeton : `${releaseDate;upper}` → `movie.releaseDate;date(…);upper`.
#[derive(Debug, Clone, Default)]
pub struct Jetons {
    table: HashMap<String, Expression>,
}

impl Jetons {
    /// Table vide.
    pub fn nouveau() -> Self {
        Self::default()
    }

    /// Définit un jeton ; `expression` suit la syntaxe `chemin[;rendu(args)]*`.
    pub fn ajouter(&mut self, jeton: &str, expression: &str) -> Resultat<()> {
        let e = syntaxe::analyser_expression(expression, 0).map_err(|e| ErreurGabarit::Jeton {
            jeton: jeton.to_string(),
            message: e.to_string(),
        })?;
        self.table.insert(jeton.to_lowercase(), e);
        Ok(())
    }

    /// Construit depuis des paires `(jeton, expression)`.
    pub fn depuis_paires<'a>(
        paires: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Resultat<Self> {
        let mut j = Self::nouveau();
        for (a, b) in paires {
            j.ajouter(a, b)?;
        }
        Ok(j)
    }

    /// Nombre de jetons définis.
    pub fn len(&self) -> usize {
        self.table.len()
    }

    /// Vrai si aucun jeton n'est défini.
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
}

/// Gabarit analysé, réutilisable pour plusieurs contextes.
#[derive(Debug, Clone, PartialEq)]
pub struct Gabarit {
    pub(crate) noeuds: Vec<Noeud>,
}

impl Gabarit {
    /// Analyse un gabarit (erreur de syntaxe ⇒ `Err`, jamais de panique).
    pub fn analyser(source: &str) -> Resultat<Self> {
        Ok(Gabarit {
            noeuds: syntaxe::analyser(source)?,
        })
    }

    /// Rend le gabarit.
    pub fn rendre(
        &self,
        contexte: &Contexte,
        jetons: &Jetons,
        options: &OptionsRendu,
    ) -> Resultat<String> {
        let racine = contexte.en_valeur();
        let mut ev = Evaluateur {
            racine: &racine,
            jetons,
            options,
            portees: Vec::new(),
        };
        let mut sortie = String::new();
        ev.noeuds(&self.noeuds, &mut sortie)?;
        Ok(sortie)
    }
}

/// Analyse et rend en une fois, sans jetons ni options particulières.
pub fn rendre(gabarit: &str, contexte: &Contexte) -> Resultat<String> {
    Gabarit::analyser(gabarit)?.rendre(contexte, &Jetons::default(), &OptionsRendu::default())
}

/// Analyse et rend ; en cas d'erreur renvoie le gabarit brut non transformé
/// (comportement du programme d'origine).
pub fn rendre_ou_brut(
    gabarit: &str,
    contexte: &Contexte,
    jetons: &Jetons,
    options: &OptionsRendu,
) -> String {
    Gabarit::analyser(gabarit)
        .and_then(|g| g.rendre(contexte, jetons, options))
        .unwrap_or_else(|_| gabarit.to_string())
}

pub(crate) struct Evaluateur<'a> {
    pub racine: &'a Valeur,
    pub jetons: &'a Jetons,
    pub options: &'a OptionsRendu,
    pub portees: Vec<(String, Valeur)>,
}

impl Evaluateur<'_> {
    pub(crate) fn noeuds(&mut self, noeuds: &[Noeud], sortie: &mut String) -> Resultat<()> {
        for n in noeuds {
            match n {
                Noeud::Texte(t) => sortie.push_str(t),
                Noeud::Sortie {
                    prefixe,
                    expr,
                    suffixe,
                } => {
                    let v = self.texte_expression(expr)?;
                    if !v.is_empty() {
                        sortie.push_str(prefixe);
                        sortie.push_str(&v);
                        sortie.push_str(suffixe);
                    }
                }
                Noeud::Si {
                    condition,
                    alors,
                    sinon,
                } => {
                    if self.condition(condition)? {
                        self.noeuds(alors, sortie)?;
                    } else {
                        self.noeuds(sinon, sortie)?;
                    }
                }
                Noeud::Pour {
                    liste,
                    variable,
                    separateur,
                    corps,
                } => {
                    let elements = match self.valeur(liste)? {
                        Valeur::Liste(l) => l,
                        Valeur::Dictionnaire(d) => d
                            .into_iter()
                            .map(|(k, v)| {
                                Valeur::Dictionnaire(
                                    [
                                        ("key".to_string(), Valeur::Texte(k)),
                                        ("value".to_string(), v),
                                    ]
                                    .into_iter()
                                    .collect(),
                                )
                            })
                            .collect(),
                        Valeur::Vide => Vec::new(),
                        autre => vec![autre],
                    };
                    let total = elements.len();
                    for (i, el) in elements.into_iter().enumerate() {
                        if i > 0 {
                            sortie.push_str(separateur);
                        }
                        let base = self.portees.len();
                        self.portees.push((variable.clone(), el));
                        self.portees
                            .push((format!("index_{variable}"), Valeur::Texte(i.to_string())));
                        self.portees
                            .push((format!("first_{variable}"), Valeur::Booleen(i == 0)));
                        self.portees
                            .push((format!("last_{variable}"), Valeur::Booleen(i + 1 == total)));
                        let r = self.noeuds(corps, sortie);
                        self.portees.truncate(base);
                        r?;
                    }
                }
                Noeud::Regexp(None) => {}
                Noeud::Regexp(Some((motif, source, modele))) => {
                    let texte = self.texte_expression_brut(source)?;
                    let re = RegexBuilder::new(motif)
                        .case_insensitive(true)
                        .size_limit(1 << 20)
                        .build()
                        .map_err(|e| ErreurGabarit::Regex {
                            motif: motif.clone(),
                            message: e.to_string(),
                        })?;
                    if let Some(c) = re.captures(&texte) {
                        sortie.push_str(&remplacer_groupes(modele, &c));
                    }
                }
            }
        }
        Ok(())
    }

    fn condition(&self, c: &Condition) -> Resultat<bool> {
        Ok(match c {
            Condition::Ou(l) => {
                for x in l {
                    if self.condition(x)? {
                        return Ok(true);
                    }
                }
                false
            }
            Condition::Et(l) => {
                for x in l {
                    if !self.condition(x)? {
                        return Ok(false);
                    }
                }
                true
            }
            Condition::Atome {
                negation,
                expr,
                comparaison,
            } => {
                let v = self.valeur(expr)?;
                let r = match comparaison {
                    None => v.est_vrai(),
                    Some((egalite, attendu)) => {
                        let egal = egales(&v, attendu);
                        egal == *egalite
                    }
                };
                r != *negation
            }
        })
    }

    /// Texte rendu d'une expression, avec filtrage des séparateurs.
    fn texte_expression(&self, e: &Expression) -> Resultat<String> {
        let t = self.valeur(e)?.en_texte();
        Ok(self.filtrer(e, t))
    }

    fn texte_expression_brut(&self, e: &Expression) -> Resultat<String> {
        Ok(self.valeur(e)?.en_texte())
    }

    fn filtrer(&self, e: &Expression, t: String) -> String {
        if self.options.substitution_separateurs == SubstitutionSeparateurs::Aucune
            || !(t.contains('/') || t.contains('\\'))
        {
            return t;
        }
        // Les jetons « parent » ne sont jamais filtrés.
        let (chemin, _) = self.developper(e);
        if chemin.iter().any(|s| {
            self.options
                .jetons_sans_filtre
                .iter()
                .any(|j| j.eq_ignore_ascii_case(&s.nom))
        }) {
            return t;
        }
        match self.options.substitution_separateurs {
            SubstitutionSeparateurs::Espace => t.replace(['/', '\\'], " "),
            SubstitutionSeparateurs::Unicode => {
                t.replace('/', "\u{2044}").replace('\\', "\u{2216}")
            }
            SubstitutionSeparateurs::Aucune => t,
        }
    }

    /// Développe un éventuel jeton en tête de chemin.
    fn developper(&self, e: &Expression) -> (Vec<syntaxe::Segment>, Vec<syntaxe::AppelRendu>) {
        let racine = &e.chemin[0];
        let lie = self.portees.iter().any(|(n, _)| *n == racine.nom);
        if !lie && let Some(t) = self.jetons.table.get(&racine.nom.to_lowercase()) {
            let mut chemin = t.chemin.clone();
            if let Some(dernier) = chemin.last_mut() {
                dernier.indices.extend(racine.indices.iter().cloned());
            }
            chemin.extend(e.chemin[1..].iter().cloned());
            let mut rendus = t.rendus.clone();
            rendus.extend(e.rendus.iter().cloned());
            return (chemin, rendus);
        }
        (e.chemin.clone(), e.rendus.clone())
    }

    /// Valeur d'une expression (chemin, indices, rendus).
    pub(crate) fn valeur(&self, e: &Expression) -> Resultat<Valeur> {
        let (chemin, rendus) = self.developper(e);
        let mut it = chemin.iter();
        let Some(premier) = it.next() else {
            return Ok(Valeur::Vide);
        };
        let mut v = match self.portees.iter().rev().find(|(n, _)| *n == premier.nom) {
            Some((_, v)) => v.clone(),
            None => self.racine.champ(&premier.nom),
        };
        v = appliquer_indices(v, &premier.indices);
        for seg in it {
            v = appliquer_indices(v.champ(&seg.nom), &seg.indices);
        }
        for r in &rendus {
            v = rendus::appliquer(&r.nom, r.args.as_deref(), v, self.options, 0)?;
        }
        Ok(v)
    }
}

fn egales(v: &Valeur, attendu: &str) -> bool {
    if let (Some(a), Ok(b)) = (v.en_nombre(), attendu.trim().parse::<f64>())
        && !matches!(v, Valeur::Texte(_))
    {
        return a == b;
    }
    if let Valeur::Booleen(b) = v {
        return b.to_string() == attendu;
    }
    v.en_texte_brut() == attendu
}

fn appliquer_indices(mut v: Valeur, indices: &[Indice]) -> Valeur {
    for i in indices {
        v = match (v, i) {
            (Valeur::Liste(l), Indice::Dernier) => l.last().cloned().unwrap_or_default(),
            (Valeur::Liste(l), Indice::Position(n)) => l.get(*n).cloned().unwrap_or_default(),
            (Valeur::Liste(l), Indice::Tranche(d, c)) => {
                Valeur::Liste(l.into_iter().skip(*d).take(*c).collect())
            }
            (Valeur::Texte(t), Indice::Dernier) => t
                .chars()
                .last()
                .map(|c| Valeur::Texte(c.to_string()))
                .unwrap_or_default(),
            (Valeur::Texte(t), Indice::Position(n)) => t
                .chars()
                .nth(*n)
                .map(|c| Valeur::Texte(c.to_string()))
                .unwrap_or_default(),
            (Valeur::Texte(t), Indice::Tranche(d, c)) => {
                let s: String = t.chars().skip(*d).take(*c).collect();
                if s.is_empty() {
                    Valeur::Vide
                } else {
                    Valeur::Texte(s)
                }
            }
            _ => Valeur::Vide,
        };
    }
    v
}

/// Remplace `$1`…`$9` (groupes de capture) dans `modele`.
fn remplacer_groupes(modele: &str, c: &regex::Captures<'_>) -> String {
    let mut s = String::new();
    let mut it = modele.chars().peekable();
    while let Some(ch) = it.next() {
        if ch == '$'
            && let Some(d) = it.peek().and_then(|x| x.to_digit(10))
        {
            it.next();
            if let Some(m) = c.get(d as usize) {
                s.push_str(m.as_str());
            }
            continue;
        }
        s.push(ch);
    }
    s
}
