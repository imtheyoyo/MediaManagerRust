//! Arbre XML minimal et lecture tolérante.
//!
//! Les NFO rencontrés « dans la nature » sont souvent imparfaits : texte avant
//! ou après la racine, BOM, encodage non UTF-8, balises fermantes manquantes ou
//! mal imbriquées, `&` non échappés. Ce module construit un arbre simple à
//! partir de ce contenu sans jamais échouer : en cas d'erreur de syntaxe, la
//! lecture s'arrête et les éléments encore ouverts sont refermés.
//!
//! Le même arbre sert à l'écriture (sérialisation indentée de 2 espaces).

use quick_xml::escape::{escape, partial_escape};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;

/// Nœud de l'arbre : élément, texte ou commentaire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Noeud {
    Element(Element),
    Texte(String),
    Commentaire(String),
}

/// Élément XML : nom (casse d'origine), attributs ordonnés et enfants.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Element {
    pub nom: String,
    pub attributs: Vec<(String, String)>,
    pub enfants: Vec<Noeud>,
}

impl Element {
    /// Élément vide.
    pub fn nouveau(nom: &str) -> Self {
        Self {
            nom: nom.to_string(),
            ..Self::default()
        }
    }

    /// Élément contenant uniquement du texte (aucun enfant si le texte est vide).
    pub fn avec_texte(nom: &str, texte: &str) -> Self {
        let mut e = Self::nouveau(nom);
        if !texte.is_empty() {
            e.enfants.push(Noeud::Texte(texte.to_string()));
        }
        e
    }

    /// Ajoute un attribut (forme chaînable).
    pub fn attr(mut self, cle: &str, valeur: &str) -> Self {
        self.attributs.push((cle.to_string(), valeur.to_string()));
        self
    }

    /// Ajoute un élément enfant.
    pub fn ajouter(&mut self, e: Element) {
        self.enfants.push(Noeud::Element(e));
    }

    /// Ajoute un enfant `<nom>texte</nom>` (émis même si le texte est vide).
    pub fn ajouter_texte(&mut self, nom: &str, texte: &str) {
        self.ajouter(Element::avec_texte(nom, texte));
    }

    /// Ajoute un commentaire.
    pub fn ajouter_commentaire(&mut self, texte: &str) {
        self.enfants.push(Noeud::Commentaire(texte.to_string()));
    }

    /// Comparaison du nom insensible à la casse.
    pub fn a_nom(&self, nom: &str) -> bool {
        self.nom.eq_ignore_ascii_case(nom)
    }

    /// Valeur d'un attribut (nom insensible à la casse).
    pub fn attribut(&self, nom: &str) -> Option<&str> {
        self.attributs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(nom))
            .map(|(_, v)| v.as_str())
    }

    /// Éléments enfants directs.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.enfants.iter().filter_map(|n| match n {
            Noeud::Element(e) => Some(e),
            _ => None,
        })
    }

    /// Enfants directs portant un nom donné (insensible à la casse).
    pub fn enfants_nommes<'a>(&'a self, nom: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.elements().filter(move |e| e.a_nom(nom))
    }

    /// Enfant direct **unique** portant ce nom (`None` s'il est absent ou
    /// présent plusieurs fois).
    pub fn enfant_unique(&self, nom: &str) -> Option<&Element> {
        let mut it = self.elements().filter(|e| e.a_nom(nom));
        let premier = it.next()?;
        if it.next().is_some() {
            return None;
        }
        Some(premier)
    }

    /// Texte des enfants texte directs, rogné.
    pub fn texte_propre(&self) -> String {
        let mut s = String::new();
        for n in &self.enfants {
            if let Noeud::Texte(t) = n {
                s.push_str(t);
            }
        }
        s.trim().to_string()
    }

    /// Texte de tous les descendants (retours à la ligne internes conservés),
    /// rogné aux extrémités.
    pub fn texte_complet(&self) -> String {
        fn collecter(e: &Element, s: &mut String) {
            for n in &e.enfants {
                match n {
                    Noeud::Texte(t) => s.push_str(t),
                    Noeud::Element(x) => collecter(x, s),
                    Noeud::Commentaire(_) => {}
                }
            }
        }
        let mut s = String::new();
        collecter(self, &mut s);
        s.trim().to_string()
    }

    /// Texte propre de l'enfant unique `nom`, s'il est non vide.
    pub fn texte_unique(&self, nom: &str) -> Option<String> {
        self.enfant_unique(nom)
            .map(Element::texte_propre)
            .filter(|t| !t.is_empty())
    }

    /// Texte complet de l'enfant unique `nom`, s'il est non vide.
    pub fn texte_complet_unique(&self, nom: &str) -> Option<String> {
        self.enfant_unique(nom)
            .map(Element::texte_complet)
            .filter(|t| !t.is_empty())
    }

    /// Vrai si l'élément ne contient que du texte (ou rien).
    fn est_feuille(&self) -> bool {
        self.enfants.iter().all(|n| matches!(n, Noeud::Texte(_)))
    }

    /// Vrai si l'élément mélange texte non blanc et sous-éléments.
    fn est_mixte(&self) -> bool {
        let a_texte = self
            .enfants
            .iter()
            .any(|n| matches!(n, Noeud::Texte(t) if !t.trim().is_empty()));
        a_texte && !self.est_feuille()
    }

    /// Sérialise l'élément avec une indentation de 2 espaces par niveau.
    /// Les lignes sont séparées par `\n` (la conversion CRLF se fait à la fin).
    pub fn serialiser(&self, niveau: usize, sortie: &mut String) {
        let retrait = "  ".repeat(niveau);
        sortie.push_str(&retrait);
        self.ouvrir(sortie);
        if self.enfants.is_empty() {
            sortie.push_str("/>\n");
            return;
        }
        if self.est_feuille() || self.est_mixte() {
            sortie.push('>');
            self.serialiser_en_ligne_contenu(sortie);
            sortie.push_str(&format!("</{}>\n", self.nom));
            return;
        }
        sortie.push_str(">\n");
        for n in &self.enfants {
            match n {
                Noeud::Element(e) => e.serialiser(niveau + 1, sortie),
                Noeud::Commentaire(c) => {
                    sortie.push_str(&format!("{retrait}  <!--{}-->\n", nettoyer_commentaire(c)))
                }
                // Texte blanc d'indentation ignoré.
                Noeud::Texte(_) => {}
            }
        }
        sortie.push_str(&format!("{retrait}</{}>\n", self.nom));
    }

    fn ouvrir(&self, sortie: &mut String) {
        sortie.push('<');
        sortie.push_str(&self.nom);
        for (k, v) in &self.attributs {
            sortie.push_str(&format!(" {k}=\"{}\"", escape(v.as_str())));
        }
    }

    /// Contenu sérialisé sans aucune mise en forme (préserve le texte mixte).
    fn serialiser_en_ligne_contenu(&self, sortie: &mut String) {
        for n in &self.enfants {
            match n {
                Noeud::Texte(t) => sortie.push_str(&partial_escape(t.as_str())),
                Noeud::Commentaire(c) => {
                    sortie.push_str(&format!("<!--{}-->", nettoyer_commentaire(c)))
                }
                Noeud::Element(e) => {
                    e.ouvrir(sortie);
                    if e.enfants.is_empty() {
                        sortie.push_str("/>");
                    } else {
                        sortie.push('>');
                        e.serialiser_en_ligne_contenu(sortie);
                        sortie.push_str(&format!("</{}>", e.nom));
                    }
                }
            }
        }
    }
}

/// Un commentaire ne peut pas contenir `--`.
fn nettoyer_commentaire(c: &str) -> String {
    let mut s = c.to_string();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    s
}

// ---------------------------------------------------------------------------
// Décodage des octets
// ---------------------------------------------------------------------------

/// Caractères Windows-1252 de la plage 0x80–0x9F (les autres octets ≥ 0xA0
/// correspondent à Latin-1).
const CP1252: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8D}', 'Ž', '\u{8F}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9D}', 'ž', 'Ÿ',
];

fn decoder_utf16(octets: &[u8], petit_boutiste: bool) -> String {
    let unites: Vec<u16> = octets
        .chunks_exact(2)
        .map(|c| {
            if petit_boutiste {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&unites)
}

fn decoder_cp1252(octets: &[u8]) -> String {
    octets
        .iter()
        .map(|&b| match b {
            0x80..=0x9F => CP1252[(b - 0x80) as usize],
            _ => b as char,
        })
        .collect()
}

/// Décode le contenu brut d'un NFO : BOM UTF-8 / UTF-16, UTF-16 sans BOM
/// (octets nuls alternés), UTF-8, puis repli Windows-1252 / Latin-1.
pub fn decoder_octets(octets: &[u8]) -> String {
    if let Some(reste) = octets.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(reste).into_owned();
    }
    if let Some(reste) = octets.strip_prefix(&[0xFF, 0xFE]) {
        return decoder_utf16(reste, true);
    }
    if let Some(reste) = octets.strip_prefix(&[0xFE, 0xFF]) {
        return decoder_utf16(reste, false);
    }
    if octets.len() >= 4 && octets.len().is_multiple_of(2) {
        if octets[0] != 0 && octets[1] == 0 && octets[2] != 0 && octets[3] == 0 {
            return decoder_utf16(octets, true);
        }
        if octets[0] == 0 && octets[1] != 0 && octets[2] == 0 && octets[3] != 0 {
            return decoder_utf16(octets, false);
        }
    }
    match std::str::from_utf8(octets) {
        Ok(s) => s.to_string(),
        Err(_) => decoder_cp1252(octets),
    }
}

// ---------------------------------------------------------------------------
// Lecture tolérante
// ---------------------------------------------------------------------------

/// Position de la première balise ouvrante `<nom` (un des noms donnés,
/// insensible à la casse) suivie d'un blanc, de `>` ou de `/`.
pub fn localiser_racine(texte: &str, noms: &[&str]) -> Option<usize> {
    let minuscule = texte.to_ascii_lowercase();
    let octets = minuscule.as_bytes();
    let mut meilleure: Option<usize> = None;
    for nom in noms {
        let motif = format!("<{}", nom.to_ascii_lowercase());
        let mut depart = 0;
        while let Some(i) = minuscule[depart..].find(&motif) {
            let pos = depart + i;
            let suivant = octets.get(pos + motif.len()).copied();
            if matches!(suivant, Some(b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'/')) {
                meilleure = Some(meilleure.map_or(pos, |m| m.min(pos)));
                break;
            }
            depart = pos + motif.len();
        }
    }
    meilleure
}

fn element_depuis(debut: &BytesStart) -> Element {
    let nom = String::from_utf8_lossy(debut.name().as_ref()).into_owned();
    let mut attributs = Vec::new();
    for a in debut.attributes().with_checks(false).flatten() {
        let cle = String::from_utf8_lossy(a.key.as_ref()).into_owned();
        let valeur = match a.unescape_value() {
            Ok(v) => v.into_owned(),
            Err(_) => String::from_utf8_lossy(&a.value).into_owned(),
        };
        attributs.push((cle, valeur));
    }
    Element {
        nom,
        attributs,
        enfants: Vec::new(),
    }
}

fn attacher(pile: &mut [Element], racines: &mut Vec<Element>, mut e: Element) {
    // Les blancs d'indentation entre sous-éléments ne sont pas du contenu.
    if e.enfants.iter().any(|n| matches!(n, Noeud::Element(_))) {
        e.enfants
            .retain(|n| !matches!(n, Noeud::Texte(t) if t.trim().is_empty()));
    }
    match pile.last_mut() {
        Some(parent) => parent.enfants.push(Noeud::Element(e)),
        None => racines.push(e),
    }
}

fn ajouter_texte(pile: &mut [Element], texte: &str) {
    if texte.is_empty() {
        return;
    }
    // Le texte hors de tout élément (bruit autour de la racine) est ignoré.
    if let Some(parent) = pile.last_mut() {
        if let Some(Noeud::Texte(t)) = parent.enfants.last_mut() {
            t.push_str(texte);
        } else {
            parent.enfants.push(Noeud::Texte(texte.to_string()));
        }
    }
}

/// Résout une référence d'entité (`amp`, `#233`, `#xE9`…).
fn resoudre_entite(nom: &str) -> String {
    if let Some(num) = nom.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok(),
            None => num.parse::<u32>().ok(),
        };
        if let Some(c) = code.and_then(char::from_u32) {
            return c.to_string();
        }
        return format!("&{nom};");
    }
    match nom {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{A0}",
        _ => return format!("&{nom};"),
    }
    .to_string()
}

/// Analyse un fragment XML et renvoie les éléments de premier niveau. Ne
/// renvoie jamais d'erreur : la lecture s'arrête à la première erreur de
/// syntaxe et les éléments encore ouverts sont refermés.
pub fn analyser_fragment(xml: &str) -> Vec<Element> {
    // Normalisation des fins de ligne imposée par XML.
    let xml = xml.replace("\r\n", "\n").replace('\r', "\n");
    let mut lecteur = Reader::from_str(&xml);
    {
        let c = lecteur.config_mut();
        c.trim_text(false);
        c.check_end_names = false;
        c.allow_unmatched_ends = true;
        c.allow_dangling_amp = true;
        c.check_comments = false;
    }
    let mut pile: Vec<Element> = Vec::new();
    let mut racines: Vec<Element> = Vec::new();
    loop {
        match lecteur.read_event() {
            Ok(Event::Start(e)) => pile.push(element_depuis(&e)),
            Ok(Event::Empty(e)) => {
                let el = element_depuis(&e);
                attacher(&mut pile, &mut racines, el);
            }
            Ok(Event::End(e)) => {
                let nom = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                // Fermeture tolérante : on referme jusqu'à l'élément de même
                // nom ; une fermeture orpheline est ignorée.
                if let Some(pos) = pile.iter().rposition(|x| x.a_nom(&nom)) {
                    while pile.len() > pos {
                        let el = pile.pop().expect("pile non vide");
                        attacher(&mut pile, &mut racines, el);
                    }
                }
            }
            Ok(Event::Text(t)) => {
                let s = String::from_utf8_lossy(&t.into_inner()).into_owned();
                ajouter_texte(&mut pile, &s);
            }
            Ok(Event::CData(c)) => {
                let s = String::from_utf8_lossy(&c.into_inner()).into_owned();
                ajouter_texte(&mut pile, &s);
            }
            Ok(Event::GeneralRef(r)) => {
                let nom = String::from_utf8_lossy(&r.into_inner()).into_owned();
                ajouter_texte(&mut pile, &resoudre_entite(&nom));
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {}
        }
    }
    while let Some(el) = pile.pop() {
        attacher(&mut pile, &mut racines, el);
    }
    racines
}

/// Trouve la première racine portant un des noms donnés dans un texte
/// éventuellement bruité, et renvoie les éléments de premier niveau analysés
/// à partir de cette position (seuls ceux portant un des noms sont gardés).
pub fn racines_nommees(texte: &str, noms: &[&str]) -> Vec<Element> {
    let Some(debut) = localiser_racine(texte, noms) else {
        return Vec::new();
    };
    analyser_fragment(&texte[debut..])
        .into_iter()
        .filter(|e| noms.iter().any(|n| e.a_nom(n)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lecture_simple_et_entites() {
        let r = analyser_fragment("<a x=\"1 &amp; 2\"><b>Tom &amp; Jerry &#233;</b><c/></a>");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].attribut("X"), Some("1 & 2"));
        assert_eq!(r[0].texte_unique("B").as_deref(), Some("Tom & Jerry é"));
        assert!(r[0].enfant_unique("c").is_some());
    }

    #[test]
    fn lecture_tolerante() {
        // & non échappé, fermeture manquante, fermeture orpheline.
        let r = analyser_fragment("<movie><title>A & B</title><plot>x</oups><year>2001</movie>");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].texte_unique("title").as_deref(), Some("A & B"));
        // `year` n'a pas été refermé : il est refermé par `</movie>`.
        let plot = r[0].enfant_unique("plot").unwrap();
        assert_eq!(plot.texte_complet(), "x2001");
    }

    #[test]
    fn racine_dans_texte_bruite() {
        let t = "Release by X <b>bold</b>\n<?xml version=\"1.0\"?>\n<movie><title>T</title></movie>\nhttp://www.imdb.com/title/tt0133093/";
        let r = racines_nommees(t, &["movie"]);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].texte_unique("title").as_deref(), Some("T"));
    }

    #[test]
    fn decodage() {
        assert_eq!(decoder_octets(&[0xEF, 0xBB, 0xBF, b'a']), "a");
        assert_eq!(decoder_octets(&[0xFF, 0xFE, b'a', 0, 0xE9, 0]), "aé");
        assert_eq!(decoder_octets(&[b'c', b'a', b'f', 0xE9, 0x80]), "café€");
        assert_eq!(decoder_octets(&[b'a', 0, b'b', 0]), "ab");
    }

    #[test]
    fn serialisation() {
        let mut e = Element::nouveau("r");
        e.ajouter_texte("t", "a < b & c");
        e.ajouter(Element::nouveau("vide").attr("k", "\"v\""));
        let mut mixte = Element::nouveau("m");
        mixte.enfants.push(Noeud::Texte("x".into()));
        mixte.ajouter_texte("i", "y");
        e.ajouter(mixte);
        let mut s = String::new();
        e.serialiser(0, &mut s);
        assert_eq!(
            s,
            "<r>\n  <t>a &lt; b &amp; c</t>\n  <vide k=\"&quot;v&quot;\"/>\n  <m>x<i>y</i></m>\n</r>\n"
        );
    }
}
