//! 3.0.4 Phase-1 — `ink.cost.*` Bund stdlib: the AI cost ledger, read-only. Bund
//! reads the same per-category call tally and configured daily caps `inkhaven
//! cost` shows. Nothing here spends — it only reports what has been spent and
//! what the ceilings are, so a hook can decide whether to fire an LLM word.
//!
//! - `ink.cost.usage` ( -- list )  today's ledger, each a dict {category, calls}.
//! - `ink.cost.caps`  ( -- dict )  the configured daily caps
//!   {world, inner_socrates, inner_editor, retention_days}.
//! - `ink.cost.today` ( -- dict )  today's spend the way `inkhaven cost` shows it:
//!   {day, total_calls, entries:[{name, calls_today, daily_cap}]}. This one opens
//!   the companion sidecar DBs fresh (their slow-track calls are deliberately
//!   kept out of the flat usage ledger) — safe, the shipped `*.usage.today`
//!   words do the same.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, push};

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.cost.usage", w_usage),
        ("ink.cost.caps", w_caps),
        ("ink.cost.today", w_today),
    ];
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

/// ( -- dict ) today's spend {day, total_calls, entries:[{name, calls_today, daily_cap}]}.
fn w_today(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_today(vm).map_err(to_bund_err)
}
fn do_today(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.cost.today";
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    crate::dayclock::set_boundary(cfg.goals.day_boundary);
    let day = crate::dayclock::today_key();
    let report = crate::cli::cost::gather(
        store.project_root(),
        &day,
        cfg.cost.world_daily_call_cap,
        cfg.cost.inner_socrates_daily_call_cap,
        cfg.inner_editor.llm.editor_engagement.max_calls_per_day,
    );
    let entries: Vec<Value> = report
        .entries
        .iter()
        .map(|e| {
            let mut d: HashMap<String, Value> = HashMap::new();
            d.insert("name".into(), Value::from_string(&e.name));
            d.insert("calls_today".into(), Value::from_int(e.calls_today));
            d.insert("daily_cap".into(), Value::from_int(e.daily_cap));
            Value::from_dict(d)
        })
        .collect();
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("day".into(), Value::from_string(&report.day));
    m.insert("total_calls".into(), Value::from_int(report.total_calls()));
    m.insert("entries".into(), Value::from_list(entries));
    push(vm, Value::from_dict(m));
    Ok(vm)
}
