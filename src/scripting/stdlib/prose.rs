//! NARR-1 — `ink.prose.*` Bund stdlib: narrative-voice profiling from scripts.
//!
//! - `ink.prose.profile`    ( -- list )  stored per-scope voice profiles.
//! - `ink.prose.drift`      ( -- list )  per-chapter deltas vs the baseline.
//! - `ink.prose.violations` ( -- list )  threshold crossings vs the baseline.
//! - `ink.prose.refresh`    ( -- count ) recompute (hash-lazy) + return count.
//!
//! The first three READ the stored profiles (`prose.duckdb`); `refresh`
//! recomputes them. All zero-AI. `null` (not `0.0`) is returned for
//! language-sensitive metrics on an unsupported-language book.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, push};
use crate::config::Config;
use crate::project::ProjectLayout;
use crate::prose::{ProseStore, VoiceProfile, VoiceScope, refresh_book};
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.prose.profile", w_profile),
        ("ink.prose.drift", w_drift),
        ("ink.prose.violations", w_violations),
        ("ink.prose.refresh", w_refresh),
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

/// Resolve `(store, cfg, hierarchy, single-user-book, prose-store)`.
fn ctx(tag: &str) -> Result<(&'static Store, &'static Config, Hierarchy, Node, ProseStore)> {
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = crate::cli::resolve_user_book(&h, None, tag)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .clone();
    let pstore = ProseStore::open(store.project_root()).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok((store, cfg, h, book, pstore))
}

fn opt(o: Option<f32>) -> Value {
    match o {
        Some(v) => Value::from_float(v as f64),
        None => Value::nodata(),
    }
}

fn dopt(a: Option<f32>, b: Option<f32>) -> Value {
    match (a, b) {
        (Some(a), Some(b)) => Value::from_float((a - b) as f64),
        _ => Value::nodata(),
    }
}

fn profile_to_value(p: &VoiceProfile) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("scope".into(), Value::from_string(&p.scope.as_str()));
    m.insert(
        "chapter".into(),
        match p.scope.chapter_ord() {
            Some(n) => Value::from_int(n as i64),
            None => Value::nodata(),
        },
    );
    m.insert("language".into(), Value::from_string(p.prose_language.as_code()));
    m.insert("word_count".into(), Value::from_int(p.word_count as i64));
    m.insert("sentence_count".into(), Value::from_int(p.sentence_count as i64));
    m.insert("cv".into(), Value::from_float(p.cv as f64));
    m.insert("burstiness".into(), Value::from_float(p.burstiness as f64));
    m.insert("mattr".into(), Value::from_float(p.mattr as f64));
    m.insert("modal_density".into(), opt(p.modal_density));
    m.insert("interiority_ratio".into(), opt(p.interiority_ratio));
    m.insert(
        "de_erlebte_rede_particle_density".into(),
        opt(p.de_erlebte_rede_particle_density),
    );
    let tier2 = match &p.tier2 {
        Some(t) => {
            let mut t2: HashMap<String, Value> = HashMap::new();
            t2.insert("sensory_visual".into(), Value::from_float(t.sensory[0] as f64));
            t2.insert("sensory_auditory".into(), Value::from_float(t.sensory[1] as f64));
            t2.insert("sensory_olfactory".into(), Value::from_float(t.sensory[2] as f64));
            t2.insert("sensory_tactile".into(), Value::from_float(t.sensory[3] as f64));
            t2.insert("sensory_kinesthetic".into(), Value::from_float(t.sensory[4] as f64));
            t2.insert(
                "active_passive_ratio".into(),
                Value::from_float(t.active_passive_ratio as f64),
            );
            Value::from_dict(t2)
        }
        None => Value::nodata(),
    };
    m.insert("tier2".into(), tier2);
    Value::from_dict(m)
}

// ── words ────────────────────────────────────────────────────────────────────

fn w_profile(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_profile(vm).map_err(to_bund_err)
}
fn do_profile(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.prose.profile";
    let (_store, _cfg, _h, book, pstore) = ctx(tag)?;
    let profiles = pstore.get_all(&book.slug).map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = profiles.iter().map(profile_to_value).collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_drift(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_drift(vm).map_err(to_bund_err)
}
fn do_drift(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.prose.drift";
    let (_store, cfg, _h, book, pstore) = ctx(tag)?;
    let profiles = pstore.get_all(&book.slug).map_err(|e| anyhow!("{tag}: {e}"))?;
    let base_ord = cfg.prose.baseline_chapter;
    let base = profiles.iter().find(|p| p.scope == VoiceScope::Chapter(base_ord));
    let mut items = Vec::new();
    if let Some(base) = base {
        for p in &profiles {
            let Some(ord) = p.scope.chapter_ord() else { continue };
            if ord == base_ord {
                continue;
            }
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("chapter".into(), Value::from_int(ord as i64));
            m.insert("d_cv".into(), Value::from_float((p.cv - base.cv) as f64));
            m.insert("d_mattr".into(), Value::from_float((p.mattr - base.mattr) as f64));
            m.insert("d_modal_density".into(), dopt(p.modal_density, base.modal_density));
            m.insert(
                "d_interiority_ratio".into(),
                dopt(p.interiority_ratio, base.interiority_ratio),
            );
            items.push(Value::from_dict(m));
        }
    }
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_violations(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_violations(vm).map_err(to_bund_err)
}
fn do_violations(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.prose.violations";
    let (_store, cfg, _h, book, pstore) = ctx(tag)?;
    let profiles = pstore.get_all(&book.slug).map_err(|e| anyhow!("{tag}: {e}"))?;
    let viols =
        crate::prose::violations::violations(&profiles, cfg.prose.baseline_chapter, &cfg.prose.thresholds);
    let items: Vec<Value> = viols
        .iter()
        .map(|v| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("chapter".into(), Value::from_int(v.chapter as i64));
            m.insert("metric".into(), Value::from_string(v.metric));
            m.insert("baseline".into(), Value::from_float(v.baseline as f64));
            m.insert("value".into(), Value::from_float(v.value as f64));
            m.insert("delta".into(), Value::from_float(v.delta as f64));
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
    let tag = "ink.prose.refresh";
    let (store, cfg, h, book, pstore) = ctx(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let now = chrono::Utc::now().to_rfc3339();
    let profiles = refresh_book(
        &pstore,
        &layout,
        &h,
        cfg,
        &book,
        cfg.prose.language.as_deref(),
        cfg.prose.deep_metrics,
        cfg.prose.mattr_window,
        &now,
    )
    .map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_int(profiles.len() as i64));
    Ok(vm)
}
