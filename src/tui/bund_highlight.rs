//! Lightweight Bund syntax highlighter.
//!
//! Bund's syntax is sparse — Forth-shaped postfix code with curly-
//! brace lambdas, dotted word names, and `// ...` line comments.
//! No need for a full grammar; a small line-by-line tokenizer
//! emits the same `StyledRun` shape the typst and HJSON paths
//! produce.
//!
//! Token classes:
//!   * `// …`            — line comment            → `syntax_comment`
//!   * `"…"`             — string literal          → `syntax_string`
//!   * integer / float   — numeric literal         → `syntax_number`
//!   * `{` / `}`         — lambda braces (mauve)   → tree_script_fg
//!   * known core words  — `register`, `eval`,
//!     `println`, `if`, `drop`, etc.               → `syntax_keyword`
//!   * `ink.*`           — inkhaven stdlib         → `syntax_function`
//!   * `hook.*`          — hook names              → `syntax_function`
//!   * everything else                              — pane foreground
//!
//! Cross-line state is only needed for unterminated strings (a
//! `"` with no closing `"` on the same line shouldn't blow up the
//! next line's highlight). We track that and continue scanning
//! in `InString` on the next line.

use ratatui::style::Style;

use super::highlight::StyledRun;
use super::theme::Theme;

pub fn highlight_bund_lines(source: &str, theme: &Theme) -> Vec<Vec<StyledRun>> {
    let mut state = LineState::Normal;
    let lines_in: Vec<&str> = source.split('\n').collect();
    let mut out: Vec<Vec<StyledRun>> = Vec::with_capacity(lines_in.len());
    for line in lines_in {
        let (tokens, next) = tokenize_line(line, state, theme);
        out.push(tokens);
        state = next;
    }
    if out.is_empty() {
        out.push(Vec::new());
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineState {
    Normal,
    InString,
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

    let str_style = Style::default().fg(theme.syntax_string);
    let num_style = Style::default().fg(theme.syntax_number);
    let kw_style = Style::default().fg(theme.syntax_keyword);
    let fn_style = Style::default().fg(theme.syntax_function);
    let comment_style = Style::default().fg(theme.syntax_comment);
    let brace_style = Style::default().fg(theme.tree_script_fg);

    // Resume an open string from the previous line.
    if state == LineState::InString {
        let (consumed, closed) = scan_until_string_close(&chars, 0);
        let text: String = chars[0..consumed].iter().collect();
        out.push(StyledRun {
            text,
            style: str_style,
        });
        i = consumed;
        state = if closed {
            LineState::Normal
        } else {
            LineState::InString
        };
    }

    while i < n {
        let c = chars[i];

        // `// ...` line comment runs to EOL.
        if c == '/' && i + 1 < n && chars[i + 1] == '/' {
            let text: String = chars[i..].iter().collect();
            out.push(StyledRun {
                text,
                style: comment_style,
            });
            i = n;
            continue;
        }

        // `"..."` string literal — may span lines.
        if c == '"' {
            let (end, closed) = scan_until_string_close(&chars, i + 1);
            // Include the opening quote in the styled run.
            let slice_end = if closed { end } else { n };
            let text: String = chars[i..slice_end].iter().collect();
            out.push(StyledRun {
                text,
                style: str_style,
            });
            i = slice_end;
            if !closed {
                state = LineState::InString;
            }
            continue;
        }

        // Numeric literal.
        if c.is_ascii_digit() || (c == '-' && i + 1 < n && chars[i + 1].is_ascii_digit()) {
            let start = i;
            i += 1;
            while i < n
                && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '_')
            {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            out.push(StyledRun {
                text,
                style: num_style,
            });
            continue;
        }

        // Lambda braces in their own colour.
        if c == '{' || c == '}' {
            out.push(StyledRun {
                text: c.to_string(),
                style: brace_style,
            });
            i += 1;
            continue;
        }

        // Whitespace flows through unstyled — preserves spacing
        // without producing zero-length style spans.
        if c.is_whitespace() {
            let start = i;
            i += 1;
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            out.push(StyledRun {
                text: chars[start..i].iter().collect(),
                style: Style::default(),
            });
            continue;
        }

        // Word: Bund identifiers are liberal — `.`, `_`, `?`, `!`,
        // arithmetic punctuation are all valid name characters.
        // Terminate at whitespace or one of the meta punctuators
        // (`"`, `{`, `}`, `/` if `//`).
        if is_word_start(c) {
            let start = i;
            i += 1;
            while i < n && is_word_continue(chars[i]) {
                // Special-case `//` so word scanning stops before
                // the comment token consumes the rest of the line.
                if chars[i] == '/' && i + 1 < n && chars[i + 1] == '/' {
                    break;
                }
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let style = classify_word(&text, &kw_style, &fn_style);
            out.push(StyledRun { text, style });
            continue;
        }

        // Fallback: single character at the default style. Keeps
        // unrecognised punctuation visible without a panic.
        out.push(StyledRun {
            text: c.to_string(),
            style: Style::default(),
        });
        i += 1;
    }

    (out, state)
}

/// Scan from `start` to the next unescaped `"`. Returns
/// `(index_after_quote, true)` when the quote is found this line,
/// or `(line_len, false)` when the string continues onto the next.
fn scan_until_string_close(chars: &[char], start: usize) -> (usize, bool) {
    let mut i = start;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && i + 1 < chars.len() {
            // Skip the escape sequence wholesale — no escape that
            // matters for highlighting boundary detection contains
            // an unescaped quote.
            i += 2;
            continue;
        }
        if c == '"' {
            return (i + 1, true);
        }
        i += 1;
    }
    (chars.len(), false)
}

fn is_word_start(c: char) -> bool {
    c.is_ascii_alphabetic()
        || c == '_'
        || c == '.'
        || c == '?'
        || c == '!'
        || c == '+'
        || c == '-'
        || c == '*'
        || c == '<'
        || c == '>'
        || c == '='
}

fn is_word_continue(c: char) -> bool {
    !c.is_whitespace()
        && c != '"'
        && c != '{'
        && c != '}'
        && c != '['
        && c != ']'
        && c != '('
        && c != ')'
}

/// Map a word to its highlight style. Three buckets:
///
///   * built-in keywords (control flow + core operations) — kw colour
///   * `ink.*` and `hook.*` namespaces (functions / RAG entry points)
///     — function colour
///   * everything else — default style (which `tokenize_line` skips,
///     so the caller falls back to the pane foreground)
fn classify_word(word: &str, kw_style: &Style, fn_style: &Style) -> Style {
    // Cheap startswith checks before the keyword table — pulls the
    // bulk of inkhaven-specific words out of the keyword scan.
    if word.starts_with("ink.") || word.starts_with("hook.") {
        return *fn_style;
    }
    if KEYWORDS.contains(&word) {
        return *kw_style;
    }
    Style::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ThemeConfig;

    fn theme() -> Theme {
        Theme::from_config(&ThemeConfig::default())
    }

    #[test]
    fn line_comment_runs_to_eol() {
        let src = "dup // a comment\n42";
        let lines = highlight_bund_lines(src, &theme());
        let comment = lines[0]
            .iter()
            .find(|r| r.text.starts_with("//"))
            .expect("comment run");
        assert_eq!(comment.style.fg, Some(theme().syntax_comment));
        // Second line is untouched by the comment.
        assert!(lines[1].iter().any(|r| r.text == "42"));
    }

    #[test]
    fn quoted_string_gets_string_colour() {
        let src = r#""hello world" println"#;
        let lines = highlight_bund_lines(src, &theme());
        let s = lines[0]
            .iter()
            .find(|r| r.text.starts_with('"'))
            .expect("string run");
        assert_eq!(s.style.fg, Some(theme().syntax_string));
        assert!(s.text.ends_with('"'));
    }

    #[test]
    fn lambda_braces_get_script_colour() {
        let src = r#""hook.on_save" { drop "saved" println } register"#;
        let lines = highlight_bund_lines(src, &theme());
        let open = lines[0].iter().find(|r| r.text == "{").unwrap();
        let close = lines[0].iter().find(|r| r.text == "}").unwrap();
        assert_eq!(open.style.fg, Some(theme().tree_script_fg));
        assert_eq!(close.style.fg, Some(theme().tree_script_fg));
    }

    #[test]
    fn ink_namespace_gets_function_colour() {
        let src = "ink.node.list";
        let lines = highlight_bund_lines(src, &theme());
        let w = lines[0].iter().find(|r| r.text == "ink.node.list").unwrap();
        assert_eq!(w.style.fg, Some(theme().syntax_function));
    }

    #[test]
    fn keyword_gets_keyword_colour() {
        let src = "register";
        let lines = highlight_bund_lines(src, &theme());
        let w = lines[0].iter().find(|r| r.text == "register").unwrap();
        assert_eq!(w.style.fg, Some(theme().syntax_keyword));
    }

    #[test]
    fn integer_and_float_literals_get_number_colour() {
        for src in ["42", "3.14", "-7"] {
            let lines = highlight_bund_lines(src, &theme());
            let n = lines[0]
                .iter()
                .find(|r| r.text == src)
                .unwrap_or_else(|| panic!("no run for {src:?}"));
            assert_eq!(n.style.fg, Some(theme().syntax_number), "src={src}");
        }
    }

    #[test]
    fn unterminated_string_continues_on_next_line() {
        let src = "\"opening\nclosing\"\nafter";
        let lines = highlight_bund_lines(src, &theme());
        // Both opening + closing should be string-coloured, and
        // "after" on the third line should NOT be.
        assert!(lines[0]
            .iter()
            .any(|r| r.style.fg == Some(theme().syntax_string)));
        assert!(lines[1]
            .iter()
            .any(|r| r.style.fg == Some(theme().syntax_string)));
        assert!(lines[2]
            .iter()
            .any(|r| r.text == "after" && r.style.fg.is_none()));
    }
}

/// Bundcore + multistackvm vanilla stdlib words inkhaven cares
/// about. Not a complete enumeration — just the ones a typical
/// user script touches. Add more as the stdlib surface grows.
const KEYWORDS: &[&str] = &[
    // lambda / class registration
    "register",
    "unregister",
    "resolve",
    // control flow + composition
    "if",
    "else",
    "while",
    "for",
    "return",
    "break",
    "continue",
    "context",
    "endcontext",
    "execute",
    "execute.",
    // boolean / nil
    "true",
    "false",
    "nodata",
    // stack words (Forth canon)
    "dup",
    "drop",
    "swap",
    "over",
    "rot",
    "nip",
    "tuck",
    "pick",
    // i/o
    "print",
    "println",
    "space",
    "nl",
    // type builders / introspection
    "lambda",
    "class",
    "list",
    "dict",
    "valuemap",
    "object",
    "ptr",
    "pair",
    "text",
    "metrics",
    "conditional",
    "complex",
    // aliases
    "alias",
    "unalias",
];
