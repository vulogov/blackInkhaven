//! 1.2.19+ C.1 — echo / repetition-at-distance detector.
//!
//! Catches the revision-stage tic where a *distinctive*
//! word or phrase gets reused close together — "she
//! *walked* to the window… he *walked* across… they
//! *walked* out" — that reads fine sentence-by-sentence
//! but clangs when the eye takes in the cluster.
//!
//! ## Multilingual by reuse (Tier 2)
//!
//! This is "concordance machinery + a sliding window + a
//! distinctiveness ceiling," so it inherits inkhaven's
//! existing multilingual NLP for free:
//!
//!   * Tokenise with UAX-#29 (`unicode-segmentation`).
//!   * Stem with the project's Snowball algorithm
//!     (`parse_stemmer_language` — 18 languages) so
//!     inflected forms collapse: en `walked`/`walking`
//!     → `walk`; ru `корабль`/`корабля`/`корабле`
//!     → `корабл`.  (Snowball is a stemmer, not a
//!     lemmatiser — most declensions collapse, but some
//!     irregular verb forms don't.  A `ё`→`е` fold is
//!     applied first so the two are unified, as real
//!     Russian text treats them.)
//!   * Drop per-language stop-words
//!     (`built_in_stop_words`, or a configured override).
//!
//! Languages outside the Snowball set (Japanese, Chinese)
//! degrade to exact-surface-form matching — inflected
//! variants won't collapse, but identical repeats still
//! flag.  Documented, consistent with the lexicon
//! overlay's fallback.
//!
//! ## Distinctiveness without a frequency corpus
//!
//! A stem is "distinctive" relative to *this unit's own*
//! distribution: it must repeat (≥ `min_repeats`) but not
//! be common vocabulary (global count ≤ `max_global`).  A
//! word used 200 times across the book is vocabulary, not
//! an echo, even when it clusters; a word used 3 times
//! total, all within four paragraphs, is a glaring echo.
//! No per-language frequency tables to ship — the measure
//! is corpus-relative + language-agnostic.

use rust_stemmers::Stemmer;
use unicode_segmentation::UnicodeSegmentation;

use crate::config::{built_in_stop_words, parse_stemmer_language};

/// Tunables for the detector.
#[derive(Debug, Clone)]
pub struct EchoConfig {
    /// Window size in consecutive paragraphs.  An echo is
    /// flagged when `min_repeats` occurrences fall within
    /// this many paragraphs.
    pub window: usize,
    /// Occurrences within the window required to flag.
    pub min_repeats: usize,
    /// Distinctiveness ceiling: stems whose total count in
    /// the unit exceeds this are common vocabulary +
    /// skipped, even if clustered.
    pub max_global: usize,
    /// Minimum word length (in chars) to consider — skips
    /// short function words the stop-list might miss.
    pub min_word_len: usize,
}

impl Default for EchoConfig {
    fn default() -> Self {
        Self {
            window: 5,
            min_repeats: 3,
            max_global: 40,
            min_word_len: 4,
        }
    }
}

/// One flagged echo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EchoFinding {
    /// The collapsed stem the echo clusters on.
    pub stem: String,
    /// Distinct surface forms seen in the flagged window,
    /// in first-seen order.
    pub surface_forms: Vec<String>,
    /// 1-based paragraph number of the first occurrence in
    /// the flagged window.
    pub para_start: usize,
    /// 1-based paragraph number of the last occurrence.
    pub para_end: usize,
    /// Occurrences inside the flagged window.
    pub count: usize,
}

#[derive(Clone)]
struct Occurrence {
    para: usize, // 0-based paragraph index
    surface: String,
}

/// Detect echoes across `paragraphs` (one plain-prose
/// string per paragraph, in reading order).
///
/// `language` selects the Snowball stemmer + the built-in
/// stop-word list.  `stop_override`, when `Some` and
/// non-empty, replaces the built-in stop-words (already
/// the project's configured list).  Pure — no I/O.
pub fn detect_echoes(
    paragraphs: &[String],
    language: &str,
    stop_override: Option<&[String]>,
    cfg: &EchoConfig,
) -> Vec<EchoFinding> {
    let stemmer = parse_stemmer_language(language).map(Stemmer::create);
    let stem_of = |w: &str| -> String { crate::text::normalize_stem(w, &stemmer) };

    // Resolve + stem the stop-word set so it aligns with
    // the stemmed tokens.
    let stop_source: Vec<String> = match stop_override {
        Some(list) if !list.is_empty() => list.to_vec(),
        _ => built_in_stop_words(language)
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    };
    let stop_set: std::collections::HashSet<String> = stop_source
        .iter()
        .map(|w| stem_of(w))
        .filter(|s| !s.is_empty())
        .collect();

    // stem → ordered occurrences; stem → total count.
    let mut occ: std::collections::HashMap<String, Vec<Occurrence>> =
        std::collections::HashMap::new();
    for (p_idx, para) in paragraphs.iter().enumerate() {
        for word in para.unicode_words() {
            if word.chars().count() < cfg.min_word_len {
                continue;
            }
            // Skip pure-number + non-alphabetic tokens.
            if !word.chars().any(|c| c.is_alphabetic()) {
                continue;
            }
            let stem = stem_of(word);
            if stem.is_empty() || stop_set.contains(&stem) {
                continue;
            }
            occ.entry(stem).or_default().push(Occurrence {
                para: p_idx,
                surface: word.to_lowercase(),
            });
        }
    }

    let mut findings: Vec<EchoFinding> = Vec::new();
    for (stem, occs) in occ {
        let total = occs.len();
        // Must repeat, but not be common vocabulary.
        if total < cfg.min_repeats || total > cfg.max_global {
            continue;
        }
        if let Some(f) = densest_window(&stem, &occs, cfg) {
            findings.push(f);
        }
    }

    // Deterministic order: by first paragraph, then stem.
    findings.sort_by(|a, b| {
        a.para_start
            .cmp(&b.para_start)
            .then_with(|| a.stem.cmp(&b.stem))
    });
    findings
}

/// Find the densest window of `cfg.window` consecutive
/// paragraphs containing `≥ min_repeats` of `occs`.
/// Returns the window with the most occurrences (earliest
/// on ties), or `None` when no window qualifies.
///
/// `occs` is in paragraph order (occurrences are pushed in
/// reading order).
fn densest_window(
    stem: &str,
    occs: &[Occurrence],
    cfg: &EchoConfig,
) -> Option<EchoFinding> {
    let mut best: Option<(usize, usize, usize)> = None; // (count, i, j)
    let mut j = 0;
    for i in 0..occs.len() {
        if j < i {
            j = i;
        }
        // Extend j while within the window span.
        while j + 1 < occs.len()
            && occs[j + 1].para.saturating_sub(occs[i].para) < cfg.window
        {
            j += 1;
        }
        let count = j - i + 1;
        if count >= cfg.min_repeats {
            match best {
                Some((bc, _, _)) if count <= bc => {}
                _ => best = Some((count, i, j)),
            }
        }
    }
    let (count, i, j) = best?;
    let mut surface_forms: Vec<String> = Vec::new();
    for o in &occs[i..=j] {
        if !surface_forms.contains(&o.surface) {
            surface_forms.push(o.surface.clone());
        }
    }
    Some(EchoFinding {
        stem: stem.to_string(),
        surface_forms,
        para_start: occs[i].para + 1,
        para_end: occs[j].para + 1,
        count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> EchoConfig {
        EchoConfig::default()
    }

    fn paras(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // ── English: inflected forms collapse via stemmer ──

    #[test]
    fn flags_inflected_echo_in_english() {
        let p = paras(&[
            "She walked to the window slowly.",
            "He was walking across the room.",
            "They walked out together at last.",
        ]);
        let f = detect_echoes(&p, "english", None, &cfg());
        assert_eq!(f.len(), 1, "expected one echo, got {f:?}");
        assert_eq!(f[0].stem, "walk");
        assert_eq!(f[0].count, 3);
        assert_eq!(f[0].para_start, 1);
        assert_eq!(f[0].para_end, 3);
        // walked / walking both seen.
        assert!(f[0].surface_forms.contains(&"walked".to_string()));
        assert!(f[0].surface_forms.contains(&"walking".to_string()));
    }

    // ── Russian: noun-case declension collapses ───────

    #[test]
    fn flags_inflected_echo_in_russian() {
        // "корабль" (ship) in three cases — nominative,
        // genitive, prepositional — all stem to `корабл`.
        // (Noun declension collapses reliably under
        // Snowball; some verb past-tense forms don't, a
        // documented stemming limitation — lemmatisation
        // would, stemming doesn't.)
        let p = paras(&[
            "На горизонте показался корабль.",
            "Капитан корабля молчал долго.",
            "На корабле горели три фонаря.",
        ]);
        let f = detect_echoes(&p, "russian", None, &cfg());
        assert_eq!(f.len(), 1, "expected one Russian echo, got {f:?}");
        assert_eq!(f[0].count, 3);
        // Three distinct surface forms collapsed to one stem.
        assert_eq!(f[0].surface_forms.len(), 3);
    }

    // ── exact-form fallback for non-Snowball languages ──

    #[test]
    fn unsupported_language_uses_exact_forms() {
        // "japanese" has no Snowball algorithm — so only
        // identical surface forms collapse.  Three
        // identical tokens still flag.
        let p = paras(&[
            "the artefact glimmered faintly here",
            "the artefact rested on the table",
            "the artefact vanished by morning",
        ]);
        let f = detect_echoes(&p, "japanese", None, &cfg());
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].stem, "artefact");
        assert_eq!(f[0].surface_forms, vec!["artefact".to_string()]);
    }

    // ── stop-words excluded ────────────────────────────

    #[test]
    fn stop_words_not_flagged() {
        // "the" appears many times but is a stop-word.
        let p = paras(&[
            "the cat sat on the mat",
            "the dog ran by the fence",
            "the bird flew over the hill",
        ]);
        let f = detect_echoes(&p, "english", None, &cfg());
        assert!(
            !f.iter().any(|e| e.stem == "the"),
            "stop-word 'the' must not be flagged",
        );
    }

    // ── distinctiveness ceiling ────────────────────────

    #[test]
    fn common_vocabulary_above_ceiling_skipped() {
        // "star" appears 5×; with max_global = 4 it's
        // treated as common vocabulary, not an echo.
        let p = paras(&[
            "star star",
            "star star",
            "star",
        ]);
        let mut c = cfg();
        c.max_global = 4;
        let f = detect_echoes(&p, "english", None, &c);
        assert!(f.is_empty(), "above-ceiling word should skip, got {f:?}");
    }

    // ── window boundary ────────────────────────────────

    #[test]
    fn occurrences_beyond_window_not_flagged() {
        // "lantern" 3× but spread across 9 paragraphs;
        // window = 5 → no qualifying cluster.
        let mut v = vec![
            "the lantern flickered once".to_string(),
        ];
        for i in 0..7 {
            v.push(format!("filler paragraph number {i} here"));
        }
        v.push("the lantern flickered twice".to_string());
        v.push("the lantern flickered thrice".to_string());
        // lantern at paras 0, 8, 9 — paras 8+9 are within
        // window but that's only 2 (< min_repeats 3).
        let f = detect_echoes(&v, "english", None, &cfg());
        assert!(
            !f.iter().any(|e| e.stem == "lantern"),
            "spread-out occurrences must not flag, got {f:?}",
        );
    }

    #[test]
    fn tight_cluster_within_window_flagged() {
        // Same word, all in one paragraph → window 1 span,
        // 3 occurrences → flagged.
        let p = paras(&[
            "The harbor was a harbor unlike any harbor he knew.",
        ]);
        let f = detect_echoes(&p, "english", None, &cfg());
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].stem, "harbor");
        assert_eq!(f[0].count, 3);
        assert_eq!(f[0].para_start, 1);
        assert_eq!(f[0].para_end, 1);
    }

    // ── min_repeats boundary ───────────────────────────

    #[test]
    fn below_min_repeats_not_flagged() {
        // Two occurrences, min_repeats = 3 → no flag.
        let p = paras(&[
            "the beacon shone bright",
            "the beacon shone again",
        ]);
        let f = detect_echoes(&p, "english", None, &cfg());
        assert!(!f.iter().any(|e| e.stem == "beacon"));
    }

    // ── short words skipped ────────────────────────────

    #[test]
    fn short_words_skipped() {
        // "ran" is 3 chars; min_word_len 4 → skipped even
        // though it repeats.
        let p = paras(&[
            "she ran ran ran",
        ]);
        let f = detect_echoes(&p, "english", None, &cfg());
        assert!(f.is_empty());
    }

    // ── stop-override replaces built-in ────────────────

    #[test]
    fn stop_override_replaces_builtin() {
        // Override marks "harbor" as a stop-word → it's
        // no longer flagged.
        let p = paras(&[
            "The harbor was a harbor unlike any harbor.",
        ]);
        let stop = vec!["harbor".to_string()];
        let f = detect_echoes(&p, "english", Some(&stop), &cfg());
        assert!(f.is_empty(), "overridden stop-word should suppress");
    }

    // ── multiple distinct echoes ───────────────────────

    #[test]
    fn multiple_echoes_sorted_by_paragraph() {
        let p = paras(&[
            "glimmered glimmered glimmered here",
            "nothing of note in this filler",
            "shimmered shimmered shimmered there",
        ]);
        let f = detect_echoes(&p, "english", None, &cfg());
        assert_eq!(f.len(), 2);
        // Sorted by first paragraph.
        assert_eq!(f[0].stem, "glimmer");
        assert_eq!(f[0].para_start, 1);
        assert_eq!(f[1].stem, "shimmer");
        assert_eq!(f[1].para_start, 3);
    }

    #[test]
    fn empty_input_no_findings() {
        assert!(detect_echoes(&[], "english", None, &cfg()).is_empty());
        assert!(
            detect_echoes(&paras(&["", "  "]), "english", None, &cfg())
                .is_empty()
        );
    }
}

