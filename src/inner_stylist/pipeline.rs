//! Inner Stylist pipeline (CH-P7) — run every CHORUS pillar over a book and
//! synthesise the findings. Deterministic (no LLM): the dialogue + prose stores
//! cache by content hash, so repeated runs are cheap.

use crate::config::Config;
use crate::dialogue::{DialogueStore, refresh_book};
use crate::project::ProjectLayout;
use crate::prose::ProseStore;
use crate::prose::violations::Violation;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

use super::Finding;
use super::fast::synthesize;

/// Gather every pillar's findings for `book`. `now` is the RFC3339 timestamp for
/// the (cached) dialogue/prose recompute.
pub(crate) fn gather(
    store: &Store,
    layout: &ProjectLayout,
    h: &Hierarchy,
    cfg: &Config,
    book: &Node,
    now: &str,
) -> Result<Vec<Finding>, String> {
    let root = store.project_root();

    // Pillar A — character voices → distinctiveness + drift.
    let ds = DialogueStore::open(root).map_err(|e| e.to_string())?;
    refresh_book(&ds, layout, h, cfg, book, None, now).map_err(|e| e.to_string())?;
    let pstore = ProseStore::open(root).map_err(|e| e.to_string())?;
    let voices = crate::chorus::voices::character_profiles(&pstore, &ds, cfg, book, None, now)
        .map_err(|e| e.to_string())?;
    let matrix = crate::chorus::distinct::matrix(
        &voices,
        cfg.chorus.distinct_threshold,
        &cfg.chorus.distinct_ignore_pairs,
    );
    let drifts: Vec<(String, Vec<Violation>)> = voices
        .iter()
        .map(|v| (v.name.clone(), crate::chorus::drift::character_drift(v, &cfg.prose.thresholds)))
        .filter(|(_, d)| !d.is_empty())
        .collect();

    // Pillar B — POV/head-hop + tense. Pillar C — register.
    let head_hops = crate::chorus::pov::scan_head_hops(layout, h, cfg, book);
    let tense = crate::chorus::tense::scan_tense(layout, h, cfg, book);
    let register = crate::chorus::register::scan_register(layout, h, cfg, book);

    Ok(synthesize(&matrix, &drifts, &head_hops, &tense, &register))
}
