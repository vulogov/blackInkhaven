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
use crate::store::node::{Node, NodeKind};

/// Register every `ink.*` word (read + write + db).
pub fn register(vm: &mut VM) -> Result<()> {
    // ── Read-only (store_read) ────────────────────────────────
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
    vm.register_inline("ink.path.to_uuid".to_string(), ink_path_to_uuid)
        .map_err(|e| anyhow!("register ink.path.to_uuid: {e}"))?;

    // ── Tree mutation (store_write) ───────────────────────────
    vm.register_inline("ink.tree.add".to_string(), ink_tree_add)
        .map_err(|e| anyhow!("register ink.tree.add: {e}"))?;
    vm.register_inline("ink.tree.delete".to_string(), ink_tree_delete)
        .map_err(|e| anyhow!("register ink.tree.delete: {e}"))?;
    vm.register_inline("ink.tree.rename".to_string(), ink_tree_rename)
        .map_err(|e| anyhow!("register ink.tree.rename: {e}"))?;
    vm.register_inline("ink.tree.move_up".to_string(), ink_tree_move_up)
        .map_err(|e| anyhow!("register ink.tree.move_up: {e}"))?;
    vm.register_inline("ink.tree.move_down".to_string(), ink_tree_move_down)
        .map_err(|e| anyhow!("register ink.tree.move_down: {e}"))?;
    vm.register_inline("ink.tree.morph".to_string(), ink_tree_morph)
        .map_err(|e| anyhow!("register ink.tree.morph: {e}"))?;

    // ── Paragraph mutation (store_write) ──────────────────────
    vm.register_inline("ink.paragraph.set_status".to_string(), ink_paragraph_set_status)
        .map_err(|e| anyhow!("register ink.paragraph.set_status: {e}"))?;
    vm.register_inline("ink.paragraph.set_target".to_string(), ink_paragraph_set_target)
        .map_err(|e| anyhow!("register ink.paragraph.set_target: {e}"))?;
    vm.register_inline("ink.paragraph.target".to_string(), ink_paragraph_target)
        .map_err(|e| anyhow!("register ink.paragraph.target: {e}"))?;
    vm.register_inline("ink.paragraph.save".to_string(), ink_paragraph_save)
        .map_err(|e| anyhow!("register ink.paragraph.save: {e}"))?;

    // ── DB management (store_write) ───────────────────────────
    vm.register_inline("ink.db.sync".to_string(), ink_db_sync)
        .map_err(|e| anyhow!("register ink.db.sync: {e}"))?;
    vm.register_inline("ink.db.checkpoint".to_string(), ink_db_checkpoint)
        .map_err(|e| anyhow!("register ink.db.checkpoint: {e}"))?;
    vm.register_inline("ink.db.reindex".to_string(), ink_db_reindex)
        .map_err(|e| anyhow!("register ink.db.reindex: {e}"))?;

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

// ─────────────────────────────────────────────────────────────────────
// Phase A — write-side ink.* words
// ─────────────────────────────────────────────────────────────────────

use crate::scripting::stdlib::helpers::{active_config, resolve_path};
use crate::store::InsertPosition;

// ── ink.path.to_uuid ─────────────────────────────────────────────────
// Stack: ( slug_path -- uuid_string | NODATA )

fn ink_path_to_uuid(vm: &mut VM) -> BundResult<'_> {
    do_ink_path_to_uuid(vm).map_err(to_bund_err)
}

fn do_ink_path_to_uuid(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.path.to_uuid";
    require_depth(vm, 1, tag)?;
    let path = value_to_string(pull(vm, tag)?, "path", tag)?;
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag} hierarchy: {e}"))?;
    let out = match resolve_path(&hierarchy, &path, tag)? {
        Some(id) => Value::from_string(id.to_string()),
        None => Value::nodata(),
    };
    push(vm, out);
    Ok(vm)
}

// ── ink.tree.add ─────────────────────────────────────────────────────
// Stack: ( parent_path kind title -- uuid_string )
// kind ∈ { "book", "chapter", "subchapter", "paragraph", "hjson",
//         "script", "bund" }. "hjson" → Paragraph with content_type
// patched after create; "bund" is an alias for "script".

fn ink_tree_add(vm: &mut VM) -> BundResult<'_> {
    do_ink_tree_add(vm).map_err(to_bund_err)
}

fn do_ink_tree_add(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.tree.add";
    require_depth(vm, 3, tag)?;
    let title = value_to_string(pull(vm, tag)?, "title", tag)?;
    let kind_str = value_to_string(pull(vm, tag)?, "kind", tag)?;
    let parent_path = value_to_string(pull(vm, tag)?, "parent_path", tag)?;
    let (kind, post_morph_ct) = match kind_str.as_str() {
        "book" => (NodeKind::Book, None),
        "chapter" => (NodeKind::Chapter, None),
        "subchapter" => (NodeKind::Subchapter, None),
        "paragraph" => (NodeKind::Paragraph, None),
        "hjson" => (NodeKind::Paragraph, Some("hjson")),
        "script" | "bund" => (NodeKind::Script, None),
        other => return Err(anyhow!("{tag}: unknown kind `{other}`")),
    };

    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag} hierarchy: {e}"))?;
    let parent_id = resolve_path(&hierarchy, &parent_path, tag)?;
    let parent_node: Option<Node> = parent_id.and_then(|id| hierarchy.get(id).cloned());
    let created = store
        .create_node(
            cfg,
            &hierarchy,
            kind,
            &title,
            parent_node.as_ref(),
            None,
            InsertPosition::End,
        )
        .map_err(|e| anyhow!("{tag} create: {e}"))?;

    // For "hjson", we created a Paragraph then need to flip its
    // content_type. `Store::convert_leaf` does both the metadata
    // stamp AND the file rename.
    let final_id = if let Some(ct) = post_morph_ct {
        let h2 = Hierarchy::load(store).map_err(|e| anyhow!("{tag} reload: {e}"))?;
        let morphed = store
            .convert_leaf(&h2, created.id, NodeKind::Paragraph, Some(ct))
            .map_err(|e| anyhow!("{tag} morph: {e}"))?;
        morphed.id
    } else {
        created.id
    };

    push(vm, Value::from_string(final_id.to_string()));
    Ok(vm)
}

// ── ink.tree.delete ──────────────────────────────────────────────────
// Stack: ( path -- )
// Deletes the node + its entire subtree. Fires `hook.on_delete`
// once per removed id.

fn ink_tree_delete(vm: &mut VM) -> BundResult<'_> {
    do_ink_tree_delete(vm).map_err(to_bund_err)
}

fn do_ink_tree_delete(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.tree.delete";
    require_depth(vm, 1, tag)?;
    let path = value_to_string(pull(vm, tag)?, "path", tag)?;
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag} hierarchy: {e}"))?;
    let node_id = resolve_path(&hierarchy, &path, tag)?
        .ok_or_else(|| anyhow!("{tag}: cannot delete root"))?;
    let node = hierarchy
        .get(node_id)
        .cloned()
        .ok_or_else(|| anyhow!("{tag}: node {node_id} vanished"))?;
    let ids: Vec<uuid::Uuid> = hierarchy.collect_subtree(node.id).into_iter().collect();
    // The `layout` arg to `Hierarchy::fs_path` is currently
    // ignored by the implementation; pass a layout reconstructed
    // from project root.
    let layout = crate::project::ProjectLayout::new(store.project_root());
    let fs_rel = hierarchy.fs_path(&node, &layout);
    store
        .delete_subtree(&fs_rel, &ids)
        .map_err(|e| anyhow!("{tag} delete: {e}"))?;
    Ok(vm)
}

// ── ink.tree.rename ──────────────────────────────────────────────────
// Stack: ( path new_title -- )

fn ink_tree_rename(vm: &mut VM) -> BundResult<'_> {
    do_ink_tree_rename(vm).map_err(to_bund_err)
}

fn do_ink_tree_rename(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.tree.rename";
    require_depth(vm, 2, tag)?;
    let new_title = value_to_string(pull(vm, tag)?, "new_title", tag)?;
    let path = value_to_string(pull(vm, tag)?, "path", tag)?;
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag} hierarchy: {e}"))?;
    let node_id = resolve_path(&hierarchy, &path, tag)?
        .ok_or_else(|| anyhow!("{tag}: cannot rename root"))?;
    store
        .rename_node(&hierarchy, node_id, &new_title)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

// ── ink.tree.move_up / .move_down ───────────────────────────────────
// Stack: ( path -- )
// Swap with the previous / next sibling. No-op when at the edge.

fn ink_tree_move_up(vm: &mut VM) -> BundResult<'_> {
    do_ink_tree_move(vm, MoveDir::Up).map_err(to_bund_err)
}

fn ink_tree_move_down(vm: &mut VM) -> BundResult<'_> {
    do_ink_tree_move(vm, MoveDir::Down).map_err(to_bund_err)
}

#[derive(Clone, Copy)]
enum MoveDir {
    Up,
    Down,
}

fn do_ink_tree_move(vm: &mut VM, dir: MoveDir) -> Result<&mut VM> {
    let tag = match dir {
        MoveDir::Up => "ink.tree.move_up",
        MoveDir::Down => "ink.tree.move_down",
    };
    require_depth(vm, 1, tag)?;
    let path = value_to_string(pull(vm, tag)?, "path", tag)?;
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag} hierarchy: {e}"))?;
    let node_id = resolve_path(&hierarchy, &path, tag)?
        .ok_or_else(|| anyhow!("{tag}: cannot move root"))?;
    let node = hierarchy
        .get(node_id)
        .cloned()
        .ok_or_else(|| anyhow!("{tag}: node missing"))?;
    let siblings = hierarchy.children_of(node.parent_id);
    let pos = siblings
        .iter()
        .position(|n| n.id == node.id)
        .ok_or_else(|| anyhow!("{tag}: node not among siblings"))?;
    let neighbour_idx = match dir {
        MoveDir::Up => pos.checked_sub(1),
        MoveDir::Down => {
            if pos + 1 < siblings.len() {
                Some(pos + 1)
            } else {
                None
            }
        }
    };
    let Some(idx) = neighbour_idx else {
        // edge — silently no-op (matches the TUI's move_current
        // behaviour at the boundary).
        return Ok(vm);
    };
    let neighbour_id = siblings[idx].id;
    store
        .swap_siblings(&hierarchy, node.id, neighbour_id)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

// ── ink.tree.morph ──────────────────────────────────────────────────
// Stack: ( path -- )
// Cycle leaf flavour: Paragraph(typst) → Paragraph(hjson) →
// Script(bund) → Paragraph(typst). Same logic as the `Ctrl+B M`
// chord in the TUI.

fn ink_tree_morph(vm: &mut VM) -> BundResult<'_> {
    do_ink_tree_morph(vm).map_err(to_bund_err)
}

fn do_ink_tree_morph(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.tree.morph";
    require_depth(vm, 1, tag)?;
    let path = value_to_string(pull(vm, tag)?, "path", tag)?;
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag} hierarchy: {e}"))?;
    let node_id = resolve_path(&hierarchy, &path, tag)?
        .ok_or_else(|| anyhow!("{tag}: cannot morph root"))?;
    let node = hierarchy
        .get(node_id)
        .cloned()
        .ok_or_else(|| anyhow!("{tag}: node missing"))?;
    let (new_kind, new_ct) = match (node.kind, node.content_type.as_deref()) {
        (NodeKind::Paragraph, None | Some("typst")) => {
            (NodeKind::Paragraph, Some("hjson"))
        }
        (NodeKind::Paragraph, Some("hjson")) => (NodeKind::Script, Some("bund")),
        (NodeKind::Script, _) => (NodeKind::Paragraph, None),
        (k, ct) => {
            return Err(anyhow!(
                "{tag}: {} ({ct:?}) is not a text leaf",
                k.as_str()
            ));
        }
    };
    store
        .convert_leaf(&hierarchy, node_id, new_kind, new_ct)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

// ── ink.paragraph.set_status ─────────────────────────────────────────
// Stack: ( path status -- )
// status ∈ { "None", "Napkin", "First", "Second", "Third", "Final",
//           "Ready" }.

fn ink_paragraph_set_status(vm: &mut VM) -> BundResult<'_> {
    do_ink_paragraph_set_status(vm).map_err(to_bund_err)
}

fn do_ink_paragraph_set_status(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.paragraph.set_status";
    require_depth(vm, 2, tag)?;
    let status = value_to_string(pull(vm, tag)?, "status", tag)?;
    let path = value_to_string(pull(vm, tag)?, "path", tag)?;
    if !matches!(
        status.as_str(),
        "None" | "Napkin" | "First" | "Second" | "Third" | "Final" | "Ready"
    ) {
        return Err(anyhow!(
            "{tag}: unknown status `{status}`. Use None / Napkin / First / Second / Third / Final / Ready."
        ));
    }
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag} hierarchy: {e}"))?;
    let node_id = resolve_path(&hierarchy, &path, tag)?
        .ok_or_else(|| anyhow!("{tag}: empty path"))?;
    let mut node = hierarchy
        .get(node_id)
        .cloned()
        .ok_or_else(|| anyhow!("{tag}: node missing"))?;
    node.status = if status == "None" {
        None
    } else {
        Some(status)
    };
    node.modified_at = chrono::Utc::now();
    store
        .raw()
        .update_metadata(node.id, node.to_json())
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    store.sync().map_err(|e| anyhow!("{tag} sync: {e}"))?;
    Ok(vm)
}

// ── ink.paragraph.save ───────────────────────────────────────────────
// Stack: ( path body -- )
// Replaces the paragraph's content with `body`, mirrors to disk,
// re-embeds both vectors. Fires `hook.on_save`.

fn ink_paragraph_save(vm: &mut VM) -> BundResult<'_> {
    do_ink_paragraph_save(vm).map_err(to_bund_err)
}

fn do_ink_paragraph_save(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.paragraph.save";
    require_depth(vm, 2, tag)?;
    let body = value_to_string(pull(vm, tag)?, "body", tag)?;
    let path = value_to_string(pull(vm, tag)?, "path", tag)?;
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag} hierarchy: {e}"))?;
    let node_id = resolve_path(&hierarchy, &path, tag)?
        .ok_or_else(|| anyhow!("{tag}: empty path"))?;
    let mut node = hierarchy
        .get(node_id)
        .cloned()
        .ok_or_else(|| anyhow!("{tag}: node missing"))?;
    if !matches!(node.kind, NodeKind::Paragraph | NodeKind::Script) {
        return Err(anyhow!(
            "{tag}: {} is not a text leaf — only paragraphs / scripts are saveable",
            node.kind.as_str()
        ));
    }
    // Mirror the in-process save path: rewrite the on-disk file,
    // then update the store + reembed.
    if let Some(rel) = node.file.clone() {
        let abs = store.project_root().join(rel);
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| anyhow!("{tag} mkdir: {e}"))?;
        }
        std::fs::write(&abs, body.as_bytes())
            .map_err(|e| anyhow!("{tag} write: {e}"))?;
    }
    store
        .update_paragraph_content(&mut node, body.as_bytes())
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    store.sync().map_err(|e| anyhow!("{tag} sync: {e}"))?;
    Ok(vm)
}

// ── ink.db.sync / .checkpoint / .reindex ─────────────────────────────

fn ink_db_sync(vm: &mut VM) -> BundResult<'_> {
    do_ink_db_call(vm, "ink.db.sync", DbOp::Sync).map_err(to_bund_err)
}

fn ink_db_checkpoint(vm: &mut VM) -> BundResult<'_> {
    do_ink_db_call(vm, "ink.db.checkpoint", DbOp::Checkpoint).map_err(to_bund_err)
}

fn ink_db_reindex(vm: &mut VM) -> BundResult<'_> {
    do_ink_db_call(vm, "ink.db.reindex", DbOp::Reindex).map_err(to_bund_err)
}

enum DbOp {
    Sync,
    Checkpoint,
    Reindex,
}

fn do_ink_db_call<'a>(vm: &'a mut VM, tag: &str, op: DbOp) -> Result<&'a mut VM> {
    let store = active_store(tag)?;
    match op {
        DbOp::Sync => store.sync().map_err(|e| anyhow!("{tag}: {e}"))?,
        DbOp::Checkpoint => store.checkpoint().map_err(|e| anyhow!("{tag}: {e}"))?,
        DbOp::Reindex => {
            // Walk every Paragraph / Script node, re-read disk,
            // call update_paragraph_content where the bytes drifted.
            // Mirrors `cli/reindex.rs` without the orphan-adoption
            // path (that needs filesystem walking which is broader
            // than a Bund word should encompass).
            let hierarchy = Hierarchy::load(store)
                .map_err(|e| anyhow!("{tag} hierarchy: {e}"))?;
            let mut updated = 0usize;
            for node in hierarchy.iter() {
                if !matches!(node.kind, NodeKind::Paragraph | NodeKind::Script) {
                    continue;
                }
                let Some(rel) = node.file.as_ref() else {
                    continue;
                };
                let abs = store.project_root().join(rel);
                if !abs.is_file() {
                    continue;
                }
                let bytes = std::fs::read(&abs).map_err(|e| anyhow!("{tag} read: {e}"))?;
                let current = store
                    .get_content(node.id)
                    .map_err(|e| anyhow!("{tag} get: {e}"))?;
                if current.as_deref() == Some(bytes.as_slice()) {
                    continue;
                }
                let mut n = node.clone();
                store
                    .update_paragraph_content(&mut n, &bytes)
                    .map_err(|e| anyhow!("{tag} update: {e}"))?;
                updated += 1;
            }
            store.sync().map_err(|e| anyhow!("{tag} sync: {e}"))?;
            push(vm, Value::from_int(updated as i64));
            return Ok(vm);
        }
    }
    Ok(vm)
}

// ── ink.paragraph.set_target ─────────────────────────────────────────
// Stack: ( path target -- )
// Sets / clears the per-paragraph word-count goal. `target ≤ 0`
// clears the goal (also clears `target_hit_at_status` so the
// auto-promote machinery re-fires when a goal is set again).

fn ink_paragraph_set_target(vm: &mut VM) -> BundResult<'_> {
    do_ink_paragraph_set_target(vm).map_err(to_bund_err)
}

fn do_ink_paragraph_set_target(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.paragraph.set_target";
    require_depth(vm, 2, tag)?;
    let target = value_to_i64(pull(vm, tag)?, "target", tag)?;
    let path = value_to_string(pull(vm, tag)?, "path", tag)?;
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag} hierarchy: {e}"))?;
    let node_id = resolve_path(&hierarchy, &path, tag)?
        .ok_or_else(|| anyhow!("{tag}: empty path"))?;
    let mut node = hierarchy
        .get(node_id)
        .cloned()
        .ok_or_else(|| anyhow!("{tag}: node missing"))?;
    if node.kind != NodeKind::Paragraph {
        return Err(anyhow!("{tag}: `{}` is not a paragraph", node.title));
    }
    if target <= 0 {
        node.target_words = None;
        node.target_hit_at_status = None;
    } else {
        node.target_words = Some(target.clamp(0, i32::MAX as i64) as i32);
    }
    node.modified_at = chrono::Utc::now();
    store
        .raw()
        .update_metadata(node.id, node.to_json())
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    store.sync().map_err(|e| anyhow!("{tag} sync: {e}"))?;
    Ok(vm)
}

// ── ink.paragraph.target ─────────────────────────────────────────────
// Stack: ( path -- int | NODATA )
// Returns the paragraph's word-count goal (None → NODATA).

fn ink_paragraph_target(vm: &mut VM) -> BundResult<'_> {
    do_ink_paragraph_target(vm).map_err(to_bund_err)
}

fn do_ink_paragraph_target(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.paragraph.target";
    require_depth(vm, 1, tag)?;
    let path = value_to_string(pull(vm, tag)?, "path", tag)?;
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag} hierarchy: {e}"))?;
    let node_id = resolve_path(&hierarchy, &path, tag)?
        .ok_or_else(|| anyhow!("{tag}: empty path"))?;
    let node = hierarchy
        .get(node_id)
        .ok_or_else(|| anyhow!("{tag}: node missing"))?;
    match node.target_words {
        Some(n) => push(vm, Value::from_int(n as i64)),
        None => push(vm, Value::nodata()),
    }
    Ok(vm)
}
