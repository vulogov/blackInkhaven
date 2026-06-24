//! General AI-call usage tracking (road to 1.4.0).
//!
//! Every inkhaven AI inference records a `(UTC-day, category)` tally so the cost
//! dashboard reflects total spend — chat, grammar, explain, continuation, critique,
//! … — not just the two capped slow tracks. A process-global project root is
//! installed at TUI/CLI startup (like the Output pane); [`record`] is a no-op until
//! then (headless, or before a project is open). Tallies live in
//! `<project>/.inkhaven/ai_usage.json` keyed `day → category → calls`; days older
//! than the most recent 30 are pruned on write.
//!
//! Counts are by **calendar day (UTC)** — the same clock the slow-track daily caps
//! use — so a day's tally implicitly resets at 00:00 UTC.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, RwLock};

static ROOT: RwLock<Option<PathBuf>> = RwLock::new(None);
/// Serialises the read-modify-write so concurrent inferences don't lose updates.
static WRITE: Mutex<()> = Mutex::new(());
/// Trailing days of tallies to keep (config-driven; `cost.usage_retention_days`).
static RETENTION_DAYS: AtomicUsize = AtomicUsize::new(30);

/// Per-day, per-category call tallies.
type Usage = BTreeMap<String, BTreeMap<String, i64>>;

/// Point the tracker at a project (call once at TUI / CLI startup) and
/// set the retention window from config.
pub fn install(project_root: &Path, retention_days: usize) {
    if let Ok(mut g) = ROOT.write() {
        *g = Some(project_root.to_path_buf());
    }
    RETENTION_DAYS.store(retention_days.max(1), Ordering::Relaxed);
}

fn file(root: &Path) -> PathBuf {
    root.join(".inkhaven").join("ai_usage.json")
}

fn load(root: &Path) -> Usage {
    std::fs::read_to_string(file(root))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn today() -> String {
    crate::dayclock::today_key()
}

/// Record one AI call under `category` for today. No-op when uninstalled.
pub fn record(category: &str) {
    let root = match ROOT.read() {
        Ok(g) => g.clone(),
        Err(_) => None,
    };
    let Some(root) = root else { return };
    // Recover the guard even if a prior holder panicked — dropping the
    // PoisonError would otherwise proceed *unserialized* and lose
    // concurrent tallies, the opposite of this lock's purpose (M8).
    let _guard = WRITE.lock().unwrap_or_else(|e| e.into_inner());
    let mut usage = load(&root);
    *usage
        .entry(today())
        .or_default()
        .entry(category.to_string())
        .or_insert(0) += 1;
    // Keep only the most recent `RETENTION_DAYS` days (BTreeMap keys sort,
    // so the front is oldest).
    let keep = RETENTION_DAYS.load(Ordering::Relaxed).max(1);
    while usage.len() > keep {
        let Some(oldest) = usage.keys().next().cloned() else { break };
        usage.remove(&oldest);
    }
    let path = file(&root);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(&usage) {
        let _ = crate::io_atomic::write(&path, json.as_bytes());
    }
}

/// The `(category, calls)` tallies recorded for `day` (sorted by category).
pub fn usage_for(root: &Path, day: &str) -> Vec<(String, i64)> {
    load(root)
        .get(day)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), *v)).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_tallies_by_category_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        install(dir.path(), 30);
        record("chat");
        record("chat");
        record("grammar");
        let day = today();
        let mut got = usage_for(dir.path(), &day);
        got.sort();
        assert_eq!(got, vec![("chat".to_string(), 2), ("grammar".to_string(), 1)]);
    }

    #[test]
    fn usage_for_unknown_day_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(usage_for(dir.path(), "1999-01-01").is_empty());
    }
}
