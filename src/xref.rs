//! XREF (1.6.15+) — promote unresolved Typst cross-references from the raw
//! compile diagnostics into first-class Output-pane findings.
//!
//! Referencing a label that doesn't exist (`@fig:x` with no `<fig:x>`) is a hard
//! Typst error: it halts compilation, and today the only trace is a line buried
//! in the compiler's stderr blob. This module scans that stderr, recognises the
//! "label does not exist" diagnostic, and emits a `kinds::XREF` finding carrying
//! the label + `file:line:col`, so a dangling cross-reference is actionable in
//! the Output pane the way an uncited claim or an argument gap already is.
//!
//! The scan is a pure string pass ([`scan_diagnostics`]) so it works for both
//! compile engines — the in-process one (`--> path:line:col`) and the external
//! `typst` binary (Ariadne's `┌─ path:line:col`). The diagnostic *message* is
//! identical across both because it originates in Typst itself.

/// One unresolved cross-reference recovered from the compile diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrefFinding {
    /// The full diagnostic message (e.g. ``label `<fig:x>` does not exist …``).
    pub message: String,
    /// The referenced label including angle brackets (`<fig:x>`), when the
    /// message exposed it.
    pub label: Option<String>,
    /// `path:line:col` from the diagnostic's location line, when present.
    pub location: Option<String>,
}

/// Whether a diagnostic message names an unresolved cross-reference. Typst
/// phrases it as ``label `<x>` does not exist in the document``; we also accept
/// a couple of defensive variants so a Typst wording change doesn't silently
/// drop the finding.
fn is_xref_message(message: &str) -> bool {
    let m = message.to_lowercase();
    m.contains("does not exist")
        || m.contains("unresolved reference")
        || (m.contains("label") && m.contains("unknown"))
}

/// Extract the referenced label (`<fig:x>`) from a diagnostic message, if it
/// spells one out between angle brackets.
fn extract_label(message: &str) -> Option<String> {
    let start = message.find('<')?;
    let end = message[start..].find('>')?;
    Some(message[start..=start + end].to_string())
}

/// Pull a `path:line:col` location out of a diagnostic's location line. Handles
/// the in-process engine's `  --> path:line:col` and the external binary's
/// box-drawing `  ┌─ path:line:col`, plus a bare `path:line:col`.
fn extract_location(line: &str) -> Option<String> {
    let stripped = line.trim().trim_start_matches(|c: char| {
        c == '-' || c == '>' || c == '┌' || c == '─' || c == '│' || c == '|' || c == ' '
    });
    let candidate = stripped.trim();
    // The last two colon-separated fields must be numeric (line, col). rsplitn
    // keeps any colons in the path itself in the final segment.
    let parts: Vec<&str> = candidate.rsplitn(3, ':').collect();
    if parts.len() == 3
        && !parts[0].is_empty()
        && parts[0].chars().all(|c| c.is_ascii_digit())
        && !parts[1].is_empty()
        && parts[1].chars().all(|c| c.is_ascii_digit())
        && !parts[2].is_empty()
    {
        Some(candidate.to_string())
    } else {
        None
    }
}

/// Scan a compiler stderr/stdout blob for unresolved cross-references. Returns
/// one finding per distinct (message, location); order follows first
/// appearance. Pure — no I/O, no emission.
pub fn scan_diagnostics(diagnostics: &str) -> Vec<XrefFinding> {
    let lines: Vec<&str> = diagnostics.lines().collect();
    let mut out: Vec<XrefFinding> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(rest) = line.trim_start().strip_prefix("error:") else {
            continue;
        };
        let message = rest.trim();
        if !is_xref_message(message) {
            continue;
        }
        // Look a few lines ahead for the location, stopping at the next block.
        let mut location = None;
        for look in lines.iter().skip(i + 1).take(4) {
            if look.trim_start().starts_with("error:")
                || look.trim_start().starts_with("warning:")
            {
                break;
            }
            if let Some(loc) = extract_location(look) {
                location = Some(loc);
                break;
            }
        }
        let finding = XrefFinding {
            message: message.to_string(),
            label: extract_label(message),
            location,
        };
        if !out.iter().any(|f| f.message == finding.message && f.location == finding.location) {
            out.push(finding);
        }
    }
    out
}

/// Emit each finding as a `kinds::XREF` Output-pane message. Grouped by
/// label+location so a repeated build collapses rather than piling up.
pub fn emit_findings(findings: &[XrefFinding]) {
    use crate::pane::output::{emit, kinds, Lifetime, Message, Severity};
    for f in findings {
        let label = f.label.clone().unwrap_or_default();
        let location = f.location.clone().unwrap_or_default();
        let text = if label.is_empty() {
            "unresolved cross-reference".to_string()
        } else {
            format!("unresolved cross-reference {label}")
        };
        let msg = Message::new(
            kinds::XREF,
            Severity::Warning,
            Lifetime::UntilActedOn,
            serde_json::json!({
                "text": text,
                "category": "xref",
                "label": label,
                "location": location,
                "detail": f.message,
            }),
        )
        .with_group_key(format!("xref:{label}:{location}"));
        emit(&msg);
    }
}

/// Collect Typst label _definitions_ (`<name>`) from source text — the targets
/// a cross-reference (`@name`) can point at. A valid label starts with a letter
/// and contains only word characters plus `:._-`, so comparisons and arrows
/// (`<=`, `->`, `a < b`) and version-like tokens are not mistaken for labels.
/// Deduplicated, in first-seen order. Backs the XREF-2 label picker.
pub fn collect_labels(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '<' {
            i += 1;
            continue;
        }
        // First char after `<` must be a letter for this to be a label.
        let mut valid = chars.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic());
        let mut name = String::new();
        let mut j = i + 1;
        while j < chars.len() && chars[j] != '>' {
            let c = chars[j];
            if c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '_' | '-') {
                name.push(c);
            } else {
                valid = false;
            }
            j += 1;
        }
        if j < chars.len() && valid && !name.is_empty() {
            // chars[j] == '>'
            if seen.insert(name.clone()) {
                out.push(name);
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// A human category for a prefixed label (`fig:` → "figure"), for the picker's
/// descriptor line. Unknown / unprefixed labels are just "label".
pub fn label_category(label: &str) -> &'static str {
    match label.split(':').next().unwrap_or("") {
        "fig" => "figure",
        "eq" | "eqt" | "eqn" => "equation",
        "tbl" | "tab" => "table",
        "sec" => "section",
        "thm" => "theorem",
        "lst" | "lstlisting" => "listing",
        "app" => "appendix",
        "alg" => "algorithm",
        _ => "label",
    }
}

/// Scan the diagnostics and emit any unresolved-cross-reference findings.
/// Returns the number emitted (0 → nothing to surface). Call from the
/// compile-failure path.
pub fn scan_and_emit(diagnostics: &str) -> usize {
    let findings = scan_diagnostics(diagnostics);
    emit_findings(&findings);
    findings.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_typst_unresolved_label_inprocess_format() {
        let stderr = "\
error: label `<fig:flux>` does not exist in the document
  --> /home/u/book/my-book.typ:42:8
  hint: check the label spelling
";
        let found = scan_diagnostics(stderr);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].label.as_deref(), Some("<fig:flux>"));
        assert_eq!(
            found[0].location.as_deref(),
            Some("/home/u/book/my-book.typ:42:8")
        );
    }

    #[test]
    fn recognises_external_binary_box_format() {
        let stderr = "\
error: label `<eq:energy>` does not exist in the document
  ┌─ /tmp/paper.typ:7:15
  │
7 │ See @eq:energy for details.
";
        let found = scan_diagnostics(stderr);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].label.as_deref(), Some("<eq:energy>"));
        assert_eq!(found[0].location.as_deref(), Some("/tmp/paper.typ:7:15"));
    }

    #[test]
    fn ignores_unrelated_compile_errors() {
        let stderr = "\
error: unexpected end of block
  --> /tmp/x.typ:3:1
error: unknown variable: foobar
  --> /tmp/x.typ:9:2
";
        assert!(scan_diagnostics(stderr).is_empty());
    }

    #[test]
    fn dedupes_repeated_identical_findings() {
        let stderr = "\
error: label `<fig:a>` does not exist in the document
  --> a.typ:1:1
error: label `<fig:a>` does not exist in the document
  --> a.typ:1:1
error: label `<fig:b>` does not exist in the document
  --> a.typ:2:1
";
        let found = scan_diagnostics(stderr);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn collect_labels_extracts_definitions_and_ignores_operators() {
        let src = "\
#figure(image(\"f.png\"), caption: [Flux]) <fig:flux>
$ E = m c^2 $ <eq:energy>
See @fig:flux. For a <= b and a -> c and x < y > z we want nothing.
= Intro <sec:intro>
Duplicate <fig:flux> again.
";
        let labels = collect_labels(src);
        assert_eq!(labels, vec!["fig:flux", "eq:energy", "sec:intro"]);
    }

    #[test]
    fn label_category_maps_common_prefixes() {
        assert_eq!(label_category("fig:x"), "figure");
        assert_eq!(label_category("eq:y"), "equation");
        assert_eq!(label_category("tbl:z"), "table");
        assert_eq!(label_category("sec:i"), "section");
        assert_eq!(label_category("whatever"), "label");
    }

    #[test]
    fn finding_without_location_still_captured() {
        let stderr = "error: label `<sec:intro>` does not exist in the document\n";
        let found = scan_diagnostics(stderr);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].label.as_deref(), Some("<sec:intro>"));
        assert!(found[0].location.is_none());
    }
}
