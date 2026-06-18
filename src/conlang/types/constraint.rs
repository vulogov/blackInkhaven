//! Phonotactic constraints (LANG-1 P1.1).
//!
//! These are the *deterministic* legality rules a generated word must
//! satisfy. P1.1 covers the constraints that need no syllabification — a
//! single linear pass over the phoneme sequence decides them. Onset / coda
//! restrictions and sonority sequencing (which need syllable boundaries)
//! arrive alongside the syllabifier in a later P1 increment.

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhonotacticConstraint {
    /// No run of more than `n` consecutive consonants anywhere in the word.
    MaxClusterSize(usize),
    /// No two identical adjacent phonemes (e.g. forbids `tt`, `aa`).
    NoGeminate,
    /// Forbid phoneme `a` immediately followed by phoneme `b` (by IPA).
    ForbidBigram(String, String),
}

/// Wire form: `{ kind: "max_cluster_size", value: 2 }`,
/// `{ kind: "no_geminate" }`, `{ kind: "forbid_bigram", a: "s", b: "r" }`.
#[derive(Deserialize)]
struct RawConstraint {
    kind: String,
    #[serde(default)]
    value: Option<usize>,
    #[serde(default)]
    a: Option<String>,
    #[serde(default)]
    b: Option<String>,
}

impl TryFrom<RawConstraint> for PhonotacticConstraint {
    type Error = String;

    fn try_from(r: RawConstraint) -> Result<Self, Self::Error> {
        match r.kind.trim().to_ascii_lowercase().as_str() {
            "max_cluster_size" | "max_cluster" => {
                let n = r.value.ok_or("max_cluster_size needs `value`")?;
                Ok(Self::MaxClusterSize(n))
            }
            "no_geminate" | "no_geminates" => Ok(Self::NoGeminate),
            "forbid_bigram" => {
                let a = r.a.ok_or("forbid_bigram needs `a`")?;
                let b = r.b.ok_or("forbid_bigram needs `b`")?;
                Ok(Self::ForbidBigram(a, b))
            }
            other => Err(format!("unknown constraint kind `{other}`")),
        }
    }
}

impl<'de> Deserialize<'de> for PhonotacticConstraint {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawConstraint::deserialize(d)?;
        PhonotacticConstraint::try_from(raw).map_err(serde::de::Error::custom)
    }
}
