//! Dirty-buffer rescue flush.
//!
//! When the panic hook fires, every dirty editor
//! buffer needs its in-memory contents preserved
//! somewhere durable so the user can recover them on
//! restart.  We can't reach back into the panicking
//! App to ask "what does the textarea look like right
//! now" — the call stack is unwinding — so the App
//! eagerly mirrors every dirty buffer into the
//! process-wide [`super::CrashContext`] on every
//! transition.  The mirror is what gets written here.
//!
//! Rescue files live next to the original paragraph
//! file as `<original>.inkhaven-rescue`.  The
//! companion-file naming means:
//!
//!   * a stale rescue file is obviously associated
//!     with its paragraph,
//!   * the recover CLI can find rescues by walking
//!     the project tree without needing a separate
//!     index,
//!   * if the user is fastidious about gitignore,
//!     `*.inkhaven-rescue` is one line.
//!
//! Write is atomic (temp + fsync + rename) via the
//! same [`super::write_atomic`] helper as the crash
//! report itself.  Failures don't abort the rescue —
//! the per-file [`RescueOutcome`] records what
//! happened so the user can read it in the crash
//! report.

use serde::{Deserialize, Serialize};

/// Snapshot of an open editor buffer's mutable state.
/// Stored in the [`super::CrashContext`] keyed by
/// paragraph-file relative path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirtyMirror {
    /// Full buffer body as the user has it RIGHT now.
    /// Stored as UTF-8 text; binary leaves shouldn't
    /// reach the editor surface.
    pub content: String,
    /// Cursor row + column (0-indexed) at the moment
    /// of mirror.  Lets the recover CLI restore
    /// cursor position after applying the rescue.
    pub cursor_row: usize,
    pub cursor_col: usize,
    /// Wall-clock of when this mirror was captured.
    /// HJSON-friendly ISO 8601.
    pub captured_at: String,
}

impl DirtyMirror {
    pub fn new(content: String, cursor_row: usize, cursor_col: usize) -> Self {
        Self {
            content,
            cursor_row,
            cursor_col,
            captured_at: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string(),
        }
    }
}

/// What happened when we tried to rescue one buffer.
/// Embedded in the crash report so the user reading the
/// report can tell whether each unsaved buffer made it
/// to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RescueOutcome {
    /// Paragraph rel-path used as the dict key.
    pub paragraph_rel_path: String,
    /// Absolute path of the rescue file on disk (where
    /// the bytes were written).  Present even on
    /// failure — tells the user where to LOOK.
    pub rescue_path: String,
    /// Size of the rescued buffer in bytes (UTF-8
    /// encoded).
    pub bytes: usize,
    pub cursor_row: usize,
    pub cursor_col: usize,
    /// Wall-clock from the mirror.
    pub mirror_captured_at: String,
    /// `None` on success.  `Some(message)` on failure
    /// (e.g. disk full, permission denied).
    pub error: Option<String>,
}

/// Walk every mirror in the context snapshot and write
/// it to disk.  `project_path` is used to resolve
/// rel-paths to absolute paths; when `None` (no
/// project open at panic time), we can't write
/// rescues and return an empty vec.
pub fn flush_dirty_buffers(
    project_path: Option<&std::path::Path>,
    mirrors: &std::collections::HashMap<String, DirtyMirror>,
) -> Vec<RescueOutcome> {
    let Some(project) = project_path else {
        return Vec::new();
    };
    let mut outcomes = Vec::with_capacity(mirrors.len());
    for (rel_path, mirror) in mirrors {
        let abs_path = project.join(rel_path);
        let rescue_path = rescue_path_for(&abs_path);
        let bytes = mirror.content.len();
        let err = super::write_atomic(&rescue_path, mirror.content.as_bytes())
            .err()
            .map(|e| e.to_string());
        outcomes.push(RescueOutcome {
            paragraph_rel_path: rel_path.clone(),
            rescue_path: rescue_path.display().to_string(),
            bytes,
            cursor_row: mirror.cursor_row,
            cursor_col: mirror.cursor_col,
            mirror_captured_at: mirror.captured_at.clone(),
            error: err,
        });
    }
    outcomes
}

/// Companion-file path for a paragraph file.
/// `<path>.typ` → `<path>.typ.inkhaven-rescue`.
pub(crate) fn rescue_path_for(paragraph_path: &std::path::Path) -> std::path::PathBuf {
    let mut s = paragraph_path.as_os_str().to_os_string();
    s.push(".inkhaven-rescue");
    std::path::PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rescue_path_appends_extension() {
        let p = std::path::Path::new("/tmp/proj/foo/bar.typ");
        let rp = rescue_path_for(p);
        assert_eq!(
            rp.to_string_lossy(),
            "/tmp/proj/foo/bar.typ.inkhaven-rescue"
        );
    }

    #[test]
    fn flush_with_no_project_returns_empty() {
        let mut mirrors = std::collections::HashMap::new();
        mirrors.insert(
            "x.typ".into(),
            DirtyMirror::new("hi".into(), 0, 0),
        );
        let outcomes = flush_dirty_buffers(None, &mirrors);
        assert!(outcomes.is_empty());
    }

    #[test]
    fn flush_with_project_writes_rescue_files() {
        let dir = std::env::temp_dir().join(format!(
            "inkhaven-rescue-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("book/ch1")).unwrap();
        // Original file exists (recover-CLI side-effect-free; the
        // rescue write doesn't require it, but the layout matches
        // what the App would have).
        let original = dir.join("book/ch1/para.typ");
        std::fs::write(&original, "original body").unwrap();

        let mut mirrors = std::collections::HashMap::new();
        mirrors.insert(
            "book/ch1/para.typ".into(),
            DirtyMirror::new("dirty body in memory".into(), 2, 4),
        );
        let outcomes = flush_dirty_buffers(Some(&dir), &mirrors);
        assert_eq!(outcomes.len(), 1);
        let o = &outcomes[0];
        assert!(o.error.is_none(), "unexpected error: {:?}", o.error);
        assert_eq!(o.bytes, "dirty body in memory".len());
        assert_eq!(o.cursor_row, 2);
        assert_eq!(o.cursor_col, 4);

        let rescue = dir.join("book/ch1/para.typ.inkhaven-rescue");
        assert!(rescue.exists());
        let body = std::fs::read_to_string(&rescue).unwrap();
        assert_eq!(body, "dirty body in memory");

        // Original file must be untouched.
        assert_eq!(std::fs::read_to_string(&original).unwrap(), "original body");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn flush_records_per_file_errors_without_aborting() {
        // Point at a project directory that exists, but
        // a rel-path whose parent doesn't.  Should
        // surface as an error in the outcome, not a
        // panic, and the OTHER mirror should still
        // succeed.
        let dir = std::env::temp_dir().join(format!(
            "inkhaven-rescue-err-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("ok")).unwrap();

        let mut mirrors = std::collections::HashMap::new();
        mirrors.insert(
            "ok/good.typ".into(),
            DirtyMirror::new("ok body".into(), 0, 0),
        );
        mirrors.insert(
            "nonexistent/missing-parent/bad.typ".into(),
            DirtyMirror::new("bad body".into(), 0, 0),
        );

        let outcomes = flush_dirty_buffers(Some(&dir), &mirrors);
        assert_eq!(outcomes.len(), 2);
        let ok = outcomes
            .iter()
            .find(|o| o.paragraph_rel_path == "ok/good.typ")
            .unwrap();
        let bad = outcomes
            .iter()
            .find(|o| o.paragraph_rel_path == "nonexistent/missing-parent/bad.typ")
            .unwrap();
        assert!(ok.error.is_none());
        assert!(bad.error.is_some(), "expected an error for missing parent");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
