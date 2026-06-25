//! `inkhaven book-rag` — the terminal counterpart to the AI pane's Book
//! scope (BOOK_RAG-1, "Chat with Your Book").
//!
//! `retrieve <query>` runs the exact same retrieval the TUI grounds its
//! Book-scope chat on — semantic search over the vecstore, filtered to the
//! book + included author-content system books, expanded with surrounding
//! paragraphs and token-budgeted — and prints the passages (or the composed
//! grounding context with `--context`). No LLM call, no config writes: it is
//! the inspect-what-the-model-sees tool. It reuses
//! `book_rag::retrieval::retrieve`, so the CLI and the pane never drift.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;

/// `inkhaven book-rag retrieve <query> [--book-name N] [--top-k K] [--context]`.
#[allow(clippy::too_many_arguments)]
pub fn retrieve(
    project: &Path,
    query: &str,
    book_name: Option<&str>,
    top_k: Option<usize>,
    context: bool,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let h = Hierarchy::load(&store)?;

    let book = crate::cli::resolve_user_book(&h, book_name, "book-rag retrieve")
        .map_err(Error::Store)?;

    // A per-invocation top-k override leaves the project config untouched.
    let mut rag_cfg = cfg.book_rag.clone();
    if let Some(k) = top_k {
        rag_cfg.top_k = k.max(1);
    }

    let passages = crate::book_rag::retrieval::retrieve(&store, &h, &rag_cfg, book.id, query)
        .map_err(Error::Store)?;

    if passages.is_empty() {
        eprintln!("No passages in `{}` matched the query semantically.", book.title);
        return Ok(());
    }

    // `--context` prints the exact grounding block the model receives;
    // otherwise a human-readable per-passage listing.
    if context {
        println!("{}", crate::book_rag::compose_context_prefix(&passages));
        return Ok(());
    }

    let hits = passages.iter().filter(|p| p.is_hit).count();
    let total_tokens: usize = passages
        .iter()
        .map(|p| crate::book_rag::estimate_tokens(&p.body))
        .sum();
    println!(
        "Book-RAG retrieval — `{}`  ({} passage{}, {} direct hit{}, ~{} tokens)",
        book.title,
        passages.len(),
        if passages.len() == 1 { "" } else { "s" },
        hits,
        if hits == 1 { "" } else { "s" },
        total_tokens,
    );
    println!();
    for p in &passages {
        let marker = if p.is_hit { "★" } else { " " };
        println!("{marker} {:>5.3}  {}", p.score, p.breadcrumb);
        println!("        id: {}", p.id);
        let snippet = first_line(&p.body);
        if !snippet.is_empty() {
            println!("        {snippet}");
        }
        println!();
    }
    Ok(())
}

/// First non-empty line of the passage body, truncated for the listing.
fn first_line(body: &str) -> String {
    let line = body.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    if line.chars().count() > 100 {
        let head: String = line.chars().take(100).collect();
        format!("{head}…")
    } else {
        line.to_string()
    }
}
