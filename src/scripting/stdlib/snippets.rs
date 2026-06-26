//! 1.4.9 REUSE-1 — `ink.snippets.*` Bund stdlib: read the Snippets book + the
//! `#include` references. All read-only (`store_read`, default-allowed).

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_store, pull, push, require_depth, value_to_string};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::{NodeKind, Store, SYSTEM_TAG_SNIPPETS};

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.snippets.list", w_list),
        ("ink.snippets.get", w_get),
        ("ink.snippets.check", w_check),
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

fn snippets_book<'a>(h: &'a Hierarchy) -> Option<&'a Node> {
    h.iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_SNIPPETS))
}

fn read_body(store: &Store, node: &Node) -> Option<String> {
    let rel = node.file.as_ref()?;
    std::fs::read_to_string(store.project_root().join(rel)).ok()
}

/// ( -- list ) every snippet as `{ slug, title }`.
fn w_list(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_list(vm).map_err(to_bund_err)
}
fn do_list(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.snippets.list";
    let store = active_store(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let mut items = Vec::new();
    if let Some(book) = snippets_book(&h) {
        for id in h.collect_subtree(book.id) {
            let Some(n) = h.get(id) else { continue };
            if n.kind != NodeKind::Paragraph {
                continue;
            }
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("slug".into(), Value::from_string(&n.slug));
            m.insert("title".into(), Value::from_string(&n.title));
            items.push(Value::from_dict(m));
        }
    }
    push(vm, Value::from_list(items));
    Ok(vm)
}

/// ( slug -- dict | NODATA ) the snippet `{ slug, title, body }`.
fn w_get(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_get(vm).map_err(to_bund_err)
}
fn do_get(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.snippets.get";
    require_depth(vm, 1, tag)?;
    let want = value_to_string(pull(vm, tag)?, "slug", tag)?;
    let store = active_store(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let out = snippets_book(&h)
        .and_then(|book| {
            h.collect_subtree(book.id)
                .into_iter()
                .filter_map(|id| h.get(id))
                .find(|n| n.kind == NodeKind::Paragraph && n.slug == want.trim())
        })
        .map(|n| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("slug".into(), Value::from_string(&n.slug));
            m.insert("title".into(), Value::from_string(&n.title));
            m.insert(
                "body".into(),
                Value::from_string(read_body(store, n).unwrap_or_default()),
            );
            Value::from_dict(m)
        })
        .unwrap_or_else(Value::nodata);
    push(vm, out);
    Ok(vm)
}

/// ( -- list ) missing snippet references as `{ slug, path, line }` (a
/// `#include` whose slug isn't defined). Empty list = clean.
fn w_check(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_check(vm).map_err(to_bund_err)
}
fn do_check(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.snippets.check";
    let store = active_store(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let defined: std::collections::HashSet<String> = snippets_book(&h)
        .map(|book| {
            h.collect_subtree(book.id)
                .into_iter()
                .filter_map(|id| h.get(id))
                .filter(|n| n.kind == NodeKind::Paragraph)
                .map(|n| n.slug.clone())
                .collect()
        })
        .unwrap_or_default();

    let mut out = Vec::new();
    for book in h
        .children_of(None)
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
    {
        for id in h.collect_subtree(book.id) {
            let Some(node) = h.get(id) else { continue };
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            let Some(body) = read_body(store, node) else { continue };
            let path = h.slug_path(node);
            for (i, line) in body.lines().enumerate() {
                for slug in crate::typst_check::snippet_references(line) {
                    if !defined.contains(&slug) {
                        let mut m: HashMap<String, Value> = HashMap::new();
                        m.insert("slug".into(), Value::from_string(&slug));
                        m.insert("path".into(), Value::from_string(&path));
                        m.insert("line".into(), Value::from_int((i + 1) as i64));
                        out.push(Value::from_dict(m));
                    }
                }
            }
        }
    }
    push(vm, Value::from_list(out));
    Ok(vm)
}
