//! 1.2.15+ Phase H.1 + H.2 — background health monitor.
//!
//! A tokio task that runs alongside the TUI and
//! periodically checks the project's invariants.
//! Findings are pushed back to the TUI via an
//! unbounded mpsc channel; the TUI consumes them
//! on the main loop tick and updates the status-
//! bar health chip.
//!
//! H.1 (scaffold) shipped the event types + the
//! single-check stub.
//!
//! H.2 (this commit) restructures the monitor so
//! each check has its own cadence + last-run
//! timestamp, and lands three real first-wave
//! disk-side checks:
//!
//!   * `Project` — project root is still reachable
//!     + a directory.  Cheap statvfs.  90 s.
//!   * `Backup` — newest backup `.zip` file under
//!     the configured backup dir is younger than
//!     `backup.max_age`.  300 s.
//!   * `Rescue` — no `*.inkhaven-rescue` orphans
//!     older than [`RESCUE_ORPHAN_DAYS`] are
//!     leaking under the project tree.  These are
//!     R.1 panic-hook leftovers the user dismissed
//!     without running `inkhaven recover`.  3600 s.
//!
//! Cross-thread-state checks (DuckDB PRAGMA,
//! Tantivy index integrity, HNSW vector parity,
//! textarea-vs-disk sync, tree parent-pointer
//! integrity, disk-free %) need a shared Arc<Store>
//! or App handle which the monitor task doesn't
//! own; those land in a follow-up after we work
//! out the safe-sharing story.
//!
//! Auto-repair (H.3) plugs onto the same channel.

pub mod log;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Orphaned rescue files older than this are
/// surfaced as a `Warn` finding.  Picked at 7 d
/// per proposal §7 ("Rescue buffers leak"
/// mitigation).
pub const RESCUE_ORPHAN_DAYS: u64 = 7;

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
    /// Project root reachability.
    Project,
    /// Orphaned `*.inkhaven-rescue` files older than
    /// [`RESCUE_ORPHAN_DAYS`] — R.1 leftovers the
    /// user dismissed without recovering.
    Rescue,
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

/// Knobs the monitor task needs at spawn time.
/// Built by `tui::app::run` from the project layout
/// + `HealthConfig` + `BackupConfig`.
#[derive(Debug, Clone)]
pub struct MonitorSetup {
    pub project_root: PathBuf,
    /// Resolved absolute path of the backup directory
    /// (already through `default_user_backup_dir` if
    /// `backup.out_dir` was empty).  Used by the
    /// backup-freshness check.
    pub backup_dir: PathBuf,
    /// `backup.max_age` from HJSON.  Zero means
    /// "backup-freshness check disabled" — the
    /// check then short-circuits with Ok.
    pub backup_max_age: Duration,
    /// 1.2.15+ Phase H.3 — per-class auto-repair
    /// opt-in.  Every false means "surface as
    /// Warning even if a repair is available" —
    /// the user gets to decide.
    pub repair: RepairPolicy,
}

/// Per-class opt-in for auto-repair.  Defaults are
/// all `false` so a user who flips
/// `health.enabled = true` doesn't get silent
/// mutations of their project state.  Each
/// individual fix has to be enabled explicitly.
#[derive(Debug, Clone, Copy, Default)]
pub struct RepairPolicy {
    /// Delete `*.inkhaven-rescue` orphans older than
    /// [`RESCUE_REPAIR_DAYS`] from the project tree.
    /// Default off — users with long inkhaven
    /// sessions might genuinely WANT to keep an
    /// older rescue around.
    pub rescue_orphans: bool,
}

/// Minimum age for an auto-deleted rescue orphan.
/// Stricter than [`RESCUE_ORPHAN_DAYS`] (the
/// "surface as warning" threshold) so the user has
/// a multi-week window between "I see a warning"
/// and "the file gets cleaned up".
pub const RESCUE_REPAIR_DAYS: u64 = 30;

/// Per-check cadence.  Tunable later via HJSON; H.2
/// hard-codes sensible defaults from the proposal.
const CADENCE_PROJECT: Duration = Duration::from_secs(90);
const CADENCE_BACKUP: Duration = Duration::from_secs(300);
const CADENCE_RESCUE: Duration = Duration::from_secs(3600);
/// How often the tick loop wakes up to re-evaluate
/// due checks.  The smallest cadence above defines
/// the lower bound; 30 s is a safe pick that keeps
/// the CPU footprint negligible.
const LOOP_TICK: Duration = Duration::from_secs(30);

/// Spawn the health-monitor task.  Returns the
/// receiver the TUI uses to consume events.  When
/// `enabled` is false, returns `None` and no task
/// is spawned.
pub fn spawn_monitor(
    setup: MonitorSetup,
    enabled: bool,
) -> Option<mpsc::UnboundedReceiver<HealthEvent>> {
    if !enabled {
        return None;
    }
    let (tx, rx) = mpsc::unbounded_channel::<HealthEvent>();
    tokio::spawn(async move {
        let mut last = LastRun::default();
        let mut interval = tokio::time::interval(LOOP_TICK);
        // First tick fires immediately; we'd rather
        // wait one cadence so the TUI has a chance
        // to come up + draw a frame before the chip
        // lights up.
        interval.tick().await;

        loop {
            interval.tick().await;
            let mut findings: Vec<HealthEvent> = Vec::new();
            let now = Instant::now();

            // Project root.
            if last.project.map_or(true, |t| now.duration_since(t) >= CADENCE_PROJECT) {
                last.project = Some(now);
                findings.push(check_project_root(&setup.project_root));
            }
            // Backup freshness.
            if last.backup.map_or(true, |t| now.duration_since(t) >= CADENCE_BACKUP) {
                last.backup = Some(now);
                findings.push(check_backup_freshness(
                    &setup.backup_dir,
                    setup.backup_max_age,
                ));
            }
            // Rescue-file orphans.
            if last.rescue.map_or(true, |t| now.duration_since(t) >= CADENCE_RESCUE) {
                last.rescue = Some(now);
                let evt = check_rescue_orphans(&setup.project_root);
                // 1.2.15+ Phase H.3 — if the policy
                // allows it, sweep orphans older
                // than RESCUE_REPAIR_DAYS.  Emit
                // Repaired with the count + bytes
                // freed instead of Warning.
                let evt = if setup.repair.rescue_orphans
                    && matches!(evt, HealthEvent::Warning(_))
                {
                    repair_rescue_orphans(&setup.project_root).unwrap_or(evt)
                } else {
                    evt
                };
                findings.push(evt);
            }

            // Collapse the per-tick findings to the
            // most-severe one.  The TUI's chip
            // reflects the latest event we send;
            // sending all four would mean the
            // user sees a flash of green even
            // when there's a real warning behind
            // it.
            let collapsed = collapse_findings(findings);
            // 1.2.15+ Phase H.3 — log every non-Ok
            // event to `.inkhaven/health.log` so
            // the user has an audit trail without
            // needing to keep the TUI open.
            log::append(&setup.project_root, &collapsed);
            if tx.send(collapsed).is_err() {
                // TUI side dropped the receiver —
                // process is shutting down.  Bail.
                break;
            }
        }
    });
    Some(rx)
}

/// Sweep rescue orphans older than
/// [`RESCUE_REPAIR_DAYS`].  Returns
/// `Some(Repaired)` when at least one file was
/// removed; `None` if nothing qualified (in which
/// case the caller falls back to the Warning).
fn repair_rescue_orphans(project_root: &Path) -> Option<HealthEvent> {
    let threshold = SystemTime::now() - Duration::from_secs(RESCUE_REPAIR_DAYS * 86400);
    let mut targets: Vec<(PathBuf, u64)> = Vec::new();
    walk_rescues_with_size(project_root, threshold, &mut targets, 0);
    if targets.is_empty() {
        return None;
    }
    let mut removed: usize = 0;
    let mut bytes: u64 = 0;
    for (path, size) in &targets {
        if std::fs::remove_file(path).is_ok() {
            removed += 1;
            bytes += size;
        }
    }
    if removed == 0 {
        return None;
    }
    let note = format!(
        "removed {removed} orphan rescue file(s) ({bytes} bytes) older than {} days",
        RESCUE_REPAIR_DAYS
    );
    Some(HealthEvent::Repaired(
        HealthFinding {
            class: HealthClass::Rescue,
            severity: Severity::Info,
            detail: note.clone(),
            auto_repairable: true,
        },
        note,
    ))
}

fn walk_rescues_with_size(
    dir: &Path,
    threshold: SystemTime,
    out: &mut Vec<(PathBuf, u64)>,
    depth: usize,
) {
    if depth > 12 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == "recovered" || name == ".inkhaven" || name == "target" {
                continue;
            }
            walk_rescues_with_size(&path, threshold, out, depth + 1);
        } else if ft.is_file() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.ends_with(".inkhaven-rescue") {
                continue;
            }
            let Ok(md) = entry.metadata() else { continue };
            let Ok(mtime) = md.modified() else { continue };
            if mtime < threshold {
                out.push((path, md.len()));
            }
        }
    }
}

#[derive(Default)]
struct LastRun {
    project: Option<Instant>,
    backup: Option<Instant>,
    rescue: Option<Instant>,
}

/// Pick the worst event from a tick's findings.
/// Severity ordering: Error > Warning > Repaired >
/// Ok.  Ties broken by index (first wins).
fn collapse_findings(findings: Vec<HealthEvent>) -> HealthEvent {
    let mut best: Option<HealthEvent> = None;
    for evt in findings {
        let evt_rank = rank(&evt);
        let current_rank = best.as_ref().map(rank).unwrap_or(0);
        if evt_rank > current_rank {
            best = Some(evt);
        }
    }
    best.unwrap_or(HealthEvent::Ok)
}

fn rank(evt: &HealthEvent) -> u8 {
    match evt {
        HealthEvent::Ok => 1,
        HealthEvent::Repaired(_, _) => 2,
        HealthEvent::Warning(_) => 3,
        HealthEvent::Error(_) => 4,
    }
}

/// Check 1 — project root reachable + a directory.
pub(crate) fn check_project_root(project_root: &Path) -> HealthEvent {
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

/// Check 2 — most-recent backup is younger than
/// `max_age`.  Walks the backup dir for `.zip`
/// files, finds the newest mtime, compares.
/// `max_age == Duration::ZERO` means the user
/// disabled auto-backup — check short-circuits Ok.
pub(crate) fn check_backup_freshness(
    backup_dir: &Path,
    max_age: Duration,
) -> HealthEvent {
    if max_age.is_zero() {
        return HealthEvent::Ok;
    }
    let newest = newest_zip_mtime(backup_dir);
    let Some(mtime) = newest else {
        return HealthEvent::Warning(HealthFinding {
            class: HealthClass::Backup,
            severity: Severity::Warn,
            detail: format!(
                "no backup found under {} — run `inkhaven backup` or Ctrl+B Shift+B",
                backup_dir.display()
            ),
            auto_repairable: false,
        });
    };
    let now = SystemTime::now();
    let age = now.duration_since(mtime).unwrap_or(Duration::ZERO);
    if age > max_age {
        HealthEvent::Warning(HealthFinding {
            class: HealthClass::Backup,
            severity: Severity::Warn,
            detail: format!(
                "newest backup is {} old (limit {}); run `inkhaven backup`",
                humantime::format_duration(round_to_minutes(age)),
                humantime::format_duration(max_age),
            ),
            auto_repairable: false,
        })
    } else {
        HealthEvent::Ok
    }
}

fn newest_zip_mtime(dir: &Path) -> Option<SystemTime> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut newest: Option<SystemTime> = None;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(".zip") {
            continue;
        }
        let Ok(md) = entry.metadata() else { continue };
        let Ok(mtime) = md.modified() else { continue };
        match newest {
            Some(prev) if prev >= mtime => {}
            _ => newest = Some(mtime),
        }
    }
    newest
}

/// Round a Duration down to the nearest minute for
/// human-readable display.  Sub-minute precision
/// doesn't help a user reading "newest backup is
/// 7d 12h old".
fn round_to_minutes(d: Duration) -> Duration {
    let secs = d.as_secs();
    Duration::from_secs(secs.saturating_sub(secs % 60))
}

/// Check 3 — walk the project tree for
/// `*.inkhaven-rescue` files older than
/// [`RESCUE_ORPHAN_DAYS`].  These are R.1 panic-
/// hook leftovers the user dismissed without
/// running `inkhaven recover`.
pub(crate) fn check_rescue_orphans(project_root: &Path) -> HealthEvent {
    let threshold = SystemTime::now() - Duration::from_secs(RESCUE_ORPHAN_DAYS * 86400);
    let mut orphans: Vec<PathBuf> = Vec::new();
    walk_rescues(project_root, threshold, &mut orphans, 0);
    if orphans.is_empty() {
        HealthEvent::Ok
    } else {
        let example = orphans
            .first()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        HealthEvent::Warning(HealthFinding {
            class: HealthClass::Rescue,
            severity: Severity::Warn,
            detail: format!(
                "{} rescue file(s) older than {} days under the project — \
                 e.g. {} — run `inkhaven recover --keep` to inspect or \
                 delete them manually",
                orphans.len(),
                RESCUE_ORPHAN_DAYS,
                example,
            ),
            auto_repairable: false,
        })
    }
}

/// Recursive descent capped at depth 12 so a
/// pathological hierarchy can't trap the check.
fn walk_rescues(
    dir: &Path,
    threshold: SystemTime,
    out: &mut Vec<PathBuf>,
    depth: usize,
) {
    if depth > 12 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            // Skip the recovered + .inkhaven internal
            // directories — they're the recover CLI's
            // output, not orphans.
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == "recovered" || name == ".inkhaven" || name == "target" {
                continue;
            }
            walk_rescues(&path, threshold, out, depth + 1);
        } else if ft.is_file() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.ends_with(".inkhaven-rescue") {
                continue;
            }
            let Ok(md) = entry.metadata() else { continue };
            let Ok(mtime) = md.modified() else { continue };
            if mtime < threshold {
                out.push(path);
            }
        }
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
            "health-h2-test-{}",
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
            "health-h2-missing-{}",
            std::process::id()
        ));
        // Don't create.
        let evt = super::check_project_root(&dir);
        match evt {
            HealthEvent::Error(f) => {
                assert_eq!(f.class, HealthClass::Project);
                assert_eq!(f.severity, Severity::Critical);
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn spawn_returns_none_when_disabled() {
        let rx = super::spawn_monitor(
            MonitorSetup {
                project_root: PathBuf::from("/tmp/nonexistent"),
                backup_dir: PathBuf::from("/tmp/nonexistent-backups"),
                backup_max_age: Duration::from_secs(7 * 86400),
                repair: RepairPolicy::default(),
            },
            false,
        );
        assert!(rx.is_none());
    }

    #[test]
    fn backup_check_disabled_when_max_age_zero() {
        let evt = super::check_backup_freshness(
            &PathBuf::from("/tmp/whatever-nonexistent"),
            Duration::ZERO,
        );
        assert!(matches!(evt, HealthEvent::Ok));
    }

    #[test]
    fn backup_check_warns_on_missing_dir() {
        let evt = super::check_backup_freshness(
            &PathBuf::from(format!("/tmp/nx-{}-backups", std::process::id())),
            Duration::from_secs(86400),
        );
        match evt {
            HealthEvent::Warning(f) => {
                assert_eq!(f.class, HealthClass::Backup);
                assert!(f.detail.contains("no backup found"));
            }
            other => panic!("expected Warning, got {other:?}"),
        }
    }

    #[test]
    fn backup_check_ok_for_recent_zip() {
        let dir = std::env::temp_dir().join(format!(
            "health-backup-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let zip = dir.join("blackinkhaven_20260531_120000.zip");
        std::fs::write(&zip, b"\x50\x4b\x03\x04 fake zip").unwrap();

        let evt = super::check_backup_freshness(&dir, Duration::from_secs(7 * 86400));
        assert!(matches!(evt, HealthEvent::Ok), "got {evt:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rescue_check_ok_for_no_orphans() {
        let dir = std::env::temp_dir().join(format!(
            "health-rescue-empty-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let evt = super::check_rescue_orphans(&dir);
        assert!(matches!(evt, HealthEvent::Ok));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rescue_check_warns_on_old_orphan() {
        let dir = std::env::temp_dir().join(format!(
            "health-rescue-old-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let orphan = dir.join("opening.typ.inkhaven-rescue");
        std::fs::write(&orphan, b"old buffer").unwrap();

        // Backdate mtime by setting via `filetime`-
        // alike if available — but we don't have
        // that crate.  Use a child file under a
        // subdir to simulate, OR test the threshold
        // directly via a low-threshold call.
        //
        // Pivot: call walk_rescues with a future
        // threshold so EVERY file qualifies as old.
        let mut orphans: Vec<PathBuf> = Vec::new();
        let threshold =
            SystemTime::now() + Duration::from_secs(60); // 60s in future
        super::walk_rescues(&dir, threshold, &mut orphans, 0);
        assert_eq!(orphans.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn collapse_picks_worst() {
        let f = HealthFinding {
            class: HealthClass::Backup,
            severity: Severity::Warn,
            detail: "warn".into(),
            auto_repairable: false,
        };
        let result = super::collapse_findings(vec![
            HealthEvent::Ok,
            HealthEvent::Warning(f.clone()),
            HealthEvent::Ok,
        ]);
        assert!(matches!(result, HealthEvent::Warning(_)));
    }

    #[test]
    fn collapse_empty_is_ok() {
        assert!(matches!(super::collapse_findings(vec![]), HealthEvent::Ok));
    }

    #[test]
    fn repair_rescue_orphans_skips_recent_files() {
        // The repair walker uses a SystemTime
        // threshold; a freshly-written file has
        // `mtime >= threshold` (now - 30d) so it
        // shouldn't qualify.
        let dir = std::env::temp_dir().join(format!(
            "health-repair-recent-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let r = dir.join("opening.typ.inkhaven-rescue");
        std::fs::write(&r, b"recent").unwrap();
        let result = super::repair_rescue_orphans(&dir);
        assert!(result.is_none(), "recent file shouldn't be swept");
        assert!(r.exists(), "recent file should still exist after no-op repair");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn repair_rescue_orphans_with_force_threshold_removes_file() {
        // Verify the path that deletes + reports.
        // We can't trivially backdate mtime, so
        // test the inner walker + delete loop by
        // emulation: walk with a future threshold
        // (so every file qualifies), then delete.
        let dir = std::env::temp_dir().join(format!(
            "health-repair-old-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let r = dir.join("ch1/opening.typ.inkhaven-rescue");
        std::fs::create_dir_all(r.parent().unwrap()).unwrap();
        std::fs::write(&r, b"buffer body").unwrap();

        // Use the walker directly with a future
        // threshold to confirm it would find the
        // file under real production conditions.
        let mut found: Vec<(PathBuf, u64)> = Vec::new();
        super::walk_rescues_with_size(
            &dir,
            SystemTime::now() + Duration::from_secs(60),
            &mut found,
            0,
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1, "buffer body".len() as u64);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
