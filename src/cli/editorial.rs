//! 1.3.6 EDITORIAL-1 P0 — `inkhaven edit`: run the editorial detectors,
//! read the already-computed sidecars, and fold in `plan check`'s
//! structural findings, then hand them to the pure aggregator
//! ([`crate::editorial`]) for one ranked worklist.
//!
//! Deterministic: no live AI here — the AI findings (Facts / tension) are
//! read from their sidecars where present (the `--deep` tier that *runs*
//! them arrives in P3).

use std::path::Path;

use crate::cli::doctor_scan::{self, ScanClass};
use crate::editorial::{self, EditorialFinding};
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;

/// Run every editorial detector, read the sidecars, fold in `plan check`,
/// resolve locations against the hierarchy, and aggregate — the shared
/// entry point for the CLI and the TUI cockpit. `only` filters by category.
pub fn collect(
    project: &Path,
    book_name: Option<&str>,
    only: Option<&[String]>,
    include_deferred: bool,
) -> Result<editorial::EditorialReport> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;

    let mut raw: Vec<EditorialFinding> = Vec::new();

    // 1) doctor's editorial classes — the default scan (deterministic) plus
    //    the opt-in unresolved-tension class (reads the tension ledger).
    let mut scan = doctor_scan::scan_project(project, None)?.findings;
    scan.extend(doctor_scan::scan_project(project, Some(ScanClass::UnresolvedTension))?.findings);
    raw.extend(scan.iter().filter_map(editorial::from_scan_finding));

    // 2) Facts-scan contradictions (sidecar; empty if never run).
    if let Ok(facts) = crate::facts_scan::FactScanReport::load(&layout.root) {
        raw.extend(facts.findings.iter().map(editorial::from_fact_finding));
    }

    // 3) `plan check` structural findings (skipped when there's no plan).
    raw.extend(plan_warnings(project, book_name).into_iter().map(|w| editorial::from_plan_warning(&w)));

    // --only category filter.
    if let Some(cats) = only {
        raw.retain(|f| cats.iter().any(|c| c.trim().eq_ignore_ascii_case(&f.category)));
    }

    // Resolve each finding's location to a paragraph node id where it can,
    // so the cockpit can jump (P1). Best-effort; needs the hierarchy.
    if let Ok(cfg) = crate::config::Config::load_layered(&layout.config_path()) {
        if let Ok(store) = crate::store::Store::open(layout.clone(), &cfg) {
            if let Ok(h) = Hierarchy::load(&store) {
                resolve_locations(&mut raw, &h, &layout);
            }
        }
    }

    // Hide deferred findings (P2) unless explicitly included.
    let before = raw.len();
    if !include_deferred {
        let dismissed = editorial::Dismissed::load(&layout.root);
        if !dismissed.fingerprints.is_empty() {
            raw.retain(|f| !dismissed.fingerprints.contains(&f.fingerprint()));
        }
    }
    let deferred = before - raw.len();

    let mut report = editorial::aggregate(raw);
    report.deferred = deferred;
    Ok(report)
}

/// Fill `location.paragraph` (the jump target) from a `path` (file match) or
/// a `chapter` title (→ chapter node → its first paragraph). Best-effort.
fn resolve_locations(findings: &mut [EditorialFinding], h: &Hierarchy, layout: &ProjectLayout) {
    for f in findings.iter_mut() {
        if f.location.paragraph.is_some() {
            continue;
        }
        // by file path → the owning paragraph
        if let Some(p) = &f.location.path {
            let rel = Path::new(p)
                .strip_prefix(&layout.root)
                .map(|r| r.to_string_lossy().into_owned())
                .unwrap_or_else(|_| p.clone());
            if let Some(node) = h.iter().find(|n| n.file.as_deref() == Some(rel.as_str())) {
                f.location.paragraph = Some(node.id);
                continue;
            }
        }
        // by chapter title/slug → the chapter's first paragraph
        if let Some(ch) = &f.location.chapter {
            if let Some(chapter) = h.iter().find(|n| {
                n.kind == NodeKind::Chapter
                    && (n.title.eq_ignore_ascii_case(ch) || n.slug.eq_ignore_ascii_case(ch))
            }) {
                if let Some(first) = h.collect_subtree(chapter.id).into_iter().find(|id| {
                    h.get(*id).map(|n| n.kind == NodeKind::Paragraph).unwrap_or(false)
                }) {
                    f.location.paragraph = Some(first);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    project: &Path,
    json: bool,
    only: Option<Vec<String>>,
    book_name: Option<&str>,
    show_deferred: bool,
    deep: bool,
    provider: Option<&str>,
) -> Result<()> {
    if deep {
        if json {
            return Err(Error::Store(
                "edit: --deep can't combine with --json (the AI scans print progress) — run the scans separately, then `edit --json`".into(),
            ));
        }
        deep_refresh(project, provider);
    }
    let report = collect(project, book_name, only.as_deref(), show_deferred)?;

    if json {
        let out = serde_json::to_string_pretty(&report)
            .map_err(|e| Error::Store(format!("edit: {e}")))?;
        println!("{out}");
    } else {
        render(&report);
    }
    Ok(())
}

/// `--deep` — run the AI scans that populate the editorial sidecars (Facts
/// contradictions, the tension ledger, the continuity bible), each printing
/// its own progress, so the next `collect` sees the fresh semantic
/// findings. A scan that can't run (no provider) is skipped with a note —
/// the pass degrades to deterministic-only rather than aborting.
fn deep_refresh(project: &Path, provider: Option<&str>) {
    eprintln!("edit --deep: refreshing AI sidecars (facts · tension · continuity)…");
    let p = || provider.map(String::from);
    if let Err(e) = super::facts_scan::run(project, super::FactsCommand::Scan { provider: p(), json: false }) {
        eprintln!("  facts scan skipped: {e}");
    }
    if let Err(e) = super::tension::run(project, super::TensionCommand::Scan { provider: p() }) {
        eprintln!("  tension scan skipped: {e}");
    }
    if let Err(e) = super::continuity::run(project, super::ContinuityCommand::Extract { provider: p() }) {
        eprintln!("  continuity extract skipped: {e}");
    }
    eprintln!();
}

/// The `plan check` warnings for the book, or empty when there's no plan
/// (no beats / no chapters) — the editorial pass never errors on a project
/// that hasn't adopted the Planning Board.
fn plan_warnings(project: &Path, book_name: Option<&str>) -> Vec<String> {
    let layout = ProjectLayout::new(project);
    let Ok(cfg) = crate::config::Config::load_layered(&layout.config_path()) else {
        return Vec::new();
    };
    let Ok(store) = crate::store::Store::open(layout.clone(), &cfg) else {
        return Vec::new();
    };
    let Ok(h) = crate::store::hierarchy::Hierarchy::load(&store) else {
        return Vec::new();
    };
    let Ok(book) = super::resolve_user_book(&h, book_name, "edit") else {
        return Vec::new();
    };
    let book = book.clone();
    match super::plan::build_report(&store, &layout, &h, &book, 0.10) {
        Ok((report, _, _)) => report.warnings,
        Err(_) => Vec::new(),
    }
}

fn render(report: &editorial::EditorialReport) {
    if report.findings.is_empty() {
        println!("editorial pass: ✓ no findings — the manuscript reads clean");
        return;
    }
    println!(
        "EDITORIAL PASS · {} finding(s)  ({} error · {} warn · {} info)\n",
        report.findings.len(),
        report.errors,
        report.warnings,
        report.infos,
    );
    for f in &report.findings {
        println!(
            "  {} {:<10} {:<14} {}",
            f.severity.icon(),
            f.category,
            truncate(&f.location.label(), 14),
            f.message,
        );
        if let Some(hint) = &f.hint {
            println!("                                    ↳ {hint}");
        }
    }
    if report.deferred > 0 {
        println!(
            "\n  ({} deferred, hidden — `--show-deferred` to include, or clear in the cockpit)",
            report.deferred
        );
    }
    println!("\n  walk + jump + defer in the cockpit: `Ctrl+V Shift+R` (1.3.6)");
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}
