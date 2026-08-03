//! `inkhaven check` — the unified review pass (road to 1.4.0).
//!
//! Runs every applicable **fast / deterministic** checker over a scope in one pass
//! and prints a consolidated summary: the world fact-checker, the Inner Socrates
//! fast track, and the timeline critique. Each native finding also emits to the
//! Output store (a no-op headless, so the TUI path in P1 is free); the CLI prints
//! the findings + per-checker counts to stdout. The fast tracks are LLM-free, so a
//! default `check` is instant and costs nothing.

use std::path::Path;

use anyhow::{anyhow, Result};
use uuid::Uuid;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::Store;

/// A normalized finding for the summary + stdout display (the native finding still
/// carries full per-kind metadata into the Output pane).
struct CheckFinding {
    checker: &'static str,
    severity: String,
    label: String,
    body: String,
    paragraph: Option<Uuid>,
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    project: &Path,
    paragraph: Option<&str>,
    book_name: Option<&str>,
    no_fact: bool,
    no_socrates: bool,
    no_timeline: bool,
    no_continuity: bool,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let book_filter_id = match book_name {
        Some(name) => Some(
            crate::cli::resolve_user_book(&hierarchy, Some(name), "check")
                .map_err(|m| anyhow!(m))?
                .id,
        ),
        None => None,
    };

    let scope = scope_paragraphs(&store, &hierarchy, paragraph, book_filter_id)?;
    if scope.is_empty() {
        println!("(no prose paragraphs in scope — nothing to check)");
        return Ok(());
    }

    let mut findings: Vec<CheckFinding> = Vec::new();

    // ── per-paragraph checkers: fact-check + socrates ────────────────────────
    let world = (!no_fact).then(|| crate::cli::realworld::build_world_context(project));
    let socratic = (!no_socrates).then(|| {
        let ledger = crate::inner_socrates::storage::InnerSocratesStore::open_for_project(project)
            .ok()
            .and_then(|s| s.load_ledger().ok())
            .unwrap_or_default();
        let persona = crate::inner_socrates::personas::active(project);
        (persona, ledger)
    });

    for (id, prose) in &scope {
        if let Some((ledger, ctx)) = &world {
            for f in crate::world::fact_check::check_paragraph(prose, ledger, &[], ctx.as_ref()) {
                crate::world::fact_check::emit_finding(&f, Some(*id));
                findings.push(CheckFinding {
                    checker: "fact-check",
                    severity: f.severity.clone(),
                    label: f.category.clone(),
                    body: f.body.clone(),
                    paragraph: Some(*id),
                });
            }
        }
        if let Some((persona, ledger)) = &socratic {
            let fctx = crate::inner_socrates::intent::FindingContext {
                paragraph_id: Some(id.to_string()),
                ..Default::default()
            };
            for f in crate::inner_socrates::fast::check_paragraph(prose, persona, ledger, &fctx) {
                crate::inner_socrates::output::emit_finding(&f, Some(*id));
                findings.push(CheckFinding {
                    checker: "socrates",
                    severity: f.severity.label().to_string(),
                    label: f.category.id().to_string(),
                    body: f.question.clone(),
                    paragraph: Some(*id),
                });
            }
        }
    }

    // ── project-wide checker: timeline critique ──────────────────────────────
    let mut timeline_ran = false;
    if !no_timeline && cfg.timeline.enabled && cfg.timeline.critique.enabled {
        timeline_ran = true;
        run_timeline_critique(&hierarchy, &cfg, &mut findings);
    }

    // ── project-wide checker: SENTINEL continuity ledger ─────────────────────
    // `timeline` is skipped — the critique above already covers orphan / overlap.
    let mut continuity_ran = false;
    if !no_continuity {
        continuity_ran = true;
        run_continuity(&layout, &cfg, &store, &hierarchy, &mut findings);
    }

    print_report(
        &findings,
        scope.len(),
        &cfg,
        world.is_some(),
        socratic.is_some(),
        timeline_ran,
        continuity_ran,
        &hierarchy,
    );
    Ok(())
}

/// SENTINEL (CT-P4) — run the unified continuity engine (minus `timeline`, which
/// has its own section) and fold its findings into the `check` summary. Each
/// native finding also lands in the Output store via the engine's own emit path
/// on the TUI; here we collect the summary rows.
fn run_continuity(
    layout: &ProjectLayout,
    cfg: &Config,
    store: &Store,
    hierarchy: &Hierarchy,
    findings: &mut Vec<CheckFinding>,
) {
    use crate::continuity_intel::{engine, Severity};
    let sel = engine::selector(&[], &["timeline".to_string()]);
    for f in engine::run(store, cfg, layout, hierarchy, &sel) {
        let severity = match f.severity {
            Severity::Contradiction => "contradiction",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        findings.push(CheckFinding {
            checker: "continuity",
            severity: severity.to_string(),
            label: f.source.to_string(),
            body: f.message.clone(),
            paragraph: f.anchor,
        });
    }
}

/// Resolve the scope to `(paragraph_id, prose)` pairs: a single `--paragraph`, or
/// every prose paragraph (non-event) under the `--book` (or every user book).
fn scope_paragraphs(
    store: &Store,
    hierarchy: &Hierarchy,
    paragraph: Option<&str>,
    book_filter_id: Option<Uuid>,
) -> Result<Vec<(Uuid, String)>> {
    let ids: Vec<Uuid> = match paragraph {
        Some(pid) => {
            let id = Uuid::parse_str(pid.trim())
                .map_err(|e| anyhow!("bad paragraph id `{pid}`: {e}"))?;
            vec![id]
        }
        None => hierarchy
            .flatten()
            .into_iter()
            .filter(|(n, _)| n.kind == NodeKind::Paragraph && n.event.is_none())
            .filter(|(n, _)| match book_filter_id {
                Some(bid) => node_in_book(hierarchy, n, bid),
                None => node_under_user_book(hierarchy, n),
            })
            .map(|(n, _)| n.id)
            .collect(),
    };
    let mut out = Vec::new();
    for id in ids {
        let prose = store
            .get_content(id)
            .ok()
            .flatten()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        if !prose.trim().is_empty() {
            out.push((id, prose));
        }
    }
    Ok(out)
}

/// The Book ancestor of a node, if any.
fn book_of<'a>(hierarchy: &'a Hierarchy, node: &'a Node) -> Option<&'a Node> {
    let mut cur = node;
    loop {
        if cur.kind == NodeKind::Book {
            return Some(cur);
        }
        cur = hierarchy.get(cur.parent_id?)?;
    }
}

fn node_in_book(hierarchy: &Hierarchy, node: &Node, book_id: Uuid) -> bool {
    book_of(hierarchy, node).map(|b| b.id == book_id).unwrap_or(false)
}

/// True when the node's root book is a user book (not a system book like Notes /
/// Help / Prompts), so `check` skips system-book scratch.
fn node_under_user_book(hierarchy: &Hierarchy, node: &Node) -> bool {
    book_of(hierarchy, node).map(|b| b.system_tag.is_none()).unwrap_or(false)
}

/// Run the orphan + fuzzy-overlap critique over the whole project's events and emit
/// + collect the findings.
fn run_timeline_critique(hierarchy: &Hierarchy, cfg: &Config, out: &mut Vec<CheckFinding>) {
    use crate::timeline::critique;
    use crate::timeline::{Calendar, Precision, TimelinePoint};

    let calendar = Calendar::from_config(cfg.timeline.calendar.clone());
    let default_track = cfg.timeline.default_track.clone();
    let now = chrono::Utc::now();
    let events: Vec<critique::CritiqueEvent> = hierarchy
        .flatten()
        .into_iter()
        .filter_map(|(n, _)| n.event.as_ref().map(|e| (n, e)))
        .map(|(n, ev)| critique::CritiqueEvent {
            id: n.id,
            title: n.title.clone(),
            start_ticks: ev.start_ticks,
            end_ticks: ev.end_ticks,
            precision: ev.precision,
            track: ev.track.clone().unwrap_or_else(|| default_track.clone()),
            is_orphan: ev.is_orphan(&n.linked_paragraphs),
            linked_paragraph_count: n.linked_paragraphs.len(),
            characters: ev.characters.clone(),
            places: ev.places.clone(),
            age_days: Some((now - n.modified_at).num_days().max(0)),
        })
        .collect();
    if events.is_empty() {
        return;
    }
    let cc = &cfg.timeline.critique;
    let fuzz = critique::fuzz_windows(&calendar);
    let mut report = critique::run(
        &events,
        &fuzz,
        cc.min_significance(),
        cc.min_suspicion(),
        cc.fuzzy_overlap.cluster_min_size.max(2),
        critique::DEFAULT_STALENESS_DAYS,
    );
    if !cc.orphan.enabled {
        report.orphans.clear();
    }
    if !cc.fuzzy_overlap.enabled {
        report.overlaps.clear();
    }
    let lang = critique::lang::lang_from_name(&cfg.language);
    for f in &report.orphans {
        let date = calendar.format(TimelinePoint::from_ticks(f.start_ticks), f.precision);
        critique::pane::emit_orphan(f, &date, None, lang);
        out.push(CheckFinding {
            checker: "timeline",
            severity: f.severity.label().to_string(),
            label: "orphan".into(),
            body: format!("\"{}\" — {}", f.title, f.reasons.join(" ")),
            paragraph: None,
        });
    }
    for f in &report.overlaps {
        let window = calendar.format(TimelinePoint::from_ticks(f.overlap_window.0), Precision::Season);
        critique::pane::emit_overlap(f, &window, &[], &[], None, lang);
        out.push(CheckFinding {
            checker: "timeline",
            severity: f.severity.label().to_string(),
            label: "fuzzy_overlap".into(),
            body: format!("{} — {}", f.titles.join(" + "), f.reasons.join(" ")),
            paragraph: None,
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn print_report(
    findings: &[CheckFinding],
    paragraphs: usize,
    cfg: &Config,
    fact_ran: bool,
    socratic_ran: bool,
    timeline_ran: bool,
    continuity_ran: bool,
    hierarchy: &Hierarchy,
) {
    if findings.is_empty() {
        println!("✓ no issues across {paragraphs} paragraph(s)");
    } else {
        for f in findings {
            let icon = match f.severity.as_str() {
                "contradiction" | "Probe" | "Contradiction" => "⊗",
                "warning" | "Inquiry" | "Warning" => "⚠",
                _ => "●",
            };
            let para = f
                .paragraph
                .and_then(|id| hierarchy.get(id))
                .map(|n| format!("  ({})", n.title))
                .unwrap_or_default();
            println!("{icon} [{}/{}] {}{para}", f.checker, f.label, f.body);
        }
        println!();
    }

    // Per-checker summary.
    let count = |c: &str| findings.iter().filter(|f| f.checker == c).count();
    println!("review pass · {paragraphs} paragraph(s)");
    if fact_ran {
        println!("  fact-check : {}", count("fact-check"));
    } else {
        println!("  fact-check : skipped");
    }
    if socratic_ran {
        println!("  socrates   : {}", count("socrates"));
    } else {
        println!("  socrates   : skipped");
    }
    if timeline_ran {
        println!("  timeline   : {}", count("timeline"));
    } else {
        let why = if !cfg.timeline.enabled { " (timeline off)" } else { "" };
        println!("  timeline   : skipped{why}");
    }
    if continuity_ran {
        println!("  continuity : {}", count("continuity"));
    } else {
        println!("  continuity : skipped");
    }
    println!("  ─────────────");
    println!("  TOTAL      : {}", findings.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(v: serde_json::Value) -> Node {
        serde_json::from_value(v).expect("test node")
    }

    fn base(id: uuid::Uuid, kind: &str, slug: &str, parent: Option<uuid::Uuid>) -> serde_json::Value {
        serde_json::json!({
            "id": id, "kind": kind, "title": slug, "slug": slug,
            "path": [], "parent_id": parent, "order": 1, "file": null,
            "modified_at": "2026-01-01T00:00:00Z",
        })
    }

    #[test]
    fn scope_includes_user_book_paragraphs_excludes_system_books() {
        let user_book = uuid::Uuid::now_v7();
        let sys_book = uuid::Uuid::now_v7();
        let user_para = uuid::Uuid::now_v7();
        let sys_para = uuid::Uuid::now_v7();
        let mut sysb = base(sys_book, "book", "notes", None);
        sysb["system_tag"] = serde_json::json!("notes");
        let h = Hierarchy::from_nodes_for_test(vec![
            node(base(user_book, "book", "velmaron", None)),
            node(sysb),
            node(base(user_para, "paragraph", "scene", Some(user_book))),
            node(base(sys_para, "paragraph", "jotting", Some(sys_book))),
        ]);
        let up = h.get(user_para).unwrap();
        let sp = h.get(sys_para).unwrap();
        assert!(node_under_user_book(&h, up), "user-book paragraph is in scope");
        assert!(!node_under_user_book(&h, sp), "system-book paragraph is skipped");
        assert!(node_in_book(&h, up, user_book));
        assert!(!node_in_book(&h, up, sys_book));
    }
}
