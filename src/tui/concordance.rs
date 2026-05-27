//! 1.2.9+ — project-wide concordance.
//!
//! Walks every paragraph in the open project, tokenises
//! its body with `unicode-segmentation`'s UAX-#29 word
//! segmenter, optionally applies Snowball stemming, drops
//! configured stop-words (so the top of the list is meaningful
//! lexical content, not `the` / `and` / `и` / `в`), and emits a
//! ranked list of headwords with KWIC samples.
//!
//! The build is one-shot at modal open — `Modal::Concordance`
//! holds the result so the render loop doesn't re-scan the
//! corpus every frame.  Cheap at literary scale: a 100k-word
//! book completes in well under a second on the author's
//! laptop.
//!
//! Multilingual story (same plumbing as `style_warnings`):
//!
//!   * Stemmer picked via `parse_stemmer_language(language)` —
//!     so `said` / `says` / `saying` all key on `said`;
//!     Russian `сказал/сказала/сказали` all key on the same
//!     stem.
//!   * Stop-words drawn from the per-language
//!     `repeated_phrases.<lang>_stop_words` config field
//!     (configured list wins; built-in fallback otherwise) —
//!     reusing the same multilingual stop list already audited
//!     for the repeated-phrase detector.
//!   * Pure-digit tokens, one-character tokens, and tokens
//!     made entirely of underscores are skipped.

use std::collections::HashMap;

use rust_stemmers::Stemmer;
use unicode_segmentation::UnicodeSegmentation;

use crate::config::{
    built_in_stop_words, parse_stemmer_language, RepeatedPhrasesConfig,
};

/// Maximum number of KWIC samples per headword.  Three is
/// enough for the reader to triangulate where a word lives
/// without ballooning memory for high-frequency words like
/// proper names that may appear hundreds of times.
const SAMPLES_PER_ENTRY: usize = 3;

/// Maximum unique surface forms kept per stem.  Used to
/// build the "(seems, seemed, seeming)" trailer on the
/// headword row.
const VARIANTS_PER_ENTRY: usize = 5;

/// Half-width of the KWIC window in characters — the
/// sample shows ~`KWIC_HALF_WIDTH` chars before and after
/// the matched token, clipped at row bounds.
const KWIC_HALF_WIDTH: usize = 32;

/// One paragraph fed to the builder.  Caller supplies the
/// human-readable identifier (slug path) plus the body
/// pre-split into rows.  Splitting upstream keeps the
/// builder free of `\r\n` quirks.
pub struct ParagraphInput<'a> {
    pub slug_path: String,
    pub lines: &'a [String],
}

#[derive(Debug, Clone)]
pub struct ConcordanceSample {
    /// Slug path of the source paragraph.
    pub slug_path: String,
    /// 1-based row index inside that paragraph.
    pub line_no: usize,
    /// KWIC-rendered context line, ≈ 2× `KWIC_HALF_WIDTH`
    /// characters wide.  The matched word is wrapped in
    /// `«…»` so it's visually obvious in monospace output.
    pub kwic: String,
}

#[derive(Debug, Clone)]
pub struct ConcordanceEntry {
    /// The most-common surface form for this stem
    /// (lowercase).  Shown as the row's headword so the
    /// reader sees real prose, not a stemmed key.
    pub headword: String,
    /// Stem key shared by every variant.  Empty when no
    /// stemmer is active.
    pub stem: String,
    /// Total occurrences across the project.
    pub count: usize,
    /// Unique surface forms, oldest first, capped at
    /// `VARIANTS_PER_ENTRY`.
    pub variants: Vec<String>,
    /// Up to `SAMPLES_PER_ENTRY` KWIC samples.
    pub samples: Vec<ConcordanceSample>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Count,
    Alphabetical,
}

impl SortMode {
    pub fn toggle(self) -> Self {
        match self {
            Self::Count => Self::Alphabetical,
            Self::Alphabetical => Self::Count,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Alphabetical => "alphabetical",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConcordanceData {
    pub entries: Vec<ConcordanceEntry>,
    /// Counted tokens (after stop-word + digit + tiny-
    /// token filters).
    pub total_tokens: usize,
    /// Distinct stem keys present in `entries`.  Always
    /// equals `entries.len()`, kept as a convenience for
    /// the modal header.
    pub distinct_words: usize,
    /// Total paragraphs scanned.
    pub paragraphs_scanned: usize,
}

/// Internal accumulator — one per stem key.  Kept private
/// so the public `ConcordanceEntry` shape can change
/// independently.
struct Acc {
    count: usize,
    surface_counts: HashMap<String, usize>,
    surface_order: Vec<String>,
    samples: Vec<ConcordanceSample>,
}

impl Acc {
    fn new() -> Self {
        Self {
            count: 0,
            surface_counts: HashMap::new(),
            surface_order: Vec::new(),
            samples: Vec::new(),
        }
    }
}

/// Build a concordance from the project corpus.
///
/// `cfg.use_stemming` and the per-language stop-word lists
/// gate stemming + stop-word filtering, respectively —
/// reusing the same `RepeatedPhrasesConfig` block so authors
/// don't tune two parallel lists.  Stop-word filtering uses
/// built-in defaults when the configured list is empty.
pub fn build(
    cfg: &RepeatedPhrasesConfig,
    language: &str,
    paragraphs: &[ParagraphInput<'_>],
) -> ConcordanceData {
    let stemmer = if cfg.use_stemming {
        parse_stemmer_language(language).map(Stemmer::create)
    } else {
        None
    };

    // Stop-word set: configured list (when non-empty) OR
    // built-in default.  Stemmed so it aligns with the
    // tokens being filtered.
    let stop_configured: &Vec<String> = match language.to_lowercase().as_str() {
        "russian" => &cfg.russian_stop_words,
        "french" => &cfg.french_stop_words,
        "german" => &cfg.german_stop_words,
        "spanish" => &cfg.spanish_stop_words,
        _ => &cfg.english_stop_words,
    };
    let stop_source: Vec<String> = if stop_configured.is_empty() {
        built_in_stop_words(language)
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    } else {
        stop_configured.clone()
    };
    let normalise = |w: &str| -> String {
        let lc = w.trim().to_lowercase();
        match &stemmer {
            Some(s) => s.stem(&lc).into_owned(),
            None => lc,
        }
    };
    let mut stop_set: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for w in &stop_source {
        let key = normalise(w);
        if !key.is_empty() {
            stop_set.insert(key);
        }
    }

    let mut by_stem: HashMap<String, Acc> = HashMap::new();
    let mut total_tokens: usize = 0;
    let mut paragraphs_scanned: usize = 0;

    for para in paragraphs {
        paragraphs_scanned += 1;
        for (row_idx, line) in para.lines.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            for (byte_start, word) in line.unicode_word_indices() {
                if !is_countable(word) {
                    continue;
                }
                let surface = word.to_lowercase();
                let stem_key = match &stemmer {
                    Some(s) => s.stem(&surface).into_owned(),
                    None => surface.clone(),
                };
                if stem_key.is_empty() || stop_set.contains(&stem_key) {
                    continue;
                }
                total_tokens += 1;
                let acc = by_stem.entry(stem_key).or_insert_with(Acc::new);
                acc.count += 1;
                let entry = acc
                    .surface_counts
                    .entry(surface.clone())
                    .or_insert(0);
                if *entry == 0 {
                    acc.surface_order.push(surface.clone());
                }
                *entry += 1;
                if acc.samples.len() < SAMPLES_PER_ENTRY {
                    acc.samples.push(ConcordanceSample {
                        slug_path: para.slug_path.clone(),
                        line_no: row_idx + 1,
                        kwic: kwic_snippet(line, byte_start, word.len()),
                    });
                }
            }
        }
    }

    let distinct_words = by_stem.len();
    let mut entries: Vec<ConcordanceEntry> = by_stem
        .into_iter()
        .map(|(stem, acc)| {
            // Headword = highest-count surface form;
            // ties broken by first-seen order so output
            // is deterministic.
            let headword = acc
                .surface_order
                .iter()
                .max_by_key(|s| acc.surface_counts.get(*s).copied().unwrap_or(0))
                .cloned()
                .unwrap_or_else(|| stem.clone());
            let mut variants: Vec<String> = acc.surface_order.clone();
            variants.sort_by(|a, b| {
                acc.surface_counts
                    .get(b)
                    .cmp(&acc.surface_counts.get(a))
                    .then_with(|| a.cmp(b))
            });
            variants.truncate(VARIANTS_PER_ENTRY);
            ConcordanceEntry {
                headword,
                stem,
                count: acc.count,
                variants,
                samples: acc.samples,
            }
        })
        .collect();

    entries.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.headword.cmp(&b.headword))
    });

    ConcordanceData {
        entries,
        total_tokens,
        distinct_words,
        paragraphs_scanned,
    }
}

/// 1.2.9+ — re-sort `entries` in place to match
/// `mode`.  Headword comparison is locale-naive
/// `String::cmp` (lexicographic on Unicode scalar
/// values) — same convention the lexicon view uses.
pub fn sort_in_place(entries: &mut [ConcordanceEntry], mode: SortMode) {
    match mode {
        SortMode::Count => entries.sort_by(|a, b| {
            b.count
                .cmp(&a.count)
                .then_with(|| a.headword.cmp(&b.headword))
        }),
        SortMode::Alphabetical => entries.sort_by(|a, b| a.headword.cmp(&b.headword)),
    }
}

/// True when `word` is worth counting.  Reject single
/// characters (mostly noise: stray `s`, `i`, `o`), pure
/// digit runs (`1995`), and tokens whose every char is
/// underscore (Typst sees `_emph_` but the segmenter can
/// hand back the underscores alone).
fn is_countable(word: &str) -> bool {
    let mut chars = word.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };
    // 1-char tokens: drop.  These are mostly conjunctions
    // or stray initials; the stop list catches the rest.
    if chars.next().is_none() {
        return false;
    }
    // All digits → drop.
    if first.is_ascii_digit() && word.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    // All underscores → drop.
    if word.chars().all(|c| c == '_') {
        return false;
    }
    true
}

/// Build a KWIC snippet for the matched token at
/// `byte_start..byte_start+byte_len` inside `line`.
/// Returns a string with `«match»` wrapped in
/// double-angle quotes plus up to `KWIC_HALF_WIDTH`
/// characters of context on either side, clipped at row
/// bounds and leading/trailing ellipses when the snippet
/// doesn't reach the row edge.
fn kwic_snippet(line: &str, byte_start: usize, byte_len: usize) -> String {
    let byte_end = byte_start + byte_len;
    let before = &line[..byte_start];
    let matched = &line[byte_start..byte_end];
    let after = &line[byte_end..];

    let before_chars: Vec<char> = before.chars().collect();
    let after_chars: Vec<char> = after.chars().collect();

    let (left_clipped, left_text): (bool, String) =
        if before_chars.len() > KWIC_HALF_WIDTH {
            let start = before_chars.len() - KWIC_HALF_WIDTH;
            (true, before_chars[start..].iter().collect())
        } else {
            (false, before_chars.iter().collect())
        };
    let (right_clipped, right_text): (bool, String) =
        if after_chars.len() > KWIC_HALF_WIDTH {
            (true, after_chars[..KWIC_HALF_WIDTH].iter().collect())
        } else {
            (false, after_chars.iter().collect())
        };

    let mut out = String::with_capacity(line.len() + 8);
    if left_clipped {
        out.push('…');
    }
    out.push_str(left_text.trim_start());
    out.push('«');
    out.push_str(matched);
    out.push('»');
    out.push_str(right_text.trim_end());
    if right_clipped {
        out.push('…');
    }
    // Collapse runs of whitespace so the snippet stays
    // visually clean even when the source row had a
    // tab-indented continuation.
    let mut compact = String::with_capacity(out.len());
    let mut last_space = false;
    for c in out.chars() {
        if c.is_whitespace() {
            if !last_space {
                compact.push(' ');
            }
            last_space = true;
        } else {
            compact.push(c);
            last_space = false;
        }
    }
    compact
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_default() -> RepeatedPhrasesConfig {
        RepeatedPhrasesConfig {
            enabled: true,
            n: 2,
            threshold: 2,
            use_stemming: true,
            english_stop_words: Vec::new(),
            russian_stop_words: Vec::new(),
            french_stop_words: Vec::new(),
            german_stop_words: Vec::new(),
            spanish_stop_words: Vec::new(),
        }
    }

    fn para<'a>(slug: &str, lines: &'a [String]) -> ParagraphInput<'a> {
        ParagraphInput {
            slug_path: slug.into(),
            lines,
        }
    }

    #[test]
    fn empty_corpus_yields_empty_concordance() {
        let cfg = cfg_default();
        let result = build(&cfg, "english", &[]);
        assert_eq!(result.entries.len(), 0);
        assert_eq!(result.total_tokens, 0);
        assert_eq!(result.paragraphs_scanned, 0);
    }

    #[test]
    fn stop_words_dropped() {
        let cfg = cfg_default();
        let lines = vec!["The cat and the dog".to_string()];
        let result = build(&cfg, "english", &[para("a/b", &lines)]);
        let headwords: Vec<&str> = result
            .entries
            .iter()
            .map(|e| e.headword.as_str())
            .collect();
        assert!(headwords.contains(&"cat"));
        assert!(headwords.contains(&"dog"));
        assert!(!headwords.contains(&"the"));
        assert!(!headwords.contains(&"and"));
    }

    #[test]
    fn stemming_groups_variants() {
        let cfg = cfg_default();
        let lines = vec![
            "She walked away.".to_string(),
            "He was walking quickly.".to_string(),
            "They walk at dawn.".to_string(),
        ];
        let result = build(&cfg, "english", &[para("book/ch", &lines)]);
        let walk = result
            .entries
            .iter()
            .find(|e| e.variants.iter().any(|v| v.starts_with("walk")))
            .expect("walk stem missing");
        assert_eq!(walk.count, 3);
        assert!(walk.variants.iter().any(|v| v == "walked"));
        assert!(walk.variants.iter().any(|v| v == "walking"));
        assert!(walk.variants.iter().any(|v| v == "walk"));
    }

    #[test]
    fn digits_and_one_char_tokens_skipped() {
        let cfg = cfg_default();
        let lines = vec!["I saw 1995 cats — a parade".to_string()];
        let result = build(&cfg, "english", &[para("a", &lines)]);
        let words: Vec<&str> =
            result.entries.iter().map(|e| e.headword.as_str()).collect();
        // The bare digit "1995" must be gone; single-char
        // "I" + "a" must be gone too.
        assert!(!words.contains(&"1995"));
        assert!(!words.contains(&"i"));
        assert!(!words.contains(&"a"));
        // The countable surface words still came through.
        assert!(words.contains(&"saw") || words.contains(&"cat"));
    }

    #[test]
    fn sort_in_place_alphabetical() {
        let cfg = cfg_default();
        let lines = vec![
            "Zebra apple zebra mango banana mango banana".to_string(),
        ];
        let mut result = build(&cfg, "english", &[para("a", &lines)]);
        sort_in_place(&mut result.entries, SortMode::Alphabetical);
        let order: Vec<&str> = result
            .entries
            .iter()
            .map(|e| e.headword.as_str())
            .collect();
        // Sorted ascending: apple < banana < mango < zebra.
        let pos = |w: &str| order.iter().position(|s| *s == w).unwrap();
        assert!(pos("apple") < pos("banana"));
        assert!(pos("banana") < pos("mango"));
        assert!(pos("mango") < pos("zebra"));
    }

    #[test]
    fn samples_capped_per_entry() {
        let cfg = cfg_default();
        let lines: Vec<String> = (0..10)
            .map(|i| format!("Repeated word number {i}"))
            .collect();
        let result = build(&cfg, "english", &[para("a", &lines)]);
        let repeated = result
            .entries
            .iter()
            .find(|e| e.headword == "repeated")
            .expect("repeated word missing");
        assert!(repeated.count >= 10);
        assert!(repeated.samples.len() <= SAMPLES_PER_ENTRY);
    }

    #[test]
    fn kwic_marks_match() {
        let snippet = kwic_snippet("the cat sat", 4, 3);
        assert!(snippet.contains("«cat»"));
    }

    #[test]
    fn kwic_clips_long_lines() {
        let long_left = "a ".repeat(80); // 160 chars
        let line = format!("{long_left}word here");
        let pos = long_left.len();
        let snippet = kwic_snippet(&line, pos, 4);
        assert!(snippet.starts_with('…'), "snippet = {snippet:?}");
        assert!(snippet.contains("«word»"));
    }
}
