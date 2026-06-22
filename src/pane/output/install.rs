//! PANE-1 — the process-global active Output store.
//!
//! `output.db` must be opened **once per process**: two DuckDB instances of the
//! same file conflict. The TUI App and the `ink.io.*` Bund words both need the
//! store, so — exactly like `crate::progress` — a single instance is installed
//! into a global slot and shared. Failures degrade to `None` (Output disabled),
//! never a crash.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;

use super::store::OutputStore;

static ACTIVE: std::sync::OnceLock<Arc<Mutex<Option<(PathBuf, OutputStore)>>>> =
    std::sync::OnceLock::new();

fn slot() -> Arc<Mutex<Option<(PathBuf, OutputStore)>>> {
    ACTIVE.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

/// Open (or reuse) the Output store for `project_root` and install it as the
/// process-global active store. Reuses the existing instance when the root is
/// unchanged. Returns a handle (the store is `Clone`, sharing the pool).
pub fn install(project_root: &Path) -> Result<OutputStore> {
    let s = slot();
    let mut g = s.lock();
    if let Some((root, os)) = g.as_ref() {
        if root == project_root {
            return Ok(os.clone());
        }
    }
    let os = OutputStore::open_for_project(project_root)?;
    *g = Some((project_root.to_path_buf(), os.clone()));
    Ok(os)
}

/// The active Output store, or `None` if none is installed.
pub fn active() -> Option<OutputStore> {
    slot().lock().as_ref().map(|(_, os)| os.clone())
}

/// Drop the active store (App shutdown / project switch).
pub fn uninstall() {
    *slot().lock() = None;
}
