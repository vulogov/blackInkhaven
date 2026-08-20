//! 3.0.4 — `ink.doctor.*` Bund stdlib: the health checks `inkhaven doctor` runs.
//! Every word here reuses the ALREADY-OPEN active store (integrity/vectors are
//! store methods; scan/autofix go through `doctor_scan::scan_with_store` /
//! `apply_fix_with`, the `&Store` variants added so an in-session caller never
//! needs a second `Store::open`). `integrity`/`vectors`/`scan` are read-only;
//! `autofix` mutates the store (store_write, default-denied).
//!
//! - `ink.doctor.integrity` ( -- dict )  DuckDB integrity {meta, blobs, ok}.
//! - `ink.doctor.vectors`   ( -- dict )  vector-index parity {status, rows,
//!   vectors, detail}.
//! - `ink.doctor.scan`      ( -- list )  the full project scan findings, each
//!   {class, severity, path, detail}.
//! - `ink.doctor.autofix`   ( -- dict )  scan + apply every auto-fixable repair:
//!   {applied, failed, results:[{class, path, ok, message}]}.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, push};
use crate::cli::doctor_scan::{self, ScanFinding};
use crate::health::{HealthEvent, HealthFinding};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.doctor.integrity", w_integrity),
        ("ink.doctor.vectors", w_vectors),
        ("ink.doctor.scan", w_scan),
        ("ink.doctor.autofix", w_autofix),
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

/// ( -- dict ) DuckDB integrity: the `(meta, blobs)` status pair plus a derived
/// `ok` gate (both "ok").
fn w_integrity(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_integrity(vm).map_err(to_bund_err)
}
fn do_integrity(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.doctor.integrity";
    let store = active_store(tag)?;
    let (meta, blobs) = store.integrity_check().map_err(|e| anyhow!("{tag}: {e}"))?;
    let ok = meta == "ok" && blobs == "ok";
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("meta".into(), Value::from_string(&meta));
    m.insert("blobs".into(), Value::from_string(&blobs));
    m.insert("ok".into(), Value::from_bool(ok));
    push(vm, Value::from_dict(m));
    Ok(vm)
}

fn add_finding(m: &mut HashMap<String, Value>, f: &HealthFinding) {
    m.insert("detail".into(), Value::from_string(&f.detail));
    m.insert("class".into(), Value::from_string(&format!("{:?}", f.class)));
    m.insert("severity".into(), Value::from_string(&format!("{:?}", f.severity)));
    m.insert("auto_repairable".into(), Value::from_bool(f.auto_repairable));
}

/// ( -- dict ) vector-index parity: {status, rows, vectors, detail}. `status` is
/// one of ok / warning / error / repaired, mirroring the doctor's health chip.
fn w_vectors(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_vectors(vm).map_err(to_bund_err)
}
fn do_vectors(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.doctor.vectors";
    let store = active_store(tag)?;
    let event = crate::health::check_vector_parity(store);
    let rows = store.row_count().map_err(|e| anyhow!("{tag}: {e}"))?;
    let vectors = store.vector_count().map_err(|e| anyhow!("{tag}: {e}"))?;

    let mut m: HashMap<String, Value> = HashMap::new();
    match &event {
        HealthEvent::Ok => {
            m.insert("status".into(), Value::from_string("ok"));
        }
        HealthEvent::Warning(f) => {
            m.insert("status".into(), Value::from_string("warning"));
            add_finding(&mut m, f);
        }
        HealthEvent::Error(f) => {
            m.insert("status".into(), Value::from_string("error"));
            add_finding(&mut m, f);
        }
        HealthEvent::Repaired(f, note) => {
            m.insert("status".into(), Value::from_string("repaired"));
            add_finding(&mut m, f);
            m.insert("note".into(), Value::from_string(note));
        }
    }
    m.insert("rows".into(), Value::from_int(rows as i64));
    m.insert("vectors".into(), Value::from_int(vectors as i64));
    push(vm, Value::from_dict(m));
    Ok(vm)
}

fn finding_dict(f: &ScanFinding) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("class".into(), Value::from_string(f.class.slug()));
    m.insert("severity".into(), Value::from_string(f.severity.slug()));
    if let Some(p) = &f.path {
        m.insert("path".into(), Value::from_string(p));
    }
    m.insert("detail".into(), Value::from_string(&f.detail));
    Value::from_dict(m)
}

/// ( -- list ) the full project scan findings, reusing the active store
/// (`scan_with_store`) — no second `Store::open`.
fn w_scan(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_scan(vm).map_err(to_bund_err)
}
fn do_scan(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.doctor.scan";
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let report = doctor_scan::scan_with_store(store, &layout, cfg, &h, None);
    let items: Vec<Value> = report.findings.iter().map(finding_dict).collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

/// ( -- dict ) scan + apply every auto-fixable repair over the active store.
/// Auto-fixable classes (zero-byte / orphan / missing-file / corrupt-comments /
/// bdslib-only) repair; author-judgment and detection-only findings come back
/// `ok:false` with the reason (they mutate nothing). Each fix is logged to
/// `.inkhaven/doctor.log`, exactly as the CLI / TUI autofix does.
fn w_autofix(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_autofix(vm).map_err(to_bund_err)
}
fn do_autofix(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.doctor.autofix";
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let root = store.project_root().to_path_buf();
    let layout = ProjectLayout::new(&root);
    let mut h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let report = doctor_scan::scan_with_store(store, &layout, cfg, &h, None);

    let (mut applied, mut failed) = (0i64, 0i64);
    let mut results: Vec<Value> = Vec::new();
    for f in &report.findings {
        let outcome = doctor_scan::apply_fix_with(store, &layout, &h, f);
        doctor_scan::log_fix(&root, f, &outcome);
        let mut d: HashMap<String, Value> = HashMap::new();
        d.insert("class".into(), Value::from_string(f.class.slug()));
        if let Some(p) = &f.path {
            d.insert("path".into(), Value::from_string(p));
        }
        match &outcome {
            Ok(note) => {
                applied += 1;
                d.insert("ok".into(), Value::from_bool(true));
                d.insert("message".into(), Value::from_string(note));
                // A successful fix mutated the store — refresh the hierarchy so a
                // later fix sees current state (apply_fix reloads fresh per call).
                if let Ok(nh) = Hierarchy::load(store) {
                    h = nh;
                }
            }
            Err(e) => {
                failed += 1;
                d.insert("ok".into(), Value::from_bool(false));
                d.insert("message".into(), Value::from_string(&e.to_string()));
            }
        }
        results.push(Value::from_dict(d));
    }

    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("applied".into(), Value::from_int(applied));
    m.insert("failed".into(), Value::from_int(failed));
    m.insert("results".into(), Value::from_list(results));
    push(vm, Value::from_dict(m));
    Ok(vm)
}
