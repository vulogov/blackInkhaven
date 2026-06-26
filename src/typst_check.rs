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
/// REUSE-1 — validate snippet `#include "…/snippets/<slug>.typ"` references in
/// `source` against `known_slugs` (the slugs defined in the Snippets book).
/// Returns one diagnostic per snippet include whose slug is **not** defined —
/// catching typos and references to renamed/deleted snippets the moment you save.
///
/// Only *snippet* includes are checked (those ending `…/snippets/<slug>.typ`);
/// generic Typst `#include`s resolve against arbitrary paths and are left alone.
/// Validating against the live Snippets book (not the assembled artefacts) means
/// it works **before** assembly and never produces "assemble first" noise.
pub fn check_includes(
    source: &str,
    known_slugs: &std::collections::HashSet<String>,
) -> Vec<TypstDiagnostic> {
    let mut out = Vec::new();
    let mut line_start = 0usize; // byte offset of the current line within `source`
    for (i, line) in source.lines().enumerate() {
        let mut search = 0usize;
        while let Some(rel) = line[search..].find("#include") {
            let after = search + rel + "#include".len();
            // The opening quote must follow `#include` (allowing whitespace).
            let Some(q1_rel) = line[after..].find('"') else { break };
            let path_start = after + q1_rel + 1;
            let Some(q2_rel) = line[path_start..].find('"') else { break };
            let path_end = path_start + q2_rel;
            let path = &line[path_start..path_end];
            // Two failure modes:
            //  (1) a proper `…/snippets/<slug>.typ` whose slug isn't defined;
            //  (2) an include whose *filename* matches a known snippet slug but
            //      whose path isn't a proper snippets path (a typo'd directory,
            //      e.g. `../nippets/<slug>.typ`).
            let message = match snippet_slug_of(path) {
                Some(slug) if !known_slugs.contains(&slug) => Some(format!(
                    "#include: no snippet `{slug}` in the Snippets book"
                )),
                Some(_) => None, // a defined snippet — fine
                None => basename_slug(path)
                    .filter(|base| known_slugs.contains(base))
                    .map(|base| {
                        format!(
                            "#include: path does not point at the snippets directory — \
                             did you mean a `snippets/{base}.typ` include?"
                        )
                    }),
            };
            if let Some(message) = message {
                let col = line[..path_start].chars().count() + 1;
                out.push(TypstDiagnostic {
                    line: i + 1,
                    col,
                    byte_start: line_start + path_start,
                    byte_end: line_start + path_end,
                    message,
                    hints: vec!["snippet includes resolve as `…/snippets/<slug>.typ`".into()],
                });
            }
            search = path_end + 1;
        }
        line_start += line.len() + 1; // + the '\n' that `lines()` stripped
    }
    out
}

/// REUSE-1 — every snippet slug referenced by an `#include "…/snippets/<slug>.typ"`
/// in `source`, in order (duplicates kept — the caller tallies). Used for
/// reference counts and `snippets check`.
pub fn snippet_references(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in source.lines() {
        let mut search = 0usize;
        while let Some(rel) = line[search..].find("#include") {
            let after = search + rel + "#include".len();
            let Some(q1_rel) = line[after..].find('"') else { break };
            let path_start = after + q1_rel + 1;
            let Some(q2_rel) = line[path_start..].find('"') else { break };
            let path_end = path_start + q2_rel;
            if let Some(slug) = snippet_slug_of(&line[path_start..path_end]) {
                out.push(slug);
            }
            search = path_end + 1;
        }
    }
    out
}

/// The snippet slug of an include path shaped `…/snippets/<slug>.typ`, else
/// `None`. Requires `snippets` to be the second-to-last path segment, so a
/// `mysnippets/x.typ` does not match.
pub fn snippet_slug_of(path: &str) -> Option<String> {
    let segs: Vec<&str> = path.trim().split('/').collect();
    if segs.len() < 2 || segs[segs.len() - 2] != "snippets" {
        return None;
    }
    let slug = segs.last()?.strip_suffix(".typ")?;
    (!slug.is_empty()).then(|| slug.to_string())
}

/// The filename of an include path minus `.typ`, regardless of directory — used
/// to spot a typo'd snippets directory (`../nippets/<slug>.typ`) by matching the
/// basename against the known snippet slugs.
fn basename_slug(path: &str) -> Option<String> {
    let file = path.trim().rsplit('/').next()?;
    let slug = file.strip_suffix(".typ")?;
    (!slug.is_empty()).then(|| slug.to_string())
}

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

    fn slugs(s: &[&str]) -> std::collections::HashSet<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn check_includes_flags_unknown_snippet_slug() {
        let known = slugs(&["warning-box"]);
        // A known slug → clean.
        assert!(check_includes(
            "text\n#include \"../../snippets/warning-box.typ\"\n",
            &known
        )
        .is_empty());
        // An unknown slug → one diagnostic on the right line.
        let d = check_includes("line one\n#include \"../snippets/missing.typ\"\n", &known);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].line, 2);
        assert!(d[0].message.contains("missing"), "{}", d[0].message);
    }

    #[test]
    fn check_includes_flags_typoed_snippets_directory() {
        // The reported bug: `../nippets/<slug>.typ` (typo in the directory) —
        // the filename matches a known snippet, so flag it as a likely misspell.
        let known = slugs(&["this-is-the-snippet-1"]);
        let d = check_includes("#include \"../nippets/this-is-the-snippet-1.typ\"", &known);
        assert_eq!(d.len(), 1);
        assert!(d[0].message.contains("snippets directory"), "{}", d[0].message);
        // But a non-snippet include whose basename isn't a known slug is left alone.
        assert!(check_includes("#include \"../lib/helpers.typ\"", &known).is_empty());
        // And the correct path is clean.
        assert!(check_includes("#include \"../snippets/this-is-the-snippet-1.typ\"", &known).is_empty());
    }

    #[test]
    fn check_includes_ignores_non_snippet_includes() {
        let known = slugs(&[]);
        // Generic includes resolve against arbitrary paths — not our concern.
        assert!(check_includes("#include \"globals.typ\"", &known).is_empty());
        assert!(check_includes("#include \"../other/foo.typ\"", &known).is_empty());
        // `mysnippets/` is not the snippets dir (segment must equal `snippets`).
        assert!(check_includes("#include \"mysnippets/x.typ\"", &known).is_empty());
    }

    #[test]
    fn snippet_references_extracts_slugs() {
        let src = "#include \"../snippets/a.typ\"\ntext\n#include \"../../snippets/b.typ\" #include \"globals.typ\"\n";
        assert_eq!(snippet_references(src), vec!["a", "b"]);
        // Duplicates are kept (callers tally).
        assert_eq!(
            snippet_references("#include \"../snippets/x.typ\"\n#include \"../snippets/x.typ\""),
            vec!["x", "x"]
        );
        assert!(snippet_references("no includes here").is_empty());
    }

    #[test]
    fn check_includes_two_on_one_line_flags_only_the_unknown() {
        let known = slugs(&["a"]);
        let src = "#include \"../snippets/a.typ\" then #include \"../snippets/b.typ\"";
        let d = check_includes(src, &known);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].line, 1);
        assert!(d[0].message.contains("`b`"), "{}", d[0].message);
    }

    use proptest::prelude::*;

    proptest! {
        /// REUSE-1 — the include scanner must never panic on arbitrary source
        /// (multibyte, unbalanced quotes, stray `#include` fragments).
        #[test]
        fn check_includes_never_panics(src in "\\PC{0,400}") {
            let known: std::collections::HashSet<String> =
                ["a", "b"].iter().map(|s| s.to_string()).collect();
            let _ = check_includes(&src, &known);
        }

        /// 1.3.36 hardening — `check` parses arbitrary editor source
        /// (the user's prose + Typst markup). It must return a
        /// diagnostics Vec and never panic, including on lone
        /// surrogates-free Unicode, unbalanced delimiters, and the
        /// byte-offset → line/col mapping over multibyte input.
        #[test]
        fn check_never_panics(src in "\\PC{0,400}") {
            let _ = check(&src);
        }

        /// Token-salad of Typst markup delimiters + prose — exercises
        /// the parser's bracket/brace/dollar paths past what random
        /// printable strings reach.
        #[test]
        fn check_never_panics_on_markup_salad(
            toks in proptest::collection::vec(
                proptest::sample::select(vec![
                    "$", "#", "[", "]", "{", "}", "(", ")", "*", "_", "=",
                    "\\", "/*", "*/", "let", "x", " ", "\n", "café", "—",
                ]),
                0..200,
            ),
        ) {
            let _ = check(&toks.concat());
        }
    }
}
