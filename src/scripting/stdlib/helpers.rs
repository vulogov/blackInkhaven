//! Small helpers shared by every `ink.*` word handler. Mirrors the
//! `doc_helpers` pattern from bdslib's `vm/stdlib/db/` tree — pull/
//! push/depth-check/UUID-parse — but bound to anyhow errors and
//! inkhaven's own `Store`.

use anyhow::{anyhow, bail, Result};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;
use uuid::Uuid;

use crate::store::Store;

/// Pull one `Value` off the top of the VM's current stack, or error
/// with the supplied prefix tag.
pub fn pull(vm: &mut VM, err_prefix: &str) -> Result<Value> {
    vm.stack
        .pull()
        .ok_or_else(|| anyhow!("{err_prefix} stack underflow"))
}

/// Push one `Value` onto the top of the VM's current stack.
pub fn push(vm: &mut VM, v: Value) {
    let _ = vm.stack.push(v);
}

/// Assert at least `n` items are on the stack, or bail with the
/// supplied prefix tag.
pub fn require_depth(vm: &mut VM, n: usize, err_prefix: &str) -> Result<()> {
    let depth = vm.stack.current_stack_len();
    if depth < n {
        bail!("{err_prefix} requires {n} item(s) but only {depth} on stack");
    }
    Ok(())
}

/// Cast a `Value` to a `Uuid`, expecting it to have arrived as a
/// string. Yields a tagged error if the cast or parse fails — keeps
/// the script author's debug experience reasonable.
pub fn value_to_uuid(v: Value, err_prefix: &str) -> Result<Uuid> {
    let s = v
        .cast_string()
        .map_err(|e| anyhow!("{err_prefix} UUID string cast failed: {e}"))?;
    Uuid::parse_str(&s).map_err(|e| anyhow!("{err_prefix} UUID parse failed: {e}"))
}

/// Cast a `Value` to an `i64`. Used for the `limit` args of search /
/// listing words.
pub fn value_to_i64(v: Value, field: &str, err_prefix: &str) -> Result<i64> {
    v.cast_int()
        .map_err(|e| anyhow!("{err_prefix} {field} int cast failed: {e}"))
}

/// Cast a `Value` to a `String`.
pub fn value_to_string(v: Value, field: &str, err_prefix: &str) -> Result<String> {
    v.cast_string()
        .map_err(|e| anyhow!("{err_prefix} {field} string cast failed: {e}"))
}

/// Look up the active store. Used by every read-only `ink.*` word.
/// Errors with the supplied prefix when no project is registered —
/// e.g., when the user runs `inkhaven bund` without `--project`.
pub fn active_store(err_prefix: &str) -> Result<&'static Store> {
    crate::scripting::active_store().ok_or_else(|| {
        anyhow!(
            "{err_prefix} no project store registered \
             (run inside the TUI or pass --project on the CLI)"
        )
    })
}
