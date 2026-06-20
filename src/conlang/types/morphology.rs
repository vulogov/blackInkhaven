//! Morphology (LANG-1 P3.1).
//!
//! Morpheme inventory + paradigm templates, reconstructed from the typed
//! HJSON block in the language's `Morphology` chapter. A paradigm template
//! lists cells (feature bundles) and the morpheme sequence each cell applies
//! to a root; the generator (`morphology::paradigm`) realizes them into
//! surface forms, running the P1.3 allophony engine across the affix
//! boundaries. P3.1 covers prefix + suffix affixes; infix / circumfix /
//! processes (ablaut, reduplication) arrive in later P3 increments.

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AffixPosition {
    Prefix,
    Suffix,
    Infix,
    Circumfix,
}

impl AffixPosition {
    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "prefix" => Some(Self::Prefix),
            "suffix" => Some(Self::Suffix),
            "infix" => Some(Self::Infix),
            "circumfix" => Some(Self::Circumfix),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for AffixPosition {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(d)?;
        AffixPosition::parse(&s).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "unknown affix position `{s}` (prefix | suffix | infix | circumfix)"
            ))
        })
    }
}

/// One morpheme: a glossable affix with a form and a position.
#[derive(Debug, Clone, Deserialize)]
pub struct MorphemeSpec {
    /// Reference id used by paradigm cells.
    pub id: String,
    /// Leipzig-style gloss tag (`PL`, `PST`, `DAT`).
    #[serde(default)]
    pub gloss: String,
    /// The affix's written form (`i`, `ne`, `ge`).
    #[serde(default)]
    pub form: String,
    pub position: AffixPosition,
    /// Grammatical category (`number`, `tense`, `case`) — parsed now; the
    /// grammar questionnaire + book consume it in a later increment.
    #[serde(default)]
    #[allow(dead_code)]
    pub category: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub value: String,
    /// How close this affix sits to the root when several affixes of the same
    /// side stack: `0` = any position (the declared order is kept), `1` =
    /// immediately next to the root, `2` = the next slot out, and so on. A
    /// lower non-zero value is closer to the root; `0` affixes drift outermost.
    #[serde(default)]
    pub precedence: u8,
}

/// One cell of a paradigm: a feature bundle + the morphemes (by id) it
/// applies to the root.
#[derive(Debug, Clone, Deserialize)]
pub struct ParadigmCell {
    #[serde(default)]
    pub features: BTreeMap<String, String>,
    #[serde(default)]
    pub morphemes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParadigmTemplate {
    pub name: String,
    #[serde(default)]
    pub cells: Vec<ParadigmCell>,
}

/// A *derivational* rule — applies an affix to a root to coin a new lexeme
/// (an agent noun, a verbal noun, …), as opposed to an inflectional paradigm
/// cell (a grammatical form of the same lexeme).
#[derive(Debug, Clone, Deserialize)]
pub struct DerivationRule {
    pub name: String,
    /// Gloss tag for the derived sense (`AGENT`, `DIM`), used when no
    /// `gloss_template` is given.
    #[serde(default)]
    pub gloss: String,
    /// The affix form.
    #[serde(default)]
    pub form: String,
    pub position: AffixPosition,
    /// Applies only to roots of this part of speech (`None` = any).
    #[serde(default)]
    pub from_pos: Option<String>,
    /// Part of speech of the derived lexeme.
    #[serde(default)]
    pub to_pos: String,
    /// Optional gloss template; `{}` is replaced by the root's gloss
    /// (`"one who {}s"`, `"little {}"`).
    #[serde(default)]
    pub gloss_template: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Morphology {
    /// Typological type (`agglutinative` / `fusional` / …); informational.
    #[serde(default)]
    #[allow(dead_code)]
    pub kind: String,
    #[serde(default)]
    pub morphemes: Vec<MorphemeSpec>,
    #[serde(default)]
    pub paradigms: Vec<ParadigmTemplate>,
    /// Derivational rules (P3.3).
    #[serde(default)]
    pub derivations: Vec<DerivationRule>,
}

impl Morphology {
    /// Parse from a `Morphology`-chapter paragraph body (pure HJSON or a
    /// fenced ```` ```hjson ```` block, like the phonology / dictionary).
    pub fn from_hjson(body: &str) -> Result<Option<Self>, String> {
        if body.trim().is_empty() {
            return Ok(None);
        }
        let block = crate::language_entry::extract_hjson_block(body).unwrap_or(body);
        serde_hjson::from_str::<Self>(block)
            .map(Some)
            .map_err(|e| format!("morphology HJSON parse failed: {e}"))
    }

    pub fn morpheme(&self, id: &str) -> Option<&MorphemeSpec> {
        self.morphemes.iter().find(|m| m.id == id)
    }

    pub fn paradigm(&self, name: &str) -> Option<&ParadigmTemplate> {
        self.paradigms.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    }
}
