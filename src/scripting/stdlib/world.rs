//! WORLD-REPORT (3.3.0 W1) — `ink.world.*` Bund stdlib: the world-report reads
//! (`inkhaven world`, from a script), read-only. `report` is the headline world
//! state (entity inventory + issue counts), `undescribed` the defined-but-never-
//! named entities (a coverage gap), `check` a pass/fail gate. The already-wrapped
//! `ink.world.fact_check.timeline.*`, `ink.continuity.*`, `ink.myth.*`, and
//! `ink.utopia.*` cover the other world axes — this fills the core-report gap.
//!
//! - `ink.world.report`      ( -- dict )  {facts_total, characters, places, artefacts, continuity_attributes, entity_total, issue_count, undescribed, stale, summary}
//! - `ink.world.undescribed` ( -- list )  {name, kind}
//! - `ink.world.findings`    ( -- list )  the discrete conflicts + anachronisms ({kind, …})
//! - `ink.world.check`       ( -- dict )  {issues, undescribed, clean}

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, push};
use crate::store::hierarchy::Hierarchy;
use crate::world_report::WorldReport;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.world.report", w_report),
        ("ink.world.undescribed", w_undescribed),
        ("ink.world.findings", w_findings),
        ("ink.world.check", w_check),
    ];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f).map_err(|e| anyhow!("register {name}: {e}"))?;
    }
    for (name, _) in words {
        if let Some(short) = name.strip_prefix("ink.") {
            let _ = vm.register_alias(short.to_string(), name.to_string());
        }
    }
    Ok(())
}

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(e.to_string())
}

macro_rules! word {
    ($w:ident, $do:ident) => {
        fn $w(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
            $do(vm).map_err(to_bund_err)
        }
    };
}

/// Build the active project's world report (counts + issues) plus its undescribed
/// entities (the prose-walk coverage pass that `report_from` leaves to the
/// caller, so its `undescribed` field is populated here).
fn active_report(tag: &str) -> Result<(WorldReport, Vec<(String, String)>)> {
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let root = store.project_root();
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let mut report = crate::cli::world::report_from(store, &h, cfg, root);
    let undescribed: Vec<(String, String)> = crate::cli::world::undescribed_entities(store, &h)
        .into_iter()
        .map(|(name, kind, _id)| (name, kind.label().to_string()))
        .collect();
    report.undescribed = undescribed.iter().map(|(n, _)| n.clone()).collect();
    Ok((report, undescribed))
}

fn report_dict(r: &WorldReport) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("facts_total".into(), Value::from_int(r.facts_total as i64));
    m.insert("characters".into(), Value::from_int(r.characters as i64));
    m.insert("places".into(), Value::from_int(r.places as i64));
    m.insert("artefacts".into(), Value::from_int(r.artefacts as i64));
    m.insert("continuity_attributes".into(), Value::from_int(r.continuity_attributes as i64));
    m.insert("entity_total".into(), Value::from_int(r.entity_total() as i64));
    m.insert("issue_count".into(), Value::from_int(r.issue_count() as i64));
    m.insert("undescribed".into(), Value::from_int(r.undescribed.len() as i64));
    m.insert("stale".into(), Value::from_bool(r.stale));
    m.insert("summary".into(), Value::from_string(&r.summary()));
    Value::from_dict(m)
}

word!(w_report, do_report);
fn do_report(vm: &mut VM) -> Result<&mut VM> {
    let (report, _) = active_report("ink.world.report")?;
    push(vm, report_dict(&report));
    Ok(vm)
}

word!(w_undescribed, do_undescribed);
fn do_undescribed(vm: &mut VM) -> Result<&mut VM> {
    let (_, undescribed) = active_report("ink.world.undescribed")?;
    let rows: Vec<Value> = undescribed
        .iter()
        .map(|(name, kind)| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("name".into(), Value::from_string(name));
            m.insert("kind".into(), Value::from_string(kind));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(rows));
    Ok(vm)
}

/// E2 — flatten the world report's discrete conflicts + anachronisms into one
/// finding list, the `ink.world.*` analog of `ink.knowledge.findings` /
/// `ink.bonds.findings`. Prose-vs-fact contradictions are a count only in the
/// report (their items live in the facts-scan sidecar), so they aren't enumerated
/// here. Pure.
fn findings_list(r: &WorldReport) -> Vec<Value> {
    let mut rows: Vec<Value> = Vec::new();
    for c in &r.facts_conflicts {
        let mut m: HashMap<String, Value> = HashMap::new();
        m.insert("kind".into(), Value::from_string("fact_conflict"));
        m.insert("a".into(), Value::from_string(&c.a));
        m.insert("b".into(), Value::from_string(&c.b));
        m.insert("detail".into(), Value::from_string(&c.detail));
        rows.push(Value::from_dict(m));
    }
    for c in &r.drift_conflicts {
        let mut m: HashMap<String, Value> = HashMap::new();
        m.insert("kind".into(), Value::from_string("drift"));
        m.insert("entity".into(), Value::from_string(&c.entity));
        m.insert("a".into(), Value::from_string(&c.a));
        m.insert("b".into(), Value::from_string(&c.b));
        m.insert("chapter_a".into(), Value::from_string(&c.chapter_a));
        m.insert("chapter_b".into(), Value::from_string(&c.chapter_b));
        m.insert("detail".into(), Value::from_string(&c.detail));
        rows.push(Value::from_dict(m));
    }
    for a in &r.anachronism_flags {
        let mut m: HashMap<String, Value> = HashMap::new();
        m.insert("kind".into(), Value::from_string("anachronism"));
        m.insert("term".into(), Value::from_string(&a.term));
        m.insert("chapter".into(), Value::from_string(&a.chapter));
        rows.push(Value::from_dict(m));
    }
    rows
}

word!(w_findings, do_findings);
fn do_findings(vm: &mut VM) -> Result<&mut VM> {
    let (report, _) = active_report("ink.world.findings")?;
    push(vm, Value::from_list(findings_list(&report)));
    Ok(vm)
}

word!(w_check, do_check);
fn do_check(vm: &mut VM) -> Result<&mut VM> {
    let (report, _) = active_report("ink.world.check")?;
    let issues = report.issue_count() as i64;
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("issues".into(), Value::from_int(issues));
    m.insert("undescribed".into(), Value::from_int(report.undescribed.len() as i64));
    // `clean` = no world *issue* (a contradiction / anachronism). Undescribed
    // entities are coverage, not an issue, so they don't fail the gate.
    m.insert("clean".into(), Value::from_bool(issues == 0));
    push(vm, Value::from_dict(m));
    Ok(vm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_dict_carries_counts_and_a_clean_summary() {
        let r = WorldReport {
            facts_total: 12,
            characters: 3,
            places: 2,
            artefacts: 1,
            ..WorldReport::default()
        };
        let d = report_dict(&r).cast_dict().expect("a dict");
        assert_eq!(d.get("facts_total").and_then(|v| v.clone().cast_int().ok()), Some(12));
        assert_eq!(d.get("entity_total").and_then(|v| v.clone().cast_int().ok()), Some(6));
        // No sidecars → no issues.
        assert_eq!(d.get("issue_count").and_then(|v| v.clone().cast_int().ok()), Some(0));
        assert_eq!(d.get("undescribed").and_then(|v| v.clone().cast_int().ok()), Some(0));
        // The one-line summary reflects a clean world.
        let summary = d.get("summary").and_then(|v| v.cast_string().ok()).unwrap_or_default();
        assert!(summary.contains("consistent"), "summary: {summary}");
    }

    #[test]
    fn findings_list_flattens_each_conflict_kind() {
        // E2 — one of each discrete conflict type → three tagged rows.
        let r = WorldReport {
            facts_conflicts: vec![crate::facts_scan::FactConflict {
                a: "born 1820".into(),
                b: "born 1830".into(),
                detail: "two birth years".into(),
            }],
            anachronism_flags: vec![crate::world_report::AnachronismFlag {
                term: "wristwatch".into(),
                chapter: "ch. 2".into(),
            }],
            ..WorldReport::default()
        };
        let rows = findings_list(&r);
        assert_eq!(rows.len(), 2, "one fact conflict + one anachronism");
        let kinds: Vec<String> = rows
            .iter()
            .filter_map(|v| v.clone().cast_dict().ok())
            .filter_map(|d| d.get("kind").and_then(|k| k.clone().cast_string().ok()))
            .collect();
        assert!(kinds.contains(&"fact_conflict".to_string()), "kinds: {kinds:?}");
        assert!(kinds.contains(&"anachronism".to_string()), "kinds: {kinds:?}");
        // A clean report yields no findings.
        assert!(findings_list(&WorldReport::default()).is_empty());
    }
}
