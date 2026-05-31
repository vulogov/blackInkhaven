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

    // Wipe the entire `<artefacts>/<book-slug>/` directory and start
    // fresh. The user asked for a clean slate every time so stale
    // chapters, paragraphs, or PDFs from previous runs don't linger
    // and confuse a follow-up `typst compile`.
    if out_book.exists() {
        std::fs::remove_dir_all(&out_book).map_err(Error::Io)?;
    }
    std::fs::create_dir_all(&out_book_subtree).map_err(Error::Io)?;

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
        copy_typst_skeleton_files(store, cfg, layout, &hierarchy, book_node, &out_book, &artefacts_root, &mut done, total, progress)?;

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
            NodeKind::Paragraph | NodeKind::Image => count += 1,
            // Scripts never participate in Typst assembly — they
            // live alongside book content but aren't rendered.
            NodeKind::Script => {}
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
        // 1.2.6+: the Timeline chapter and the event paragraphs
        // inside it are metadata about the manuscript, not part
        // of the rendered prose. Skip both at the assembler so
        // nothing leaks into PDF / Markdown / TeX / EPUB exports.
        if child.kind == NodeKind::Chapter
            && child.system_tag.as_deref()
                == Some(crate::store::SYSTEM_TAG_BOOK_TIMELINE)
        {
            continue;
        }
        if child.kind == NodeKind::Paragraph && child.event.is_some() {
            continue;
        }
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
            NodeKind::Image => {
                let fname = child.fs_name(); // "NN-slug.<ext>"
                let dst = out_dir.join(&fname);
                copy_image_file(store, child, &dst)?;
                *done += 1;
                let rel = dst.strip_prefix(artefacts_root).unwrap_or(&dst);
                progress(*done, total, rel);
                child_refs.push(ChildRef::Image {
                    fname,
                    title: child.title.clone(),
                    caption: child.image_caption.clone(),
                    alt: child.image_alt.clone(),
                });
            }
            NodeKind::Book => {
                // Books can't be nested under other books in this
                // hierarchy; skip defensively.
            }
            NodeKind::Script => {
                // Scripts are executable Bund — they're not part
                // of the rendered manuscript.
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
/// the right include / wrap_paragraph / sub-include / wrap_image_*
/// line.
enum ChildRef {
    Paragraph { fname: String },
    Branch { dname: String },
    Image {
        fname: String,
        title: String,
        caption: Option<String>,
        alt: Option<String>,
    },
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

    match level {
        BranchLevel::BookRoot => {
            // `book/index.typ` is included from the root `<slug>.typ`
            // via `wrap_book(include "book/index.typ")`. We're at
            // file scope = markup mode, so every statement needs a
            // `#` prefix or it renders as literal text (which was the
            // original "{ include … }" bug — bare braces showed up
            // verbatim in the PDF).
            if children.is_empty() {
                out.push_str("// (empty book)\n");
            }
            for child in children {
                match child {
                    ChildRef::Paragraph { fname } => {
                        out.push_str(&format!(
                            "#wrap_paragraph(include \"{fname}\")\n"
                        ));
                    }
                    ChildRef::Branch { dname } => {
                        out.push_str(&format!(
                            "#include \"{dname}/index.typ\"\n"
                        ));
                    }
                    ChildRef::Image {
                        fname,
                        title,
                        caption,
                        alt,
                    } => {
                        // Image directly under a Book → frontispiece /
                        // book-art treatment via `wrap_image_book`.
                        out.push_str(&render_image_call(
                            "wrap_image_book",
                            fname,
                            title,
                            caption.as_deref(),
                            alt.as_deref(),
                            /*markup_prefix=*/ true,
                        ));
                    }
                }
            }
        }
        BranchLevel::Chapter | BranchLevel::Subchapter => {
            // Inside `wrap_*(title, { … })` we're in code mode in the
            // second argument — function names resolve directly, no
            // `#` prefix. Each statement evaluates to content; their
            // values join to form the wrapper's body argument.
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
                    ChildRef::Image {
                        fname,
                        title,
                        caption,
                        alt,
                    } => {
                        // Image under Chapter → `wrap_image_chapter`,
                        // under Subchapter → `wrap_image_subchapter`.
                        // Inside the code-mode `{ … }` argument so no
                        // `#` prefix.
                        // 1.2.15+ Phase S.5 — log +
                        // skip on BookRoot instead of
                        // `unreachable!()`.  The
                        // caller's filter excludes
                        // BookRoot, but a future
                        // refactor that loses that
                        // filter should produce a
                        // missing image, not a crash.
                        let wrap_fn = match level {
                            BranchLevel::Chapter => "wrap_image_chapter",
                            BranchLevel::Subchapter => "wrap_image_subchapter",
                            BranchLevel::BookRoot => {
                                tracing::warn!(
                                    target: "inkhaven::assemble",
                                    "image render reached BookRoot level — caller filter missed it; skipping",
                                );
                                continue;
                            }
                        };
                        body.push_str("  ");
                        body.push_str(&render_image_call(
                            wrap_fn,
                            fname,
                            title,
                            caption.as_deref(),
                            alt.as_deref(),
                            /*markup_prefix=*/ false,
                        ));
                    }
                }
            }
            if body.is_empty() {
                body.push_str("  []\n"); // empty content placeholder
            }
            let title = escape_typst_string(&branch.title);
            // 1.2.15+ Phase S.5 — log + early-return
            // on BookRoot instead of `unreachable!()`.
            // We're inside `match level { … Chapter |
            // Subchapter => { … } }` — no enclosing
            // loop — so the "skip" is a return with
            // whatever index we built so far.
            let wrap_fn = match level {
                BranchLevel::Chapter => "wrap_chapter",
                BranchLevel::Subchapter => "wrap_subchapter",
                BranchLevel::BookRoot => {
                    tracing::warn!(
                        target: "inkhaven::assemble",
                        "branch render reached BookRoot level — caller filter missed it; returning partial index",
                    );
                    return out;
                }
            };
            out.push_str(&format!("#{wrap_fn}(\"{title}\", {{\n"));
            out.push_str(&body);
            out.push_str("})\n");
        }
    }
    out
}

/// Format one `wrap_image_*` function call for inclusion in an
/// `index.typ`. `markup_prefix` adds the `#` so the call works at file
/// scope (markup mode); inside a code-mode `{ … }` block the prefix
/// is dropped. None values for caption / alt become Typst `none`.
fn render_image_call(
    wrap_fn: &str,
    fname: &str,
    title: &str,
    caption: Option<&str>,
    alt: Option<&str>,
    markup_prefix: bool,
) -> String {
    let title_lit = quote_or_none(Some(title));
    let caption_lit = quote_or_none(caption);
    let alt_lit = quote_or_none(alt);
    let prefix = if markup_prefix { "#" } else { "" };
    format!(
        "{prefix}{wrap_fn}(\"{}\", {title_lit}, {caption_lit}, alt: {alt_lit})\n",
        fname.replace('\\', "\\\\").replace('"', "\\\""),
    )
}

/// `"..."` for a Some, the bare keyword `none` for a None. Strings
/// get their `\` and `"` escaped.
fn quote_or_none(s: Option<&str>) -> String {
    match s.and_then(|t| if t.is_empty() { None } else { Some(t) }) {
        None => "none".to_string(),
        Some(t) => format!(
            "\"{}\"",
            t.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', " ")
        ),
    }
}

/// Pull an Image node's bytes out of bdslib (source of truth) and
/// write them to the assembled tree at `dst`. The on-disk copy under
/// `books/<...>` is the working copy; bdslib is authoritative so a
/// hand-edit there isn't accidentally re-ingested by the assembler.
fn copy_image_file(store: &Store, node: &Node, dst: &Path) -> Result<()> {
    let bytes = match store.image_bytes(node.id)? {
        Some(b) => b,
        None => {
            return Err(Error::Store(format!(
                "assemble: image `{}` has no bytes in bdslib",
                node.title
            )));
        }
    };
    std::fs::write(dst, &bytes).map_err(Error::Io)?;
    Ok(())
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
    cfg: &Config,
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
                // HJSON-driven header (#set page / #set text / #set par
                // synthesised from typst_page / typst_fonts /
                // typst_layout) followed by the user's free-form
                // paragraph content. Wiping the artefacts copy each
                // run is fine — bdslib holds the user's source.
                let mut composed = cfg.synthesised_settings_typ_header();
                if !stripped.trim().is_empty() {
                    composed.push('\n');
                    composed.push_str(&stripped);
                    if !composed.ends_with('\n') {
                        composed.push('\n');
                    }
                }
                let dst = out_book.join("settings.typ");
                std::fs::write(&dst, composed.as_bytes()).map_err(Error::Io)?;
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

    fn mk_node(kind: NodeKind, title: &str, slug: &str, order: u32) -> Node {
        Node {
            id: uuid::Uuid::nil(),
            kind,
            title: title.into(),
            slug: slug.into(),
            path: Vec::new(),
            parent_id: None,
            order,
            file: None,
            word_count: 0,
            modified_at: chrono::Utc::now(),
            protected: false,
            system_tag: None,
            image_ext: None,
            image_caption: None,
            image_alt: None,
            content_type: None,
            status: None,
            target_words: None,
            target_hit_at_status: None,
            linked_paragraphs: Vec::new(),
            bookmark: false,
            tags: Vec::new(),
            ai_memory: Vec::new(),
            event: None,
        }
    }

    #[test]
    fn book_root_index_emits_markup_mode_statements() {
        // Regression: bare `{ include … }` at file scope was rendered
        // as literal text in the PDF. The BookRoot index.typ must
        // emit `#`-prefixed top-level statements (markup-mode code
        // expressions), not a bare code block.
        let book = mk_node(NodeKind::Book, "Novel", "novel", 0);
        let children = vec![
            ChildRef::Branch { dname: "01-prologue".into() },
            ChildRef::Paragraph { fname: "02-stand-alone.typ".into() },
        ];
        let out = build_branch_index(&book, BranchLevel::BookRoot, &children, "../globals.typ");
        assert!(out.contains("#include \"01-prologue/index.typ\""), "got:\n{out}");
        assert!(out.contains("#wrap_paragraph(include \"02-stand-alone.typ\")"));
        // Crucially, NO bare `{` at column 0 — that's what previously
        // ended up as literal text in the rendered PDF.
        for line in out.lines() {
            assert!(
                !line.starts_with('{'),
                "BookRoot index must not open a bare code block: `{line}`\n--full--\n{out}"
            );
        }
    }

    #[test]
    fn chapter_index_wraps_with_function_call() {
        let chap = mk_node(NodeKind::Chapter, "Prologue", "prologue", 1);
        let children = vec![ChildRef::Paragraph {
            fname: "01-first.typ".into(),
        }];
        let out = build_branch_index(&chap, BranchLevel::Chapter, &children, "../../globals.typ");
        assert!(out.contains("#wrap_chapter(\"Prologue\""), "got:\n{out}");
        // Inside the code-block argument, `wrap_paragraph` is bare —
        // no `#` since we're already in code mode.
        assert!(out.contains("wrap_paragraph(include \"01-first.typ\")"));
    }

    #[test]
    fn render_image_call_omits_none_caption_alt() {
        let s = render_image_call(
            "wrap_image_chapter",
            "01-cover.png",
            "Cover Art",
            None,
            None,
            false,
        );
        // No `#` because we asked for code-mode form.
        assert!(s.starts_with("wrap_image_chapter("), "got: {s}");
        assert!(s.contains("\"Cover Art\""));
        assert!(s.contains(", none"), "expected `none` for caption: {s}");
        assert!(s.contains("alt: none"), "expected `alt: none`: {s}");
    }

    #[test]
    fn render_image_call_markup_prefix_for_book_root() {
        let s = render_image_call(
            "wrap_image_book",
            "01-frontispiece.png",
            "Frontispiece",
            Some("Lighthouse at dawn"),
            Some("alt text"),
            true,
        );
        assert!(s.starts_with("#wrap_image_book("), "got: {s}");
        assert!(s.contains("\"01-frontispiece.png\""));
        assert!(s.contains("\"Lighthouse at dawn\""));
        assert!(s.contains("alt: \"alt text\""));
    }

    #[test]
    fn build_book_root_emits_wrap_image_book() {
        let book = mk_node(NodeKind::Book, "Novel", "novel", 0);
        let children = vec![ChildRef::Image {
            fname: "01-cover.png".into(),
            title: "Cover".into(),
            caption: Some("By Vladimir".into()),
            alt: None,
        }];
        let out = build_branch_index(
            &book,
            BranchLevel::BookRoot,
            &children,
            "../globals.typ",
        );
        assert!(out.contains("#wrap_image_book(\"01-cover.png\""), "got:\n{out}");
        assert!(out.contains("\"By Vladimir\""));
    }

    #[test]
    fn build_chapter_emits_wrap_image_chapter_in_code_mode() {
        let chap = mk_node(NodeKind::Chapter, "Prologue", "prologue", 1);
        let children = vec![ChildRef::Image {
            fname: "01-opener.jpg".into(),
            title: "Opener".into(),
            caption: None,
            alt: None,
        }];
        let out = build_branch_index(
            &chap,
            BranchLevel::Chapter,
            &children,
            "../../globals.typ",
        );
        // Wrapped in #wrap_chapter("Prologue", { ... }), inner call
        // is code-mode so NO `#` prefix.
        assert!(out.contains("#wrap_chapter(\"Prologue\""));
        assert!(
            out.contains("  wrap_image_chapter(\"01-opener.jpg\""),
            "got:\n{out}"
        );
    }

    #[test]
    fn build_subchapter_uses_wrap_image_subchapter() {
        let sub = mk_node(NodeKind::Subchapter, "Vista", "vista", 1);
        let children = vec![ChildRef::Image {
            fname: "01-vista.webp".into(),
            title: "Vista".into(),
            caption: None,
            alt: None,
        }];
        let out = build_branch_index(
            &sub,
            BranchLevel::Subchapter,
            &children,
            "../../../globals.typ",
        );
        assert!(out.contains("#wrap_subchapter(\"Vista\""));
        assert!(out.contains("  wrap_image_subchapter(\"01-vista.webp\""));
    }

    #[test]
    fn empty_chapter_emits_placeholder_content() {
        let chap = mk_node(NodeKind::Chapter, "Empty", "empty", 1);
        let out = build_branch_index(&chap, BranchLevel::Chapter, &[], "../../globals.typ");
        assert!(out.contains("#wrap_chapter(\"Empty\""));
        // Empty branch must not produce a parse-failing `wrap_chapter("Empty", {})`.
        assert!(out.contains("[]"), "got:\n{out}");
    }
}
