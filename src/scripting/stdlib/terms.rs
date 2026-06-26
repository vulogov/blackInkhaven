//! 1.4.8 TERMS-1 — `ink.terms.*` Bund stdlib: read the Glossary + banned-synonym
//! findings, and declare a deliberate terminology variant.
//!
//! Policy: `list` / `get` / `check` are read-only (`store_read`, default-allowed).
//! `declare_intent` writes an intent-ledger row (`store_write`, default-denied —
//! opt in via `scripting.enabled_categories`), mirroring the Inner Editor's
//! intent declarers.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_store, pull, push, require_depth, value_to_string};
use crate::glossary::{self, GlossaryEntry};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::NodeKind;
use crate::tui::style_warnings::BannedSynonymDetector;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.terms.list", w_list),
        ("ink.terms.get", w_get),
        ("ink.terms.check", w_check),
        ("ink.terms.declare_intent", w_declare_intent),
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

fn opt(map: &mut HashMap<String, Value>, key: &str, v: &Option<String>) {
    if let Some(s) = v {
        if !s.trim().is_empty() {
            map.insert(key.into(), Value::from_string(s));
        }
    }
}

fn entry_dict(e: &GlossaryEntry) -> Value {
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("term".into(), Value::from_string(&e.term));
    h.insert("definition".into(), Value::from_string(&e.definition));
    let syn: Vec<Value> = e.synonyms.iter().map(Value::from_string).collect();
    h.insert("synonyms".into(), Value::from_list(syn));
    opt(&mut h, "scope", &e.scope);
    opt(&mut h, "note", &e.note);
    Value::from_dict(h)
}

/// ( -- list ) every glossary entry as a dict.
fn w_list(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_list(vm).map_err(to_bund_err)
}
fn do_list(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.terms.list";
    let store = active_store(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let entries = glossary::glossary_entries_from_store(store, &h, None);
    let items: Vec<Value> = entries.iter().map(entry_dict).collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

/// ( term -- dict | NODATA ) the glossary entry for a canonical term.
fn w_get(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_get(vm).map_err(to_bund_err)
}
fn do_get(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.terms.get";
    require_depth(vm, 1, tag)?;
    let want = value_to_string(pull(vm, tag)?, "term", tag)?;
    let store = active_store(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let entries = glossary::glossary_entries_from_store(store, &h, None);
    let out = entries
        .iter()
        .find(|e| e.term.eq_ignore_ascii_case(want.trim()))
        .map(entry_dict)
        .unwrap_or_else(Value::nodata);
    push(vm, out);
    Ok(vm)
}

/// ( book_slug -- list ) banned-synonym findings for a book, each
/// `{ path, line, synonym, canonical }`. Empty list = clean.
fn w_check(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_check(vm).map_err(to_bund_err)
}
fn do_check(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.terms.check";
    require_depth(vm, 1, tag)?;
    let slug = value_to_string(pull(vm, tag)?, "book_slug", tag)?;
    let store = active_store(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let Some(book) = h.children_of(None).into_iter().find(|n| {
        n.kind == NodeKind::Book && n.system_tag.is_none() && n.slug == slug.trim()
    }) else {
        return Err(anyhow!("{tag}: no user book with slug `{slug}`"));
    };
    let detector = BannedSynonymDetector::from_store(store, &h, Some(&book.slug));
    let mut out = Vec::new();
    if !detector.is_empty() {
        for id in h.collect_subtree(book.id) {
            let Some(node) = h.get(id) else { continue };
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            let Some(body) = read_body(store, node) else { continue };
            let path = h.slug_path(node);
            for (i, line) in body.lines().enumerate() {
                for hit in detector.detect(line) {
                    if let Some((synonym, canonical)) = detector.hint_at(line, hit.col_start) {
                        let mut m: HashMap<String, Value> = HashMap::new();
                        m.insert("path".into(), Value::from_string(&path));
                        m.insert("line".into(), Value::from_int((i + 1) as i64));
                        m.insert("synonym".into(), Value::from_string(synonym));
                        m.insert("canonical".into(), Value::from_string(canonical));
                        out.push(Value::from_dict(m));
                    }
                }
            }
        }
    }
    push(vm, Value::from_list(out));
    Ok(vm)
}

/// ( canonical scope -- ) declare a canonical term a deliberate variant, so the
/// banned-synonym overlay + `terms check` stop flagging its synonyms. `scope` is
/// `global` (project-wide) or a book/chapter slug.
fn w_declare_intent(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_declare_intent(vm).map_err(to_bund_err)
}
fn do_declare_intent(vm: &mut VM) -> Result<&mut VM> {
    use crate::inner_socrates::intent::{IntentKind, IntentScope, ScopeLevel};
    use crate::inner_socrates::storage::InnerSocratesStore;
    let tag = "ink.terms.declare_intent";
    require_depth(vm, 2, tag)?;
    let scope_arg = value_to_string(pull(vm, tag)?, "scope", tag)?;
    let canonical = value_to_string(pull(vm, tag)?, "canonical", tag)?;
    let canonical = canonical.trim();
    if canonical.is_empty() {
        return Err(anyhow!("{tag}: canonical term is empty"));
    }
    let store = active_store(tag)?;
    let is = InnerSocratesStore::open_for_project(store.project_root())
        .map_err(|e| anyhow!("{tag}: inner-socrates store: {e}"))?;
    let scope = match scope_arg.trim() {
        "" | "global" | "project" => IntentScope::Project,
        slug => IntentScope::Chapter(slug.to_string()),
    };
    let id = uuid::Uuid::new_v4().to_string();
    is.add_intent_raw(
        &id,
        &IntentKind::DeliberateVariant,
        canonical, // description = the canonical term (read back for suppression)
        &scope,
        &["banned_synonym".to_string()],
        ScopeLevel::Project,
    )
    .map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_string(canonical));
    Ok(vm)
}

fn read_body(store: &crate::store::Store, node: &Node) -> Option<String> {
    let rel = node.file.as_ref()?;
    let raw = std::fs::read_to_string(store.project_root().join(rel)).ok()?;
    let body = if raw.trim_start().starts_with("= ") {
        raw.splitn(2, '\n').nth(1).unwrap_or("").to_string()
    } else {
        raw
    };
    Some(body)
}
