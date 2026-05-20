//! Place/Character name lexicon for editor highlighting.
//!
//! After every save (and on project open) we walk the Places and Characters
//! system books, collect every paragraph title nested under them, and stem
//! each title's words using the Snowball algorithms configured in
//! `editor.stemming.languages`. The editor renderer then overlays cyan
//! (Places) and yellow (Characters) on each match.
//!
//! Match semantics:
//! * **Word-sequence over stems.** Each name is split into Unicode words,
//!   each word is stemmed with every configured language, and matching is
//!   done by sliding over the buffer's words. So "Москва" with the Russian
//!   stemmer also lights up "Москве", "Москвою", etc.
//! * **Case-insensitive** at the stemming layer (we lowercase before
//!   stemming on both sides).
//! * **Phrase** — multi-word titles ("King's Landing", "Серая Гавань")
//!   match as a contiguous sequence of word-tokens; punctuation between
//!   them is allowed.
//! * **Longest-first** — when two names overlap at the same start position
//!   (one being a prefix sequence of another), the longer wins.
//! * **No stemming → exact lowercase comparison.** If `languages` is empty
//!   the lexicon is still useful, just without inflection support.
//!
//! Performance: at literary scale (a few hundred names, lines well under
//! 1k chars), per-line `row_hits` is fine. Stems are precomputed once at
//! `build()` time.

use std::collections::HashMap;

use rust_stemmers::{Algorithm, Stemmer};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;

/// Highlight category for a lexicon hit. Drives both the editor
/// overlay style and the per-category status messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexCategory {
    Place,
    Character,
    Note,
    Artefact,
}

/// One match on a row of editor text, in character (not byte) coordinates.
#[derive(Debug, Clone, Copy)]
pub struct LexHit {
    pub col_start: usize,
    pub col_end: usize,
    pub category: LexCategory,
}

/// A single name's compiled form: the lowercased stem sequence we'll compare
/// against the editor buffer's stem sequence. `stems_per_word` outer index
/// is "which word in the name", inner Vec is "all stem candidates for that
/// word across configured languages" (lowercased originals included so a
/// name with zero matching stemmers still works).
#[derive(Debug)]
struct CompiledName {
    /// One entry per word in the name. Each entry is the set of acceptable
    /// stems for that word position (any inner stem matching the buffer
    /// word's stem set counts as a hit).
    stems_per_word: Vec<Vec<String>>,
    category: LexCategory,
}

#[derive(Default, Debug)]
pub struct Lexicon {
    names: Vec<CompiledName>,
    /// Snowball algorithms used to expand both sides of the match.
    algos: Vec<Algorithm>,
}

impl Lexicon {
    /// Walk the hierarchy, collect paragraph titles under each
    /// supplied book, and stem them with every configured Snowball
    /// algorithm. `books` is a list of `(book_id, category)` pairs;
    /// duplicate titles across categories are deduplicated case-
    /// insensitively but the FIRST entry's category wins, so put the
    /// higher-priority category first.
    pub fn build(
        hierarchy: &Hierarchy,
        books: &[(Uuid, LexCategory)],
        algorithms: Vec<Algorithm>,
    ) -> Self {
        let mut names: Vec<CompiledName> = Vec::new();
        let mut seen: HashMap<String, ()> = HashMap::new();
        for (book_id, category) in books {
            collect_names_into(
                hierarchy,
                *book_id,
                *category,
                &algorithms,
                &mut names,
                &mut seen,
            );
        }
        // Longer phrases first so we prefer "King's Landing" over "King".
        names.sort_by(|a, b| b.stems_per_word.len().cmp(&a.stems_per_word.len()));
        Self {
            names,
            algos: algorithms,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// All hits on a single editor row. Returns non-overlapping hits in
    /// left-to-right order; on overlap, the longer phrase wins.
    pub fn row_hits(&self, line: &str) -> Vec<LexHit> {
        if self.is_empty() {
            return Vec::new();
        }
        let tokens = tokenize_with_offsets(line);
        if tokens.is_empty() {
            return Vec::new();
        }
        // Pre-stem every buffer word once.
        let token_stems: Vec<Vec<String>> = tokens
            .iter()
            .map(|t| stems_for(&t.text, &self.algos))
            .collect();

        let mut out: Vec<LexHit> = Vec::new();
        let mut i = 0usize;
        while i < tokens.len() {
            // Pick the longest name whose stems match starting at i.
            let mut matched: Option<(usize, LexCategory)> = None; // (len_in_tokens, cat)
            for name in &self.names {
                let n = name.stems_per_word.len();
                if i + n > tokens.len() {
                    continue;
                }
                let mut ok = true;
                for k in 0..n {
                    let accept = &name.stems_per_word[k];
                    let cand = &token_stems[i + k];
                    if !cand.iter().any(|s| accept.iter().any(|a| a == s)) {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    if matched.map_or(true, |(prev_n, _)| n > prev_n) {
                        matched = Some((n, name.category));
                    }
                }
            }
            if let Some((n, category)) = matched {
                let start_char = tokens[i].char_start;
                let end_char = tokens[i + n - 1].char_end;
                out.push(LexHit {
                    col_start: start_char,
                    col_end: end_char,
                    category,
                });
                i += n; // skip past the matched span so overlaps don't duplicate.
            } else {
                i += 1;
            }
        }
        out
    }
}

fn collect_names_into(
    hierarchy: &Hierarchy,
    root: Uuid,
    category: LexCategory,
    algos: &[Algorithm],
    out: &mut Vec<CompiledName>,
    seen: &mut HashMap<String, ()>,
) {
    for id in hierarchy.collect_subtree(root) {
        if id == root {
            continue;
        }
        let Some(node) = hierarchy.get(id) else {
            continue;
        };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let trimmed = node.title.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_lowercase();
        if seen.insert(key, ()).is_some() {
            continue;
        }
        let words = tokenize_words(trimmed);
        if words.is_empty() {
            continue;
        }
        let stems_per_word: Vec<Vec<String>> = words
            .iter()
            .map(|w| stems_for(w, algos))
            .collect();
        out.push(CompiledName {
            stems_per_word,
            category,
        });
    }
}

/// Lowercased word tokens of `s`, with no positional info. Uses Unicode
/// word segmentation so Cyrillic, CJK punctuation, etc. all behave.
fn tokenize_words(s: &str) -> Vec<String> {
    s.unicode_words()
        .map(|w| w.to_lowercase())
        .filter(|w| !w.is_empty())
        .collect()
}

#[derive(Debug, Clone)]
struct Token {
    text: String,
    /// Character offset (not byte) where this token begins in the source.
    char_start: usize,
    /// Character offset (not byte) one past the end.
    char_end: usize,
}

/// Same as `tokenize_words` but retains source positions. Positions are in
/// CHARACTER coordinates so they line up with the editor's char-based
/// cursor and span builder.
fn tokenize_with_offsets(s: &str) -> Vec<Token> {
    let mut out = Vec::new();
    // unicode-segmentation gives byte indices; convert each to char index
    // by precomputing the cumulative char count up to each byte boundary.
    // We keep a running counter as we iterate the source.
    let mut byte_to_char: Vec<usize> = Vec::with_capacity(s.len() + 1);
    {
        let mut c = 0usize;
        for (b, _) in s.char_indices() {
            // Pad entries between prior byte and this byte (multi-byte char)
            while byte_to_char.len() < b {
                byte_to_char.push(c);
            }
            byte_to_char.push(c);
            c += 1;
        }
        // Final sentinel for the end-of-string boundary.
        while byte_to_char.len() <= s.len() {
            byte_to_char.push(c);
        }
    }
    for (b, w) in s.unicode_word_indices() {
        let start = byte_to_char[b];
        let end = byte_to_char[b + w.len()];
        let text = w.to_lowercase();
        if !text.is_empty() {
            out.push(Token {
                text,
                char_start: start,
                char_end: end,
            });
        }
    }
    out
}

/// All acceptable stems for `word` across every configured algorithm. The
/// lowercased original is always included so an empty algorithm list still
/// matches the exact word.
fn stems_for(word: &str, algos: &[Algorithm]) -> Vec<String> {
    let lc = word.to_lowercase();
    let mut out: Vec<String> = Vec::with_capacity(1 + algos.len());
    out.push(lc.clone());
    for a in algos {
        let s = Stemmer::create(*a).stem(&lc).into_owned();
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_stemmers::Algorithm;

    /// Test-only convenience: build a Lexicon directly from name lists
    /// instead of walking a real Hierarchy.
    fn build_test_lex(
        places: &[&str],
        characters: &[&str],
        algos: Vec<Algorithm>,
    ) -> Lexicon {
        build_test_lex_cats(&[
            (LexCategory::Place, places),
            (LexCategory::Character, characters),
        ], algos)
    }

    fn build_test_lex_cats(
        groups: &[(LexCategory, &[&str])],
        algos: Vec<Algorithm>,
    ) -> Lexicon {
        let mut names: Vec<CompiledName> = Vec::new();
        for (cat, list) in groups {
            for n in *list {
                let words = tokenize_words(n);
                if words.is_empty() {
                    continue;
                }
                names.push(CompiledName {
                    stems_per_word: words.iter().map(|w| stems_for(w, &algos)).collect(),
                    category: *cat,
                });
            }
        }
        names.sort_by(|a, b| b.stems_per_word.len().cmp(&a.stems_per_word.len()));
        Lexicon { names, algos }
    }

    #[test]
    fn notes_and_artefacts_hit_with_distinct_categories() {
        let lex = build_test_lex_cats(
            &[
                (LexCategory::Note, &["dragonglass"]),
                (LexCategory::Artefact, &["valyrian steel"]),
            ],
            vec![Algorithm::English],
        );
        let hits = lex.row_hits("She wielded the Valyrian Steel against the Dragonglass.");
        let cats: Vec<LexCategory> = hits.iter().map(|h| h.category).collect();
        assert!(cats.contains(&LexCategory::Note), "got: {hits:?}");
        assert!(cats.contains(&LexCategory::Artefact), "got: {hits:?}");
    }

    #[test]
    fn english_inflections_match() {
        // "city" → "citi", "cities" → "citi" → both stem identically.
        let lex = build_test_lex(&["cities"], &[], vec![Algorithm::English]);
        let hits = lex.row_hits("She walked through the city at dawn.");
        assert_eq!(hits.len(), 1, "expected one hit, got {hits:?}");
    }

    #[test]
    fn russian_inflections_of_moscow() {
        let lex = build_test_lex(&[], &["Москва"], vec![Algorithm::Russian]);
        let line = "Из Москвы в Москве и снова в Москвою.";
        let hits = lex.row_hits(line);
        assert!(
            hits.len() >= 2,
            "expected at least two Москва forms, got {hits:?}"
        );
        for h in &hits {
            assert_eq!(h.category, LexCategory::Character);
        }
    }

    #[test]
    fn whole_word_via_segmentation() {
        // "Tomorrow" must not match "Tom".
        let lex = build_test_lex(&[], &["Tom"], vec![Algorithm::English]);
        let hits = lex.row_hits("Tomorrow is another day.");
        assert!(hits.is_empty(), "got unexpected hits: {hits:?}");
        let hits2 = lex.row_hits("Then Tom arrived.");
        assert_eq!(hits2.len(), 1);
    }

    #[test]
    fn multi_word_phrase() {
        let lex = build_test_lex(&["King's Landing"], &[], vec![Algorithm::English]);
        let line = "She returned to King's Landing alone.";
        let hits = lex.row_hits(line);
        assert_eq!(hits.len(), 1);
        let matched: String = line
            .chars()
            .skip(hits[0].col_start)
            .take(hits[0].col_end - hits[0].col_start)
            .collect();
        assert!(matched.contains("King") && matched.contains("Landing"));
    }

    #[test]
    fn longest_match_wins() {
        let lex = build_test_lex(
            &[],
            &["Robb", "Robb Stark"],
            vec![Algorithm::English],
        );
        let hits = lex.row_hits("Robb Stark rode north.");
        assert_eq!(hits.len(), 1);
        // The match must cover both words, not just "Robb".
        assert!(hits[0].col_end - hits[0].col_start >= "Robb Stark".len());
    }

    #[test]
    fn empty_lexicon_no_hits() {
        let lex = Lexicon::default();
        assert!(lex.is_empty());
        assert!(lex.row_hits("anything").is_empty());
    }

    #[test]
    fn empty_algos_falls_back_to_exact() {
        let lex = build_test_lex(&["Rohan"], &[], vec![]);
        let hits = lex.row_hits("Rohan rides.");
        assert_eq!(hits.len(), 1);
        // No stemmer → "Rohans" wouldn't match. Verify.
        let none = lex.row_hits("Rohans ride.");
        assert!(none.is_empty());
    }
}
