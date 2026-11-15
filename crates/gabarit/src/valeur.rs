//! Valeurs typées du contexte d'évaluation.

use std::collections::BTreeMap;

use crate::dates::DateHeure;

/// Valeur d'un contexte : texte, nombre, booléen, date, liste ou dictionnaire.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Valeur {
    /// Valeur absente (rendu : chaîne vide).
    #[default]
    Vide,
    Texte(String),
    Entier(i64),
    Decimal(f64),
    Booleen(bool),
    Date(DateHeure),
    Liste(Vec<Valeur>),
    Dictionnaire(BTreeMap<String, Valeur>),
}

impl Valeur {
    /// Vrai pour `Vide`.
    pub fn est_vide(&self) -> bool {
        matches!(self, Valeur::Vide)
    }

    /// Vérité d'une valeur pour `${if ...}` : vide, texte vide ou `false`,
    /// zéro, `false`, liste/dictionnaire vide ⇒ faux ; le reste ⇒ vrai.
    pub fn est_vrai(&self) -> bool {
        match self {
            Valeur::Vide => false,
            Valeur::Texte(t) => !t.is_empty() && !t.eq_ignore_ascii_case("false"),
            Valeur::Entier(n) => *n != 0,
            Valeur::Decimal(x) => *x != 0.0 && !x.is_nan(),
            Valeur::Booleen(b) => *b,
            Valeur::Date(_) => true,
            Valeur::Liste(l) => !l.is_empty(),
            Valeur::Dictionnaire(d) => !d.is_empty(),
        }
    }

    /// Rendu par défaut : un nombre valant 0 s'affiche vide ; une liste est
    /// jointe par `", "` ; un dictionnaire s'affiche via son champ
    /// `nom`/`name`/`title`/`value`, sinon vide.
    pub fn en_texte(&self) -> String {
        match self {
            Valeur::Entier(0) => String::new(),
            Valeur::Decimal(x) if *x == 0.0 => String::new(),
            _ => self.en_texte_brut(),
        }
    }

    /// Rendu sans la règle « zéro ⇒ vide » (utilisé par `%s`, `number`…).
    pub fn en_texte_brut(&self) -> String {
        match self {
            Valeur::Vide => String::new(),
            Valeur::Texte(t) => t.clone(),
            Valeur::Entier(n) => n.to_string(),
            Valeur::Decimal(x) => formater_decimal(*x),
            Valeur::Booleen(b) => b.to_string(),
            Valeur::Date(d) => d.formater("dd.MM.yyyy HH:mm:ss Z").unwrap_or_default(),
            Valeur::Liste(l) => l
                .iter()
                .map(Valeur::en_texte_brut)
                .collect::<Vec<_>>()
                .join(", "),
            Valeur::Dictionnaire(d) => ["nom", "name", "title", "value"]
                .iter()
                .find_map(|k| d.get(*k))
                .map(Valeur::en_texte_brut)
                .unwrap_or_default(),
        }
    }

    /// Conversion numérique (texte numérique accepté).
    pub fn en_nombre(&self) -> Option<f64> {
        match self {
            Valeur::Entier(n) => Some(*n as f64),
            Valeur::Decimal(x) => Some(*x),
            Valeur::Texte(t) => t.trim().replace(',', ".").parse::<f64>().ok(),
            _ => None,
        }
    }

    /// Accès à un champ d'un dictionnaire (`Vide` sinon).
    pub fn champ(&self, nom: &str) -> Valeur {
        match self {
            Valeur::Dictionnaire(d) => d.get(nom).cloned().unwrap_or_default(),
            _ => Valeur::Vide,
        }
    }
}

/// Affiche un flottant sans décimales inutiles (`7.8`, `8`).
pub(crate) fn formater_decimal(x: f64) -> String {
    if x.is_finite() && x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

impl From<&str> for Valeur {
    fn from(v: &str) -> Self {
        Valeur::Texte(v.to_string())
    }
}
impl From<String> for Valeur {
    fn from(v: String) -> Self {
        Valeur::Texte(v)
    }
}
impl From<&String> for Valeur {
    fn from(v: &String) -> Self {
        Valeur::Texte(v.clone())
    }
}
impl From<bool> for Valeur {
    fn from(v: bool) -> Self {
        Valeur::Booleen(v)
    }
}
impl From<f64> for Valeur {
    fn from(v: f64) -> Self {
        Valeur::Decimal(v)
    }
}
impl From<f32> for Valeur {
    fn from(v: f32) -> Self {
        Valeur::Decimal(f64::from(v))
    }
}
impl From<DateHeure> for Valeur {
    fn from(v: DateHeure) -> Self {
        Valeur::Date(v)
    }
}
macro_rules! depuis_entier {
    ($($t:ty),*) => {$(
        impl From<$t> for Valeur {
            fn from(v: $t) -> Self { Valeur::Entier(i64::from(v)) }
        }
    )*};
}
depuis_entier!(i8, i16, i32, i64, u8, u16, u32);

impl From<usize> for Valeur {
    fn from(v: usize) -> Self {
        Valeur::Entier(i64::try_from(v).unwrap_or(i64::MAX))
    }
}
impl<T: Into<Valeur>> From<Vec<T>> for Valeur {
    fn from(v: Vec<T>) -> Self {
        Valeur::Liste(v.into_iter().map(Into::into).collect())
    }
}
impl<T: Into<Valeur>> From<Option<T>> for Valeur {
    fn from(v: Option<T>) -> Self {
        v.map(Into::into).unwrap_or_default()
    }
}
impl From<Contexte> for Valeur {
    fn from(v: Contexte) -> Self {
        Valeur::Dictionnaire(v.0)
    }
}

/// Contexte d'évaluation : dictionnaire clé → valeur.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Contexte(pub(crate) BTreeMap<String, Valeur>);

impl Contexte {
    /// Contexte vide.
    pub fn nouveau() -> Self {
        Self::default()
    }

    /// Insère une valeur (version chaînable).
    #[must_use]
    pub fn avec(mut self, cle: &str, valeur: impl Into<Valeur>) -> Self {
        self.0.insert(cle.to_string(), valeur.into());
        self
    }

    /// Insère une valeur.
    pub fn inserer(&mut self, cle: &str, valeur: impl Into<Valeur>) {
        self.0.insert(cle.to_string(), valeur.into());
    }

    /// Lit une valeur de premier niveau.
    pub fn obtenir(&self, cle: &str) -> Option<&Valeur> {
        self.0.get(cle)
    }

    /// Converti en dictionnaire de valeurs.
    pub fn en_valeur(&self) -> Valeur {
        Valeur::Dictionnaire(self.0.clone())
    }
}

impl<K: Into<String>, V: Into<Valeur>> FromIterator<(K, V)> for Contexte {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Contexte(
            iter.into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        )
    }
}
