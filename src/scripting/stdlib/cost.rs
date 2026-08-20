//! 3.0.4 Phase-1 — `ink.cost.*` Bund stdlib: the AI cost ledger, read-only. Bund
//! reads the same per-category call tally and configured daily caps `inkhaven
//! cost` shows. Nothing here spends — it only reports what has been spent and
//! what the ceilings are, so a hook can decide whether to fire an LLM word.
//!
//! - `ink.cost.usage` ( -- list )  today's ledger, each a dict {category, calls}.
//! - `ink.cost.caps`  ( -- dict )  the configured daily caps
//!   {world, inner_socrates, inner_editor, retention_days}.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, push};

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] =
        &[("ink.cost.usage", w_usage), ("ink.cost.caps", w_caps)];
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

/// ( -- list ) today's per-category call tally, each {category, calls}.
fn w_usage(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_usage(vm).map_err(to_bund_err)
}
fn do_usage(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.cost.usage";
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    // Same day-boundary the CLI honours, so "today" agrees with `inkhaven cost`.
    crate::dayclock::set_boundary(cfg.goals.day_boundary);
    let day = crate::dayclock::today_key();
    let rows = crate::ai::usage::usage_for(store.project_root(), &day);
    let items: Vec<Value> = rows
        .iter()
        .map(|(category, calls)| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("category".into(), Value::from_string(category));
            m.insert("calls".into(), Value::from_int(*calls));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

/// ( -- dict ) the configured daily call caps + retention.
fn w_caps(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_caps(vm).map_err(to_bund_err)
}
fn do_caps(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.cost.caps";
    let cfg = active_config(tag)?;
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("world".into(), Value::from_int(cfg.cost.world_daily_call_cap));
    m.insert(
        "inner_socrates".into(),
        Value::from_int(cfg.cost.inner_socrates_daily_call_cap),
    );
    m.insert(
        "inner_editor".into(),
        Value::from_int(cfg.inner_editor.llm.editor_engagement.max_calls_per_day),
    );
    m.insert("retention_days".into(), Value::from_int(cfg.cost.usage_retention_days as i64));
    push(vm, Value::from_dict(m));
    Ok(vm)
}
