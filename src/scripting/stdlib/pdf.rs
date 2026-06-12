//! 1.3.0 PDF-1 — `ink.pdf.*` Bund stdlib.
//!
//! PDF documents are held as opaque **handles** (small integers) in a
//! thread-local registry, not copied on every word boundary (RFC §9
//! open-Q 3).  `ink.pdf.load` returns a handle; the page / metadata ops
//! take one.  Mutating ops (`delete` / `rotate` / `reorder` / `set_*` /
//! `strip_metadata`) edit the handle's doc in place and push the same
//! handle back, so they chain; `extract` and `merge` return a fresh
//! handle (and `merge` consumes its two inputs).
//!
//! `load` / `save` confine their paths through the same project-root
//! sandbox as `ink.fs.*`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{pull, push, require_depth, resolve_fs_path, value_to_i64, value_to_string};
use crate::pdf::ops::{self, PageSpec, Rotation};
use crate::pdf::{self, PdfDoc};

thread_local! {
    static REGISTRY: RefCell<HashMap<i64, PdfDoc>> = RefCell::new(HashMap::new());
    static NEXT: Cell<i64> = const { Cell::new(1) };
}

fn store_doc(doc: PdfDoc) -> i64 {
    let id = NEXT.with(|n| {
        let id = n.get();
        n.set(id + 1);
        id
    });
    REGISTRY.with(|r| r.borrow_mut().insert(id, doc));
    id
}

fn take_doc(handle: i64, tag: &str) -> Result<PdfDoc> {
    REGISTRY
        .with(|r| r.borrow_mut().remove(&handle))
        .ok_or_else(|| anyhow!("{tag}: unknown pdf handle {handle}"))
}

/// Borrow the handle's doc mutably for `f`.  (Used for reads too — a
/// `&mut` borrow that only reads is fine.)
fn with_doc<R>(handle: i64, tag: &str, f: impl FnOnce(&mut PdfDoc) -> Result<R>) -> Result<R> {
    REGISTRY.with(|r| {
        let mut map = r.borrow_mut();
        let doc = map
            .get_mut(&handle)
            .ok_or_else(|| anyhow!("{tag}: unknown pdf handle {handle}"))?;
        f(doc)
    })
}

fn pe(e: pdf::Error) -> anyhow::Error {
    anyhow!("{e}")
}

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(e.to_string())
}

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.pdf.load", w_load),
        ("ink.pdf.save", w_save),
        ("ink.pdf.pages", w_pages),
        ("ink.pdf.extract", w_extract),
        ("ink.pdf.delete", w_delete),
        ("ink.pdf.rotate", w_rotate),
        ("ink.pdf.reorder", w_reorder),
        ("ink.pdf.merge", w_merge),
        ("ink.pdf.impose", w_impose),
        ("ink.pdf.title", w_title),
        ("ink.pdf.set_title", w_set_title),
        ("ink.pdf.set_author", w_set_author),
        ("ink.pdf.strip_metadata", w_strip_metadata),
    ];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f)
            .map_err(|e| anyhow!("register {name}: {e}"))?;
    }
    Ok(())
}

// ── words ───────────────────────────────────────────────────────────

// ( path -- handle )
fn w_load(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_load(vm).map_err(to_bund_err)
}
fn do_load(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.pdf.load";
    require_depth(vm, 1, tag)?;
    let path = value_to_string(pull(vm, tag)?, "path", tag)?;
    let resolved = resolve_fs_path(tag, &path)?;
    let doc = PdfDoc::load(&resolved).map_err(pe)?;
    push(vm, Value::from_int(store_doc(doc)));
    Ok(vm)
}

// ( handle path -- )
fn w_save(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_save(vm).map_err(to_bund_err)
}
fn do_save(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.pdf.save";
    require_depth(vm, 2, tag)?;
    let path = value_to_string(pull(vm, tag)?, "path", tag)?;
    let handle = value_to_i64(pull(vm, tag)?, "handle", tag)?;
    let resolved = resolve_fs_path(tag, &path)?;
    with_doc(handle, tag, |doc| doc.save(&resolved).map_err(pe))?;
    Ok(vm)
}

// ( handle -- n )
fn w_pages(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_pages(vm).map_err(to_bund_err)
}
fn do_pages(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.pdf.pages";
    require_depth(vm, 1, tag)?;
    let handle = value_to_i64(pull(vm, tag)?, "handle", tag)?;
    let n = with_doc(handle, tag, |doc| Ok(doc.page_count() as i64))?;
    push(vm, Value::from_int(n));
    Ok(vm)
}

// ( handle spec -- handle' )   keeps only the given pages, new doc
fn w_extract(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_extract(vm).map_err(to_bund_err)
}
fn do_extract(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.pdf.extract";
    require_depth(vm, 2, tag)?;
    let spec = value_to_string(pull(vm, tag)?, "spec", tag)?;
    let handle = value_to_i64(pull(vm, tag)?, "handle", tag)?;
    let pspec = PageSpec::parse(&spec).map_err(pe)?;
    let new = with_doc(handle, tag, |doc| ops::extract(&*doc, &pspec).map_err(pe))?;
    push(vm, Value::from_int(store_doc(new)));
    Ok(vm)
}

// ( handle spec -- handle )   in place
fn w_delete(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_delete(vm).map_err(to_bund_err)
}
fn do_delete(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.pdf.delete";
    require_depth(vm, 2, tag)?;
    let spec = value_to_string(pull(vm, tag)?, "spec", tag)?;
    let handle = value_to_i64(pull(vm, tag)?, "handle", tag)?;
    let pspec = PageSpec::parse(&spec).map_err(pe)?;
    with_doc(handle, tag, |doc| ops::delete(doc, &pspec).map_err(pe))?;
    push(vm, Value::from_int(handle));
    Ok(vm)
}

// ( handle spec degrees -- handle )   in place
fn w_rotate(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_rotate(vm).map_err(to_bund_err)
}
fn do_rotate(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.pdf.rotate";
    require_depth(vm, 3, tag)?;
    let degrees = value_to_i64(pull(vm, tag)?, "degrees", tag)?;
    let spec = value_to_string(pull(vm, tag)?, "spec", tag)?;
    let handle = value_to_i64(pull(vm, tag)?, "handle", tag)?;
    let pspec = PageSpec::parse(&spec).map_err(pe)?;
    let rot = Rotation::from_degrees(degrees).map_err(pe)?;
    with_doc(handle, tag, |doc| ops::rotate(doc, &pspec, rot).map_err(pe))?;
    push(vm, Value::from_int(handle));
    Ok(vm)
}

// ( handle mapping -- handle )   in place; 1-based permutation string
fn w_reorder(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_reorder(vm).map_err(to_bund_err)
}
fn do_reorder(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.pdf.reorder";
    require_depth(vm, 2, tag)?;
    let mapping = value_to_string(pull(vm, tag)?, "mapping", tag)?;
    let handle = value_to_i64(pull(vm, tag)?, "handle", tag)?;
    let map: Vec<usize> = mapping
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<usize>()
                .ok()
                .filter(|&n| n >= 1)
                .map(|n| n - 1)
                .ok_or_else(|| anyhow!("{tag}: mapping must be comma-separated 1-based page numbers"))
        })
        .collect::<Result<_>>()?;
    with_doc(handle, tag, |doc| ops::reorder(doc, &map).map_err(pe))?;
    push(vm, Value::from_int(handle));
    Ok(vm)
}

// ( handle1 handle2 -- handle' )   concatenate; consumes both inputs
fn w_merge(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_merge(vm).map_err(to_bund_err)
}
fn do_merge(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.pdf.merge";
    require_depth(vm, 2, tag)?;
    let h2 = value_to_i64(pull(vm, tag)?, "handle2", tag)?;
    let h1 = value_to_i64(pull(vm, tag)?, "handle1", tag)?;
    let d1 = take_doc(h1, tag)?;
    let d2 = take_doc(h2, tag)?;
    let merged = ops::merge(&[d1, d2]).map_err(pe)?;
    push(vm, Value::from_int(store_doc(merged)));
    Ok(vm)
}

// ( handle profile -- handle' )   impose into signatures, new doc
fn w_impose(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_impose(vm).map_err(to_bund_err)
}
fn do_impose(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.pdf.impose";
    require_depth(vm, 2, tag)?;
    let profile = value_to_string(pull(vm, tag)?, "profile", tag)?;
    let handle = value_to_i64(pull(vm, tag)?, "handle", tag)?;
    // Imposition profiles from the active project config (project +
    // global cascade); the built-in `default` / `chapbook` profiles work
    // even with no project registered.
    let imp_cfg = crate::scripting::active_config()
        .map(|c| c.imposition.clone())
        .unwrap_or_default();
    let params = imp_cfg
        .resolve(&profile)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    let new = with_doc(handle, tag, |doc| {
        crate::pdf::impose::impose(&*doc, &params).map_err(pe)
    })?;
    push(vm, Value::from_int(store_doc(new)));
    Ok(vm)
}

// ( handle -- title )
fn w_title(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_title(vm).map_err(to_bund_err)
}
fn do_title(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.pdf.title";
    require_depth(vm, 1, tag)?;
    let handle = value_to_i64(pull(vm, tag)?, "handle", tag)?;
    let title = with_doc(handle, tag, |doc| {
        Ok(pdf::meta::read_metadata(doc).title.unwrap_or_default())
    })?;
    push(vm, Value::from_string(title));
    Ok(vm)
}

// ( handle title -- handle )
fn w_set_title(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_set_field(vm, "ink.pdf.set_title", Field::Title).map_err(to_bund_err)
}
// ( handle author -- handle )
fn w_set_author(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_set_field(vm, "ink.pdf.set_author", Field::Author).map_err(to_bund_err)
}

enum Field {
    Title,
    Author,
}

fn do_set_field<'a>(vm: &'a mut VM, tag: &str, field: Field) -> Result<&'a mut VM> {
    require_depth(vm, 2, tag)?;
    let val = value_to_string(pull(vm, tag)?, "value", tag)?;
    let handle = value_to_i64(pull(vm, tag)?, "handle", tag)?;
    with_doc(handle, tag, move |doc| {
        let mut m = pdf::meta::read_metadata(doc);
        match field {
            Field::Title => m.title = Some(val),
            Field::Author => m.author = Some(val),
        }
        pdf::meta::write_metadata(doc, &m).map_err(pe)
    })?;
    push(vm, Value::from_int(handle));
    Ok(vm)
}

// ( handle -- handle )
fn w_strip_metadata(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_strip(vm).map_err(to_bund_err)
}
fn do_strip(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.pdf.strip_metadata";
    require_depth(vm, 1, tag)?;
    let handle = value_to_i64(pull(vm, tag)?, "handle", tag)?;
    with_doc(handle, tag, |doc| pdf::meta::strip_metadata(doc).map_err(pe))?;
    push(vm, Value::from_int(handle));
    Ok(vm)
}
