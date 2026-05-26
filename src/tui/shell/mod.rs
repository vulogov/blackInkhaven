//! In-process nushell pane (1.2.8+).
//!
//! Phase 1 (this commit): foundation spike. Pulls the four
//! nu-* crates we'll need (`nu-protocol`, `nu-engine`,
//! `nu-parser`, `nu-command`) into Cargo.toml and confirms
//! they compile cleanly alongside our existing dep set.
//! No public API yet — the engine builder, the modal, and
//! the selection-mode UI land in subsequent phases.
//!
//! Architecture (target shape):
//!   - `Engine` owns a nu `EngineState` + `Stack`, plus the
//!     per-project SQLite history connection.
//!   - `Pane` owns the modal-side state: input line, output
//!     turn buffer (capped per HJSON), selection cursor.
//!   - `eval(line) -> ShellOutput` parses + evals + captures
//!     pipeline output to bytes.  Long-running TTY-needing
//!     commands (`vim`, `top`) are explicitly out of scope —
//!     this is for one-shot pipelines whose output we
//!     surface in the pane and may insert into the editor as
//!     a typst raw-block.
//!
//! Tracker: tui::shell is gated `pub(super)`; the rest of
//! `tui::app` will reach it only via the Modal lifecycle.

#![allow(dead_code)]  // Phase 1 spike — public API lands in Phase 2.

/// Reserved for the Phase 2 engine builder. Importing
/// nu-protocol here once forces Cargo to resolve + build
/// the whole nu dep graph, so the spike catches dep
/// conflicts at build time even with no runtime call site.
/// LTO + dead-code elimination strips the dep from the
/// linked binary until Phase 2 makes real API calls.
#[allow(unused_imports)]
use nu_protocol::engine::EngineState as _NuEngineStateForLinkCheck;

/// Stub returned by Phase 2's `eval`. Placeholder fields so
/// Phase 1 doesn't carry compilation-only no-ops past the
/// spike.
pub(super) struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}
