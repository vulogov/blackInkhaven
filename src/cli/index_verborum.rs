//! LEXICON — `inkhaven index-verborum`. Harvests the scholarly-lexicon terms that
//! actually appear in the manuscript and renders an Index Verborum — each term's
//! original-language form, its distinct senses, and the chapters that use it — as
//! Markdown / Typst / JSON.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::glossary::{self, GlossaryEntry};
use crate::index_verborum::{self, LexTerm, SenseRow, TermUsage};
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

    // The scholarly lexicon terms (Glossary entries with a form or senses).
    let mut lex_entries = glossary::glossary_entries_from_store(&store, &h, book_name);
    lex_entries.retain(|e| e.is_valid() && e.is_lexicon_term());
    let lexicon: Vec<LexTerm> = lex_entries.iter().map(to_lex_term).collect();

    let usages = harvest_usages(&layout, &h, book_name, &lex_entries)?;
    let entries = index_verborum::build(&lexicon, &usages);
    let heading = index_verborum::heading_for_language(&cfg.language);

    let rendered = match format.to_ascii_lowercase().as_str() {
        "md" | "markdown" => index_verborum::render_md(&entries, heading),
        "typ" | "typst" => index_verborum::render_typst(&entries, heading),
        "json" => index_verborum::render_json(&entries),
        other => {
            return Err(Error::Config(format!(
                "index-verborum: unknown --format `{other}` (use `md`, `typst`, or `json`)"
            )))
        }
    };

    match out {
        Some(p) => {
            std::fs::write(p, rendered.as_bytes()).map_err(Error::Io)?;
            println!("index-verborum: {} term(s) → {}", entries.len(), p.display());
        }
        None => print!("{rendered}"),
    }
    Ok(())
}

/// A Glossary lexicon entry → the pure builder's `LexTerm`.
fn to_lex_term(e: &GlossaryEntry) -> LexTerm {
    LexTerm {
        term: e.term.trim().to_string(),
        original_forms: e.original_forms.iter().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        senses: e
            .senses
            .iter()
            .map(|s| SenseRow { label: s.label.trim().to_string(), gloss: s.gloss.trim().to_string() })
            .collect(),
    }
}

/// Record `(term, chapter)` for every lexicon term that appears (whole-word, any
/// surface form) in a chapter's prose. Reads plain text (Typst stripped).
fn harvest_usages(
    layout: &ProjectLayout,
    h: &Hierarchy,
    book_name: Option<&str>,
    lex: &[GlossaryEntry],
) -> Result<Vec<TermUsage>> {
    // Precompute each term's surface forms once.
    let forms: Vec<(String, Vec<String>)> =
        lex.iter().map(|e| (e.term.trim().to_string(), e.surface_forms())).collect();

    let books: Vec<&Node> = match book_name {
        Some(_) => vec![super::resolve_user_book(h, book_name, "index-verborum").map_err(Error::Store)?],
        None => h
            .children_of(None)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
            .collect(),
    };
    let mut usages = Vec::new();
    for book in &books {
        for chapter in h.children_of(Some(book.id)).into_iter().filter(|n| n.kind == NodeKind::Chapter) {
            // Aggregate the chapter's prose (lowercased) once.
            let mut text = String::new();
            for id in h.collect_subtree(chapter.id) {
                let Some(n) = h.get(id) else { continue };
                if n.kind != NodeKind::Paragraph || n.content_type.as_deref() == Some("jinja") {
                    continue;
                }
                let Some(rel) = n.file.as_ref() else { continue };
                if let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) {
                    text.push_str(&crate::audiobook::typst_to_plain(&raw));
                    text.push('\n');
                }
            }
            let lc = text.to_lowercase();
            for (term, surface) in &forms {
                let used = surface
                    .iter()
                    .any(|f| crate::world::fact_check_lang::contains_word(&lc, f));
                if used {
                    usages.push(TermUsage { term: term.clone(), chapter: chapter.title.clone() });
                }
            }
        }
    }
    Ok(usages)
}
