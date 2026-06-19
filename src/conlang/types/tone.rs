//! Tone systems (LANG-1 P1.6).
//!
//! A tone language marks each syllable with a tone (a register level like
//! H/L, a contour like rising/falling, or a pitch accent). Lexicon entries
//! carry per-syllable tones; **tone sandhi** rewrites the tone string in
//! context (Mandarin's classic third-tone sandhi: a low-dipping tone before
//! another becomes rising — `3 > 2 / _ 3`). Sandhi reuses the same ordered
//! context-rewrite engine as allophony, applied to tone labels instead of
//! phonemes — so a `ToneSandhiRule` *is* an [`AllophonyRule`].
//!
//! The evaluator's input — per-syllable lexical tone — comes from the
//! lexicon (P2). The sandhi rules and the engine ship here so the system is
//! complete and testable with an explicit tone sequence.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::AllophonyRule;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToneKind {
    Register,
    Contour,
    PitchAccent,
}

impl ToneKind {
    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "register" | "level" => Some(Self::Register),
            "contour" => Some(Self::Contour),
            "pitch_accent" | "pitch-accent" | "accent" => Some(Self::PitchAccent),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for ToneKind {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(d)?;
        ToneKind::parse(&s).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "unknown tone kind `{s}` (register | contour | pitch_accent)"
            ))
        })
    }
}

/// A tone system: its kind, the inventory of tone labels, optional tone
/// classes (for sandhi contexts like `T` = any tone), and the ordered
/// sandhi rules.
#[derive(Debug, Clone, Deserialize)]
pub struct ToneSystem {
    /// Classification (informational now; consumed by tone validation + the
    /// grammar book in a later increment).
    #[allow(dead_code)]
    pub kind: ToneKind,
    /// Tone labels, e.g. `["H", "L"]` or Mandarin `["1", "2", "3", "4"]`.
    /// Parsed now; the inventory drives tone validation later.
    #[serde(default)]
    #[allow(dead_code)]
    pub tones: Vec<String>,
    /// Named tone classes for sandhi contexts (default empty → literal
    /// tone matching).
    #[serde(default)]
    pub classes: BTreeMap<String, Vec<String>>,
    /// Ordered tone-sandhi rules (same notation as allophony, over tones).
    #[serde(default)]
    pub sandhi: Vec<AllophonyRule>,
}
