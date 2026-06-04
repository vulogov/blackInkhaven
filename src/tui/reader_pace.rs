//! 1.2.18+ R.4 — reader-pace preview.
//!
//! Drives a teleprompter-style modal (`Ctrl+B Shift+E`)
//! that advances a highlight through the open
//! paragraph's prose at the configured reading speed
//! (`editor.reading_wpm`).  Reading your own prose at a
//! reader's pace — not at editing-glance speed — surfaces
//! pacing problems (a run-on that drags, a too-abrupt
//! beat) that are invisible when you skim.
//!
//! This module holds the pure pieces: tokenising prose
//! into displayable words, and computing which word the
//! highlight should be on given the elapsed time + wpm.
//! The modal state + render + tick live on `App`.

/// Split prose into whitespace-delimited words for the
/// teleprompter.  Newlines collapse to spaces (the
/// preview is a flowing read, not a layout).  Returns an
/// empty vec for empty / whitespace-only input.
pub(crate) fn split_words(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_string).collect()
}

/// Which word the highlight should be on after
/// `elapsed_secs` at `wpm`, clamped to `[0, total]`.
///
/// `total` is the word count; the returned index can
/// equal `total` to signal "finished" (the caller stops
/// advancing + shows the end state).  `wpm == 0` or
/// `total == 0` pins to 0.
pub(crate) fn word_index_at(elapsed_secs: f64, wpm: u32, total: usize) -> usize {
    if wpm == 0 || total == 0 || elapsed_secs <= 0.0 {
        return 0;
    }
    let words_per_sec = wpm as f64 / 60.0;
    let idx = (elapsed_secs * words_per_sec).floor() as usize;
    idx.min(total)
}

/// Seconds of prose remaining from word `index` at
/// `wpm`.  Used for the modal's "N s left" footer.
pub(crate) fn remaining_secs(index: usize, total: usize, wpm: u32) -> u64 {
    if wpm == 0 {
        return 0;
    }
    let left = total.saturating_sub(index) as f64;
    (left / (wpm as f64 / 60.0)).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── split_words ───────────────────────────────────

    #[test]
    fn split_basic() {
        assert_eq!(
            split_words("the quick brown fox"),
            vec!["the", "quick", "brown", "fox"],
        );
    }

    #[test]
    fn split_collapses_whitespace_and_newlines() {
        assert_eq!(
            split_words("a\n\n  b   c"),
            vec!["a", "b", "c"],
        );
    }

    #[test]
    fn split_empty_is_empty() {
        assert!(split_words("").is_empty());
        assert!(split_words("   \n  ").is_empty());
    }

    // ── word_index_at ─────────────────────────────────

    #[test]
    fn index_zero_at_start() {
        assert_eq!(word_index_at(0.0, 200, 100), 0);
    }

    #[test]
    fn index_advances_at_wpm() {
        // 200 wpm = 200/60 ≈ 3.333 words/sec.
        // At 3s: floor(10) = 10.
        assert_eq!(word_index_at(3.0, 200, 100), 10);
        // At 1s: floor(3.333) = 3.
        assert_eq!(word_index_at(1.0, 200, 100), 3);
    }

    #[test]
    fn index_clamps_to_total() {
        // Way past the end → clamped to total (finished).
        assert_eq!(word_index_at(1000.0, 200, 50), 50);
    }

    #[test]
    fn index_zero_wpm_pins_to_start() {
        assert_eq!(word_index_at(100.0, 0, 50), 0);
    }

    #[test]
    fn index_empty_text_is_zero() {
        assert_eq!(word_index_at(10.0, 200, 0), 0);
    }

    #[test]
    fn index_negative_elapsed_is_zero() {
        assert_eq!(word_index_at(-5.0, 200, 100), 0);
    }

    // ── remaining_secs ────────────────────────────────

    #[test]
    fn remaining_full_at_start() {
        // 200 words at 200 wpm = 60s.
        assert_eq!(remaining_secs(0, 200, 200), 60);
    }

    #[test]
    fn remaining_zero_at_end() {
        assert_eq!(remaining_secs(200, 200, 200), 0);
    }

    #[test]
    fn remaining_halfway() {
        // 100 of 200 words left at 200 wpm = 30s.
        assert_eq!(remaining_secs(100, 200, 200), 30);
    }

    #[test]
    fn remaining_zero_wpm_is_zero() {
        assert_eq!(remaining_secs(0, 200, 0), 0);
    }
}
