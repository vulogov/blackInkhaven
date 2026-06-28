//! DIALOG-1 — `ink.dialogue.*` Bund stdlib: dialogue quality & attribution from
//! scripts. Mirrors `ink.prose.*` (NARR-1).
//!
//! - `ink.dialogue.stats`       ( -- list ) per-chapter dialogue stats.
//! - `ink.dialogue.fingerprint` ( -- list ) per-character fingerprints.
//! - `ink.dialogue.violations`  ( -- list ) chapter ordinals with a violation.
//! - `ink.dialogue.spans`       ( -- list ) every detected span (book-wide).
//! - `ink.dialogue.refresh`     ( -- count ) recompute (hash-lazy) → #findings.
//!
//! All deterministic / zero-AI. The first four READ `dialogue.duckdb`;
//! `refresh` writes only that derived cache (not the manuscript), so every word
//! is classified `store_read`, exactly like `ink.prose.refresh`.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, push};
use crate::config::Config;
use crate::dialogue::{DialogueStore, refresh_book};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.dialogue.stats", w_stats),
        ("ink.dialogue.fingerprint", w_fingerprint),
        ("ink.dialogue.violations", w_violations),
        ("ink.dialogue.spans", w_spans),
        ("ink.dialogue.refresh", w_refresh),
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

/// Resolve `(store, cfg, hierarchy, single-user-book, dialogue-store)`.
fn ctx(tag: &str) -> Result<(&'static Store, &'static Config, Hierarchy, Node, DialogueStore)> {
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = crate::cli::resolve_user_book(&h, None, tag)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .clone();
    let ds = DialogueStore::open(store.project_root()).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok((store, cfg, h, book, ds))
}

fn opt_str(o: &Option<String>) -> Value {
    match o {
        Some(s) => Value::from_string(s),
        None => Value::nodata(),
    }
}

// ── words ────────────────────────────────────────────────────────────────────

fn w_stats(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_stats(vm).map_err(to_bund_err)
}
fn do_stats(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.dialogue.stats";
    let (_s, _c, _h, book, ds) = ctx(tag)?;
    let stats = ds.all_chapter_stats(&book.slug).map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = stats
        .iter()
        .map(|s| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("chapter".into(), Value::from_int(s.chapter_ord as i64));
            m.insert("total_spans".into(), Value::from_int(s.total_spans as i64));
            m.insert("zero_attribution".into(), Value::from_int(s.zero_attribution_count as i64));
            m.insert("said_bookism".into(), Value::from_int(s.said_bookism_count as i64));
            m.insert("said_bookism_density".into(), Value::from_float(s.said_bookism_density as f64));
            m.insert("talking_head_sequences".into(), Value::from_int(s.talking_head_sequences as i64));
            m.insert("dialogue_density_ratio".into(), Value::from_float(s.dialogue_density_ratio as f64));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_fingerprint(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_fingerprint(vm).map_err(to_bund_err)
}
fn do_fingerprint(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.dialogue.fingerprint";
    let (_s, _c, _h, book, ds) = ctx(tag)?;
    let fps = ds.all_fingerprints(&book.slug).map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = fps
        .iter()
        .map(|f| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("character".into(), Value::from_string(&f.character_name));
            m.insert("utterance_count".into(), Value::from_int(f.utterance_count as i64));
            m.insert("mean_utterance_words".into(), Value::from_float(f.mean_utterance_words as f64));
            m.insert("mattr".into(), Value::from_float(f.utterance_mattr as f64));
            m.insert("question_ratio".into(), Value::from_float(f.question_ratio as f64));
            m.insert("exclamation_ratio".into(), Value::from_float(f.exclamation_ratio as f64));
            m.insert("hedge_density".into(), Value::from_float(f.hedge_density as f64));
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
    let tag = "ink.dialogue.violations";
    let (_s, _c, _h, book, ds) = ctx(tag)?;
    let stats = ds.all_chapter_stats(&book.slug).map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = stats
        .iter()
        .filter(|s| s.zero_attribution_count > 0 || s.talking_head_sequences > 0)
        .map(|s| Value::from_int(s.chapter_ord as i64))
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_spans(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_spans(vm).map_err(to_bund_err)
}
fn do_spans(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.dialogue.spans";
    let (_s, _c, _h, book, ds) = ctx(tag)?;
    let stats = ds.all_chapter_stats(&book.slug).map_err(|e| anyhow!("{tag}: {e}"))?;
    let mut items = Vec::new();
    for s in &stats {
        let spans = ds
            .spans_for_chapter(&book.slug, s.chapter_ord)
            .map_err(|e| anyhow!("{tag}: {e}"))?;
        for sp in &spans {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("chapter".into(), Value::from_int(s.chapter_ord as i64));
            m.insert("para_id".into(), Value::from_string(&sp.para_id));
            m.insert("form".into(), Value::from_string(sp.form.as_code()));
            m.insert("speech".into(), Value::from_string(&sp.speech_text));
            m.insert("word_count".into(), Value::from_int(sp.word_count as i64));
            m.insert("attribution".into(), opt_str(&sp.attribution_name));
            m.insert("attribution_conf".into(), Value::from_string(sp.attribution_conf.as_code()));
            m.insert("tag_verb".into(), opt_str(&sp.tag_verb));
            items.push(Value::from_dict(m));
        }
    }
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_refresh(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_refresh(vm).map_err(to_bund_err)
}
fn do_refresh(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.dialogue.refresh";
    let (store, cfg, h, book, ds) = ctx(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let now = chrono::Utc::now().to_rfc3339();
    let findings =
        refresh_book(&ds, &layout, &h, cfg, &book, None, &now).map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_int(findings.len() as i64));
    Ok(vm)
}
