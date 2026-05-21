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

/// Look up the active project config. Used by the tree-mutating
/// `ink.*` words that need `cfg` for hierarchy validation /
/// artefact-path resolution.
pub fn active_config(err_prefix: &str) -> Result<&'static crate::config::Config> {
    crate::scripting::active_config().ok_or_else(|| {
        anyhow!(
            "{err_prefix} no project config registered \
             (run inside the TUI or pass --project on the CLI)"
        )
    })
}

/// Resolve a slug-path (`"book/chapter/paragraph"`) to a node
/// UUID by walking the hierarchy. Empty path returns `None`
/// (signalling "root").
///
/// Each component is matched against `Node::slug` of the
/// children of the current cursor. Order prefixes (`01-`) are
/// stripped automatically so both `"story/morning-light"` and
/// `"story/01-morning-light"` resolve to the same node.
pub fn resolve_path(
    hierarchy: &crate::store::hierarchy::Hierarchy,
    path: &str,
    err_prefix: &str,
) -> Result<Option<uuid::Uuid>> {
    let trimmed = path.trim().trim_matches('/');
    if trimmed.is_empty() {
        return Ok(None);
    }
    let mut current: Option<uuid::Uuid> = None;
    for raw in trimmed.split('/') {
        // Strip `NN-` order prefix so the writer can paste in a
        // path from `fs_name()` and have it still resolve.
        let segment = raw
            .split_once('-')
            .map(|(prefix, rest)| {
                if prefix.chars().all(|c| c.is_ascii_digit()) && !prefix.is_empty() {
                    rest
                } else {
                    raw
                }
            })
            .unwrap_or(raw);
        let candidates = hierarchy.children_of(current);
        let found = candidates
            .iter()
            .find(|n| n.slug == segment)
            .ok_or_else(|| {
                anyhow!(
                    "{err_prefix} segment `{segment}` not found under {}",
                    match current {
                        Some(id) => hierarchy
                            .get(id)
                            .map(|n| n.slug.clone())
                            .unwrap_or_else(|| id.to_string()),
                        None => "<root>".to_string(),
                    }
                )
            })?;
        current = Some(found.id);
    }
    Ok(current)
}
