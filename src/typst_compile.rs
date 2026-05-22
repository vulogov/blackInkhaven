//! Thin wrapper around the typst compiler. The TUI's Ctrl+B B / Ctrl+B O
//! procedures run this after Book assembly to turn the synthesised
//! `<artefacts>/<book-slug>/<book-slug>.typ` into a PDF.
//!
//! Two engines live behind one shape:
//!
//! * **External** (default): spawn the host's `typst` binary as a
//!   child process. Original behaviour from 1.2.3 — preserved here
//!   as the default path.
//! * **In-process** (1.2.5+, opt-in via `typst_compile.engine =
//!   "inprocess"` in HJSON): run `typst::compile::<PagedDocument>()`
//!   on a worker thread and emit the PDF via `typst-pdf`. See
//!   `crate::typst_inprocess`.
//!
//! The TUI never cares which engine ran — it spawns, polls, and
//! finishes through the `CompileHandle` abstraction below. Routing
//! is decided once in `spawn_with_config` based on the user's HJSON
//! setting (and a runtime gate that today always picks `external`
//! when the in-process path isn't selected).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::typst_inprocess::{spawn_thread, InprocessHandle};

/// Locate the host's `typst` binary on `PATH`. Returns `None` when
/// the binary is missing — the external engine spawn will then
/// produce a clean error pointing at the install docs (or at the
/// in-process engine knob). Shared with `cli/export.rs` so both
/// paths report the same path.
pub fn typst_external_path() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join("typst");
        if candidate.is_file() {
            return Some(candidate);
        }
        let with_ext = dir.join("typst.exe");
        if with_ext.is_file() {
            return Some(with_ext);
        }
    }
    None
}

/// One-line summary of which engine is active, suitable for the
/// compile splash, the credits pane, or a status-bar message.
/// Always includes a concrete path (or `"not found on PATH"`) for
/// the external engine so the user can see exactly what would run.
pub fn engine_summary(cfg: &Config) -> String {
    if cfg.typst_compile.use_inprocess_engine() {
        let bundle = cfg.typst_compile.bundle_fonts;
        let system = cfg.typst_compile.use_system_fonts;
        let pkgs = cfg.typst_compile.packages_enabled;
        let font_label = match (bundle, system) {
            (true, true) => "bundled + system",
            (true, false) => "bundled",
            (false, true) => "system",
            (false, false) => "NO FONTS",
        };
        format!(
            "internal · fonts: {font_label} · @preview: {}",
            if pkgs { "on" } else { "off" }
        )
    } else {
        match typst_external_path() {
            Some(p) => format!("external · {}", p.display()),
            None => "external · `typst` NOT FOUND on PATH".to_owned(),
        }
    }
}

/// What the compiler produced, regardless of engine.
#[derive(Debug)]
pub struct CompileOutcome {
    pub success: bool,
    pub stderr: String,
    pub stdout: String,
    /// Where the PDF lands — same path as the input `.typ` with the
    /// extension swapped. Captured even on failure so callers can
    /// `take` a known location if a prior run produced a stale PDF
    /// (we explicitly wipe before compile so this only matters on
    /// the success path).
    pub pdf_path: PathBuf,
}

/// One-of-two compile handles: a `std::process::Child` for the
/// external engine, or an in-process worker-thread handle. Drives
/// the spinner loop in the TUI through the same `try_wait` /
/// `finish` shape.
pub enum CompileHandle {
    External { child: Child, pdf_path: PathBuf },
    Inprocess(InprocessHandle),
}

impl CompileHandle {
    /// Non-blocking poll. `Ok(Some(()))` means "done — call
    /// `finish`"; `Ok(None)` means "still running".
    pub fn try_wait(&mut self) -> std::io::Result<Option<()>> {
        match self {
            Self::External { child, .. } => child.try_wait().map(|opt| opt.map(|_| ())),
            Self::Inprocess(h) => h.try_wait_mut(),
        }
    }

    /// User-requested cancellation. The TUI's spinner loop fires
    /// this when the user presses Esc while a compile is in
    /// flight.
    ///
    /// * **External**: send SIGTERM (via `Child::kill`) and reap.
    /// * **In-process**: drop the receiver so `into_outcome`
    ///   short-circuits to a "cancelled" outcome. The worker
    ///   thread keeps running until typst finishes naturally —
    ///   typst is deterministic and bounded, so the worst case
    ///   is a few seconds of CPU after the user gave up.
    ///
    /// Either way the caller should consume the handle with
    /// `finish` after `kill` to recover a definite outcome.
    pub fn kill(&mut self) {
        match self {
            Self::External { child, .. } => {
                let _ = child.kill();
            }
            Self::Inprocess(h) => {
                h.cancel();
            }
        }
    }
}

/// Engine-aware spawn. Reads `cfg.typst_compile.use_inprocess_engine()`
/// to pick between the external child-process path (default) and the
/// new in-process worker-thread path. The HJSON setting that flips
/// engines is `typst_compile.engine = "inprocess"`; that gate also
/// emits a startup warning when the engine is unavailable in this
/// build (today: never — see `use_inprocess_engine`).
pub fn spawn_with_config(cfg: &Config, typ_path: &Path) -> Result<CompileHandle> {
    if cfg.typst_compile.use_inprocess_engine() {
        // The project root is the directory that contains the
        // assembled `<book>.typ`'s parent grandparent — actually
        // the simplest convention is "use the .typ's parent as
        // the World root". Book assembly puts every file the
        // typst compile needs under `<artefacts>/<book>/`, so
        // anchoring there keeps relative imports resolvable.
        let root = typ_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let settings =
            crate::typst_world::WorldSettings::from_cfg(&cfg.typst_compile);
        let handle = spawn_thread(&root, typ_path, settings)?;
        return Ok(CompileHandle::Inprocess(handle));
    }
    spawn_external(typ_path)
}

fn spawn_external(typ_path: &Path) -> Result<CompileHandle> {
    let pdf_path = typ_path.with_extension("pdf");
    let child = Command::new("typst")
        .arg("compile")
        .arg(typ_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::Store(
                    "`typst` not found in PATH — install Typst from typst.app/docs/install/ \
                     or set `typst_compile.engine = \"inprocess\"` in inkhaven.hjson"
                        .into(),
                )
            } else {
                Error::Store(format!("spawn `typst compile`: {e}"))
            }
        })?;
    Ok(CompileHandle::External { child, pdf_path })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn cfg_with(engine: &str) -> Config {
        let mut cfg = Config::default();
        cfg.typst_compile.engine = engine.to_owned();
        cfg
    }

    #[test]
    fn engine_summary_internal_default_flags() {
        let cfg = cfg_with("inprocess");
        let s = engine_summary(&cfg);
        assert!(s.starts_with("internal"), "got: {s}");
        assert!(s.contains("bundled + system"), "got: {s}");
        assert!(s.contains("@preview: on"), "got: {s}");
    }

    #[test]
    fn engine_summary_internal_hermetic() {
        let mut cfg = cfg_with("inprocess");
        cfg.typst_compile.use_system_fonts = false;
        cfg.typst_compile.packages_enabled = false;
        let s = engine_summary(&cfg);
        assert!(s.contains("fonts: bundled"), "got: {s}");
        assert!(s.contains("@preview: off"), "got: {s}");
    }

    #[test]
    fn engine_summary_external_reports_path_or_missing() {
        let cfg = cfg_with("external");
        let s = engine_summary(&cfg);
        assert!(s.starts_with("external"), "got: {s}");
        // Either a path or the missing-PATH message — both are
        // explicit; we just want one of the two shapes.
        assert!(
            s.contains("/") || s.contains("NOT FOUND"),
            "expected a concrete path or NOT FOUND marker, got: {s}",
        );
    }
}

/// Consume the handle, recover the final outcome, and return it.
/// Pairs with `spawn` / `spawn_with_config`.
pub fn finish(handle: CompileHandle) -> Result<CompileOutcome> {
    match handle {
        CompileHandle::External { child, pdf_path } => {
            let output = child
                .wait_with_output()
                .map_err(|e| Error::Store(format!("wait_with_output on `typst compile`: {e}")))?;
            Ok(CompileOutcome {
                success: output.status.success(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                pdf_path,
            })
        }
        CompileHandle::Inprocess(h) => Ok(h.into_outcome()),
    }
}
