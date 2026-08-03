//! INNER-STYLIST — `ink.stylist.*` Bund stdlib: the Inner Stylist's synthesised
//! voice-at-scale findings + suppression management. Deterministic (the LLM
//! `--coach` is not exposed here). Over the single user book.
//!
//! - `ink.stylist.findings`     ( -- list )  synthesised Praise/Note/Concern, minus suppressions.
//! - `ink.stylist.suppress`     ( key -- )    silence a finding by its key.
//! - `ink.stylist.unsuppress`   ( key -- )    restore it.
//! - `ink.stylist.suppressions` ( -- list )   the silenced keys.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, pull, push, value_to_string};
use crate::inner_stylist::storage::InnerStylistStore;
use crate::project::ProjectLayout;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.stylist.findings", w_findings),
        ("ink.stylist.suppress", w_suppress),
        ("ink.stylist.unsuppress", w_unsuppress),
        ("ink.stylist.suppressions", w_suppressions),
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

fn open_store(tag: &str) -> Result<InnerStylistStore> {
    let store = active_store(tag)?;
    InnerStylistStore::open_for_project(store.project_root()).map_err(|e| anyhow!("{tag}: {e}"))
}

macro_rules! word {
    ($w:ident, $do:ident) => {
        fn $w(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
            $do(vm).map_err(to_bund_err)
        }
    };
}

word!(w_findings, do_findings);
fn do_findings(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.stylist.findings";
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = crate::store::hierarchy::Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = crate::cli::resolve_user_book(&h, None, tag).map_err(|e| anyhow!("{tag}: {e}"))?.clone();
    let now = chrono::Utc::now().to_rfc3339();
    let mut findings = crate::inner_stylist::pipeline::gather(store, &layout, &h, cfg, &book, &now)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    if let Ok(silenced) = open_store(tag)?.all_suppressions() {
        findings.retain(|f| !silenced.contains(&f.key));
    }
    let items: Vec<Value> = findings
        .iter()
        .map(|f| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("severity".into(), Value::from_string(f.severity.label()));
            m.insert("kind".into(), Value::from_string(f.kind));
            m.insert("key".into(), Value::from_string(&f.key));
            m.insert("message".into(), Value::from_string(&f.message));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

word!(w_suppress, do_suppress);
fn do_suppress(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.stylist.suppress";
    let key = value_to_string(pull(vm, tag)?, "key", tag)?;
    open_store(tag)?.suppress(&key).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

word!(w_unsuppress, do_unsuppress);
fn do_unsuppress(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.stylist.unsuppress";
    let key = value_to_string(pull(vm, tag)?, "key", tag)?;
    open_store(tag)?.unsuppress(&key).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

word!(w_suppressions, do_suppressions);
fn do_suppressions(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.stylist.suppressions";
    let keys = open_store(tag)?.all_suppressions().map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_list(keys.iter().map(|k| Value::from_string(k)).collect()));
    Ok(vm)
}
