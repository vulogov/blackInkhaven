//! Lightweight HJSON syntax highlighter.
//!
//! HJSON is a superset of JSON with comments, unquoted keys, optional
//! commas, and multi-line strings. The official spec is at
//! <https://hjson.github.io>. We don't ship a tree-sitter grammar for
//! it — instead a small hand-rolled state machine emits StyledRun
//! tokens that slot into the same renderer the typst highlighter
//! uses.
//!
//! Tracks state across lines (block comments, triple-quoted multi-
//! line strings) so a `/* … */` or `''' … '''` that spans many lines
//! still highlights correctly.
//!
//! Mode classification:
//!   * `// ...` line comments (HJSON / Hjson-script extension)
//!   * `# ...` line comments
//!   * `/* ... */` block comments (may span lines)
//!   * `"..."` quoted strings (with JSON escapes)
//!   * `'''...'''` multi-line strings (HJSON extension)
//!   * `true`, `false`, `null` literals (keywords)
//!   * Integer / float number literals
//!   * Punctuation: `{`, `}`, `[`, `]`, `,`, `:`
//!   * Keys (identifier followed by `:`) get the function colour
//!   * Everything else (unquoted-string content, whitespace) stays
//!     the pane foreground
//!
//! Theme: the highlighter reuses the existing `syntax_*` colour set —
//! no new HJSON-specific theme fields. Keys → `syntax_function`,
//! strings → `syntax_string`, numbers → `syntax_number`, keywords →
//! `syntax_keyword`, comments → `syntax_comment`.

use ratatui::style::{Modifier, Style};

use super::highlight::StyledRun;
use super::theme::Theme;

/// Tokenise `source` line-by-line into the same StyledRun-per-line
/// shape the typst highlighter produces.
pub fn highlight_hjson_lines(source: &str, theme: &Theme) -> Vec<Vec<StyledRun>> {
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

/// Cross-line lexer state. Default is Normal; a `/* …` runs the
/// scanner through BlockComment until it finds `*/`; a `'''` opens
/// MultilineString until the closing `'''`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineState {
    Normal,
    BlockComment,
    MultilineString,
}

fn tokenize_line(
    line: &str,
    enter: LineState,
    theme: &Theme,
) -> (Vec<StyledRun>, LineState) {
    let mut out: Vec<StyledRun> = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut i = 0;
    let mut state = enter;

    // If we entered the line inside a multi-line construct, consume
    // until its closing marker before doing token-by-token scanning.
    if state == LineState::BlockComment {
        let (consumed, finished) = scan_until_block_comment_close(&chars, 0);
        let text: String = chars[0..consumed].iter().collect();
        out.push(StyledRun {
            text,
            style: Style::default().fg(theme.syntax_comment),
        });
        i = consumed;
        if finished {
            state = LineState::Normal;
        } else {
            return (out, state);
        }
    } else if state == LineState::MultilineString {
        let (consumed, finished) = scan_until_triple_quote_close(&chars, 0);
        let text: String = chars[0..consumed].iter().collect();
        out.push(StyledRun {
            text,
            style: Style::default().fg(theme.syntax_string),
        });
        i = consumed;
        if finished {
            state = LineState::Normal;
        } else {
            return (out, state);
        }
    }

    while i < n {
        let c = chars[i];

        // Whitespace — emit unstyled run.
        if c.is_whitespace() {
            let start = i;
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            out.push(StyledRun {
                text: chars[start..i].iter().collect(),
                style: Style::default(),
            });
            continue;
        }

        // Line comments: `//` and `#` consume the rest of the line.
        if c == '/' && peek(&chars, i + 1) == Some('/') {
            out.push(StyledRun {
                text: chars[i..].iter().collect(),
                style: Style::default().fg(theme.syntax_comment),
            });
            i = n;
            continue;
        }
        if c == '#' {
            out.push(StyledRun {
                text: chars[i..].iter().collect(),
                style: Style::default().fg(theme.syntax_comment),
            });
            i = n;
            continue;
        }

        // Block comment `/* ... */` may span lines.
        if c == '/' && peek(&chars, i + 1) == Some('*') {
            let start = i;
            let (end_offset, finished) = scan_until_block_comment_close(&chars, i + 2);
            let end = end_offset;
            out.push(StyledRun {
                text: chars[start..end].iter().collect(),
                style: Style::default().fg(theme.syntax_comment),
            });
            i = end;
            if !finished {
                state = LineState::BlockComment;
                return (out, state);
            }
            continue;
        }

        // Multi-line string `'''...'''`.
        if c == '\'' && peek(&chars, i + 1) == Some('\'') && peek(&chars, i + 2) == Some('\'') {
            let start = i;
            let (end_offset, finished) = scan_until_triple_quote_close(&chars, i + 3);
            let end = end_offset;
            out.push(StyledRun {
                text: chars[start..end].iter().collect(),
                style: Style::default().fg(theme.syntax_string),
            });
            i = end;
            if !finished {
                state = LineState::MultilineString;
                return (out, state);
            }
            continue;
        }

        // Quoted string (single line, JSON-style escapes).
        if c == '"' || c == '\'' {
            let quote = c;
            let start = i;
            i += 1;
            while i < n {
                if chars[i] == '\\' && i + 1 < n {
                    i += 2;
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push(StyledRun {
                text: chars[start..i].iter().collect(),
                style: Style::default().fg(theme.syntax_string),
            });
            continue;
        }

        // Numbers — integer or float, optional leading sign.
        if c.is_ascii_digit() || (c == '-' && peek(&chars, i + 1).is_some_and(|x| x.is_ascii_digit())) {
            let start = i;
            if c == '-' {
                i += 1;
            }
            while i < n && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == 'e'
                || chars[i] == 'E' || chars[i] == '+' || chars[i] == '-')
            {
                i += 1;
            }
            out.push(StyledRun {
                text: chars[start..i].iter().collect(),
                style: Style::default().fg(theme.syntax_number),
            });
            continue;
        }

        // Punctuation.
        if matches!(c, '{' | '}' | '[' | ']' | ',' | ':') {
            out.push(StyledRun {
                text: c.to_string(),
                style: Style::default().fg(theme.pane_fg),
            });
            i += 1;
            continue;
        }

        // Unquoted identifier or keyword. HJSON allows unquoted keys
        // and bare-value strings: anything that's not punctuation or
        // a string opener becomes an identifier-run until a delimiter.
        let start = i;
        while i < n
            && !chars[i].is_whitespace()
            && !matches!(
                chars[i],
                '{' | '}' | '[' | ']' | ',' | ':' | '"' | '\'' | '#'
            )
        {
            // `//` mid-token starts a comment — bail out before it.
            if chars[i] == '/' && peek(&chars, i + 1) == Some('/') {
                break;
            }
            i += 1;
        }
        let token: String = chars[start..i].iter().collect();
        // Distinguish keywords / numbers (after the fact, in case the
        // numeric check above missed an unsigned float case) / keys /
        // plain text.
        let style = match token.as_str() {
            "true" | "false" | "null" => Style::default()
                .fg(theme.syntax_keyword)
                .add_modifier(Modifier::BOLD),
            _ => {
                // Look ahead: skip whitespace and check for `:` — if
                // yes, this is a key.
                let mut j = i;
                while j < n && chars[j] == ' ' {
                    j += 1;
                }
                if j < n && chars[j] == ':' {
                    Style::default().fg(theme.syntax_function)
                } else {
                    Style::default().fg(theme.pane_fg)
                }
            }
        };
        out.push(StyledRun { text: token, style });
    }
    (out, state)
}

fn peek(chars: &[char], i: usize) -> Option<char> {
    chars.get(i).copied()
}

/// Return `(end_offset, finished)` — `end_offset` is one past the
/// position of `*/` if found, or `chars.len()` if not. `finished`
/// is true when the comment closes on this line.
fn scan_until_block_comment_close(chars: &[char], start: usize) -> (usize, bool) {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == '*' && chars[i + 1] == '/' {
            return (i + 2, true);
        }
        i += 1;
    }
    (chars.len(), false)
}

fn scan_until_triple_quote_close(chars: &[char], start: usize) -> (usize, bool) {
    let mut i = start;
    while i + 2 < chars.len() {
        if chars[i] == '\'' && chars[i + 1] == '\'' && chars[i + 2] == '\'' {
            return (i + 3, true);
        }
        i += 1;
    }
    (chars.len(), false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ThemeConfig;

    fn theme() -> Theme {
        Theme::from_config(&ThemeConfig::default())
    }

    #[test]
    fn line_comments_get_comment_colour() {
        let src = "key: 1 // tail comment";
        let lines = highlight_hjson_lines(src, &theme());
        let line = &lines[0];
        let comment = line.iter().find(|r| r.text.starts_with("//")).unwrap();
        assert_eq!(comment.style.fg, Some(theme().syntax_comment));
    }

    #[test]
    fn hash_line_comment() {
        let src = "# this is a comment";
        let lines = highlight_hjson_lines(src, &theme());
        let line = &lines[0];
        assert!(line.iter().any(|r| r.text.starts_with("# this")));
        let comment = line.iter().find(|r| r.text.starts_with("# this")).unwrap();
        assert_eq!(comment.style.fg, Some(theme().syntax_comment));
    }

    #[test]
    fn block_comment_spans_multiple_lines() {
        let src = "key: 1 /* start\nstill inside\nend */ after";
        let lines = highlight_hjson_lines(src, &theme());
        assert_eq!(lines.len(), 3);
        // The "still inside" middle line should be entirely comment-coloured.
        let mid = &lines[1];
        for run in mid {
            if run.text.trim().is_empty() {
                continue;
            }
            assert_eq!(run.style.fg, Some(theme().syntax_comment), "got run: {:?}", run);
        }
    }

    #[test]
    fn quoted_strings_get_string_colour() {
        let src = "key: \"hello\"";
        let lines = highlight_hjson_lines(src, &theme());
        let line = &lines[0];
        let s = line.iter().find(|r| r.text == "\"hello\"").expect("found string");
        assert_eq!(s.style.fg, Some(theme().syntax_string));
    }

    #[test]
    fn unquoted_keys_get_function_colour() {
        let src = "myKey: \"value\"";
        let lines = highlight_hjson_lines(src, &theme());
        let line = &lines[0];
        let key = line
            .iter()
            .find(|r| r.text == "myKey")
            .expect("key run");
        assert_eq!(key.style.fg, Some(theme().syntax_function));
    }

    #[test]
    fn keywords_are_bold() {
        let src = "flag: true";
        let lines = highlight_hjson_lines(src, &theme());
        let line = &lines[0];
        let kw = line.iter().find(|r| r.text == "true").expect("keyword");
        assert_eq!(kw.style.fg, Some(theme().syntax_keyword));
        assert!(kw.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn numbers_get_number_colour() {
        let src = "count: 42";
        let lines = highlight_hjson_lines(src, &theme());
        let line = &lines[0];
        let n = line.iter().find(|r| r.text == "42").expect("number");
        assert_eq!(n.style.fg, Some(theme().syntax_number));
    }

    #[test]
    fn triple_quoted_multiline_string() {
        let src = "doc: '''\nFirst line.\nSecond line.\n'''";
        let lines = highlight_hjson_lines(src, &theme());
        assert!(lines.len() >= 3);
        // Middle line should be string-coloured.
        let mid = &lines[1];
        assert!(mid.iter().any(|r| r.style.fg == Some(theme().syntax_string)));
    }
}
