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

pub fn run(
    project: &Path,
    json: bool,
    only: Option<Vec<String>>,
    book_name: Option<&str>,
) -> Result<()> {
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
    if let Some(cats) = &only {
        raw.retain(|f| cats.iter().any(|c| c.trim().eq_ignore_ascii_case(&f.category)));
    }

    let report = editorial::aggregate(raw);

    if json {
        let out = serde_json::to_string_pretty(&report)
            .map_err(|e| Error::Store(format!("edit: {e}")))?;
        println!("{out}");
    } else {
        render(&report);
    }
    Ok(())
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
    println!("\n  jump to any of these in the cockpit: `Ctrl+V Shift+E` (1.3.6 P1)");
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}
