//! Typst parse-time diagnostics. Phase 1 of the typst-as-library
//! plan (1.2.5+).
//!
//! Pulls in `typst-syntax` only — no eval, no layout, no render,
//! no fonts, no package resolver. Gives us "is this even valid
//! Typst?" at the source level so the editor can surface a parse
//! error at the line where it lives, without spawning a child
//! `typst compile` process.
//!
//! The eventual Phase 4 swap (in-process compile + PDF emit gated
//! behind `typst.engine = "inprocess"`) lives separately; this
//! module is intentionally the smallest possible step on that
//! path.

use typst_syntax::Source;

/// One parse-time diagnostic, anchored at a specific position in
/// the source buffer.
///
/// `line` and `col` are **1-based** so they match how the editor
/// pane and human-facing status messages talk about positions
/// elsewhere in inkhaven. `byte_start` / `byte_end` are 0-based
/// byte offsets in the original source (useful if a future
/// caller wants to highlight the exact span).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypstDiagnostic {
    pub line: usize,
    pub col: usize,
    pub byte_start: usize,
    pub byte_end: usize,
    pub message: String,
    pub hints: Vec<String>,
}

impl TypstDiagnostic {
    /// One-line, human-readable summary. Used for status-bar
    /// messages and the `inkhaven` log output.
    pub fn summary(&self) -> String {
        format!("typst: line {}:{} — {}", self.line, self.col, self.message)
    }
}

/// Parse `source` and return every syntactic error the typst
/// parser found. An empty vec means the buffer parses cleanly —
/// no statement about whether the document would actually
/// *compile* (no eval / layout / typst-stdlib lookup is run);
/// it just says the grammar is satisfied.
///
/// `source` is passed by reference but `Source::detached` takes
/// ownership of a `String`, so we copy. Buffers are typically
/// a few KB to a few hundred KB; the cost is dominated by the
/// parser itself, not the clone.
pub fn check(source: &str) -> Vec<TypstDiagnostic> {
    let source = Source::detached(source.to_owned());
    let root = source.root();
    let errors = root.errors();
    if errors.is_empty() {
        return Vec::new();
    }
    let lines = source.lines();
    let mut out = Vec::with_capacity(errors.len());
    for err in errors {
        let range = match source.range(err.span) {
            Some(r) => r,
            None => continue, // detached / synthetic span — skip
        };
        let (line0, col0) = lines
            .byte_to_line_column(range.start)
            .unwrap_or((0, 0));
        out.push(TypstDiagnostic {
            line: line0 + 1,
            col: col0 + 1,
            byte_start: range.start,
            byte_end: range.end,
            message: err.message.to_string(),
            hints: err.hints.iter().map(|h| h.to_string()).collect(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_is_clean() {
        assert!(check("").is_empty());
    }

    #[test]
    fn plain_prose_is_clean() {
        let src = "The storm came up at three.\n\nThe sea kept rising.\n";
        assert!(check(src).is_empty(), "got: {:?}", check(src));
    }

    #[test]
    fn well_formed_heading_is_clean() {
        let src = "= Chapter one\n\nThe storm came up at three.\n";
        assert!(check(src).is_empty(), "got: {:?}", check(src));
    }

    #[test]
    fn unterminated_string_is_an_error() {
        // Code-mode string literal that never closes — the parser
        // should emit an error at the opening quote.
        let src = r#"#let x = "hello
broken
"#;
        let diags = check(src);
        assert!(!diags.is_empty(), "expected at least one diagnostic");
        let first = &diags[0];
        assert!(first.line >= 1);
        assert!(first.col >= 1);
        // Sanity: message should be non-empty.
        assert!(!first.message.is_empty());
    }

    #[test]
    fn unbalanced_brace_reports_a_position() {
        // Open brace in code mode, no close.
        let src = "#let f() = {\n  1 + 1\n";
        let diags = check(src);
        assert!(!diags.is_empty());
        // Every diagnostic must have a valid (line, col) pair.
        for d in &diags {
            assert!(d.line >= 1, "line was {}", d.line);
            assert!(d.col >= 1, "col was {}", d.col);
            assert!(
                d.byte_end >= d.byte_start,
                "byte range must be non-negative",
            );
        }
    }

    #[test]
    fn summary_contains_line_and_message() {
        let d = TypstDiagnostic {
            line: 12,
            col: 5,
            byte_start: 100,
            byte_end: 110,
            message: "unexpected token".to_owned(),
            hints: vec![],
        };
        let s = d.summary();
        assert!(s.contains("line 12:5"));
        assert!(s.contains("unexpected token"));
    }
}
