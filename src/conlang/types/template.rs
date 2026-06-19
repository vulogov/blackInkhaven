//! Syllable / morpheme templates (LANG-1 P1.1).
//!
//! A template is a pattern over phoneme *classes* — the skeleton a word is
//! built from. P1.1 supports required (`C`) and optional (`(C)`) class
//! slots; literal phonemes, bounded repeats, and alternation arrive in a
//! later P1 increment.

use serde::{Deserialize, Serialize};

/// Where a template applies in word formation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemplateRole {
    Root,
    Prefix,
    Suffix,
    Infix,
    Circumfix,
    Compound,
}

impl TemplateRole {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "root" => Some(Self::Root),
            "prefix" => Some(Self::Prefix),
            "suffix" => Some(Self::Suffix),
            "infix" => Some(Self::Infix),
            "circumfix" => Some(Self::Circumfix),
            "compound" => Some(Self::Compound),
            _ => None,
        }
    }

    /// The lowercase key used in the HJSON `templates` map.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Prefix => "prefix",
            Self::Suffix => "suffix",
            Self::Infix => "infix",
            Self::Circumfix => "circumfix",
            Self::Compound => "compound",
        }
    }
}

/// One atom of a template pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateAtom {
    /// A required slot filled from the named class.
    Class(String),
    /// An optional slot — filled ~half the time from the named class.
    OptionalClass(String),
}

impl TemplateAtom {
    /// Parse one atom from the compact DSL: `C` (required class) or `(C)`
    /// (optional class). The class name is whatever sits between the
    /// optional parens.
    pub fn parse(token: &str) -> Option<Self> {
        let t = token.trim();
        if t.is_empty() {
            return None;
        }
        if let Some(inner) = t.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
            let inner = inner.trim();
            if inner.is_empty() {
                return None;
            }
            Some(Self::OptionalClass(inner.to_string()))
        } else {
            Some(Self::Class(t.to_string()))
        }
    }

    pub fn class_name(&self) -> &str {
        match self {
            Self::Class(c) | Self::OptionalClass(c) => c,
        }
    }

    pub fn is_optional(&self) -> bool {
        matches!(self, Self::OptionalClass(_))
    }
}

/// A syllable / morpheme template plus its sampling weight.
///
/// Deserialized from either form:
///   `{ pattern: "C V (C)", weight: 2.0 }`
///   `{ pattern: ["C", "V", "(C)"] }`
#[derive(Debug, Clone, Deserialize)]
pub struct SyllableTemplate {
    #[serde(deserialize_with = "de_atoms")]
    pub pattern: Vec<TemplateAtom>,
    #[serde(default = "default_weight")]
    pub weight: f32,
}

fn default_weight() -> f32 {
    1.0
}

/// Accept the pattern as a whitespace-separated string *or* a list of
/// tokens — whichever the author finds clearer.
fn de_atoms<'de, D>(d: D) -> Result<Vec<TemplateAtom>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Repr {
        Str(String),
        List(Vec<String>),
    }
    let tokens: Vec<String> = match Repr::deserialize(d)? {
        Repr::Str(s) => s.split_whitespace().map(String::from).collect(),
        Repr::List(v) => v,
    };
    tokens
        .iter()
        .map(|t| {
            TemplateAtom::parse(t)
                .ok_or_else(|| serde::de::Error::custom(format!("bad template atom `{t}`")))
        })
        .collect()
}
