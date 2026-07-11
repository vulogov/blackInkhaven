//! 1.3.1 SUBMISSION-1 P3.5 — `ink.export.*` Bund stdlib.
//!
//! Export a user book to `docx` / `manuscript` (Shunn typst) / `markdown`
//! / `tex` / `epub` from a Bund script, so a release script can emit every
//! artefact in one pass — completing for the prose formats what
//! `ink.pdf.*` started for PDFs.
//!
//! Each word is `( book path -- )`: `book` is a case-insensitive title /
//! slug (empty → the sole user book), `path` is sandboxed to the project
//! root exactly like `ink.fs.write` / `ink.pdf.save`.  All five are gated
//! `fs_write` in the scripting policy.

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{
    active_config, active_store, pull, require_depth, resolve_fs_path, value_to_string,
};
use crate::manuscript::{ManuscriptChapter, ManuscriptMeta};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

type BundResult<'a> = std::result::Result<&'a mut VM, BundError>;

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(e.to_string())
}

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> BundResult<'_>)] = &[
        ("ink.export.docx", w_docx),
        ("ink.export.manuscript", w_manuscript),
        ("ink.export.markdown", w_markdown),
        ("ink.export.tex", w_tex),
        ("ink.export.epub", w_epub),
    ];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f)
            .map_err(|e| anyhow!("register {name}: {e}"))?;
    }
    Ok(())
}

/// Resolved `( book path -- )` context: sandboxed output path + project
/// layout/config/hierarchy + the resolved book node.
struct ExportCtx {
    path: std::path::PathBuf,
    layout: ProjectLayout,
    cfg: &'static crate::config::Config,
    h: Hierarchy,
    book: Node,
}

fn prologue(vm: &mut VM, tag: &str) -> Result<ExportCtx> {
    require_depth(vm, 2, tag)?;
    let path = value_to_string(pull(vm, tag)?, "path", tag)?;
    let book = value_to_string(pull(vm, tag)?, "book", tag)?;
    let resolved = resolve_fs_path(tag, &path)?;
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: hierarchy: {e}"))?;
    let bn = if book.trim().is_empty() {
        None
    } else {
        Some(book.as_str())
    };
    let node = crate::cli::resolve_user_book(&h, bn, tag)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .clone();
    Ok(ExportCtx {
        path: resolved,
        layout,
        cfg,
        h,
        book: node,
    })
}

fn model(ctx: &ExportCtx) -> Result<(ManuscriptMeta, Vec<ManuscriptChapter>)> {
    crate::cli::manuscript::build_model(&ctx.layout, ctx.cfg, &ctx.h, &ctx.book, None, None, None)
        .map_err(|e| anyhow!("{e}"))
}

fn combined(ctx: &ExportCtx, tag: &str) -> Result<String> {
    crate::export::assemble_typst_source_filtered(&ctx.layout, &ctx.h, Some(ctx.book.id), None, None)
        .map_err(|e| anyhow!("{tag}: assemble: {e}"))
}

fn write_bytes(ctx: &ExportCtx, tag: &str, bytes: &[u8]) -> Result<()> {
    crate::io_atomic::write(&ctx.path, bytes)
        .map_err(|e| anyhow!("{tag}: write {}: {e}", ctx.path.display()))
}

// ── words ───────────────────────────────────────────────────────────

fn w_docx(vm: &mut VM) -> BundResult<'_> {
    do_docx(vm).map_err(to_bund_err)
}
fn do_docx(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.export.docx";
    let ctx = prologue(vm, tag)?;
    let (meta, chapters) = model(&ctx)?;
    let bytes = crate::export::docx::build_docx(
        &meta,
        &chapters,
        crate::export::docx::DocxFont::TimesNewRoman,
    )
    .map_err(|e| anyhow!("{tag}: {e}"))?;
    write_bytes(&ctx, tag, &bytes)?;
    Ok(vm)
}

fn w_manuscript(vm: &mut VM) -> BundResult<'_> {
    do_manuscript(vm).map_err(to_bund_err)
}
fn do_manuscript(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.export.manuscript";
    let ctx = prologue(vm, tag)?;
    let (meta, chapters) = model(&ctx)?;
    let typ = crate::manuscript::build_typst(&meta, &chapters);
    write_bytes(&ctx, tag, typ.as_bytes())?;
    Ok(vm)
}

fn w_markdown(vm: &mut VM) -> BundResult<'_> {
    do_markdown(vm).map_err(to_bund_err)
}
fn do_markdown(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.export.markdown";
    let ctx = prologue(vm, tag)?;
    let combined = combined(&ctx, tag)?;
    crate::export::build_markdown(&combined)
        .write_to(&ctx.path)
        .map_err(|e| anyhow!("{tag}: write {}: {e}", ctx.path.display()))?;
    Ok(vm)
}

fn w_tex(vm: &mut VM) -> BundResult<'_> {
    do_tex(vm).map_err(to_bund_err)
}
fn do_tex(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.export.tex";
    let ctx = prologue(vm, tag)?;
    let combined = combined(&ctx, tag)?;
    crate::export::build_tex(&combined, &ctx.cfg.tex_export)
        .write_to(&ctx.path)
        .map_err(|e| anyhow!("{tag}: write {}: {e}", ctx.path.display()))?;
    Ok(vm)
}

fn w_epub(vm: &mut VM) -> BundResult<'_> {
    do_epub(vm).map_err(to_bund_err)
}
fn do_epub(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.export.epub";
    let ctx = prologue(vm, tag)?;
    let combined = combined(&ctx, tag)?;
    let md = crate::export::markdown::typst_to_markdown(&combined);
    let title = crate::cli::epub::clean_title(&ctx.book.title);
    crate::export::build_epub(&md, &title)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .write_to(&ctx.path)
        .map_err(|e| anyhow!("{tag}: write {}: {e}", ctx.path.display()))?;
    Ok(vm)
}
