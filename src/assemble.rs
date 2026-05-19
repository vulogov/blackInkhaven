//! Book assembly — bound to Ctrl+B A.
//!
//! Walks the subtree of a single user Book, copies it into
//! `<artefacts-root>/<book-slug>/book/`, and synthesises an
//! `index.typ` at every level that imports children and calls the
//! `wrap_*` functions defined in the user's per-book `globals.typ`.
//! The Typst system book's chapter named after the book also
//! contributes its `globals.typ` / `settings.typ` to the output root,
//! plus a top-level `<slug>.typ` that imports both and calls
//! `wrap_book(...)` on `book/index.typ`. The resulting tree is what
//! `typst compile` runs against.
//!
//! The assembler is pure I/O: it reads from `Store` + filesystem and
//! writes to `<artefacts-root>`. No bdslib writes. Callers can pass a
//! progress callback that fires after each output file is written; the
//! TUI uses that to drive its splash redraws.

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::SYSTEM_TAG_TYPST;

/// Per-file progress signal. `label` is the path being written
/// relative to `<artefacts-root>` so the splash can show what's
/// currently being assembled.
pub type ProgressFn<'a> = dyn FnMut(usize, usize, &Path) + 'a;

/// Aggregate result of one assembly run.
#[derive(Debug, Default)]
pub struct AssemblyReport {
    pub files_written: usize,
    pub root_typ: PathBuf,
}

/// Assemble `book_node` (must be a root-level Book that isn't a system
/// book) into the configured artefacts directory. Returns the absolute
/// path of the root `<slug>.typ` so the caller can surface it in the
/// status bar — that's what the user passes to `typst compile`.
///
/// Wipes `<artefacts>/<book-slug>/book/` before re-emitting so
/// stale chapter directories from a previous assembly don't linger.
/// The sibling `globals.typ` / `settings.typ` / `<slug>.typ` are
/// rewritten in place — they're tiny and the user is meant to
/// customise them through the Typst system book paragraphs, not by
/// editing the artefacts copies.
pub fn assemble_book(
    store: &Store,
    layout: &ProjectLayout,
    cfg: &Config,
    book_node: &Node,
    progress: &mut ProgressFn,
) -> Result<AssemblyReport> {
    if book_node.kind != NodeKind::Book || book_node.parent_id.is_some() {
        return Err(Error::Store(format!(
            "assemble: `{}` is not a root-level book",
            book_node.title
        )));
    }
    if book_node.system_tag.is_some() {
        return Err(Error::Store(format!(
            "assemble: `{}` is a system book — pick a user book",
            book_node.title
        )));
    }

    let hierarchy = Hierarchy::load(store)?;
    let artefacts_root = store.resolve_artefacts_dir(cfg);
    let out_book = artefacts_root.join(&book_node.slug);
    let out_book_subtree = out_book.join("book");

    // Pre-count work for the progress bar. Each paragraph file is one
    // unit; each branch's index.typ is one unit; plus three top-level
    // files (root .typ, settings, globals).
    let total = count_work(&hierarchy, book_node);
    let mut done: usize = 0;

    // Wipe the previous `book/` tree but keep the sibling configs in
    // place — `<slug>.typ`, `settings.typ`, `globals.typ` are
    // overwritten further down, but anything else the user dropped in
    // (e.g. fonts/) survives.
    if out_book_subtree.exists() {
        std::fs::remove_dir_all(&out_book_subtree).map_err(Error::Io)?;
    }
    std::fs::create_dir_all(&out_book_subtree).map_err(Error::Io)?;
    std::fs::create_dir_all(&out_book).map_err(Error::Io)?;

    // Walk the book's children and emit the subtree.
    write_branch(
        store,
        layout,
        &hierarchy,
        book_node,
        &out_book_subtree,
        BranchLevel::BookRoot,
        &mut done,
        total,
        &artefacts_root,
        progress,
    )?;

    // Extract the Typst system book's chapter (titled the same as the
    // user book) → its three seed paragraphs map to the output's
    // globals.typ / settings.typ / index.typ.
    let typst_root_index_body =
        copy_typst_skeleton_files(store, layout, &hierarchy, book_node, &out_book, &artefacts_root, &mut done, total, progress)?;

    // Root .typ for the book — applies settings, calls wrap_book on
    // the assembled subtree. The Typst chapter's index.typ body is
    // appended so any user setup (image search paths, imports of
    // additional helpers) flows through.
    let root_typ = out_book.join(format!("{}.typ", book_node.slug));
    let root_body = build_root_typ(book_node, &typst_root_index_body);
    std::fs::write(&root_typ, root_body.as_bytes()).map_err(Error::Io)?;
    done += 1;
    progress(done, total, &PathBuf::from(format!("{}.typ", book_node.slug)));

    Ok(AssemblyReport {
        files_written: done,
        root_typ,
    })
}

/// Count files the assembler will write. Used to pre-size the progress
/// bar — exact total isn't required for correctness, just a tighter
/// "X%" readout.
fn count_work(hierarchy: &Hierarchy, book: &Node) -> usize {
    let mut count: usize = 1; // root <slug>.typ
    count += 3; // globals.typ + settings.typ + book-root index.typ from typst chapter
    for id in hierarchy.collect_subtree(book.id) {
        let Some(n) = hierarchy.get(id) else { continue };
        match n.kind {
            NodeKind::Book => count += 1, // book/index.typ
            NodeKind::Chapter | NodeKind::Subchapter => count += 1,
            NodeKind::Paragraph => count += 1,
        }
    }
    count
}

#[derive(Clone, Copy)]
enum BranchLevel {
    /// The book itself — produces `book/index.typ` listing chapters.
    BookRoot,
    /// A nested chapter / subchapter — produces an index.typ wrapped
    /// in `wrap_chapter` / `wrap_subchapter`.
    Chapter,
    Subchapter,
}

/// Recursively emit `<out_dir>/index.typ` plus children (paragraph
/// files copied as-is, sub-branches recursed into their own
/// directories named `<NN-slug>`).
fn write_branch(
    store: &Store,
    layout: &ProjectLayout,
    hierarchy: &Hierarchy,
    branch: &Node,
    out_dir: &Path,
    level: BranchLevel,
    done: &mut usize,
    total: usize,
    artefacts_root: &Path,
    progress: &mut ProgressFn,
) -> Result<()> {
    std::fs::create_dir_all(out_dir).map_err(Error::Io)?;

    // Children, sorted by `order` (children_of already returns them
    // sorted).
    let children = hierarchy.children_of(Some(branch.id));

    // Emit per-child output first so the parent's index.typ can
    // reference filenames that already exist.
    let mut child_refs: Vec<ChildRef> = Vec::new();
    for child in &children {
        match child.kind {
            NodeKind::Paragraph => {
                let fname = child.fs_name(); // "NN-slug.typ"
                let dst = out_dir.join(&fname);
                copy_paragraph_file(layout, child, &dst)?;
                *done += 1;
                let rel = dst.strip_prefix(artefacts_root).unwrap_or(&dst);
                progress(*done, total, rel);
                child_refs.push(ChildRef::Paragraph { fname });
            }
            NodeKind::Chapter | NodeKind::Subchapter => {
                let dname = child.fs_name(); // "NN-slug"
                let dst_dir = out_dir.join(&dname);
                let next_level = if child.kind == NodeKind::Chapter {
                    BranchLevel::Chapter
                } else {
                    BranchLevel::Subchapter
                };
                write_branch(
                    store,
                    layout,
                    hierarchy,
                    child,
                    &dst_dir,
                    next_level,
                    done,
                    total,
                    artefacts_root,
                    progress,
                )?;
                child_refs.push(ChildRef::Branch { dname });
            }
            NodeKind::Book => {
                // Books can't be nested under other books in this
                // hierarchy; skip defensively.
            }
        }
    }

    // Write the index.typ for this branch.
    let index_path = out_dir.join("index.typ");
    let depth = match level {
        BranchLevel::BookRoot => 1,  // book/index.typ → ../globals.typ
        BranchLevel::Chapter => 2,   // book/<chap>/index.typ → ../../globals.typ
        BranchLevel::Subchapter => 3, // book/<chap>/<sub>/index.typ → ../../../globals.typ
    };
    let globals_rel = "../".repeat(depth) + "globals.typ";
    let body = build_branch_index(branch, level, &child_refs, &globals_rel);
    std::fs::write(&index_path, body.as_bytes()).map_err(Error::Io)?;
    *done += 1;
    let rel = index_path.strip_prefix(artefacts_root).unwrap_or(&index_path);
    progress(*done, total, rel);

    Ok(())
}

/// References each `index.typ` keeps to its children so it can emit
/// the right include / wrap_paragraph / sub-include line.
enum ChildRef {
    Paragraph { fname: String },
    Branch { dname: String },
}

fn build_branch_index(
    branch: &Node,
    level: BranchLevel,
    children: &[ChildRef],
    globals_rel: &str,
) -> String {
    let mut out = String::new();
    out.push_str("// Auto-generated by inkhaven Book assembly.\n");
    out.push_str(&format!("#import \"{globals_rel}\": *\n\n"));

    // Body content — same for all levels, but wrapped differently.
    let mut body = String::new();
    for child in children {
        match child {
            ChildRef::Paragraph { fname } => {
                body.push_str(&format!(
                    "  wrap_paragraph(include \"{fname}\")\n"
                ));
            }
            ChildRef::Branch { dname } => {
                body.push_str(&format!(
                    "  include \"{dname}/index.typ\"\n"
                ));
            }
        }
    }
    if body.is_empty() {
        body.push_str("  // (empty branch)\n");
    }

    match level {
        BranchLevel::BookRoot => {
            // book/index.typ is just the concatenation — the root
            // <slug>.typ wraps the whole thing in wrap_book().
            out.push_str("{\n");
            out.push_str(&body);
            out.push_str("}\n");
        }
        BranchLevel::Chapter => {
            let title = escape_typst_string(&branch.title);
            out.push_str(&format!("#wrap_chapter(\"{title}\", {{\n"));
            out.push_str(&body);
            out.push_str("})\n");
        }
        BranchLevel::Subchapter => {
            let title = escape_typst_string(&branch.title);
            out.push_str(&format!("#wrap_subchapter(\"{title}\", {{\n"));
            out.push_str(&body);
            out.push_str("})\n");
        }
    }
    out
}

/// Strip the leading `= Title\n` editor-chrome heading off a paragraph
/// body when writing it into the artefacts tree. Paragraph editor
/// titles are an inkhaven concept, not part of the user's prose, so
/// they shouldn't surface in the compiled PDF.
fn copy_paragraph_file(layout: &ProjectLayout, node: &Node, dst: &Path) -> Result<()> {
    let Some(rel) = &node.file else {
        return Err(Error::Store(format!(
            "assemble: paragraph `{}` has no file on disk",
            node.title
        )));
    };
    let src = layout.root.join(rel);
    let body = std::fs::read_to_string(&src).map_err(Error::Io)?;
    let body = strip_leading_heading(&body);
    std::fs::write(dst, body.as_bytes()).map_err(Error::Io)?;
    Ok(())
}

/// Drop a leading `= ...` heading line (and any blank lines that
/// immediately follow) from a paragraph body. Mirrors what
/// `strip_leading_typst_heading` in the AI prompt path does.
fn strip_leading_heading(body: &str) -> String {
    let mut lines: Vec<&str> = body.lines().collect();
    if let Some(first) = lines.first() {
        if first.trim_start().starts_with('=') {
            lines.remove(0);
            while lines.first().is_some_and(|l| l.trim().is_empty()) {
                lines.remove(0);
            }
        }
    }
    lines.join("\n")
}

/// Copy the Typst system book's matching chapter's `globals.typ` /
/// `settings.typ` to the artefacts directory, and return the body of
/// the chapter's own `index.typ` so the root `<slug>.typ` can inline
/// it before calling `wrap_book(...)`.
fn copy_typst_skeleton_files(
    _store: &Store,
    layout: &ProjectLayout,
    hierarchy: &Hierarchy,
    book: &Node,
    out_book: &Path,
    artefacts_root: &Path,
    done: &mut usize,
    total: usize,
    progress: &mut ProgressFn,
) -> Result<String> {
    let typst_book = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_TYPST)
        })
        .cloned()
        .ok_or_else(|| Error::Store("assemble: Typst system book not found".into()))?;
    let chapter = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Chapter
                && n.parent_id == Some(typst_book.id)
                && n.title == book.title
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store(format!(
                "assemble: no Typst chapter named `{}` — open the book once \
                 to seed it, or re-create it under Typst",
                book.title
            ))
        })?;

    let mut index_body = String::new();
    for child in hierarchy.children_of(Some(chapter.id)) {
        if child.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(rel) = &child.file else { continue };
        let src = layout.root.join(rel);
        let body = std::fs::read_to_string(&src).map_err(Error::Io)?;
        let stripped = strip_leading_heading(&body);
        match child.title.as_str() {
            "globals.typ" => {
                let dst = out_book.join("globals.typ");
                std::fs::write(&dst, stripped.as_bytes()).map_err(Error::Io)?;
                *done += 1;
                let rel = dst.strip_prefix(artefacts_root).unwrap_or(&dst);
                progress(*done, total, rel);
            }
            "settings.typ" => {
                let dst = out_book.join("settings.typ");
                std::fs::write(&dst, stripped.as_bytes()).map_err(Error::Io)?;
                *done += 1;
                let rel = dst.strip_prefix(artefacts_root).unwrap_or(&dst);
                progress(*done, total, rel);
            }
            "index.typ" => {
                // Returned to the caller — gets stitched into the
                // root <slug>.typ so any user imports / setup run
                // before wrap_book.
                index_body = stripped;
                *done += 1;
                progress(*done, total, &PathBuf::from("(typst-chapter index.typ)"));
            }
            _ => {}
        }
    }
    Ok(index_body)
}

fn build_root_typ(book: &Node, typst_chapter_index_body: &str) -> String {
    let mut out = String::new();
    out.push_str("// Auto-generated by inkhaven Book assembly.\n");
    out.push_str(&format!("// Book: {}\n\n", book.title));
    out.push_str("#import \"globals.typ\": *\n");
    out.push_str("#import \"settings.typ\": *\n\n");
    let chapter_setup = typst_chapter_index_body.trim();
    if !chapter_setup.is_empty() {
        out.push_str("// User setup from Typst -> ");
        out.push_str(&book.title);
        out.push_str(" -> index.typ\n");
        out.push_str(chapter_setup);
        out.push_str("\n\n");
    }
    out.push_str("#wrap_book(include \"book/index.typ\")\n");
    out
}

/// Backslash-escape `\` and `"` so a title can safely sit inside a
/// Typst string literal. Newlines in titles are extremely unlikely
/// (the TUI rejects them) but we replace them with spaces defensively.
fn escape_typst_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' => out.push(' '),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_handles_quotes_and_backslashes() {
        assert_eq!(escape_typst_string("plain"), "plain");
        assert_eq!(escape_typst_string("a\"b"), "a\\\"b");
        assert_eq!(escape_typst_string("path\\sub"), "path\\\\sub");
        assert_eq!(escape_typst_string("line1\nline2"), "line1 line2");
    }

    #[test]
    fn strip_leading_heading_drops_title_and_blank() {
        let s = "= Chapter\n\nFirst line.\nSecond line.\n";
        assert_eq!(strip_leading_heading(s), "First line.\nSecond line.");
    }

    #[test]
    fn strip_leading_heading_keeps_body_without_heading() {
        let s = "First line.\nSecond line.\n";
        assert_eq!(strip_leading_heading(s), "First line.\nSecond line.");
    }
}
