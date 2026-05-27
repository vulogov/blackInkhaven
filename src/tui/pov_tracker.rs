//! 1.2.9+ — POV / character tracker for the status bar.
//!
//! Walks the open paragraph, extracts every lexicon hit
//! categorised as `Character`, and ranks the cast by
//! mention count (ties broken by first-mention order).
//! The top name surfaces in the status bar as the
//! presumed POV character; up to three additional names
//! trail behind as the supporting cast.
//!
//! Heuristic rationale:
//!
//!   * In third-person limited prose, the POV character
//!     is almost always the most-frequently-named entity
//!     in the scene — the narrator's gaze inherently
//!     centers them.  First-person POV is a degenerate
//!     case (the narrator is `I`, who isn't in the
//!     character lexicon) — for those scenes the chip
//!     correctly surfaces the *other* prominent character,
//!     which is still useful context.
//!
//!   * Ties broken by first-mention preserve the
//!     "scene-opening character" convention — when two
//!     characters appear equally often, the one named
//!     first in the paragraph is usually the anchor.
//!
//! The function operates on already-computed
//! `Vec<Vec<LexHit>>` so the caller can reuse the
//! lexicon scan it's already doing for syntax-highlight
//! purposes if it wants to.  `compute_pov_chip` is the
//! convenience entry point that runs `lexicon.row_hits`
//! per line.

use std::collections::HashMap;

use crate::tui::lexicon::{LexCategory, LexHit, Lexicon};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PovChip {
    /// The presumed POV character — most-mentioned in
    /// the open paragraph.
    pub pov: String,
    /// Other named characters present, in
    /// most-mentioned-first order, capped at 3 entries
    /// so the chip stays compact in the status bar.
    pub supporting: Vec<String>,
}

/// Convenience entry point — runs the lexicon scan per
/// row and feeds the hits into `compute_pov_chip_from_hits`.
/// Cheap for normal paragraph sizes (<1k words); the
/// per-row scan is the same work the syntax-highlight
/// pass already performs, so calling this on every
/// status-bar repaint adds only milliseconds.
pub fn compute_pov_chip(lex: &Lexicon, lines: &[String]) -> Option<PovChip> {
    if lex.is_empty() || lines.is_empty() {
        return None;
    }
    let hits_per_row: Vec<Vec<LexHit>> =
        lines.iter().map(|l| lex.row_hits(l)).collect();
    compute_pov_chip_from_hits(&hits_per_row, lines)
}

/// Same as `compute_pov_chip`, but consumes pre-computed
/// per-row hits.  Exposed separately so tests can drive
/// the ranking logic without constructing a Lexicon.
pub fn compute_pov_chip_from_hits(
    hits_per_row: &[Vec<LexHit>],
    lines: &[String],
) -> Option<PovChip> {
    if hits_per_row.is_empty() {
        return None;
    }
    // Track per-name: count + first-mention sequence.
    // The sequence is the index of the first hit across
    // the whole paragraph (row-major), used as the tie-
    // breaker for count.
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut first_seen: HashMap<String, usize> = HashMap::new();
    // The canonical (display) form: keep the first-
    // encountered surface form so the chip shows
    // "Anna" not "anna" even if later mentions are
    // lowercase.
    let mut display: HashMap<String, String> = HashMap::new();

    let mut seq: usize = 0;
    for (row_idx, row_hits) in hits_per_row.iter().enumerate() {
        let line = match lines.get(row_idx) {
            Some(l) => l,
            None => continue,
        };
        let chars: Vec<char> = line.chars().collect();
        for hit in row_hits {
            if !matches!(hit.category, LexCategory::Character) {
                continue;
            }
            let start = hit.col_start.min(chars.len());
            let end = hit.col_end.min(chars.len());
            if end <= start {
                continue;
            }
            let surface: String = chars[start..end].iter().collect();
            let surface_trim = surface.trim();
            if surface_trim.is_empty() {
                continue;
            }
            let key = surface_trim.to_lowercase();
            *counts.entry(key.clone()).or_insert(0) += 1;
            first_seen.entry(key.clone()).or_insert(seq);
            display
                .entry(key.clone())
                .or_insert_with(|| surface_trim.to_string());
            seq += 1;
        }
    }

    if counts.is_empty() {
        return None;
    }
    let mut ranked: Vec<(String, usize, usize)> = counts
        .iter()
        .map(|(k, c)| {
            let f = first_seen.get(k).copied().unwrap_or(usize::MAX);
            (k.clone(), *c, f)
        })
        .collect();
    // Sort by count desc, ties by first_seen asc.
    ranked.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| a.2.cmp(&b.2))
    });
    let mut iter = ranked.into_iter();
    let (pov_key, _, _) = iter.next()?;
    let pov_display = display
        .get(&pov_key)
        .cloned()
        .unwrap_or(pov_key);
    let supporting: Vec<String> = iter
        .take(3)
        .map(|(k, _, _)| display.get(&k).cloned().unwrap_or(k))
        .collect();
    Some(PovChip {
        pov: pov_display,
        supporting,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(col_start: usize, col_end: usize) -> LexHit {
        LexHit {
            col_start,
            col_end,
            category: LexCategory::Character,
        }
    }
    fn place_hit(col_start: usize, col_end: usize) -> LexHit {
        LexHit {
            col_start,
            col_end,
            category: LexCategory::Place,
        }
    }

    #[test]
    fn no_lines_no_chip() {
        assert!(compute_pov_chip_from_hits(&[], &[]).is_none());
    }

    #[test]
    fn no_character_hits_no_chip() {
        let lines = vec!["Anna walked".to_string()];
        let hits: Vec<Vec<LexHit>> = vec![vec![place_hit(0, 4)]];
        assert!(compute_pov_chip_from_hits(&hits, &lines).is_none());
    }

    #[test]
    fn single_character_wins() {
        let lines = vec!["Anna walked away".to_string()];
        let hits: Vec<Vec<LexHit>> = vec![vec![hit(0, 4)]];
        let chip = compute_pov_chip_from_hits(&hits, &lines).unwrap();
        assert_eq!(chip.pov, "Anna");
        assert!(chip.supporting.is_empty());
    }

    #[test]
    fn most_mentioned_wins() {
        // Bob appears 3x, Anna once → POV = Bob.
        let lines = vec![
            "Anna saw Bob".to_string(),
            "Bob smiled".to_string(),
            "Bob left".to_string(),
        ];
        let hits: Vec<Vec<LexHit>> = vec![
            vec![hit(0, 4), hit(9, 12)],   // Anna 0..4, Bob 9..12
            vec![hit(0, 3)],                // Bob 0..3
            vec![hit(0, 3)],                // Bob 0..3
        ];
        let chip = compute_pov_chip_from_hits(&hits, &lines).unwrap();
        assert_eq!(chip.pov, "Bob");
        assert_eq!(chip.supporting, vec!["Anna".to_string()]);
    }

    #[test]
    fn ties_broken_by_first_mention() {
        // Anna and Bob each mentioned once.  Anna first
        // → POV = Anna.
        let lines = vec!["Anna saw Bob".to_string()];
        let hits: Vec<Vec<LexHit>> = vec![vec![hit(0, 4), hit(9, 12)]];
        let chip = compute_pov_chip_from_hits(&hits, &lines).unwrap();
        assert_eq!(chip.pov, "Anna");
        assert_eq!(chip.supporting, vec!["Bob".to_string()]);
    }

    #[test]
    fn supporting_cast_capped_at_three() {
        // Five distinct characters, each mentioned once.
        let line = "Anna Bob Carol Dave Eve".to_string();
        // Char positions: Anna 0..4, Bob 5..8, Carol 9..14, Dave 15..19, Eve 20..23
        let hits: Vec<Vec<LexHit>> = vec![vec![
            hit(0, 4),
            hit(5, 8),
            hit(9, 14),
            hit(15, 19),
            hit(20, 23),
        ]];
        let lines = vec![line];
        let chip = compute_pov_chip_from_hits(&hits, &lines).unwrap();
        // POV = Anna (first-mention tiebreak), supporting
        // = Bob, Carol, Dave (Eve drops off the cap).
        assert_eq!(chip.pov, "Anna");
        assert_eq!(
            chip.supporting,
            vec!["Bob".to_string(), "Carol".to_string(), "Dave".to_string()]
        );
    }

    #[test]
    fn case_normalised_for_count_display_preserves_first() {
        // "Anna" then "anna" then "ANNA" — same character,
        // count = 3, display = first surface = "Anna".
        let lines = vec![
            "Anna laughed".to_string(),
            "Then anna sighed".to_string(),
            "ANNA stood up".to_string(),
        ];
        let hits: Vec<Vec<LexHit>> = vec![
            vec![hit(0, 4)],
            vec![hit(5, 9)],
            vec![hit(0, 4)],
        ];
        let chip = compute_pov_chip_from_hits(&hits, &lines).unwrap();
        assert_eq!(chip.pov, "Anna");
    }

    #[test]
    fn non_character_hits_ignored() {
        // Mix of Place + Character hits — only characters
        // count for POV.
        let lines = vec!["Anna entered Winterfell".to_string()];
        // Anna 0..4 (Character), Winterfell 13..23 (Place)
        let hits: Vec<Vec<LexHit>> =
            vec![vec![hit(0, 4), place_hit(13, 23)]];
        let chip = compute_pov_chip_from_hits(&hits, &lines).unwrap();
        assert_eq!(chip.pov, "Anna");
        assert!(chip.supporting.is_empty());
    }
}
