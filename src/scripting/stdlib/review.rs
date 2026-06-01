//! 1.2.16+ Phase I.4.a — Bund stdlib expansion:
//! review tools.
//!
//! Three words for programmatic interaction with
//! the inline-comments system (1.2.14 Phase C):
//!
//!   * `ink.review.list`        — `( -- list )` —
//!     return every comment in the project as a
//!     list of dicts.  Each entry carries the
//!     paragraph UUID + slug it lives under so a
//!     consumer can group/filter cheaply.  Default-
//!     allowed (`store_read`).
//!   * `ink.review.add_comment` — `( para_uuid
//!     char_start char_end body -- comment_uuid )`
//!     — create a new comment anchored to the
//!     char range.  Sidecar `<para>.comments.
//!     json` is touched.  Atomic write via
//!     `io_atomic`.  Default-denied
//!     (`store_write`).
//!   * `ink.review.resolve`     — `( comment_uuid
//!     -- bool )` — close a comment by id.  Returns
//!     true on success, false if the comment id
//!     wasn't found anywhere in the project.
//!     Default-denied (`store_write`).
//!
//! The category gates inherit from the existing
//! `store_read` / `store_write` taxonomy — a
//! project that already enables `store_write` to
//! let scripts mutate the tree automatically
//! grants review-write too.  This matches the
//! "review tools are a sub-flavour of tree
//! mutation" mental model.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use chrono::Utc;
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;
use uuid::Uuid;

use super::helpers::{active_store, pull, push, require_depth, value_to_string, value_to_uuid};
use crate::store::{hierarchy::Hierarchy, NodeKind, Store};
use crate::tui::comments::{
    load_from_sidecar, save_to_sidecar, sidecar_path, Comment,
};

pub fn register(vm: &mut VM) -> Result<()> {
    vm.register_inline("ink.review.list".to_string(), ink_review_list)
        .map_err(|e| anyhow!("register ink.review.list: {e}"))?;
    vm.register_inline("ink.review.add_comment".to_string(), ink_review_add_comment)
        .map_err(|e| anyhow!("register ink.review.add_comment: {e}"))?;
    vm.register_inline("ink.review.resolve".to_string(), ink_review_resolve)
        .map_err(|e| anyhow!("register ink.review.resolve: {e}"))?;
    Ok(())
}

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(e.to_string())
}

// ── ink.review.list ──────────────────────────────────────────────────
// Stack: ( -- list )

fn ink_review_list(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_review_list(vm).map_err(to_bund_err)
}

fn do_ink_review_list(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.review.list";
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag} hierarchy: {e}"))?;
    let root = store.project_root();
    let mut items: Vec<Value> = Vec::new();
    for node in hierarchy.iter() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(rel) = node.file.as_ref() else { continue };
        let abs = root.join(rel);
        let path = sidecar_path(&abs);
        if !path.exists() {
            continue;
        }
        let file = match load_from_sidecar(&abs) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(
                    target: "inkhaven::review",
                    "{tag}: sidecar load failed for {}: {e}",
                    abs.display(),
                );
                continue;
            }
        };
        for c in &file.comments {
            items.push(comment_dict(c, node));
        }
    }
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn comment_dict(c: &Comment, paragraph: &crate::store::node::Node) -> Value {
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("id".into(), Value::from_string(c.id.to_string()));
    h.insert(
        "paragraph_id".into(),
        Value::from_string(paragraph.id.to_string()),
    );
    h.insert(
        "paragraph_slug".into(),
        Value::from_string(&paragraph.slug),
    );
    h.insert("char_start".into(), Value::from_int(c.char_start as i64));
    h.insert("char_end".into(), Value::from_int(c.char_end as i64));
    h.insert("author".into(), Value::from_string(&c.author));
    h.insert(
        "created_at".into(),
        Value::from_string(c.created_at.to_rfc3339()),
    );
    h.insert("resolved".into(), Value::from_bool(c.resolved));
    if let Some(r) = c.resolved_at {
        h.insert(
            "resolved_at".into(),
            Value::from_string(r.to_rfc3339()),
        );
    }
    h.insert("text".into(), Value::from_string(&c.text));
    h.insert("reply_count".into(), Value::from_int(c.replies.len() as i64));
    Value::from_dict(h)
}

// ── ink.review.add_comment ───────────────────────────────────────────
// Stack: ( para_uuid char_start char_end body -- comment_uuid )

fn ink_review_add_comment(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_review_add_comment(vm).map_err(to_bund_err)
}

fn do_ink_review_add_comment(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.review.add_comment";
    require_depth(vm, 4, tag)?;
    let body = value_to_string(pull(vm, tag)?, "body", tag)?;
    let char_end = pull_usize(vm, "char_end", tag)?;
    let char_start = pull_usize(vm, "char_start", tag)?;
    let para_uuid = value_to_uuid(pull(vm, tag)?, tag)?;
    if char_start > char_end {
        return Err(anyhow!(
            "{tag}: char_start ({char_start}) > char_end ({char_end})"
        ));
    }
    if body.trim().is_empty() {
        return Err(anyhow!("{tag}: body cannot be empty"));
    }
    let store = active_store(tag)?;
    let abs_path = paragraph_abs_path(store, para_uuid, tag)?;

    let mut file = load_from_sidecar(&abs_path).map_err(|e| anyhow!("{tag}: {e}"))?;
    // Build the new comment.  Author taken from
    // env vars via the existing resolver — matches
    // what the TUI does for interactive Ctrl+V c
    // comments.
    let author = crate::tui::comments::resolve_author(None);
    let new_id = Uuid::new_v4();
    let comment = Comment {
        id: new_id,
        char_start,
        char_end,
        author,
        created_at: Utc::now(),
        resolved: false,
        resolved_at: None,
        text: body,
        replies: Vec::new(),
    };
    file.comments.push(comment);
    save_to_sidecar(&abs_path, &file).map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_string(new_id.to_string()));
    Ok(vm)
}

// ── ink.review.resolve ───────────────────────────────────────────────
// Stack: ( comment_uuid -- bool )

fn ink_review_resolve(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ink_review_resolve(vm).map_err(to_bund_err)
}

fn do_ink_review_resolve(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.review.resolve";
    require_depth(vm, 1, tag)?;
    let target = value_to_uuid(pull(vm, tag)?, tag)?;
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag} hierarchy: {e}"))?;
    let root = store.project_root();
    let now = Utc::now();
    for node in hierarchy.iter() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(rel) = node.file.as_ref() else { continue };
        let abs = root.join(rel);
        let path = sidecar_path(&abs);
        if !path.exists() {
            continue;
        }
        let mut file = match load_from_sidecar(&abs) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut touched = false;
        for c in &mut file.comments {
            if c.id == target && !c.resolved {
                c.resolved = true;
                c.resolved_at = Some(now);
                touched = true;
                break;
            }
        }
        if touched {
            save_to_sidecar(&abs, &file).map_err(|e| anyhow!("{tag}: {e}"))?;
            push(vm, Value::from_bool(true));
            return Ok(vm);
        }
    }
    // No matching comment found anywhere.
    push(vm, Value::from_bool(false));
    Ok(vm)
}

/// Helper — pull a non-negative integer from the
/// stack.  Used by `add_comment` for char_start /
/// char_end.
fn pull_usize(vm: &mut VM, field: &str, err_prefix: &str) -> Result<usize> {
    let v = pull(vm, err_prefix)?;
    let n = v
        .cast_int()
        .map_err(|e| anyhow!("{err_prefix}: {field} must be an integer: {e}"))?;
    if n < 0 {
        return Err(anyhow!(
            "{err_prefix}: {field} must be non-negative (got {n})"
        ));
    }
    Ok(n as usize)
}

/// Helper — resolve a paragraph UUID to its abs
/// path on disk via the hierarchy.
fn paragraph_abs_path(
    store: &Store,
    para_uuid: Uuid,
    err_prefix: &str,
) -> Result<PathBuf> {
    let hierarchy =
        Hierarchy::load(store).map_err(|e| anyhow!("{err_prefix} hierarchy: {e}"))?;
    let node = hierarchy
        .get(para_uuid)
        .ok_or_else(|| anyhow!("{err_prefix}: paragraph {para_uuid} not found"))?;
    if node.kind != NodeKind::Paragraph {
        return Err(anyhow!(
            "{err_prefix}: {para_uuid} is a `{:?}`, not a Paragraph",
            node.kind,
        ));
    }
    let rel = node
        .file
        .as_ref()
        .ok_or_else(|| anyhow!("{err_prefix}: paragraph {para_uuid} has no on-disk file"))?;
    Ok(store.project_root().join(rel))
}

/// Suppress unused-imports warning when nothing else
/// in the module uses Path directly.
#[allow(dead_code)]
fn _path_anchor(_p: &Path) {}
