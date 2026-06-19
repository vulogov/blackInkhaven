//! IPA sonority reference (LANG-1 P1.2).
//!
//! Syllabification and the Sonority Sequencing Principle need a sonority
//! rank per phoneme. This table classifies the common IPA symbols on the
//! standard ascending scale (obstruents low, vowels high). It is
//! intentionally sonority-focused — the fuller distinctive-feature
//! decomposition that allophony needs (place / manner / voice) arrives with
//! the allophony engine in P1.3.
//!
//! A phoneme may override its rank with an explicit `sonority` field; an
//! unlisted symbol falls back to its coarse kind (vowel high, consonant
//! low) so an invented sound still syllabifies sensibly.

use crate::conlang::types::{PhonemeKind, Phonology};

/// Ascending sonority ranks. Higher = more sonorous (closer to a syllable
/// peak).
pub const STOP: u8 = 1;
pub const AFFRICATE: u8 = 2;
pub const FRICATIVE: u8 = 3;
pub const NASAL: u8 = 4;
pub const LIQUID: u8 = 5;
pub const GLIDE: u8 = 6;
pub const VOWEL: u8 = 7;

/// `(IPA symbol, sonority rank)`, longest-symbol-first within each manner so
/// multi-char affricates are matched before their stop component elsewhere.
const TABLE: &[(&str, u8)] = &[
    // affricates (multi-char first)
    ("t͡ʃ", AFFRICATE), ("d͡ʒ", AFFRICATE), ("tʃ", AFFRICATE), ("dʒ", AFFRICATE),
    ("ts", AFFRICATE), ("dz", AFFRICATE),
    // stops
    ("p", STOP), ("b", STOP), ("t", STOP), ("d", STOP), ("k", STOP), ("g", STOP),
    ("q", STOP), ("ʔ", STOP), ("c", STOP), ("ɟ", STOP), ("ʈ", STOP), ("ɖ", STOP),
    // fricatives
    ("f", FRICATIVE), ("v", FRICATIVE), ("θ", FRICATIVE), ("ð", FRICATIVE),
    ("s", FRICATIVE), ("z", FRICATIVE), ("ʃ", FRICATIVE), ("ʒ", FRICATIVE),
    ("x", FRICATIVE), ("ɣ", FRICATIVE), ("h", FRICATIVE), ("ç", FRICATIVE),
    ("ħ", FRICATIVE), ("ʕ", FRICATIVE), ("ɸ", FRICATIVE), ("β", FRICATIVE),
    // nasals
    ("m", NASAL), ("n", NASAL), ("ŋ", NASAL), ("ɲ", NASAL), ("ɳ", NASAL), ("ɱ", NASAL),
    // liquids
    ("l", LIQUID), ("r", LIQUID), ("ɾ", LIQUID), ("ʀ", LIQUID), ("ʁ", LIQUID),
    ("ɭ", LIQUID), ("ɽ", LIQUID), ("ʎ", LIQUID),
    // glides / approximants
    ("j", GLIDE), ("w", GLIDE), ("ɥ", GLIDE), ("ʋ", GLIDE), ("ɰ", GLIDE),
    // vowels
    ("a", VOWEL), ("e", VOWEL), ("i", VOWEL), ("o", VOWEL), ("u", VOWEL),
    ("ɛ", VOWEL), ("ɔ", VOWEL), ("ɪ", VOWEL), ("ʊ", VOWEL), ("ə", VOWEL),
    ("æ", VOWEL), ("ɑ", VOWEL), ("y", VOWEL), ("ø", VOWEL), ("œ", VOWEL),
    ("ɨ", VOWEL), ("ʉ", VOWEL), ("ɯ", VOWEL),
];

/// Look up the bare-symbol sonority of an IPA string, ignoring any
/// inventory context.
fn table_lookup(ipa: &str) -> Option<u8> {
    TABLE.iter().find(|(sym, _)| *sym == ipa).map(|(_, r)| *r)
}

/// Sonority rank of a phoneme in this language: an explicit `sonority`
/// override wins, then the IPA table, then a kind-based fallback (vowel
/// high, consonant low) so invented sounds still place sensibly.
pub fn sonority_of(phon: &Phonology, ipa: &str) -> u8 {
    if let Some(p) = phon.phoneme(ipa) {
        if let Some(s) = p.sonority {
            return s;
        }
        if let Some(s) = table_lookup(ipa) {
            return s;
        }
        return match p.kind {
            PhonemeKind::Vowel => VOWEL,
            PhonemeKind::Consonant => STOP,
        };
    }
    table_lookup(ipa).unwrap_or(STOP)
}
