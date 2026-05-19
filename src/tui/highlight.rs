//! Tree-sitter-driven syntax highlighting for the editor pane.
//!
//! The widget we use (`tui_textarea::TextArea`) has zero public hooks for
//! per-token coloring (its span builder is `pub(crate)`), so we use it
//! purely as a state model — lines, cursor, selection, undo — and drive
//! rendering ourselves with the data tree-sitter-highlight produces.

use ratatui::style::{Color, Modifier, Style};
use tree_sitter_highlight::{
    HighlightConfiguration, HighlightEvent, Highlighter as TsHighlighter,
};

/// Highlight names registered with tree-sitter-highlight. Order matters: when
/// a query captures a name like `@markup.heading.1`, tree-sitter-highlight
/// uses the longest prefix match against this list. So `markup.heading.1`
/// must appear before `markup.heading`, which must appear before `markup`.
const HIGHLIGHT_NAMES: &[&str] = &[
    "constant.numeric",
    "constant.character.escape",
    "constant.character",
    "constant.builtin.boolean",
    "constant.builtin",
    "constant",
    "string",
    "function.method",
    "function",
    "keyword.control.conditional",
    "keyword.control.repeat",
    "keyword.control.import",
    "keyword.control",
    "keyword.storage.type",
    "keyword.operator",
    "keyword",
    "operator",
    "tag",
    "variable",
    "markup.heading.marker",
    "markup.heading.1",
    "markup.heading.2",
    "markup.heading.3",
    "markup.heading.4",
    "markup.heading.5",
    "markup.heading.6",
    "markup.heading",
    "markup.bold",
    "markup.italic",
    "markup.quote",
    "markup.raw.block",
    "markup.raw",
    "markup.list",
    "comment",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation",
];

fn style_for_name(name: &str) -> Style {
    match name {
        "comment" => Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),

        "string" => Style::default().fg(Color::Green),

        "constant.numeric" => Style::default().fg(Color::Magenta),
        "constant.character.escape" => Style::default().fg(Color::Cyan),
        "constant.character" => Style::default().fg(Color::Yellow),
        "constant.builtin.boolean" | "constant.builtin" | "constant" => {
            Style::default().fg(Color::Magenta)
        }

        "function" | "function.method" => Style::default().fg(Color::Yellow),

        "keyword.control.conditional" | "keyword.control.repeat" | "keyword.control.import"
        | "keyword.control" | "keyword.storage.type" | "keyword" => Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
        "keyword.operator" => Style::default().fg(Color::Magenta),
        "operator" => Style::default().fg(Color::Cyan),

        "tag" => Style::default().fg(Color::Blue),
        "variable" => Style::default(),

        "markup.heading.marker" => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::DIM),
        "markup.heading.1" => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        "markup.heading.2" => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        "markup.heading.3" | "markup.heading.4" | "markup.heading.5" | "markup.heading.6"
        | "markup.heading" => Style::default().fg(Color::Cyan),
        "markup.bold" => Style::default().add_modifier(Modifier::BOLD),
        "markup.italic" => Style::default().add_modifier(Modifier::ITALIC),
        "markup.quote" => Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
        "markup.raw.block" | "markup.raw" => {
            Style::default().fg(Color::LightYellow).bg(Color::Reset)
        }
        "markup.list" => Style::default().fg(Color::Magenta),

        "punctuation.bracket" | "punctuation.delimiter" | "punctuation" => {
            Style::default().add_modifier(Modifier::DIM)
        }

        _ => Style::default(),
    }
}

/// A contiguous run of source characters that share a single style.
#[derive(Debug, Clone)]
pub struct StyledRun {
    pub text: String,
    pub style: Style,
}

/// Inclusive rectangular selection in source coordinates.
#[derive(Debug, Clone, Copy)]
pub struct BlockSelection {
    pub row_min: usize,
    pub row_max: usize,
    pub col_min: usize,
    pub col_max: usize,
}

impl BlockSelection {
    pub fn from_anchor_and_cursor(anchor: (usize, usize), cursor: (usize, usize)) -> Self {
        let (a_r, a_c) = anchor;
        let (c_r, c_c) = cursor;
        Self {
            row_min: a_r.min(c_r),
            row_max: a_r.max(c_r),
            col_min: a_c.min(c_c),
            col_max: a_c.max(c_c),
        }
    }

    pub fn contains(&self, row: usize, col: usize) -> bool {
        row >= self.row_min && row <= self.row_max && col >= self.col_min && col <= self.col_max
    }
}

/// A wrapped visual row: a subset of one source line's styled runs, along
/// with the source-character column at which this visual row starts. Used by
/// the word-wrap editor path so selection / cursor logic can map between
/// source and visual coordinates.
#[derive(Debug, Clone)]
pub struct VisualRow {
    pub runs: Vec<StyledRun>,
    pub src_row: usize,
    /// Character index in the source line where this visual row begins.
    pub src_col_start: usize,
    /// Total characters on this visual row (sum of run lengths).
    pub width_chars: usize,
}

/// Word-wrap one source line's runs to fit within `width` terminal columns.
/// Prefers breaking at whitespace (last space within the segment); falls back
/// to hard-breaking when a single token exceeds the width. Always returns at
/// least one row, even for an empty source line.
pub fn wrap_line(runs: &[StyledRun], src_row: usize, width: usize) -> Vec<VisualRow> {
    if width == 0 {
        return vec![VisualRow {
            runs: runs.to_vec(),
            src_row,
            src_col_start: 0,
            width_chars: runs.iter().map(|r| r.text.chars().count()).sum(),
        }];
    }

    // Flatten to (char, style) so wrap boundaries can fall mid-run.
    let chars: Vec<(char, Style)> = runs
        .iter()
        .flat_map(|r| r.text.chars().map(move |c| (c, r.style)))
        .collect();

    if chars.is_empty() {
        return vec![VisualRow {
            runs: Vec::new(),
            src_row,
            src_col_start: 0,
            width_chars: 0,
        }];
    }

    let mut out: Vec<VisualRow> = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let remaining = chars.len() - i;
        let take = remaining.min(width);
        let mut end = i + take;
        // If we didn't consume the rest of the line, try to break on a space.
        if end < chars.len() {
            if let Some(rel) = chars[i..end]
                .iter()
                .rposition(|(c, _)| c.is_whitespace())
            {
                // Break AFTER the whitespace.
                end = i + rel + 1;
            }
        }
        let segment = &chars[i..end];
        // Compress segment back into runs by merging adjacent identical styles.
        let mut row_runs: Vec<StyledRun> = Vec::new();
        for (c, style) in segment {
            if let Some(last) = row_runs.last_mut() {
                if last.style == *style {
                    last.text.push(*c);
                    continue;
                }
            }
            row_runs.push(StyledRun {
                text: c.to_string(),
                style: *style,
            });
        }
        out.push(VisualRow {
            runs: row_runs,
            src_row,
            src_col_start: i,
            width_chars: end - i,
        });
        i = end;
    }
    out
}

/// Per-character "added since last save" bitmap for a single source line.
/// True means the char is new (will be rendered bold).
pub type AddedFlags<'a> = Option<&'a [bool]>;

/// One regex hit projected onto a single source row, used by the renderer
/// to paint matches red. `is_current` marks the hit that the cursor is
/// parked on (gets a brighter highlight).
#[derive(Debug, Clone, Copy)]
pub struct RowHit {
    pub col_start: usize,
    pub col_end: usize,
    pub is_current: bool,
}

fn match_style_at(row_hits: &[RowHit], col: usize) -> Option<Style> {
    for hit in row_hits {
        if col >= hit.col_start && col < hit.col_end {
            return Some(if hit.is_current {
                Style::default()
                    .bg(Color::LightRed)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().bg(Color::Red).fg(Color::White)
            });
        }
    }
    None
}

/// Foreground style for a Place/Character lexicon hit at the given column,
/// or None if the column doesn't fall inside any hit. Place wins over
/// Character if both categories match the same column (shouldn't happen at
/// the lexicon-build layer, but cheap to be defensive).
fn lex_style_at(hits: &[super::lexicon::LexHit], col: usize) -> Option<Style> {
    use super::lexicon::LexCategory;
    let mut chosen: Option<LexCategory> = None;
    for hit in hits {
        if col >= hit.col_start && col < hit.col_end {
            // Prefer Place if we already saw it; otherwise keep the first
            // category that hits.
            match (chosen, hit.category) {
                (Some(LexCategory::Place), _) => {}
                (_, cat) => chosen = Some(cat),
            }
        }
    }
    chosen.map(|cat| match cat {
        LexCategory::Place => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        LexCategory::Character => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    })
}

/// Compute which characters in `current` differ from `saved`, returning a
/// bool-per-char vector aligned with `current`. Uses the longest common
/// prefix + suffix method: characters between the two unchanged regions are
/// marked added. Works well for the common case of typing inside one line;
/// for cross-line inserts the per-line index alignment may misattribute, but
/// the next save resets the snapshot so drift is bounded.
pub fn diff_added(saved: &str, current: &str) -> Vec<bool> {
    let s: Vec<char> = saved.chars().collect();
    let c: Vec<char> = current.chars().collect();
    let prefix = s.iter().zip(c.iter()).take_while(|(a, b)| a == b).count();
    let s_rem = &s[prefix..];
    let c_rem = &c[prefix..];
    let suffix = s_rem
        .iter()
        .rev()
        .zip(c_rem.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let mut flags = vec![false; c.len()];
    let end = c.len().saturating_sub(suffix);
    for f in &mut flags[prefix..end] {
        *f = true;
    }
    flags
}

/// Build spans for a wrapped visual row. Unlike `build_row_spans`, no
/// horizontal scrolling applies (the row already fits the viewport). Selection
/// is in source coordinates and intersected with this row's source range.
pub fn build_visual_row_spans(
    row: &VisualRow,
    selection: Option<((usize, usize), (usize, usize))>,
    block: Option<BlockSelection>,
    added: AddedFlags,
    matches: &[RowHit],
    lex_hits: &[super::lexicon::LexHit],
) -> Vec<ratatui::text::Span<'static>> {
    use ratatui::text::Span;

    let sel_range_in_row: Option<(usize, usize)> = selection.and_then(|((r1, c1), (r2, c2))| {
        let row_start = row.src_col_start;
        let row_end = row.src_col_start + row.width_chars;
        if row.src_row < r1 || row.src_row > r2 {
            return None;
        }
        let sel_start = if row.src_row == r1 { c1 } else { 0 };
        let sel_end = if row.src_row == r2 { c2 } else { usize::MAX };
        let s = sel_start.max(row_start);
        let e = sel_end.min(row_end);
        if s >= e {
            None
        } else {
            // Convert to relative-to-row indices.
            Some((s - row_start, e - row_start))
        }
    });

    // Process per-char so the "added" bitmap can give different styles to
    // adjacent characters. Sibling spans merge when their styles match, so
    // the cell count is at most O(chars).
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut visual_col = 0usize;
    for run in &row.runs {
        for c in run.text.chars() {
            let src_col = row.src_col_start + visual_col;
            let is_selected =
                sel_range_in_row.is_some_and(|(s, e)| visual_col >= s && visual_col < e);
            let is_block = block.is_some_and(|b| b.contains(row.src_row, src_col));
            let is_added = added
                .and_then(|flags| flags.get(src_col).copied())
                .unwrap_or(false);
            let mut style = run.style;
            if is_added {
                style = style.add_modifier(Modifier::BOLD);
            }
            // Lexicon (cyan/yellow) sits between added-bold and search
            // match: search hits override the colour for the current
            // search session, but cyan/yellow show through otherwise.
            if let Some(lex_style) = lex_style_at(lex_hits, src_col) {
                style = style.patch(lex_style);
            }
            // Match highlight applies AFTER added-bold but BEFORE selection
            // (so selection's REVERSED wins on cells that are both).
            if let Some(match_style) = match_style_at(matches, src_col) {
                // Compose: keep added's bold, take match's fg/bg.
                style = style.patch(match_style);
            }
            if is_selected || is_block {
                style = style.add_modifier(Modifier::REVERSED);
            }
            if let Some(last) = out.last_mut() {
                if last.style == style {
                    last.content.to_mut().push(c);
                    visual_col += 1;
                    continue;
                }
            }
            out.push(Span::styled(c.to_string(), style));
            visual_col += 1;
        }
    }
    out
}

/// Build the Spans for a single visible row, applying horizontal scroll and
/// the selection overlay (REVERSED modifier). `selection` is the result of
/// `TextArea::selection_range()`.
pub fn build_row_spans(
    runs: &[StyledRun],
    row: usize,
    scroll_col: usize,
    width: usize,
    selection: Option<((usize, usize), (usize, usize))>,
    block: Option<BlockSelection>,
    added: AddedFlags,
    matches: &[RowHit],
    lex_hits: &[super::lexicon::LexHit],
) -> Vec<ratatui::text::Span<'static>> {
    use ratatui::text::Span;

    if width == 0 {
        return Vec::new();
    }

    // Compute the selected char range for this row, if any.
    let sel_range: Option<(usize, usize)> = selection.and_then(|((r1, c1), (r2, c2))| {
        if row < r1 || row > r2 {
            return None;
        }
        let start = if row == r1 { c1 } else { 0 };
        let end = if row == r2 { c2 } else { usize::MAX };
        if start >= end {
            None
        } else {
            Some((start, end))
        }
    });

    let mut out: Vec<Span<'static>> = Vec::new();
    let mut col = 0usize;
    let viewport_end = scroll_col.saturating_add(width);

    for run in runs {
        let chars: Vec<char> = run.text.chars().collect();
        let run_start = col;
        let run_end = col + chars.len();

        if run_end <= scroll_col {
            col = run_end;
            continue;
        }
        if run_start >= viewport_end {
            break;
        }

        let chunk_start = run_start.max(scroll_col);
        let chunk_end = run_end.min(viewport_end);

        for src_col in chunk_start..chunk_end {
            let rel = src_col - run_start;
            let ch = chars[rel];
            let is_selected = sel_range.is_some_and(|(s, e)| src_col >= s && src_col < e);
            let is_block = block.is_some_and(|b| b.contains(row, src_col));
            let is_added = added
                .and_then(|flags| flags.get(src_col).copied())
                .unwrap_or(false);
            let mut style = run.style;
            if is_added {
                style = style.add_modifier(Modifier::BOLD);
            }
            if let Some(lex_style) = lex_style_at(lex_hits, src_col) {
                style = style.patch(lex_style);
            }
            if let Some(match_style) = match_style_at(matches, src_col) {
                style = style.patch(match_style);
            }
            if is_selected || is_block {
                style = style.add_modifier(Modifier::REVERSED);
            }
            if let Some(last) = out.last_mut() {
                if last.style == style {
                    last.content.to_mut().push(ch);
                    continue;
                }
            }
            out.push(Span::styled(ch.to_string(), style));
        }
        col = run_end;
    }

    out
}

pub struct TypstHighlighter {
    inner: TsHighlighter,
    config: HighlightConfiguration,
}

impl TypstHighlighter {
    pub fn new() -> Result<Self, String> {
        let highlights = include_str!("../../assets/typst/highlights.scm");
        let mut config =
            HighlightConfiguration::new(tree_sitter_typst::language(), highlights, "", "")
                .map_err(|e| format!("tree-sitter-typst highlights query: {e}"))?;
        config.configure(HIGHLIGHT_NAMES);
        Ok(Self {
            inner: TsHighlighter::new(),
            config,
        })
    }

    /// Highlight `source` and return one `Vec<StyledRun>` per source line
    /// (split on `\n`). Lines are never wrapped or trimmed.
    ///
    /// On parse failure or any unexpected highlighter error, falls back to
    /// returning unhighlighted runs so the editor stays usable.
    pub fn highlight_lines(&mut self, source: &str) -> Vec<Vec<StyledRun>> {
        match self.try_highlight(source) {
            Ok(lines) => lines,
            Err(_) => plain_lines(source),
        }
    }

    fn try_highlight(&mut self, source: &str) -> Result<Vec<Vec<StyledRun>>, String> {
        let bytes = source.as_bytes();
        let events = self
            .inner
            .highlight(&self.config, bytes, None, |_| None)
            .map_err(|e| format!("highlight: {e}"))?;

        let mut stack: Vec<Style> = Vec::new();
        let mut current_style = Style::default();
        // Per-line runs we're building up.
        let mut lines: Vec<Vec<StyledRun>> = vec![Vec::new()];

        let push_text = |lines: &mut Vec<Vec<StyledRun>>, text: &str, style: Style| {
            for (i, segment) in text.split('\n').enumerate() {
                if i > 0 {
                    lines.push(Vec::new());
                }
                if segment.is_empty() {
                    continue;
                }
                let line = lines.last_mut().unwrap();
                if let Some(last) = line.last_mut() {
                    if last.style == style {
                        last.text.push_str(segment);
                        continue;
                    }
                }
                line.push(StyledRun {
                    text: segment.to_string(),
                    style,
                });
            }
        };

        for event in events {
            match event.map_err(|e| format!("highlight event: {e}"))? {
                HighlightEvent::Source { start, end } => {
                    let text = std::str::from_utf8(&bytes[start..end])
                        .map_err(|e| format!("non-utf8 source: {e}"))?;
                    push_text(&mut lines, text, current_style);
                }
                HighlightEvent::HighlightStart(h) => {
                    stack.push(current_style);
                    let name = HIGHLIGHT_NAMES
                        .get(h.0)
                        .copied()
                        .unwrap_or("");
                    let inherited = style_for_name(name);
                    current_style = merge(current_style, inherited);
                }
                HighlightEvent::HighlightEnd => {
                    current_style = stack.pop().unwrap_or_default();
                }
            }
        }

        // tree-sitter-highlight may emit an empty trailing line when the
        // source ends with `\n`; normalize to match `&str::split('\n')` output.
        if lines.len() > 1 && lines.last().map_or(false, |l| l.is_empty()) {
            // Keep it — `lines().join("\n")` round-trip expects this.
        }

        Ok(lines)
    }
}

fn plain_lines(source: &str) -> Vec<Vec<StyledRun>> {
    source
        .split('\n')
        .map(|line| {
            if line.is_empty() {
                Vec::new()
            } else {
                vec![StyledRun {
                    text: line.to_string(),
                    style: Style::default(),
                }]
            }
        })
        .collect()
}

/// Merge two styles. The inner style's foreground/background/modifiers take
/// precedence when set; otherwise the outer style's values survive.
fn merge(outer: Style, inner: Style) -> Style {
    let fg = inner.fg.or(outer.fg);
    let bg = inner.bg.or(outer.bg);
    let modifier = outer.add_modifier | inner.add_modifier;
    Style::default()
        .add_modifier(modifier)
        .patch(Style {
            fg,
            bg,
            ..Style::default()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_gets_highlighted() {
        let mut h = TypstHighlighter::new().unwrap();
        let lines = h.highlight_lines("= Hello world\n\nplain text");
        assert!(!lines.is_empty(), "highlight produced no lines");
        // Line 0 should have at least one styled run with a non-default style
        // (the heading marker or the heading itself).
        let line0 = &lines[0];
        assert!(!line0.is_empty(), "heading line had no runs: {:?}", line0);
        let has_color = line0.iter().any(|r| r.style.fg.is_some());
        assert!(has_color, "expected a colored run in `= Hello world`, got {:?}", line0);
    }

    #[test]
    fn comment_recognized() {
        let mut h = TypstHighlighter::new().unwrap();
        let lines = h.highlight_lines("// a comment");
        let line0 = &lines[0];
        let has_dark = line0
            .iter()
            .any(|r| r.text.contains("comment") && r.style.fg == Some(Color::DarkGray));
        assert!(has_dark, "expected comment to be DarkGray, got {:?}", line0);
    }

    #[test]
    fn empty_input_one_empty_line() {
        let mut h = TypstHighlighter::new().unwrap();
        let lines = h.highlight_lines("");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].is_empty());
    }
}
