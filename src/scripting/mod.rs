//! Bund scripting integration.
//!
//! Inkhaven's foothold for Vladimir's Bund language — a stack-based
//! scripting layer (`bundcore` + `bund_language_parser` +
//! `rust_multistackvm`) intended to host user-authored hooks, custom
//! AI prompt templates, and save-time rules.
//!
//! ## Adam
//!
//! "Adam" is the canonical name for the process-wide singleton VM —
//! the first one, the one that already has stdlib loaded. The name
//! comes from bundcore itself (`BundVM::adam`). `init_adam()` is
//! called exactly once per process, lazily on the first `eval()`.
//!
//! ## Active store
//!
//! `ink.*` stdlib words (Phase 1) need access to the project's
//! `Store`. Inkhaven runs single-project-per-process, so we install
//! the store into a global `ACTIVE_STORE` slot once and read it out
//! of each word handler. CLI commands that don't open a project
//! (only `inkhaven bund` so far) simply leave the slot empty and
//! the ink words error gracefully.
//!
//! ## What's wired in each phase
//!
//! - **P0**: `init_adam()`, `eval()` — round-trip a Bund script.
//! - **P1** *(this file)*: `register_active_store()` + read-only
//!   `ink.*` stdlib words via `stdlib::register_ink_stdlib`.
//! - **P3**: sandbox policy.
//! - **P4**: hook points fired from `src/store/mod.rs`.
//! - **P5**: first-class `NodeKind::Script` + Bund-aware editor.
//! - **P6**: ephemeral worker pool + result queue.

pub mod policy;
pub mod stdlib;

use anyhow::{anyhow, Result};
use bundcore::bundcore::Bund;
use parking_lot::RwLock;
use rust_dynamic::value::Value;
use std::sync::OnceLock;

use crate::store::Store;
use policy::Policy;

/// Process-wide singleton Bund VM. Borrows the bundcore "Adam"
/// terminology — see module docs.
static ADAM: OnceLock<RwLock<Bund>> = OnceLock::new();

/// The project store, set once at startup (either when the TUI
/// opens a project or when the CLI's `bund` subcommand chooses to
/// expose the project). `None` is a valid state: scripts that try
/// to use `ink.*` words against a script-only invocation will see
/// a clean "no project store registered" error.
static ACTIVE_STORE: OnceLock<Store> = OnceLock::new();

/// Sandbox policy to apply when Adam is built. Setters land before
/// the first `eval()` triggers lazy init; once Adam exists, the
/// policy is frozen for the process. `None` (no setter called) ⇒
/// the bundcore vanilla default, which deny destructive categories.
static POLICY: OnceLock<Policy> = OnceLock::new();

/// Initialise the Adam VM exactly once. Idempotent — subsequent
/// calls are no-ops. Loads bundcore's stdlib (arithmetic, string,
/// conditional, …) via the `Bund::new()` constructor, layers on
/// inkhaven's read-only `ink.*` words, then applies the sandbox
/// policy (re-registers denied words with a stub).
pub fn init_adam() -> Result<()> {
    if ADAM.get().is_some() {
        return Ok(());
    }
    let mut bund = Bund::new();
    stdlib::register_ink_stdlib(&mut bund.vm)
        .map_err(|e| anyhow!("register ink stdlib: {e}"))?;
    let p = POLICY.get().cloned().unwrap_or_default();
    if !p.is_open() {
        policy::apply_policy(&mut bund.vm, &p)
            .map_err(|e| anyhow!("apply policy: {e}"))?;
    }
    let _ = ADAM.set(RwLock::new(bund));
    Ok(())
}

/// Install the sandbox policy. Must be called BEFORE the first
/// `eval()` — otherwise Adam is already constructed under the
/// default policy and the call is a no-op. Idempotent in practice
/// (single-project-per-process).
pub fn set_policy(policy: Policy) {
    let _ = POLICY.set(policy);
}

/// Install the project store into the global slot. Called by the
/// TUI startup path and by the CLI when a subcommand wants its
/// Bund expressions to see the project. Idempotent in practice —
/// subsequent calls silently no-op (single-project-per-process).
pub fn register_active_store(store: Store) {
    let _ = ACTIVE_STORE.set(store);
}

/// Read access to the active store, used by `ink.*` word handlers.
/// `None` means no project has been opened in this process.
pub fn active_store() -> Option<&'static Store> {
    ACTIVE_STORE.get()
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
/// Strategy: scalar variants (string/int/float/bool) render as
/// their bare value so `inkhaven bund "40 2 +"` prints `42` rather
/// than `Value { … }`. Compound variants (list, map) go through
/// rust_dynamic's `cast_value_to_json` and get pretty-printed —
/// suitable for piping into `jq` or eyeballing the structure.
pub fn format_value(v: &Value) -> String {
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
    let j = value_to_json(v);
    serde_json::to_string_pretty(&j).unwrap_or_else(|_| j.to_string())
}

/// Recursive `Value` → `serde_json::Value` converter that fills the
/// gap in `rust_dynamic::Value::cast_value_to_json`: the upstream
/// helper doesn't handle the STRING variant (it errors at the
/// `_ =>` arm), so a list-of-maps-of-strings — which is exactly
/// what every `ink.*` word returns — comes out as Debug noise.
/// This walks the value ourselves and falls through to a debug
/// stringification only for variants we don't recognise.
fn value_to_json(v: &Value) -> serde_json::Value {
    if let Ok(s) = v.clone().cast_string() {
        return serde_json::Value::String(s);
    }
    if let Ok(i) = v.clone().cast_int() {
        return serde_json::Value::from(i);
    }
    if let Ok(f) = v.clone().cast_float() {
        return serde_json::Value::from(f);
    }
    if let Ok(b) = v.clone().cast_bool() {
        return serde_json::Value::Bool(b);
    }
    if let Ok(list) = v.clone().cast_list() {
        return serde_json::Value::Array(list.iter().map(value_to_json).collect());
    }
    if let Ok(dict) = v.clone().cast_dict() {
        let mut m = serde_json::Map::new();
        for (k, val) in dict.iter() {
            m.insert(k.clone(), value_to_json(val));
        }
        return serde_json::Value::Object(m);
    }
    // Last resort: NONE, NODATA, unrecognised variants.
    serde_json::Value::Null
}
