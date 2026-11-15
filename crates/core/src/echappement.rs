//! Échappement des valeurs injectées dans les modèles d'export (HTML, XML,
//! JSON, CSV).
//!
//! Le moteur de gabarits n'échappe rien : l'échappement est appliqué en
//! amont, sur toutes les valeurs textuelles du contexte
//! ([`echapper_valeur`]), selon l'extension du fichier produit.

use mediamanager_gabarit::Valeur;

/// Encodage appliqué aux valeurs textuelles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Echappement {
    /// Aucun échappement (texte brut).
    Aucun,
    Html,
    Xml,
    /// Contenu d'une chaîne JSON (sans les guillemets englobants).
    Json,
    /// Champ CSV : toujours entre guillemets, guillemets doublés.
    Csv,
}

impl Echappement {
    /// Encodeur associé à l'extension du fichier de sortie (`html`/`htm` →
    /// HTML, `xml` → XML, `json` → JSON, `csv` → CSV, autre → aucun).
    pub fn depuis_extension(extension: &str) -> Self {
        match extension.trim_start_matches('.').to_lowercase().as_str() {
            "html" | "htm" | "xhtml" => Echappement::Html,
            "xml" => Echappement::Xml,
            "json" => Echappement::Json,
            "csv" => Echappement::Csv,
            _ => Echappement::Aucun,
        }
    }

    /// Échappe un texte.
    pub fn echapper(self, s: &str) -> String {
        match self {
            Echappement::Aucun => s.to_string(),
            Echappement::Html => echapper_html(s),
            Echappement::Xml => echapper_xml(s),
            Echappement::Json => echapper_json(s),
            Echappement::Csv => champ_csv(s),
        }
    }
}

/// Échappement HTML : `& < > " '`.
pub fn echapper_html(s: &str) -> String {
    let mut r = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => r.push_str("&amp;"),
            '<' => r.push_str("&lt;"),
            '>' => r.push_str("&gt;"),
            '"' => r.push_str("&quot;"),
            '\'' => r.push_str("&#39;"),
            _ => r.push(c),
        }
    }
    r
}

/// Échappement XML : comme le HTML, avec `&apos;`, et sans les caractères de
/// contrôle interdits en XML 1.0.
pub fn echapper_xml(s: &str) -> String {
    let mut r = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => r.push_str("&amp;"),
            '<' => r.push_str("&lt;"),
            '>' => r.push_str("&gt;"),
            '"' => r.push_str("&quot;"),
            '\'' => r.push_str("&apos;"),
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            _ => r.push(c),
        }
    }
    r
}

/// Contenu d'une chaîne JSON.
pub fn echapper_json(s: &str) -> String {
    let mut r = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => r.push_str("\\\""),
            '\\' => r.push_str("\\\\"),
            '\n' => r.push_str("\\n"),
            '\r' => r.push_str("\\r"),
            '\t' => r.push_str("\\t"),
            c if (c as u32) < 0x20 => r.push_str(&format!("\\u{:04x}", c as u32)),
            _ => r.push(c),
        }
    }
    r
}

/// Champ CSV entre guillemets (RFC 4180) : `"` devient `""`.
pub fn champ_csv(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

/// Applique l'échappement à toutes les valeurs textuelles (récursivement).
///
/// En CSV, une liste de textes devient un seul champ (éléments joints par
/// `", "`, puis un seul jeu de guillemets) : la jointure par défaut du moteur
/// produirait sinon des guillemets au milieu du champ.
pub fn echapper_valeur(v: &Valeur, mode: Echappement) -> Valeur {
    if mode == Echappement::Aucun {
        return v.clone();
    }
    match v {
        Valeur::Texte(t) => Valeur::Texte(mode.echapper(t)),
        Valeur::Liste(l) => {
            if mode == Echappement::Csv
                && !l.is_empty()
                && l.iter().all(|x| matches!(x, Valeur::Texte(_)))
            {
                let joint = l
                    .iter()
                    .map(Valeur::en_texte_brut)
                    .collect::<Vec<_>>()
                    .join(", ");
                return Valeur::Texte(champ_csv(&joint));
            }
            Valeur::Liste(l.iter().map(|x| echapper_valeur(x, mode)).collect())
        }
        Valeur::Dictionnaire(d) => Valeur::Dictionnaire(
            d.iter()
                .map(|(k, x)| (k.clone(), echapper_valeur(x, mode)))
                .collect(),
        ),
        autre => autre.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mediamanager_gabarit::Contexte;

    #[test]
    fn html_echappe_les_cinq_caracteres() {
        assert_eq!(
            echapper_html(r#"<a href="x">T&J's</a>"#),
            "&lt;a href=&quot;x&quot;&gt;T&amp;J&#39;s&lt;/a&gt;"
        );
    }

    #[test]
    fn xml_retire_les_controles() {
        assert_eq!(echapper_xml("a\u{1}b<'"), "ab&lt;&apos;");
    }

    #[test]
    fn json_echappe_guillemets_et_controles() {
        assert_eq!(echapper_json("a\"b\\c\n\u{2}"), "a\\\"b\\\\c\\n\\u0002");
    }

    #[test]
    fn csv_double_les_guillemets_et_garde_les_retours() {
        assert_eq!(
            champ_csv("Il dit \"oui\", puis\nnon"),
            "\"Il dit \"\"oui\"\", puis\nnon\""
        );
    }

    #[test]
    fn extension_vers_encodeur() {
        assert_eq!(Echappement::depuis_extension("HTML"), Echappement::Html);
        assert_eq!(Echappement::depuis_extension(".csv"), Echappement::Csv);
        assert_eq!(Echappement::depuis_extension("txt"), Echappement::Aucun);
    }

    #[test]
    fn valeur_recursive_et_listes_csv() {
        let ctx = Contexte::nouveau()
            .avec("t", "A<B")
            .avec("n", 5)
            .avec("g", vec!["Action", "Sci\"Fi"]);
        let v = echapper_valeur(&ctx.en_valeur(), Echappement::Html);
        assert_eq!(v.champ("t"), Valeur::Texte("A&lt;B".into()));
        assert_eq!(v.champ("n"), Valeur::Entier(5));
        let c = echapper_valeur(&ctx.en_valeur(), Echappement::Csv);
        assert_eq!(c.champ("g"), Valeur::Texte("\"Action, Sci\"\"Fi\"".into()));
    }
}
