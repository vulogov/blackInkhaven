//! Inline style-warning detectors.
//!
//! The shared infrastructure for "highlight stylistically
//! weak prose in the editor".  1.2.9 ships filter-word
//! detection (`just`, `really`, `very`, …); future
//! detectors (repeated-phrase, show-don't-tell regex
//! fallback, weak-verb constructs) will land in this
//! module and emit the same `StyleHit` shape so the
//! render pipeline doesn't grow per-feature plumbing.
//!
//! Multilingual story:
//!
//!   * Per-language filter-word lists live in HJSON
//!     under `editor.style_warnings.filter_words.<lang>`.
//!     Empty list = "use built-in default for this
//!     language"; non-empty = "replace the default".
//!     `extra_words` always adds on top.
//!   * Snowball stemming via `rust-stemmers`.  Each
//!     entry stems once at detector-init time; each
//!     editor word stems at scan time; matches happen
//!     on stems.  So `seem` in the list catches
//!     `seemed` / `seems` / `seeming`; `казаться`
//!     catches `казался / казалась / казалось /
//!     казались`.  Disable via
//!     `filter_words.use_stemming = false` for exact-
//!     lowercased match.
//!   * Tokenisation uses `unicode-segmentation`'s
//!     `unicode_word_indices()` — UAX-#29-compliant.
//!     Cyrillic, Latin, Greek, Devanagari word
//!     boundaries all work.

use std::collections::HashSet;

use rust_stemmers::Stemmer;
use unicode_segmentation::UnicodeSegmentation;

use crate::config::{
    built_in_filter_words, parse_stemmer_language, FilterWordsConfig,
};

/// What kind of stylistic warning a hit represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleWarningKind {
    /// `just`, `really`, `very`, `просто`, `очень`, …
    FilterWord,
}

/// One stylistic-warning hit on a row of editor text.
/// Char-indexed (not byte-indexed) so multi-byte text
/// (Cyrillic, em-dash, smart quotes) doesn't shift the
/// highlight columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleHit {
    pub col_start: usize,
    pub col_end: usize,
    pub kind: StyleWarningKind,
}

/// Per-paragraph precompiled detector.  Built once per
/// editor render frame from the project config; reused
/// across every line in the buffer.  Cheap to build
/// (~100 hash inserts + 5-50 stemmer calls).
pub struct FilterWordsDetector {
    targets: HashSet<String>,
    stemmer: Option<Stemmer>,
}

impl FilterWordsDetector {
    /// Compose a detector from the project config.
    /// Picks the language-specific list (configured or
    /// built-in fallback), unions in `extra_words`, and
    /// stems each entry once.  If the language doesn't
    /// have a Snowball algorithm or `use_stemming` is
    /// off, falls back to exact-lowercased match.
    pub fn new(cfg: &FilterWordsConfig, language: &str) -> Self {
        let configured: &Vec<String> = match language.to_lowercase().as_str() {
            "russian" => &cfg.russian,
            "french" => &cfg.french,
            "german" => &cfg.german,
            "spanish" => &cfg.spanish,
            _ => &cfg.english,
        };
        // Either the user's list (when non-empty) OR the
        // built-in default — same precedence the
        // `effective_filter_words` helper documents.
        let stemmer = if cfg.use_stemming {
            parse_stemmer_language(language).map(Stemmer::create)
        } else {
            None
        };
        let normalise = |w: &str| -> String {
            let lc = w.trim().to_lowercase();
            match &stemmer {
                Some(s) => s.stem(&lc).into_owned(),
                None => lc,
            }
        };
        let mut targets: HashSet<String> = HashSet::new();
        if configured.is_empty() {
            for w in built_in_filter_words(language) {
                let key = normalise(w);
                if !key.is_empty() {
                    targets.insert(key);
                }
            }
        } else {
            for w in configured {
                let key = normalise(w);
                if !key.is_empty() {
                    targets.insert(key);
                }
            }
        }
        for w in &cfg.extra_words {
            let key = normalise(w);
            if !key.is_empty() {
                targets.insert(key);
            }
        }
        Self { targets, stemmer }
    }

    /// True when there's nothing to match — caller can
    /// short-circuit before per-line scanning.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Walk `line` and return every filter-word hit at
    /// this row's character columns.  Stems each
    /// editor word once via the same stemmer used at
    /// detector init.
    pub fn detect(&self, line: &str) -> Vec<StyleHit> {
        if self.targets.is_empty() || line.is_empty() {
            return Vec::new();
        }
        // Byte → char map so multi-byte chars don't
        // shift columns.  Pre-built per line.
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
            let lc = word.to_lowercase();
            let key = match &self.stemmer {
                Some(s) => s.stem(&lc).into_owned(),
                None => lc,
            };
            if !self.targets.contains(&key) {
                continue;
            }
            let byte_end = byte_start + word.len();
            let col_start = byte_to_char[byte_start];
            let col_end = byte_to_char.get(byte_end).copied().unwrap_or(char_count);
            out.push(StyleHit {
                col_start,
                col_end,
                kind: StyleWarningKind::FilterWord,
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_default() -> FilterWordsConfig {
        FilterWordsConfig::default()
    }

    fn cols_of(hits: &[StyleHit]) -> Vec<(usize, usize)> {
        hits.iter().map(|h| (h.col_start, h.col_end)).collect()
    }

    #[test]
    fn english_filter_word_basic() {
        let d = FilterWordsDetector::new(&cfg_default(), "english");
        let hits = d.detect("I just wanted to see");
        assert_eq!(cols_of(&hits), vec![(2, 6)]);
    }

    #[test]
    fn english_case_insensitive() {
        let d = FilterWordsDetector::new(&cfg_default(), "english");
        let hits = d.detect("Just wait. JUST a moment.");
        assert_eq!(cols_of(&hits), vec![(0, 4), (11, 15)]);
    }

    #[test]
    fn russian_basic() {
        let d = FilterWordsDetector::new(&cfg_default(), "russian");
        let hits = d.detect("Он был очень устал и просто хотел спать.");
        // очень + просто → 2 hits
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn russian_stemming_catches_inflections() {
        // "казалось" (it seemed) → stems to same lemma as
        // "казался" / "казалась" / "казались".  The
        // built-in Russian list has "казаться" (infinitive);
        // all four forms should match via Snowball.
        let d = FilterWordsDetector::new(&cfg_default(), "russian");
        for form in &["казалось", "казался", "казалась", "казались"] {
            let hits = d.detect(form);
            assert!(
                !hits.is_empty(),
                "Russian stemmer failed to match `{form}`: cfg had `казаться`"
            );
        }
    }

    #[test]
    fn english_stemming_catches_inflections() {
        // "seem" is in the list; stemmer catches "seemed",
        // "seems", "seeming".
        let d = FilterWordsDetector::new(&cfg_default(), "english");
        for form in &["seem", "seemed", "seems", "seeming"] {
            let hits = d.detect(form);
            assert!(
                !hits.is_empty(),
                "English stemmer failed to match `{form}`"
            );
        }
    }

    #[test]
    fn use_stemming_off_disables_inflection_matching() {
        let mut cfg = cfg_default();
        cfg.use_stemming = false;
        let d = FilterWordsDetector::new(&cfg, "english");
        // `seem` IS in the list verbatim → matches.
        assert!(!d.detect("seem").is_empty());
        // `seemed` is NOT in the list verbatim → no match
        // without stemming.
        assert!(d.detect("seemed").is_empty());
    }

    #[test]
    fn user_override_replaces_default_for_that_language() {
        let mut cfg = cfg_default();
        cfg.english = vec!["foo".into(), "bar".into()];
        let d = FilterWordsDetector::new(&cfg, "english");
        // "just" was in the default but the user replaced
        // the English list — so "just" shouldn't match.
        assert!(d.detect("just a test").is_empty());
        // "foo" + "bar" should match.
        assert_eq!(d.detect("foo bar baz").len(), 2);
    }

    #[test]
    fn extra_words_add_on_top_of_default() {
        let mut cfg = cfg_default();
        cfg.extra_words = vec!["foo".into()];
        let d = FilterWordsDetector::new(&cfg, "english");
        // Default ("just") + extra ("foo") = 2 hits.
        assert_eq!(d.detect("just foo here").len(), 2);
    }

    #[test]
    fn unknown_language_falls_back_to_english() {
        let d = FilterWordsDetector::new(&cfg_default(), "klingon");
        assert_eq!(d.detect("just a test").len(), 1);
    }

    #[test]
    fn cyrillic_columns_are_char_indexed_not_byte() {
        let d = FilterWordsDetector::new(&cfg_default(), "russian");
        let hits = d.detect("очень устал");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].col_start, 0);
        assert_eq!(hits[0].col_end, 5);
    }

    #[test]
    fn no_partial_word_match() {
        let d = FilterWordsDetector::new(&cfg_default(), "english");
        // "justice" must NOT be flagged because of "just"
        // — the Snowball English stemmer reduces
        // "justice" to "justic", not "just", so the
        // match doesn't fire.
        let hits = d.detect("justice is essential");
        assert!(
            !hits.iter().any(|h| h.col_start == 0 && h.col_end == 7),
            "false positive on `justice`: {hits:?}"
        );
    }

    #[test]
    fn punctuation_doesnt_break_match() {
        let d = FilterWordsDetector::new(&cfg_default(), "english");
        let hits = d.detect("And just.");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].col_start, 4);
        assert_eq!(hits[0].col_end, 8);
    }
}
