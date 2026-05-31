//! 1.2.15+ Phase R.1 — crash report writer and panic hook.
//!
//! Survivability layer: when something panics anywhere
//! in inkhaven, this module's hook
//!
//!   1. attempts to flush every dirty editor buffer
//!      to a `<path>.inkhaven-rescue` companion file
//!      via [`rescue::flush_dirty_buffers`],
//!   2. captures the panic context + project state
//!      + recent action ring + environment fingerprint
//!      into a [`CrashReport`],
//!   3. serialises the report to HJSON and writes it
//!      atomically to `inkhaven-crash-<ts>.hjson` in
//!      the current working directory,
//!   4. restores the terminal (best effort) so the
//!      user's shell isn't stuck in raw-mode +
//!      alternate-screen,
//!   5. prints a one-line breadcrumb to stderr telling
//!      the user where the report landed.
//!
//! Every step is wrapped in `let _ = …` so a failure in
//! one step (e.g. disk full while writing the report)
//! doesn't prevent the others from trying.  The hook is
//! best-effort — the user must always end up with a
//! restored terminal even when nothing else worked.
//!
//! The hook reads from a process-wide
//! [`CrashContext`] singleton that the rest of the app
//! updates on meaningful state transitions (project
//! open, paragraph open/close, save, action dispatch,
//! buffer mutation that flips dirty).  The hook itself
//! never touches App state directly — that would
//! deadlock on the lock the panicking thread is
//! holding.

pub mod actions;
pub mod rescue;
pub mod report;

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub use actions::{ActionRecord, ActionRing};
// `RescueOutcome` is consumed by the recover CLI in
// R.2; the re-export keeps the public API together.
#[allow(unused_imports)]
pub use rescue::{DirtyMirror, RescueOutcome};
pub use report::CrashReport;

/// Maximum size of the recent-action ring.  Each entry
/// is ~80 bytes typical, so 50 caps at ~4 KB — small
/// enough to keep around forever, large enough to
/// reconstruct what the user was doing when the panic
/// fired.
pub const ACTION_RING_CAP: usize = 50;

/// Process-wide state that the panic hook reads at
/// crash time.  Updated by the App on every meaningful
/// state transition.  The hook itself never mutates
/// this — it only snapshots.
pub struct CrashContext {
    inner: Mutex<CrashState>,
}

#[derive(Default, Debug, Clone)]
pub struct CrashState {
    pub project_path: Option<PathBuf>,
    pub open_book: Option<String>,
    pub open_paragraph: Option<String>,
    pub open_paragraph_rel_path: Option<String>,
    pub actions: ActionRing,
    /// Keyed by the paragraph's relative path under the
    /// project root.  The mirror is the
    /// last-known-good buffer state — content + cursor.
    /// Cleared on save (which means "this buffer is
    /// no longer dirty, no rescue needed").
    pub dirty_buffers: std::collections::HashMap<String, DirtyMirror>,
}

static CONTEXT: OnceLock<CrashContext> = OnceLock::new();

/// Access the process-wide crash context.  Initialises
/// on first call.
pub fn context() -> &'static CrashContext {
    CONTEXT.get_or_init(|| CrashContext {
        inner: Mutex::new(CrashState::default()),
    })
}

impl CrashContext {
    /// Update the project path.  Called once per TUI
    /// session, just after the project store opens.
    pub fn set_project(&self, path: PathBuf) {
        if let Ok(mut s) = self.inner.lock() {
            s.project_path = Some(path);
        }
    }

    /// Update the open-paragraph triple.  Called on
    /// every `load_paragraph` / `close_paragraph`.
    pub fn set_open_paragraph(
        &self,
        book: Option<String>,
        paragraph: Option<String>,
        rel_path: Option<String>,
    ) {
        if let Ok(mut s) = self.inner.lock() {
            s.open_book = book;
            s.open_paragraph = paragraph;
            s.open_paragraph_rel_path = rel_path;
        }
    }

    /// Push an action to the ring.  Capped at
    /// [`ACTION_RING_CAP`]; oldest entries drop off the
    /// front.
    pub fn push_action(&self, action: ActionRecord) {
        if let Ok(mut s) = self.inner.lock() {
            s.actions.push(action);
        }
    }

    /// Mirror a dirty buffer.  Called when a buffer
    /// transitions dirty → … (every keystroke is too
    /// noisy; callers debounce).  `rel_path` is keyed
    /// by the paragraph file's path relative to the
    /// project root.
    pub fn mirror_buffer(&self, rel_path: String, mirror: DirtyMirror) {
        if let Ok(mut s) = self.inner.lock() {
            s.dirty_buffers.insert(rel_path, mirror);
        }
    }

    /// Clear a buffer's mirror.  Called on save —
    /// "this paragraph no longer needs rescue".
    pub fn clear_mirror(&self, rel_path: &str) {
        if let Ok(mut s) = self.inner.lock() {
            s.dirty_buffers.remove(rel_path);
        }
    }

    /// Read-only snapshot for the panic hook.  Returns
    /// a clone so the hook can release the lock
    /// immediately.
    pub fn snapshot(&self) -> Option<CrashState> {
        self.inner.lock().ok().map(|s| s.clone())
    }
}

type TerminalRestore = Box<dyn Fn() + Send + Sync + 'static>;

static TERMINAL_RESTORE: OnceLock<Mutex<Option<TerminalRestore>>> = OnceLock::new();

fn terminal_restore_slot() -> &'static Mutex<Option<TerminalRestore>> {
    TERMINAL_RESTORE.get_or_init(|| Mutex::new(None))
}

/// Register a closure the panic hook should run before
/// writing the report — usually `disable_raw_mode` +
/// `LeaveAlternateScreen`.  Called by `tui::app::run`
/// just after switching the terminal into raw mode,
/// and called again with `None` on graceful TUI exit.
///
/// Stored in a process-wide slot so the hook can find
/// it without owning anything App-specific.
pub fn set_terminal_restore(restore: Option<TerminalRestore>) {
    if let Ok(mut slot) = terminal_restore_slot().lock() {
        *slot = restore;
    }
}

/// Install the crash-report panic hook.  Call exactly
/// once in `main()` before any code that might panic.
/// The previous hook is captured and chained — so the
/// default backtrace printer still runs after our
/// report writer.
///
/// Side effects: the hook will, on panic,
///   - call the terminal-restore closure registered
///     via [`set_terminal_restore`] (if any) to undo
///     raw-mode + alternate-screen,
///   - read the [`CrashContext`] snapshot,
///   - flush dirty buffers as `.inkhaven-rescue`
///     companions of the original files,
///   - write `inkhaven-crash-<ts>.hjson` to cwd,
///   - print a breadcrumb to stderr,
///   - chain to the previously-installed hook
///     (the default Rust hook prints the panic + an
///     optional backtrace).
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Step 1 — restore the terminal so anything we
        // print is actually visible.  Must come first
        // because the rest of the steps may take a
        // beat.
        if let Ok(slot) = terminal_restore_slot().lock() {
            if let Some(restore) = slot.as_ref() {
                restore();
            }
        }

        // Step 2 — best-effort snapshot.  If the
        // mutex is poisoned (likely — we ARE in a
        // panic), we get an empty state and continue.
        let state = context().snapshot().unwrap_or_default();

        // Step 3 — flush dirty buffers.
        let rescue_outcomes =
            rescue::flush_dirty_buffers(state.project_path.as_deref(), &state.dirty_buffers);

        // Step 4 — build the report.
        let report = CrashReport::capture(info, &state, &rescue_outcomes);

        // Step 5 — write atomically.
        let report_path = report_target_path();
        let write_result = report.write_atomic(&report_path);

        // Step 6 — breadcrumb.
        match write_result {
            Ok(()) => {
                eprintln!(
                    "\ninkhaven crashed — crash report written to {}",
                    report_path.display()
                );
                if !rescue_outcomes.is_empty() {
                    eprintln!(
                        "  {} unsaved buffer(s) rescued.  Run `inkhaven recover {}` to restore.",
                        rescue_outcomes.len(),
                        report_path.display()
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "\ninkhaven crashed — could not write crash report ({e})",
                );
            }
        }

        // Step 7 — chain.  This is what prints the
        // panic message + backtrace.
        previous(info);
    }));
}

/// Compute the absolute path to write the crash report.
///
/// Tries `std::env::current_dir()` first; on failure
/// (very rare — typically only when the cwd was
/// deleted out from under the process), falls back to
/// `std::env::temp_dir()`.  Filename is
/// `inkhaven-crash-<UTC ISO8601 compact>.hjson`.
fn report_target_path() -> PathBuf {
    let stem = format!(
        "inkhaven-crash-{}.hjson",
        chrono::Utc::now().format("%Y%m%dT%H%M%S"),
    );
    std::env::current_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join(stem)
}

/// Atomic write helper used by both the report writer
/// and the rescue flush.  Writes to `<target>.tmp`,
/// fsyncs the file handle, renames into place, then
/// best-effort-fsyncs the parent directory on Unix.
///
/// Failures at any step are returned to the caller —
/// the panic hook uses `let _ = …` so the higher-level
/// best-effort guarantee still holds.
pub(crate) fn write_atomic(target: &std::path::Path, body: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let parent = target.parent().unwrap_or(std::path::Path::new("."));
    let tmp_name = match target.file_name() {
        Some(name) => {
            let mut s = name.to_os_string();
            s.push(".tmp");
            s
        }
        None => return Err(std::io::Error::other("crash: target has no file_name")),
    };
    let tmp = parent.join(tmp_name);

    // Write + fsync the file.
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&tmp)?;
    f.write_all(body)?;
    f.sync_all()?;
    drop(f);

    // Rename into place.
    std::fs::rename(&tmp, target)?;

    // Best-effort fsync the parent dir on Unix.  Windows
    // doesn't support opening a directory as a file.
    #[cfg(unix)]
    {
        if let Ok(d) = std::fs::File::open(parent) {
            let _ = d.sync_all();
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_starts_empty() {
        // First call initialises; further calls return
        // the same instance.  Don't assert on state
        // because the test process is shared.
        let c = context();
        let _ = c.snapshot();
    }

    #[test]
    fn set_project_persists_in_snapshot() {
        let c = context();
        c.set_project(PathBuf::from("/tmp/inkhaven-test-project"));
        let snap = c.snapshot().expect("snapshot succeeds");
        assert_eq!(
            snap.project_path.as_deref(),
            Some(std::path::Path::new("/tmp/inkhaven-test-project"))
        );
    }

    #[test]
    fn write_atomic_creates_target_and_removes_tmp() {
        let tmp_dir = std::env::temp_dir().join(format!(
            "inkhaven-crash-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&tmp_dir).unwrap();
        let target = tmp_dir.join("hello.txt");
        super::write_atomic(&target, b"hello world\n").expect("atomic write succeeds");
        assert!(target.exists(), "target file should exist");
        assert!(
            !target.with_extension("txt.tmp").exists(),
            "tmp file should have been renamed away"
        );
        let body = std::fs::read_to_string(&target).unwrap();
        assert_eq!(body, "hello world\n");
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn target_path_has_inkhaven_crash_prefix() {
        let p = super::report_target_path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            name.starts_with("inkhaven-crash-"),
            "name = {name}"
        );
        assert!(name.ends_with(".hjson"), "name = {name}");
    }
}
