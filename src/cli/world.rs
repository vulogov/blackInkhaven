//! 1.3.11 WORLD-3 — `inkhaven world`: the consolidated consistency snapshot.
//!
//! Aggregates the world-layer sidecars (facts check / facts scan / drift /
//! continuity) + deterministic counts (Facts-book size, entity coverage,
//! anachronisms) into a [`WorldReport`] and renders a sectioned dashboard.
//! Deterministic by default (reads computed sidecars, no AI); `--deep`
//! refreshes the AI scans first.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::{
    Store, SYSTEM_TAG_ARTEFACTS, SYSTEM_TAG_CHARACTERS, SYSTEM_TAG_FACTS, SYSTEM_TAG_PLACES,
};
use crate::world_report::WorldReport;

pub fn run(project: &Path, json: bool, deep: bool, provider: Option<&str>) -> Result<()> {
    if deep {
        if json {
            return Err(Error::Store(
                "world: --deep can't combine with --json (the AI scans print progress) — run the scans separately, then `world --json`".into(),
            ));
        }
        deep_refresh(project, provider);
    }
    let report = gather(project)?;
    if json {
        let out = serde_json::to_string_pretty(&report)
            .map_err(|e| Error::Store(format!("world: {e}")))?;
        println!("{out}");
    } else {
        render(&report);
    }
    Ok(())
}

/// Load the sidecars + walk the store to build the snapshot. Tolerates any
/// missing sidecar (counts as zero / empty), so it works on a project that
/// hasn't run every scan.
pub fn gather(project: &Path) -> Result<WorldReport> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    Ok(report_from(&store, &h, &cfg, &layout.root))
}

/// Build the snapshot from an already-open store / hierarchy / config — used
/// by `gather` and by the TUI story bible (which already holds them, so it
/// must not reopen the project). Infallible: missing sidecars count as zero.
pub fn report_from(store: &Store, h: &Hierarchy, cfg: &Config, root: &Path) -> WorldReport {
    let facts_check = crate::facts_scan::FactCheckReport::load(root).unwrap_or_default();
    let facts_scan = crate::facts_scan::FactScanReport::load(root).unwrap_or_default();
    let drift = crate::drift::DriftReport::load(root).unwrap_or_default();
    let continuity = crate::continuity_bible::ContinuityBible::load(root).unwrap_or_default();

    WorldReport {
        facts_total: count_paragraphs(h, Some(SYSTEM_TAG_FACTS)),
        facts_conflicts: facts_check.conflicts,
        facts_prose_findings: facts_scan.findings.len(),
        drift_conflicts: drift.conflicts,
        continuity_attributes: continuity.facts.len(),
        characters: count_paragraphs(h, Some(SYSTEM_TAG_CHARACTERS)),
        places: count_paragraphs(h, Some(SYSTEM_TAG_PLACES)),
        artefacts: count_paragraphs(h, Some(SYSTEM_TAG_ARTEFACTS)),
        anachronisms: count_anachronisms(cfg, store, h),
    }
}

/// Count the paragraphs inside the system book carrying `tag`.
fn count_paragraphs(h: &Hierarchy, tag: Option<&str>) -> usize {
    let Some(book) = h
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == tag)
    else {
        return 0;
    };
    h.collect_subtree(book.id)
        .into_iter()
        .filter(|id| h.get(*id).map(|n| n.kind) == Some(NodeKind::Paragraph))
        .count()
}

/// Count anachronistic terms across the user books (the deterministic detector
/// — off, and thus zero, until `anachronism.year` is set).
fn count_anachronisms(cfg: &Config, store: &Store, h: &Hierarchy) -> usize {
    let det =
        crate::tui::style_warnings::AnachronismDetector::new(&cfg.editor.style_warnings.anachronism);
    if det.is_empty() {
        return 0;
    }
    let mut n = 0;
    for book in h.iter().filter(|b| b.kind == NodeKind::Book && b.system_tag.is_none()) {
        for pid in h.collect_subtree(book.id) {
            if h.get(pid).map(|nd| nd.kind) != Some(NodeKind::Paragraph) {
                continue;
            }
            if let Ok(Some(bytes)) = store.get_content(pid) {
                let text = String::from_utf8_lossy(&bytes);
                for line in text.lines() {
                    n += det.detect(line).len();
                }
            }
        }
    }
    n
}

/// `--deep` — refresh the world-layer AI sidecars (facts check, facts scan,
/// drift, continuity), each printing its own progress, so the snapshot reads
/// fresh. A scan that can't run (no provider) is skipped with a note.
fn deep_refresh(project: &Path, provider: Option<&str>) {
    eprintln!("world --deep: refreshing AI sidecars (facts check · facts scan · drift · continuity)…");
    let p = || provider.map(String::from);
    if let Err(e) = super::facts_scan::run(project, super::FactsCommand::Check { provider: p(), json: false }) {
        eprintln!("  facts check skipped: {e}");
    }
    if let Err(e) = super::facts_scan::run(project, super::FactsCommand::Scan { provider: p(), json: false }) {
        eprintln!("  facts scan skipped: {e}");
    }
    if let Err(e) = super::drift::run(project, super::DriftCommand::Scan { provider: p(), json: false }) {
        eprintln!("  drift scan skipped: {e}");
    }
    if let Err(e) = super::continuity::run(project, super::ContinuityCommand::Extract { provider: p() }) {
        eprintln!("  continuity extract skipped: {e}");
    }
    eprintln!();
}

fn render(r: &WorldReport) {
    println!("{}\n", r.summary());

    println!("Facts");
    println!("  established: {}", r.facts_total);
    if r.facts_conflicts.is_empty() {
        println!("  internal conflicts: 0");
    } else {
        println!("  internal conflicts: {}", r.facts_conflicts.len());
        for c in &r.facts_conflicts {
            println!("    ⚠ {}  ⟷  {}  — {}", c.a, c.b, c.detail);
        }
    }
    println!("  prose-vs-fact contradictions: {}", r.facts_prose_findings);

    println!("\nDrift");
    if r.drift_conflicts.is_empty() {
        println!("  0 description contradiction(s)");
    } else {
        println!("  {} description contradiction(s):", r.drift_conflicts.len());
        for c in &r.drift_conflicts {
            println!(
                "    ⚠ {} ({}) — [{}] “{}”  ⟷  [{}] “{}”",
                c.entity, c.kind.label(), c.chapter_a, c.a, c.chapter_b, c.b
            );
        }
    }

    println!("\nContinuity");
    println!("  {} tracked attribute(s)", r.continuity_attributes);

    println!("\nAnachronisms");
    println!("  {} flagged term(s)", r.anachronisms);

    println!("\nCoverage");
    println!(
        "  {} character(s) · {} place(s) · {} artefact(s)",
        r.characters, r.places, r.artefacts
    );

    if r.issue_count() > 0 {
        eprintln!("\n(also walkable in `inkhaven edit` — this is the world-layer snapshot)");
    }
}
