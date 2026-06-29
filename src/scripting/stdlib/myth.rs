//! MYTH-1 (M-P11) — `ink.myth.*` Bund stdlib. Mirrors `ink.theologian.*`. The
//! deterministic surface is `store_read` (writes only the derived caches); the
//! LLM checks stay explicit (`inkhaven myth check` / TUI), so they are not Bund
//! words. `suppress` is the one `store_write`.
//!
//! - `ink.myth.symbols`    ( -- list )   declared symbols.
//! - `ink.myth.motifs`     ( -- list )   declared motifs.
//! - `ink.myth.archetypes` ( -- list )   declared archetypes.
//! - `ink.myth.density`    ( -- list )   per-symbol per-chapter occurrence counts.
//! - `ink.myth.findings`   ( -- list )   recompute deterministic findings + return.
//! - `ink.myth.suppress`   ( finding -- bool )  suppress a finding by id.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, pull, push, value_to_string};
use crate::config::Config;
use crate::myth::MythStore;
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

// Deterministic-finding final-act window (mirrors the CLI default until M-P15).
const FINAL_ACT_PCT: u32 = 25;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.myth.symbols", w_symbols),
        ("ink.myth.motifs", w_motifs),
        ("ink.myth.archetypes", w_archetypes),
        ("ink.myth.density", w_density),
        ("ink.myth.findings", w_findings),
        ("ink.myth.suppress", w_suppress),
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

/// Resolve `(store, cfg, hierarchy, single-user-book, layout, myth-store)`.
fn ctx(tag: &str) -> Result<(&'static Store, &'static Config, Hierarchy, Node, ProjectLayout, MythStore)> {
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = crate::cli::resolve_user_book(&h, None, tag).map_err(|e| anyhow!("{tag}: {e}"))?.clone();
    let layout = ProjectLayout::new(store.project_root());
    let ms = MythStore::open(store.project_root()).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok((store, cfg, h, book, layout, ms))
}

fn w_symbols(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_symbols(vm).map_err(to_bund_err)
}
fn do_symbols(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.myth.symbols";
    let (_s, _c, h, book, layout, ms) = ctx(tag)?;
    crate::myth::refresh_inventory(&ms, &layout, &h, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = ms
        .symbols(&book.slug)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .iter()
        .map(|s| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("para_id".into(), Value::from_string(&s.para_id));
            m.insert(
                "vocabulary".into(),
                Value::from_list(s.vocabulary.iter().map(Value::from_string).collect()),
            );
            m.insert("meaning".into(), Value::from_string(&s.meaning));
            m.insert("valence".into(), Value::from_string(s.valence.as_code()));
            m.insert(
                "traditions".into(),
                Value::from_list(s.traditions.iter().map(Value::from_string).collect()),
            );
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_motifs(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_motifs(vm).map_err(to_bund_err)
}
fn do_motifs(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.myth.motifs";
    let (_s, _c, h, book, layout, ms) = ctx(tag)?;
    crate::myth::refresh_inventory(&ms, &layout, &h, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = ms
        .motifs(&book.slug)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .iter()
        .map(|m| {
            let mut d: HashMap<String, Value> = HashMap::new();
            d.insert("para_id".into(), Value::from_string(&m.para_id));
            d.insert("name".into(), Value::from_string(&m.name));
            d.insert("description".into(), Value::from_string(&m.description));
            d.insert("valence".into(), Value::from_string(m.valence.as_code()));
            Value::from_dict(d)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_archetypes(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_archetypes(vm).map_err(to_bund_err)
}
fn do_archetypes(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.myth.archetypes";
    let (_s, _c, h, book, layout, ms) = ctx(tag)?;
    crate::myth::refresh_inventory(&ms, &layout, &h, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = ms
        .archetypes(&book.slug)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .iter()
        .map(|a| {
            let mut d: HashMap<String, Value> = HashMap::new();
            d.insert("para_id".into(), Value::from_string(&a.para_id));
            d.insert("role".into(), Value::from_string(a.role.as_code()));
            d.insert("character".into(), Value::from_string(&a.character_name));
            d.insert("function".into(), Value::from_string(&a.function_desc));
            Value::from_dict(d)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_density(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_density(vm).map_err(to_bund_err)
}
fn do_density(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.myth.density";
    let (_s, _c, h, book, layout, ms) = ctx(tag)?;
    crate::myth::refresh_inventory(&ms, &layout, &h, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    crate::myth::run_density_scan(&ms, &layout, &h, &book, false).map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = ms
        .symbols(&book.slug)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .iter()
        .map(|s| {
            let cells: Vec<Value> = ms
                .density_for_symbol(&book.slug, &s.para_id)
                .unwrap_or_default()
                .iter()
                .map(|(ord, c)| {
                    let mut cd: HashMap<String, Value> = HashMap::new();
                    cd.insert("chapter".into(), Value::from_int(*ord as i64));
                    cd.insert("count".into(), Value::from_int(*c as i64));
                    Value::from_dict(cd)
                })
                .collect();
            let mut d: HashMap<String, Value> = HashMap::new();
            d.insert(
                "symbol".into(),
                Value::from_string(s.vocabulary.first().map(|x| x.as_str()).unwrap_or("")),
            );
            d.insert("para_id".into(), Value::from_string(&s.para_id));
            d.insert("chapters".into(), Value::from_list(cells));
            Value::from_dict(d)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_findings(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_findings(vm).map_err(to_bund_err)
}
fn do_findings(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.myth.findings";
    let (_s, _c, h, book, layout, ms) = ctx(tag)?;
    crate::myth::refresh_inventory(&ms, &layout, &h, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    crate::myth::run_density_scan(&ms, &layout, &h, &book, false).map_err(|e| anyhow!("{tag}: {e}"))?;
    crate::myth::collect_explicit_motifs(&ms, &layout, &h, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    crate::myth::run_deterministic_checks(&ms, &layout, &h, &book, FINAL_ACT_PCT)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = ms
        .findings_with_ids(&book.slug, false)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .iter()
        .map(|(id, f)| {
            let mut d: HashMap<String, Value> = HashMap::new();
            d.insert("id".into(), Value::from_string(id));
            d.insert("type".into(), Value::from_string(f.finding_type.as_code()));
            d.insert("description".into(), Value::from_string(&f.description));
            if let Some(ev) = &f.evidence {
                d.insert("evidence".into(), Value::from_string(ev));
            }
            if let Some(ep) = &f.entry_para_id {
                d.insert("entry_para_id".into(), Value::from_string(ep));
            }
            Value::from_dict(d)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_suppress(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_suppress(vm).map_err(to_bund_err)
}
fn do_suppress(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.myth.suppress";
    let finding = value_to_string(pull(vm, tag)?, "finding", tag)?;
    let (_s, _c, _h, book, _layout, ms) = ctx(tag)?;
    let ok = ms.suppress_finding(&book.slug, &finding).map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_bool(ok));
    Ok(vm)
}
