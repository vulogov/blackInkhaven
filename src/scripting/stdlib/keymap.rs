//! `ink.key.*` Bund stdlib — runtime chord rebinding.
//!
//! Stage 2 of the rebindable-keys roadmap. Lets Bund scripts
//! (typically the project bootstrap or a Script node) mutate the
//! same `KeyBindings` table the App reads from on every chord
//! dispatch. The shared state lives in `tui::keybind::ACTIVE`;
//! all four words below acquire its write lock for the duration
//! of one mutation, then release.
//!
//! ## Sandbox
//!
//! All four words belong to the `keymap` policy category. That
//! category is in `policy::DEFAULT_DENIED_CATEGORIES`, so a stock
//! project gets the deny-stub. Users opt in by listing `keymap`
//! in `scripting.enabled_categories` in `inkhaven.hjson`.
//!
//! The `ACTIVE` table in `tui::keybind` is lazily initialised
//! with `KeyBindings::defaults()`, so CLI smoke (`inkhaven bund
//! "..."` outside a TUI) sees a functioning binding table even
//! before any project is opened.
//!
//! ## API
//!
//! ```bund
//! // Bind a chord to a named built-in action.
//! "Ctrl+b y"   "tree.morph_type"   ink.key.bind
//!
//! // Bind a chord to an inline lambda. The lambda is registered
//! // under a synthetic name and invoked via the existing hooks
//! // machinery (with recursion cap + policy).
//! "Ctrl+b j"   { "jot!" println }  ink.key.bind_lambda
//!
//! // Unbind a chord.
//! "Ctrl+b y"                       ink.key.unbind
//!
//! // List every active binding as a JSON-friendly Value.
//!                                  ink.key.list
//! ```

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::types::LAMBDA;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;
use std::collections::HashMap;
use std::sync::Arc;

use super::helpers::{pull, push, require_depth, value_to_string};
use crate::tui::keybind::{self, Action, BindingEntry, Scope};

/// Register every `ink.key.*` word on `vm`. Called once from
/// `register_ink_stdlib` after the policy + bootstrap have run,
/// so the policy sandbox sees these words and can deny them.
pub fn register(vm: &mut VM) -> Result<()> {
    vm.register_inline("ink.key.bind".to_string(), ink_key_bind)
        .map_err(|e| anyhow!("register ink.key.bind: {e}"))?;
    vm.register_inline("ink.key.bind_lambda".to_string(), ink_key_bind_lambda)
        .map_err(|e| anyhow!("register ink.key.bind_lambda: {e}"))?;
    vm.register_inline("ink.key.unbind".to_string(), ink_key_unbind)
        .map_err(|e| anyhow!("register ink.key.unbind: {e}"))?;
    vm.register_inline("ink.key.list".to_string(), ink_key_list)
        .map_err(|e| anyhow!("register ink.key.list: {e}"))?;
    Ok(())
}

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(e.to_string())
}

// ── ink.key.bind ─────────────────────────────────────────────────────
// Stack: ( chord_str action_str -- )
// Action name uses the dotted form (e.g. "tree.morph_type"). To
// disable a chord without rebinding, pass "none".

fn ink_key_bind(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_key_bind(vm).map_err(to_bund_err)
}

fn do_ink_key_bind(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.key.bind";
    require_depth(vm, 2, tag)?;
    let action_str = value_to_string(pull(vm, tag)?, "action", tag)?;
    let chord_str = value_to_string(pull(vm, tag)?, "chord", tag)?;
    let mut bindings = keybind::write();
    let (layer, suffix) = bindings
        .parse_sub_chord(&chord_str)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    let action = parse_action(&action_str)
        .map_err(|e| anyhow!("{tag} action: {e}"))?;
    bindings.add(
        layer,
        BindingEntry {
            chord: suffix,
            action,
            scope: Scope::Any,
        },
    );
    Ok(vm)
}

// ── ink.key.bind_lambda ──────────────────────────────────────────────
// Stack: ( chord_str lambda -- )
// Registers the lambda under a synthetic name in vm.lambdas, then
// binds the chord to Action::BundLambda(name). Errors if the
// top of stack isn't a LAMBDA value.

fn ink_key_bind_lambda(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_key_bind_lambda(vm).map_err(to_bund_err)
}

fn do_ink_key_bind_lambda(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.key.bind_lambda";
    require_depth(vm, 2, tag)?;
    let lambda = pull(vm, tag)?;
    let chord_str = value_to_string(pull(vm, tag)?, "chord", tag)?;
    if lambda.type_of() != LAMBDA {
        return Err(anyhow!(
            "{tag}: top of stack must be a LAMBDA value (use `{{ … }}` braces)"
        ));
    }

    // Resolve the binding under a write lock first so we fail
    // before mutating VM state if the chord parse fails.
    let mut bindings = keybind::write();
    let (layer, suffix) = bindings
        .parse_sub_chord(&chord_str)
        .map_err(|e| anyhow!("{tag}: {e}"))?;

    let name = format!(
        "__keybind_{}__",
        uuid::Uuid::now_v7().as_simple()
    );
    // register_lambda lives on VM; it stores the lambda value in
    // vm.lambdas keyed by name. The bund-side caller doesn't need
    // to know the synthetic name — it lives only as the BundLambda
    // payload in the binding table.
    vm.register_lambda(name.clone(), lambda)
        .map_err(|e| anyhow!("{tag} register_lambda: {e}"))?;

    bindings.add(
        layer,
        BindingEntry {
            chord: suffix,
            action: Action::BundLambda(Arc::from(name.as_str())),
            scope: Scope::Any,
        },
    );
    Ok(vm)
}

// ── ink.key.unbind ───────────────────────────────────────────────────
// Stack: ( chord_str -- )
// Drops every binding for the named chord (any scope).

fn ink_key_unbind(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_key_unbind(vm).map_err(to_bund_err)
}

fn do_ink_key_unbind(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.key.unbind";
    require_depth(vm, 1, tag)?;
    let chord_str = value_to_string(pull(vm, tag)?, "chord", tag)?;
    let mut bindings = keybind::write();
    let (layer, suffix) = bindings
        .parse_sub_chord(&chord_str)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    // Remove from every scope so the user doesn't need to know
    // which scope a default was registered under.
    for scope in [Scope::Any, Scope::Editor, Scope::Tree, Scope::Ai] {
        bindings.remove(layer, &suffix, scope);
    }
    Ok(vm)
}

// ── ink.key.list ─────────────────────────────────────────────────────
// Stack: ( -- list )
// Pushes a JSON-friendly list of { layer, chord, action, scope }
// entries for every currently registered binding.

fn ink_key_list(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_key_list(vm).map_err(to_bund_err)
}

fn do_ink_key_list(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.key.list";
    let bindings = keybind::read();
    let _ = tag; // kept for symmetry with the other handlers
    let mut out: Vec<Value> = Vec::new();
    push_entries(&mut out, &bindings.meta_sub, "meta");
    push_entries(&mut out, &bindings.bund_sub, "bund");
    push(vm, Value::from_list(out));
    Ok(vm)
}

fn push_entries(out: &mut Vec<Value>, table: &[BindingEntry], layer: &str) {
    for e in table {
        let mut h: HashMap<String, Value> = HashMap::new();
        h.insert("layer".into(), Value::from_string(layer));
        h.insert("chord".into(), Value::from_string(chord_to_string(&e.chord)));
        h.insert("action".into(), Value::from_string(action_to_string(&e.action)));
        h.insert("scope".into(), Value::from_string(scope_to_string(e.scope)));
        out.push(Value::from_dict(h));
    }
}

fn chord_to_string(c: &crate::tui::keymap::KeyChord) -> String {
    // Quick-and-dirty: rely on Debug for now. A pretty printer
    // would round-trip via the parser, which is a follow-up.
    format!("{c:?}")
}

fn action_to_string(a: &Action) -> String {
    serde_json::to_string(a)
        .ok()
        .and_then(|s| serde_json::from_str::<String>(&s).ok())
        .unwrap_or_else(|| format!("{a:?}"))
}

fn scope_to_string(s: Scope) -> String {
    match s {
        Scope::Any => "any".into(),
        Scope::Editor => "editor".into(),
        Scope::Tree => "tree".into(),
        Scope::Ai => "ai".into(),
    }
}

fn parse_action(s: &str) -> Result<Action, String> {
    // Same JSON-string round-trip as the HJSON overlay parser.
    serde_json::from_str::<Action>(&format!("\"{s}\""))
        .map_err(|e| format!("action `{s}`: {e}"))
}
