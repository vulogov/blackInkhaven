//! SENTINEL-1 (CT-P8) — `ink.continuity.*` Bund stdlib: the unified deterministic
//! continuity ledger, read-only. The LLM coherence pass is NOT exposed here (it
//! costs and touches cost state); Bund reads what the always-on zero-AI sweep
//! finds — co-location, timeline, numeric, char-fact drift, and the
//! referenced-before-introduced invariant.
//!
//! - `ink.continuity.findings` ( -- list )  the ranked, deduped findings as dicts
//!   `{kind, severity, chapter, source, message, entities}`.
//! - `ink.continuity.check`    ( -- dict )  summary counts `{total, contradictions,
//!   warnings, info, by_kind}`.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, push};
use crate::continuity_intel::{engine, ContinuityFinding, Severity};
use crate::project::ProjectLayout;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.continuity.findings", w_findings),
        ("ink.continuity.check", w_check),
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

/// Run the deterministic engine (every detector) over the active project.
fn gather(tag: &str) -> Result<Vec<ContinuityFinding>> {
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = crate::store::hierarchy::Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let sel = engine::selector(&[], &[]);
    Ok(engine::run(store, cfg, &layout, &h, &sel))
}

fn finding_to_dict(f: &ContinuityFinding) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("kind".into(), Value::from_string(f.kind));
    m.insert("severity".into(), Value::from_string(f.severity.label()));
    m.insert("chapter".into(), Value::from_int(f.chapter as i64));
    m.insert("source".into(), Value::from_string(f.source));
    m.insert("message".into(), Value::from_string(&f.message));
    m.insert(
        "entities".into(),
        Value::from_list(f.entities.iter().map(|e| Value::from_string(e)).collect()),
    );
    Value::from_dict(m)
}

word!(w_findings, do_findings);
fn do_findings(vm: &mut VM) -> Result<&mut VM> {
    let findings = gather("ink.continuity.findings")?;
    let items: Vec<Value> = findings.iter().map(finding_to_dict).collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

word!(w_check, do_check);
fn do_check(vm: &mut VM) -> Result<&mut VM> {
    let findings = gather("ink.continuity.check")?;
    let (mut contradictions, mut warnings, mut info) = (0i64, 0i64, 0i64);
    let mut by_kind: HashMap<String, i64> = HashMap::new();
    for f in &findings {
        *by_kind.entry(f.kind.to_string()).or_insert(0) += 1;
        match f.severity {
            Severity::Contradiction => contradictions += 1,
            Severity::Warning => warnings += 1,
            Severity::Info => info += 1,
        }
    }
    let by_kind: HashMap<String, Value> =
        by_kind.into_iter().map(|(k, v)| (k, Value::from_int(v))).collect();
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("total".into(), Value::from_int(findings.len() as i64));
    m.insert("contradictions".into(), Value::from_int(contradictions));
    m.insert("warnings".into(), Value::from_int(warnings));
    m.insert("info".into(), Value::from_int(info));
    m.insert("by_kind".into(), Value::from_dict(by_kind));
    push(vm, Value::from_dict(m));
    Ok(vm)
}
