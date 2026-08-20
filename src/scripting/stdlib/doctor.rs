//! 3.0.4 Phase-1 — `ink.doctor.*` Bund stdlib: the cheap standalone health
//! checks `inkhaven doctor` runs, read-only. Both wrap methods on the already-open
//! active store, so they carry no re-open cost. The full project scan
//! (`doctor_scan::scan_project`) and any auto-repair are NOT exposed here — a
//! script only reads the diagnosis.
//!
//! - `ink.doctor.integrity` ( -- dict )  DuckDB integrity {meta, blobs, ok}.
//! - `ink.doctor.vectors`   ( -- dict )  vector-index parity {status, rows,
//!   vectors, detail}.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_store, push};
use crate::health::{HealthEvent, HealthFinding};

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] =
        &[("ink.doctor.integrity", w_integrity), ("ink.doctor.vectors", w_vectors)];
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
