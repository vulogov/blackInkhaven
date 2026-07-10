//! INDEX-1 — `inkhaven index`. Builds a back-of-book index from the Glossary's
//! canonical terms (+ `docs.index.terms`), locating each in the manuscript, and
//! renders it as Markdown / Typst / JSON.

use std::path::Path;

use crate::book_index::{self, IndexEntry, IndexUnit};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::Store;

pub fn run(project: &Path, book_name: Option<&str>, format: &str, out: Option<&Path>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;

    // Terms: Glossary canonical terms (+ synonyms as see-refs) + config extras.
    let mut terms: Vec<String> = Vec::new();
    let mut see_refs: Vec<(String, String)> = Vec::new();
    if cfg.docs.index.from_glossary {
        for e in crate::glossary::glossary_entries_from_store(&store, &h, book_name) {
            for syn in &e.synonyms {
                see_refs.push((syn.clone(), e.term.clone()));
            }
            terms.push(e.term);
        }
    }
    terms.extend(cfg.docs.index.terms.iter().cloned());
    if terms.is_empty() {
        return Err(Error::Config(
            "index: no terms to index — add Glossary entries or list them in `docs.index.terms`".into(),
        ));
    }

    let units = gather_units(&layout, &h, book_name)?;
    let entries = book_index::build(&terms, &see_refs, &units);

    let rendered = match format.to_ascii_lowercase().as_str() {
        "md" | "markdown" => render_md(&entries),
        "typ" | "typst" => render_typst(&entries),
        "json" => render_json(&entries),
        other => {
            return Err(Error::Config(format!(
                "index: unknown --format `{other}` (use `md`, `typst`, or `json`)"
            )))
        }
    };

    match out {
        Some(p) => {
            std::fs::write(p, rendered.as_bytes()).map_err(Error::Io)?;
            println!("index: {} entry(ies) → {}", entries.len(), p.display());
        }
        None => print!("{rendered}"),
    }
    Ok(())
}

/// One index unit per manuscript paragraph, tagged with its chapter.
fn gather_units(layout: &ProjectLayout, h: &Hierarchy, book_name: Option<&str>) -> Result<Vec<IndexUnit>> {
    let books: Vec<&Node> = match book_name {
        Some(_) => vec![super::resolve_user_book(h, book_name, "index").map_err(Error::Store)?],
        None => h
            .children_of(None)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
            .collect(),
    };
    let mut units = Vec::new();
    for book in &books {
        for chapter in h.children_of(Some(book.id)).into_iter().filter(|n| n.kind == NodeKind::Chapter) {
            for id in h.collect_subtree(chapter.id) {
                let Some(n) = h.get(id) else { continue };
                if n.kind != NodeKind::Paragraph {
                    continue;
                }
                let Some(rel) = n.file.as_ref() else { continue };
                let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) else { continue };
                units.push(IndexUnit {
                    chapter: chapter.title.clone(),
                    anchor: chapter.title.clone(),
                    text: crate::audiobook::typst_to_plain(&raw),
                });
            }
        }
    }
    Ok(units)
}

fn render_md(entries: &[IndexEntry]) -> String {
    let mut s = String::from("# Index\n\n");
    for e in entries {
        match &e.see {
            Some(canonical) => s.push_str(&format!("**{}** — *see* {}\n\n", e.term, canonical)),
            None => {
                let locs: Vec<&str> = e.locations.iter().map(|l| l.chapter.as_str()).collect();
                s.push_str(&format!("**{}** — {}\n\n", e.term, locs.join(", ")));
            }
        }
    }
    s
}

fn render_typst(entries: &[IndexEntry]) -> String {
    let mut s = String::from("= Index\n\n");
    for e in entries {
        match &e.see {
            Some(canonical) => s.push_str(&format!("*{}* — _see_ {}\n\n", e.term, canonical)),
            None => {
                let locs: Vec<String> = e.locations.iter().map(|l| l.chapter.clone()).collect();
                s.push_str(&format!("*{}* — {}\n\n", e.term, locs.join(", ")));
            }
        }
    }
    s
}

fn render_json(entries: &[IndexEntry]) -> String {
    let arr: Vec<_> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "term": e.term,
                "see": e.see,
                "locations": e.locations.iter()
                    .map(|l| serde_json::json!({ "chapter": l.chapter, "anchor": l.anchor }))
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({ "index": arr, "count": entries.len() }))
        .unwrap_or_else(|_| "{}".into())
}
