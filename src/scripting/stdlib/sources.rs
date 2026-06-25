//! 1.4.5 SOURCES-1 — `ink.sources.*` Bund stdlib: read the bibliography from a
//! script.
//!
//! Policy: every word here is **read-only** (`store_read`, default-allowed).
//! `list` / `get` / `bibtex` read the citation entries defined in the Sources
//! book; `check` validates the `@key` tokens in prose against them (honouring
//! `sources.all` scope). Authoring stays in the CLI (`inkhaven sources import`)
//! and the TUI (add a paragraph under Sources, or `Ctrl+V @`) — Bund does not
//! mutate user content here.
//!
//! Bodies are read from disk (the files `inkhaven build` compiles), so the
//! scripted view matches what assembly will emit.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, pull, push, require_depth, value_to_string};
use crate::sources::{self, BibEntry};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::{NodeKind, Store, SYSTEM_TAG_SOURCES};

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.sources.list", w_list),
        ("ink.sources.get", w_get),
        ("ink.sources.check", w_check),
        ("ink.sources.bibtex", w_bibtex),
    ];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f)
            .map_err(|e| anyhow!("register {name}: {e}"))?;
    }
    // Ergonomic alias: `ink.sources.X` also answers to `sources.X`.
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

// ── shared collection (disk-read, matches assembly) ─────────────────────────

fn strip_heading(body: &str) -> &str {
    if body.trim_start().starts_with("= ") {
        body.splitn(2, '\n').nth(1).unwrap_or("")
    } else {
        body
    }
}

fn read_body(store: &Store, node: &Node) -> Option<String> {
    let rel = node.file.as_ref()?;
    let raw = std::fs::read_to_string(store.project_root().join(rel)).ok()?;
    Some(strip_heading(&raw).to_string())
}

/// Every valid citation entry under the Sources book, paired with the title of
/// its enclosing chapter (the scope key).
fn collect_entries(store: &Store, h: &Hierarchy) -> Vec<(String, BibEntry)> {
    let Some(sources) = h.iter().find(|n| {
        n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_SOURCES)
    }) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for id in h.collect_subtree(sources.id) {
        if id == sources.id {
            continue;
        }
        let Some(node) = h.get(id) else { continue };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(body) = read_body(store, node) else { continue };
        if let Some(e) = BibEntry::from_hjson(&body) {
            if e.is_valid() {
                let mut cur: Option<&Node> = Some(node);
                let mut chapter = String::new();
                while let Some(n) = cur {
                    if n.parent_id == Some(sources.id) {
                        chapter = n.title.clone();
                        break;
                    }
                    cur = n.parent_id.and_then(|pid| h.get(pid));
                }
                out.push((chapter, e));
            }
        }
    }
    out
}

fn opt(map: &mut HashMap<String, Value>, key: &str, v: &Option<String>) {
    if let Some(s) = v {
        if !s.trim().is_empty() {
            map.insert(key.into(), Value::from_string(s));
        }
    }
}

fn entry_summary_dict(chapter: &str, e: &BibEntry) -> Value {
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("key".into(), Value::from_string(&e.key));
    h.insert("type".into(), Value::from_string(&e.entry_type));
    h.insert("author".into(), Value::from_string(&e.author));
    h.insert("title".into(), Value::from_string(&e.title));
    h.insert("year".into(), Value::from_string(&e.year));
    h.insert("chapter".into(), Value::from_string(chapter));
    Value::from_dict(h)
}

fn entry_full_dict(chapter: &str, e: &BibEntry) -> Value {
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("key".into(), Value::from_string(&e.key));
    h.insert("type".into(), Value::from_string(&e.entry_type));
    h.insert("author".into(), Value::from_string(&e.author));
    h.insert("title".into(), Value::from_string(&e.title));
    h.insert("year".into(), Value::from_string(&e.year));
    h.insert("chapter".into(), Value::from_string(chapter));
    opt(&mut h, "journal", &e.journal);
    opt(&mut h, "volume", &e.volume);
    opt(&mut h, "number", &e.number);
    opt(&mut h, "pages", &e.pages);
    opt(&mut h, "publisher", &e.publisher);
    opt(&mut h, "booktitle", &e.booktitle);
    opt(&mut h, "editor", &e.editor);
    opt(&mut h, "edition", &e.edition);
    opt(&mut h, "url", &e.url);
    opt(&mut h, "doi", &e.doi);
    opt(&mut h, "isbn", &e.isbn);
    opt(&mut h, "note", &e.note);
    opt(&mut h, "abstract", &e.abstract_);
    opt(&mut h, "keywords", &e.keywords);
    Value::from_dict(h)
}

// ── words ───────────────────────────────────────────────────────────────────

/// ( -- list ) every defined citation entry as a summary dict.
fn w_list(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_list(vm).map_err(to_bund_err)
}
fn do_list(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.sources.list";
    let store = active_store(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let mut entries = collect_entries(store, &h);
    entries.sort_by(|a, b| a.1.key.to_lowercase().cmp(&b.1.key.to_lowercase()));
    let items: Vec<Value> = entries
        .iter()
        .map(|(chapter, e)| entry_summary_dict(chapter, e))
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

/// ( key -- dict | NODATA ) the full entry whose `key` matches (case-sensitive).
fn w_get(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_get(vm).map_err(to_bund_err)
}
fn do_get(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.sources.get";
    require_depth(vm, 1, tag)?;
    let want = value_to_string(pull(vm, tag)?, "key", tag)?;
    let store = active_store(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let entries = collect_entries(store, &h);
    let out = entries
        .iter()
        .find(|(_, e)| e.key == want)
        .map(|(chapter, e)| entry_full_dict(chapter, e))
        .unwrap_or_else(Value::nodata);
    push(vm, out);
    Ok(vm)
}

/// ( -- string ) the compiled BibTeX for all defined entries.
fn w_bibtex(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_bibtex(vm).map_err(to_bund_err)
}
fn do_bibtex(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.sources.bibtex";
    let store = active_store(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let entries: Vec<BibEntry> = collect_entries(store, &h).into_iter().map(|(_, e)| e).collect();
    let (text, _n) = sources::compile_bibtex(&entries);
    push(vm, Value::from_string(text));
    Ok(vm)
}

/// ( -- list ) undefined `@key` citations in prose, each a dict
/// `{ key, book, paragraph }`. Honours `sources.all` scope per book.
fn w_check(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_check(vm).map_err(to_bund_err)
}
fn do_check(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.sources.check";
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let all_entries = collect_entries(store, &h);

    let mut missing: Vec<Value> = Vec::new();
    for book in h
        .children_of(None)
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
    {
        let defined: std::collections::BTreeSet<&str> = all_entries
            .iter()
            .filter(|(chapter, _)| cfg.sources.all || chapter == &book.title)
            .map(|(_, e)| e.key.as_str())
            .collect();
        for id in h.collect_subtree(book.id) {
            let Some(node) = h.get(id) else { continue };
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            let Some(body) = read_body(store, node) else { continue };
            for key in sources::extract_cite_keys(&body) {
                if !defined.contains(key.as_str()) {
                    let mut m: HashMap<String, Value> = HashMap::new();
                    m.insert("key".into(), Value::from_string(&key));
                    m.insert("book".into(), Value::from_string(&book.title));
                    m.insert("paragraph".into(), Value::from_string(&node.title));
                    missing.push(Value::from_dict(m));
                }
            }
        }
    }
    push(vm, Value::from_list(missing));
    Ok(vm)
}
