//! WORLD-6 — `ink.utopia.*` Bund stdlib: the utopian/dystopian coherence
//! checker from scripts. Reads `utopia.duckdb`; never triggers an LLM stage
//! (run those via `inkhaven world utopia-check`).
//!
//! - `ink.utopia.model`      ( -- list )            extracted claims.
//! - `ink.utopia.findings`   ( -- list )            unsuppressed findings.
//! - `ink.utopia.violations` ( -- list )            chapter ords w/ entailment.
//! - `ink.utopia.suppress`   ( finding reason -- )  mark a finding suppressed.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_store, pull, push, require_depth, value_to_string};
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::world::utopia::{FindingType, UtopiaStore};

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.utopia.model", w_model),
        ("ink.utopia.findings", w_findings),
        ("ink.utopia.violations", w_violations),
        ("ink.utopia.suppress", w_suppress),
    ];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f)
            .map_err(|e| anyhow!("register {name}: {e}"))?;
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

fn ctx(tag: &str) -> Result<(&'static Store, Node, UtopiaStore)> {
    let store = active_store(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = crate::cli::resolve_user_book(&h, None, tag)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .clone();
    let us = UtopiaStore::open(store.project_root()).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok((store, book, us))
}

fn opt_str(o: &Option<String>) -> Value {
    match o {
        Some(s) => Value::from_string(s),
        None => Value::nodata(),
    }
}

fn w_model(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_model(vm).map_err(to_bund_err)
}
fn do_model(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.utopia.model";
    let (_s, book, us) = ctx(tag)?;
    let claims = us.all_claims(&book.slug).map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = claims
        .iter()
        .map(|c| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("group".into(), Value::from_string(&c.premise_group));
            m.insert("type".into(), Value::from_string(c.claim_type.as_code()));
            m.insert("text".into(), Value::from_string(&c.claim_text));
            m.insert("source_para_id".into(), Value::from_string(&c.source_para_id));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_findings(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_findings(vm).map_err(to_bund_err)
}
fn do_findings(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.utopia.findings";
    let (_s, book, us) = ctx(tag)?;
    let findings = us.findings(&book.slug, true).map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = findings
        .iter()
        .map(|f| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("type".into(), Value::from_string(f.finding_type.as_code()));
            m.insert("domain".into(), Value::from_string(f.finding_domain.as_code()));
            m.insert("group".into(), Value::from_string(&f.premise_group));
            m.insert("description".into(), Value::from_string(&f.description));
            m.insert(
                "chapter".into(),
                match f.chapter_ord {
                    Some(c) => Value::from_int(c as i64),
                    None => Value::nodata(),
                },
            );
            m.insert("para_id".into(), opt_str(&f.para_id));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_violations(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_violations(vm).map_err(to_bund_err)
}
fn do_violations(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.utopia.violations";
    let (_s, book, us) = ctx(tag)?;
    let findings = us.findings(&book.slug, true).map_err(|e| anyhow!("{tag}: {e}"))?;
    let mut chapters: Vec<i64> = findings
        .iter()
        .filter(|f| f.finding_type == FindingType::EntailmentViolation)
        .filter_map(|f| f.chapter_ord.map(|c| c as i64))
        .collect();
    chapters.sort_unstable();
    chapters.dedup();
    let items: Vec<Value> = chapters.into_iter().map(Value::from_int).collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_suppress(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_suppress(vm).map_err(to_bund_err)
}
fn do_suppress(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.utopia.suppress";
    require_depth(vm, 2, tag)?;
    // Pushed finding then reason, so reason pops first.
    let reason = value_to_string(pull(vm, tag)?, "reason", tag)?;
    let finding = value_to_string(pull(vm, tag)?, "finding", tag)?;
    let (_s, book, us) = ctx(tag)?;
    let ok = us
        .suppress_finding(&book.slug, &finding, &reason)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    if !ok {
        return Err(anyhow!("{tag}: no finding with id `{finding}`"));
    }
    push(vm, Value::nodata());
    Ok(vm)
}
