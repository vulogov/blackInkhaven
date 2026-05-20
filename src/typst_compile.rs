//! Thin wrapper around `typst compile`. The TUI's Ctrl+B B / Ctrl+B O
//! procedures run this after Book assembly to turn the synthesised
//! `<artefacts>/<book-slug>/<book-slug>.typ` into a PDF.
//!
//! Implementation is intentionally minimal: spawn the `typst` binary
//! as a child process and capture stdout / stderr / exit status. The
//! TUI handles the splash + spinner around the call by polling
//! `Child::try_wait`; this module just exposes the building blocks
//! and a convenience blocking `compile` that waits internally.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::error::{Error, Result};

/// What `typst compile` produced, after the child process finished.
#[derive(Debug)]
pub struct CompileOutcome {
    pub success: bool,
    pub stderr: String,
    pub stdout: String,
    /// Where `typst compile` writes the PDF — same path as the input
    /// `.typ`, with the extension swapped. Captured even on failure
    /// so callers can `take` a known location if a previous run
    /// produced a stale PDF (we explicitly wipe before compile so
    /// this only matters for the success path).
    pub pdf_path: PathBuf,
}

/// Spawn `typst compile <typ_path>` and return the child handle along
/// with the expected PDF output path. Caller is expected to poll
/// `child.try_wait()` (with `wait_with_output` at the end) so the TUI
/// can keep the splash animated.
pub fn spawn(typ_path: &Path) -> Result<(Child, PathBuf)> {
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
                    "`typst` not found in PATH — install Typst from typst.app/docs/install/"
                        .into(),
                )
            } else {
                Error::Store(format!("spawn `typst compile`: {e}"))
            }
        })?;
    Ok((child, pdf_path))
}

/// Wait for an already-spawned `typst compile` child and collect its
/// streams + exit status into a `CompileOutcome`. Pairs with `spawn`.
pub fn finish(child: Child, pdf_path: PathBuf) -> Result<CompileOutcome> {
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
