//! INNER-THEOLOGIAN-1 (IT-P10) — `ink.theologian.*` Bund stdlib. Mirrors
//! `ink.char.*`. The fast-track is deterministic / zero-AI; the slow-track LLM
//! session stays interactive (`Ctrl+B J→T` / `inkhaven theologian session`), so
//! the Bund surface is just the two words a submission-gate script needs.
//!
//! - `ink.theologian.signals`  ( -- list )  recompute + return unsuppressed
//!   fast-track signals. `store_read` (writes only the derived cache).
//! - `ink.theologian.suppress` ( para -- count )  suppress a paragraph's
//!   signals; pushes how many were affected. `store_write`.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, pull, push, value_to_string};
use crate::config::Config;
use crate::inner_theologian::{DetectWindows, TheologianStore, run_fast_scan};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.theologian.signals", w_signals),
        ("ink.theologian.suppress", w_suppress),
        // Namespace consistency with ink.inner_socrates.* / ink.inner_editor.* —
        // registered as real, policy-gated words (not aliases, which would skip
        // the store_write gate on `suppress`). The shorter names stay for compat.
        ("ink.inner_theologian.signals", w_signals),
        ("ink.inner_theologian.suppress", w_suppress),
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

/// Resolve `(store, cfg, hierarchy, single-user-book, theologian-store)`.
fn ctx(tag: &str) -> Result<(&'static Store, &'static Config, Hierarchy, Node, TheologianStore)> {
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = crate::cli::resolve_user_book(&h, None, tag)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .clone();
    let ts = TheologianStore::open(store.project_root()).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok((store, cfg, h, book, ts))
}

fn w_signals(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_signals(vm).map_err(to_bund_err)
}
fn do_signals(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.theologian.signals";
    let (store, cfg, h, book, ts) = ctx(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let win = DetectWindows {
        moral_invisibility: cfg.theologian.moral_invisibility_window,
        consequence_gap: cfg.theologian.consequence_gap_window,
    };
    run_fast_scan(&ts, &layout, &h, cfg, &book, win, cfg.theologian.sacred_levity_signal)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    let findings = ts.findings(&book.slug, false).map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = findings
        .iter()
        .map(|f| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("signal_type".into(), Value::from_string(f.signal_type.as_code()));
            m.insert("chapter_ord".into(), Value::from_int(f.chapter_ord as i64));
            m.insert("para_id".into(), Value::from_string(&f.para_id));
            m.insert("description".into(), Value::from_string(&f.description));
            m.insert("suppressed".into(), Value::from_bool(f.suppressed));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn w_suppress(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_suppress(vm).map_err(to_bund_err)
}
fn do_suppress(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.theologian.suppress";
    let para = value_to_string(pull(vm, tag)?, "para", tag)?;
    let (_store, _cfg, _h, book, ts) = ctx(tag)?;
    let n = ts.suppress_paragraph(&book.slug, &para).map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_int(n as i64));
    Ok(vm)
}
