//! 1.2.20+ C.1.b — live echo overlay for the editor.
//!
//! The `echo-repetition` doctor scan (C.1) flags a
//! distinctive word reused close together across a
//! chapter.  This is its live editor counterpart: while
//! the overlay is on (Ctrl+B Shift+K), every occurrence
//! in the *open* paragraph of a word that's echoing
//! around it is underlined as you write — the inline
//! companion to the Ctrl+B Shift+F style-warning overlay.
//!
//! Echo detection is chapter-wide, so the heavy half —
//! gathering the chapter's paragraphs and running
//! `crate::echo::detect_echoes` — lives in
//! `App::refresh_echo_overlay`, which caches the resulting
//! set of "stems echoing near the open paragraph".  This
//! type is the cheap per-line half that turns that stem
//! set into `StyleHit`s, mirroring `FilterWordsDetector`.

use std::collections::HashSet;

use rust_stemmers::Stemmer;
use unicode_segmentation::UnicodeSegmentation;

use super::style_warnings::{StyleHit, StyleWarningKind};
use crate::config::parse_stemmer_language;

/// Per-line echo underliner.  Built once per render frame
/// from the App's cached echoed-stem set.
pub struct EchoHighlighter {
    /// Already-normalised stem keys that are echoing near
    /// the open paragraph.
    targets: HashSet<String>,
    stemmer: Option<Stemmer>,
}

impl EchoHighlighter {
    /// `echoed_stems` are normalised stem keys produced by
    /// the chapter echo pass.  `language` selects the same
    /// Snowball stemmer so per-line matching folds `ё` and
    /// inflections identically (via `text::normalize_stem`).
    pub fn new(echoed_stems: &HashSet<String>, language: &str) -> Self {
        let stemmer = parse_stemmer_language(language).map(Stemmer::create);
        Self {
            targets: echoed_stems.clone(),
            stemmer,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Underline every word on `line` whose stem is in the
    /// echoed set.  Char-indexed columns so multi-byte
    /// text doesn't shift the highlight (mirrors
    /// `FilterWordsDetector::detect`).
    pub fn detect(&self, line: &str) -> Vec<StyleHit> {
        if self.targets.is_empty() || line.is_empty() {
            return Vec::new();
        }
        // byte → char column map for this row.
        let mut byte_to_char: Vec<usize> = Vec::with_capacity(line.len() + 1);
        let mut char_count = 0usize;
        for (b, _) in line.char_indices() {
            while byte_to_char.len() < b {
                byte_to_char.push(char_count);
            }
            byte_to_char.push(char_count);
            char_count += 1;
        }
        while byte_to_char.len() <= line.len() {
            byte_to_char.push(char_count);
        }

        let mut out = Vec::new();
        for (byte_start, word) in line.unicode_word_indices() {
            let stem = crate::text::normalize_stem(word, &self.stemmer);
            if !self.targets.contains(&stem) {
                continue;
            }
            let byte_end = byte_start + word.len();
            let col_start = byte_to_char[byte_start];
            let col_end =
                byte_to_char.get(byte_end).copied().unwrap_or(char_count);
            out.push(StyleHit {
                col_start,
                col_end,
                kind: StyleWarningKind::Echo,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(words: &[&str]) -> HashSet<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn underlines_echoed_stem_occurrences() {
        // "shimmer" stems to "shimmer"; both the bare form
        // and the inflection should underline.
        let h = EchoHighlighter::new(&set(&["shimmer"]), "english");
        let hits = h.detect("The shimmer and the shimmering light.");
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|x| x.kind == StyleWarningKind::Echo));
    }

    #[test]
    fn empty_target_set_is_noop() {
        let h = EchoHighlighter::new(&set(&[]), "english");
        assert!(h.is_empty());
        assert!(h.detect("anything at all").is_empty());
    }

    #[test]
    fn russian_yo_folds_for_matching() {
        // The target stem is produced with the russian
        // stemmer from the е-spelling; the ё form in the
        // buffer must still underline, because
        // text::normalize_stem folds ё→е before stemming.
        let ru = Some(Stemmer::create(rust_stemmers::Algorithm::Russian));
        let target = crate::text::normalize_stem("зеленый", &ru);
        let h = EchoHighlighter::new(&set(&[target.as_str()]), "russian");
        assert!(!h.detect("зелёный лес").is_empty());
    }
}
