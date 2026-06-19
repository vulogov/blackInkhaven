//! Romanization schemes (LANG-1 P1.5).
//!
//! A named, bidirectional mapping between the language's IPA phonemes and a
//! written form. A language may carry several schemes (a "tolkien"
//! transcription, a strict-ISO one, …); one is the default. The forward
//! direction (IPA → text) is unambiguous; the reverse (text → IPA) can be
//! ambiguous when two phonemes share a grapheme, so a scheme may add
//! single-segment *contextual* rules to disambiguate (the classic "c is /s/
//! before a front vowel, else /k/").
//!
//! This generalizes the per-phoneme `romanize` field (P1.1), which remains
//! the implicit default scheme used by `Phonology::segment`.

use serde::Deserialize;

use super::PatternAtom;

/// One IPA ↔ grapheme correspondence.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Mapping {
    pub ipa: String,
    pub roman: String,
}

/// Disambiguates a grapheme that decodes to more than one phoneme, by the
/// phoneme immediately before (`after`) and/or after (`before`) it. A
/// context atom is a class name (when declared) or a literal phoneme, or `#`
/// for a word edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextualRule {
    pub roman: String,
    pub ipa: String,
    pub before: Option<PatternAtom>,
    pub after: Option<PatternAtom>,
}

#[derive(Deserialize)]
struct RawContextual {
    roman: String,
    ipa: String,
    #[serde(default)]
    before: Option<String>,
    #[serde(default)]
    after: Option<String>,
}

fn atom(s: Option<String>) -> Option<PatternAtom> {
    let t = s?;
    let t = t.trim();
    if t.is_empty() {
        None
    } else if t == "#" {
        Some(PatternAtom::Boundary)
    } else {
        Some(PatternAtom::Symbol(t.to_string()))
    }
}

impl<'de> Deserialize<'de> for ContextualRule {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let r = RawContextual::deserialize(d)?;
        Ok(ContextualRule {
            roman: r.roman,
            ipa: r.ipa,
            before: atom(r.before),
            after: atom(r.after),
        })
    }
}

/// A named bidirectional romanization scheme.
#[derive(Debug, Clone, Deserialize)]
pub struct RomanizationScheme {
    pub name: String,
    #[serde(default)]
    pub mappings: Vec<Mapping>,
    #[serde(default)]
    pub contextual: Vec<ContextualRule>,
}
