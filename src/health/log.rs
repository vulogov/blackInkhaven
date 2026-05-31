//! 1.2.15+ Phase H.3 — health-monitor log file.
//!
//! Every finding the monitor produces — every
//! `Warning`, `Repaired`, `Error` — gets a line
//! appended to `<project>/.inkhaven/health.log`.
//! The user (or future-us) can read it back to see
//! what the monitor has been doing over time
//! without having to keep the TUI open.
//!
//! Log format is one finding per line, pipe-
//! separated for easy `cut` / `awk`:
//!
//! ```text
//! 2026-05-31T14:00:00Z|Warning|Backup|backup older than 7d
//! 2026-05-31T14:00:30Z|Repaired|Rescue|removed 2 orphan files (4823 bytes)
//! ```
//!
//! Rotation is size-based: when the log reaches
//! [`MAX_LOG_BYTES`], it's renamed to
//! `health.log.1`; existing `.1` → `.2`, `.2` →
//! `.3`, etc., with `.5` dropped on the floor.
//! Keeps total disk footprint bounded at
//! `6 × MAX_LOG_BYTES` (~6 MB by default).
//!
//! Failures from any I/O step are silent
//! (`let _ = …`) — the log is diagnostic, not
//! load-bearing, and we'd rather the monitor task
//! keep checking than die on a write error.

use std::path::{Path, PathBuf};

use super::HealthEvent;

/// Rotate when the current log exceeds this size.
/// 1 MB per file × 6 files (current + 5 rotated)
/// caps the total log footprint at ~6 MB which is
/// negligible against typical project sizes.
pub const MAX_LOG_BYTES: u64 = 1024 * 1024;

/// How many rotated copies to keep.  `.log` is the
/// active file; `.log.1` … `.log.{KEEP}` are the
/// rotated archives.
pub const KEEP: usize = 5;

/// Path of the active log file inside the project.
pub fn log_path(project_root: &Path) -> PathBuf {
    project_root.join(".inkhaven").join("health.log")
}

/// Append one event to the log file.  Rotates
/// first if the active log is at or above
/// [`MAX_LOG_BYTES`].  No-op for `HealthEvent::Ok`
/// — we don't log the clean ticks (every 30 s
/// they'd dominate the file).
pub fn append(project_root: &Path, evt: &HealthEvent) {
    if matches!(evt, HealthEvent::Ok) {
        return;
    }
    let path = log_path(project_root);
    let _ = ensure_parent_dir(&path);
    rotate_if_needed(&path);
    let line = format_event(evt);
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(line.as_bytes())
        });
}

fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn format_event(evt: &HealthEvent) -> String {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    match evt {
        HealthEvent::Ok => format!("{now}|Ok|-|clean\n"),
        HealthEvent::Warning(f) => format!(
            "{now}|Warning|{:?}|{}\n",
            f.class,
            f.detail.replace('\n', " ")
        ),
        HealthEvent::Repaired(f, note) => format!(
            "{now}|Repaired|{:?}|{}\n",
            f.class,
            note.replace('\n', " ")
        ),
        HealthEvent::Error(f) => format!(
            "{now}|Error|{:?}|{}\n",
            f.class,
            f.detail.replace('\n', " ")
        ),
    }
}

fn rotate_if_needed(active: &Path) {
    let Ok(md) = std::fs::metadata(active) else { return };
    if md.len() < MAX_LOG_BYTES {
        return;
    }
    // Drop the oldest, then shift each up.
    let oldest = numbered(active, KEEP);
    let _ = std::fs::remove_file(&oldest);
    for i in (1..KEEP).rev() {
        let from = numbered(active, i);
        let to = numbered(active, i + 1);
        let _ = std::fs::rename(from, to);
    }
    let dst = numbered(active, 1);
    let _ = std::fs::rename(active, dst);
}

fn numbered(active: &Path, n: usize) -> PathBuf {
    let mut s = active.as_os_str().to_os_string();
    s.push(format!(".{n}"));
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::{HealthClass, HealthFinding, Severity};

    fn tmp_project(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "health-log-{}-{}-{}",
            label,
            std::process::id(),
            chrono::Utc::now()
                .timestamp_nanos_opt()
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn warn(detail: &str) -> HealthEvent {
        HealthEvent::Warning(HealthFinding {
            class: HealthClass::Backup,
            severity: Severity::Warn,
            detail: detail.into(),
            auto_repairable: false,
        })
    }

    #[test]
    fn append_writes_line_per_event() {
        let dir = tmp_project("append");
        append(&dir, &warn("backup stale"));
        append(&dir, &warn("backup still stale"));
        let body = std::fs::read_to_string(log_path(&dir)).unwrap();
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("|Warning|Backup|"));
        assert!(lines[0].contains("backup stale"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn append_skips_ok_events() {
        let dir = tmp_project("ok");
        append(&dir, &HealthEvent::Ok);
        assert!(
            !log_path(&dir).exists(),
            "Ok events should not create the log file"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotation_renames_active_to_dot1() {
        let dir = tmp_project("rotate");
        let active = log_path(&dir);
        std::fs::create_dir_all(active.parent().unwrap()).unwrap();

        // Pre-populate the active log with > MAX_LOG_BYTES
        // bytes so the next append triggers rotation.
        let body = vec![b'X'; (MAX_LOG_BYTES as usize) + 256];
        std::fs::write(&active, &body).unwrap();

        append(&dir, &warn("triggers rotate"));

        // `.log.1` should now contain the old body.
        let rotated = active.with_extension("log.1");
        assert!(rotated.exists(), "rotated file missing: {}", rotated.display());
        // Active log should now hold only the new line.
        let new_body = std::fs::read_to_string(&active).unwrap();
        assert_eq!(new_body.lines().count(), 1);
        assert!(new_body.contains("triggers rotate"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotation_drops_oldest_at_keep_limit() {
        let dir = tmp_project("rotate-cap");
        let active = log_path(&dir);
        std::fs::create_dir_all(active.parent().unwrap()).unwrap();

        // Set up an active file + .log.1 … .log.5 with
        // distinct content, then trigger rotation.
        let mut tag = |n: usize| -> Vec<u8> {
            let mut v = format!("tag{n}-").into_bytes();
            v.extend(std::iter::repeat(b'.').take((MAX_LOG_BYTES as usize) + 16));
            v
        };
        std::fs::write(&active, tag(0)).unwrap();
        for i in 1..=KEEP {
            std::fs::write(numbered(&active, i), tag(i)).unwrap();
        }

        append(&dir, &warn("kicks rotation"));

        // After rotation: previously .5 should be gone,
        // each numbered file shifts up by 1.
        let body1 = std::fs::read(numbered(&active, 1)).unwrap();
        assert!(body1.starts_with(b"tag0-"), "expected former active in .log.1");
        let body5 = std::fs::read(numbered(&active, 5)).unwrap();
        assert!(body5.starts_with(b"tag4-"), "expected former .log.4 in .log.5");
        // There should be no .log.6 — KEEP=5.
        assert!(
            !numbered(&active, 6).exists(),
            ".log.6 should not exist after rotation",
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
