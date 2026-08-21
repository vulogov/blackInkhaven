//! 3.0.4 — `ink.research.*` Bund stdlib: the research evidence base, read-only.
//! Bund inspects the Facts book (disputed vs undisputed), a fact's provenance,
//! the ingested source chunks near a query, and the persisted SCHOLAR report —
//! composable into "state of the evidence" scripts. Everything reads the active
//! store or a `.inkhaven/` sidecar; the network ingest, the LLM contradiction
//! scans, and the confirmation-gated fact writes all stay CLI-only.
//!
//! - `ink.research.facts`      ( -- list )       disputed Facts as {id, location, text}.
//! - `ink.research.undisputed` ( -- list )       the fact:undisputed authorial facts.
//! - `ink.research.provenance` ( node-id -- dict | NODATA )  where a fact came from.
//! - `ink.research.sources`    ( query k -- list )  source chunks near `query` {name, body}.
//! - `ink.research.report`     ( -- string )     the persisted SCHOLAR report.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;
use uuid::Uuid;

use super::helpers::{
    active_config, active_store, pull, push, require_depth, value_to_i64, value_to_string,
};
use crate::project::ProjectLayout;
use crate::research::factcheck::{self, FactEntry};
use crate::research::provenance::Provenance;
use crate::research::{rag, report_render};
use crate::store::hierarchy::Hierarchy;
use crate::store::{NodeKind, Store, SYSTEM_TAG_FACTS};

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.research.facts", w_facts),
        ("ink.research.undisputed", w_undisputed),
        ("ink.research.provenance", w_provenance),
        ("ink.research.sources", w_sources),
        ("ink.research.report", w_report),
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

/// The Facts system book id, if the project has one. Read via the normal
/// Hierarchy — Facts is an ordinary system book, no separate database.
fn facts_book(h: &Hierarchy) -> Option<Uuid> {
    h.iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_FACTS))
        .map(|n| n.id)
}

fn fact_dict(f: &FactEntry) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("id".into(), Value::from_string(&f.id.to_string()));
    m.insert("location".into(), Value::from_string(&f.location));
    m.insert("text".into(), Value::from_string(&f.text));
    Value::from_dict(m)
}

/// Shared body for `facts` / `undisputed` — resolve the Facts book off the
/// active store and gather the requested set.
fn gather(tag: &str, undisputed: bool) -> Result<Vec<FactEntry>> {
    let store: &Store = active_store(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let Some(book) = facts_book(&h) else { return Ok(Vec::new()) };
    Ok(if undisputed {
        factcheck::gather_undisputed(store, &h, book)
    } else {
        factcheck::gather_facts(store, &h, book)
    })
}

word!(w_facts, do_facts);
fn do_facts(vm: &mut VM) -> Result<&mut VM> {
    let facts = gather("ink.research.facts", false)?;
    push(vm, Value::from_list(facts.iter().map(fact_dict).collect()));
    Ok(vm)
}

word!(w_undisputed, do_undisputed);
fn do_undisputed(vm: &mut VM) -> Result<&mut VM> {
    let facts = gather("ink.research.undisputed", true)?;
    push(vm, Value::from_list(facts.iter().map(fact_dict).collect()));
    Ok(vm)
}

word!(w_provenance, do_provenance);
fn do_provenance(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.research.provenance";
    require_depth(vm, 1, tag)?;
    let node_id = value_to_string(pull(vm, tag)?, "node-id", tag)?;
    let store = active_store(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let prov = Provenance::load(&layout);
    let out = match prov.for_node(&node_id) {
        Some(rec) => {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("origin".into(), Value::from_string(&rec.origin));
            m.insert("detail".into(), Value::from_string(&rec.detail));
            m.insert("query".into(), Value::from_string(&rec.query));
            m.insert("thread".into(), Value::from_string(&rec.thread));
            m.insert("created_at".into(), Value::from_string(&rec.created_at));
            m.insert("summary".into(), Value::from_string(&rec.summary()));
            Value::from_dict(m)
        }
        None => Value::nodata(),
    };
    push(vm, out);
    Ok(vm)
}

/// Upper bound on the requested chunk count. `rag::retrieve_source_passages`
/// searches for `k * 4 + 8` candidates, so an unclamped user `k` would overflow
/// `usize` (panic in debug, oversized allocation in release). 200 is far more
/// source chunks than any dashboard needs.
const MAX_SOURCE_K: i64 = 200;

word!(w_sources, do_sources);
fn do_sources(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.research.sources";
    require_depth(vm, 2, tag)?;
    let k = value_to_i64(pull(vm, tag)?, "k", tag)?.clamp(0, MAX_SOURCE_K) as usize;
    let query = value_to_string(pull(vm, tag)?, "query", tag)?;
    let store = active_store(tag)?;
    let items: Vec<Value> = rag::retrieve_source_passages(store, &query, k)
        .iter()
        .map(|p| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("name".into(), Value::from_string(&p.name));
            m.insert("body".into(), Value::from_string(&p.body));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

word!(w_report, do_report);
fn do_report(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.research.report";
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let layout = ProjectLayout::new(store.project_root());
    let text = report_render(store, &h, &layout, facts_book(&h), &cfg.language);
    push(vm, Value::from_string(text));
    Ok(vm)
}
