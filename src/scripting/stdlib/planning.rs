//! 3.0.4 Phase-1 — `ink.planning.*` Bund stdlib: the story-structure frameworks,
//! read-only and pure (no store, no LLM). Bund reads the same canonical beat
//! tables `inkhaven plan` works from — the AI structural critique
//! (`plan analyze`) is deliberately NOT exposed (it costs), matching the
//! LECTOR / REDLINE / KEN precedent.
//!
//! - `ink.planning.frameworks` ( -- list )       every framework as {slug, label}.
//! - `ink.planning.beats`      ( framework -- list )  a framework's canonical beat
//!   table, each a dict {name, act, target_position, expected_tension}.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{pull, push, require_depth, value_to_string};
use crate::planning::Framework;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] =
        &[("ink.planning.frameworks", w_frameworks), ("ink.planning.beats", w_beats)];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f).map_err(|e| anyhow!("register {name}: {e}"))?;
    }
    for (name, _) in words {
        if let Some(short) = name.strip_prefix("ink.") {
            let _ = vm.register_alias(short.to_string(), name.to_string());
        }
    }
    Ok(())
}

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(e.to_string())
}

/// ( -- list ) every framework as {slug, label}.
fn w_frameworks(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let items: Vec<Value> = Framework::ALL
        .iter()
        .map(|f| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("slug".into(), Value::from_string(f.slug()));
            m.insert("label".into(), Value::from_string(f.label()));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

/// ( framework -- list ) the canonical beat table for `framework` (accepts any
/// slug/alias `Framework::parse` recognises). Errors on an unknown framework.
fn w_beats(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_beats(vm).map_err(to_bund_err)
}
fn do_beats(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.planning.beats";
    require_depth(vm, 1, tag)?;
    let want = value_to_string(pull(vm, tag)?, "framework", tag)?;
    let fw = Framework::parse(&want).ok_or_else(|| {
        anyhow!(
            "{tag}: unknown framework `{want}` (try one of: {})",
            Framework::ALL.iter().map(|f| f.slug()).collect::<Vec<_>>().join(", ")
        )
    })?;
    let items: Vec<Value> = fw
        .beats()
        .iter()
        .map(|b| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("name".into(), Value::from_string(b.name));
            m.insert("act".into(), Value::from_int(b.act as i64));
            m.insert("target_position".into(), Value::from_float(b.target_position as f64));
            m.insert("expected_tension".into(), Value::from_float(b.expected_tension as f64));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

#[cfg(test)]
mod tests {
    use crate::scripting;

    #[test]
    fn frameworks_lists_every_canon_framework() {
        // The pure words need no project store, so they run under bare `eval`.
        let out = scripting::eval("planning.frameworks").expect("eval");
        let list = out.top.expect("a result").cast_list().expect("a list");
        assert_eq!(list.len(), super::Framework::ALL.len());
        let first = list[0].cast_dict().expect("a dict");
        assert!(first.contains_key("slug") && first.contains_key("label"));
    }

    #[test]
    fn beats_returns_the_framework_canon_table() {
        let out = scripting::eval("\"three_act\" planning.beats").expect("eval");
        let list = out.top.expect("a result").cast_list().expect("a list");
        assert!(!list.is_empty(), "three-act has canonical beats");
        let beat = list[0].cast_dict().expect("a dict");
        for k in ["name", "act", "target_position", "expected_tension"] {
            assert!(beat.contains_key(k), "beat dict missing `{k}`");
        }
    }

    #[test]
    fn beats_rejects_an_unknown_framework() {
        // An unknown framework is a clean script error, not a panic.
        assert!(scripting::eval("\"no_such_framework\" planning.beats").is_err());
    }
}
