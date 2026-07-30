//! RESRCH-1 — paragraph insertion into the Facts / Notes books. Shared by manual
//! entry (`n`, R-P4) and `/fact` / `/note` (R-P10/R-P11).
//!
//! Uses the real store primitives (audit correction: there is no
//! `create_paragraph`): `Store::create_node` then `update_paragraph_content`,
//! which **auto-reembeds** into the shared HNSW — so the inserted paragraph is
//! immediately retrievable by `/diff` and every Facts consumer, no manual embed.
//! The caller reloads the `Hierarchy` afterwards.

use anyhow::{Context, Result};
use uuid::Uuid;

use crate::config::Config;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::{InsertPosition, NodeKind, Store};

/// Resolve where a new paragraph lands relative to a target node:
/// - target is a Book / Chapter / Subchapter → a child at the end of it;
/// - target is a Paragraph → a sibling immediately after it.
/// Returns `(parent_node_clone, position)`.
fn resolve_target<'a>(h: &'a Hierarchy, target: Option<Uuid>) -> Option<(Node, InsertPosition)> {
    let target = target?;
    let node = h.get(target)?;
    match node.kind {
        NodeKind::Book | NodeKind::Chapter | NodeKind::Subchapter => {
            Some((node.clone(), InsertPosition::End))
        }
        NodeKind::Paragraph => {
            let parent = node.parent_id.and_then(|p| h.get(p))?;
            Some((parent.clone(), InsertPosition::After(target)))
        }
        _ => None,
    }
}

/// Insert a paragraph titled `title` with body `body` near `target` (or at the
/// end of `book_id` when `target` is `None` / unresolvable). Returns the new
/// node's id. The caller is responsible for reloading the hierarchy.
pub(crate) fn insert_paragraph(
    store: &Store,
    cfg: &Config,
    h: &Hierarchy,
    book_id: Uuid,
    target: Option<Uuid>,
    title: &str,
    body: &str,
) -> Result<Uuid> {
    let (parent, position) = resolve_target(h, target).or_else(|| {
        h.get(book_id).map(|b| (b.clone(), InsertPosition::End))
    }).context("no valid insertion parent (Facts book missing?)")?;

    let title = if title.trim().is_empty() { "Untitled" } else { title.trim() };
    let mut node = store
        .create_node(cfg, h, NodeKind::Paragraph, title, Some(&parent), None, position)
        .context("create paragraph node")?;
    // create_node seeds the `.typ` file with `= {title}`. Overwrite it with the
    // real body FIRST (the editor reads the `.typ` file), THEN update the DB
    // blob + reembed — the canonical two-step the store's own creators use.
    if let Some(rel) = &node.file {
        let abs = store.project_root().join(rel);
        crate::io_atomic::write(&abs, body.as_bytes()).context("write paragraph .typ body")?;
    }
    store
        .update_paragraph_content(&mut node, body.as_bytes())
        .context("write paragraph body (reembeds)")?;
    Ok(node.id)
}
