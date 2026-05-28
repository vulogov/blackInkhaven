//! 1.2.10+ — backup listing for the prompts-editor
//! rollback picker.
//!
//! Backups land in `<project>/.prompts-backups/` with
//! filenames like `prompts_YYYYMMDD_HHMMSS.hjson` —
//! the format `app::perform_save` emits.  This module
//! lists / reads / deletes them and formats relative
//! times for the picker's rendered rows.
//!
//! Restore semantics: loading a backup *stages* its
//! contents into the working schema (every entry
//! marked dirty / added / removed relative to the
//! live `prompts.hjson` on disk).  The user reviews,
//! then `Ctrl+S` commits — which writes a fresh
//! backup of the pre-restore state on the way, so the
//! safety chain stays intact.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDateTime};

const PREFIX: &str = "prompts_";
const SUFFIX: &str = ".hjson";
const TS_FORMAT: &str = "%Y%m%d_%H%M%S";
pub const BACKUP_DIR: &str = ".prompts-backups";

#[derive(Debug, Clone)]
pub struct BackupEntry {
    pub path: PathBuf,
    pub filename: String,
    pub timestamp: Option<DateTime<Local>>,
    pub size_bytes: u64,
}

/// List every `prompts_*.hjson` file in
/// `<project>/.prompts-backups/`, newest-first by
/// parsed timestamp.  Missing directory → empty
/// Vec (no error).
pub fn list(project_root: &Path) -> Result<Vec<BackupEntry>> {
    let dir = project_root.join(BACKUP_DIR);
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

pub fn read(entry: &BackupEntry) -> Result<String> {
    fs::read_to_string(&entry.path)
        .with_context(|| format!("read {}", entry.path.display()))
}

pub fn delete(entry: &BackupEntry) -> Result<()> {
    fs::remove_file(&entry.path)
        .with_context(|| format!("remove {}", entry.path.display()))
}

/// Human-readable relative-time string ("12 minutes
/// ago", "3 days ago", "yesterday").  Falls back to
/// absolute date when the parsed timestamp is missing
/// or the entry is older than 14 days.
pub fn relative_time(entry: &BackupEntry, now: DateTime<Local>) -> String {
    let Some(ts) = entry.timestamp else {
        return "(unparsed timestamp)".to_string();
    };
    let delta = now.signed_duration_since(ts);
    let total_secs = delta.num_seconds();
    if total_secs < 0 {
        return ts.format("%Y-%m-%d %H:%M:%S").to_string();
    }
    if total_secs < 60 {
        return "just now".to_string();
    }
    let minutes = total_secs / 60;
    if minutes < 60 {
        return format!(
            "{minutes} minute{} ago",
            if minutes == 1 { "" } else { "s" }
        );
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
            path: PathBuf::from(format!(
                "/tmp/prompts_{}.hjson",
                ts.format(TS_FORMAT)
            )),
            filename: format!("prompts_{}.hjson", ts.format(TS_FORMAT)),
            timestamp: Some(ts),
            size_bytes: 2048,
        }
    }

    fn local_dt(y: i32, m: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(y, m, d, h, mi, s)
            .single()
            .expect("valid local datetime")
    }

    fn tempdir_in_test() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir()
            .join(format!("inkhaven_prompts_backup_test_{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn relative_time_just_now() {
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
    fn relative_time_yesterday() {
        let ts = local_dt(2026, 5, 26, 18, 0, 0);
        let now = local_dt(2026, 5, 27, 18, 30, 0);
        assert_eq!(relative_time(&entry_at(ts), now), "yesterday");
    }

    #[test]
    fn list_sorts_newest_first_skips_unrelated() {
        let dir = tempdir_in_test();
        let nested = dir.join(BACKUP_DIR);
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("prompts_20260527_103000.hjson"), "{}").unwrap();
        fs::write(nested.join("prompts_20260527_103045.hjson"), "{}").unwrap();
        fs::write(nested.join("prompts_20260527_103030.hjson"), "{}").unwrap();
        // Wrong prefix — must not be listed.
        fs::write(nested.join("inkhaven_20260527_103045.hjson"), "{}").unwrap();
        // Wrong extension — must not be listed.
        fs::write(nested.join("prompts_20260527_103045.json"), "{}").unwrap();
        fs::write(nested.join("readme.txt"), "ignore").unwrap();
        let entries = list(&dir).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries[0].filename.ends_with("103045.hjson"));
        assert!(entries[2].filename.ends_with("103000.hjson"));
    }

    #[test]
    fn list_missing_dir_returns_empty() {
        let dir = tempdir_in_test();
        let entries = list(&dir).unwrap();
        assert!(entries.is_empty());
    }
}
