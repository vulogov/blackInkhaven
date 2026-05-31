//! 1.2.14+ Phase C.1 — inline comments on paragraph
//! prose.  Sidecar JSON storage adjacent to the
//! `.typ` file so comments travel with the
//! paragraph in git and diff cleanly across
//! commits.
//!
//! Character-offset spans (not byte) so UTF-8
//! boundary edits don't corrupt anchoring.  Stable
//! per-comment UUIDs so a referenced comment in
//! an AI digest or beta-reader export survives
//! author edits to the surrounding text.
//!
//! Sidecar shape (1.2.14 schema v1):
//!
//! ```json
//! {
//!   "schema_version": 1,
//!   "comments": [
//!     {
//!       "id": "01939c2a-...",
//!       "char_start": 412,
//!       "char_end": 487,
//!       "author": "vladimir",
//!       "created_at": "2026-06-01T10:23:00Z",
//!       "resolved": false,
//!       "resolved_at": null,
//!       "text": "Does this sentence land the redemption beat?",
//!       "replies": []
//!     }
//!   ]
//! }
//! ```
//!
//! Phase C.1 ships add + render + footer chip.
//! Phase C.2 adds the comments panel (`Ctrl+V
//! Shift+C`), resolve / unresolve, AI digest, and
//! the CLI surface.  See
//! `Documentation/PROPOSALS/1.2.14_PLAN.md` §4.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One inline comment anchored to a character span
/// in a paragraph's prose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: Uuid,
    /// Character offset (NOT byte) where the
    /// comment span begins.  Inclusive.
    pub char_start: usize,
    /// Character offset where the comment span
    /// ends.  Exclusive.
    pub char_end: usize,
    pub author: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub resolved: bool,
    #[serde(default)]
    pub resolved_at: Option<DateTime<Utc>>,
    pub text: String,
    /// Reserved for Phase C.2 threading.  Empty
    /// vec round-trips cleanly.
    #[serde(default)]
    pub replies: Vec<CommentReply>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentReply {
    pub id: Uuid,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub text: String,
}

/// Top-level container in the sidecar JSON file.
/// `schema_version` lets future format migrations
/// detect old files without guessing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentsFile {
    pub schema_version: u32,
    #[serde(default)]
    pub comments: Vec<Comment>,
}

impl CommentsFile {
    pub fn new() -> Self {
        Self {
            schema_version: 1,
            comments: Vec::new(),
        }
    }
}

impl Default for CommentsFile {
    fn default() -> Self {
        Self::new()
    }
}

/// 1.2.14+ Phase C.1 — derive the sidecar path
/// from the paragraph's `.typ` path.  Just swap
/// the extension: `foo.typ` → `foo.comments.json`.
/// Files without a `.typ` extension are still
/// accepted (e.g. `.hjson` or `.bund` paragraphs)
/// — we append `.comments.json` to the stem.
pub fn sidecar_path(typ_path: &Path) -> PathBuf {
    let mut out = typ_path.to_path_buf();
    let stem = out
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "paragraph".to_string());
    out.set_file_name(format!("{stem}.comments.json"));
    out
}

/// 1.2.14+ Phase C.1 — load comments for a
/// paragraph.  Returns an empty `CommentsFile`
/// when the sidecar doesn't exist (the common
/// case — most paragraphs carry no comments).
/// Errors only on malformed JSON, which surfaces
/// up so the editor can show a diagnostic
/// instead of silently dropping the user's
/// comment history.
pub fn load_from_sidecar(typ_abs_path: &Path) -> Result<CommentsFile, String> {
    let path = sidecar_path(typ_abs_path);
    if !path.exists() {
        return Ok(CommentsFile::new());
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;
    if raw.trim().is_empty() {
        return Ok(CommentsFile::new());
    }
    serde_json::from_str(&raw)
        .map_err(|e| format!("parse {}: {e}", path.display()))
}

/// 1.2.14+ Phase C.1 — persist comments for a
/// paragraph.  Writes pretty JSON for git-friendly
/// diffs.  Removes the sidecar entirely when the
/// comments list is empty (avoid littering the
/// project with empty JSON files).
pub fn save_to_sidecar(
    typ_abs_path: &Path,
    file: &CommentsFile,
) -> Result<(), String> {
    let path = sidecar_path(typ_abs_path);
    if file.comments.is_empty() {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| {
                format!("remove empty sidecar {}: {e}", path.display())
            })?;
        }
        return Ok(());
    }
    let raw = serde_json::to_string_pretty(file)
        .map_err(|e| format!("serialise: {e}"))?;
    std::fs::write(&path, raw)
        .map_err(|e| format!("write {}: {e}", path.display()))
}

/// 1.2.14+ Phase C.1 — resolve the comment
/// author.  Priority:
///   1. `editor.comment_author` HJSON field if set
///      (project-level override).
///   2. `$USER` env var.
///   3. `$LOGNAME` env var.
///   4. Hostname via `hostname` env.
///   5. `"anonymous"` fallback.
pub fn resolve_author(configured: Option<&str>) -> String {
    if let Some(s) = configured {
        let t = s.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    for var in ["USER", "LOGNAME", "HOSTNAME"] {
        if let Ok(v) = std::env::var(var) {
            let t = v.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    "anonymous".to_string()
}

/// 1.2.14+ Phase C.1 — convert a global character
/// offset within the paragraph body to (row, col)
/// in the textarea's `lines()` view.  Returns
/// `None` when the offset is past end-of-buffer.
/// Counting matches `tui-textarea`'s convention:
/// each line is one row, `\n` separators count as
/// one character between rows.
pub fn char_offset_to_row_col(
    lines: &[String],
    offset: usize,
) -> Option<(usize, usize)> {
    let mut consumed = 0usize;
    for (row, line) in lines.iter().enumerate() {
        let line_chars = line.chars().count();
        if offset <= consumed + line_chars {
            return Some((row, offset - consumed));
        }
        // +1 for the newline between this line and
        // the next.
        consumed += line_chars + 1;
    }
    None
}

/// 1.2.14+ Phase C.1 — convert a (row, col) cursor
/// position to a global character offset.  Inverse
/// of `char_offset_to_row_col`; used by the `Ctrl+V
/// c` handler to translate the textarea cursor /
/// selection range into sidecar-storage
/// coordinates.
pub fn row_col_to_char_offset(
    lines: &[String],
    row: usize,
    col: usize,
) -> usize {
    let mut offset = 0usize;
    for (r, line) in lines.iter().enumerate() {
        if r == row {
            // Cap at the line's character count so a
            // col past EOL still produces a sensible
            // offset.
            return offset + col.min(line.chars().count());
        }
        offset += line.chars().count() + 1;
    }
    offset
}

/// 1.2.14+ Phase C.1 — derive a comment span from
/// the textarea's cursor / selection state.  When
/// a selection exists, the comment anchors to that
/// range.  When no selection, anchors to the word
/// at the cursor (Unicode word boundaries).
/// Returns `None` only when the cursor is on a
/// blank line outside any word AND no selection
/// is active — caller should surface a friendly
/// error in that case.
pub fn derive_anchor_span(
    lines: &[String],
    cursor: (usize, usize),
    selection: Option<((usize, usize), (usize, usize))>,
) -> Option<(usize, usize)> {
    if let Some(((r1, c1), (r2, c2))) = selection {
        let start = row_col_to_char_offset(lines, r1, c1);
        let end = row_col_to_char_offset(lines, r2, c2);
        if start != end {
            // tui-textarea may swap start/end
            // depending on selection direction —
            // normalise.
            let (s, e) = if start < end { (start, end) } else { (end, start) };
            return Some((s, e));
        }
    }
    // No selection — anchor to the word at the
    // cursor.  Find word boundaries on the cursor's
    // line by walking outward.
    let (cur_row, cur_col) = cursor;
    let line = lines.get(cur_row)?;
    let chars: Vec<char> = line.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let cur_col = cur_col.min(chars.len());
    // Walk left while the char before is
    // word-character-y.
    let is_word_char = |c: char| c.is_alphanumeric() || c == '_' || c == '\'';
    let mut start = cur_col;
    while start > 0 && is_word_char(chars[start - 1]) {
        start -= 1;
    }
    let mut end = cur_col;
    while end < chars.len() && is_word_char(chars[end]) {
        end += 1;
    }
    if start == end {
        return None;
    }
    let abs_start = row_col_to_char_offset(lines, cur_row, start);
    let abs_end = row_col_to_char_offset(lines, cur_row, end);
    Some((abs_start, abs_end))
}

/// 1.2.14+ Phase C.1 — for each editor row, return
/// the (col_start, col_end, comment_index) tuples
/// of comment spans that intersect that row.
/// `col_end == usize::MAX` signals the span
/// continues past the visible row width — the
/// renderer clamps.
pub fn per_row_hits(
    lines: &[String],
    comments: &[Comment],
) -> Vec<Vec<RowHit>> {
    let mut out: Vec<Vec<RowHit>> = vec![Vec::new(); lines.len()];
    for (idx, c) in comments.iter().enumerate() {
        let Some((r_start, c_start)) =
            char_offset_to_row_col(lines, c.char_start)
        else {
            continue;
        };
        // Span end maps to (row, col) AT the end
        // boundary; if the offset is exactly the
        // newline between rows, the span ends at
        // EOL of `r_start`.
        let (r_end, c_end) = match char_offset_to_row_col(lines, c.char_end) {
            Some(p) => p,
            None => {
                // Offset past end-of-buffer — clamp
                // to the last line's EOL.
                let last_row = lines.len().saturating_sub(1);
                let last_col = lines
                    .last()
                    .map(|l| l.chars().count())
                    .unwrap_or(0);
                (last_row, last_col)
            }
        };
        for row in r_start..=r_end.min(lines.len().saturating_sub(1)) {
            let line_chars = lines[row].chars().count();
            let start_col = if row == r_start { c_start } else { 0 };
            let end_col = if row == r_end { c_end } else { line_chars };
            if start_col >= end_col {
                continue;
            }
            out[row].push(RowHit {
                col_start: start_col,
                col_end: end_col,
                comment_idx: idx,
                resolved: c.resolved,
            });
        }
    }
    out
}

#[derive(Debug, Clone, Copy)]
pub struct RowHit {
    pub col_start: usize,
    pub col_end: usize,
    /// Index into the paragraph's
    /// `CommentsFile.comments` vector — used by
    /// Phase C.2 panel navigation to map a cell
    /// back to its source comment.
    #[allow(dead_code)]
    pub comment_idx: usize,
    pub resolved: bool,
}

/// 1.2.14+ Phase C.1 — find the comment whose
/// span contains the cursor position (if any).
/// Used by the editor footer to surface the
/// comment text when the cursor lands inside a
/// commented span.  Returns the first matching
/// index — overlapping comments tie-break by
/// insertion order.
pub fn comment_at_cursor(
    lines: &[String],
    comments: &[Comment],
    cursor: (usize, usize),
) -> Option<usize> {
    let cursor_offset = row_col_to_char_offset(lines, cursor.0, cursor.1);
    comments.iter().position(|c| {
        cursor_offset >= c.char_start && cursor_offset < c.char_end
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_lines(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| (*x).to_string()).collect()
    }

    #[test]
    fn sidecar_path_swaps_extension() {
        let p = Path::new("/proj/books/manuscript/chapter-1/03-rain.typ");
        assert_eq!(
            sidecar_path(p).to_string_lossy(),
            "/proj/books/manuscript/chapter-1/03-rain.comments.json"
        );
    }

    #[test]
    fn sidecar_path_handles_non_typ_extension() {
        let p = Path::new("/proj/x/note.hjson");
        assert_eq!(
            sidecar_path(p).to_string_lossy(),
            "/proj/x/note.comments.json"
        );
    }

    #[test]
    fn char_offset_to_row_col_basic() {
        let lines = mk_lines(&["hello", "world"]);
        // "hello\n" = chars 0..5 then newline = char 5;
        // "world" starts at char 6.
        assert_eq!(char_offset_to_row_col(&lines, 0), Some((0, 0)));
        assert_eq!(char_offset_to_row_col(&lines, 5), Some((0, 5)));
        assert_eq!(char_offset_to_row_col(&lines, 6), Some((1, 0)));
        assert_eq!(char_offset_to_row_col(&lines, 11), Some((1, 5)));
        assert_eq!(char_offset_to_row_col(&lines, 12), None);
    }

    #[test]
    fn row_col_roundtrip() {
        let lines = mk_lines(&["hello", "world", "tira"]);
        for (r, c) in
            [(0, 0), (0, 3), (1, 0), (1, 5), (2, 4)]
        {
            let off = row_col_to_char_offset(&lines, r, c);
            assert_eq!(char_offset_to_row_col(&lines, off), Some((r, c)));
        }
    }

    #[test]
    fn char_offset_handles_unicode() {
        // Cyrillic is 2 bytes per char in UTF-8 but
        // one CHARACTER each.  Our offsets are
        // character-based so the math is the same as
        // ASCII.
        let lines = mk_lines(&["Москва", "Питер"]);
        assert_eq!(char_offset_to_row_col(&lines, 6), Some((0, 6)));
        assert_eq!(char_offset_to_row_col(&lines, 7), Some((1, 0)));
        assert_eq!(char_offset_to_row_col(&lines, 11), Some((1, 4)));
    }

    #[test]
    fn derive_anchor_uses_selection_when_present() {
        let lines = mk_lines(&["hello world", "foo bar"]);
        // Select "world" via ((0, 6), (0, 11)).
        let span = derive_anchor_span(
            &lines,
            (0, 11),
            Some(((0, 6), (0, 11))),
        );
        assert_eq!(span, Some((6, 11)));
    }

    #[test]
    fn derive_anchor_normalises_reverse_selection() {
        let lines = mk_lines(&["hello world"]);
        // Selection captured backward (end before start).
        let span = derive_anchor_span(
            &lines,
            (0, 0),
            Some(((0, 11), (0, 6))),
        );
        assert_eq!(span, Some((6, 11)));
    }

    #[test]
    fn derive_anchor_falls_back_to_word_at_cursor() {
        let lines = mk_lines(&["hello world here"]);
        // Cursor in the middle of "world".
        let span = derive_anchor_span(&lines, (0, 8), None);
        assert_eq!(span, Some((6, 11)));
    }

    #[test]
    fn derive_anchor_returns_none_on_blank() {
        let lines = mk_lines(&["", "   ", ""]);
        assert!(derive_anchor_span(&lines, (0, 0), None).is_none());
        assert!(derive_anchor_span(&lines, (2, 0), None).is_none());
    }

    fn mk_comment(start: usize, end: usize) -> Comment {
        Comment {
            id: Uuid::nil(),
            char_start: start,
            char_end: end,
            author: "test".into(),
            created_at: Utc::now(),
            resolved: false,
            resolved_at: None,
            text: "test comment".into(),
            replies: Vec::new(),
        }
    }

    #[test]
    fn per_row_hits_single_line_span() {
        let lines = mk_lines(&["hello world"]);
        let cs = vec![mk_comment(6, 11)];
        let hits = per_row_hits(&lines, &cs);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].len(), 1);
        assert_eq!(hits[0][0].col_start, 6);
        assert_eq!(hits[0][0].col_end, 11);
        assert!(!hits[0][0].resolved);
    }

    #[test]
    fn per_row_hits_propagates_resolved_flag() {
        let lines = mk_lines(&["hello world"]);
        let mut c = mk_comment(6, 11);
        c.resolved = true;
        let hits = per_row_hits(&lines, &[c]);
        assert!(hits[0][0].resolved);
    }

    #[test]
    fn per_row_hits_multiline_span() {
        let lines = mk_lines(&["hello world", "foo bar baz"]);
        // "hello world" is chars 0..11, newline is
        // char 11, "foo bar baz" is chars 12..23.
        // Span (6, 19) covers "world\nfoo bar".
        // End char 19 - 12 = col 7 on row 1
        // (exclusive end → highlight stops at the
        // space after "bar").
        let cs = vec![mk_comment(6, 19)];
        let hits = per_row_hits(&lines, &cs);
        assert_eq!(hits[0].len(), 1);
        assert_eq!(hits[0][0].col_start, 6);
        assert_eq!(hits[0][0].col_end, 11);
        assert_eq!(hits[1].len(), 1);
        assert_eq!(hits[1][0].col_start, 0);
        assert_eq!(hits[1][0].col_end, 7);
    }

    #[test]
    fn comment_at_cursor_finds_containing_comment() {
        let lines = mk_lines(&["hello world here"]);
        let cs = vec![mk_comment(6, 11)];
        assert_eq!(comment_at_cursor(&lines, &cs, (0, 8)), Some(0));
        assert_eq!(comment_at_cursor(&lines, &cs, (0, 6)), Some(0));
        // 11 is exclusive — cursor at col 11 is past
        // the span.
        assert_eq!(comment_at_cursor(&lines, &cs, (0, 11)), None);
        assert_eq!(comment_at_cursor(&lines, &cs, (0, 3)), None);
    }

    #[test]
    fn resolve_author_uses_configured_when_set() {
        assert_eq!(resolve_author(Some("vladimir")), "vladimir");
        // Whitespace-only configured value falls
        // through to env vars.
        assert_eq!(resolve_author(Some("  ")).len() > 0, true);
    }

    #[test]
    fn comments_file_roundtrip() {
        let mut f = CommentsFile::new();
        f.comments.push(mk_comment(0, 5));
        let raw = serde_json::to_string(&f).unwrap();
        let back: CommentsFile = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.schema_version, 1);
        assert_eq!(back.comments.len(), 1);
    }
}
