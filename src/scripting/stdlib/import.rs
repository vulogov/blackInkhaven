//! 3.0.4 Phase-3 — `ink.import.*` Bund stdlib: the Scrivener / EPUB importers,
//! write-side (default-denied). `scrivener` / `epub` create Book → Chapter →
//! Paragraph nodes in the ACTIVE store (store_write — opt in via
//! `enabled_categories: ["store_write"]`); `scrivener_preview` is a dry run that
//! reports what *would* be created and writes nothing (fs_read).
//!
//! The bundle path is confined by the fs sandbox (`resolve_fs_path`): a bundle
//! that lives outside the project needs `scripting.fs_unsandboxed: true`, exactly
//! like `ink.fs.read`. So importing an external `.scriv`/`.epub` is a deliberate
//! double opt-in (store_write to run + fs_unsandboxed for the external path).
//!
//! - `ink.import.scrivener`         ( path -- dict )  import a `.scriv` bundle.
//! - `ink.import.epub`              ( path -- dict )  import an `.epub` file.
//! - `ink.import.scrivener_preview` ( path -- dict )  dry-run a `.scriv` import.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{
    active_config, active_store, pull, push, require_depth, resolve_fs_path, value_to_string,
};
use crate::epub_import::import::{import_epub, EpubImportOpts, EpubImportReport};
use crate::scrivener::import::{import_scrivener_project, ImportOpts, ImportReport};

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.import.scrivener", w_scrivener),
        ("ink.import.epub", w_epub),
        ("ink.import.scrivener_preview", w_scrivener_preview),
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

fn strs(xs: &[String]) -> Value {
    Value::from_list(xs.iter().map(Value::from_string).collect())
}

fn scrivener_dict(r: &ImportReport) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("books_created".into(), Value::from_int(r.books_created as i64));
    m.insert("chapters_created".into(), Value::from_int(r.chapters_created as i64));
    m.insert("subchapters_created".into(), Value::from_int(r.subchapters_created as i64));
    m.insert("paragraphs_created".into(), Value::from_int(r.paragraphs_created as i64));
    m.insert("paragraphs_skipped".into(), Value::from_int(r.paragraphs_skipped as i64));
    m.insert("errors".into(), strs(&r.errors));
    Value::from_dict(m)
}

fn epub_dict(r: &EpubImportReport) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("book_title".into(), Value::from_string(&r.book_title));
    if let Some(author) = &r.author {
        m.insert("author".into(), Value::from_string(author));
    }
    m.insert("chapters_created".into(), Value::from_int(r.chapters_created as i64));
    m.insert("paragraphs_created".into(), Value::from_int(r.paragraphs_created as i64));
    m.insert("images_imported".into(), Value::from_int(r.images_imported as i64));
    m.insert("images_extracted".into(), Value::from_int(r.images_extracted as i64));
    m.insert("errors".into(), strs(&r.errors));
    Value::from_dict(m)
}

/// Pull a bundle path off the stack and confine it to the fs sandbox.
fn pull_path(vm: &mut VM, tag: &str) -> Result<std::path::PathBuf> {
    require_depth(vm, 1, tag)?;
    let raw = value_to_string(pull(vm, tag)?, "path", tag)?;
    resolve_fs_path(tag, &raw)
}

word!(w_scrivener, do_scrivener);
fn do_scrivener(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.import.scrivener";
    let path = pull_path(vm, tag)?;
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let report = import_scrivener_project(&path, store, cfg, &ImportOpts::default())
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, scrivener_dict(&report));
    Ok(vm)
}

word!(w_scrivener_preview, do_scrivener_preview);
fn do_scrivener_preview(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.import.scrivener_preview";
    let path = pull_path(vm, tag)?;
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    // dry_run: parse + count what would be created; write nothing.
    let opts = ImportOpts { dry_run: true, ..Default::default() };
    let report = import_scrivener_project(&path, store, cfg, &opts)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, scrivener_dict(&report));
    Ok(vm)
}

word!(w_epub, do_epub);
fn do_epub(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.import.epub";
    let path = pull_path(vm, tag)?;
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let report = import_epub(&path, store, cfg, &EpubImportOpts::default())
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, epub_dict(&report));
    Ok(vm)
}
