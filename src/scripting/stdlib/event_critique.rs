//! TIMELINE-2-INTEGRATION P4 — the `ink.event.critique.*` Bund stdlib.
//!
//! Read-only access to the two retained, timeline-internal critique checks (orphan
//! + fuzzy-precision overlap) plus the critique config, for project-specific
//! scripting. All `store_read` (the detectors only read the timeline). The
//! `custom` word is a reserved no-op placeholder for the future Bund-programmatic
//! rules the RFC defers — it returns an empty list today.

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_store, push};
use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::timeline::critique::{self, CritSeverity};
use crate::timeline::Calendar;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.event.critique.orphan_check", w_orphan_check),
        ("ink.event.critique.fuzzy_overlap_check", w_fuzzy_overlap_check),
        ("ink.event.critique.run", w_run),
        ("ink.event.critique.config", w_config),
        ("ink.event.critique.custom", w_custom),
    ];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f).map_err(|e| anyhow!("register {name}: {e}"))?;
    }
    Ok(())
}

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(format!("{e}"))
}

/// Load the project's critique events + calendar + config (the data the words
/// operate on). Orphan age is days since last modification (no creation timestamp).
fn critique_data(tag: &str) -> Result<(Vec<critique::CritiqueEvent>, Calendar, Config)> {
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let cfg = Config::load_layered(&ProjectLayout::new(store.project_root()).config_path())
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    let calendar = Calendar::from_config(cfg.timeline.calendar.clone());
    let default_track = cfg.timeline.default_track.clone();
    let now = chrono::Utc::now();
    let events: Vec<critique::CritiqueEvent> = hierarchy
        .flatten()
        .into_iter()
        .filter_map(|(n, _)| n.event.as_ref().map(|e| (n, e)))
        .map(|(n, ev)| {
            let age_days = (now - n.modified_at).num_days().max(0);
            critique::CritiqueEvent {
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
                age_days: Some(age_days),
            }
        })
        .collect();
    Ok((events, calendar, cfg))
}

fn sev_str(s: CritSeverity) -> &'static str {
    match s {
        CritSeverity::Info => "info",
        CritSeverity::Warning => "warning",
        CritSeverity::Contradiction => "contradiction",
    }
}

fn dict(pairs: Vec<(&str, Value)>) -> Value {
    let mut h = std::collections::HashMap::new();
    for (k, v) in pairs {
        h.insert(k.to_string(), v);
    }
    Value::from_dict(h)
}

fn orphan_dict(f: &critique::OrphanFinding) -> Value {
    dict(vec![
        ("event_id", Value::from_string(f.event_id.to_string())),
        ("title", Value::from_string(f.title.clone())),
        ("track", Value::from_string(f.track.clone())),
        ("severity", Value::from_string(sev_str(f.severity).to_string())),
        ("reason", Value::from_string(f.reasons.join(" "))),
        ("age_days", Value::from_int(f.age_days.unwrap_or(-1))),
    ])
}

fn overlap_dict(f: &critique::FuzzyOverlapFinding) -> Value {
    let ids: Vec<Value> =
        f.event_ids.iter().map(|id| Value::from_string(id.to_string())).collect();
    dict(vec![
        ("event_ids", Value::from_list(ids)),
        ("titles", Value::from_string(f.titles.join(" + "))),
        ("track", Value::from_string(f.track.clone())),
        ("severity", Value::from_string(sev_str(f.severity).to_string())),
        ("is_cluster", Value::from_bool(f.is_cluster)),
        ("reason", Value::from_string(f.reasons.join(" "))),
    ])
}

/// Run the report with the project's configured thresholds (honouring the
/// per-category enable switches).
fn run_report(
    events: &[critique::CritiqueEvent],
    calendar: &Calendar,
    cfg: &Config,
) -> critique::CritiqueReport {
    let cc = &cfg.timeline.critique;
    let fuzz = critique::fuzz_windows(calendar);
    let mut report = critique::run(
        events,
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
    report
}

// ( -- list )  orphan findings as a list of dicts.
fn w_orphan_check(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_orphan_check(vm).map_err(to_bund_err)
}
fn do_orphan_check(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.event.critique.orphan_check";
    let (events, calendar, cfg) = critique_data(tag)?;
    let report = run_report(&events, &calendar, &cfg);
    let items: Vec<Value> = report.orphans.iter().map(orphan_dict).collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

// ( -- list )  fuzzy-overlap findings as a list of dicts.
fn w_fuzzy_overlap_check(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_fuzzy_overlap_check(vm).map_err(to_bund_err)
}
fn do_fuzzy_overlap_check(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.event.critique.fuzzy_overlap_check";
    let (events, calendar, cfg) = critique_data(tag)?;
    let report = run_report(&events, &calendar, &cfg);
    let items: Vec<Value> = report.overlaps.iter().map(overlap_dict).collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

// ( -- dict )  both checks: { orphans: list, overlaps: list, total: int }.
fn w_run(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_run(vm).map_err(to_bund_err)
}
fn do_run(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.event.critique.run";
    let (events, calendar, cfg) = critique_data(tag)?;
    let report = run_report(&events, &calendar, &cfg);
    let orphans: Vec<Value> = report.orphans.iter().map(orphan_dict).collect();
    let overlaps: Vec<Value> = report.overlaps.iter().map(overlap_dict).collect();
    let out = dict(vec![
        ("orphans", Value::from_list(orphans)),
        ("overlaps", Value::from_list(overlaps)),
        ("total", Value::from_int(report.total() as i64)),
    ]);
    push(vm, out);
    Ok(vm)
}

// ( -- dict )  the critique config (enabled flags + thresholds).
fn w_config(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_config(vm).map_err(to_bund_err)
}
fn do_config(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.event.critique.config";
    let (_, _, cfg) = critique_data(tag)?;
    let cc = &cfg.timeline.critique;
    let out = dict(vec![
        ("enabled", Value::from_bool(cc.enabled)),
        ("orphan_enabled", Value::from_bool(cc.orphan.enabled)),
        ("orphan_min_significance", Value::from_string(cc.orphan.min_significance.clone())),
        ("orphan_min_age_days", Value::from_int(cc.orphan.min_orphan_age_days)),
        ("overlap_enabled", Value::from_bool(cc.fuzzy_overlap.enabled)),
        ("overlap_min_suspicion", Value::from_string(cc.fuzzy_overlap.min_suspicion.clone())),
        ("overlap_cluster_min_size", Value::from_int(cc.fuzzy_overlap.cluster_min_size as i64)),
        ("elaboration_enabled", Value::from_bool(cc.elaboration.enabled)),
    ]);
    push(vm, out);
    Ok(vm)
}

// ( -- list )  RESERVED for future Bund-programmatic timeline rules. No-op in the
// MVP: returns an empty list so scripts can compose against it now.
fn w_custom(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    push(vm, Value::from_list(Vec::new()));
    Ok(vm)
}
