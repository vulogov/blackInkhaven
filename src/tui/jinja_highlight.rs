//! Lightweight Jinja2 / minijinja syntax highlighter.
//!
//! STRUCT-1 `content_type: "jinja"` paragraphs are Jinja templates that the
//! assembler renders to Typst. The body is mostly Typst prose (passed through
//! verbatim) interleaved with three Jinja constructs. We don't ship a grammar —
//! a small hand-rolled state machine, mirroring `hjson_highlight`, emits the same
//! `StyledRun` tokens the renderer already understands.
//!
//! Tracks state across lines so a `{# … #}` comment (or a `{{ … }}` /
//! `{% … %}` that wraps onto the next line) still highlights correctly.
//!
//! Token → colour:
//!   * `{# … #}` comment           → `syntax_comment`
//!   * `{{ … }}` expression        → `syntax_function`
//!   * `{% … %}` statement         → `syntax_keyword`
//!   * string literals inside both → `syntax_string`
//!   * `| filter` names            → `syntax_operator`
//!   * everything else (the Typst output) → pane foreground
//!
//! Theme: reuses the shared `syntax_*` set — no Jinja-specific theme fields.

use ratatui::style::Style;

use super::highlight::StyledRun;
use super::theme::Theme;

/// Tokenise `source` line-by-line into the same StyledRun-per-line shape the
/// typst / HJSON highlighters produce.
pub fn highlight_jinja_lines(source: &str, theme: &Theme) -> Vec<Vec<StyledRun>> {
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

/// Cross-line lexer state. A `{#`/`{{`/`{%` that doesn't close on its own line
/// carries the corresponding state into the next line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineState {
    Normal,
    InComment,    // inside `{# … #}`
    InExpression, // inside `{{ … }}`
    InStatement,  // inside `{% … %}`
}

fn tokenize_line(line: &str, enter: LineState, theme: &Theme) -> (Vec<StyledRun>, LineState) {
    let mut out: Vec<StyledRun> = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut i = 0usize;
    let mut state = enter;

    // Continuation from a prior line still inside a delimiter.
    match state {
        LineState::InComment => {
            let (end, finished) = scan_until(&chars, 0, '#', '}');
            push_run(&mut out, &chars, 0, end, comment_style(theme));
            i = end;
            if finished {
                state = LineState::Normal;
            } else {
                return (out, state);
            }
        }
        LineState::InExpression => {
            let (end, finished) =
                emit_inner(&chars, 0, '}', '}', expr_style(theme), theme, &mut out);
            i = end;
            if finished {
                state = LineState::Normal;
            } else {
                return (out, state);
            }
        }
        LineState::InStatement => {
            let (end, finished) =
                emit_inner(&chars, 0, '%', '}', stmt_style(theme), theme, &mut out);
            i = end;
            if finished {
                state = LineState::Normal;
            } else {
                return (out, state);
            }
        }
        LineState::Normal => {}
    }

    // Scan prose, peeling out delimited regions as they open.
    let mut prose_start = i;
    while i < n {
        let opener = if chars[i] == '{' { peek(&chars, i + 1) } else { None };
        match opener {
            Some('#') => {
                push_run(&mut out, &chars, prose_start, i, prose_style(theme));
                let (end, finished) = scan_until(&chars, i + 2, '#', '}');
                push_run(&mut out, &chars, i, end, comment_style(theme));
                i = end;
                prose_start = i;
                if !finished {
                    return (out, LineState::InComment);
                }
            }
            Some('{') => {
                push_run(&mut out, &chars, prose_start, i, prose_style(theme));
                push_run(&mut out, &chars, i, i + 2, expr_style(theme));
                let (end, finished) =
                    emit_inner(&chars, i + 2, '}', '}', expr_style(theme), theme, &mut out);
                i = end;
                prose_start = i;
                if !finished {
                    return (out, LineState::InExpression);
                }
            }
            Some('%') => {
                push_run(&mut out, &chars, prose_start, i, prose_style(theme));
                push_run(&mut out, &chars, i, i + 2, stmt_style(theme));
                let (end, finished) =
                    emit_inner(&chars, i + 2, '%', '}', stmt_style(theme), theme, &mut out);
                i = end;
                prose_start = i;
                if !finished {
                    return (out, LineState::InStatement);
                }
            }
            _ => i += 1,
        }
    }
    push_run(&mut out, &chars, prose_start, n, prose_style(theme));
    (out, state)
}

/// Walk the inside of a `{{ … }}` / `{% … %}` region from `start`, emitting
/// runs in `base` with string literals and `| filter` names pulled out, until
/// the `close_a close_b` marker (which is emitted in `base`). Returns
/// `(index_after_close_or_eol, finished)`.
fn emit_inner(
    chars: &[char],
    start: usize,
    close_a: char,
    close_b: char,
    base: Style,
    theme: &Theme,
    out: &mut Vec<StyledRun>,
) -> (usize, bool) {
    let n = chars.len();
    let mut i = start;
    let mut run_start = start;
    while i < n {
        // Closing marker.
        if i + 1 < n && chars[i] == close_a && chars[i + 1] == close_b {
            push_run(out, chars, run_start, i, base);
            push_run(out, chars, i, i + 2, base);
            return (i + 2, true);
        }
        // String literal.
        if chars[i] == '"' || chars[i] == '\'' {
            push_run(out, chars, run_start, i, base);
            let quote = chars[i];
            let s = i;
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
            push_run(out, chars, s, i, Style::default().fg(theme.syntax_string));
            run_start = i;
            continue;
        }
        // Filter: `| name` (but not the `or` operator `||`).
        if chars[i] == '|' && peek(chars, i + 1) != Some('|') {
            push_run(out, chars, run_start, i, base);
            let s = i;
            i += 1;
            while i < n && chars[i] == ' ' {
                i += 1;
            }
            while i < n && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            push_run(out, chars, s, i, Style::default().fg(theme.syntax_operator));
            run_start = i;
            continue;
        }
        i += 1;
    }
    push_run(out, chars, run_start, n, base);
    (n, false)
}

/// Find the two-char `a b` marker from `start`; returns `(index_past_marker,
/// finished)`, or `(len, false)` if it doesn't close on this line.
fn scan_until(chars: &[char], start: usize, a: char, b: char) -> (usize, bool) {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == a && chars[i + 1] == b {
            return (i + 2, true);
        }
        i += 1;
    }
    (chars.len(), false)
}

fn push_run(out: &mut Vec<StyledRun>, chars: &[char], from: usize, to: usize, style: Style) {
    if to > from {
        out.push(StyledRun {
            text: chars[from..to].iter().collect(),
            style,
        });
    }
}

fn peek(chars: &[char], i: usize) -> Option<char> {
    chars.get(i).copied()
}

fn prose_style(theme: &Theme) -> Style {
    Style::default().fg(theme.pane_fg)
}
fn comment_style(theme: &Theme) -> Style {
    Style::default().fg(theme.syntax_comment)
}
fn expr_style(theme: &Theme) -> Style {
    Style::default().fg(theme.syntax_function)
}
fn stmt_style(theme: &Theme) -> Style {
    Style::default().fg(theme.syntax_keyword)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ThemeConfig;

    fn theme() -> Theme {
        Theme::from_config(&ThemeConfig::default())
    }

    fn find<'a>(line: &'a [StyledRun], text: &str) -> &'a StyledRun {
        line.iter()
            .find(|r| r.text == text)
            .unwrap_or_else(|| panic!("no run `{text}` in {line:?}"))
    }

    #[test]
    fn comment_gets_comment_colour() {
        let lines = highlight_jinja_lines("{# a note #}", &theme());
        assert_eq!(lines[0][0].style.fg, Some(theme().syntax_comment));
        assert!(lines[0][0].text.contains("a note"));
    }

    #[test]
    fn expression_and_inner_string() {
        let lines = highlight_jinja_lines("= {{ linked[\"aria\"].name }}", &theme());
        let line = &lines[0];
        // The leading `= ` Typst prose stays foreground.
        assert!(line.iter().any(|r| r.text.contains("= ") && r.style.fg == Some(theme().pane_fg)));
        // The opening `{{` is expression-coloured.
        assert!(line.iter().any(|r| r.text == "{{" && r.style.fg == Some(theme().syntax_function)));
        // The quoted key is string-coloured.
        assert_eq!(find(line, "\"aria\"").style.fg, Some(theme().syntax_string));
    }

    #[test]
    fn statement_gets_keyword_colour() {
        let lines = highlight_jinja_lines("{% if x %}body{% endif %}", &theme());
        let line = &lines[0];
        assert!(line.iter().any(|r| r.text == "{%" && r.style.fg == Some(theme().syntax_keyword)));
        // The literal Typst between the tags is prose.
        assert_eq!(find(line, "body").style.fg, Some(theme().pane_fg));
    }

    #[test]
    fn filter_name_gets_operator_colour() {
        let lines = highlight_jinja_lines("{{ name | upper }}", &theme());
        let line = &lines[0];
        let filt = line.iter().find(|r| r.text.contains("upper")).expect("filter run");
        assert_eq!(filt.style.fg, Some(theme().syntax_operator));
    }

    #[test]
    fn comment_spans_multiple_lines() {
        let lines = highlight_jinja_lines("{# start\nstill inside\nend #} after", &theme());
        assert_eq!(lines.len(), 3);
        for run in &lines[1] {
            if run.text.trim().is_empty() {
                continue;
            }
            assert_eq!(run.style.fg, Some(theme().syntax_comment), "got {run:?}");
        }
        // After `#}` the trailing prose is foreground again.
        assert!(lines[2].iter().any(|r| r.text.contains("after") && r.style.fg == Some(theme().pane_fg)));
    }

    #[test]
    fn plain_typst_stays_foreground() {
        let lines = highlight_jinja_lines("#block[Just Typst, no Jinja here.]", &theme());
        for run in &lines[0] {
            assert_eq!(run.style.fg, Some(theme().pane_fg), "got {run:?}");
        }
    }

    #[test]
    fn never_panics_on_unbalanced() {
        // Unclosed delimiters / stray markers must not panic.
        for src in ["{{", "{%", "{#", "}}", "{{ a", "text {{ x | }}", "{# c"] {
            let _ = highlight_jinja_lines(src, &theme());
        }
    }
}
