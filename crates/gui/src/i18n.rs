//! Internationalisation : chaînes embarquées (`include_str!`), repli clé par clé sur l'anglais,
//! paramètres nommés `{nom}` et pluriels explicites (`clé.one` / `clé.other`).

use std::collections::HashMap;

/// Nom du produit affiché dans l'interface (paramètre `{app}` des traductions).
pub const NOM_PRODUIT: &str = "MediaManagerRust";

const SOURCE_FR: &str = include_str!("../i18n/fr.properties");
const SOURCE_EN: &str = include_str!("../i18n/en.properties");

/// Langues d'interface disponibles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Langue {
    Francais,
    Anglais,
}

impl Langue {
    pub const TOUTES: [Langue; 2] = [Langue::Francais, Langue::Anglais];

    pub fn code(self) -> &'static str {
        match self {
            Langue::Francais => "fr",
            Langue::Anglais => "en",
        }
    }

    pub fn depuis_code(code: &str) -> Option<Langue> {
        let court = code.split(['-', '_']).next().unwrap_or("").to_lowercase();
        Langue::TOUTES.into_iter().find(|l| l.code() == court)
    }

    pub fn nom_natif(self) -> &'static str {
        match self {
            Langue::Francais => "Français",
            Langue::Anglais => "English",
        }
    }

    fn source(self) -> &'static str {
        match self {
            Langue::Francais => SOURCE_FR,
            Langue::Anglais => SOURCE_EN,
        }
    }

    /// Catégorie de pluriel : `true` = « one ».
    fn est_singulier(self, n: u64) -> bool {
        match self {
            Langue::Francais => n < 2,
            Langue::Anglais => n == 1,
        }
    }
}

/// Analyse un fichier `clé=valeur` (lignes `#` ignorées ; `\n` et `\\` déséchappés).
pub fn analyser(source: &str) -> HashMap<String, String> {
    let mut table = HashMap::new();
    for ligne in source.lines() {
        let ligne = ligne.trim_end_matches('\r');
        if ligne.trim().is_empty() || ligne.trim_start().starts_with('#') {
            continue;
        }
        if let Some((cle, valeur)) = ligne.split_once('=') {
            table.insert(cle.trim().to_string(), desechapper(valeur));
        }
    }
    table
}

fn desechapper(valeur: &str) -> String {
    let mut sortie = String::with_capacity(valeur.len());
    let mut chars = valeur.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => sortie.push('\n'),
                Some('t') => sortie.push('\t'),
                Some('\\') => sortie.push('\\'),
                Some(autre) => {
                    sortie.push('\\');
                    sortie.push(autre);
                }
                None => sortie.push('\\'),
            }
        } else {
            sortie.push(c);
        }
    }
    sortie
}

/// Remplace les `{nom}` connus ; les marqueurs inconnus sont laissés tels quels.
pub fn interpoler(modele: &str, args: &[(&str, &str)]) -> String {
    let mut sortie = String::with_capacity(modele.len() + 16);
    let mut reste = modele;
    while let Some(debut) = reste.find('{') {
        sortie.push_str(&reste[..debut]);
        let apres = &reste[debut + 1..];
        match apres.find('}') {
            Some(fin)
                if !apres[..fin].is_empty()
                    && apres[..fin]
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_') =>
            {
                let nom = &apres[..fin];
                if let Some((_, v)) = args.iter().find(|(k, _)| *k == nom) {
                    sortie.push_str(v);
                } else if nom == "app" {
                    sortie.push_str(NOM_PRODUIT);
                } else {
                    sortie.push('{');
                    sortie.push_str(nom);
                    sortie.push('}');
                }
                reste = &apres[fin + 1..];
            }
            _ => {
                sortie.push('{');
                reste = apres;
            }
        }
    }
    sortie.push_str(reste);
    sortie
}

/// Catalogue de traductions : langue courante + anglais de référence.
#[derive(Clone, Debug)]
pub struct I18n {
    langue: Langue,
    courante: HashMap<String, String>,
    reference: HashMap<String, String>,
}

impl I18n {
    pub fn new(langue: Langue) -> Self {
        Self {
            langue,
            courante: analyser(langue.source()),
            reference: analyser(SOURCE_EN),
        }
    }

    pub fn langue(&self) -> Langue {
        self.langue
    }

    /// Valeur brute : langue courante, puis anglais.
    pub fn brut(&self, cle: &str) -> Option<&str> {
        self.courante
            .get(cle)
            .or_else(|| self.reference.get(cle))
            .map(String::as_str)
    }

    /// Traduction sans paramètre (`{app}` est toujours résolu). Clé inconnue : `???clé???`.
    pub fn t(&self, cle: &str) -> String {
        self.t_args(cle, &[])
    }

    pub fn t_args(&self, cle: &str, args: &[(&str, &str)]) -> String {
        match self.brut(cle) {
            Some(modele) => interpoler(modele, args),
            None => format!("???{cle}???"),
        }
    }

    /// Pluriel : cherche `clé.one` / `clé.other` selon `n`, puis `clé`. `{count}` = `n`.
    pub fn t_pluriel(&self, cle: &str, n: u64, args: &[(&str, &str)]) -> String {
        let suffixe = if self.langue.est_singulier(n) {
            "one"
        } else {
            "other"
        };
        let cle_precise = format!("{cle}.{suffixe}");
        let cle_utilisee = if self.brut(&cle_precise).is_some() {
            cle_precise
        } else {
            cle.to_string()
        };
        let compte = n.to_string();
        let mut tous: Vec<(&str, &str)> = vec![("count", compte.as_str())];
        tous.extend_from_slice(args);
        self.t_args(&cle_utilisee, &tous)
    }

    /// Décimal avec le séparateur de la langue (virgule en français).
    pub fn decimal(&self, valeur: f64, decimales: usize) -> String {
        let texte = format!("{valeur:.decimales$}");
        match self.langue {
            Langue::Francais => texte.replace('.', ","),
            Langue::Anglais => texte,
        }
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new(Langue::Francais)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cles_francaises_de_la_specification() {
        let i = I18n::new(Langue::Francais);
        assert_eq!(i.t("app.movies"), "Films");
        assert_eq!(i.t("Toolbar.rename"), "Renommer & Nettoyer");
        assert_eq!(i.t("Button.ok"), "Valider");
        assert_eq!(i.t("app.nothingselected"), "Aucune entrée sélectionnée");
    }

    #[test]
    fn parametre_app_toujours_resolu() {
        let i = I18n::new(Langue::Francais);
        assert_eq!(
            i.t("app.welcome"),
            "Bienvenue dans l'assistant de configuration de MediaManagerRust"
        );
    }

    #[test]
    fn repli_sur_l_anglais_cle_par_cle() {
        let fr = I18n::new(Langue::Francais);
        assert!(!fr.courante.contains_key("message.scrape.forceoverwrite"));
        assert_eq!(
            fr.t("message.scrape.forceoverwrite"),
            "Force overwriting with empty values"
        );
    }

    #[test]
    fn cle_inconnue() {
        assert_eq!(I18n::default().t("n.existe.pas"), "???n.existe.pas???");
    }

    #[test]
    fn parametres_nommes() {
        let i = I18n::new(Langue::Francais);
        let s = i.t_args(
            "gui.movies.count",
            &[("filtered", "842"), ("total", "1204")],
        );
        assert_eq!(s, "Films : 842 de 1204");
        // marqueur inconnu laissé tel quel, accolade isolée conservée
        assert_eq!(interpoler("a {x} b { c", &[]), "a {x} b { c");
    }

    #[test]
    fn pluriels() {
        let fr = I18n::new(Langue::Francais);
        assert_eq!(fr.t_pluriel("movie.selected", 0, &[]), "0 film sélectionné");
        assert_eq!(fr.t_pluriel("movie.selected", 1, &[]), "1 film sélectionné");
        assert_eq!(
            fr.t_pluriel("movie.selected", 3, &[]),
            "3 films sélectionnés"
        );
        let en = I18n::new(Langue::Anglais);
        assert_eq!(en.t_pluriel("movie.selected", 0, &[]), "0 movies selected");
        assert_eq!(en.t_pluriel("movie.selected", 1, &[]), "1 movie selected");
    }

    #[test]
    fn decimal_localise() {
        assert_eq!(I18n::new(Langue::Francais).decimal(21.4, 1), "21,4");
        assert_eq!(I18n::new(Langue::Anglais).decimal(21.4, 1), "21.4");
    }

    #[test]
    fn echappements_et_commentaires() {
        let t = analyser("# c\n\na=1\\n2\nb=x=y\n");
        assert_eq!(t["a"], "1\n2");
        assert_eq!(t["b"], "x=y");
    }

    #[test]
    fn toutes_les_cles_francaises_existent_en_anglais() {
        let fr = analyser(SOURCE_FR);
        let en = analyser(SOURCE_EN);
        for cle in fr.keys() {
            assert!(en.contains_key(cle), "clé absente de l'anglais : {cle}");
        }
    }

    #[test]
    fn aucune_mention_du_programme_d_origine() {
        for source in [SOURCE_FR, SOURCE_EN] {
            assert!(
                !source
                    .to_lowercase()
                    .contains(&["tiny", "media", "manager"].concat())
            );
        }
    }

    #[test]
    fn code_de_langue() {
        assert_eq!(Langue::depuis_code("fr_FR"), Some(Langue::Francais));
        assert_eq!(Langue::depuis_code("en-GB"), Some(Langue::Anglais));
        assert_eq!(Langue::depuis_code("xx"), None);
    }
}
