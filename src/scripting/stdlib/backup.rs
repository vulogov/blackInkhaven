//! 3.0.4 Phase-1 — `ink.backup.*` Bund stdlib: the backup record, read-only.
//! Phase 1 exposes the last-backup timestamp (a pure filesystem read of the
//! `.inkhaven-backup.json` sidecar — no store open). Making a backup
//! (`ink.backup.make`, fs_write) lands in a later phase.
//!
//! - `ink.backup.last` ( -- dict | NODATA )  {last_at} of the most recent backup,
//!   or NODATA if the project has never been backed up.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_store, push};
use crate::backup::BackupState;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] =
        &[("ink.backup.last", w_last)];
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

/// ( -- dict | NODATA ) the last-backup timestamp, or NODATA if never backed up.
fn w_last(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_last(vm).map_err(to_bund_err)
}
fn do_last(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.backup.last";
    let store = active_store(tag)?;
    let out = match BackupState::load(store.project_root()) {
        Some(state) => {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("last_at".into(), Value::from_string(&state.last_at.to_rfc3339()));
            Value::from_dict(m)
        }
        None => Value::nodata(),
    };
    push(vm, out);
    Ok(vm)
}
