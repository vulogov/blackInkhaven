//! In-buffer regex search + replace for the editor pane.
//!
//! Pattern matching is per-line — no cross-line matches in this iteration.
//! Use `(?i)` for case-insensitive, `(?s)` for dotall, etc. (full Rust
//! regex syntax).

use regex::Regex;

/// One regex hit, in source coordinates (char indices, not bytes).
#[derive(Debug, Clone)]
pub struct MatchRange {
    pub row: usize,
    pub col_start: usize,
    pub col_end: usize,
}

/// Active search session attached to an open paragraph.
#[derive(Debug)]
pub struct SearchState {
    pub pattern: String,
    pub regex: Regex,
    pub matches: Vec<MatchRange>,
    /// Index into `matches` for the "current" hit (jumps target / next
    /// replace target). When `matches` is empty this is 0 by convention but
    /// callers should always check `matches.is_empty()` first.
    pub current: usize,
    /// Some(...) → replace mode. None → search-only mode.
    pub replace_with: Option<String>,
}

impl SearchState {
    /// Compile `pattern`, find every match across the given lines. Returns
    /// `Err` with the compile error message if the pattern is invalid.
    pub fn build(
        pattern: &str,
        replace_with: Option<String>,
        lines: &[String],
    ) -> Result<Self, String> {
        let regex = Regex::new(pattern).map_err(|e| e.to_string())?;
        let matches = collect_matches(&regex, lines);
        Ok(Self {
            pattern: pattern.to_string(),
            regex,
            matches,
            current: 0,
            replace_with,
        })
    }

    /// Re-find matches against an updated buffer. Used after a replacement
    /// mutates the lines so subsequent positions stay valid.
    pub fn refresh(&mut self, lines: &[String]) {
        self.matches = collect_matches(&self.regex, lines);
        if self.current >= self.matches.len() {
            self.current = 0;
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }

    pub fn current_match(&self) -> Option<&MatchRange> {
        self.matches.get(self.current)
    }

    /// Move the cursor to the next match (wraps around). Caller is
    /// responsible for actually jumping the textarea cursor.
    pub fn advance(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        self.current = (self.current + 1) % self.matches.len();
    }

    /// A3 — move to the previous match (wraps). Mirror of [`advance`].
    pub fn retreat(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        self.current = (self.current + self.matches.len() - 1) % self.matches.len();
    }

    /// A3 — set `current` to the first match at or after `cursor` (row, col),
    /// wrapping to the first match when the cursor is past the last one. So a
    /// fresh find jumps FORWARD from where the cursor is, not to the document top.
    pub fn seed_from_cursor(&mut self, cursor: (usize, usize)) {
        if self.matches.is_empty() {
            return;
        }
        self.current = self
            .matches
            .iter()
            .position(|m| (m.row, m.col_start) >= cursor)
            .unwrap_or(0);
    }
}

fn collect_matches(regex: &Regex, lines: &[String]) -> Vec<MatchRange> {
    let mut out = Vec::new();
    for (row, line) in lines.iter().enumerate() {
        for m in regex.find_iter(line) {
            // Convert byte offsets to char offsets so we line up with the
            // editor's char-based cursor and span builder.
            let col_start = line[..m.start()].chars().count();
            let col_end = line[..m.end()].chars().count();
            // Skip zero-width matches; they'd loop forever in advance().
            if col_end > col_start {
                out.push(MatchRange {
                    row,
                    col_start,
                    col_end,
                });
            }
        }
    }
    out
}

/// Matches that fall on a given source row, with a flag for whether each
/// range is the "current" hit. Built per-row from `SearchState` for the
/// renderer to consume.
#[derive(Debug, Clone, Copy)]
pub struct RowMatch {
    pub col_start: usize,
    pub col_end: usize,
    pub is_current: bool,
}

pub fn row_matches(state: &SearchState, row: usize) -> Vec<RowMatch> {
    state
        .matches
        .iter()
        .enumerate()
        .filter(|(_, m)| m.row == row)
        .map(|(i, m)| RowMatch {
            col_start: m.col_start,
            col_end: m.col_end,
            is_current: i == state.current,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_simple_matches() {
        let lines = vec![
            "the quick brown fox".to_string(),
            "the lazy dog".to_string(),
        ];
        let s = SearchState::build("the", None, &lines).unwrap();
        assert_eq!(s.matches.len(), 2);
        assert_eq!(s.matches[0].row, 0);
        assert_eq!(s.matches[0].col_start, 0);
        assert_eq!(s.matches[0].col_end, 3);
        assert_eq!(s.matches[1].row, 1);
    }

    #[test]
    fn case_insensitive_flag() {
        let lines = vec!["The cat and THE dog".to_string()];
        let s = SearchState::build("(?i)the", None, &lines).unwrap();
        assert_eq!(s.matches.len(), 2);
    }

    #[test]
    fn unicode_offsets_are_char_indices() {
        let lines = vec!["утренний рассвет".to_string()];
        let s = SearchState::build("рассвет", None, &lines).unwrap();
        assert_eq!(s.matches.len(), 1);
        // "утренний " is 9 chars; match starts at col 9
        assert_eq!(s.matches[0].col_start, 9);
        assert_eq!(s.matches[0].col_end, 16);
    }

    #[test]
    fn advance_wraps() {
        let lines = vec!["a a a".to_string()];
        let mut s = SearchState::build("a", None, &lines).unwrap();
        assert_eq!(s.current, 0);
        s.advance();
        assert_eq!(s.current, 1);
        s.advance();
        assert_eq!(s.current, 2);
        s.advance();
        assert_eq!(s.current, 0);
    }

    #[test]
    fn retreat_wraps_backwards() {
        let lines = vec!["a a a".to_string()];
        let mut s = SearchState::build("a", None, &lines).unwrap();
        assert_eq!(s.current, 0);
        s.retreat();
        assert_eq!(s.current, 2); // wraps to last
        s.retreat();
        assert_eq!(s.current, 1);
    }

    #[test]
    fn seed_from_cursor_starts_forward_not_at_top() {
        // A3 — three matches; a cursor on line 1 seeds the match on line 1, not the
        // document-top match on line 0.
        let lines = vec!["x here".to_string(), "x mid".to_string(), "x end".to_string()];
        let mut s = SearchState::build("x", None, &lines).unwrap();
        s.seed_from_cursor((1, 0));
        assert_eq!(s.current, 1);
        // A cursor past the last match wraps to the first.
        s.seed_from_cursor((9, 0));
        assert_eq!(s.current, 0);
        // A cursor before everything picks the first.
        s.seed_from_cursor((0, 0));
        assert_eq!(s.current, 0);
    }

    #[test]
    fn bad_regex_errors() {
        let lines = vec!["foo".to_string()];
        let err = SearchState::build("(unbalanced", None, &lines).unwrap_err();
        assert!(err.contains("regex"));
    }
}
