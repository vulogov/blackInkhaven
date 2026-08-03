//! SENTINEL-1 CT-P7 — the on-demand slow coherence pass.
//!
//! The one fuzzy detector SENTINEL will *invoke* but never run automatically: an
//! LLM reads a run of paragraphs and flags cross-paragraph contradictions the
//! deterministic detectors can't see — a fact asserted then quietly reversed, a
//! time-of-day that can't follow, a name that changes. Explicit, cost-capped,
//! opt-in (the permissive principle).
//!
//! It *reuses* the world fact-checker's coherence machinery (`COHERENCE_SYSTEM` +
//! `build_coherence_prompt` + the cost-capped `slow_llm_call`) — no prompt or
//! call logic is re-implemented. Findings arrive tagged `source:"coherence"`.

use std::path::Path;

use uuid::Uuid;

use super::{ContinuityFinding, Severity};
use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::Store;

/// Run the LLM coherence pass over the paragraphs under `scope` (a book/chapter
/// node), or every user-book paragraph when `None`, in reading order. Returns the
/// cross-paragraph contradictions as continuity findings (`source:"coherence"`).
/// Cost-capped; tolerates a missing world.hjson (the magic ledger, when present,
/// still excuses declared exceptions). Self-contained (opens its own store), so it
/// is safe to call from a background worker.
pub(crate) fn run(
    project: &Path,
    scope: Option<Uuid>,
    max_cost: usize,
    force: bool,
) -> Result<Vec<ContinuityFinding>, String> {
    use crate::world::fact_check_slow::{build_coherence_prompt, magic_summary, COHERENCE_SYSTEM};

    let layout = ProjectLayout::new(project);
    layout.require_initialized().map_err(|e| e.to_string())?;
    let cfg = Config::load_layered(&layout.config_path()).map_err(|e| e.to_string())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| e.to_string())?;
    let h = Hierarchy::load(&store).map_err(|e| e.to_string())?;

    let labeled = gather_paragraphs(&store, &h, scope);
    if labeled.iter().all(|(_, t)| t.trim().is_empty()) {
        return Ok(Vec::new());
    }

    // Magic ledger (empty when no world.hjson) so a declared teleportation rule
    // still excuses a co-location. The world summary is left empty — continuity
    // cares about cross-paragraph contradiction, not world flavour.
    let ledger = std::fs::read_to_string(layout.root.join("world.hjson"))
        .ok()
        .and_then(|raw| crate::world::types::WorldDefinition::from_hjson(&raw).ok())
        .and_then(|d| d.magic)
        .unwrap_or_default();
    let magic = magic_summary(&ledger);

    let (prompt, _kept) = build_coherence_prompt(&labeled, "", &magic);
    let findings = crate::cli::realworld::slow_llm_call(
        project,
        "continuity-coherence",
        COHERENCE_SYSTEM,
        prompt,
        max_cost,
        force,
    )
    .map_err(|e| e.to_string())?;

    Ok(findings.into_iter().enumerate().map(|(i, f)| map_finding(i, f)).collect())
}

/// Reading-order `(label, text)` for paragraphs under `scope` (or every user-book
/// paragraph). Event paragraphs (timeline metadata) are excluded.
fn gather_paragraphs(store: &Store, h: &Hierarchy, scope: Option<Uuid>) -> Vec<(String, String)> {
    let ids: Vec<Uuid> = match scope {
        Some(root) => h
            .collect_subtree(root)
            .into_iter()
            .filter(|id| {
                h.get(*id)
                    .map(|n| n.kind == NodeKind::Paragraph && n.event.is_none())
                    .unwrap_or(false)
            })
            .collect(),
        None => h
            .flatten()
            .into_iter()
            .filter(|(n, _)| n.kind == NodeKind::Paragraph && n.event.is_none())
            .filter(|(n, _)| under_user_book(h, n))
            .map(|(n, _)| n.id)
            .collect(),
    };
    ids.into_iter()
        .map(|id| {
            let label = h.get(id).map(|n| h.slug_path(n)).unwrap_or_else(|| id.to_string());
            let text = store
                .get_content(id)
                .ok()
                .flatten()
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default();
            (label, text)
        })
        .collect()
}

/// True when `node`'s nearest Book ancestor is a user (non-system) book.
fn under_user_book(h: &Hierarchy, node: &Node) -> bool {
    let mut cur = Some(node);
    while let Some(n) = cur {
        if n.kind == NodeKind::Book {
            return n.system_tag.is_none();
        }
        cur = n.parent_id.and_then(|p| h.get(p));
    }
    false
}

fn map_finding(i: usize, f: crate::world::fact_check::Finding) -> ContinuityFinding {
    let severity = match f.severity.as_str() {
        "contradiction" => Severity::Contradiction,
        "warning" => Severity::Warning,
        _ => Severity::Info,
    };
    ContinuityFinding {
        kind: "coherence",
        severity,
        chapter: 0,
        anchor: None,
        // The LLM findings carry no roster entities; key on the index + message so
        // distinct coherence findings survive the ledger dedup.
        dedup_key: format!("coherence|{i}|{}", f.body),
        entities: Vec::new(),
        message: f.body,
        source: "coherence",
    }
}
