//! CHAR-1 — `ink.char.*` Bund stdlib: character-arc tracking from scripts.
//! Mirrors `ink.dialogue.*` (DIALOG-1) / `ink.prose.*` (NARR-1).
//!
//! - `ink.char.arc`     ( -- list )  per-character arc summary (declaration +
//!   state-chain length + mean agency).
//! - `ink.char.stalls`  ( -- list )  stall findings (deterministic, from the
//!   cached state chain).
//! - `ink.char.checks`  ( -- list )  cached arc-completeness checks.
//! - `ink.char.plan`    ( -- list )  Planning-Board coverage gaps (recomputed).
//! - `ink.char.refresh` ( -- count ) recompute the deterministic layers (agency
//!   + planning) → number of (character, chapter) agency cells.
//!
//! Every word is deterministic / zero-AI (the LLM passes stay on the CLI). The
//! readers READ `char.duckdb`; `plan`/`refresh` write only that derived cache
//! (not the manuscript), so all words are classified `store_read`, exactly like
//! `ink.dialogue.refresh`.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, push};
use crate::character::{CharStore, detect_stall, run_agency, run_planning};
use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.char.arc", w_arc),
        ("ink.char.stalls", w_stalls),
        ("ink.char.checks", w_checks),
        ("ink.char.plan", w_plan),
        ("ink.char.refresh", w_refresh),
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

/// Resolve `(store, cfg, hierarchy, single-user-book, char-store)`.
fn ctx(tag: &str) -> Result<(&'static Store, &'static Config, Hierarchy, Node, CharStore)> {
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = crate::cli::resolve_user_book(&h, None, tag)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .clone();
    let cs = CharStore::open(store.project_root()).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok((store, cfg, h, book, cs))
}

/// Mirror the author's `character_arc` declarations into the store, then return
/// the tracked-character roster (declared ∪ have-state).
fn tracked_names(cs: &CharStore, h: &Hierarchy, layout: &ProjectLayout, book_slug: &str) -> Result<Vec<String>> {
    for (decl, hash) in crate::character::read_arc_declarations(h, layout) {
        let _ = cs.upsert_declaration(book_slug, &decl, hash);
    }
    let mut names: Vec<String> = cs
        .all_declarations(book_slug)
        .map_err(|e| anyhow!("{e}"))?
        .into_iter()
        .map(|d| d.character_name)
        .collect();
    if let Ok(more) = cs.characters_with_states(book_slug) {
        for n in more {
            if !names.iter().any(|x| x.eq_ignore_ascii_case(&n)) {
                names.push(n);
            }
        }
    }
    Ok(names)
}

// ── words ────────────────────────────────────────────────────────────────────

fn w_arc(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_arc(vm).map_err(to_bund_err)
}
fn do_arc(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.char.arc";
    let (store, _c, h, book, cs) = ctx(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let names = tracked_names(&cs, &h, &layout, &book.slug)?;
    let mut items = Vec::new();
    for name in &names {
        let decl = cs.declaration(&book.slug, name).map_err(|e| anyhow!("{tag}: {e}"))?;
        let states = cs.states_for_character(&book.slug, name).map_err(|e| anyhow!("{tag}: {e}"))?;
        let agencies: Vec<f32> = states.iter().filter_map(|s| s.agency_score).collect();
        let mean_agency =
            (!agencies.is_empty()).then(|| agencies.iter().sum::<f32>() / agencies.len() as f32);
        let mut m: HashMap<String, Value> = HashMap::new();
        m.insert("character".into(), Value::from_string(name));
        m.insert(
            "arc_type".into(),
            decl.as_ref().map(|d| Value::from_string(d.arc_type.as_code())).unwrap_or_else(Value::nodata),
        );
        m.insert("chapters".into(), Value::from_int(states.len() as i64));
        m.insert(
            "changes".into(),
            Value::from_int(states.iter().filter(|s| s.changed).count() as i64),
        );
        m.insert(
            "mean_agency".into(),
            mean_agency.map(|a| Value::from_float(a as f64)).unwrap_or_else(Value::nodata),
        );
        items.push(Value::from_dict(m));
    }
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_stalls(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_stalls(vm).map_err(to_bund_err)
}
fn do_stalls(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.char.stalls";
    let (store, cfg, h, book, cs) = ctx(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let names = tracked_names(&cs, &h, &layout, &book.slug)?;
    let mut items = Vec::new();
    for name in &names {
        let states = cs.states_for_character(&book.slug, name).map_err(|e| anyhow!("{tag}: {e}"))?;
        if let Some(stall) = detect_stall(name, &states, cfg.char.stall_threshold) {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("character".into(), Value::from_string(name));
            m.insert(
                "chapter".into(),
                stall.chapter_ord.map(|o| Value::from_int(o as i64)).unwrap_or_else(Value::nodata),
            );
            m.insert("description".into(), Value::from_string(&stall.description));
            items.push(Value::from_dict(m));
        }
    }
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_checks(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_checks(vm).map_err(to_bund_err)
}
fn do_checks(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.char.checks";
    let (store, _c, h, book, cs) = ctx(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let names = tracked_names(&cs, &h, &layout, &book.slug)?;
    let mut items = Vec::new();
    for name in &names {
        let checks = cs.checks_for_character(&book.slug, name).map_err(|e| anyhow!("{tag}: {e}"))?;
        for c in &checks {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("character".into(), Value::from_string(name));
            m.insert("check".into(), Value::from_string(c.check_type.as_code()));
            m.insert("verdict".into(), Value::from_string(c.verdict.as_code()));
            m.insert("problem".into(), Value::from_bool(c.verdict.is_problem()));
            m.insert(
                "chapter".into(),
                c.chapter_ord.map(|o| Value::from_int(o as i64)).unwrap_or_else(Value::nodata),
            );
            m.insert("description".into(), Value::from_string(&c.description));
            items.push(Value::from_dict(m));
        }
    }
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_plan(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_plan(vm).map_err(to_bund_err)
}
fn do_plan(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.char.plan";
    let (store, _c, h, book, cs) = ctx(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    // Sync declarations, then recompute the deterministic Planning-Board gaps.
    let _ = tracked_names(&cs, &h, &layout, &book.slug)?;
    run_planning(&cs, store, &h, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let findings = cs.planning_findings(&book.slug).map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = findings
        .iter()
        .map(|(name, ft, desc)| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("character".into(), Value::from_string(name));
            m.insert("type".into(), Value::from_string(ft));
            m.insert("description".into(), Value::from_string(desc));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_refresh(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_refresh(vm).map_err(to_bund_err)
}
fn do_refresh(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.char.refresh";
    let (store, cfg, h, book, cs) = ctx(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let _ = tracked_names(&cs, &h, &layout, &book.slug)?;
    let cells = run_agency(&cs, &layout, &h, cfg, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    run_planning(&cs, store, &h, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_int(cells as i64));
    Ok(vm)
}
