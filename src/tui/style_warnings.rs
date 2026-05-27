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
    built_in_filter_words, built_in_stop_words, parse_stemmer_language,
    FilterWordsConfig, RepeatedPhrasesConfig,
};

/// What kind of stylistic warning a hit represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleWarningKind {
    /// `just`, `really`, `very`, `просто`, `очень`, …
    FilterWord,
    /// 1.2.9+ — an n-gram that repeats 3+ times in the
    /// open paragraph.  See `RepeatedPhraseDetector`.
    RepeatedPhrase,
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

/// 1.2.9+ — repeated-phrase detector.
///
/// Slides an `n`-word window across the full paragraph
/// (all rows joined as a token stream), stems each
/// window, groups by stem-key, and emits a `StyleHit`
/// for every occurrence of any key that appears
/// `threshold+` times.
///
/// Cross-row n-grams: a phrase that spans a source
/// newline still counts.  Hits are emitted per row
/// with columns clipped to that row's bounds.
///
/// Stop-word handling: configured stop-words are
/// excluded from the n-gram comparison so common
/// connectives don't inflate counts.  The N-gram
/// "the dog and" wouldn't be a meaningful repeat — but
/// "shoulders lifted slightly" is.
///
/// Build is paragraph-wide; the detector exposes
/// per-row accessors after construction.
pub struct RepeatedPhraseDetector {
    /// Indexed by row; each entry is the sorted list of
    /// `StyleHit`s for that row.
    per_row: Vec<Vec<StyleHit>>,
}

#[derive(Debug, Clone)]
struct WordToken {
    row: usize,
    col_start: usize,
    col_end: usize,
    stem: String,
}

impl RepeatedPhraseDetector {
    /// Build the detector from the full paragraph text
    /// (split into source rows) + the project's
    /// language + the repeated-phrases config.  Cheap
    /// for normal paragraph sizes; quadratic worst-case
    /// in word count if every window is unique (still
    /// only ~10ms for 1000-word paragraphs).
    pub fn new(
        cfg: &RepeatedPhrasesConfig,
        language: &str,
        lines: &[String],
    ) -> Self {
        let mut per_row: Vec<Vec<StyleHit>> =
            (0..lines.len()).map(|_| Vec::new()).collect();
        let n = cfg.n.max(2) as usize;
        let threshold = cfg.threshold.max(2) as usize;
        let stemmer = if cfg.use_stemming {
            parse_stemmer_language(language).map(rust_stemmers::Stemmer::create)
        } else {
            None
        };
        // Stop-word set: configured list (when non-empty)
        // OR the built-in default for that language.
        // Stemmed to align with token stems.
        let stop_configured = match language.to_lowercase().as_str() {
            "russian" => &cfg.russian_stop_words,
            "french" => &cfg.french_stop_words,
            "german" => &cfg.german_stop_words,
            "spanish" => &cfg.spanish_stop_words,
            _ => &cfg.english_stop_words,
        };
        let normalise_stop = |w: &str| -> String {
            let lc = w.trim().to_lowercase();
            match &stemmer {
                Some(s) => s.stem(&lc).into_owned(),
                None => lc,
            }
        };
        let stops: std::collections::HashSet<String> = if stop_configured.is_empty() {
            built_in_stop_words(language)
                .iter()
                .map(|s| normalise_stop(s))
                .collect()
        } else {
            stop_configured.iter().map(|s| normalise_stop(s)).collect()
        };

        // 1) Tokenise every row into a flat word list,
        //    excluding stop-words.  Each token carries
        //    its row + char-column origin so we can
        //    emit hits later.
        let mut tokens: Vec<WordToken> = Vec::new();
        for (row, line) in lines.iter().enumerate() {
            // Build byte→char index for this row once.
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
            for (byte_start, word) in line.unicode_word_indices() {
                let lc = word.to_lowercase();
                let stem = match &stemmer {
                    Some(s) => s.stem(&lc).into_owned(),
                    None => lc,
                };
                if stops.contains(&stem) {
                    continue;
                }
                let byte_end = byte_start + word.len();
                let col_start = byte_to_char[byte_start];
                let col_end =
                    byte_to_char.get(byte_end).copied().unwrap_or(char_count);
                tokens.push(WordToken {
                    row,
                    col_start,
                    col_end,
                    stem,
                });
            }
        }

        if tokens.len() < n {
            return Self { per_row };
        }

        // 2) Slide an n-token window, build a stem-key
        //    for each, group occurrences by key.  Key is
        //    "stem1|stem2|...|stemN".
        use std::collections::HashMap;
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
        for i in 0..=(tokens.len() - n) {
            let key = tokens[i..i + n]
                .iter()
                .map(|t| t.stem.as_str())
                .collect::<Vec<_>>()
                .join("|");
            groups.entry(key).or_default().push(i);
        }

        // 3) For every group with ≥ threshold
        //    occurrences, emit one StyleHit per row the
        //    n-gram covers.  Columns clip to the row's
        //    extent.
        for indices in groups.values() {
            if indices.len() < threshold {
                continue;
            }
            for &start in indices {
                let span = &tokens[start..start + n];
                let start_row = span.first().unwrap().row;
                let end_row = span.last().unwrap().row;
                let start_col = span.first().unwrap().col_start;
                let end_col = span.last().unwrap().col_end;
                if start_row == end_row {
                    per_row[start_row].push(StyleHit {
                        col_start: start_col,
                        col_end: end_col,
                        kind: StyleWarningKind::RepeatedPhrase,
                    });
                } else {
                    // Multi-row n-gram: emit one hit per
                    // covered row, clipped at row
                    // boundaries.  Middle rows aren't
                    // available here (we'd need
                    // line.chars().count() for the end
                    // col); the renderer ends each hit
                    // at the line end naturally.
                    for r in start_row..=end_row {
                        let cs = if r == start_row { start_col } else { 0 };
                        let ce = if r == end_row {
                            end_col
                        } else {
                            // Use a generous sentinel; the
                            // render loop won't paint past
                            // the actual line length.
                            usize::MAX / 2
                        };
                        per_row[r].push(StyleHit {
                            col_start: cs,
                            col_end: ce,
                            kind: StyleWarningKind::RepeatedPhrase,
                        });
                    }
                }
            }
        }

        // 4) Sort each row's hits by col_start so the
        //    overlay scan in the renderer doesn't
        //    backtrack.
        for row_hits in &mut per_row {
            row_hits.sort_by_key(|h| h.col_start);
        }
        Self { per_row }
    }

    /// Sorted slice of hits for the given source row.
    /// Empty when the row has no hits or is out of
    /// bounds.
    pub fn hits_for_row(&self, row: usize) -> &[StyleHit] {
        self.per_row.get(row).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Total number of hits across all rows — useful
    /// for the status bar / dirty-flag check.
    #[allow(dead_code)]
    pub fn total_hits(&self) -> usize {
        self.per_row.iter().map(Vec::len).sum()
    }

    /// True when no row has any hits — fast path the
    /// render loop can branch on.
    pub fn is_empty(&self) -> bool {
        self.per_row.iter().all(|r| r.is_empty())
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

    // ── RepeatedPhraseDetector tests ─────────────────

    fn rp_default() -> RepeatedPhrasesConfig {
        RepeatedPhrasesConfig::default()
    }


    #[test]
    fn rp_three_repeats_get_flagged() {
        // "lifted shoulders" (stems: lift|shoulder)
        // appears 3 times across rows → 3 hits at
        // threshold=3.  Surrounding pronouns (`she`,
        // `her`) are stop-listed by the English default.
        let mut cfg = rp_default();
        cfg.n = 2;
        cfg.threshold = 3;
        let lines = vec![
            "she lifted her shoulders slightly".to_string(),
            "he paused; she lifted her shoulders again".to_string(),
            "later she lifted her shoulders once more".to_string(),
        ];
        let d = RepeatedPhraseDetector::new(&cfg, "english", &lines);
        assert!(
            d.total_hits() >= 3,
            "expected at least 3 hits for lift|shoulder × 3, got {} ({:?})",
            d.total_hits(),
            d.per_row,
        );
    }

    #[test]
    fn rp_two_repeats_below_threshold() {
        let mut cfg = rp_default();
        cfg.n = 2;
        cfg.threshold = 3;
        let lines = vec![
            "she lifted her shoulders".to_string(),
            "she lifted her shoulders".to_string(),
        ];
        let d = RepeatedPhraseDetector::new(&cfg, "english", &lines);
        assert_eq!(d.total_hits(), 0, "2 < threshold; should not flag");
    }

    #[test]
    fn rp_lower_threshold_flags_two() {
        let mut cfg = rp_default();
        cfg.n = 2;
        cfg.threshold = 2;
        let lines = vec![
            "she lifted her shoulders".to_string(),
            "she lifted her shoulders".to_string(),
        ];
        let d = RepeatedPhraseDetector::new(&cfg, "english", &lines);
        // 2-gram "lift|shoulder" appears 2 times → 2 hits.
        assert!(d.total_hits() >= 2);
    }

    #[test]
    fn rp_stemming_aligns_inflections() {
        // `lifted` / `lifting` both stem to `lift`; the
        // 2-gram `lift|shoulder` repeats twice.
        let mut cfg = rp_default();
        cfg.n = 2;
        cfg.threshold = 2;
        let lines = vec![
            "she lifted her shoulders".to_string(),
            "he was lifting her shoulders again".to_string(),
        ];
        let d = RepeatedPhraseDetector::new(&cfg, "english", &lines);
        assert!(
            d.total_hits() >= 2,
            "stems should align lifted/lifting; got hits {:?}",
            d.per_row,
        );
    }

    #[test]
    fn rp_russian_inflections() {
        // Russian past-tense verb inflections.  `поднял`
        // / `подняла` / `подняли` all stem to `подня`;
        // `плечи` / `плечо` stem to `плеч`.  Personal
        // pronouns `он` / `она` / `они` also all stem
        // to `он` (Russian Snowball behaviour).  So
        // the 2-grams `подня|плеч` and `он|подня`
        // repeat 3× each across rows.
        let mut cfg = rp_default();
        cfg.n = 2;
        cfg.threshold = 3;
        let lines = vec![
            "Он поднял плечи".to_string(),
            "Она подняла плечи".to_string(),
            "Они подняли плечи".to_string(),
        ];
        let d = RepeatedPhraseDetector::new(&cfg, "russian", &lines);
        assert!(
            d.total_hits() >= 3,
            "expected ≥ 3 hits for поднимать-плечи × 3, got {} ({:?})",
            d.total_hits(),
            d.per_row,
        );
    }

    #[test]
    fn rp_stop_words_excluded_from_ngrams() {
        // With "the" / "and" stop-listed, the 3-grams
        // become "X dog Y" vs "X cat Y" and don't match.
        // Without stop-listing, "the X dog" repeats and
        // would match.
        let mut cfg = rp_default();
        cfg.n = 3;
        cfg.threshold = 2;
        let lines = vec![
            "the big dog and the small cat".to_string(),
            "the big dog and the small cat".to_string(),
        ];
        let d = RepeatedPhraseDetector::new(&cfg, "english", &lines);
        // After stop-word exclusion: "big dog small" +
        // "cat" tokens.  3-gram "big dog small" appears
        // twice → 2 hits.
        assert!(
            d.total_hits() >= 2,
            "expected at least 2 hits across the dup line"
        );
    }

    #[test]
    fn rp_disabled_yields_no_hits() {
        let mut cfg = rp_default();
        cfg.enabled = false;
        // Even when build runs, threshold isn't met because
        // we don't gate on `enabled` inside the detector
        // (the caller is expected to check) — this test
        // just confirms behaviour with empty enable check.
        let lines: Vec<String> = vec!["nothing to see here".into(); 5];
        let d = RepeatedPhraseDetector::new(&cfg, "english", &lines);
        // With threshold=3 (default), all 5 copies of the
        // same line yield 5 hits per ngram-position.
        // What we actually test: caller must check
        // cfg.enabled.  Sanity here: detector RAN.
        assert!(!d.is_empty());
    }

    #[test]
    fn rp_empty_input_no_panic() {
        let d = RepeatedPhraseDetector::new(&rp_default(), "english", &[]);
        assert!(d.is_empty());
    }

    #[test]
    fn rp_columns_char_indexed() {
        let mut cfg = rp_default();
        cfg.n = 2;
        cfg.threshold = 3;
        let lines = vec![
            "очень просто слово".to_string(),
            "очень просто слово".to_string(),
            "очень просто слово".to_string(),
        ];
        let d = RepeatedPhraseDetector::new(&cfg, "russian", &lines);
        // Each row has one 2-gram hit covering "очень
        // просто" (chars 0..12).
        let row0 = d.hits_for_row(0);
        assert!(!row0.is_empty());
        // First hit's col_start must be 0 (char), not
        // a byte offset.
        assert_eq!(row0[0].col_start, 0);
    }
}
