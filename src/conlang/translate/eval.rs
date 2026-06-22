//! LANG-3 P2 — the evaluation harness (RFC §8.8).
//!
//! A conlang has no human reference translations, so translation quality is
//! gauged *without* one, from project data alone:
//!
//! - **Round-trip semantic similarity** (§8.8.1) — translate English → conlang,
//!   then [`reverse`](super::reverse) it back; the recovered English should mean
//!   the same as the source. The similarity is an embedding cosine, computed by
//!   the caller (the app layer owns the `fastembed` model); this module produces
//!   the `(source, recovered)` pairs to compare.
//! - **Coverage** (the RBMT analogue of §8.8.2's grammar pass-rate) — the
//!   fraction of a test set the lexicon can translate with no untranslatable
//!   word. RBMT output is grammatical by construction, so coverage is the honest
//!   measure of how far the language reaches.
//!
//! Author-acceptance (§8.8.3) is inherently editor-coupled and lands with the
//! translation pane.
//!
//! Round-trip measures the **rule-based** engine (no translation memory — the
//! memory would echo confirmed pairs and score a trivial 1.0), so it reflects
//! the language itself. Pure and deterministic.

use std::collections::BTreeMap;

use crate::conlang::types::morphology::Morphology;
use crate::conlang::Phonology;
use crate::language_entry::DictionaryEntry;

/// One round-trip: a source sentence, its conlang translation, and the English
/// recovered by reversing that translation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundTrip {
    pub english: String,
    pub conlang: String,
    pub recovered: String,
    /// Whether the forward translation resolved every word (no `«…»`). Round-trip
    /// similarity is only meaningful for covered sentences.
    pub covered: bool,
}

/// Translate `english` with the rule-based engine, then reverse the result.
pub fn round_trip(
    phon: &Phonology,
    morph: &Morphology,
    typology: &BTreeMap<String, String>,
    entries: &[DictionaryEntry],
    english: &str,
) -> RoundTrip {
    let fwd = super::translate(phon, morph, typology, entries, english);
    let covered = fwd.unresolved.is_empty() && !fwd.target.trim().is_empty();
    let rev = super::reverse::reverse(phon, morph, typology, entries, &fwd.target);
    RoundTrip {
        english: english.to_string(),
        conlang: fwd.target,
        recovered: rev.english,
        covered,
    }
}

/// Round-trip every sentence in `test_set`.
pub fn round_trip_all(
    phon: &Phonology,
    morph: &Morphology,
    typology: &BTreeMap<String, String>,
    entries: &[DictionaryEntry],
    test_set: &[String],
) -> Vec<RoundTrip> {
    test_set
        .iter()
        .filter(|s| !s.trim().is_empty())
        .map(|s| round_trip(phon, morph, typology, entries, s.trim()))
        .collect()
}

/// The fraction of `items` whose forward translation was fully covered.
pub fn coverage(items: &[RoundTrip]) -> f32 {
    if items.is_empty() {
        return 0.0;
    }
    items.iter().filter(|i| i.covered).count() as f32 / items.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(word: &str, pos: &str, translation: &str) -> DictionaryEntry {
        DictionaryEntry {
            word: word.into(),
            pos: pos.into(),
            translation: translation.into(),
            ..Default::default()
        }
    }

    #[test]
    fn round_trip_recovers_a_covered_sentence() {
        let phon = Phonology::default();
        let morph = Morphology::default();
        let mut typ = BTreeMap::new();
        typ.insert("word_order".to_string(), "svo".to_string());
        let entries = vec![
            entry("kira", "noun", "bird"),
            entry("nami", "verb", "to see"),
            entry("pata", "noun", "stone"),
        ];
        let rt = round_trip(&phon, &morph, &typ, &entries, "the bird sees the stone");
        assert!(rt.covered);
        assert_eq!(rt.conlang, "kira nami pata");
        // Round-trips back to the source meaning (3sg agreement on the verb).
        assert_eq!(rt.recovered, "the bird sees the stone");
    }

    #[test]
    fn uncovered_sentence_is_flagged() {
        let phon = Phonology::default();
        let morph = Morphology::default();
        let mut typ = BTreeMap::new();
        typ.insert("word_order".to_string(), "svo".to_string());
        let entries = vec![entry("kira", "noun", "bird"), entry("nami", "verb", "to see")];
        let items =
            round_trip_all(&phon, &morph, &typ, &entries, &["the bird sees the dragon".to_string()]);
        assert!(!items[0].covered); // "dragon" unresolved
        assert_eq!(coverage(&items), 0.0);
    }
}
