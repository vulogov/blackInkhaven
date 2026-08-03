//! SEMNET — `ink.graph.*` Bund stdlib: read (and triage) the knowledge graph
//! from scripts. Deterministic; the LLM `graph ask` / `graph link` are not
//! exposed here (a network call can't run inline on the VM thread).
//!
//! - `ink.graph.stats`        ( -- dict )            node/edge counts + per-kind.
//! - `ink.graph.neighbors`    ( node -- list )       one-hop edges of a node.
//! - `ink.graph.contradicting`( node -- list )       stance clashes touching it.
//! - `ink.graph.loci`         ( node -- list )        cited primary-source loci.
//! - `ink.graph.paths`        ( from to -- list|nil ) bounded citation/link path.
//! - `ink.graph.pending`      ( -- list )             the judged edge inbox.
//! - `ink.graph.rebuild`      ( -- dict )             re-derive structural edges.
//! - `ink.graph.promote`      ( edge -- bool )        judged → promoted.
//! - `ink.graph.dismiss`      ( edge -- )             delete a stance edge.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;
use uuid::Uuid;

use super::helpers::{active_config, active_store, pull, push, value_to_uuid};
use crate::store::Store;
use crate::store::graph::{EdgeKind, EndpointRef};
use crate::store::hierarchy::Hierarchy;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.graph.stats", w_stats),
        ("ink.graph.neighbors", w_neighbors),
        ("ink.graph.contradicting", w_contradicting),
        ("ink.graph.loci", w_loci),
        ("ink.graph.paths", w_paths),
        ("ink.graph.pending", w_pending),
        ("ink.graph.rebuild", w_rebuild),
        ("ink.graph.promote", w_promote),
        ("ink.graph.dismiss", w_dismiss),
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

fn endpoint_value(ep: &EndpointRef, h: &Hierarchy) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    match ep {
        EndpointRef::Node(u) => {
            m.insert("type".into(), Value::from_string("node"));
            m.insert("id".into(), Value::from_string(&u.to_string()));
            let label = h
                .get(*u)
                .map(|n| n.title.clone())
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| format!("node {}", &u.to_string()[..8]));
            m.insert("label".into(), Value::from_string(&label));
        }
        EndpointRef::Extern(_) => {
            let (k, r) = ep.as_columns();
            m.insert("type".into(), Value::from_string("extern"));
            m.insert("kind".into(), Value::from_string(&k));
            m.insert("ref".into(), Value::from_string(&r));
        }
    }
    Value::from_dict(m)
}

/// One edge relative to `focus`, as a dict.
fn edge_value(e: &crate::store::graph::Edge, focus: Uuid, h: &Hierarchy) -> Value {
    let here = EndpointRef::Node(focus);
    let dir = if !e.directed {
        "sym"
    } else if e.src == here {
        "out"
    } else {
        "in"
    };
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("kind".into(), Value::from_string(e.kind.as_str()));
    m.insert("dir".into(), Value::from_string(dir));
    m.insert("other".into(), endpoint_value(e.other_endpoint(&here), h));
    m.insert(
        "reason".into(),
        match e.reason.as_deref().filter(|r| !r.is_empty()) {
            Some(r) => Value::from_string(r),
            None => Value::nodata(),
        },
    );
    Value::from_dict(m)
}

fn ctx(tag: &str) -> Result<(&'static Store, Hierarchy)> {
    let store = active_store(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok((store, h))
}

fn node_arg(vm: &mut VM, tag: &str) -> Result<Uuid> {
    value_to_uuid(pull(vm, tag)?, tag)
}

macro_rules! word {
    ($w:ident, $do:ident) => {
        fn $w(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
            $do(vm).map_err(to_bund_err)
        }
    };
}

word!(w_stats, do_stats);
fn do_stats(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.graph.stats";
    let (store, _h) = ctx(tag)?;
    let s = store.graph_stats().map_err(|e| anyhow!("{tag}: {e}"))?;
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("nodes".into(), Value::from_int(s.nodes as i64));
    m.insert("edges".into(), Value::from_int(s.edges as i64));
    let by_kind: HashMap<String, Value> =
        s.by_kind.iter().map(|(k, n)| (k.clone(), Value::from_int(*n as i64))).collect();
    m.insert("by_kind".into(), Value::from_dict(by_kind));
    push(vm, Value::from_dict(m));
    Ok(vm)
}

word!(w_neighbors, do_neighbors);
fn do_neighbors(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.graph.neighbors";
    let id = node_arg(vm, tag)?;
    let (store, h) = ctx(tag)?;
    let edges = store.subgraph(id, 1, &[]).map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = edges.iter().map(|e| edge_value(e, id, &h)).collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

word!(w_contradicting, do_contradicting);
fn do_contradicting(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.graph.contradicting";
    let id = node_arg(vm, tag)?;
    let (store, h) = ctx(tag)?;
    let edges = store.contradicting(id).map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = edges.iter().map(|e| edge_value(e, id, &h)).collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

word!(w_loci, do_loci);
fn do_loci(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.graph.loci";
    let id = node_arg(vm, tag)?;
    let (store, _h) = ctx(tag)?;
    let edges = store.edges_out(id, &[EdgeKind::CitesLocus]).map_err(|e| anyhow!("{tag}: {e}"))?;
    let items: Vec<Value> = edges
        .iter()
        .map(|e| {
            let (_k, r) = e.dst.as_columns();
            let key = e.attrs.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("key".into(), Value::from_string(key));
            m.insert("locus".into(), Value::from_string(&r));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

word!(w_paths, do_paths);
fn do_paths(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.graph.paths";
    // ( from to -- … ) — pop `to` first (top of stack).
    let to = node_arg(vm, tag)?;
    let from = node_arg(vm, tag)?;
    let (store, h) = ctx(tag)?;
    let path = store
        .paths(from, to, &[EdgeKind::Cites, EdgeKind::LinksTo], 8)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    match path {
        Some(nodes) => {
            let items: Vec<Value> = nodes
                .iter()
                .map(|u| {
                    let label = h
                        .get(*u)
                        .map(|n| n.title.clone())
                        .unwrap_or_else(|| u.to_string());
                    let mut m: HashMap<String, Value> = HashMap::new();
                    m.insert("id".into(), Value::from_string(&u.to_string()));
                    m.insert("label".into(), Value::from_string(&label));
                    Value::from_dict(m)
                })
                .collect();
            push(vm, Value::from_list(items));
        }
        None => push(vm, Value::nodata()),
    }
    Ok(vm)
}

word!(w_pending, do_pending);
fn do_pending(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.graph.pending";
    let (store, h) = ctx(tag)?;
    let edges = store.pending_edges().map_err(|e| anyhow!("{tag}: {e}"))?;
    let label = |ep: &EndpointRef| -> String {
        match ep {
            EndpointRef::Node(u) => h
                .get(*u)
                .map(|n| n.title.clone())
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| format!("node {}", &u.to_string()[..8])),
            EndpointRef::Extern(_) => {
                let (k, r) = ep.as_columns();
                format!("{k} {r}")
            }
        }
    };
    let items: Vec<Value> = edges
        .iter()
        .map(|e| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("id".into(), Value::from_string(&e.id.to_string()));
            m.insert("kind".into(), Value::from_string(e.kind.as_str()));
            m.insert("src".into(), Value::from_string(&label(&e.src)));
            m.insert("dst".into(), Value::from_string(&label(&e.dst)));
            m.insert(
                "reason".into(),
                match e.reason.as_deref().filter(|r| !r.is_empty()) {
                    Some(r) => Value::from_string(r),
                    None => Value::nodata(),
                },
            );
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

word!(w_rebuild, do_rebuild);
fn do_rebuild(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.graph.rebuild";
    let (store, _h) = ctx(tag)?;
    let cfg = active_config(tag)?;
    let r = store.graph_rebuild(cfg).map_err(|e| anyhow!("{tag}: {e}"))?;
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("cleared".into(), Value::from_int(r.cleared as i64));
    m.insert("added".into(), Value::from_int(r.added as i64));
    push(vm, Value::from_dict(m));
    Ok(vm)
}

word!(w_promote, do_promote);
fn do_promote(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.graph.promote";
    let id = value_to_uuid(pull(vm, tag)?, tag)?;
    let (store, _h) = ctx(tag)?;
    let ok = store.promote_edge(id).map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_bool(ok));
    Ok(vm)
}

word!(w_dismiss, do_dismiss);
fn do_dismiss(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.graph.dismiss";
    let id = value_to_uuid(pull(vm, tag)?, tag)?;
    let (store, _h) = ctx(tag)?;
    store.dismiss_edge(id).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}
