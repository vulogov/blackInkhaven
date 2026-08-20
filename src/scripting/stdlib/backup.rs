//! 3.0.4 — `ink.backup.*` Bund stdlib: the backup record, read-only. `last`
//! reads the `.inkhaven-backup.json` sidecar (pure filesystem, no store open);
//! `list` enumerates the backup zips on disk (an fs_read of the project's backup
//! directory). Making a backup (`ink.backup.make`, fs_write) lands in a later
//! phase.
//!
//! - `ink.backup.last` ( -- dict | NODATA )  {last_at} of the most recent backup,
//!   or NODATA if the project has never been backed up.
//! - `ink.backup.list` ( -- list )  the backup zips, newest first, each
//!   {name, bytes, modified}.
//! - `ink.backup.make` ( -- dict )  create a backup zip now (fs_write,
//!   default-denied) → {archive, kept}. Mirrors `inkhaven backup`; also records
//!   the `.inkhaven-backup.json` timestamp `ink.backup.last` reads.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, push};
use crate::backup::BackupState;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] =
        &[("ink.backup.last", w_last), ("ink.backup.list", w_list), ("ink.backup.make", w_make)];
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

/// ( -- list ) the backup zips in the project's backup dir, newest first, each
/// {name, bytes, modified}. Mirrors `prune_backups`' selection predicate.
fn w_list(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_list(vm).map_err(to_bund_err)
}
fn do_list(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.backup.list";
    let store = active_store(tag)?;
    let dir = crate::store::default_user_backup_dir(store.project_root());

    // (mtime, dict) so we can sort newest-first like the pruner does.
    let mut rows: Vec<(std::time::SystemTime, Value)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !(name.starts_with("blackinkhaven_") && name.ends_with(".zip")) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("name".into(), Value::from_string(&name));
            m.insert("bytes".into(), Value::from_int(meta.len() as i64));
            m.insert(
                "modified".into(),
                Value::from_string(&chrono::DateTime::<chrono::Utc>::from(mtime).to_rfc3339()),
            );
            rows.push((mtime, Value::from_dict(m)));
        }
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    push(vm, Value::from_list(rows.into_iter().map(|(_, v)| v).collect()));
    Ok(vm)
}

/// ( -- dict ) create a backup zip now → {archive, kept}. The same
/// filesystem-level snapshot `inkhaven backup` writes (no store open needed);
/// enforces the `backup.keep_last` retention cap and records the timestamp.
fn w_make(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_make(vm).map_err(to_bund_err)
}
fn do_make(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.backup.make";
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    // Canonicalize like the CLI: `create_backup` strip_prefixes included paths
    // against the project root, so a non-canonical root would fail.
    let root = std::fs::canonicalize(store.project_root())
        .map_err(|e| anyhow!("{tag}: canonicalize project root: {e}"))?;
    let out = crate::store::default_user_backup_dir(&root);
    let skip = crate::cli::backup::skip_dirs_for(&root, &out);
    let archive = crate::backup::create_backup(&root, &out, &skip, None)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    crate::backup::prune_backups(&out, cfg.backup.keep_last);

    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("archive".into(), Value::from_string(&archive.display().to_string()));
    m.insert("kept".into(), Value::from_int(cfg.backup.keep_last as i64));
    push(vm, Value::from_dict(m));
    Ok(vm)
}
