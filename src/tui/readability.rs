//! Deterministic prose readability metrics — lexical diversity and sentence
//! rhythm — for the `inkhaven style` report (1.8.1). A companion to the
//! span-based [`style_warnings`](crate::tui::style_warnings) detectors: where
//! those flag individual spots, this measures the whole text quantitatively.
//!
//! Multilingual by construction: word and sentence segmentation are
//! Unicode-aware, so the numbers are meaningful for Russian, French, German, or
//! any script — no English-only assumptions. Pure, offline, zero AI.

use std::collections::HashSet;

use unicode_segmentation::UnicodeSegmentation;

/// Accumulates readability statistics across many paragraphs; call [`add`] for
/// each and [`finish`] once at the end.
///
/// [`add`]: Readability::add
/// [`finish`]: Readability::finish
#[derive(Default)]
pub struct Readability {
    tokens: u64,
    types: HashSet<String>,
    sentence_lengths: Vec<u32>,
}

/// The finished per-scope readability summary.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadabilityReport {
    /// Total word tokens.
    pub tokens: u64,
    /// Distinct word types (case-folded).
    pub types: u64,
    /// Type–token ratio — lexical diversity, 0..1. Length-sensitive, so most
    /// useful compared across texts of similar size.
    pub ttr: f64,
    /// Sentence count.
    pub sentences: u64,
    /// Mean words per sentence.
    pub mean_sentence_len: f64,
    /// Standard deviation of sentence length — the *rhythm*. A very low value
    /// relative to the mean is monotone prose.
    pub stdev_sentence_len: f64,
    /// The longest single sentence, in words.
    pub longest_sentence: u32,
}

impl ReadabilityReport {
    /// Coefficient of variation of sentence length (stdev / mean) — a
    /// scale-free measure of rhythm. Near 0 is metronomic; healthy narrative
    /// prose is typically 0.5 or higher. `0.0` when there is nothing to measure.
    pub fn rhythm_cv(&self) -> f64 {
        if self.mean_sentence_len > 0.0 {
            self.stdev_sentence_len / self.mean_sentence_len
        } else {
            0.0
        }
    }
}

impl Readability {
    /// Fold one paragraph (or any run of text) into the accumulator.
    pub fn add(&mut self, text: &str) {
        for w in text.unicode_words() {
            self.tokens += 1;
            self.types.insert(w.to_lowercase());
        }
        for sentence in split_sentences(text) {
            let n = sentence.unicode_words().count() as u32;
            if n > 0 {
                self.sentence_lengths.push(n);
            }
        }
    }

    /// Whether any prose was seen at all.
    pub fn is_empty(&self) -> bool {
        self.tokens == 0
    }

    /// Compute the summary.
    pub fn finish(&self) -> ReadabilityReport {
        let tokens = self.tokens;
        let types = self.types.len() as u64;
        let ttr = if tokens > 0 { types as f64 / tokens as f64 } else { 0.0 };

        let sentences = self.sentence_lengths.len() as u64;
        let (mean, stdev, longest) = if sentences > 0 {
            let sum: u64 = self.sentence_lengths.iter().map(|&x| u64::from(x)).sum();
            let mean = sum as f64 / sentences as f64;
            let var = self
                .sentence_lengths
                .iter()
                .map(|&x| {
                    let d = f64::from(x) - mean;
                    d * d
                })
                .sum::<f64>()
                / sentences as f64;
            (mean, var.sqrt(), self.sentence_lengths.iter().copied().max().unwrap_or(0))
        } else {
            (0.0, 0.0, 0)
        };

        ReadabilityReport {
            tokens,
            types,
            ttr,
            sentences,
            mean_sentence_len: mean,
            stdev_sentence_len: stdev,
            longest_sentence: longest,
        }
    }
}

/// Split text into sentences at terminal punctuation, Unicode-aware and
/// language-agnostic. `.`, `!`, `?`, `…` and their fullwidth CJK variants
/// terminate a sentence; a line break does too (headings, list items, verse
/// lines). Deliberately simple — a deterministic approximation, not a parser;
/// abbreviations may over-split, which is acceptable for an aggregate metric.
fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if matches!(ch, '.' | '!' | '?' | '…' | '\n' | '\r' | '。' | '！' | '？' | '‽') {
            if !cur.trim().is_empty() {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttr_counts_types_over_tokens_case_folded() {
        let mut r = Readability::default();
        r.add("The cat sat. The Cat ran.");
        let rep = r.finish();
        // tokens: the cat sat the cat ran = 6; types: the cat sat ran = 4.
        assert_eq!(rep.tokens, 6);
        assert_eq!(rep.types, 4);
        assert!((rep.ttr - 4.0 / 6.0).abs() < 1e-9);
    }

    #[test]
    fn sentence_lengths_and_rhythm() {
        let mut r = Readability::default();
        // Two sentences of 3 and 5 words.
        r.add("one two three. four five six seven eight!");
        let rep = r.finish();
        assert_eq!(rep.sentences, 2);
        assert!((rep.mean_sentence_len - 4.0).abs() < 1e-9);
        assert_eq!(rep.longest_sentence, 5);
        // stdev of {3,5} about mean 4 is 1.0; CV = 0.25.
        assert!((rep.stdev_sentence_len - 1.0).abs() < 1e-9);
        assert!((rep.rhythm_cv() - 0.25).abs() < 1e-9);
    }

    #[test]
    fn russian_prose_is_measured() {
        let mut r = Readability::default();
        // Cyrillic words and a Russian question mark; two sentences.
        r.add("Кот сидит на окне. Что он видит?");
        let rep = r.finish();
        assert_eq!(rep.sentences, 2);
        // Кот сидит на окне (4) · Что он видит (3) = 7 tokens, all distinct.
        assert_eq!(rep.tokens, 7);
        assert_eq!(rep.types, 7);
    }

    #[test]
    fn empty_text_is_safe() {
        let r = Readability::default();
        assert!(r.is_empty());
        let rep = r.finish();
        assert_eq!(rep.tokens, 0);
        assert_eq!(rep.ttr, 0.0);
        assert_eq!(rep.rhythm_cv(), 0.0);
    }
}
