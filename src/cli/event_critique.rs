//! `inkhaven event critique …` (TIMELINE-2-INTEGRATION P3).
//!
//! The CLI surface for the refactored timeline critique. Runs the two retained,
//! timeline-internal checks (orphan + fuzzy-precision overlap) over a scope and
//! prints structured findings, with optional cost-capped LLM elaboration. The
//! `--legacy` flag preserves the original five-item AI audit (deprecated);
//! `--migration-check` / `--diff` show where the removed categories now live.

use anyhow::{anyhow, Result};

use crate::config::Config;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{EventData, Node, NodeKind};
use crate::store::Store;
use crate::timeline::critique::{self, elaboration};
use crate::timeline::{Calendar, Precision, TimelinePoint};
use crate::tui::timeline_state::TimelineEvent;

/// Where each removed legacy-critique category now lives (for migration / diff).
const MIGRATION_MAP: &[(&str, &str, &str)] = &[
    ("Travel-time conflicts", "travel_time", "inkhaven realworld fact-check --timeline-aware"),
    ("Co-location conflicts", "co_location", "inkhaven realworld co-location"),
    ("Paragraph-date mismatches", "date_coherence", "inkhaven realworld fact-check --timeline-aware"),
    ("Pacing analysis", "temporal_density", "inkhaven inner-socrates check"),
];

#[allow(clippy::too_many_arguments)]
pub fn run(
    cfg: &Config,
    store: &Store,
    calendar: &Calendar,
    track: Option<&str>,
    book_name: Option<&str>,
    legacy: bool,
    migration_check: bool,
    diff: bool,
    no_elaborate: bool,
    force: bool,
) -> Result<()> {
    let hierarchy = Hierarchy::load(store)?;
    let book_filter_id = match book_name {
        Some(name) => Some(
            crate::cli::resolve_user_book(&hierarchy, Some(name), "event critique")
                .map_err(|m| anyhow!(m))?
                .id,
        ),
        None => None,
    };

    if diff {
        print_diff();
        return Ok(());
    }

    if legacy {
        return run_legacy(cfg, &hierarchy, calendar, book_filter_id, track);
    }

    if !cfg.timeline.critique.enabled {
        return Err(anyhow!(
            "timeline critique is disabled (set timeline.critique.enabled: true)"
        ));
    }

    let events = gather(store, &hierarchy, calendar, cfg, book_filter_id, track);
    if events.is_empty() {
        println!("(no events in scope — nothing to critique)");
        return Ok(());
    }

    let crit_cfg = &cfg.timeline.critique;
    let fuzz = critique::fuzz_windows(calendar);
    let mut report = critique::run(
        &events,
        &fuzz,
        crit_cfg.min_significance(),
        crit_cfg.min_suspicion(),
        crit_cfg.fuzzy_overlap.cluster_min_size.max(2),
        critique::DEFAULT_STALENESS_DAYS,
    );
    if !crit_cfg.orphan.enabled {
        report.orphans.clear();
    }
    if !crit_cfg.fuzzy_overlap.enabled {
        report.overlaps.clear();
    }

    if migration_check {
        print_migration_check(events.len(), &report);
        return Ok(());
    }

    if report.is_empty() {
        println!("✓ no orphan or fuzzy-overlap issues in {} event(s)", events.len());
        return Ok(());
    }

    // Optional LLM elaboration, gated by the per-run budget. The provider is
    // resolved lazily; absence simply means pattern-only.
    let mut budget = if no_elaborate {
        elaboration::ElaborationBudget::off()
    } else {
        let have_llm = crate::ai::AiClient::from_config(&cfg.llm).is_ok();
        elaboration::ElaborationBudget::from_config(&crit_cfg.elaboration, have_llm)
    };
    if budget.needs_confirmation(report.total()) && !force {
        eprintln!(
            "elaboration: {} findings exceeds the confirm cap ({}); elaborating up to {} \
             (re-run with --force to raise) — others stay pattern-only",
            report.total(),
            budget.confirm_above,
            budget.remaining()
        );
    }

    // Localize finding text to the project's working language.
    let lang = critique::lang::lang_from_name(&cfg.language);
    for f in &report.orphans {
        let date = calendar.format(TimelinePoint::from_ticks(f.start_ticks), f.precision);
        let reasons = critique::lang::localize_orphan(f, lang);
        let prompt = elaboration::orphan_prompt(&f.title, &date, &f.track, &reasons);
        let extra = elaborate_one(cfg, &mut budget, &prompt);
        let icon = sev_icon(f.severity);
        println!("{icon} ⊘ [orphan] \"{}\" ({date}, {} track)", f.title, f.track);
        println!("    {}", reasons.join(" "));
        if let Some(text) = extra {
            println!("    ↳ {text}");
        }
    }
    for f in &report.overlaps {
        let window = calendar.format(TimelinePoint::from_ticks(f.overlap_window.0), Precision::Season);
        let reasons = critique::lang::overlap_reasons(f, lang);
        let prompt =
            elaboration::overlap_prompt(&f.titles, &window, f.is_cluster, &reasons);
        let extra = elaborate_one(cfg, &mut budget, &prompt);
        let icon = sev_icon(f.severity);
        let tag = if f.is_cluster { "cluster" } else { "overlap" };
        println!("{icon} ⧉ [fuzzy_{tag}] {}", f.titles.join(" + "));
        println!("    {} · window ~{window}", reasons.join(" "));
        if let Some(text) = extra {
            println!("    ↳ {text}");
        }
    }
    println!(
        "\n{} finding(s): {} orphan, {} overlap.",
        report.total(),
        report.orphans.len(),
        report.overlaps.len()
    );
    Ok(())
}

fn sev_icon(s: critique::CritSeverity) -> &'static str {
    match s {
        critique::CritSeverity::Contradiction => "⊗",
        critique::CritSeverity::Warning => "⚠",
        critique::CritSeverity::Info => "●",
    }
}

/// One elaboration call via the blocking provider. Returns `None` (pattern-only) on
/// any failure, exhausted budget, or missing provider.
fn elaborate_one(
    cfg: &Config,
    budget: &mut elaboration::ElaborationBudget,
    prompt: &str,
) -> Option<String> {
    if !budget.is_enabled() {
        return None;
    }
    let ai = crate::ai::AiClient::from_config(&cfg.llm).ok()?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, None).ok()?;
    let model = model.to_string();
    elaboration::elaborate(prompt, budget, |p| {
        crate::ai::stream::collect_blocking(
            ai.client.clone(),
            model.clone(),
            Some(elaboration::ELABORATION_SYSTEM.to_string()),
            p.to_string(),
        )
        .ok()
    })
}

/// Project in-scope events into the detectors' input. Orphan age is days since last
/// modification (the model carries no creation timestamp) — an untouched orphan
/// reads as stale.
fn gather(
    store: &Store,
    hierarchy: &Hierarchy,
    _calendar: &Calendar,
    cfg: &Config,
    book_filter_id: Option<uuid::Uuid>,
    track: Option<&str>,
) -> Vec<critique::CritiqueEvent> {
    let default_track = cfg.timeline.default_track.clone();
    let now = chrono::Utc::now();
    // Fuzzy-overlap is advisory, so it may union in scene-derived participants.
    let derived = crate::world::timeline_context::derived_participants(store, hierarchy);
    scope_nodes(hierarchy, book_filter_id, track, &default_track)
        .into_iter()
        .map(|(n, ev)| {
            let resolved = ev.track.clone().unwrap_or_else(|| default_track.clone());
            let age_days = (now - n.modified_at).num_days();
            let (dc, dp) = derived.get(&n.id).cloned().unwrap_or_default();
            let mut characters = ev.characters.clone();
            for c in dc {
                if !characters.contains(&c) {
                    characters.push(c);
                }
            }
            let mut places = ev.places.clone();
            for p in dp {
                if !places.contains(&p) {
                    places.push(p);
                }
            }
            critique::CritiqueEvent {
                id: n.id,
                title: n.title.clone(),
                start_ticks: ev.start_ticks,
                end_ticks: ev.end_ticks,
                precision: ev.precision,
                track: resolved,
                is_orphan: ev.is_orphan(&n.linked_paragraphs),
                linked_paragraph_count: n.linked_paragraphs.len(),
                characters,
                places,
                age_days: Some(age_days.max(0)),
            }
        })
        .collect()
}

/// In-scope (node, event) pairs, applying the book + track filters.
fn scope_nodes<'a>(
    hierarchy: &'a Hierarchy,
    book_filter_id: Option<uuid::Uuid>,
    track: Option<&str>,
    default_track: &str,
) -> Vec<(&'a Node, &'a EventData)> {
    hierarchy
        .flatten()
        .into_iter()
        .filter_map(|(n, _)| n.event.as_ref().map(|e| (n, e)))
        .filter(|(n, _)| match book_filter_id {
            Some(id) => node_in_book(hierarchy, n, id),
            None => true,
        })
        .filter(|(_, ev)| match track {
            Some(t) => ev.track.as_deref().unwrap_or(default_track).eq_ignore_ascii_case(t),
            None => true,
        })
        .collect()
}

fn node_in_book(hierarchy: &Hierarchy, node: &Node, book_id: uuid::Uuid) -> bool {
    let mut cur = node;
    loop {
        if cur.kind == NodeKind::Book {
            return cur.id == book_id;
        }
        let Some(parent_id) = cur.parent_id else {
            return false;
        };
        match hierarchy.get(parent_id) {
            Some(p) => cur = p,
            None => return false,
        }
    }
}

/// The deprecated original critique: build the legacy five-item AI payload and
/// stream a synchronous completion to stdout.
fn run_legacy(
    cfg: &Config,
    hierarchy: &Hierarchy,
    calendar: &Calendar,
    book_filter_id: Option<uuid::Uuid>,
    track: Option<&str>,
) -> Result<()> {
    if cfg.timeline.critique.legacy_flag_deprecation.warn_on_use {
        eprintln!(
            "warning: `event critique --legacy` is deprecated — the travel-time, date, and \
             pacing items now live in `realworld fact-check` and `inner-socrates check`. This \
             flag will be removed in a later release. Run `--migration-check` for the mapping."
        );
    }
    let default_track = cfg.timeline.default_track.clone();
    let events: Vec<TimelineEvent> =
        scope_nodes(hierarchy, book_filter_id, track, &default_track)
            .into_iter()
            .map(|(n, ev)| TimelineEvent {
                id: n.id,
                title: n.title.clone(),
                start_ticks: ev.start_ticks,
                end_ticks: ev.end_ticks,
                precision: ev.precision,
                track: ev.track.clone(),
                is_orphan: ev.is_orphan(&n.linked_paragraphs),
                linked_paragraphs: n.linked_paragraphs.clone(),
                book_prefix: String::new(),
                characters: ev.characters.clone(),
                places: ev.places.clone(),
            })
            .collect();
    if events.is_empty() {
        println!("(no events in scope — nothing to critique)");
        return Ok(());
    }
    let crumb = match book_filter_id.and_then(|id| hierarchy.get(id)) {
        Some(n) => n.title.clone(),
        None => "(project)".to_string(),
    };
    let payload = critique::build_health_payload(
        &events,
        calendar,
        hierarchy,
        &crumb,
        track,
        &default_track,
    );
    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| anyhow!("--legacy needs an LLM provider: {e}"))?;
    let (model, _env) =
        ai.resolve_provider(&cfg.llm, None).map_err(|e| anyhow!("resolving provider: {e}"))?;
    eprintln!("legacy timeline critique · model: {model} · {} events…", events.len());
    let raw = crate::ai::stream::collect_blocking(
        ai.client.clone(),
        model.to_string(),
        None,
        payload,
    )
    .map_err(|e| anyhow!("LLM error: {e}"))?;
    println!("{}", raw.trim());
    Ok(())
}

fn print_migration_check(event_count: usize, report: &critique::CritiqueReport) {
    println!("Inkhaven Timeline Critique Migration Check");
    println!("==========================================\n");
    println!("Timeline: {event_count} event(s) in scope\n");
    println!("Legacy categories now covered elsewhere:");
    for (label, category, command) in MIGRATION_MAP {
        println!("  → {label:<26} {category:<18} {command}");
    }
    println!("\nTimeline-internal categories (retained):");
    println!("  ✓ Orphan events              timeline_orphan_warning ({} finding(s))", report.orphans.len());
    println!(
        "  ✓ Fuzzy-precision overlaps   timeline_fuzzy_overlap_warning ({} finding(s))",
        report.overlaps.len()
    );
    println!(
        "\nCoverage: {} retained finding(s) via the new infrastructure.",
        report.total()
    );
    println!(
        "For the migrated categories, run the commands above (their counts depend on a\n\
         world definition / prose pass the timeline critique no longer performs)."
    );
}

fn print_diff() {
    println!("Legacy critique categories and where they moved:\n");
    for (label, category, command) in MIGRATION_MAP {
        println!("  • {label}");
        println!("      now: {category}  —  {command}");
    }
    println!("\nRetained, and stronger than the legacy critique:");
    println!("  • Orphan events — significance × staleness severity (legacy was uniform).");
    println!("  • Fuzzy overlaps — multi-event clusters (legacy only flagged pairs).");
    println!(
        "\nThe legacy `--legacy` audit still runs all five items via one LLM call; the\n\
         refactored critique is structured, pattern-based, and emits to the Output pane."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_map_covers_the_four_removed_categories() {
        assert_eq!(MIGRATION_MAP.len(), 4);
        let cats: Vec<&str> = MIGRATION_MAP.iter().map(|(_, c, _)| *c).collect();
        assert!(cats.contains(&"travel_time"));
        assert!(cats.contains(&"co_location"));
        assert!(cats.contains(&"date_coherence"));
        assert!(cats.contains(&"temporal_density"));
    }

    #[test]
    fn severity_icons_are_distinct() {
        let icons = [
            sev_icon(critique::CritSeverity::Info),
            sev_icon(critique::CritSeverity::Warning),
            sev_icon(critique::CritSeverity::Contradiction),
        ];
        assert_eq!(icons.iter().collect::<std::collections::HashSet<_>>().len(), 3);
    }
}
