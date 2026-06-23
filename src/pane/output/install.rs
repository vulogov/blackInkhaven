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

/// Emit a message **iff** an Output store is active — a no-op otherwise. This is
/// how subsystems route to Output "when running in the TUI": the App installs the
/// store on open, so emission lights up there; a one-shot shell CLI has no store
/// installed and emits nothing (it still prints to stdout as before).
pub fn emit(message: &super::Message) -> Option<uuid::Uuid> {
    active().and_then(|s| s.emit(message).ok())
}

/// PANE-1 P5 — emit a single `ai_task_complete` notice for a finished
/// long-running task (§8.10). `target` is an optional paragraph the Primary
/// action jumps to. Shared by the TUI (deep refresh, fact-check) and any
/// CLI/Bund batch builder so the envelope (Primary/Dismiss/Pin actions,
/// `Hours(12)` lifetime) stays identical everywhere. A no-op when no store is
/// installed.
pub fn emit_task_complete(
    task: &str,
    summary: &str,
    elapsed_secs: u64,
    target: Option<uuid::Uuid>,
) -> Option<uuid::Uuid> {
    use super::types::{kinds, ActionId, Lifetime, Message, Severity};
    let mut meta = serde_json::json!({
        "text": summary,
        "task": task,
        "elapsed_seconds": elapsed_secs,
        "summary": summary,
    });
    if let (Some(obj), Some(id)) = (meta.as_object_mut(), target) {
        obj.insert("target_paragraph".into(), serde_json::Value::String(id.to_string()));
    }
    let mut msg = Message::new(kinds::AI_TASK_COMPLETE, Severity::Info, Lifetime::Hours(12.0), meta)
        .with_actions(vec![ActionId::Primary, ActionId::Dismiss, ActionId::Pin]);
    if let Some(id) = target {
        msg = msg.with_source_paragraph(id);
    }
    emit(&msg)
}
