//! Read-only `ink.*` Bund stdlib. These are the words that let a
//! script see the project — `Store` access, basically — without
//! letting it mutate anything. Write-side words land later under
//! the policy sandbox (P3+).
//!
//! All `register_inline` failures bubble back as `anyhow::Error` so
//! the caller (init_adam) can fail the whole VM construction. In
//! practice the registry only fails on duplicate names.

use anyhow::{anyhow, Result};
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;
use std::collections::HashMap;

use super::helpers::{
    active_store, pull, push, require_depth, value_to_i64, value_to_string, value_to_uuid,
};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

/// Register every read-only `ink.*` word.
pub fn register(vm: &mut VM) -> Result<()> {
    vm.register_inline("ink.node.list".to_string(), ink_node_list)
        .map_err(|e| anyhow!("register ink.node.list: {e}"))?;
    vm.register_inline("ink.node.get".to_string(), ink_node_get)
        .map_err(|e| anyhow!("register ink.node.get: {e}"))?;
    vm.register_inline("ink.node.children".to_string(), ink_node_children)
        .map_err(|e| anyhow!("register ink.node.children: {e}"))?;
    vm.register_inline("ink.paragraph.text".to_string(), ink_paragraph_text)
        .map_err(|e| anyhow!("register ink.paragraph.text: {e}"))?;
    vm.register_inline("ink.search.text".to_string(), ink_search_text)
        .map_err(|e| anyhow!("register ink.search.text: {e}"))?;
    vm.register_inline("ink.snapshot.list".to_string(), ink_snapshot_list)
        .map_err(|e| anyhow!("register ink.snapshot.list: {e}"))?;
    Ok(())
}

// VMInlineFn signature: `fn(&mut VM) -> Result<&mut VM, easy_error::Error>`.
// Bund expects easy_error; we use anyhow internally and convert at the
// boundary via `to_bund_err`.
type BundError = easy_error::Error;
type BundResult<'a> = std::result::Result<&'a mut VM, BundError>;

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(e.to_string())
}

// ── ink.node.list ────────────────────────────────────────────────────
// Stack: ( -- list )
// Pushes a list of hashes, one per node in the project. Each hash
// has keys: id, kind, title, slug, parent_id (or empty), order,
// status (or empty), content_type (or empty).

fn ink_node_list(vm: &mut VM) -> BundResult<'_> {
    do_ink_node_list(vm).map_err(to_bund_err)
}

fn do_ink_node_list(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.node.list";
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag} hierarchy load: {e}"))?;
    let items: Vec<Value> = hierarchy.iter().map(node_summary_dict).collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

// ── ink.node.get ─────────────────────────────────────────────────────
// Stack: ( uuid -- hash | NODATA )

fn ink_node_get(vm: &mut VM) -> BundResult<'_> {
    do_ink_node_get(vm).map_err(to_bund_err)
}

fn do_ink_node_get(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.node.get";
    require_depth(vm, 1, tag)?;
    let id = value_to_uuid(pull(vm, tag)?, tag)?;
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag} hierarchy load: {e}"))?;
    let out = match hierarchy.get(id) {
        Some(node) => node_full_dict(node),
        None => Value::nodata(),
    };
    push(vm, out);
    Ok(vm)
}

// ── ink.node.children ────────────────────────────────────────────────
// Stack: ( uuid_or_empty -- list )
// Empty string = root (top-level books).

fn ink_node_children(vm: &mut VM) -> BundResult<'_> {
    do_ink_node_children(vm).map_err(to_bund_err)
}

fn do_ink_node_children(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.node.children";
    require_depth(vm, 1, tag)?;
    let arg = pull(vm, tag)?;
    let s = value_to_string(arg, "parent", tag)?;
    let parent_id = if s.is_empty() {
        None
    } else {
        Some(uuid::Uuid::parse_str(&s).map_err(|e| anyhow!("{tag} UUID parse failed: {e}"))?)
    };
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag} hierarchy load: {e}"))?;
    let items: Vec<Value> = hierarchy
        .children_of(parent_id)
        .into_iter()
        .map(node_summary_dict)
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

// ── ink.paragraph.text ───────────────────────────────────────────────
// Stack: ( uuid -- string | NODATA )

fn ink_paragraph_text(vm: &mut VM) -> BundResult<'_> {
    do_ink_paragraph_text(vm).map_err(to_bund_err)
}

fn do_ink_paragraph_text(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.paragraph.text";
    require_depth(vm, 1, tag)?;
    let id = value_to_uuid(pull(vm, tag)?, tag)?;
    let store = active_store(tag)?;
    let out = match store
        .get_content(id)
        .map_err(|e| anyhow!("{tag} get_content: {e}"))?
    {
        Some(bytes) => Value::from_string(String::from_utf8_lossy(&bytes).into_owned()),
        None => Value::nodata(),
    };
    push(vm, out);
    Ok(vm)
}

// ── ink.search.text ──────────────────────────────────────────────────
// Stack: ( query limit -- list )
// limit must be a positive int. Returns a list of hit hashes with
// id, title, score, document, kind.

fn ink_search_text(vm: &mut VM) -> BundResult<'_> {
    do_ink_search_text(vm).map_err(to_bund_err)
}

fn do_ink_search_text(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.search.text";
    require_depth(vm, 2, tag)?;
    let limit_v = pull(vm, tag)?;
    let query_v = pull(vm, tag)?;
    let query = value_to_string(query_v, "query", tag)?;
    let limit = value_to_i64(limit_v, "limit", tag)?.max(0) as usize;
    let store = active_store(tag)?;
    let hits = store
        .search_text(&query, limit)
        .map_err(|e| anyhow!("{tag} search_text: {e}"))?;
    let items: Vec<Value> = hits.into_iter().map(search_hit_dict).collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

// ── ink.snapshot.list ────────────────────────────────────────────────
// Stack: ( paragraph_uuid -- list )

fn ink_snapshot_list(vm: &mut VM) -> BundResult<'_> {
    do_ink_snapshot_list(vm).map_err(to_bund_err)
}

fn do_ink_snapshot_list(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.snapshot.list";
    require_depth(vm, 1, tag)?;
    let id = value_to_uuid(pull(vm, tag)?, tag)?;
    let store = active_store(tag)?;
    let snaps = store
        .list_snapshots(id)
        .map_err(|e| anyhow!("{tag} list_snapshots: {e}"))?;
    let items: Vec<Value> = snaps
        .into_iter()
        .map(|s| {
            let mut h: HashMap<String, Value> = HashMap::new();
            h.insert("id".into(), Value::from_string(s.id.to_string()));
            h.insert(
                "created_at".into(),
                Value::from_string(s.created_at.to_rfc3339()),
            );
            h.insert("word_count".into(), Value::from_int(s.word_count as i64));
            h.insert("preview".into(), Value::from_string(s.preview));
            Value::from_dict(h)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

// ── value builders ───────────────────────────────────────────────────

fn node_summary_dict(n: &Node) -> Value {
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("id".into(), Value::from_string(n.id.to_string()));
    h.insert("kind".into(), Value::from_string(n.kind.as_str()));
    h.insert("title".into(), Value::from_string(&n.title));
    h.insert("slug".into(), Value::from_string(&n.slug));
    Value::from_dict(h)
}

fn node_full_dict(n: &Node) -> Value {
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("id".into(), Value::from_string(n.id.to_string()));
    h.insert("kind".into(), Value::from_string(n.kind.as_str()));
    h.insert("title".into(), Value::from_string(&n.title));
    h.insert("slug".into(), Value::from_string(&n.slug));
    h.insert("order".into(), Value::from_int(n.order as i64));
    h.insert(
        "word_count".into(),
        Value::from_int(n.word_count as i64),
    );
    h.insert(
        "modified_at".into(),
        Value::from_string(n.modified_at.to_rfc3339()),
    );
    h.insert(
        "parent_id".into(),
        match n.parent_id {
            Some(p) => Value::from_string(p.to_string()),
            None => Value::nodata(),
        },
    );
    h.insert(
        "system_tag".into(),
        match &n.system_tag {
            Some(s) => Value::from_string(s),
            None => Value::nodata(),
        },
    );
    h.insert(
        "status".into(),
        match &n.status {
            Some(s) => Value::from_string(s),
            None => Value::nodata(),
        },
    );
    h.insert(
        "content_type".into(),
        match &n.content_type {
            Some(s) => Value::from_string(s),
            None => Value::nodata(),
        },
    );
    Value::from_dict(h)
}

fn search_hit_dict(hit: serde_json::Value) -> Value {
    let mut h: HashMap<String, Value> = HashMap::new();
    if let Some(id) = hit.get("id").and_then(|v| v.as_str()) {
        h.insert("id".into(), Value::from_string(id));
    }
    if let Some(score) = hit.get("score").and_then(|v| v.as_f64()) {
        h.insert("score".into(), Value::from_float(score));
    }
    if let Some(meta) = hit.get("metadata") {
        if let Some(title) = meta.get("title").and_then(|v| v.as_str()) {
            h.insert("title".into(), Value::from_string(title));
        }
        if let Some(kind) = meta.get("kind").and_then(|v| v.as_str()) {
            h.insert("kind".into(), Value::from_string(kind));
        }
    }
    if let Some(doc) = hit.get("document").and_then(|v| v.as_str()) {
        h.insert("document".into(), Value::from_string(doc));
    }
    Value::from_dict(h)
}
