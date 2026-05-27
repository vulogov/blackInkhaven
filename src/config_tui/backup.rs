//! 1.2.10+ — backup listing for the rollback picker.
//!
//! Backups land in `<project>/.config-backups/` with
//! filenames like `inkhaven_YYYYMMDD_HHMMSS.hjson` —
//! the format `save.rs::write_backup` emits.  This
//! module lists them, parses timestamps from the
//! filenames, sorts newest-first, and exposes
//! preview / restore / delete helpers.
//!
//! Restore semantics: loading a backup *stages* its
//! contents into the working schema tree but does NOT
//! write to disk.  The user reviews the staged
//! changes against the live HJSON, then either
//! `Ctrl+S` commits (creating a new backup of the
//! pre-restore state on the way) or `Esc` /
//! `Ctrl+Q` to discard.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDateTime};

/// Filename prefix every backup carries.
const PREFIX: &str = "inkhaven_";
const SUFFIX: &str = ".hjson";
/// Strftime format the timestamp portion is encoded
/// with — same one `save.rs::write_backup` emits.
const TS_FORMAT: &str = "%Y%m%d_%H%M%S";

#[derive(Debug, Clone)]
pub struct BackupEntry {
    pub path: PathBuf,
    pub filename: String,
    /// Parsed timestamp (local time).  `None` when
    /// the filename doesn't match the canonical
    /// pattern — those entries land at the bottom of
    /// the list and lose the relative-time chip.
    pub timestamp: Option<DateTime<Local>>,
    pub size_bytes: u64,
}

/// List every `.hjson` file in
/// `<project>/.config-backups/`.  Returns entries
/// sorted newest-first by parsed timestamp; entries
/// with unparseable names fall to the bottom in
/// filename order.  Missing directory → empty Vec
/// (no error).
pub fn list(project_root: &Path) -> Result<Vec<BackupEntry>> {
    let dir = project_root.join(".config-backups");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries: Vec<BackupEntry> = Vec::new();
    for raw in fs::read_dir(&dir)
        .with_context(|| format!("read_dir {}", dir.display()))?
    {
        let entry = raw?;
        let path = entry.path();
        let metadata = entry.metadata().ok();
        if !path.is_file() {
            continue;
        }
        let filename = match path.file_name().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        if !filename.starts_with(PREFIX) || !filename.ends_with(SUFFIX) {
            continue;
        }
        let ts_segment = &filename[PREFIX.len()..filename.len() - SUFFIX.len()];
        let timestamp = NaiveDateTime::parse_from_str(ts_segment, TS_FORMAT)
            .ok()
            .and_then(|naive| naive.and_local_timezone(Local).single());
        let size_bytes = metadata.map(|m| m.len()).unwrap_or(0);
        entries.push(BackupEntry {
            path,
            filename,
            timestamp,
            size_bytes,
        });
    }
    entries.sort_by(|a, b| match (a.timestamp, b.timestamp) {
        (Some(ta), Some(tb)) => tb.cmp(&ta),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.filename.cmp(&b.filename),
    });
    Ok(entries)
}

/// Read a backup file's full contents.  Errors when
/// the path is missing or unreadable.
pub fn read(entry: &BackupEntry) -> Result<String> {
    fs::read_to_string(&entry.path)
        .with_context(|| format!("read {}", entry.path.display()))
}

/// Delete a backup file.  Errors when the path is
/// missing.  Caller is responsible for the user
/// confirmation modal.
pub fn delete(entry: &BackupEntry) -> Result<()> {
    fs::remove_file(&entry.path)
        .with_context(|| format!("remove {}", entry.path.display()))
}

/// Format a backup's timestamp as a human-readable
/// relative-time string ("12 minutes ago", "3 days
/// ago", "yesterday").  Falls back to absolute date
/// when the parsed timestamp is missing.
pub fn relative_time(entry: &BackupEntry, now: DateTime<Local>) -> String {
    let Some(ts) = entry.timestamp else {
        return "(unparsed timestamp)".to_string();
    };
    let delta = now.signed_duration_since(ts);
    let total_secs = delta.num_seconds();
    if total_secs < 0 {
        // Timestamp in the future — unusual; just
        // show absolute date.
        return ts.format("%Y-%m-%d %H:%M:%S").to_string();
    }
    if total_secs < 60 {
        return "just now".to_string();
    }
    let minutes = total_secs / 60;
    if minutes < 60 {
        return format!("{minutes} minute{} ago", if minutes == 1 { "" } else { "s" });
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours} hour{} ago", if hours == 1 { "" } else { "s" });
    }
    let days = hours / 24;
    if days == 1 {
        return "yesterday".to_string();
    }
    if days < 14 {
        return format!("{days} days ago");
    }
    ts.format("%Y-%m-%d %H:%M").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn entry_at(ts: DateTime<Local>) -> BackupEntry {
        BackupEntry {
            path: PathBuf::from(format!("/tmp/inkhaven_{}.hjson", ts.format(TS_FORMAT))),
            filename: format!("inkhaven_{}.hjson", ts.format(TS_FORMAT)),
            timestamp: Some(ts),
            size_bytes: 1024,
        }
    }

    fn local_dt(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(y, m, d, h, mi, s)
            .single()
            .expect("valid local datetime")
    }

    #[test]
    fn relative_time_just_now_under_60s() {
        let ts = local_dt(2026, 5, 27, 10, 0, 0);
        let now = local_dt(2026, 5, 27, 10, 0, 30);
        assert_eq!(relative_time(&entry_at(ts), now), "just now");
    }

    #[test]
    fn relative_time_minutes() {
        let ts = local_dt(2026, 5, 27, 10, 0, 0);
        let now = local_dt(2026, 5, 27, 10, 12, 0);
        assert_eq!(relative_time(&entry_at(ts), now), "12 minutes ago");
    }

    #[test]
    fn relative_time_singular_minute() {
        let ts = local_dt(2026, 5, 27, 10, 0, 0);
        let now = local_dt(2026, 5, 27, 10, 1, 30);
        assert_eq!(relative_time(&entry_at(ts), now), "1 minute ago");
    }

    #[test]
    fn relative_time_yesterday() {
        let ts = local_dt(2026, 5, 26, 18, 0, 0);
        let now = local_dt(2026, 5, 27, 18, 30, 0);
        assert_eq!(relative_time(&entry_at(ts), now), "yesterday");
    }

    #[test]
    fn relative_time_days() {
        let ts = local_dt(2026, 5, 23, 9, 0, 0);
        let now = local_dt(2026, 5, 27, 18, 0, 0);
        assert!(relative_time(&entry_at(ts), now).starts_with("4 days ago"));
    }

    #[test]
    fn relative_time_absolute_for_old_entries() {
        let ts = local_dt(2025, 12, 1, 9, 0, 0);
        let now = local_dt(2026, 5, 27, 18, 0, 0);
        // 14+ days ago → absolute YYYY-MM-DD HH:MM.
        let out = relative_time(&entry_at(ts), now);
        assert!(out.starts_with("2025-12-01"), "got: {out}");
    }

    #[test]
    fn list_skips_unrelated_files_and_sorts_newest_first() {
        // Smoke test against a synthetic directory.
        let dir = tempdir_in_test();
        let nested = dir.join(".config-backups");
        fs::create_dir_all(&nested).unwrap();
        // Create three backups + one stray file.
        fs::write(
            nested.join("inkhaven_20260527_103000.hjson"),
            "{}",
        )
        .unwrap();
        fs::write(
            nested.join("inkhaven_20260527_103045.hjson"),
            "{}",
        )
        .unwrap();
        fs::write(
            nested.join("inkhaven_20260527_103030.hjson"),
            "{}",
        )
        .unwrap();
        fs::write(nested.join("readme.txt"), "ignore me").unwrap();
        let entries = list(&dir).unwrap();
        assert_eq!(entries.len(), 3);
        // Newest-first ordering.
        assert!(entries[0].filename.ends_with("103045.hjson"));
        assert!(entries[1].filename.ends_with("103030.hjson"));
        assert!(entries[2].filename.ends_with("103000.hjson"));
    }

    #[test]
    fn list_handles_missing_backup_dir() {
        let dir = tempdir_in_test();
        // No `.config-backups/` at all.
        let entries = list(&dir).unwrap();
        assert!(entries.is_empty());
    }

    fn tempdir_in_test() -> PathBuf {
        // Cheap test-only tempdir; cleaned up by the
        // OS when the test ends.  Uniqueness comes
        // from a nanosecond timestamp.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir()
            .join(format!("inkhaven_config_tui_test_{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
