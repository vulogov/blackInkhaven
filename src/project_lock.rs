//! 1.3.36 hardening — an advisory single-instance lock on a project.
//!
//! Two `inkhaven` TUI sessions open on the same project both write
//! `metadata.db` and `.session.json`; interleaved writes can corrupt
//! the store. This guards against that **without ever hard-blocking**
//! — in keeping with Inkhaven's permissive principle, the lock
//! *informs*. The interactive launcher warns and lets the author open
//! anyway; only genuine data-safety is at stake, and the choice stays
//! theirs.
//!
//! Mechanism: an OS advisory lock (`flock`/`LockFileEx` via `fs2`) on
//! `<project>/.inkhaven.lock`. The kernel releases it automatically
//! when the holding process exits — including on a crash or `kill -9`
//! — so there is **no stale-lock cleanup** to get wrong: a dead
//! session never locks anyone out. The PID/host/time written into the
//! file are only there to make the "already open" warning friendly.

use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;

/// Identifying details of whoever currently holds the lock — read
/// back from the lockfile purely to make the warning informative.
#[derive(Debug, Clone, Default)]
pub struct LockInfo {
    pub pid: u32,
    pub host: String,
    /// Unix seconds when the holder acquired the lock.
    pub started_at: i64,
}

impl LockInfo {
    fn current() -> Self {
        let host = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_default();
        // `now` is read here (not via a forbidden Date::now in scripts)
        // — this is ordinary runtime code.
        let started_at = chrono::Utc::now().timestamp();
        Self { pid: std::process::id(), host, started_at }
    }

    fn to_line(&self) -> String {
        format!("{} {} {}\n", self.pid, self.started_at, self.host)
    }

    fn parse(s: &str) -> Self {
        let line = s.lines().next().unwrap_or("");
        let mut parts = line.splitn(3, ' ');
        let pid = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let started_at = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let host = parts.next().unwrap_or("").trim().to_string();
        Self { pid, host, started_at }
    }

    /// Human phrasing for the warning, e.g. `PID 4321 on mac since 14:02`.
    pub fn describe(&self) -> String {
        let when = chrono::DateTime::from_timestamp(self.started_at, 0)
            .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M").to_string())
            .unwrap_or_else(|| "an earlier time".into());
        let host = if self.host.is_empty() {
            String::new()
        } else {
            format!(" on {}", self.host)
        };
        format!("PID {}{host} since {when}", self.pid)
    }
}

/// A held project lock. Dropping it releases the OS lock (the kernel
/// also releases on process exit, so a crash can't leave it stuck).
pub struct ProjectLock {
    file: File,
    #[allow(dead_code)]
    path: PathBuf,
}

impl Drop for ProjectLock {
    fn drop(&mut self) {
        // Best-effort: the fd close on `file`'s drop would release the
        // lock anyway; doing it explicitly keeps intent clear. The
        // lockfile itself is left in place as a stable anchor — empty
        // and harmless, reused on the next launch — which avoids the
        // unlink-vs-reopen race a removal would introduce.
        let _ = self.file.unlock();
    }
}

/// The result of trying to lock a project.
pub enum LockOutcome {
    /// We hold the lock (or locking isn't supported on this
    /// filesystem and we proceeded permissively). Keep the value
    /// alive for the session.
    Acquired(ProjectLock),
    /// Another live instance holds it; `LockInfo` describes it for the
    /// warning. We did NOT acquire — the caller decides whether to
    /// proceed anyway.
    Busy(LockInfo),
}

/// Try to acquire the advisory lock for `project_root`.
///
/// Never blocks. Returns `Acquired` when we take the lock; `Busy` when
/// a live instance holds it. A filesystem that doesn't support
/// advisory locks (rare — some network mounts) degrades to `Acquired`
/// without a real lock rather than locking the author out.
pub fn acquire(project_root: &Path) -> io::Result<LockOutcome> {
    let path = project_root.join(".inkhaven.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&path)?;

    match file.try_lock_exclusive() {
        Ok(()) => {
            // We hold it — stamp our identity for anyone who comes next.
            write_info(&file, &LockInfo::current());
            Ok(LockOutcome::Acquired(ProjectLock { file, path }))
        }
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
            let info = std::fs::read_to_string(&path)
                .map(|s| LockInfo::parse(&s))
                .unwrap_or_default();
            Ok(LockOutcome::Busy(info))
        }
        Err(_) => {
            // Locking unsupported here. Permissive: proceed unlocked
            // rather than refuse to open the project.
            Ok(LockOutcome::Acquired(ProjectLock { file, path }))
        }
    }
}

/// Truncate + rewrite the lockfile with the current holder's identity.
/// Best-effort: a failure here only degrades the warning message, not
/// the lock itself.
fn write_info(mut file: &File, info: &LockInfo) {
    let _ = file.set_len(0);
    let _ = file.seek(SeekFrom::Start(0));
    let _ = file.write_all(info.to_line().as_bytes());
    let _ = file.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_then_contend_then_release() {
        let dir = tempfile::tempdir().unwrap();
        let first = acquire(dir.path()).unwrap();
        assert!(matches!(first, LockOutcome::Acquired(_)));

        // A second attempt while the first is held must report Busy
        // with the holder's PID — never block, never panic.
        match acquire(dir.path()).unwrap() {
            LockOutcome::Busy(info) => {
                assert_eq!(info.pid, std::process::id());
            }
            LockOutcome::Acquired(_) => panic!("expected Busy while first lock is held"),
        }

        // Releasing the first lets the next attempt succeed.
        drop(first);
        assert!(matches!(acquire(dir.path()).unwrap(), LockOutcome::Acquired(_)));
    }

    #[test]
    fn lock_info_round_trips() {
        let info = LockInfo { pid: 4321, host: "mac".into(), started_at: 1_700_000_000 };
        let parsed = LockInfo::parse(&info.to_line());
        assert_eq!(parsed.pid, 4321);
        assert_eq!(parsed.host, "mac");
        assert_eq!(parsed.started_at, 1_700_000_000);
        assert!(parsed.describe().contains("PID 4321"));
        assert!(parsed.describe().contains("on mac"));
    }

    #[test]
    fn missing_host_describes_cleanly() {
        let info = LockInfo { pid: 7, host: String::new(), started_at: 1_700_000_000 };
        let d = info.describe();
        assert!(d.contains("PID 7"));
        assert!(!d.contains(" on "));
    }
}
