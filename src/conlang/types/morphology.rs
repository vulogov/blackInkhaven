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

use super::AllophonyRule;

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

/// A non-concatenative morphological *process* — one that changes the stem
/// itself rather than gluing an affix to its edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MorphProcess {
    /// Internal sound change (e.g. a vowel swap: `sing` → `sang`), expressed as
    /// SPE `rules` applied to the stem.
    Ablaut,
    /// Copying part (or all) of the stem; the `reduplicate` mode says how much.
    Reduplication,
}

impl<'de> Deserialize<'de> for MorphProcess {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(d)?;
        match s.trim().to_ascii_lowercase().as_str() {
            "ablaut" | "apophony" => Ok(Self::Ablaut),
            "reduplication" | "reduplicate" | "reduplicative" => Ok(Self::Reduplication),
            other => Err(serde::de::Error::custom(format!(
                "unknown morphological process `{other}` (ablaut | reduplication)"
            ))),
        }
    }
}

/// One morpheme. Most are *concatenative* affixes (a `form` glued at a
/// `position`: prefix / suffix / infix / circumfix). A morpheme may instead be
/// a non-concatenative `process` (ablaut or reduplication) that reshapes the
/// stem.
#[derive(Debug, Clone, Deserialize)]
pub struct MorphemeSpec {
    /// Reference id used by paradigm cells.
    pub id: String,
    /// Leipzig-style gloss tag (`PL`, `PST`, `DAT`).
    #[serde(default)]
    pub gloss: String,
    /// The affix's written form (`i`, `ne`, `ge`). For a circumfix, a `_` marks
    /// where the stem goes (`ge_t` → `ge` + stem + `t`).
    #[serde(default)]
    pub form: String,
    /// Where a concatenative affix attaches. Absent for a `process` morpheme.
    #[serde(default)]
    pub position: Option<AffixPosition>,
    /// Grammatical category (`number`, `tense`, `case`) — used to group affixes
    /// in the reference grammar.
    #[serde(default)]
    pub category: String,
    /// The value within the category (`plural`, `past`, `dative`).
    #[serde(default)]
    pub value: String,
    /// How close this affix sits to the root when several affixes of the same
    /// side stack: `0` = any position (the declared order is kept), `1` =
    /// immediately next to the root, `2` = the next slot out, and so on. A
    /// lower non-zero value is closer to the root; `0` affixes drift outermost.
    #[serde(default)]
    pub precedence: u8,
    /// A non-concatenative process instead of an affix (`ablaut` /
    /// `reduplication`).
    #[serde(default)]
    pub process: Option<MorphProcess>,
    /// SPE rules applied to the stem for an `ablaut` process (`i > a`).
    #[serde(default)]
    pub rules: Vec<AllophonyRule>,
    /// The reduplication mode: `full` | `initial_cv` | `initial_syllable` |
    /// `final_syllable`.
    #[serde(default)]
    pub reduplicate: Option<String>,
    /// Where an *infix* lands inside the stem: `before_first_vowel` (the
    /// default — i.e. after the first consonant) or `after_first_vowel`.
    #[serde(default)]
    pub anchor: Option<String>,
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

/// An *agreement* (concord) rule: a `dependent` word copies grammatical
/// `features` from the `head` it modifies, and realises them through a named
/// `paradigm`. E.g. an adjective agrees with its noun in number and case; a
/// verb agrees with its subject in person and number.
#[derive(Debug, Clone, Deserialize)]
pub struct AgreementRule {
    /// The part of speech that agrees (`adjective`, `verb`, `determiner`).
    pub dependent: String,
    /// What it agrees with (`noun`, `subject`); informational, for the grammar.
    #[serde(default)]
    pub head: String,
    /// The features copied from the head (`number`, `case`, `gender`, `person`).
    #[serde(default)]
    pub features: Vec<String>,
    /// The paradigm used to realise the dependent's agreeing form.
    pub paradigm: String,
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
    /// Agreement / concord rules (P3.x).
    #[serde(default)]
    pub agreement: Vec<AgreementRule>,
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

    /// The agreement rule for a dependent part of speech, if any.
    pub fn agreement_for(&self, dependent: &str) -> Option<&AgreementRule> {
        self.agreement.iter().find(|a| a.dependent.eq_ignore_ascii_case(dependent))
    }
}
