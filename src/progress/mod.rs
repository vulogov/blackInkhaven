//! Writing-progress tracking.
//!
//! Records an append-only event log of writing activity and
//! exposes aggregates the status-bar widget and Ctrl+V G modal
//! consult. Storage is a self-contained DuckDB file at
//! `<project>/progress.db` — reuses the project-wide
//! `StorageEngine` connection pool primitive so failures degrade
//! gracefully (the editor stays usable if the progress store
//! goes missing).
//!
//! ## Event model
//!
//! Two tables:
//!
//! * `writing_events` — every paragraph save, status promotion,
//!   snapshot, or delete. Carries `word_delta` (signed) +
//!   `total_words` (snapshot) so daily / weekly aggregates can be
//!   read without re-walking the manuscript.
//! * `writing_baselines` — per-day snapshot of total words per
//!   book (and project-wide). `today_words` = current total −
//!   baseline-for-today. Captured once per `(day, book_id)`
//!   tuple, on the first save / on project open.
//!
//! ## What counts as a "writing day"?
//!
//! A day with at least one `save` event whose `word_delta > 0`.
//! Empty edits (formatting, status flips) don't tick the streak.
//!
//! ## Streak grace
//!
//! `streak_grace_per_week` from `GoalsConfig` allows N missed
//! days inside the trailing rolling 7-day window before the
//! streak breaks. Useful for writers who take a rest day
//! intentionally.

pub mod aggregates;
pub mod store;
pub mod word_count;

pub use aggregates::{LiveTotals, ProgressSnapshot};
pub use store::ProgressStore;
pub use word_count::count_words;

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use uuid::Uuid;

/// Global progress store handle. Set by `App::new` after the
/// project is opened; queried by `record_save` etc. from the
/// save path and by the status-bar widget on every redraw.
/// Wrapped in `OnceLock` so it's safe to read concurrently and
/// the failure mode (`None`) is a clean "progress tracking
/// disabled" rather than a crash.
static ACTIVE: std::sync::OnceLock<Arc<Mutex<Option<ProgressStore>>>> =
    std::sync::OnceLock::new();

fn slot() -> Arc<Mutex<Option<ProgressStore>>> {
    ACTIVE
        .get_or_init(|| Arc::new(Mutex::new(None)))
        .clone()
}

/// Open (or create) the progress store under the project root.
/// Failures degrade silently — the editor stays usable, but the
/// status-bar widget will show "(progress disabled)" and the
/// Ctrl+V G modal will report the error. Caller logs.
pub fn install(project_root: &Path) -> Result<()> {
    let path = project_root.join("progress.db");
    let store = ProgressStore::open(&path)?;
    let s = slot();
    *s.lock() = Some(store);
    Ok(())
}

/// Drop the active store. Called from the App shutdown hook.
pub fn uninstall() {
    let s = slot();
    *s.lock() = None;
}

/// Run `f` against the active store, returning `None` if no
/// store is installed (progress tracking disabled). The lock is
/// released before `f` returns to keep the active section short.
fn with_store<F, T>(f: F) -> Option<Result<T>>
where
    F: FnOnce(&ProgressStore) -> Result<T>,
{
    let s = slot();
    let guard = s.lock();
    let store = guard.as_ref()?;
    Some(f(store))
}

/// Record a `save` event. `prev_words` is the word count of the
/// paragraph **before** this save; `new_words` is the count
/// **after**. Both totals come from the editor's in-memory body
/// at the moment of save. `word_delta = new_words - prev_words`
/// (can be negative for deletions). Errors are logged + swallowed
/// so a progress-store hiccup never aborts a save.
pub fn record_save(node_id: Uuid, book_id: Option<Uuid>, prev_words: i64, new_words: i64) {
    let delta = new_words - prev_words;
    if let Some(Err(e)) = with_store(|s| s.record_event("save", node_id, book_id, delta, new_words, None)) {
        tracing::warn!(target: "inkhaven::progress", "record_save: {e:#}");
    }
}

/// Record a paragraph status promotion (Napkin → First → …).
/// `new_words` is included so the event row reflects the
/// paragraph's size at the moment of promotion, but
/// `word_delta` is always zero — promotions don't change words.
pub fn record_status_change(
    node_id: Uuid,
    book_id: Option<Uuid>,
    from: &str,
    to: &str,
    total_words: i64,
) {
    let extra = serde_json::json!({ "from": from, "to": to }).to_string();
    if let Some(Err(e)) = with_store(|s| {
        s.record_event(
            "status_change",
            node_id,
            book_id,
            0,
            total_words,
            Some(&extra),
        )
    }) {
        tracing::warn!(target: "inkhaven::progress", "record_status_change: {e:#}");
    }
}

/// Capture per-book + project-wide baselines for today. Idempotent
/// per (day, book_id) — the second call inside the same day is a
/// no-op. Called from `App::new` after the hierarchy + store are
/// open so daily-delta queries see a stable reference point.
pub fn capture_today_baselines(per_book: &[(Uuid, i64)], project_total: i64) {
    if let Some(Err(e)) = with_store(|s| {
        s.capture_baselines_today(per_book, project_total)
    }) {
        tracing::warn!(target: "inkhaven::progress", "capture_today_baselines: {e:#}");
    }
}

/// Build the full progress snapshot the status-bar widget +
/// Ctrl+V G modal consume. `live` carries per-book + project
/// word totals from the editor's hierarchy walk. When the store
/// is unavailable, an empty snapshot is returned so callers keep
/// rendering ("(progress disabled)" surfaces in the status bar).
pub fn snapshot(
    goals: &crate::config::GoalsConfig,
    live: &LiveTotals,
) -> ProgressSnapshot {
    let s = slot();
    let guard = s.lock();
    match guard.as_ref() {
        Some(store) => aggregates::build_snapshot(store, goals, live)
            .unwrap_or_else(|e| {
                tracing::warn!(target: "inkhaven::progress", "snapshot: {e:#}");
                ProgressSnapshot::empty()
            }),
        None => ProgressSnapshot::empty(),
    }
}
