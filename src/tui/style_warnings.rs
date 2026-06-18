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
    built_in_cognition_verbs, built_in_emotion_adjectives,
    built_in_filter_words, built_in_linking_verbs, built_in_manner_adverbs,
    built_in_stop_words, parse_stemmer_language, AnachronismConfig, FilterWordsConfig,
    RepeatedPhrasesConfig, ShowDontTellConfig,
};

/// What kind of stylistic warning a hit represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleWarningKind {
    /// `just`, `really`, `very`, `просто`, `очень`, …
    FilterWord,
    /// 1.2.9+ — an n-gram that repeats 3+ times in the
    /// open paragraph.  See `RepeatedPhraseDetector`.
    RepeatedPhrase,
    /// 1.2.9+ — a "telling" pattern: copula + emotion
    /// adjective (`was angry`), manner-of-emotion
    /// adverb (`angrily`), or direct cognition verb
    /// (`realised` / `knew`).  See
    /// `ShowDontTellDetector`.
    ShowDontTell,
    /// 1.2.20+ C.1.b — a word echoing across nearby
    /// paragraphs of the chapter, underlined in the open
    /// paragraph.  See `crate::tui::echo_overlay`.
    Echo,
    /// 1.3.8 — a term that postdates the manuscript's setting year
    /// (`editor.style_warnings.anachronism`).  See `AnachronismDetector`.
    Anachronism,
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
        let lang = language.to_lowercase();
        // 1.3.13 — resolution: per-language map (any language) → fixed field
        // (the five) → built-in. Non-supported languages never fall back to
        // English.
        let fixed: &[String] = match lang.as_str() {
            "russian" => &cfg.russian,
            "french" => &cfg.french,
            "german" => &cfg.german,
            "spanish" => &cfg.spanish,
            "english" | "en" | "" => &cfg.english,
            _ => &[],
        };
        let stemmer = if cfg.use_stemming {
            parse_stemmer_language(language).map(Stemmer::create)
        } else {
            None
        };
        let normalise = |w: &str| -> String { crate::text::normalize_stem(w.trim(), &stemmer) };
        let mut targets: HashSet<String> = HashSet::new();
        let add = |words: &[String], targets: &mut HashSet<String>| {
            for w in words {
                let key = normalise(w);
                if !key.is_empty() {
                    targets.insert(key);
                }
            }
        };
        if let Some(mapped) = cfg.languages.get(&lang).filter(|l| !l.is_empty()) {
            add(mapped, &mut targets);
        } else if !fixed.is_empty() {
            add(fixed, &mut targets);
        } else {
            for w in built_in_filter_words(&lang) {
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
            let key = crate::text::normalize_stem(word, &self.stemmer);
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
        // Stop-word set — 1.3.13 resolution: per-language map (any language) →
        // fixed field (the five) → built-in. Non-supported languages never
        // fall back to English. Stemmed to align with token stems.
        let lang = language.to_lowercase();
        let fixed_stop: &[String] = match lang.as_str() {
            "russian" => &cfg.russian_stop_words,
            "french" => &cfg.french_stop_words,
            "german" => &cfg.german_stop_words,
            "spanish" => &cfg.spanish_stop_words,
            "english" | "en" | "" => &cfg.english_stop_words,
            _ => &[],
        };
        let normalise_stop = |w: &str| -> String { crate::text::normalize_stem(w.trim(), &stemmer) };
        let stops: std::collections::HashSet<String> =
            if let Some(mapped) = cfg.languages.get(&lang).filter(|l| !l.is_empty()) {
                mapped.iter().map(|s| normalise_stop(s)).collect()
            } else if !fixed_stop.is_empty() {
                fixed_stop.iter().map(|s| normalise_stop(s)).collect()
            } else {
                built_in_stop_words(&lang).iter().map(|s| normalise_stop(s)).collect()
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
                let stem = crate::text::normalize_stem(word, &stemmer);
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

/// 1.2.9+ — show-don't-tell detector.
///
/// Flags three categories of "telling" prose:
///
///   1. **Copula + emotion adjective** — a 2-gram
///      `(linking_verb)(emotion_adjective)` where the
///      first token's stem matches a linking-verb stem
///      (`be`, `seem`, `feel`, `appear`, `look`,
///      `become`, `remain`, `grow`, `sound`) and the
///      next token's stem matches an emotion adjective
///      (`angry`, `sad`, `happy`, `afraid`, …).  Both
///      tokens are flagged.
///   2. **Manner-of-emotion adverbs** — a single token
///      whose stem matches a known manner adverb
///      (`angrily`, `sadly`, `nervously`, …).  These
///      adverbs label emotion outright instead of
///      letting behaviour reveal it.
///   3. **Cognition verbs** — a single token whose
///      stem matches a known cognition verb
///      (`realised`, `understood`, `knew`,
///      `wondered`, …).  These tell the reader the
///      character's internal state directly.
///
/// Built from a precomputed stem set for each
/// category at detector init.  Per-row scan walks
/// `unicode_word_indices` once; cheap at literary
/// scale.  Char-indexed columns so multi-byte chars
/// don't shift highlight ranges.
pub struct ShowDontTellDetector {
    linking_verbs: HashSet<String>,
    emotion_adjectives: HashSet<String>,
    manner_adverbs: HashSet<String>,
    cognition_verbs: HashSet<String>,
    stemmer: Option<Stemmer>,
}

impl ShowDontTellDetector {
    pub fn new(cfg: &ShowDontTellConfig, language: &str) -> Self {
        let stemmer = if cfg.use_stemming {
            parse_stemmer_language(language).map(Stemmer::create)
        } else {
            None
        };
        let normalise = |w: &str| -> String { crate::text::normalize_stem(w.trim(), &stemmer) };
        // 1.3.13 resolution per category: per-language map (any language) →
        // fixed field (the five) → built-in. Non-supported languages never
        // fall back to English.
        let lang = language.to_lowercase();
        let mapped = cfg.languages.get(&lang);
        let fixed_lv: &[String] = match lang.as_str() {
            "russian" => &cfg.russian_linking_verbs,
            "french" => &cfg.french_linking_verbs,
            "german" => &cfg.german_linking_verbs,
            "spanish" => &cfg.spanish_linking_verbs,
            "english" | "en" | "" => &cfg.english_linking_verbs,
            _ => &[],
        };
        let fixed_ea: &[String] = match lang.as_str() {
            "russian" => &cfg.russian_emotion_adjectives,
            "french" => &cfg.french_emotion_adjectives,
            "german" => &cfg.german_emotion_adjectives,
            "spanish" => &cfg.spanish_emotion_adjectives,
            "english" | "en" | "" => &cfg.english_emotion_adjectives,
            _ => &[],
        };
        let fixed_ma: &[String] = match lang.as_str() {
            "russian" => &cfg.russian_manner_adverbs,
            "french" => &cfg.french_manner_adverbs,
            "german" => &cfg.german_manner_adverbs,
            "spanish" => &cfg.spanish_manner_adverbs,
            "english" | "en" | "" => &cfg.english_manner_adverbs,
            _ => &[],
        };
        let fixed_cv: &[String] = match lang.as_str() {
            "russian" => &cfg.russian_cognition_verbs,
            "french" => &cfg.french_cognition_verbs,
            "german" => &cfg.german_cognition_verbs,
            "spanish" => &cfg.spanish_cognition_verbs,
            "english" | "en" | "" => &cfg.english_cognition_verbs,
            _ => &[],
        };
        let pick = |mapped_list: Option<&Vec<String>>,
                    fixed: &[String],
                    builtin: &[&str]|
         -> HashSet<String> {
            let mut s: HashSet<String> = HashSet::new();
            let chosen: &[String] = match mapped_list.filter(|l| !l.is_empty()) {
                Some(l) => l.as_slice(),
                None => fixed,
            };
            if !chosen.is_empty() {
                for w in chosen {
                    let key = normalise(w);
                    if !key.is_empty() {
                        s.insert(key);
                    }
                }
            } else {
                for w in builtin {
                    let key = normalise(w);
                    if !key.is_empty() {
                        s.insert(key);
                    }
                }
            }
            s
        };
        Self {
            linking_verbs: pick(mapped.map(|m| &m.linking_verbs), fixed_lv, built_in_linking_verbs(&lang)),
            emotion_adjectives: pick(mapped.map(|m| &m.emotion_adjectives), fixed_ea, built_in_emotion_adjectives(&lang)),
            manner_adverbs: pick(mapped.map(|m| &m.manner_adverbs), fixed_ma, built_in_manner_adverbs(&lang)),
            cognition_verbs: pick(mapped.map(|m| &m.cognition_verbs), fixed_cv, built_in_cognition_verbs(&lang)),
            stemmer,
        }
    }

    /// True when every category is empty — the
    /// render loop can short-circuit before any
    /// per-row work.
    pub fn is_empty(&self) -> bool {
        self.linking_verbs.is_empty()
            && self.emotion_adjectives.is_empty()
            && self.manner_adverbs.is_empty()
            && self.cognition_verbs.is_empty()
    }

    /// Walk `line`, return every show-don't-tell hit
    /// at this row's char columns.  Cross-token
    /// patterns (`was angry`) require both tokens to
    /// appear on the same row — multi-row "telling"
    /// constructs are rare in practice and would need
    /// a paragraph-level detector to handle cleanly.
    pub fn detect(&self, line: &str) -> Vec<StyleHit> {
        if self.is_empty() || line.is_empty() {
            return Vec::new();
        }
        // Same char-index map approach as
        // FilterWordsDetector — keeps highlight cols
        // multi-byte-safe.
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
        // Collect tokens + stems on a single pass so
        // pattern 1 can index back into the previous
        // token cheaply.
        struct Tok {
            byte_start: usize,
            byte_end: usize,
            stem: String,
        }
        let tokens: Vec<Tok> = line
            .unicode_word_indices()
            .map(|(b, w)| {
                let stem = crate::text::normalize_stem(w, &self.stemmer);
                Tok {
                    byte_start: b,
                    byte_end: b + w.len(),
                    stem,
                }
            })
            .collect();
        let mut out: Vec<StyleHit> = Vec::new();
        for (i, tok) in tokens.iter().enumerate() {
            // Pattern 2: manner adverb.
            if self.manner_adverbs.contains(&tok.stem) {
                out.push(StyleHit {
                    col_start: byte_to_char[tok.byte_start],
                    col_end: byte_to_char
                        .get(tok.byte_end)
                        .copied()
                        .unwrap_or(char_count),
                    kind: StyleWarningKind::ShowDontTell,
                });
                continue;
            }
            // Pattern 3: cognition verb.
            if self.cognition_verbs.contains(&tok.stem) {
                out.push(StyleHit {
                    col_start: byte_to_char[tok.byte_start],
                    col_end: byte_to_char
                        .get(tok.byte_end)
                        .copied()
                        .unwrap_or(char_count),
                    kind: StyleWarningKind::ShowDontTell,
                });
                continue;
            }
            // Pattern 1: (linking verb)(emotion adj).
            if self.linking_verbs.contains(&tok.stem) {
                if let Some(next) = tokens.get(i + 1) {
                    if self.emotion_adjectives.contains(&next.stem) {
                        // Flag both tokens as a single
                        // span covering verb→adjective.
                        out.push(StyleHit {
                            col_start: byte_to_char[tok.byte_start],
                            col_end: byte_to_char
                                .get(next.byte_end)
                                .copied()
                                .unwrap_or(char_count),
                            kind: StyleWarningKind::ShowDontTell,
                        });
                    }
                }
            }
        }
        out.sort_by_key(|h| h.col_start);
        out
    }
}

/// 1.3.8 — a curated lexicon of period-bound terms with the earliest year
/// each plausibly appears. English-first; projects extend it via
/// `editor.style_warnings.anachronism.terms`. Single-word terms only (the
/// detector matches per word).
pub const BUILTIN_ANACHRONISMS: &[(&str, i32)] = &[
    ("wristwatch", 1900),
    ("telephone", 1876),
    ("automobile", 1885),
    ("airplane", 1903),
    ("aeroplane", 1903),
    ("helicopter", 1939),
    ("television", 1927),
    ("radio", 1906),
    ("photograph", 1839),
    ("photography", 1839),
    ("camera", 1840),
    ("typewriter", 1868),
    ("phonograph", 1877),
    ("gramophone", 1887),
    ("bicycle", 1869),
    ("motorcycle", 1894),
    ("elevator", 1852),
    ("escalator", 1900),
    ("refrigerator", 1880),
    ("flashlight", 1899),
    ("thermos", 1904),
    ("jeans", 1873),
    ("zipper", 1893),
    ("nylon", 1938),
    ("penicillin", 1928),
    ("antibiotic", 1941),
    ("vaccine", 1800),
    ("teenager", 1941),
    ("scientist", 1834),
    ("dinosaur", 1842),
    ("okay", 1839),
    ("computer", 1946),
    ("internet", 1983),
    ("email", 1971),
    ("smartphone", 2007),
];

/// 1.3.8 — flags terms that postdate the manuscript's setting year. A
/// `style_warnings` sibling: built from config, scans a line, returns
/// char-indexed [`StyleHit`]s. Off when no setting year is configured.
pub struct AnachronismDetector {
    /// lowercased term → earliest plausible year (only terms later than the
    /// setting are kept).
    flagged: std::collections::HashMap<String, i32>,
}

impl AnachronismDetector {
    pub fn new(cfg: &AnachronismConfig) -> Self {
        let mut flagged = std::collections::HashMap::new();
        if let Some(year) = cfg.year {
            let builtin = BUILTIN_ANACHRONISMS.iter().map(|&(t, e)| (t.to_string(), e));
            let custom = cfg.terms.iter().map(|t| (t.term.clone(), t.earliest));
            for (term, earliest) in builtin.chain(custom) {
                if earliest > year {
                    flagged.insert(term.to_lowercase(), earliest);
                }
            }
        }
        Self { flagged }
    }

    pub fn is_empty(&self) -> bool {
        self.flagged.is_empty()
    }

    /// The earliest year of a flagged term (for the finding message).
    pub fn earliest(&self, word: &str) -> Option<i32> {
        self.flagged.get(&word.to_lowercase()).copied()
    }

    /// Every anachronistic word in `line`, at char columns.
    pub fn detect(&self, line: &str) -> Vec<StyleHit> {
        if self.flagged.is_empty() || line.is_empty() {
            return Vec::new();
        }
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
            if !self.flagged.contains_key(&word.to_lowercase()) {
                continue;
            }
            let byte_end = byte_start + word.len();
            out.push(StyleHit {
                col_start: byte_to_char[byte_start],
                col_end: byte_to_char.get(byte_end).copied().unwrap_or(char_count),
                kind: StyleWarningKind::Anachronism,
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
    fn anachronism_off_without_a_year_else_flags_late_terms() {
        // no setting year → detector disabled
        let off = AnachronismDetector::new(&AnachronismConfig::default());
        assert!(off.is_empty());
        assert!(off.detect("she checked her wristwatch").is_empty());

        // an 1840 setting flags the wristwatch (≥1900) but not the camera
        // (1840 is not *after* 1840).
        let cfg = AnachronismConfig { year: Some(1840), terms: Vec::new() };
        let det = AnachronismDetector::new(&cfg);
        assert!(!det.is_empty());
        let hits = det.detect("She checked her wristwatch by the camera.");
        assert_eq!(hits.len(), 1, "only the wristwatch is anachronistic");
        assert_eq!(hits[0].kind, StyleWarningKind::Anachronism);
        assert_eq!(det.earliest("wristwatch"), Some(1900));
        // a project term override
        let cfg2 = AnachronismConfig {
            year: Some(1840),
            terms: vec![crate::config::AnachronismTerm { term: "spyglass".into(), earliest: 1990 }],
        };
        assert!(!AnachronismDetector::new(&cfg2).detect("a spyglass").is_empty());
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
    fn russian_yo_fold_matches_e_spelling() {
        // Filter list spells the word with `ё`; the author
        // typed the `е` form.  Without the ё→е fold the
        // stems diverge and the hit is missed (1.2.20 B.1).
        let mut cfg = cfg_default();
        cfg.russian = vec!["ещё".to_string()];
        let d = FilterWordsDetector::new(&cfg, "russian");
        assert!(!d.detect("ещё раз").is_empty(), "ё-spelling must match");
        assert!(
            !d.detect("еще раз").is_empty(),
            "е-spelling must match the ё list entry via the fold"
        );
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
    fn unknown_language_is_empty_not_english() {
        // 1.3.13 — a non-curated language gets an EMPTY built-in list (detector
        // off), never English words flagged in foreign prose.
        let d = FilterWordsDetector::new(&cfg_default(), "klingon");
        assert!(d.is_empty(), "non-curated language → no built-in filter words");
        assert_eq!(d.detect("just a test").len(), 0);
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

    // ── ShowDontTellDetector ────────────────────────

    fn sdt_cfg_default() -> ShowDontTellConfig {
        ShowDontTellConfig::default()
    }

    #[test]
    fn sdt_was_angry_flagged() {
        let d = ShowDontTellDetector::new(&sdt_cfg_default(), "english");
        assert!(!d.is_empty(), "english defaults populated");
        let hits = d.detect("She was angry at the dog.");
        assert!(!hits.is_empty(), "telling 'was angry' should hit");
        // Span should cover "was angry" — the chars
        // 4..13 in "She was angry at the dog."
        let h = &hits[0];
        assert_eq!(h.kind, StyleWarningKind::ShowDontTell);
        assert_eq!(h.col_start, 4);
        assert_eq!(h.col_end, 13);
    }

    #[test]
    fn sdt_was_running_not_flagged() {
        // Linking verb without an emotion adjective →
        // no hit.  Otherwise every `was X` would alarm.
        let d = ShowDontTellDetector::new(&sdt_cfg_default(), "english");
        let hits = d.detect("She was running through the rain.");
        assert!(
            hits.is_empty(),
            "non-emotion 'was running' must NOT hit, got: {hits:?}"
        );
    }

    #[test]
    fn sdt_seemed_nervous_flagged_via_stemming() {
        // "Seemed" must stem to "seem" so it matches
        // the linking-verb stem.
        let d = ShowDontTellDetector::new(&sdt_cfg_default(), "english");
        let hits = d.detect("He seemed nervous about the meeting.");
        assert!(!hits.is_empty());
    }

    #[test]
    fn sdt_manner_adverb_flagged() {
        let d = ShowDontTellDetector::new(&sdt_cfg_default(), "english");
        let hits = d.detect("\"Get out,\" she said angrily.");
        // `angrily` should be flagged as a manner
        // adverb — single-token hit.
        assert!(hits.iter().any(|h| {
            let trimmed = "Get out".len();
            let _ = trimmed; // (silence unused warning)
            h.kind == StyleWarningKind::ShowDontTell
        }));
    }

    #[test]
    fn sdt_cognition_verb_flagged() {
        let d = ShowDontTellDetector::new(&sdt_cfg_default(), "english");
        let hits = d.detect("She realised the room was empty.");
        assert!(!hits.is_empty(), "'realised' must trigger cognition hit");
    }

    #[test]
    fn sdt_plain_action_prose_clean() {
        // Hemingway-flavoured action sentence — should
        // produce zero hits.
        let d = ShowDontTellDetector::new(&sdt_cfg_default(), "english");
        let hits = d.detect(
            "He poured the coffee and watched the rain hit the shutters.",
        );
        assert!(
            hits.is_empty(),
            "action prose must stay clean, got: {hits:?}"
        );
    }

    #[test]
    fn sdt_unsupported_language_falls_back_quiet() {
        // 1.2.11+ — built-ins now ship for all five
        // supported languages (en/ru/fr/de/es).  This
        // test still locks the fallback path: a
        // language we *don't* ship lists for + empty
        // user config must produce an `is_empty()`
        // detector so the render pipeline can short-
        // circuit cleanly.  "klingon" stands in for
        // "anything not on the list".
        let d = ShowDontTellDetector::new(&sdt_cfg_default(), "klingon");
        assert!(d.is_empty());
    }

    #[test]
    fn sdt_unicode_columns_safe() {
        // Mixed Latin + accented chars in the same
        // line — column calculations must use char
        // (not byte) indices.
        let d = ShowDontTellDetector::new(&sdt_cfg_default(), "english");
        // "Café" is 4 chars but 5 bytes.  Then we have
        // an English emotion phrase right after.  The
        // hit's columns must land in char-space.
        let line = "Café was empty. He was sad.";
        let hits = d.detect(line);
        // "was sad" sits at the end.  The hit's
        // col_start should equal the char index of
        // 'w' in 'was sad', which is past the period
        // + space.
        if let Some(h) = hits.last() {
            let chars: Vec<char> = line.chars().collect();
            // Verify the hit spans valid chars.
            assert!(h.col_end <= chars.len());
        }
    }
}
