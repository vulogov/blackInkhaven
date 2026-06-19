//! Stress placement (LANG-1 P1.4).
//!
//! Most natural languages have one dominant stress rule (Finnish initial,
//! French final, Polish penultimate, Latin weight-sensitive) plus lexical
//! idiosyncrasies. P1.4 models the dominant rule deterministically from
//! syllable structure; lexical exceptions + secondary (alternating) stress
//! are a later refinement.

use serde::Deserialize;

/// Where the primary stress lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StressPlacement {
    /// First syllable.
    Initial,
    /// Last syllable.
    Final,
    /// Second-to-last.
    Penultimate,
    /// Third-to-last.
    Antepenultimate,
    /// The Latin Stress Rule: the penult when it is heavy (closed or with a
    /// branching nucleus), otherwise the antepenult.
    LatinRule,
}

impl StressPlacement {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "initial" | "first" => Some(Self::Initial),
            "final" | "last" | "ultimate" => Some(Self::Final),
            "penultimate" | "penult" => Some(Self::Penultimate),
            "antepenultimate" | "antepenult" => Some(Self::Antepenultimate),
            "latin" | "weight_sensitive" | "quantity_sensitive" => Some(Self::LatinRule),
            _ => None,
        }
    }
}

/// The language's stress rule. Currently a single primary placement; the
/// struct leaves room for secondary stress + lexical exceptions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StressRule {
    pub primary: StressPlacement,
}

/// Accept either the shorthand `stress: "penultimate"` or the explicit
/// `stress: { primary: "penultimate" }`.
#[derive(Deserialize)]
#[serde(untagged)]
enum RawStress {
    Short(String),
    Full { primary: String },
}

impl<'de> Deserialize<'de> for StressRule {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let name = match RawStress::deserialize(d)? {
            RawStress::Short(s) => s,
            RawStress::Full { primary } => primary,
        };
        let primary = StressPlacement::parse(&name).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "unknown stress placement `{name}` (initial | final | penultimate | \
                 antepenultimate | latin)"
            ))
        })?;
        Ok(StressRule { primary })
    }
}
