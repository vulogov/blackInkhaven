//! LANG-3 P1.2 — synthetic corpus generation (RFC Amendment A1).
//!
//! The bridge from the rule-based spine to the retrieval datastore: run the RBMT
//! over a pool of English sentences and keep the ones the language can translate
//! *cleanly* (every content word resolved). The surviving `(English → conlang)`
//! pairs seed the [`super::memory::TranslationMemory`], so that after seeding,
//! common sentences hit the memory directly instead of being re-derived.
//!
//! Because the RBMT is deterministic and per-sentence, the only quality gate that
//! matters for a first cut is **lexicon coverage** — a sentence with an
//! untranslatable word is rejected, and the rejects are diagnostic: they tell the
//! author exactly which words the language is still missing. The acceptance rate
//! is itself a reading of how mature the lexicon is.
//!
//! Pure and deterministic. The bundled English pool ships in the binary
//! ([`BUNDLED_POOL`]); a larger pool can be supplied as a file.

use std::collections::BTreeMap;

use crate::conlang::types::morphology::Morphology;
use crate::conlang::Phonology;
use crate::language_entry::DictionaryEntry;

/// The bundled English source pool (one declarative sentence per line; `#`
/// comments and blank lines ignored). Curated from core vocabulary so a young
/// conlang already covers some of it.
pub const BUNDLED_POOL: &str = include_str!("../../../assets/conlang/english-pool-v1.txt");

/// A sentence the language could not fully translate, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected {
    pub english: String,
    /// The English words with no lexicon entry.
    pub unresolved: Vec<String>,
}

/// The outcome of a corpus-generation pass.
#[derive(Debug, Clone, Default)]
pub struct CorpusReport {
    /// How many non-blank sentences were translated.
    pub scanned: usize,
    /// `(english, conlang)` pairs that passed the coverage gate.
    pub accepted: Vec<(String, String)>,
    /// Sentences rejected for missing vocabulary (diagnostic).
    pub rejected: Vec<Rejected>,
}

impl CorpusReport {
    /// The fraction of scanned sentences that were accepted (`0.0..=1.0`).
    pub fn acceptance_rate(&self) -> f32 {
        if self.scanned == 0 {
            0.0
        } else {
            self.accepted.len() as f32 / self.scanned as f32
        }
    }

    /// The English words that most often blocked a translation, most-frequent
    /// first — what to add to the lexicon to raise coverage.
    pub fn top_missing(&self, n: usize) -> Vec<(String, usize)> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for r in &self.rejected {
            for w in &r.unresolved {
                *counts.entry(w.clone()).or_default() += 1;
            }
        }
        let mut v: Vec<(String, usize)> = counts.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(n);
        v
    }
}

/// Parse a pool into sentences, dropping `#` comments and blank lines.
pub fn parse_pool(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Run the RBMT over `pool` and partition into accepted pairs and rejects. A
/// sentence is accepted when every content word resolves (no `«…»`) and a
/// non-empty target is produced.
pub fn generate(
    phon: &Phonology,
    morph: &Morphology,
    typology: &BTreeMap<String, String>,
    entries: &[DictionaryEntry],
    pool: &[String],
) -> CorpusReport {
    let mut report = CorpusReport::default();
    for en in pool {
        let en = en.trim();
        if en.is_empty() {
            continue;
        }
        report.scanned += 1;
        let t = super::translate(phon, morph, typology, entries, en);
        if t.unresolved.is_empty() && !t.target.trim().is_empty() {
            report.accepted.push((en.to_string(), t.target));
        } else {
            report.rejected.push(Rejected { english: en.to_string(), unresolved: t.unresolved });
        }
    }
    report
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
    fn bundled_pool_parses() {
        let pool = parse_pool(BUNDLED_POOL);
        assert!(pool.len() > 50, "pool should have many sentences, got {}", pool.len());
        assert!(pool.iter().all(|s| !s.starts_with('#') && !s.is_empty()));
    }

    #[test]
    fn accepts_covered_rejects_uncovered() {
        let phon = Phonology::default();
        let morph = Morphology::default();
        let mut typ = BTreeMap::new();
        typ.insert("word_order".to_string(), "svo".to_string());
        let entries = vec![
            entry("kira", "noun", "bird"),
            entry("nami", "verb", "to see"),
            entry("pata", "noun", "stone"),
        ];
        let pool = vec![
            "the bird sees the stone".to_string(), // fully covered
            "the dragon sees the stone".to_string(), // dragon missing
        ];
        let r = generate(&phon, &morph, &typ, &entries, &pool);
        assert_eq!(r.scanned, 2);
        assert_eq!(r.accepted, vec![("the bird sees the stone".to_string(), "kira nami pata".to_string())]);
        assert_eq!(r.rejected.len(), 1);
        assert_eq!(r.acceptance_rate(), 0.5);
        assert_eq!(r.top_missing(3), vec![("dragon".to_string(), 1)]);
    }
}
