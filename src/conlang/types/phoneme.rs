//! Phoneme inventory types (LANG-1 P1.1).
//!
//! A phoneme is the smallest sound unit of the language. P1.1 keeps the
//! model deliberately small — IPA + an optional romanization + a coarse
//! vowel/consonant kind — which is everything the deterministic word
//! generator and phonotactic validator need. The full distinctive-feature
//! decomposition (place / manner / height …) arrives with the allophony
//! engine in P1.2.

use serde::{Deserialize, Serialize};

/// Coarse sound class used by cluster / sonority constraints. Finer
/// articulatory features land in P1.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PhonemeKind {
    Vowel,
    Consonant,
}

// serde_hjson (an older serde_json fork) can't deserialize an
// externally-tagged enum from a bare string, so accept the kind as a
// plain string by hand.
impl<'de> Deserialize<'de> for PhonemeKind {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(d)?;
        match s.trim().to_ascii_lowercase().as_str() {
            "vowel" | "v" => Ok(Self::Vowel),
            "consonant" | "c" => Ok(Self::Consonant),
            other => Err(serde::de::Error::custom(format!(
                "unknown phoneme kind `{other}` (expected vowel | consonant)"
            ))),
        }
    }
}

/// One phoneme in a language's inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Phoneme {
    /// Canonical IPA representation — the inventory key that class lists
    /// and templates reference.
    pub ipa: String,
    /// Optional written form used when rendering a generated word. When
    /// absent, the IPA itself is rendered.
    #[serde(default)]
    pub romanize: Option<String>,
    pub kind: PhonemeKind,
    /// Optional sonority-rank override (1 = least sonorous … 7 = vowel).
    /// When absent, the rank is read from the IPA table, then a kind-based
    /// fallback. See `phonology::ipa`. (P1.2)
    #[serde(default)]
    pub sonority: Option<u8>,
}

impl Phoneme {
    /// The grapheme used to render this phoneme in a generated word: the
    /// romanization when set, otherwise the raw IPA.
    pub fn grapheme(&self) -> &str {
        self.romanize.as_deref().unwrap_or(&self.ipa)
    }
}
