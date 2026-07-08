//! INNER_SOCRATES-1 — the `ink.inner_socrates.*` Bund stdlib. Read-only words
//! (Phase-1 sandbox convention): run the deterministic Fast track, list findings,
//! personas, and ledger entries, and read the slow-track usage tally. Mutations
//! (activate a persona, write the ledger) stay on the CLI for now.

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_store, pull, push, require_depth, value_to_string};
use crate::intent::FindingContext;
use crate::inner_socrates::storage::InnerSocratesStore;
use crate::inner_socrates::{fast, personas};

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.inner_socrates.check.fast", w_check_fast),
        ("ink.inner_socrates.findings.list", w_findings_list),
        ("ink.inner_socrates.personas.list", w_personas_list),
        ("ink.inner_socrates.persona.active", w_persona_active),
        ("ink.inner_socrates.ledger.list", w_ledger_list),
        ("ink.inner_socrates.usage.today", w_usage_today),
    ];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f).map_err(|e| anyhow!("register {name}: {e}"))?;
    }
    Ok(())
}

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(format!("{e}"))
}

// ( text -- list )  run the Fast track on `text`; push the questions as a list.
fn w_check_fast(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_check_fast(vm).map_err(to_bund_err)
}
fn do_check_fast(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.inner_socrates.check.fast";
    require_depth(vm, 1, tag)?;
    let text = value_to_string(pull(vm, tag)?, "text", tag)?;
    let root = active_store(tag)?.project_root().to_path_buf();
    let persona = personas::active(&root);
    let ledger = InnerSocratesStore::open_for_project(&root)
        .ok()
        .and_then(|s| s.load_ledger().ok())
        .unwrap_or_default();
    let findings = fast::check_paragraph(&text, &persona, &ledger, &FindingContext::default());
    push(vm, Value::from_list(findings.iter().map(|f| Value::from_string(f.question.clone())).collect()));
    Ok(vm)
}

// ( -- list )  the persisted findings, as `{category, severity, question}` dicts.
fn w_findings_list(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_findings_list(vm).map_err(to_bund_err)
}
fn do_findings_list(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.inner_socrates.findings.list";
    let root = active_store(tag)?.project_root().to_path_buf();
    let findings = InnerSocratesStore::open_for_project(&root)
        .ok()
        .and_then(|s| s.list_findings().ok())
        .unwrap_or_default();
    let items: Vec<Value> = findings
        .iter()
        .map(|sf| {
            let mut h = std::collections::HashMap::new();
            h.insert("category".to_string(), Value::from_string(sf.finding.category.id().to_string()));
            h.insert("severity".to_string(), Value::from_string(sf.finding.severity.label().to_string()));
            h.insert("question".to_string(), Value::from_string(sf.finding.question.clone()));
            Value::from_dict(h)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

// ( -- list )  the available persona ids.
fn w_personas_list(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_personas_list(vm).map_err(to_bund_err)
}
fn do_personas_list(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.inner_socrates.personas.list";
    let root = active_store(tag)?.project_root().to_path_buf();
    let ids: Vec<Value> =
        personas::load_all(&root).iter().map(|p| Value::from_string(p.id.clone())).collect();
    push(vm, Value::from_list(ids));
    Ok(vm)
}

// ( -- id )  the active persona's id.
fn w_persona_active(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_persona_active(vm).map_err(to_bund_err)
}
fn do_persona_active(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.inner_socrates.persona.active";
    let root = active_store(tag)?.project_root().to_path_buf();
    push(vm, Value::from_string(personas::active(&root).id));
    Ok(vm)
}

// ( -- list )  the intent-ledger entry descriptions.
fn w_ledger_list(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ledger_list(vm).map_err(to_bund_err)
}
fn do_ledger_list(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.inner_socrates.ledger.list";
    let root = active_store(tag)?.project_root().to_path_buf();
    let entries = InnerSocratesStore::open_for_project(&root)
        .ok()
        .and_then(|s| s.list_intents().ok())
        .unwrap_or_default();
    let items: Vec<Value> = entries
        .iter()
        .map(|e| {
            let mut h = std::collections::HashMap::new();
            h.insert("id".to_string(), Value::from_string(e.id.clone()));
            h.insert("kind".to_string(), Value::from_string(e.kind.id().to_string()));
            h.insert("description".to_string(), Value::from_string(e.description.clone()));
            Value::from_dict(h)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

// ( -- n )  slow-track LLM calls made today.
fn w_usage_today(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_usage_today(vm).map_err(to_bund_err)
}
fn do_usage_today(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.inner_socrates.usage.today";
    let root = active_store(tag)?.project_root().to_path_buf();
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let n = InnerSocratesStore::open_for_project(&root)
        .ok()
        .and_then(|s| s.llm_calls_today(&day, "slow_track").ok())
        .unwrap_or(0);
    push(vm, Value::from_int(n));
    Ok(vm)
}
