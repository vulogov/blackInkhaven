//! TERMS-1 — `inkhaven terms …` terminal commands.
//!
//! - `check` — scan prose for banned synonyms of Glossary canonical terms and
//!   report every occurrence with its location. Exits non-zero when any are
//!   found, so it slots into a pre-build CI step. `--book` scopes to one book;
//!   `--json` emits a machine-readable report.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::{NodeKind, Store};
use crate::tui::style_warnings::BannedSynonymDetector;

use super::TermsCommand;

pub fn run(project: &Path, cmd: TermsCommand) -> Result<()> {
    match cmd {
        TermsCommand::Check { book, json } => check(project, book.as_deref(), json),
    }
}

fn open(project: &Path) -> Result<(Config, Store, Hierarchy)> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;
    Ok((cfg, store, hierarchy))
}

/// Read a paragraph body from disk (what assembly compiles), stripping a leading
/// `= Title` heading.
fn read_body(store: &Store, node: &Node) -> Option<String> {
    let rel = node.file.as_ref()?;
    let raw = std::fs::read_to_string(store.project_root().join(rel)).ok()?;
    let body = if raw.trim_start().starts_with("= ") {
        raw.splitn(2, '\n').nth(1).unwrap_or("").to_string()
    } else {
        raw
    };
    Some(body)
}

struct TermFinding {
    path: String,
    line: usize,
    synonym: String,
    canonical: String,
}

fn check(project: &Path, book: Option<&str>, json: bool) -> Result<()> {
    let (_cfg, store, h) = open(project)?;

    // Target books: one named book, or every user book.
    let user_books: Vec<&Node> = match book {
        Some(_) => vec![super::resolve_user_book(&h, book, "terms check").map_err(Error::Store)?],
        None => h
            .children_of(None)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
            .collect(),
    };

    let mut findings: Vec<TermFinding> = Vec::new();
    let mut paragraphs_scanned = 0usize;
    for book in &user_books {
        // The detector is scoped to this book (global + this book's entries).
        // Suppression set is empty until T-P4 wires the intent ledger.
        let detector =
            BannedSynonymDetector::from_store(&store, &h, Some(&book.slug), Default::default());
        if detector.is_empty() {
            continue; // no glossary entries apply — nothing to flag in this book
        }
        for id in h.collect_subtree(book.id) {
            let Some(node) = h.get(id) else { continue };
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            let Some(body) = read_body(&store, node) else { continue };
            paragraphs_scanned += 1;
            let path = h.slug_path(node);
            for (i, line) in body.lines().enumerate() {
                for hit in detector.detect(line) {
                    if let Some((synonym, canonical)) = detector.hint_at(line, hit.col_start) {
                        findings.push(TermFinding {
                            path: path.clone(),
                            line: i + 1,
                            synonym,
                            canonical,
                        });
                    }
                }
            }
        }
    }

    if json {
        emit_json(paragraphs_scanned, &findings);
    } else {
        emit_human(paragraphs_scanned, &findings);
    }

    if findings.is_empty() {
        Ok(())
    } else {
        // Non-zero exit for CI; the report is already printed.
        std::process::exit(1);
    }
}

fn emit_human(scanned: usize, findings: &[TermFinding]) {
    if findings.is_empty() {
        println!("terms check: OK — no banned synonyms in {scanned} paragraph(s).");
        return;
    }
    println!(
        "terms check: {} banned-synonym occurrence(s) in {scanned} paragraph(s):",
        findings.len()
    );
    for f in findings {
        println!(
            "  {} line {}: \"{}\" → use \"{}\"",
            f.path, f.line, f.synonym, f.canonical
        );
    }
    println!("\nUse the canonical form, or declare the variant deliberate in the Glossary.");
}

fn json_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' | '\t' => out.push(' '),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn emit_json(scanned: usize, findings: &[TermFinding]) {
    let mut s = String::from("{\n");
    s.push_str(&format!("  \"paragraphs_scanned\": {scanned},\n"));
    s.push_str(&format!("  \"finding_count\": {},\n", findings.len()));
    s.push_str("  \"findings\": [");
    for (i, f) in findings.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "\n    {{ \"path\": {}, \"line\": {}, \"synonym\": {}, \"canonical\": {} }}",
            json_str(&f.path),
            f.line,
            json_str(&f.synonym),
            json_str(&f.canonical),
        ));
    }
    if !findings.is_empty() {
        s.push_str("\n  ");
    }
    s.push_str("]\n}");
    println!("{s}");
}
