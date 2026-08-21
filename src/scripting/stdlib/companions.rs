//! 3.0.4 — `ink.companions.*` Bund stdlib: the examined-authorship cockpit,
//! read-only. Bund reads the same trio `inkhaven companions` shows — Inner
//! Socrates, Inner Editor, and the World story-bible health. All deterministic
//! reads: the inner findings come from the per-project sidecar DBs
//! (`inner_socrates.db` / `inner_editor.db`, opened fresh — they don't contend
//! with the live manuscript store), and the World health is derived from the
//! ALREADY-OPEN active store via `world::report_from` (never `gather`, which
//! would reopen the project). Nothing here calls the LLM or mutates anything.
//!
//! - `ink.companions.findings`   ( -- dict )  open findings {socrates:[…], editor:[…]}.
//! - `ink.companions.promotions` ( -- dict )  promotion candidates {socrates:[…], editor:[…]}.
//! - `ink.companions.world`      ( -- dict )  World story-bible health.
//! - `ink.companions.summary`    ( -- dict )  the whole cockpit in one call.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, push};
use crate::inner_editor::InnerEditorStore;
use crate::inner_socrates::storage::InnerSocratesStore;
use crate::store::hierarchy::Hierarchy;
use crate::world_report::WorldReport;

const PROMOTION_THRESHOLD: i64 = 5;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.companions.findings", w_findings),
        ("ink.companions.promotions", w_promotions),
        ("ink.companions.world", w_world),
        ("ink.companions.summary", w_summary),
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

// ── acquisition (fresh sidecar opens — safe alongside the live store) ────────

/// Socrates open findings, or an empty list if the reader has never run (the
/// sidecar DB may not exist yet). Mirrors the shipped `ink.inner_socrates.*`.
fn socrates_findings(tag: &str) -> Vec<crate::inner_socrates::storage::StoredFinding> {
    active_store(tag)
        .ok()
        .and_then(|s| InnerSocratesStore::open_for_project(s.project_root()).ok())
        .and_then(|st| st.list_findings().ok())
        .unwrap_or_default()
}

fn editor_findings(tag: &str) -> Vec<crate::inner_editor::storage::StoredEditorFinding> {
    active_store(tag)
        .ok()
        .and_then(|s| InnerEditorStore::open_for_project(s.project_root()).ok())
        .and_then(|st| st.list_findings().ok())
        .unwrap_or_default()
}

// ── marshalling ─────────────────────────────────────────────────────────────

fn opt_uuid(m: &mut HashMap<String, Value>, key: &str, id: Option<uuid::Uuid>) {
    if let Some(id) = id {
        m.insert(key.into(), Value::from_string(&id.to_string()));
    }
}

fn soc_finding_dict(f: &crate::inner_socrates::storage::StoredFinding) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("id".into(), Value::from_string(&f.id.to_string()));
    opt_uuid(&mut m, "paragraph_id", f.paragraph_id);
    m.insert("category".into(), Value::from_string(f.finding.category.id()));
    m.insert("severity".into(), Value::from_string(f.finding.severity.label()));
    m.insert("question".into(), Value::from_string(&f.finding.question));
    Value::from_dict(m)
}

fn ed_finding_dict(f: &crate::inner_editor::storage::StoredEditorFinding) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("id".into(), Value::from_string(&f.id.to_string()));
    opt_uuid(&mut m, "paragraph_id", f.paragraph_id);
    m.insert("category".into(), Value::from_string(f.finding.category.id()));
    m.insert("severity".into(), Value::from_string(f.finding.severity.id()));
    m.insert("observation".into(), Value::from_string(&f.finding.observation));
    Value::from_dict(m)
}

fn world_report(tag: &str) -> Result<WorldReport> {
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(crate::cli::world::report_from(store, &h, cfg, store.project_root()))
}

fn world_dict(r: &WorldReport) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("facts_total".into(), Value::from_int(r.facts_total as i64));
    m.insert("facts_conflicts".into(), Value::from_int(r.facts_conflicts.len() as i64));
    m.insert("facts_prose_findings".into(), Value::from_int(r.facts_prose_findings as i64));
    m.insert("drift_conflicts".into(), Value::from_int(r.drift_conflicts.len() as i64));
    m.insert("anachronisms".into(), Value::from_int(r.anachronism_flags.len() as i64));
    m.insert("characters".into(), Value::from_int(r.characters as i64));
    m.insert("places".into(), Value::from_int(r.places as i64));
    m.insert("artefacts".into(), Value::from_int(r.artefacts as i64));
    m.insert("entities".into(), Value::from_int(r.entity_total() as i64));
    m.insert("issues".into(), Value::from_int(r.issue_count() as i64));
    m.insert("stale".into(), Value::from_bool(r.stale));
    m.insert("summary".into(), Value::from_string(&r.summary()));
    Value::from_dict(m)
}

fn count_by<T, F: Fn(&T) -> String>(items: &[T], key: F) -> HashMap<String, Value> {
    let mut counts: HashMap<String, i64> = HashMap::new();
    for it in items {
        *counts.entry(key(it)).or_insert(0) += 1;
    }
    counts.into_iter().map(|(k, v)| (k, Value::from_int(v))).collect()
}

// ── words ───────────────────────────────────────────────────────────────────

word!(w_findings, do_findings);
fn do_findings(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.companions.findings";
    let soc = socrates_findings(tag);
    let ed = editor_findings(tag);
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("socrates".into(), Value::from_list(soc.iter().map(soc_finding_dict).collect()));
    m.insert("editor".into(), Value::from_list(ed.iter().map(ed_finding_dict).collect()));
    push(vm, Value::from_dict(m));
    Ok(vm)
}

fn soc_promotions(tag: &str) -> Vec<Value> {
    let store = match active_store(tag) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let candidates = InnerSocratesStore::open_for_project(store.project_root())
        .ok()
        .and_then(|st| st.promotion_candidates(PROMOTION_THRESHOLD).ok())
        .unwrap_or_default();
    candidates
        .iter()
        .map(|c| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("category".into(), Value::from_string(c.category.id()));
            m.insert("chapter".into(), Value::from_string(&c.chapter_id));
            m.insert("count".into(), Value::from_int(c.count));
            Value::from_dict(m)
        })
        .collect()
}

fn ed_promotions(tag: &str) -> Vec<Value> {
    let store = match active_store(tag) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let candidates = InnerEditorStore::open_for_project(store.project_root())
        .ok()
        .and_then(|st| st.promotion_candidates(PROMOTION_THRESHOLD).ok())
        .unwrap_or_default();
    candidates
        .iter()
        .map(|c| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("category".into(), Value::from_string(c.category.id()));
            m.insert("chapter".into(), Value::from_string(&c.chapter_id));
            m.insert("count".into(), Value::from_int(c.count));
            Value::from_dict(m)
        })
        .collect()
}

word!(w_promotions, do_promotions);
fn do_promotions(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.companions.promotions";
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("socrates".into(), Value::from_list(soc_promotions(tag)));
    m.insert("editor".into(), Value::from_list(ed_promotions(tag)));
    push(vm, Value::from_dict(m));
    Ok(vm)
}

word!(w_world, do_world);
fn do_world(vm: &mut VM) -> Result<&mut VM> {
    let report = world_report("ink.companions.world")?;
    push(vm, world_dict(&report));
    Ok(vm)
}

word!(w_summary, do_summary);
fn do_summary(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.companions.summary";
    let soc = socrates_findings(tag);
    let ed = editor_findings(tag);

    let mut soc_dict: HashMap<String, Value> = HashMap::new();
    soc_dict.insert("open".into(), Value::from_int(soc.len() as i64));
    soc_dict.insert(
        "by_severity".into(),
        Value::from_dict(count_by(&soc, |f| f.finding.severity.label().to_string())),
    );
    soc_dict.insert("promotions".into(), Value::from_int(soc_promotions(tag).len() as i64));

    let mut ed_dict: HashMap<String, Value> = HashMap::new();
    ed_dict.insert("open".into(), Value::from_int(ed.len() as i64));
    ed_dict.insert(
        "by_severity".into(),
        Value::from_dict(count_by(&ed, |f| f.finding.severity.id().to_string())),
    );
    ed_dict.insert("promotions".into(), Value::from_int(ed_promotions(tag).len() as i64));

    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("socrates".into(), Value::from_dict(soc_dict));
    m.insert("editor".into(), Value::from_dict(ed_dict));
    // World may be unconfigured; keep the summary resilient rather than erroring.
    if let Ok(report) = world_report(tag) {
        m.insert("world".into(), world_dict(&report));
    }
    push(vm, Value::from_dict(m));
    Ok(vm)
}
