//! In-process Typst compile path (1.2.5+).
//!
//! Mirrors the `spawn` / `finish` shape of `crate::typst_compile` so
//! the TUI's spinner loop can drive both engines through the same
//! abstraction. The two differences from the external child-process
//! path:
//!
//! * Errors / warnings are first-class `SourceDiagnostic`s with span
//!   info, not opaque stderr blobs. We format them into a stderr-shaped
//!   string for compat with the existing `start_typst_error_analysis`
//!   handler, but a future commit can lift the raw diagnostic list up
//!   to the editor for in-line markers.
//! * The whole compile runs on a worker thread; the foreground TUI
//!   thread polls a channel for completion the same way it currently
//!   polls `Child::try_wait()`.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use typst::diag::{Severity, SourceDiagnostic, Warned};
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Span};
use typst::World;
use typst_pdf::PdfOptions;

use crate::error::{Error, Result};
use crate::typst_compile::CompileOutcome;
use crate::typst_world::{InkhavenWorld, WorldSettings};

/// Handle to an in-flight in-process compile. Modelled on
/// `std::process::Child` so the TUI's spinner loop can use the
/// same poll-until-done shape for both engines.
pub struct InprocessHandle {
    rx: mpsc::Receiver<CompileOutcome>,
    pdf_path: PathBuf,
    /// Once `try_wait_mut` has drained the channel it stashes the
    /// outcome here so `into_outcome` can recover it without
    /// blocking on a second `recv`.
    stash: Option<CompileOutcome>,
    /// User-requested cancel flag. `cancel()` flips it; the
    /// `into_outcome` path short-circuits with a synthetic
    /// "cancelled" outcome instead of blocking on the receiver.
    cancelled: bool,
}

impl InprocessHandle {
    /// Non-blocking poll. `Ok(Some(()))` means "the worker has
    /// finished — call `into_outcome` to recover the result";
    /// `Ok(None)` means "still running". Mirrors `Child::try_wait`
    /// so the spinner loop doesn't need to branch on engine.
    pub fn try_wait_mut(&mut self) -> std::io::Result<Option<()>> {
        if self.stash.is_some() {
            return Ok(Some(()));
        }
        match self.rx.try_recv() {
            Ok(outcome) => {
                self.stash = Some(outcome);
                Ok(Some(()))
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                // Worker panicked / disconnected without sending.
                // Synthesise a generic failure so the foreground
                // thread still gets a clean outcome.
                self.stash = Some(CompileOutcome {
                    success: false,
                    stderr: "in-process compile: worker thread disconnected".to_owned(),
                    stdout: String::new(),
                    pdf_path: self.pdf_path.clone(),
                });
                Ok(Some(()))
            }
        }
    }

    /// Consume the handle and return the compile outcome. If the
    /// caller never polled `try_wait_mut` we block here as a
    /// safety net — the foreground thread shouldn't return while
    /// the worker is still alive (unless we cancelled).
    pub fn into_outcome(mut self) -> CompileOutcome {
        if let Some(out) = self.stash.take() {
            return out;
        }
        if self.cancelled {
            return CompileOutcome {
                success: false,
                stderr: "in-process compile: cancelled by user — worker thread \
                         abandoned, may continue using CPU until typst finishes \
                         on its own"
                    .to_owned(),
                stdout: String::new(),
                pdf_path: self.pdf_path,
            };
        }
        match self.rx.recv() {
            Ok(out) => out,
            Err(_) => CompileOutcome {
                success: false,
                stderr: "in-process compile: worker thread disconnected".to_owned(),
                stdout: String::new(),
                pdf_path: self.pdf_path,
            },
        }
    }

    /// Cooperative cancel — flips the handle into "abandoned"
    /// state. The worker thread keeps running because
    /// `typst::compile` has no interrupt point, but the TUI
    /// stops blocking on it. Subsequent `into_outcome` returns
    /// a synthesized cancellation outcome.
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
}

/// Spawn a worker thread that compiles `main_typ` rooted at
/// `project_root` into a PDF on disk. Returns a poll-able handle.
pub fn spawn_thread(
    project_root: &Path,
    main_typ: &Path,
    settings: WorldSettings,
) -> Result<InprocessHandle> {
    let pdf_path = main_typ.with_extension("pdf");
    let project_root = project_root.to_path_buf();
    let main_typ = main_typ.to_path_buf();
    let pdf_path_for_thread = pdf_path.clone();
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name("inkhaven-typst-compile".into())
        .spawn(move || {
            let outcome = compile_to_pdf(
                &project_root,
                &main_typ,
                &pdf_path_for_thread,
                settings,
            );
            let _ = tx.send(outcome);
        })
        .map_err(|e| Error::Store(format!("spawn typst worker thread: {e}")))?;
    Ok(InprocessHandle {
        rx,
        pdf_path,
        stash: None,
        cancelled: false,
    })
}

/// The synchronous compile core — runs on the worker thread.
fn compile_to_pdf(
    project_root: &Path,
    main_typ: &Path,
    pdf_path: &Path,
    settings: WorldSettings,
) -> CompileOutcome {
    let world = match InkhavenWorld::new(project_root, main_typ, settings) {
        Ok(w) => w,
        Err(e) => {
            return CompileOutcome {
                success: false,
                stderr: format!("in-process compile: {e}"),
                stdout: String::new(),
                pdf_path: pdf_path.to_path_buf(),
            };
        }
    };
    let Warned { output, warnings } = typst::compile::<PagedDocument>(&world);
    let document = match output {
        Ok(doc) => doc,
        Err(errors) => {
            return CompileOutcome {
                success: false,
                stderr: format_diagnostics(&world, &errors),
                stdout: format_diagnostics(&world, &warnings),
                pdf_path: pdf_path.to_path_buf(),
            };
        }
    };
    // typst-pdf turns the laid-out document into a `Vec<u8>`.
    // PdfOptions::default() = standard defaults, same as
    // `typst compile` with no flags.
    let options = PdfOptions::default();
    let bytes = match typst_pdf::pdf(&document, &options) {
        Ok(b) => b,
        Err(errors) => {
            return CompileOutcome {
                success: false,
                stderr: format_diagnostics(&world, &errors),
                stdout: format_diagnostics(&world, &warnings),
                pdf_path: pdf_path.to_path_buf(),
            };
        }
    };
    if let Err(e) = std::fs::write(pdf_path, &bytes) {
        return CompileOutcome {
            success: false,
            stderr: format!("write {}: {e}", pdf_path.display()),
            stdout: String::new(),
            pdf_path: pdf_path.to_path_buf(),
        };
    }
    CompileOutcome {
        success: true,
        stderr: String::new(),
        stdout: format_diagnostics(&world, &warnings),
        pdf_path: pdf_path.to_path_buf(),
    }
}

/// Render a list of `SourceDiagnostic`s into a multi-line string
/// that resembles `typst compile`'s stderr — `error: <message>`
/// followed by an indented `--> path:line:col` source pointer and
/// any `hint:` lines. Empty input → empty string.
fn format_diagnostics(world: &InkhavenWorld, diags: &[SourceDiagnostic]) -> String {
    if diags.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for d in diags {
        let label = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let (path, line, col) = locate(world, d.span);
        out.push_str(&format!("{label}: {}\n", d.message));
        out.push_str(&format!("  --> {path}:{line}:{col}\n"));
        for hint in &d.hints {
            out.push_str(&format!("  hint: {hint}\n"));
        }
    }
    out
}

fn locate(world: &InkhavenWorld, span: Span) -> (String, usize, usize) {
    let id = match span.id() {
        Some(id) => id,
        None => return ("<detached>".to_owned(), 0, 0),
    };
    let path_label = file_id_label(id);
    let source = match world.source(id) {
        Ok(s) => s,
        Err(_) => return (path_label, 0, 0),
    };
    let range = match source.range(span) {
        Some(r) => r,
        None => return (path_label, 0, 0),
    };
    let (line0, col0) = source
        .lines()
        .byte_to_line_column(range.start)
        .unwrap_or((0, 0));
    (path_label, line0 + 1, col0 + 1)
}

fn file_id_label(id: FileId) -> String {
    match id.package() {
        Some(pkg) => format!("{}/{}", pkg, id.vpath().as_rooted_path().display()),
        None => id.vpath().as_rooted_path().display().to_string(),
    }
}

/// 1.2.5+ semantic-diagnostics path: run `typst::compile` against
/// a synthesised single-paragraph document and surface its
/// diagnostics as `TypstDiagnostic`s (same shape the editor
/// already consumes from `crate::typst_check`).
///
/// The paragraph is compiled **in isolation** — no book preamble,
/// no `#show`/`#set` rules from anywhere else in the project. This
/// catches semantic errors (unknown function names, type errors,
/// missing fonts referenced by name) that `typst-syntax` can't see,
/// but it WILL report false positives for paragraphs that depend
/// on book-level definitions. The HJSON knob
/// `typst_compile.semantic_diagnostics` defaults to off for that
/// reason; users opt in when their manuscripts are mostly
/// self-contained.
///
/// Runs synchronously on the caller's thread — the editor only
/// calls this on idle / save, and a stand-alone paragraph compile
/// is fast (typically 20–200 ms once the World's font cache is
/// warm).
pub fn check_semantic(
    source: &str,
    settings: WorldSettings,
) -> Vec<crate::typst_check::TypstDiagnostic> {
    // No tempfile — the World short-circuits its `main` source to
    // the in-memory body. The "root" is the system temp dir; it
    // never gets read because there are no relative imports a
    // single-paragraph buffer can resolve (those that try will
    // surface their own `FileError::NotFound` diagnostics, which
    // we filter to in-paragraph spans below).
    let world = InkhavenWorld::in_memory(
        std::env::temp_dir(),
        source.to_owned(),
        settings,
    );
    let typst::diag::Warned { output, warnings } =
        typst::compile::<typst::layout::PagedDocument>(&world);
    let main_id = world.main();
    let mut diags = Vec::new();
    match output {
        Ok(_) => {
            for w in warnings {
                if let Some(d) = lift(&world, &w, main_id) {
                    diags.push(d);
                }
            }
        }
        Err(errors) => {
            for e in errors {
                if let Some(d) = lift(&world, &e, main_id) {
                    diags.push(d);
                }
            }
            // Include warnings too — they're often the actionable
            // signal even when the compile failed for a different
            // reason.
            for w in warnings {
                if let Some(d) = lift(&world, &w, main_id) {
                    diags.push(d);
                }
            }
        }
    }
    diags
}

/// Lift one `SourceDiagnostic` into the editor's `TypstDiagnostic`
/// shape. Returns `None` for diagnostics whose span doesn't point
/// inside the open paragraph (e.g. errors from imported stdlib
/// definitions); we only surface what the user can directly edit.
fn lift(
    world: &InkhavenWorld,
    diag: &typst::diag::SourceDiagnostic,
    main_id: FileId,
) -> Option<crate::typst_check::TypstDiagnostic> {
    let id = diag.span.id()?;
    if id != main_id {
        return None;
    }
    let source = world.source(id).ok()?;
    let range = source.range(diag.span)?;
    let (line0, col0) =
        source.lines().byte_to_line_column(range.start).unwrap_or((0, 0));
    let mut message = diag.message.to_string();
    if matches!(diag.severity, typst::diag::Severity::Warning) {
        message = format!("warning: {message}");
    }
    Some(crate::typst_check::TypstDiagnostic {
        line: line0 + 1,
        col: col0 + 1,
        byte_start: range.start,
        byte_end: range.end,
        message,
        hints: diag.hints.iter().map(|h| h.to_string()).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn write_tmp(content: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("probe.typ");
        std::fs::write(&path, content).expect("write");
        (dir, path)
    }

    /// End-to-end smoke: spawn the worker, poll to completion,
    /// recover the outcome, and confirm we got *something
    /// sensible* — either a non-empty PDF on disk OR a structured
    /// failure message. Marked `#[ignore]` because the test exercises
    /// system-font discovery which is genuinely environmental;
    /// CI boxes with zero fonts will surface a `font not found`
    /// diagnostic that the test should not flag as a regression.
    /// Run manually with `cargo test --ignored typst_inprocess`.
    #[test]
    #[ignore]
    fn end_to_end_compile_smoke() {
        let (dir, path) = write_tmp(
            "#set page(width: 10cm, height: 5cm, margin: 1cm)\n\
             = Hello\nProse.\n",
        );
        let mut handle = spawn_thread(
            dir.path(),
            &path,
            WorldSettings {
                bundle_fonts: true,
                use_system_fonts: true,
                packages_enabled: false,
            },
        )
        .expect("spawn");
        let started = std::time::Instant::now();
        loop {
            match handle.try_wait_mut().expect("try_wait") {
                Some(_) => break,
                None => std::thread::sleep(std::time::Duration::from_millis(50)),
            }
            assert!(
                started.elapsed().as_secs() < 30,
                "in-process compile hung",
            );
        }
        let outcome = handle.into_outcome();
        if outcome.success {
            let bytes = std::fs::metadata(&outcome.pdf_path)
                .expect("pdf written")
                .len();
            assert!(bytes > 100, "PDF suspiciously small: {bytes} bytes");
        } else {
            // Acceptable failure modes: no fonts on this box, or
            // some other diagnostic. Just confirm the error stream
            // is non-empty and well-structured.
            assert!(
                !outcome.stderr.is_empty(),
                "failed compile should populate stderr",
            );
            eprintln!("in-process compile failed (acceptable on bare hosts):\n{}", outcome.stderr);
        }
    }

    /// `check_semantic` against an obviously-broken paragraph
    /// (call to an undefined function) should produce at least
    /// one diagnostic anchored at the bad call. Marked `#[ignore]`
    /// because semantic checks need the font cache + library to
    /// be loadable — same environmental dependency as the
    /// end-to-end smoke.
    #[test]
    #[ignore]
    fn semantic_catches_undefined_function() {
        let source = "#this_function_does_not_exist()\n";
        let diags = check_semantic(
            source,
            WorldSettings {
                bundle_fonts: true,
                use_system_fonts: true,
                packages_enabled: false,
            },
        );
        assert!(
            !diags.is_empty(),
            "expected a semantic diagnostic for the undefined function",
        );
        // The first error should mention the bogus identifier.
        let first = &diags[0];
        assert!(first.line >= 1);
        assert!(
            first.message.contains("this_function_does_not_exist")
                || first.message.to_lowercase().contains("unknown")
                || first.message.to_lowercase().contains("not found"),
            "unexpected diagnostic message: {}",
            first.message,
        );
    }

    /// Catch the early-exit path: a `main.typ` outside the project
    /// root should fail fast with a clear message — no worker
    /// thread, no half-finished PDF.
    #[test]
    fn rejects_main_outside_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let other = tempfile::tempdir().expect("tempdir2");
        let main = other.path().join("probe.typ");
        std::fs::write(&main, "= hi\n").expect("write");
        // Drive the World constructor directly — `spawn_thread`
        // doesn't surface the construction error itself (the
        // worker would emit it through stderr instead). This test
        // also exercises the canonicalisation logic.
        let err = InkhavenWorld::new(
            dir.path(),
            &main,
            WorldSettings {
                bundle_fonts: false,
                use_system_fonts: false,
                packages_enabled: false,
            },
        )
        .err()
        .expect("should reject");
        assert!(err.contains("not inside project root"), "got: {err}");
    }
}
