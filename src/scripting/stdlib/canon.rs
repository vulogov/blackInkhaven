//! CANON-LEDGER-1 (CL-P8) — `ink.canon.*` Bund stdlib: the story's canon
//! development ledger, read-only. `list` enumerates the recorded decisions;
//! `impact` and `why` answer the two revision questions ("what breaks if I cut
//! this?" / "what does this rest on?"); `forks` surfaces concurrent disagreements
//! on canonicity. The **writes are not exposed**: `commit` (setting a decision's
//! commitment) and the opt-in, author-confirmed LLM harvest (`accept`) are
//! deliberate authorial acts, so they live on the CLI and in the editor, not in
//! scripts — the same discipline `ink.chronicle` uses for `mark`.
//!
//! - `ink.canon.list` ( -- list )      every decision as a dict
//!   {uid, kind, gist, commitment, locator, node}.
//! - `ink.canon.impact` ( id -- list ) the blast radius of `id`: every decision
//!   that transitively rests on it (same dict shape).
//! - `ink.canon.why` ( id -- list )    the grounds chain `id` rests on.
//! - `ink.canon.history` ( id -- list ) `id`'s commitment trajectory over time,
//!   as dicts {level, agent, ts} (oldest first).
//! - `ink.canon.forks` ( -- list )     commitment forks as dicts {uid, gist,
//!   positions:[{agent, level}], resolved}.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_store, pull, push, value_to_string};
use crate::canon::CanonView;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.canon.list", w_list),
        ("ink.canon.impact", w_impact),
        ("ink.canon.why", w_why),
        ("ink.canon.history", w_history),
        ("ink.canon.forks", w_forks),
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

macro_rules! word {
    ($w:ident, $do:ident) => {
        fn $w(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
            $do(vm).map_err(to_bund_err)
        }
    };
}

/// A decision rendered as a Bund dict — the same fields the CLI `print_view`
/// shows (kind/commitment as their short strings).
fn view_dict(v: &CanonView) -> Value {
    let mut d: HashMap<String, Value> = HashMap::new();
    d.insert("uid".into(), Value::from_string(v.uid.short()));
    d.insert(
        "kind".into(),
        v.kind
            .map(|k| Value::from_string(k.schema_str().trim_start_matches("x.narrative/")))
            .unwrap_or_else(Value::nodata),
    );
    d.insert("gist".into(), Value::from_string(&v.gist));
    d.insert(
        "commitment".into(),
        v.commitment.map(|c| Value::from_string(c.to_string())).unwrap_or_else(Value::nodata),
    );
    d.insert(
        "locator".into(),
        v.locator.as_deref().map(Value::from_string).unwrap_or_else(Value::nodata),
    );
    d.insert(
        "node".into(),
        v.node.map(|n| Value::from_string(n.to_string())).unwrap_or_else(Value::nodata),
    );
    Value::from_dict(d)
}

/// Resolve a uid prefix argument against the active project's ledger.
fn resolve_arg(vm: &mut VM, tag: &str) -> Result<(smysl::Uid, &'static crate::canon::CanonLedger)> {
    let id = value_to_string(pull(vm, tag)?, "id", tag)?;
    let canon = active_store(tag)?.raw().canon();
    let uid = canon.resolve(&id).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok((uid, canon))
}

word!(w_list, do_list);
fn do_list(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.canon.list";
    let canon = active_store(tag)?.raw().canon();
    let views = canon.all_decisions().map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_list(views.iter().map(view_dict).collect()));
    Ok(vm)
}

word!(w_impact, do_impact);
fn do_impact(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.canon.impact";
    let (uid, canon) = resolve_arg(vm, tag)?;
    let views = canon.impact(uid).map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_list(views.iter().map(view_dict).collect()));
    Ok(vm)
}

word!(w_why, do_why);
fn do_why(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.canon.why";
    let (uid, canon) = resolve_arg(vm, tag)?;
    let views = canon.why(uid).map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_list(views.iter().map(view_dict).collect()));
    Ok(vm)
}

word!(w_history, do_history);
fn do_history(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.canon.history";
    let (uid, canon) = resolve_arg(vm, tag)?;
    let hist = canon.history(uid).map_err(|e| anyhow!("{tag}: {e}"))?;
    let events = hist.map(|h| h.trajectory).unwrap_or_default();
    let list: Vec<Value> = events
        .iter()
        .map(|e| {
            let mut d: HashMap<String, Value> = HashMap::new();
            d.insert("level".into(), Value::from_string(e.level.to_string()));
            d.insert("agent".into(), Value::from_string(&e.agent));
            d.insert("ts".into(), Value::from_int(e.wall_ms as i64));
            Value::from_dict(d)
        })
        .collect();
    push(vm, Value::from_list(list));
    Ok(vm)
}

word!(w_forks, do_forks);
fn do_forks(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.canon.forks";
    let canon = active_store(tag)?.raw().canon();
    let forks = canon.commitment_forks().map_err(|e| anyhow!("{tag}: {e}"))?;
    let list: Vec<Value> = forks
        .iter()
        .map(|fk| {
            let mut d: HashMap<String, Value> = HashMap::new();
            d.insert("uid".into(), Value::from_string(fk.unit.uid.short()));
            d.insert("gist".into(), Value::from_string(&fk.unit.gist));
            let positions: Vec<Value> = fk
                .positions
                .iter()
                .map(|(agent, level)| {
                    let mut p: HashMap<String, Value> = HashMap::new();
                    p.insert("agent".into(), Value::from_string(agent));
                    p.insert("level".into(), Value::from_string(level.to_string()));
                    Value::from_dict(p)
                })
                .collect();
            d.insert("positions".into(), Value::from_list(positions));
            d.insert("resolved".into(), Value::from_bool(fk.resolved));
            Value::from_dict(d)
        })
        .collect();
    push(vm, Value::from_list(list));
    Ok(vm)
}
