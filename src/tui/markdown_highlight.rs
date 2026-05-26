//! Lightweight Markdown syntax highlighter for the editor.
//!
//! Used in the Help book (and anywhere else `content_type =
//! Some("markdown")`).  Source-level highlighting — tokens
//! map onto the existing `syntax_*` theme colours so the
//! editor's visual style stays consistent across content
//! types.  Returns the same `Vec<Vec<StyledRun>>` shape the
//! hjson + bund highlighters produce so the renderer
//! pipeline doesn't need a new branch.
//!
//! Tracks state across lines (fenced code blocks) so triple-
//! backtick blocks colour correctly when the user scrolls
//! into the middle of one.
//!
//! Highlighted constructs:
//!   * `# Heading` … `###### Heading` — heading text +
//!     leading hashes get `syntax_function`, bold.
//!   * ` ``` ` fenced code blocks — entire block uses
//!     `syntax_string`.
//!   * `` `inline code` `` — backtick-delimited spans use
//!     `syntax_string`.
//!   * `**bold**` and `__bold__` — bold modifier on top of
//!     pane foreground.
//!   * `*italic*` and `_italic_` — italic modifier on top
//!     of pane foreground.
//!   * Links `[text](url)` — link text in `syntax_function`,
//!     URL in `syntax_string`.
//!   * Bullet markers `- `, `* `, `+ `, and numbered
//!     `1. ` / `1) ` — marker only, in `syntax_keyword`.
//!   * Blockquote `> ` — entire line dimmed.
//!   * Horizontal rule `---` / `***` / `___` — full line
//!     in `syntax_comment` colour.
//!
//! Deliberately NOT highlighted (out of scope for a
//! one-pass lexer): tables, footnotes, definition lists,
//! HTML inlines, image syntax beyond the simple link form,
//! reference-style links.  The Help book content rarely
//! uses these.

use ratatui::style::{Modifier, Style};

use super::highlight::StyledRun;
use super::theme::Theme;

/// Tokenise `source` line-by-line.  Cross-line state
/// (fenced code blocks) is tracked via `LineState`.
pub fn highlight_markdown_lines(
    source: &str,
    theme: &Theme,
) -> Vec<Vec<StyledRun>> {
    let mut state = LineState::Normal;
    let lines_in: Vec<&str> = source.split('\n').collect();
    let mut out: Vec<Vec<StyledRun>> = Vec::with_capacity(lines_in.len());
    for line in lines_in {
        let (tokens, next_state) = tokenize_line(line, state, theme);
        out.push(tokens);
        state = next_state;
    }
    if out.is_empty() {
        out.push(Vec::new());
    }
    out
}

/// Cross-line state.  `InFence` means we're inside a
/// ``` ``` ``` ``` fenced block opened on a prior line —
/// the entire current line is part of that block until we
/// see the closing fence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineState {
    Normal,
    InFence,
}

fn tokenize_line(
    line: &str,
    enter: LineState,
    theme: &Theme,
) -> (Vec<StyledRun>, LineState) {
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();

    // ── State 1: inside a fenced code block ─────────────
    if enter == LineState::InFence {
        // Closing fence ``` (possibly indented).  We check
        // for a line that consists entirely of three or
        // more backticks plus whitespace.
        let trimmed = line.trim_start();
        if trimmed.starts_with("```")
            && trimmed.trim_end_matches(|c: char| c == '`').is_empty()
        {
            // Whole closing-fence line — render in
            // `syntax_string` like the rest of the block.
            return (
                vec![StyledRun {
                    text: line.to_string(),
                    style: Style::default().fg(theme.syntax_string),
                }],
                LineState::Normal,
            );
        }
        // Body of the fenced block — single styled run.
        return (
            vec![StyledRun {
                text: line.to_string(),
                style: Style::default().fg(theme.syntax_string),
            }],
            LineState::InFence,
        );
    }

    // ── State 0: normal line ───────────────────────────
    // Opening fence: ``` (possibly with language tag).
    let trimmed_left = line.trim_start();
    if trimmed_left.starts_with("```") {
        return (
            vec![StyledRun {
                text: line.to_string(),
                style: Style::default().fg(theme.syntax_string),
            }],
            LineState::InFence,
        );
    }

    // Horizontal rule: `---`, `***`, or `___` (3+ of the
    // same char), optionally indented, ENTIRE line is the
    // rule.
    if is_horizontal_rule(&chars) {
        return (
            vec![StyledRun {
                text: line.to_string(),
                style: Style::default().fg(theme.syntax_comment),
            }],
            LineState::Normal,
        );
    }

    // ATX heading: 1-6 leading `#` chars + space + text.
    if let Some((hash_count, rest_start)) = atx_heading_prefix(&chars) {
        let mut out: Vec<StyledRun> = Vec::new();
        // Leading hashes + the space after them get
        // syntax_function so the user sees the heading
        // marker distinctly from the title text.
        let prefix: String = chars[..rest_start].iter().collect();
        out.push(StyledRun {
            text: prefix,
            style: Style::default()
                .fg(theme.syntax_function)
                .add_modifier(Modifier::BOLD),
        });
        // Heading text — bold, in syntax_function.
        let text: String = chars[rest_start..].iter().collect();
        out.push(StyledRun {
            text,
            style: Style::default()
                .fg(theme.syntax_function)
                .add_modifier(Modifier::BOLD),
        });
        let _ = hash_count;
        return (out, LineState::Normal);
    }

    // Blockquote: line starts with `>` (possibly indented).
    if trimmed_left.starts_with('>') {
        return (
            vec![StyledRun {
                text: line.to_string(),
                style: Style::default()
                    .fg(theme.syntax_comment)
                    .add_modifier(Modifier::ITALIC),
            }],
            LineState::Normal,
        );
    }

    // List bullet (- * + or `N.` / `N)`) — colour just the
    // marker so the user can visually scan items.  Body of
    // the item proceeds through the inline lexer below.
    let (marker_end, marker_present) = list_marker_end(&chars);
    let mut out: Vec<StyledRun> = Vec::new();
    let mut i = 0;
    if marker_present {
        let marker: String = chars[..marker_end].iter().collect();
        out.push(StyledRun {
            text: marker,
            style: Style::default()
                .fg(theme.syntax_keyword)
                .add_modifier(Modifier::BOLD),
        });
        i = marker_end;
    }

    // Inline lexer: walks the remainder once, emitting
    // styled runs for backtick code spans, `**bold**` /
    // `*italic*`, and `[text](url)` link blocks.  Plain
    // text falls through with the default pane foreground.
    while i < n {
        // Backtick inline code: `code`.
        if chars[i] == '`' {
            let start = i;
            i += 1;
            while i < n && chars[i] != '`' {
                i += 1;
            }
            if i < n {
                i += 1; // consume closing backtick
            }
            out.push(StyledRun {
                text: chars[start..i].iter().collect(),
                style: Style::default().fg(theme.syntax_string),
            });
            continue;
        }

        // Bold: **text** or __text__.
        if i + 1 < n
            && ((chars[i] == '*' && chars[i + 1] == '*')
                || (chars[i] == '_' && chars[i + 1] == '_'))
        {
            let opener = chars[i];
            let start = i;
            i += 2;
            while i + 1 < n
                && !(chars[i] == opener && chars[i + 1] == opener)
            {
                i += 1;
            }
            if i + 1 < n {
                i += 2; // consume closing pair
            }
            out.push(StyledRun {
                text: chars[start..i].iter().collect(),
                style: Style::default().add_modifier(Modifier::BOLD),
            });
            continue;
        }

        // Italic: *text* or _text_.  Single asterisk /
        // underscore; require the closing pair before
        // whitespace to avoid styling math expressions.
        if (chars[i] == '*' || chars[i] == '_')
            && i + 1 < n
            && !chars[i + 1].is_whitespace()
        {
            let opener = chars[i];
            let start = i;
            i += 1;
            while i < n && chars[i] != opener {
                i += 1;
            }
            if i < n {
                i += 1;
            }
            out.push(StyledRun {
                text: chars[start..i].iter().collect(),
                style: Style::default().add_modifier(Modifier::ITALIC),
            });
            continue;
        }

        // Link: [text](url).
        if chars[i] == '[' {
            // Find matching `]` then `(` then `)`.
            let bracket_start = i;
            let mut j = i + 1;
            while j < n && chars[j] != ']' {
                j += 1;
            }
            if j < n
                && j + 1 < n
                && chars[j + 1] == '('
            {
                let bracket_end = j;
                let paren_start = j + 1;
                let mut k = paren_start + 1;
                while k < n && chars[k] != ')' {
                    k += 1;
                }
                if k < n {
                    out.push(StyledRun {
                        text: chars[bracket_start..=bracket_end].iter().collect(),
                        style: Style::default().fg(theme.syntax_function),
                    });
                    out.push(StyledRun {
                        text: chars[paren_start..=k].iter().collect(),
                        style: Style::default().fg(theme.syntax_string),
                    });
                    i = k + 1;
                    continue;
                }
            }
            // Not a complete link — emit `[` as plain text
            // and continue scanning the rest verbatim.
            out.push(StyledRun {
                text: "[".to_string(),
                style: Style::default(),
            });
            i += 1;
            continue;
        }

        // Plain text run — accumulate until we hit a
        // markup-significant char.
        let start = i;
        while i < n
            && chars[i] != '`'
            && chars[i] != '*'
            && chars[i] != '_'
            && chars[i] != '['
        {
            i += 1;
        }
        if i > start {
            out.push(StyledRun {
                text: chars[start..i].iter().collect(),
                style: Style::default(),
            });
        } else {
            // Defensive: a markup char that we couldn't
            // parse (e.g. orphan `*` at end of line).
            // Emit verbatim to avoid an infinite loop.
            out.push(StyledRun {
                text: chars[i].to_string(),
                style: Style::default(),
            });
            i += 1;
        }
    }

    (out, LineState::Normal)
}

/// Detect the ATX heading prefix `# ` … `###### ` and
/// return `(hash_count, byte_offset_past_prefix)`.  Returns
/// None when the line isn't a heading.  Accepts 1-6 `#`
/// chars followed by at least one whitespace.
fn atx_heading_prefix(chars: &[char]) -> Option<(usize, usize)> {
    // Skip leading whitespace (per CommonMark, up to 3
    // spaces are allowed before the hashes).
    let mut i = 0;
    while i < chars.len() && i < 3 && chars[i] == ' ' {
        i += 1;
    }
    let hash_start = i;
    while i < chars.len() && chars[i] == '#' {
        i += 1;
    }
    let hash_count = i - hash_start;
    if !(1..=6).contains(&hash_count) {
        return None;
    }
    // Must be followed by whitespace (or end of line for an
    // empty heading).
    if i < chars.len() && !chars[i].is_whitespace() {
        return None;
    }
    // Consume the single space after the hashes if present.
    let after = if i < chars.len() && chars[i] == ' ' {
        i + 1
    } else {
        i
    };
    Some((hash_count, after))
}

/// `---`, `***`, `___` — three or more of the same char
/// optionally separated by spaces, nothing else on the
/// line.  Standard CommonMark thematic break.
fn is_horizontal_rule(chars: &[char]) -> bool {
    let mut found: Option<char> = None;
    let mut count = 0;
    for &c in chars {
        if c == ' ' || c == '\t' {
            continue;
        }
        if c == '-' || c == '*' || c == '_' {
            if let Some(seen) = found {
                if seen != c {
                    return false;
                }
            } else {
                found = Some(c);
            }
            count += 1;
        } else {
            return false;
        }
    }
    count >= 3
}

/// Find the end of a leading list marker `- `, `* `, `+ `,
/// `1. `, `1) `.  Returns `(end_index, present)` — when
/// `present = false`, `end_index` is meaningless and the
/// caller treats the line as a non-list line.
fn list_marker_end(chars: &[char]) -> (usize, bool) {
    let mut i = 0;
    // Optional leading whitespace.
    while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
        i += 1;
    }
    if i >= chars.len() {
        return (0, false);
    }
    // Bullet markers.
    if matches!(chars[i], '-' | '*' | '+')
        && i + 1 < chars.len()
        && chars[i + 1] == ' '
    {
        return (i + 2, true);
    }
    // Numbered markers: digit(s) followed by `.` or `)`
    // and a space.
    let digit_start = i;
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }
    if i > digit_start
        && i < chars.len()
        && (chars[i] == '.' || chars[i] == ')')
        && i + 1 < chars.len()
        && chars[i + 1] == ' '
    {
        return (i + 2, true);
    }
    (0, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ThemeConfig;

    fn theme() -> Theme {
        Theme::from_config(&ThemeConfig::default())
    }

    #[test]
    fn heading_levels_get_function_colour() {
        for level in 1..=6 {
            let hashes = "#".repeat(level);
            let src = format!("{hashes} Title");
            let lines = highlight_markdown_lines(&src, &theme());
            // First run is the hash prefix in syntax_function.
            assert_eq!(
                lines[0][0].style.fg,
                Some(theme().syntax_function),
                "h{level} prefix colour",
            );
        }
    }

    #[test]
    fn non_heading_not_styled_as_heading() {
        // `#text` (no space) is NOT a heading per CommonMark.
        let lines = highlight_markdown_lines("#nothash", &theme());
        // The first run should NOT be syntax_function-styled.
        assert_ne!(lines[0][0].style.fg, Some(theme().syntax_function));
    }

    #[test]
    fn fenced_code_block_spans_multiple_lines() {
        let src = "```rust\nfn main() {}\n```\nafter";
        let lines = highlight_markdown_lines(src, &theme());
        assert_eq!(lines.len(), 4);
        // Lines 0,1,2 should all be syntax_string-coloured.
        for i in 0..3 {
            assert!(
                lines[i].iter().all(|r| r.style.fg == Some(theme().syntax_string)),
                "line {i} not all in code-block colour: {:?}",
                lines[i],
            );
        }
        // Line 3 (after the close) should NOT carry the
        // code colour.
        assert!(
            !lines[3].iter().any(|r| r.style.fg == Some(theme().syntax_string)),
            "line after fence still styled as code",
        );
    }

    #[test]
    fn inline_code_gets_string_colour() {
        let lines = highlight_markdown_lines("call `foo()` to test", &theme());
        let code_run = lines[0]
            .iter()
            .find(|r| r.text.contains("foo()"))
            .expect("inline code run");
        assert_eq!(code_run.style.fg, Some(theme().syntax_string));
    }

    #[test]
    fn bold_and_italic_get_modifiers() {
        let lines = highlight_markdown_lines("here is **bold** and *italic*", &theme());
        let bold = lines[0]
            .iter()
            .find(|r| r.text.contains("**bold**"))
            .expect("bold run");
        assert!(bold.style.add_modifier.contains(Modifier::BOLD));
        let italic = lines[0]
            .iter()
            .find(|r| r.text.contains("*italic*") && !r.text.contains("**"))
            .expect("italic run");
        assert!(italic.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn link_text_and_url_styled_separately() {
        let lines = highlight_markdown_lines("see [docs](https://example.com)", &theme());
        let text = lines[0]
            .iter()
            .find(|r| r.text == "[docs]")
            .expect("link text");
        assert_eq!(text.style.fg, Some(theme().syntax_function));
        let url = lines[0]
            .iter()
            .find(|r| r.text == "(https://example.com)")
            .expect("link url");
        assert_eq!(url.style.fg, Some(theme().syntax_string));
    }

    #[test]
    fn blockquote_entire_line_dimmed() {
        let lines = highlight_markdown_lines("> quoted text", &theme());
        assert_eq!(lines[0].len(), 1);
        assert_eq!(lines[0][0].style.fg, Some(theme().syntax_comment));
    }

    #[test]
    fn horizontal_rule_recognised() {
        for hr in ["---", "***", "___", "  ---", "- - -"] {
            let lines = highlight_markdown_lines(hr, &theme());
            assert_eq!(lines[0].len(), 1, "{hr}");
            assert_eq!(
                lines[0][0].style.fg,
                Some(theme().syntax_comment),
                "{hr} not styled as HR",
            );
        }
    }

    #[test]
    fn bullet_markers_styled_separately() {
        let lines = highlight_markdown_lines("- item one", &theme());
        let marker = lines[0]
            .iter()
            .find(|r| r.text == "- ")
            .expect("bullet marker");
        assert_eq!(marker.style.fg, Some(theme().syntax_keyword));
    }

    #[test]
    fn numbered_list_marker_recognised() {
        let lines = highlight_markdown_lines("1. first", &theme());
        let marker = lines[0]
            .iter()
            .find(|r| r.text == "1. ")
            .expect("numbered marker");
        assert_eq!(marker.style.fg, Some(theme().syntax_keyword));
    }
}
