//! 1.4.3 — `ink.inner_editor.*` Bund stdlib: drive the Inner Editor companion
//! from a script. Read-only inspectors (findings / usage / config / categories /
//! system_prompt), the `intent.declare` mutator (writes the shared ledger), and
//! `engage` (an LLM pass over a paragraph). Mirrors `ink.inner_socrates.*` and
//! `ink.book_rag.*`.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;
use uuid::Uuid;

use super::helpers::{active_config, active_store, pull, push, require_depth, value_to_string};
use crate::inner_editor::types::EditorCategory;
use crate::inner_editor::{self, InnerEditorStore};
use crate::store::hierarchy::Hierarchy;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.inner_editor.findings.list", w_findings_list),
        ("ink.inner_editor.usage.today", w_usage_today),
        ("ink.inner_editor.config", w_config),
        ("ink.inner_editor.categories", w_categories),
        ("ink.inner_editor.system_prompt", w_system_prompt),
        ("ink.inner_editor.intent.declare", w_intent_declare),
        ("ink.inner_editor.engage", w_engage),
    ];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f)
            .map_err(|e| anyhow!("register {name}: {e}"))?;
    }
    Ok(())
}

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(e.to_string())
}

// ( -- list )  persisted Editor findings as {id, paragraph_id, severity, category, observation} dicts.
fn w_findings_list(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_findings_list(vm).map_err(to_bund_err)
}
fn do_findings_list(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.inner_editor.findings.list";
    let store = InnerEditorStore::open_for_project(active_store(tag)?.project_root())
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = store
        .list_findings()
        .unwrap_or_default()
        .iter()
        .map(|sf| {
            let mut h = HashMap::new();
            h.insert("id".to_string(), Value::from_string(sf.id.to_string()));
            h.insert(
                "paragraph_id".to_string(),
                Value::from_string(sf.paragraph_id.map(|p| p.to_string()).unwrap_or_default()),
            );
            h.insert("severity".to_string(), Value::from_string(sf.finding.severity.id()));
            h.insert("category".to_string(), Value::from_string(sf.finding.category.id()));
            h.insert("observation".to_string(), Value::from_string(sf.finding.observation.clone()));
            Value::from_dict(h)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

// ( -- n )  today's Editor engagement LLM calls.
fn w_usage_today(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_usage_today(vm).map_err(to_bund_err)
}
fn do_usage_today(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.inner_editor.usage.today";
    let cfg = active_config(tag)?;
    crate::dayclock::set_boundary(cfg.goals.day_boundary);
    let day = crate::dayclock::today_key();
    let store = InnerEditorStore::open_for_project(active_store(tag)?.project_root())
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    let n = store
        .llm_calls_today(&day, InnerEditorStore::ENGAGEMENT_SUB_BUDGET)
        .unwrap_or(0);
    push(vm, Value::from_int(n));
    Ok(vm)
}

// ( -- dict )  the active Inner Editor configuration.
fn w_config(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_config(vm).map_err(to_bund_err)
}
fn do_config(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.inner_editor.config";
    let cfg = active_config(tag)?;
    let ie = &cfg.inner_editor;
    let tuning = inner_editor::resolve_tuning(&ie.persona);
    let mut h = HashMap::new();
    h.insert("enabled".to_string(), Value::from_bool(ie.enabled));
    h.insert("tone".to_string(), Value::from_string(ie.persona.tone.clone()));
    h.insert("verbosity".to_string(), Value::from_string(ie.persona.verbosity.clone()));
    h.insert("praise_frequency".to_string(), Value::from_string(ie.persona.praise_frequency.clone()));
    h.insert("genre_aware".to_string(), Value::from_bool(ie.persona.genre_aware));
    h.insert("belief_stance".to_string(), Value::from_bool(ie.persona.belief_stance_enabled));
    h.insert("severity_threshold".to_string(), Value::from_string(ie.output.severity_threshold.clone()));
    h.insert("idle_threshold_seconds".to_string(), Value::from_int(ie.engagement.idle_threshold_seconds as i64));
    h.insert("cooldown_seconds".to_string(), Value::from_int(ie.engagement.cooldown_seconds as i64));
    h.insert("max_findings_per_paragraph".to_string(), Value::from_int(ie.engagement.max_findings_per_paragraph as i64));
    h.insert("preceding_paragraphs".to_string(), Value::from_int(ie.context.preceding_paragraphs as i64));
    h.insert(
        "active_categories".to_string(),
        Value::from_list(tuning.active_categories.iter().map(|c| Value::from_string(c.id())).collect()),
    );
    if let Some(g) = cfg.genre.as_deref() {
        h.insert("genre".to_string(), Value::from_string(g));
    }
    push(vm, Value::from_dict(h));
    Ok(vm)
}

// ( -- list )  the eight category ids.
fn w_categories(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    push(
        vm,
        Value::from_list(EditorCategory::ALL.iter().map(|c| Value::from_string(c.id())).collect()),
    );
    Ok(vm)
}

// ( lang -- text )  the localized Editor system prompt.
fn w_system_prompt(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_system_prompt(vm).map_err(to_bund_err)
}
fn do_system_prompt(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.inner_editor.system_prompt";
    require_depth(vm, 1, tag)?;
    let lang = value_to_string(pull(vm, tag)?, "lang", tag)?;
    push(vm, Value::from_string(inner_editor::system_prompt(&lang)));
    Ok(vm)
}

// ( category chapter -- )  declare a category deliberate; empty chapter = project-wide.
fn w_intent_declare(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_intent_declare(vm).map_err(to_bund_err)
}
fn do_intent_declare(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.inner_editor.intent.declare";
    require_depth(vm, 2, tag)?;
    let chapter = value_to_string(pull(vm, tag)?, "chapter", tag)?;
    let category = value_to_string(pull(vm, tag)?, "category", tag)?;
    let cat = EditorCategory::from_id(&category)
        .ok_or_else(|| anyhow!("{tag}: unknown category `{category}`"))?;
    let chapter = if chapter.trim().is_empty() { None } else { Some(chapter.trim()) };
    inner_editor::intent_declare::declare_intent(active_store(tag)?.project_root(), cat, chapter, None)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

// ( paragraph_id -- list )  run one Editor engagement; push its findings as dicts.
fn w_engage(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_engage(vm).map_err(to_bund_err)
}
fn do_engage(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.inner_editor.engage";
    require_depth(vm, 1, tag)?;
    let pid_s = value_to_string(pull(vm, tag)?, "paragraph_id", tag)?;
    let pid = Uuid::parse_str(pid_s.trim()).map_err(|e| anyhow!("{tag}: bad paragraph id: {e}"))?;
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let gc = inner_editor::gather_context(store, &hierarchy, pid, cfg.inner_editor.context.preceding_paragraphs)
        .ok_or_else(|| anyhow!("{tag}: paragraph `{pid}` not found"))?;
    let outcome = inner_editor::engage(inner_editor::EngageInput {
        project: store.project_root().to_path_buf(),
        paragraph_id: Some(pid),
        chapter_id: gc.chapter_id,
        prose: gc.prose,
        preceding: gc.preceding,
        language: String::new(),
        snapshot_id: None,
        system_override: None,
        force: true,
    })
    .map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = outcome
        .findings
        .iter()
        .map(|f| {
            let mut h = HashMap::new();
            h.insert("category".to_string(), Value::from_string(f.category.id()));
            h.insert("severity".to_string(), Value::from_string(f.severity.id()));
            h.insert("observation".to_string(), Value::from_string(f.observation.clone()));
            h.insert("conditional".to_string(), Value::from_bool(f.conditional));
            if let Some(ev) = &f.evidence {
                h.insert("evidence".to_string(), Value::from_string(ev.clone()));
            }
            Value::from_dict(h)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}
