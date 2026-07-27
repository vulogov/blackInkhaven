//! POEM-PHON (1.8.25) — an optional English pronouncing dictionary (CMUdict
//! format), consulted by the scansion engines to make English metre and rhyme
//! *exact* where spelling alone is only approximate. English-only: Russian,
//! German, and Spanish are near-phonemic and already accurate, so they never use
//! this.
//!
//! Inert until a dictionary is imported to `<data_dir>/inkhaven/phonemes/en.dict`
//! (`inkhaven poetry phonemes import <cmudict-file>`). Words not in the
//! dictionary fall back to the spelling heuristic, so the engines degrade
//! gracefully. No external-binary dependency — a data file plus a lookup.
//!
//! CMUdict lines look like `COMPARE  K AH0 M P EH1 R`: a headword, then ARPABET
//! phonemes where every vowel carries a stress digit (0 none · 1 primary · 2
//! secondary). We distil each entry to the three things prosody needs: the
//! syllable count (= vowel phonemes), the primary-stressed syllable, and the
//! rhyme tail (phonemes from that stressed vowel to the end).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

/// One word's pronunciation, reduced to what the poetry engines need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pron {
    /// Syllable count = the number of vowel phonemes. Exact.
    pub syllables: usize,
    /// 0-based index (among syllables) of the primary-stressed syllable.
    pub stress: usize,
    /// The rhyme tail: phonemes from the primary-stressed vowel to the end, with
    /// stress digits stripped, space-joined (e.g. `love` → `AH V`, `move` → `UW V`).
    /// Two words rhyme perfectly iff their rhyme tails are equal.
    pub rhyme_key: String,
}

/// A parsed pronouncing dictionary: normalized headword → pronunciation.
#[derive(Debug, Clone, Default)]
pub struct PhonemeDict {
    map: HashMap<String, Pron>,
}

impl PhonemeDict {
    pub fn get(&self, word: &str) -> Option<&Pron> {
        self.map.get(&normalize(word))
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Case-fold + strip surrounding punctuation for headword matching (an
/// apostrophe stays — CMUdict has `DON'T`, `IT'S`).
fn normalize(word: &str) -> String {
    word.trim()
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
        .to_lowercase()
}

/// An ARPABET vowel phoneme carries a trailing stress digit (0/1/2); consonants
/// do not. That digit is the vowel marker.
fn is_vowel_phoneme(p: &str) -> bool {
    p.chars().last().is_some_and(|c| c.is_ascii_digit())
}

/// Parse one CMUdict line into `(headword, Pron)`. Skips comments (`;;;`), blank
/// lines, and consonant-only tokens; strips a `(2)` variant marker so only a
/// word's first pronunciation is kept by the caller.
fn parse_line(line: &str) -> Option<(String, Pron)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(";;;") {
        return None;
    }
    let mut parts = line.split_whitespace();
    let raw_word = parts.next()?;
    let word = raw_word.split('(').next()?.to_lowercase();
    if word.is_empty() {
        return None;
    }
    let phonemes: Vec<&str> = parts.collect();
    let vowels: Vec<usize> = phonemes
        .iter()
        .enumerate()
        .filter(|(_, p)| is_vowel_phoneme(p))
        .map(|(i, _)| i)
        .collect();
    if vowels.is_empty() {
        return None;
    }
    let syllables = vowels.len();
    // Primary stress = the vowel ending in `1`; else the first vowel.
    let stress = vowels
        .iter()
        .position(|&i| phonemes[i].ends_with('1'))
        .unwrap_or(0);
    let rhyme_key = phonemes[vowels[stress]..]
        .iter()
        .map(|p| p.trim_end_matches(|c: char| c.is_ascii_digit()))
        .collect::<Vec<_>>()
        .join(" ");
    Some((normalize(&word), Pron { syllables, stress, rhyme_key }))
}

/// Parse a whole CMUdict-format text. The first pronunciation of a headword wins
/// (CMUdict lists the most common first, variants as `WORD(2)`).
pub fn parse(text: &str) -> PhonemeDict {
    let mut map = HashMap::new();
    for line in text.lines() {
        if let Some((w, p)) = parse_line(line) {
            map.entry(w).or_insert(p);
        }
    }
    PhonemeDict { map }
}

/// `<data_dir>/inkhaven/phonemes/` — shared across projects, beside the WordNet
/// indexes.
pub fn data_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("dev", "inkhaven", "inkhaven")
        .map(|d| d.data_dir().join("phonemes"))
}

/// The installed English dictionary's path.
pub fn en_path() -> Option<PathBuf> {
    data_dir().map(|d| d.join("en.dict"))
}

static EN: OnceLock<Option<PhonemeDict>> = OnceLock::new();

/// The English pronouncing dictionary, loaded once from disk if installed. `None`
/// when no dictionary has been imported — callers then use the spelling heuristic.
pub fn en() -> Option<&'static PhonemeDict> {
    EN.get_or_init(|| {
        let text = std::fs::read_to_string(en_path()?).ok()?;
        let dict = parse(&text);
        if dict.is_empty() { None } else { Some(dict) }
    })
    .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_syllables_stress_and_rhyme_key() {
        // The example that the whole feature exists to get right.
        let d = parse(
            ";;; a comment\n\
             LOVE  L AH1 V\n\
             MOVE  M UW1 V\n\
             COMPARE  K AH0 M P EH1 R\n\
             MOTHER  M AH1 DH ER0\n\
             MOVE(2)  M UW1 V ER0\n",
        );
        // syllable count = vowel phonemes; stress = the `1` vowel.
        assert_eq!(d.get("compare").unwrap().syllables, 2);
        assert_eq!(d.get("compare").unwrap().stress, 1); // com-PARE
        assert_eq!(d.get("mother").unwrap().syllables, 2);
        assert_eq!(d.get("mother").unwrap().stress, 0); // MOTH-er

        // love / move: same spelling tail, DIFFERENT rhyme key — the eye-rhyme fix.
        assert_eq!(d.get("love").unwrap().rhyme_key, "AH V");
        assert_eq!(d.get("move").unwrap().rhyme_key, "UW V");
        assert_ne!(d.get("love").unwrap().rhyme_key, d.get("move").unwrap().rhyme_key);

        // First pronunciation wins; the `(2)` variant is ignored.
        assert_eq!(d.get("move").unwrap().syllables, 1);

        // Punctuation-insensitive lookup; a miss is None.
        assert!(d.get("Love,").is_some());
        assert!(d.get("nonexistent").is_none());
    }

    #[test]
    fn consonant_only_and_junk_lines_are_skipped() {
        let d = parse("HMM  HH M\n\n   \nBAD");
        assert!(d.is_empty()); // "HMM" has no vowel phoneme; "BAD" has no phonemes
    }
}
