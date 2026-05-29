//! 1.2.12+ — `inkhaven export-concordance`.  Emits the
//! project-wide concordance (every distinct lexical stem
//! with count + KWIC samples) to CSV or JSON for use in
//! spreadsheets / analysis pipelines.
//!
//! Same data the `Ctrl+B Shift+L` modal in the TUI shows.
//! System books (Prompts / Characters / Places / Lore /
//! Help / Notes / Artefacts / Typst / Scripts) are
//! excluded from the corpus — same scope as the in-TUI
//! view.  See `Documentation/PROPOSALS/MULTILINGUAL_PROMPTS.md`
//! for the multilingual plumbing the builder consumes.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::{NodeKind, Store};
use crate::store::hierarchy::Hierarchy;
use crate::tui::concordance::{
    build, ConcordanceData, ConcordanceEntry, ParagraphInput,
};

use super::ConcordanceExportFormat;

pub fn run(
    project: &Path,
    format: ConcordanceExportFormat,
    output: &Path,
    min_count: usize,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;

    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    // Build the corpus.  Same walk the TUI's
    // `open_concordance` uses: skip non-paragraphs,
    // skip system-book descendants, read each
    // paragraph's body from bdslib, strip the leading
    // typst heading line.
    let mut bodies: Vec<(String, Vec<String>)> = Vec::new();
    for node in hierarchy.iter() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        if hierarchy
            .ancestors(node)
            .iter()
            .any(|a| a.system_tag.is_some())
        {
            continue;
        }
        let slug_path = hierarchy.slug_path(node);
        let raw = match store.get_content(node.id) {
            Ok(Some(bytes)) => bytes,
            _ => continue,
        };
        let text = match std::str::from_utf8(&raw) {
            Ok(s) => strip_leading_typst_heading(s),
            Err(_) => continue,
        };
        let lines: Vec<String> =
            text.split('\n').map(|s| s.to_string()).collect();
        bodies.push((slug_path, lines));
    }
    if bodies.is_empty() {
        return Err(Error::Config(
            "export-concordance: project has no paragraphs to analyse".into(),
        ));
    }

    let inputs: Vec<ParagraphInput<'_>> = bodies
        .iter()
        .map(|(slug, lines)| ParagraphInput {
            slug_path: slug.clone(),
            lines,
        })
        .collect();
    let data = build(
        &cfg.editor.style_warnings.repeated_phrases,
        &cfg.language,
        &inputs,
    );

    // Apply min_count threshold filter.
    let entries: Vec<&ConcordanceEntry> = data
        .entries
        .iter()
        .filter(|e| e.count >= min_count)
        .collect();

    match format {
        ConcordanceExportFormat::Csv => write_csv(output, &entries),
        ConcordanceExportFormat::Json => write_json(output, &data, &entries),
    }?;

    eprintln!(
        "wrote {} stem(s) ({} total scanned, {} paragraphs) to {}",
        entries.len(),
        data.distinct_words,
        data.paragraphs_scanned,
        output.display(),
    );
    Ok(())
}

/// Strip the leading `= Heading` line + any blank
/// lines after it.  Mirrors the helper the TUI's
/// concordance builder uses internally.  Duplicated
/// here (not extracted) because the TUI helper is
/// `pub(super)`-scoped and the CLI tree doesn't see
/// it; the function is one screen, copying is
/// cheaper than carving out a module-visibility hole.
fn strip_leading_typst_heading(body: &str) -> String {
    let mut lines: Vec<&str> = body.lines().collect();
    if let Some(first) = lines.first() {
        if first.trim_start().starts_with('=') {
            lines.remove(0);
            while lines.first().is_some_and(|l| l.trim().is_empty()) {
                lines.remove(0);
            }
        }
    }
    lines.join("\n")
}

fn write_csv(output: &Path, entries: &[&ConcordanceEntry]) -> Result<()> {
    let mut buf = String::new();
    buf.push_str("headword,stem,count,variants,sample_paths\n");
    for entry in entries {
        let variants = entry.variants.join("|");
        let sample_paths = entry
            .samples
            .iter()
            .map(|s| s.slug_path.as_str())
            .collect::<Vec<_>>()
            .join(";");
        buf.push_str(&format!(
            "{},{},{},{},{}\n",
            csv_quote(&entry.headword),
            csv_quote(&entry.stem),
            entry.count,
            csv_quote(&variants),
            csv_quote(&sample_paths),
        ));
    }
    std::fs::write(output, buf).map_err(Error::Io)?;
    Ok(())
}

fn write_json(
    output: &Path,
    data: &ConcordanceData,
    entries: &[&ConcordanceEntry],
) -> Result<()> {
    // Compose a serde_json::Value tree by hand —
    // ConcordanceEntry / ConcordanceSample don't derive
    // Serialize (they live in the TUI module which has
    // no serde dep on those types).  Hand-rolling
    // keeps this CLI free of any dependency change.
    let entries_json: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "headword": e.headword,
                "stem": e.stem,
                "count": e.count,
                "variants": e.variants,
                "samples": e.samples.iter().map(|s| serde_json::json!({
                    "slug_path": s.slug_path,
                    "line_no": s.line_no,
                    "kwic": s.kwic,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let payload = serde_json::json!({
        "total_tokens": data.total_tokens,
        "distinct_words": data.distinct_words,
        "paragraphs_scanned": data.paragraphs_scanned,
        "entries": entries_json,
    });
    let body = serde_json::to_string_pretty(&payload)
        .map_err(|e| Error::Config(format!("serialize JSON: {e}")))?;
    std::fs::write(output, body).map_err(Error::Io)?;
    Ok(())
}

/// RFC-4180-ish CSV quoting: wrap in double quotes
/// when the value contains a comma, newline, or
/// double quote; escape inner double quotes by
/// doubling them.  Plain ASCII values pass through
/// unchanged.
fn csv_quote(value: &str) -> String {
    let needs_quoting = value.contains(',')
        || value.contains('\n')
        || value.contains('\r')
        || value.contains('"');
    if !needs_quoting {
        return value.to_string();
    }
    let escaped = value.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_quote_passes_plain_values() {
        assert_eq!(csv_quote("walk"), "walk");
        assert_eq!(csv_quote("walked"), "walked");
    }

    #[test]
    fn csv_quote_wraps_commas() {
        assert_eq!(csv_quote("a, b"), "\"a, b\"");
    }

    #[test]
    fn csv_quote_escapes_inner_double_quotes() {
        assert_eq!(csv_quote("she said \"hi\""), "\"she said \"\"hi\"\"\"");
    }

    #[test]
    fn strip_heading_drops_first_line_and_blanks() {
        let body = "= Title\n\n\nFirst line.\nSecond.";
        assert_eq!(strip_leading_typst_heading(body), "First line.\nSecond.");
    }

    #[test]
    fn strip_heading_keeps_body_without_heading() {
        let body = "First line.\nSecond.";
        assert_eq!(strip_leading_typst_heading(body), "First line.\nSecond.");
    }
}
