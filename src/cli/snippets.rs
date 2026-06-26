//! REUSE-1 — `inkhaven snippets …` terminal commands.
//!
//! - `list`  — the snippets defined in the Snippets book + how many times each
//!   is referenced across the project.
//! - `check` — validate every `#include "…/snippets/<slug>.typ"` in prose against
//!   the defined snippets. Reports **missing** references (error → exit 1) and
//!   **orphaned** snippets (defined but unreferenced → warning).

use std::collections::HashMap;
use std::path::Path;

use crate::config::Config;
use crate::error::Result;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::{NodeKind, Store, SYSTEM_TAG_SNIPPETS};

use super::SnippetsCommand;

pub fn run(project: &Path, cmd: SnippetsCommand) -> Result<()> {
    match cmd {
        SnippetsCommand::List { json } => list(project, json),
        SnippetsCommand::Check { book, json } => check(project, book.as_deref(), json),
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

fn read_body(store: &Store, node: &Node) -> Option<String> {
    let rel = node.file.as_ref()?;
    let raw = std::fs::read_to_string(store.project_root().join(rel)).ok()?;
    Some(raw)
}

/// The slugs of the paragraphs under the Snippets book.
fn defined_slugs(h: &Hierarchy) -> Vec<String> {
    let Some(book) = h
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_SNIPPETS))
    else {
        return Vec::new();
    };
    h.collect_subtree(book.id)
        .into_iter()
        .filter_map(|id| h.get(id))
        .filter(|n| n.kind == NodeKind::Paragraph)
        .map(|n| n.slug.clone())
        .collect()
}

/// Every `#include` snippet reference across user books, with its location.
struct Reference {
    path: String,
    line: usize,
    slug: String,
}

fn collect_references(store: &Store, h: &Hierarchy, book_filter: Option<&str>) -> Vec<Reference> {
    let mut refs = Vec::new();
    for book in h
        .children_of(None)
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
    {
        if let Some(f) = book_filter {
            if book.slug != f && book.title != f {
                continue;
            }
        }
        for id in h.collect_subtree(book.id) {
            let Some(node) = h.get(id) else { continue };
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            let Some(body) = read_body(store, node) else { continue };
            let path = h.slug_path(node);
            for (i, line) in body.lines().enumerate() {
                for slug in crate::typst_check::snippet_references(line) {
                    refs.push(Reference { path: path.clone(), line: i + 1, slug });
                }
            }
        }
    }
    refs
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

fn list(project: &Path, json: bool) -> Result<()> {
    let (_cfg, store, h) = open(project)?;
    let slugs = defined_slugs(&h);
    // Reference counts across all user books.
    let mut counts: HashMap<String, usize> = slugs.iter().map(|s| (s.clone(), 0)).collect();
    for r in collect_references(&store, &h, None) {
        *counts.entry(r.slug).or_insert(0) += 1;
    }

    if json {
        let mut s = String::from("[");
        for (i, slug) in slugs.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!(
                "\n  {{ \"slug\": {}, \"references\": {} }}",
                json_str(slug),
                counts.get(slug).copied().unwrap_or(0)
            ));
        }
        if !slugs.is_empty() {
            s.push('\n');
        }
        s.push(']');
        println!("{s}");
        return Ok(());
    }

    if slugs.is_empty() {
        println!("No snippets defined in the Snippets book.");
        return Ok(());
    }
    println!("{} snippet(s):", slugs.len());
    for slug in &slugs {
        let n = counts.get(slug).copied().unwrap_or(0);
        println!("  {slug:<28} {n} reference(s)");
    }
    Ok(())
}

fn check(project: &Path, book: Option<&str>, json: bool) -> Result<()> {
    let (_cfg, store, h) = open(project)?;
    let defined: std::collections::HashSet<String> = defined_slugs(&h).into_iter().collect();
    let references = collect_references(&store, &h, book);

    // Missing: a reference to a slug that isn't defined.
    let missing: Vec<&Reference> = references
        .iter()
        .filter(|r| !defined.contains(&r.slug))
        .collect();
    // Orphaned: a defined snippet that nothing references (only meaningful for a
    // whole-project scan).
    let referenced: std::collections::HashSet<&str> =
        references.iter().map(|r| r.slug.as_str()).collect();
    let orphaned: Vec<&String> = if book.is_none() {
        defined.iter().filter(|s| !referenced.contains(s.as_str())).collect()
    } else {
        Vec::new()
    };

    if json {
        let mut s = String::from("{\n  \"missing\": [");
        for (i, r) in missing.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!(
                "\n    {{ \"slug\": {}, \"path\": {}, \"line\": {} }}",
                json_str(&r.slug),
                json_str(&r.path),
                r.line
            ));
        }
        if !missing.is_empty() {
            s.push_str("\n  ");
        }
        s.push_str("],\n  \"orphaned\": [");
        for (i, o) in orphaned.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!("\n    {}", json_str(o)));
        }
        if !orphaned.is_empty() {
            s.push_str("\n  ");
        }
        s.push_str("]\n}");
        println!("{s}");
    } else if missing.is_empty() && orphaned.is_empty() {
        println!("snippets check: OK — every #include resolves; no orphans.");
    } else {
        if !missing.is_empty() {
            println!("snippets check: {} missing reference(s):", missing.len());
            for r in &missing {
                println!("  {} line {}: no snippet `{}`", r.path, r.line, r.slug);
            }
        }
        if !orphaned.is_empty() {
            println!("snippets check: {} orphaned snippet(s) (defined, unreferenced):", orphaned.len());
            let mut o: Vec<&String> = orphaned.clone();
            o.sort();
            for slug in o {
                println!("  {slug}");
            }
        }
    }

    // Missing references are errors (exit 1); orphans are warnings (exit 0).
    if missing.is_empty() {
        Ok(())
    } else {
        std::process::exit(1);
    }
}
