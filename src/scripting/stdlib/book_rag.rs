//! 1.4.1 — `ink.book_rag.*` Bund stdlib: drive "Chat with Your Book"
//! (BOOK_RAG-1) retrieval from a script.
//!
//! Goal: the same grounding the AI pane's Book scope runs — semantic search
//! over the manuscript + curated author-content system books, expanded with
//! surrounding paragraphs and token-budgeted — is reachable from Bund, so a
//! script can retrieve passages, compose the grounding block, inspect the
//! config, and validate an answer's citations without touching the TUI.
//!
//! Policy: every word here is **read-only**. `retrieve` / `context` / `scope`
//! / `config` read the store + project config (`store_read`, default-allowed);
//! `system_prompt` / `estimate_tokens` / `cited_ids` / `validate_citations`
//! are pure but still gate under `store_read` so a paranoid project can
//! disable the whole surface in one category. Nothing here mutates or calls an
//! LLM — the retrieval is local (embeddings + vecstore), and the answer is the
//! script author's to generate.
//!
//! The retrieval core is `crate::book_rag::retrieval::retrieve` — the exact
//! function the pane and the `inkhaven book-rag` CLI use, so all three agree.

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;
use uuid::Uuid;

use super::helpers::{
    active_config, active_store, pull, push, require_depth, resolve_path, value_to_string,
};
use crate::book_rag::RetrievedPassage;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        // ── store-backed retrieval (store_read) ──
        ("ink.book_rag.retrieve", w_retrieve),
        ("ink.book_rag.context", w_context),
        ("ink.book_rag.scope", w_scope),
        ("ink.book_rag.config", w_config),
        // ── pure helpers (store_read; no store touch) ──
        ("ink.book_rag.system_prompt", w_system_prompt),
        ("ink.book_rag.estimate_tokens", w_estimate_tokens),
        ("ink.book_rag.cited_ids", w_cited_ids),
        ("ink.book_rag.validate_citations", w_validate_citations),
    ];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f)
            .map_err(|e| anyhow!("register {name}: {e}"))?;
    }
    // Ergonomic short alias: every `ink.book_rag.X` also answers to
    // `book_rag.X`. Aliases inherit the target word's policy gate.
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

// ── shared loading ─────────────────────────────────────────────────────────

/// Resolve a Bund-supplied anchor (any node's slug-path, e.g. `"manuscript"`
/// or `"manuscript/act-i/scene-3"`) to the **user book** it belongs to —
/// mirroring the TUI's Book-scope anchor resolution. The anchor itself may be
/// the Book.
fn resolve_book(hierarchy: &Hierarchy, anchor: &str, tag: &str) -> Result<Uuid> {
    let anchor = anchor.trim();
    if anchor.is_empty() {
        return Err(anyhow!(
            "{tag}: empty anchor — pass a book (or in-book node) slug-path"
        ));
    }
    let id = resolve_path(hierarchy, anchor, tag)?
        .ok_or_else(|| anyhow!("{tag}: no node at `{anchor}`"))?;
    let node = hierarchy
        .get(id)
        .ok_or_else(|| anyhow!("{tag}: node `{anchor}` vanished"))?;
    if node.kind == NodeKind::Book {
        return Ok(node.id);
    }
    hierarchy
        .ancestors(node)
        .into_iter()
        .find(|n| n.kind == NodeKind::Book)
        .map(|n| n.id)
        .ok_or_else(|| anyhow!("{tag}: `{anchor}` is not inside a book"))
}

/// One retrieved passage → a Bund dict `{id, breadcrumb, body, score, is_hit}`.
fn passage_to_value(p: &RetrievedPassage) -> Value {
    let mut h = HashMap::new();
    h.insert("id".to_string(), Value::from_string(p.id.to_string()));
    h.insert("breadcrumb".to_string(), Value::from_string(p.breadcrumb.clone()));
    h.insert("body".to_string(), Value::from_string(p.body.clone()));
    h.insert("score".to_string(), Value::from_float(p.score));
    h.insert("is_hit".to_string(), Value::from_bool(p.is_hit));
    Value::from_dict(h)
}

/// Run retrieval for an anchor + query, returning the passages. Shared by
/// `retrieve` and `context`.
fn retrieve_for(anchor: &str, query: &str, tag: &str) -> Result<Vec<RetrievedPassage>> {
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book_id = resolve_book(&hierarchy, anchor, tag)?;
    crate::book_rag::retrieval::retrieve(store, &hierarchy, &cfg.book_rag, book_id, query)
        .map_err(|e| anyhow!("{tag}: {e}"))
}

// ── store-backed retrieval ──────────────────────────────────────────────────

// ( anchor query -- passages )  passages: list of {id, breadcrumb, body, score, is_hit}
fn w_retrieve(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_retrieve(vm).map_err(to_bund_err)
}
fn do_retrieve(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.book_rag.retrieve";
    require_depth(vm, 2, tag)?;
    let query = value_to_string(pull(vm, tag)?, "query", tag)?;
    let anchor = value_to_string(pull(vm, tag)?, "anchor", tag)?;
    let passages = retrieve_for(&anchor, &query, tag)?;
    push(vm, Value::from_list(passages.iter().map(passage_to_value).collect()));
    Ok(vm)
}

// ( anchor query -- text )  the composed grounding block the model would receive.
fn w_context(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_context(vm).map_err(to_bund_err)
}
fn do_context(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.book_rag.context";
    require_depth(vm, 2, tag)?;
    let query = value_to_string(pull(vm, tag)?, "query", tag)?;
    let anchor = value_to_string(pull(vm, tag)?, "anchor", tag)?;
    let passages = retrieve_for(&anchor, &query, tag)?;
    push(vm, Value::from_string(crate::book_rag::compose_context_prefix(&passages)));
    Ok(vm)
}

// ( anchor -- ids )  the node ids in the retrieval pool (book subtree ∪ included system books).
fn w_scope(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_scope(vm).map_err(to_bund_err)
}
fn do_scope(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.book_rag.scope";
    require_depth(vm, 1, tag)?;
    let anchor = value_to_string(pull(vm, tag)?, "anchor", tag)?;
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book_id = resolve_book(&hierarchy, &anchor, tag)?;
    let ids = crate::book_rag::retrieval::scope_ids(&hierarchy, &cfg.book_rag, book_id);
    let mut list: Vec<Value> = ids.iter().map(|id| Value::from_string(id.to_string())).collect();
    list.sort_by(|a, b| {
        a.cast_string().unwrap_or_default().cmp(&b.cast_string().unwrap_or_default())
    });
    push(vm, Value::from_list(list));
    Ok(vm)
}

// ( -- dict )  the active `book_rag` config block.
fn w_config(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_config(vm).map_err(to_bund_err)
}
fn do_config(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.book_rag.config";
    let cfg = &active_config(tag)?.book_rag;
    let strs = |v: &[String]| Value::from_list(v.iter().map(Value::from_string).collect());
    let mut h = HashMap::new();
    h.insert("top_k".to_string(), Value::from_int(cfg.top_k as i64));
    h.insert("context_expansion".to_string(), Value::from_int(cfg.context_expansion as i64));
    h.insert("max_context_tokens".to_string(), Value::from_int(cfg.max_context_tokens as i64));
    h.insert("include_system_books".to_string(), strs(&cfg.include_system_books));
    h.insert("exclude_system_books".to_string(), strs(&cfg.exclude_system_books));
    push(vm, Value::from_dict(h));
    Ok(vm)
}

// ── pure helpers ────────────────────────────────────────────────────────────

// ( lang -- text )  the localized Book-RAG system prompt (EN/RU/ES/FR/DE; EN fallback).
fn w_system_prompt(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_system_prompt(vm).map_err(to_bund_err)
}
fn do_system_prompt(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.book_rag.system_prompt";
    require_depth(vm, 1, tag)?;
    let lang = value_to_string(pull(vm, tag)?, "lang", tag)?;
    push(vm, Value::from_string(crate::book_rag::system_prompt(&lang)));
    Ok(vm)
}

// ( text -- n )  rough token estimate (≈ chars/4).
fn w_estimate_tokens(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_estimate_tokens(vm).map_err(to_bund_err)
}
fn do_estimate_tokens(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.book_rag.estimate_tokens";
    require_depth(vm, 1, tag)?;
    let text = value_to_string(pull(vm, tag)?, "text", tag)?;
    push(vm, Value::from_int(crate::book_rag::estimate_tokens(&text) as i64));
    Ok(vm)
}

// ( passages -- ids )  the `id` field of each passage dict — the valid-citation set.
fn w_cited_ids(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_cited_ids(vm).map_err(to_bund_err)
}
fn do_cited_ids(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.book_rag.cited_ids";
    require_depth(vm, 1, tag)?;
    let passages = pull(vm, tag)?
        .cast_list()
        .map_err(|e| anyhow!("{tag} passages list cast failed: {e}"))?;
    let mut ids = Vec::new();
    for p in &passages {
        if let Ok(dict) = p.cast_dict() {
            if let Some(id) = dict.get("id").and_then(|v| v.cast_string().ok()) {
                ids.push(Value::from_string(id));
            }
        }
    }
    push(vm, Value::from_list(ids));
    Ok(vm)
}

// ( response ids -- text )  flag any markdown citation `](#id)` whose id is not in `ids`.
fn w_validate_citations(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_validate_citations(vm).map_err(to_bund_err)
}
fn do_validate_citations(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.book_rag.validate_citations";
    require_depth(vm, 2, tag)?;
    let ids_list = pull(vm, tag)?
        .cast_list()
        .map_err(|e| anyhow!("{tag} ids list cast failed: {e}"))?;
    let response = value_to_string(pull(vm, tag)?, "response", tag)?;
    let valid: HashSet<String> = ids_list.iter().filter_map(|v| v.cast_string().ok()).collect();
    push(vm, Value::from_string(crate::book_rag::validate_citations(&response, &valid)));
    Ok(vm)
}
