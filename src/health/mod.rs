//! 1.2.15+ Phase H.1 — background health monitor.
//!
//! A tokio task that runs alongside the TUI and
//! periodically checks the project's invariants
//! (DuckDB integrity, Tantivy index consistency,
//! HNSW vector / DB row count parity, textarea-vs-
//! disk sync, tree parent-pointer integrity, disk
//! space).  Findings are pushed back to the TUI
//! via an unbounded mpsc channel; the TUI consumes
//! them on the main loop tick and updates the
//! status-bar health chip.
//!
//! H.1 (this commit) ships the scaffold: the event
//! types, the spawner, the tick loop, the status-
//! bar chip wiring, and one no-op
//! "project-root-exists" check just so we can
//! verify end-to-end that events flow.  The actual
//! integrity checks (DuckDB pragma, Tantivy
//! invariants, vector parity, etc.) land in H.2.
//!
//! Auto-repair (H.3) plugs onto the same channel
//! — when a finding's `auto_repairable` is true,
//! the monitor task attempts the repair before
//! emitting `Repaired` instead of `Warning`.
//!
//! Design notes:
//!
//!   * **Unbounded channel.**  Findings are tiny
//!     (one HealthFinding + an optional String);
//!     the TUI drains them every iteration; the
//!     monitor wakes every 30 s or so.  A bounded
//!     channel would force us to deal with
//!     "dropped because TUI was slow" which is
//!     worse signal than "lots of findings queued
//!     up briefly".
//!
//!   * **Tokio task.**  We already run inside a
//!     multi-thread runtime (see main.rs).  Using
//!     `tokio::spawn` keeps the check loop off the
//!     TUI event-loop thread so a slow check
//!     doesn't block frame rendering.
//!
//!   * **HJSON gate.**  `health.enabled = false`
//!     short-circuits the spawn — useful for
//!     headless CI runs that don't want a
//!     background task.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// One row in the recent-findings log + one event on
/// the channel back to the TUI.  Cheap to clone;
/// findings are typically a one-line `detail`
/// string + a few enum tags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthFinding {
    pub class: HealthClass,
    pub severity: Severity,
    pub detail: String,
    pub auto_repairable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthClass {
    /// The project's DuckDB metadata.db / blobs.db.
    Db,
    /// Tantivy full-text index.
    Index,
    /// HNSW vector store parity with the DB.
    Vectors,
    /// Open editor buffer vs. on-disk file.
    Editor,
    /// Tree parent-pointer integrity.
    Tree,
    /// Disk free space.
    Disk,
    /// Backup freshness.
    Backup,
    /// Project root reachability — the H.1 stub
    /// check.
    Project,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warn,
    Error,
    Critical,
}

/// One event from the monitor task to the TUI.
#[derive(Debug, Clone)]
#[allow(dead_code)] // `Repaired` is constructed by
// the auto-repair flow in H.3 — kept in the enum
// surface now so the consumer side (TUI pump,
// chip state) is already wired.
pub enum HealthEvent {
    /// A tick completed clean — no findings.
    Ok,
    /// A finding surfaced; user-visible but the
    /// monitor didn't (or couldn't) auto-repair.
    Warning(HealthFinding),
    /// A finding surfaced and the monitor auto-
    /// repaired it.  The `String` carries a short
    /// human-readable note ("rebuilt Tantivy index
    /// — 1247 docs reindexed").
    Repaired(HealthFinding, String),
    /// A finding surfaced that the user needs to
    /// intervene on — auto-repair declined or
    /// failed.
    Error(HealthFinding),
}

impl HealthEvent {
    /// Glyph + colour suggestion for the status-bar
    /// chip.  The TUI calls this on the most recent
    /// event to derive the chip state.
    pub fn chip(&self) -> ChipState {
        match self {
            HealthEvent::Ok => ChipState::Clean,
            HealthEvent::Warning(_) => ChipState::Warning,
            HealthEvent::Repaired(_, _) => ChipState::Repaired,
            HealthEvent::Error(_) => ChipState::Error,
        }
    }
}

/// Status-bar chip state.  Derived from the latest
/// event the TUI consumed from the channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChipState {
    /// Nothing to display — the monitor hasn't sent
    /// anything yet, or `health.enabled = false`.
    #[default]
    Hidden,
    /// Last tick clean: `[health: ✓]` green.
    Clean,
    /// Last tick auto-repaired: `[health: ✎]` amber.
    Repaired,
    /// Warning open: `[health: ⚠]` yellow.
    Warning,
    /// Error open: `[health: ✗]` red — user must
    /// intervene.
    Error,
}

impl ChipState {
    /// The single-glyph display.
    pub fn glyph(&self) -> &'static str {
        match self {
            ChipState::Hidden => "",
            ChipState::Clean => "✓",
            ChipState::Repaired => "✎",
            ChipState::Warning => "⚠",
            ChipState::Error => "✗",
        }
    }
}

/// Spawn the health-monitor task.  Returns the
/// receiver the TUI uses to consume events.
///
/// Stub for H.1: one check ("project root
/// reachable") fires every `cadence_secs` seconds
/// and emits [`HealthEvent::Ok`] when clean / a
/// `HealthClass::Project` warning when not.  Real
/// integrity checks land in H.2.
///
/// Returns `None` when `enabled` is false (the TUI
/// then knows to keep the chip hidden).
pub fn spawn_monitor(
    project_root: PathBuf,
    enabled: bool,
    cadence_secs: u64,
) -> Option<mpsc::UnboundedReceiver<HealthEvent>> {
    if !enabled {
        return None;
    }
    let (tx, rx) = mpsc::unbounded_channel::<HealthEvent>();
    tokio::spawn(async move {
        // Cap cadence at [10 s, 1 h] just in case
        // someone sets a degenerate value in HJSON.
        let cadence = std::time::Duration::from_secs(cadence_secs.clamp(10, 3600));
        let mut interval = tokio::time::interval(cadence);
        // First tick fires immediately; we'd rather
        // wait one cadence so the TUI has a chance
        // to come up + draw a frame before the
        // health chip lights up.
        interval.tick().await;

        loop {
            interval.tick().await;
            let evt = check_project_root(&project_root);
            if tx.send(evt).is_err() {
                // TUI side dropped the receiver —
                // process is shutting down.  Bail.
                break;
            }
        }
    });
    Some(rx)
}

/// H.1 stub check.  Verifies the project root
/// directory still resolves — covers the (rare)
/// case where the project was moved or unmounted
/// from under a long-running TUI session.
fn check_project_root(project_root: &Path) -> HealthEvent {
    match std::fs::metadata(project_root) {
        Ok(md) if md.is_dir() => HealthEvent::Ok,
        Ok(_) => HealthEvent::Warning(HealthFinding {
            class: HealthClass::Project,
            severity: Severity::Warn,
            detail: format!(
                "project root {} exists but is not a directory",
                project_root.display()
            ),
            auto_repairable: false,
        }),
        Err(e) => HealthEvent::Error(HealthFinding {
            class: HealthClass::Project,
            severity: Severity::Critical,
            detail: format!(
                "project root {} unreachable: {e}",
                project_root.display()
            ),
            auto_repairable: false,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chip_glyphs_distinct() {
        let g: std::collections::HashSet<&'static str> = [
            ChipState::Clean,
            ChipState::Repaired,
            ChipState::Warning,
            ChipState::Error,
        ]
        .iter()
        .map(|s| s.glyph())
        .collect();
        assert_eq!(g.len(), 4);
    }

    #[test]
    fn check_project_root_ok_for_existing_dir() {
        let dir = std::env::temp_dir().join(format!(
            "health-h1-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let evt = super::check_project_root(&dir);
        assert!(matches!(evt, HealthEvent::Ok));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn check_project_root_error_for_missing() {
        let dir = std::env::temp_dir().join(format!(
            "health-h1-missing-{}",
            std::process::id()
        ));
        // Don't create.
        let evt = super::check_project_root(&dir);
        match evt {
            HealthEvent::Error(f) => {
                assert_eq!(f.class, HealthClass::Project);
                assert_eq!(f.severity, Severity::Critical);
                assert!(!f.auto_repairable);
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn check_project_root_warning_for_file_not_dir() {
        let path = std::env::temp_dir().join(format!(
            "health-h1-file-{}.txt",
            std::process::id()
        ));
        std::fs::write(&path, b"hi").unwrap();
        let evt = super::check_project_root(&path);
        match evt {
            HealthEvent::Warning(f) => {
                assert_eq!(f.class, HealthClass::Project);
                assert_eq!(f.severity, Severity::Warn);
            }
            other => panic!("expected Warning, got {other:?}"),
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn spawn_returns_none_when_disabled() {
        let rx = super::spawn_monitor(
            PathBuf::from("/tmp/nonexistent"),
            false,
            30,
        );
        assert!(rx.is_none());
    }
}
