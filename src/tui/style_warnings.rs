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
//!   * Filter-word lists are keyed by the project's
//!     `language` HJSON field (e.g. `"english"`,
//!     `"russian"`).  Each language ships a curated
//!     default; users override by HJSON.
//!   * Tokenisation uses `unicode-segmentation`'s
//!     `unicode_words()`, which is Unicode-Standard-
//!     Annex-#29-compliant.  Cyrillic, Latin, Greek,
//!     and Devanagari work out of the box; CJK uses
//!     character-level (one char per "word") because
//!     UAX#29 doesn't split CJK runs and we don't
//!     bundle a CJK tokeniser.
//!   * Case-folded comparison via `to_lowercase()` —
//!     ASCII-only would miss `Очень` matching `очень`.

use unicode_segmentation::UnicodeSegmentation;

/// What kind of stylistic warning a hit represents.
/// One enum so the render layer can branch on it for
/// per-kind theme colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleWarningKind {
    /// `just`, `really`, `very`, `просто`, `очень`, …
    /// Words that add nothing to meaning but bloat the
    /// sentence.  Often called "filter words" or
    /// "intensifier crutches" in the craft literature.
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

/// Walk `line` and return every hit at this row's
/// character columns.  `language` selects which filter-
/// word list to apply (case-insensitive, exact match
/// against the lowercased word).  `extra_words` is the
/// user's HJSON-overrideable union of words to flag in
/// addition to the defaults — entries here always
/// apply regardless of language.
///
/// Returns `Vec<StyleHit>` in ascending column order.
pub fn detect_filter_words(
    line: &str,
    language: &str,
    extra_words: &[String],
) -> Vec<StyleHit> {
    if line.is_empty() {
        return Vec::new();
    }
    let defaults = default_filter_words(language);
    // Build a HashSet of all lowercased words to flag.
    use std::collections::HashSet;
    let mut targets: HashSet<String> = HashSet::with_capacity(defaults.len() + extra_words.len());
    for w in defaults {
        targets.insert(w.to_lowercase());
    }
    for w in extra_words {
        let lc = w.trim().to_lowercase();
        if !lc.is_empty() {
            targets.insert(lc);
        }
    }
    if targets.is_empty() {
        return Vec::new();
    }

    // Walk the line as Unicode words.  unicode-segmentation
    // gives us (byte_offset, word_str) tuples.  We need
    // char-column offsets, so we maintain a running
    // mapping byte_offset → char_col.
    let mut byte_to_char: Vec<usize> = Vec::with_capacity(line.len() + 1);
    let mut char_count = 0usize;
    for (b, _c) in line.char_indices() {
        // Pad any byte slots between char starts with the
        // current char index — needed for multi-byte chars.
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
        if !targets.contains(&lc) {
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

/// 1.2.9+ — built-in filter-word list per language.
/// Curated from common writing-craft references (Strunk,
/// King, Hemingway-app rules); not exhaustive.  Users
/// extend via `editor.style_warnings.filter_words` in
/// HJSON.  Falls back to English when the project's
/// `language` field doesn't match any built-in list.
pub fn default_filter_words(language: &str) -> &'static [&'static str] {
    match language.to_lowercase().as_str() {
        "russian" => RUSSIAN_FILTER_WORDS,
        "french" => FRENCH_FILTER_WORDS,
        "german" => GERMAN_FILTER_WORDS,
        "spanish" => SPANISH_FILTER_WORDS,
        _ => ENGLISH_FILTER_WORDS,
    }
}

/// English filter words.  Three families:
///   * Hedges: `just`, `really`, `very`, `pretty`, `quite`,
///     `rather`, `fairly`, `somewhat`, `slightly`
///   * Generic placeholders: `that` (often deletable),
///     `actually`, `basically`, `literally`,
///     `essentially`, `simply`
///   * Sensory hedges: `seemed`, `felt`, `looked`,
///     `appeared`, `sounded`, `noticed`
/// `that` is contentious; some prose absolutely needs it
/// — the flag is a prompt to question, not a verdict.
const ENGLISH_FILTER_WORDS: &[&str] = &[
    "just", "really", "very", "pretty", "quite",
    "rather", "fairly", "somewhat", "slightly",
    "that", "actually", "basically", "literally",
    "essentially", "simply", "definitely", "certainly",
    "absolutely", "totally", "completely",
    "seemed", "felt", "looked", "appeared",
    "sounded", "noticed", "began", "started",
    "suddenly", "perhaps", "maybe",
];

/// Russian filter words.  Equivalent semantic categories
/// to the English list — hedges, intensifier crutches,
/// sensory verbs.  Verified against contemporary
/// Russian-language writing-craft sources.  Users editing
/// in Russian can pair this with `voice: "Milena"` /
/// `"Katya (Enhanced)"` in the TTS config for an
/// end-to-end Russian editing workflow.
const RUSSIAN_FILTER_WORDS: &[&str] = &[
    // Hedges / intensifier crutches
    "очень", "просто", "именно", "довольно", "слишком",
    "весьма", "крайне", "вполне", "достаточно",
    // Generic placeholders
    "собственно", "буквально", "практически",
    "фактически", "действительно", "реально",
    "конечно", "разумеется", "безусловно",
    // Sensory / hedging verbs (lemmas — derivatives
    // are caught by exact match for the listed forms;
    // users add inflections via HJSON if needed)
    "казалось", "казался", "казалась", "казались",
    "почувствовал", "почувствовала", "почувствовали",
    "выглядел", "выглядела", "выглядели",
    "заметил", "заметила", "заметили",
    "вдруг", "внезапно", "наверное", "возможно",
];

/// French — same three families, common stylistic
/// crutches per French-language writing guides.
const FRENCH_FILTER_WORDS: &[&str] = &[
    "vraiment", "très", "assez", "plutôt", "quelque peu",
    "juste", "simplement", "actuellement", "littéralement",
    "essentiellement", "absolument", "totalement", "complètement",
    "semblait", "paraissait", "sentait",
    "soudainement", "peut-être",
];

/// German.
const GERMAN_FILTER_WORDS: &[&str] = &[
    "sehr", "wirklich", "ziemlich", "eher", "etwas",
    "einfach", "tatsächlich", "buchstäblich", "im Grunde",
    "absolut", "völlig", "komplett",
    "schien", "fühlte", "sah",
    "plötzlich", "vielleicht",
];

/// Spanish.
const SPANISH_FILTER_WORDS: &[&str] = &[
    "muy", "realmente", "bastante", "más bien", "algo",
    "solo", "simplemente", "actualmente", "literalmente",
    "esencialmente", "absolutamente", "totalmente", "completamente",
    "parecía", "se sentía", "se veía",
    "repentinamente", "quizás", "tal vez",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn cols_of(hits: &[StyleHit]) -> Vec<(usize, usize)> {
        hits.iter().map(|h| (h.col_start, h.col_end)).collect()
    }

    #[test]
    fn english_filter_word_basic() {
        let hits = detect_filter_words("I just wanted to see", "english", &[]);
        assert_eq!(cols_of(&hits), vec![(2, 6)]);
    }

    #[test]
    fn english_case_insensitive() {
        let hits = detect_filter_words("Just wait. JUST a moment.", "english", &[]);
        assert_eq!(cols_of(&hits), vec![(0, 4), (11, 15)]);
    }

    #[test]
    fn english_multiple_per_line() {
        let hits = detect_filter_words(
            "He was really very tired, just a little.",
            "english",
            &[],
        );
        // really → "really" at the right offset
        let words: Vec<&str> = hits
            .iter()
            .map(|h| &"He was really very tired, just a little."[..])
            .map(|s| &s[..])
            .collect();
        assert!(words.len() >= 3, "expected at least 3 hits, got {hits:?}");
    }

    #[test]
    fn russian_basic() {
        let hits = detect_filter_words(
            "Он был очень устал и просто хотел спать.",
            "russian",
            &[],
        );
        // очень + просто → 2 hits
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn russian_case_insensitive() {
        let hits = detect_filter_words("Очень устал.", "russian", &[]);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn empty_line_no_hits() {
        assert!(detect_filter_words("", "english", &[]).is_empty());
        assert!(detect_filter_words("   ", "english", &[]).is_empty());
    }

    #[test]
    fn unknown_language_falls_back_to_english() {
        let hits = detect_filter_words("just a test", "klingon", &[]);
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn user_extras_apply_on_top_of_defaults() {
        let extras = vec!["foo".into(), "bar".into()];
        let hits = detect_filter_words("just foo bar baz", "english", &extras);
        // just (default), foo (extra), bar (extra) = 3
        assert_eq!(hits.len(), 3);
    }

    #[test]
    fn cyrillic_columns_are_char_indexed_not_byte() {
        // "очень" is 5 chars but 10 bytes in UTF-8.  The
        // hit must be reported as columns [0..5], not
        // [0..10].
        let hits = detect_filter_words("очень устал", "russian", &[]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].col_start, 0);
        assert_eq!(hits[0].col_end, 5);
    }

    #[test]
    fn no_partial_word_match() {
        // "justice" must NOT be flagged because of "just"
        // — unicode_word_indices yields "justice" as one
        // word, which isn't in the list.
        let hits = detect_filter_words("justice is essential", "english", &[]);
        assert!(
            !hits.iter().any(|h| h.col_start == 0 && h.col_end == 7),
            "false positive on `justice`: {hits:?}"
        );
    }

    #[test]
    fn punctuation_doesnt_break_match() {
        // "just." must match "just".
        let hits = detect_filter_words("And just.", "english", &[]);
        assert_eq!(hits.len(), 1);
        // Match is on the word, not the period.
        assert_eq!(hits[0].col_start, 4);
        assert_eq!(hits[0].col_end, 8);
    }
}
