//! LECTOR-1 (LR-P8) — `ink.readthrough.*` Bund stdlib: the deterministic
//! read-through, read-only. The LLM synthetic first-read is NOT exposed here (it
//! costs); Bund reads what the always-free forward walk finds — the measured shape
//! curve, the scene/sequel beat, and the reader findings (confusion, info-dump,
//! attention-dip, put-down risk, unpaid setup, scene/sequel arrhythmia, shape sag).
//!
//! - `ink.readthrough.report` ( -- list )  the ranked deduped findings as dicts.
//! - `ink.readthrough.curve`  ( -- list )  per-chapter {chapter, title, position,
//!   measured, expected, kind}.
//! - `ink.readthrough.check`  ( -- dict )  summary counts.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, push};
use crate::lector::{deterministic_findings, scene_sequel, shape, walk, ReaderFinding, Severity};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.readthrough.report", w_report),
        ("ink.readthrough.curve", w_curve),
        ("ink.readthrough.check", w_check),
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

fn finding_to_dict(f: &ReaderFinding) -> Value {
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

word!(w_report, do_report);
fn do_report(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.readthrough.report";
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let rt = walk::read_forward(store, cfg, &layout, &h);
    let findings = deterministic_findings(&rt, &layout, &h, cfg);
    push(vm, Value::from_list(findings.iter().map(finding_to_dict).collect()));
    Ok(vm)
}

word!(w_curve, do_curve);
fn do_curve(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.readthrough.curve";
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let rt = walk::read_forward(store, cfg, &layout, &h);
    let kinds = scene_sequel::chapter_kinds(&layout, &h, cfg);
    let fw = shape::resolve_framework(cfg);
    let expected = shape::expected_curve(fw, rt.chapters.len());
    let n = rt.chapters.len();

    let items: Vec<Value> = rt
        .chapters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let position = if n > 1 { i as f32 / (n - 1) as f32 } else { 0.0 };
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("chapter".into(), Value::from_int(c.chapter as i64));
            m.insert("title".into(), Value::from_string(&c.title));
            m.insert("position".into(), Value::from_float(position as f64));
            m.insert(
                "measured".into(),
                c.measured_intensity.map(|v| Value::from_float(v as f64)).unwrap_or_else(Value::nodata),
            );
            m.insert(
                "expected".into(),
                Value::from_float(expected.get(i).copied().unwrap_or(0.0) as f64),
            );
            m.insert(
                "kind".into(),
                Value::from_string(kinds.get(i).map(|(_, _, k)| k.label()).unwrap_or("")),
            );
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

word!(w_check, do_check);
fn do_check(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.readthrough.check";
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let rt = walk::read_forward(store, cfg, &layout, &h);
    let findings = deterministic_findings(&rt, &layout, &h, cfg);

    let (mut concerns, mut notices, mut info) = (0i64, 0i64, 0i64);
    let mut by_kind: HashMap<String, i64> = HashMap::new();
    for f in &findings {
        *by_kind.entry(f.kind.to_string()).or_insert(0) += 1;
        match f.severity {
            Severity::Concern => concerns += 1,
            Severity::Notice => notices += 1,
            Severity::Info => info += 1,
        }
    }
    let by_kind: HashMap<String, Value> =
        by_kind.into_iter().map(|(k, v)| (k, Value::from_int(v))).collect();
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("chapters".into(), Value::from_int(rt.chapters.len() as i64));
    m.insert("findings".into(), Value::from_int(findings.len() as i64));
    m.insert("concerns".into(), Value::from_int(concerns));
    m.insert("notices".into(), Value::from_int(notices));
    m.insert("info".into(), Value::from_int(info));
    m.insert("by_kind".into(), Value::from_dict(by_kind));
    push(vm, Value::from_dict(m));
    Ok(vm)
}
