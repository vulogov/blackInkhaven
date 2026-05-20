//! Bund scripting integration.
//!
//! Inkhaven's first foothold for Vladimir's Bund language — a stack-
//! based scripting layer (`bundcore` + `bund_language_parser` +
//! `rust_multistackvm`) intended to host user-authored hooks, custom
//! AI prompt templates, and save-time rules.
//!
//! ## Adam
//!
//! "Adam" is the canonical name for the process-wide singleton VM —
//! the first one, the one that already has stdlib loaded. The name
//! comes from bundcore itself (`BundVM::adam`). We follow the same
//! convention: `init_adam()` is called exactly once per process,
//! lazily on the first `eval()`.
//!
//! ## Phase 0 — minimum viable shape
//!
//! This module exists in three pieces:
//!
//! * `ADAM`: a `OnceLock<RwLock<Bund>>` holding the singleton.
//! * `init_adam()`: idempotent constructor. Calls `Bund::new()` which
//!   transitively runs `init_stdlib(&mut vm)` from bundcore — that
//!   loads arithmetic, string ops, conditional, etc. We add no
//!   inkhaven-specific words yet; that lands in P1.
//! * `eval(code)`: parse + run a script against Adam under a write
//!   lock. Returns the top-of-stack `Value` (if any), letting the
//!   caller decide how to format it.
//!
//! ## Not yet wired in P0
//!
//! - Inkhaven-specific stdlib (`ink.node.get`, `ink.search.text`, …)
//!   — that's P1.
//! - Sandbox policy — P3.
//! - Hook points in `Store` — P4.
//! - First-class `NodeKind::Script` — P5.
//! - Worker pool + result queue for async — P6.

use anyhow::{anyhow, Result};
use bundcore::bundcore::Bund;
use parking_lot::RwLock;
use rust_dynamic::value::Value;
use std::sync::OnceLock;

/// Process-wide singleton Bund VM. Borrows the bundcore "Adam"
/// terminology — see module docs.
static ADAM: OnceLock<RwLock<Bund>> = OnceLock::new();

/// Initialise the Adam VM exactly once. Idempotent — subsequent
/// calls are no-ops. Loads bundcore's stdlib (arithmetic, string,
/// conditional, …) via the `Bund::new()` constructor.
pub fn init_adam() -> Result<()> {
    if ADAM.get().is_some() {
        return Ok(());
    }
    let bund = Bund::new();
    // OnceLock::set returns Err if a racing initialiser won — that's
    // fine, we just drop our copy.
    let _ = ADAM.set(RwLock::new(bund));
    Ok(())
}

/// Parse + evaluate `code` against Adam, then pop and return the top
/// of the workbench (current stack). Returns `Ok(None)` when the
/// script produced no result.
///
/// Auto-initialises Adam on the first call.
pub fn eval(code: &str) -> Result<Option<Value>> {
    init_adam()?;
    let adam = ADAM.get().ok_or_else(|| anyhow!("Adam VM missing after init"))?;
    let mut guard = adam.write();
    guard
        .eval(code)
        .map_err(|e| anyhow!("bund eval failed: {e}"))?;
    Ok(guard.vm.stack.pull())
}

/// Render a `rust_dynamic::Value` as a human-readable string. Used
/// by the CLI subcommand and (eventually) the TUI command output.
///
/// We deliberately don't go through `Debug` because that emits Rust
/// struct-literal syntax — fine for diagnostics, ugly for the
/// "user typed a Bund expression and wants to see the answer" case.
pub fn format_value(v: &Value) -> String {
    // rust_dynamic exposes typed accessors per variant. The
    // `Display` impl in newer versions is `{:?}`-ish; this helper
    // gives us a stable plain rendering we control.
    if let Ok(s) = v.clone().cast_string() {
        return s;
    }
    if let Ok(i) = v.clone().cast_int() {
        return i.to_string();
    }
    if let Ok(f) = v.clone().cast_float() {
        return f.to_string();
    }
    if let Ok(b) = v.clone().cast_bool() {
        return b.to_string();
    }
    // Fall through to the debug-ish dump — covers lists, hashes,
    // etc. without us hand-rolling a serialiser for every variant.
    format!("{v:?}")
}
