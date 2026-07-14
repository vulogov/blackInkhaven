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
    /// Number of citation entries written to `sources.bib` (0 → no .bib file
    /// and no `#bibliography(...)` line in the root .typ). SOURCES-1.
    pub bibliography_entries: usize,
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

    // STRUCT-1: register every `.jinja` paragraph in the Snippets book as a named
    // template *before* any rendering, so manuscript and snippet Jinja templates
    // can `{% include "snippets/…" %}` each other. Empty when no Jinja snippets.
    let jinja_env = build_jinja_environment(layout, &hierarchy)?;

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
        cfg,
        &jinja_env,
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

    // SOURCES-1: collect citation entries from the Sources system book into
    // `<out_book>/sources.bib`. Scope honours `cfg.sources.all`.
    let bibliography_entries =
        collect_and_emit_sources(store, layout, cfg, &hierarchy, book_node, &out_book)?;

    // LOCI: an Index Locorum of every `@key[locus]` cited in this book, when
    // enabled. Written as `index_locorum.typ`, `#include`d after the bibliography.
    let index_locorum_sources = if cfg.sources.index_locorum {
        emit_index_locorum(layout, cfg, &hierarchy, book_node, &out_book)?
    } else {
        0
    };

    // LEXICON: an Index Verborum of the scholarly-lexicon terms used in this book,
    // when enabled. Written as `index_verborum.typ`, `#include`d after the loci.
    let index_verborum_terms = if cfg.sources.index_verborum {
        emit_index_verborum(store, layout, cfg, &hierarchy, book_node, &out_book)?
    } else {
        0
    };

    // REUSE-1: copy reusable snippets to `<out_book>/snippets/` so prose
    // `#include "…/snippets/…"` calls resolve. No-op when no Snippets book.
    let _snippets_written =
        emit_snippets_directory(layout, &hierarchy, &out_book, cfg, &jinja_env)?;

    // Root .typ for the book — applies settings, calls wrap_book on
    // the assembled subtree. The Typst chapter's index.typ body is
    // appended so any user setup (image search paths, imports of
    // additional helpers) flows through.
    let root_typ = out_book.join(format!("{}.typ", book_node.slug));
    // Emit `#bibliography(...)` only when we actually wrote entries AND the
    // user hasn't disabled the auto-line (they may prefer to place it by hand
    // inside their Typst setup).
    let bib_style = (bibliography_entries > 0 && cfg.sources.auto_bibliography)
        .then(|| cfg.sources.bibliography_style.as_str());
    // TUI book assembly is never a blind submission (that's an export-time
    // concern), so identity is always shown here.
    let front_matter = cfg
        .frontmatter
        .to_typst_block(&cfg.language, &book_node.title, false);
    let root_body = build_root_typ(
        book_node,
        &typst_root_index_body,
        bib_style,
        &front_matter,
        index_locorum_sources > 0,
        index_verborum_terms > 0,
    );
    std::fs::write(&root_typ, root_body.as_bytes()).map_err(Error::Io)?;
    done += 1;
    progress(done, total, &PathBuf::from(format!("{}.typ", book_node.slug)));

    Ok(AssemblyReport {
        files_written: done,
        root_typ,
        bibliography_entries,
    })
}

/// Collect citation entries from the **Sources** system book and write them to
/// `<out_book>/sources.bib`. Scope is set by `cfg.sources.all`: `true` →
/// every entry under Sources; `false` → only the chapter whose title matches
/// the assembled book (graceful — an absent/empty chapter just yields zero).
/// Returns the count of valid entries written; `0` writes no file.
fn collect_and_emit_sources(
    store: &Store,
    layout: &ProjectLayout,
    cfg: &Config,
    hierarchy: &Hierarchy,
    book_node: &Node,
    out_book: &Path,
) -> Result<usize> {
    let _ = store; // bodies are read from disk via layout.root, like paragraphs
    let Some(sources_book) = hierarchy.iter().find(|n| {
        n.kind == NodeKind::Book
            && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_SOURCES)
    }) else {
        return Ok(0);
    };

    let mut entries: Vec<crate::sources::BibEntry> = Vec::new();
    for id in hierarchy.collect_subtree(sources_book.id) {
        let Some(n) = hierarchy.get(id) else { continue };
        if n.kind != NodeKind::Paragraph {
            continue;
        }
        // When not collecting everything, keep only paragraphs whose
        // top-of-Sources chapter is named after this book.
        if !cfg.sources.all {
            let chapter_title = {
                let mut cur: Option<&Node> = Some(n);
                let mut found: Option<&str> = None;
                while let Some(node) = cur {
                    if node.parent_id == Some(sources_book.id) {
                        found = Some(node.title.as_str());
                        break;
                    }
                    cur = node.parent_id.and_then(|pid| hierarchy.get(pid));
                }
                found
            };
            if chapter_title != Some(book_node.title.as_str()) {
                continue;
            }
        }
        let Some(rel) = &n.file else { continue };
        let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) else {
            continue;
        };
        // Defensive: a hand-created paragraph may carry the `= Title` editor
        // heading; strip it before parsing HJSON.
        let body = strip_leading_heading(&raw);
        if let Some(e) = crate::sources::BibEntry::from_hjson(&body) {
            if e.is_valid() {
                entries.push(e);
            }
        }
    }

    let (text, count) = crate::sources::compile_bibtex(&entries);
    if count == 0 {
        return Ok(0);
    }
    std::fs::write(out_book.join("sources.bib"), text.as_bytes()).map_err(Error::Io)?;
    Ok(count)
}

/// LOCI — harvest every `@key[locus]` cited in this book, resolve each key's title
/// from the Sources book, and write `<out_book>/index_locorum.typ` (a `= Index
/// Locorum` chapter). Returns the number of sources indexed (0 → nothing written,
/// so the root `#include` is skipped). Gated by `cfg.sources.index_locorum`.
fn emit_index_locorum(
    layout: &ProjectLayout,
    cfg: &Config,
    hierarchy: &Hierarchy,
    book_node: &Node,
    out_book: &Path,
) -> Result<usize> {
    use crate::index_locorum::LocusCitation;
    // Harvest citations from this book's paragraphs (raw prose, so `[…]` survives).
    let mut cites: Vec<LocusCitation> = Vec::new();
    for chapter in hierarchy
        .children_of(Some(book_node.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter)
    {
        for id in hierarchy.collect_subtree(chapter.id) {
            let Some(n) = hierarchy.get(id) else { continue };
            if n.kind != NodeKind::Paragraph {
                continue;
            }
            let Some(rel) = &n.file else { continue };
            let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) else { continue };
            for (key, locus) in crate::sources::extract_cite_loci(&raw) {
                cites.push(LocusCitation { key, locus, chapter: chapter.title.clone() });
            }
        }
    }

    // Resolve key → source title and key → declared reference scheme from the
    // Sources book (all valid entries).
    let mut titles = std::collections::HashMap::new();
    let mut declared = std::collections::HashMap::new();
    if let Some(sources_book) = hierarchy.iter().find(|n| {
        n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_SOURCES)
    }) {
        for id in hierarchy.collect_subtree(sources_book.id) {
            let Some(n) = hierarchy.get(id) else { continue };
            if n.kind != NodeKind::Paragraph {
                continue;
            }
            let Some(rel) = &n.file else { continue };
            let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) else { continue };
            if let Some(e) = crate::sources::BibEntry::from_hjson(&strip_leading_heading(&raw)) {
                if e.is_valid() && !e.title.trim().is_empty() {
                    titles.insert(e.key.clone(), e.title.clone());
                }
                if let Some(scheme) = e.scheme.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    declared.insert(e.key.clone(), scheme.to_string());
                }
            }
        }
    }

    // Validate loci against their sources' reference schemes; a malformed locus is
    // a warning at build time (it still renders — nothing is silently dropped).
    let keys: Vec<String> = {
        let mut ks: Vec<String> = cites.iter().map(|c| c.key.clone()).collect();
        ks.sort();
        ks.dedup();
        ks
    };
    let (schemes, _errs) =
        crate::index_locorum::resolve_schemes(&cfg.sources.ref_schemes, &declared, &keys);
    let entries = crate::index_locorum::build(&cites, &titles, &schemes);
    if entries.is_empty() {
        return Ok(0);
    }
    for m in crate::index_locorum::malformed(&entries, &schemes) {
        tracing::warn!(
            "index locorum: malformed locus @{}[{}] — expected {}",
            m.key,
            m.locus,
            m.expected
        );
    }
    let heading = crate::index_locorum::heading_for_language(&cfg.language);
    let body = crate::index_locorum::render_typst(&entries, heading);
    std::fs::write(out_book.join("index_locorum.typ"), body.as_bytes()).map_err(Error::Io)?;
    Ok(entries.len())
}

/// LEXICON — harvest the scholarly-lexicon terms used in this book, and write
/// `<out_book>/index_verborum.typ` (a `= Index Verborum` chapter: each term's
/// original-language form, senses, and the chapters that use it). Returns the
/// number of terms indexed (0 → nothing written). Gated by `cfg.sources.index_verborum`.
fn emit_index_verborum(
    store: &Store,
    layout: &ProjectLayout,
    cfg: &Config,
    hierarchy: &Hierarchy,
    book_node: &Node,
    out_book: &Path,
) -> Result<usize> {
    use crate::index_verborum::{LexTerm, SenseRow, TermUsage};
    let mut lex = crate::glossary::glossary_entries_from_store(store, hierarchy, Some(&book_node.slug));
    lex.retain(|e| e.is_valid() && e.is_lexicon_term());
    if lex.is_empty() {
        return Ok(0);
    }
    let lexicon: Vec<LexTerm> = lex
        .iter()
        .map(|e| LexTerm {
            term: e.term.trim().to_string(),
            original_forms: e
                .original_forms
                .iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            senses: e
                .senses
                .iter()
                .map(|s| SenseRow { label: s.label.trim().to_string(), gloss: s.gloss.trim().to_string() })
                .collect(),
        })
        .collect();

    // Harvest usages: per chapter, aggregate its prose and match each term's forms.
    let forms: Vec<(String, Vec<String>)> =
        lex.iter().map(|e| (e.term.trim().to_string(), e.surface_forms())).collect();
    let mut usages: Vec<TermUsage> = Vec::new();
    for chapter in hierarchy.children_of(Some(book_node.id)).into_iter().filter(|n| n.kind == NodeKind::Chapter) {
        let mut text = String::new();
        for id in hierarchy.collect_subtree(chapter.id) {
            let Some(n) = hierarchy.get(id) else { continue };
            if n.kind != NodeKind::Paragraph || n.content_type.as_deref() == Some("jinja") {
                continue;
            }
            let Some(rel) = &n.file else { continue };
            if let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) {
                text.push_str(&crate::audiobook::typst_to_plain(&raw));
                text.push('\n');
            }
        }
        let lc = text.to_lowercase();
        for (term, surface) in &forms {
            if surface.iter().any(|f| crate::world::fact_check_lang::contains_word(&lc, f)) {
                usages.push(TermUsage { term: term.clone(), chapter: chapter.title.clone() });
            }
        }
    }

    let idx = crate::index_verborum::build(&lexicon, &usages);
    if idx.is_empty() {
        return Ok(0);
    }
    let heading = crate::index_verborum::heading_for_language(&cfg.language);
    let body = crate::index_verborum::render_typst(&idx, heading);
    std::fs::write(out_book.join("index_verborum.typ"), body.as_bytes()).map_err(Error::Io)?;
    Ok(idx.len())
}

/// REUSE-1: copy every paragraph in the **Snippets** system book to
/// `<out_book>/snippets/<slug>.typ`, so a `#include "…/snippets/<slug>.typ"`
/// anywhere in the prose resolves at `typst compile`. Mirrors
/// `collect_and_emit_sources`: read bodies from disk, strip the editor heading,
/// write verbatim. Returns the count written; `0` (absent/empty book) writes no
/// directory — a complete fast-path for projects not using snippets.
fn emit_snippets_directory(
    layout: &ProjectLayout,
    hierarchy: &Hierarchy,
    out_book: &Path,
    cfg: &Config,
    jinja_env: &minijinja::Environment<'static>,
) -> Result<usize> {
    let Some(snippets_book) = hierarchy.iter().find(|n| {
        n.kind == NodeKind::Book
            && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_SNIPPETS)
    }) else {
        return Ok(0);
    };
    // Collect the snippet paragraph nodes first so we only create the dir when
    // there's content.
    let nodes: Vec<&Node> = hierarchy
        .collect_subtree(snippets_book.id)
        .into_iter()
        .filter_map(|id| hierarchy.get(id))
        .filter(|n| n.kind == NodeKind::Paragraph && n.file.is_some())
        .collect();
    if nodes.is_empty() {
        return Ok(0);
    }
    let dir = out_book.join("snippets");
    std::fs::create_dir_all(&dir).map_err(Error::Io)?;
    let mut count = 0usize;
    for n in nodes {
        // The output is always `<slug>.typ` so prose `#include "…/snippets/<slug>.typ"`
        // resolves regardless of whether the source was `.typ` or `.jinja`.
        let dst = dir.join(format!("{}.typ", n.slug));
        if n.content_type.as_deref() == Some("jinja") {
            // STRUCT-1: render the snippet template standalone (its own minimal
            // context, no linked data) so REUSE-1 Typst-level includes resolve.
            // Jinja-level `{% include %}` uses the registered template instead.
            render_jinja_paragraph(layout, hierarchy, cfg, jinja_env, n, &dst)?;
        } else {
            let Some(rel) = &n.file else { continue };
            let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) else {
                continue;
            };
            // Strip the `= Title` editor heading — the snippet is included as raw
            // Typst, the title is inkhaven chrome (same as `copy_paragraph_file`).
            let body = strip_leading_heading(&raw);
            std::fs::write(&dst, body.as_bytes()).map_err(Error::Io)?;
        }
        count += 1;
    }
    Ok(count)
}

/// STRUCT-1 — build the `minijinja` environment from the Snippets system book.
/// Every `content_type: "jinja"` paragraph in the Snippets subtree is registered
/// as a named template so manuscript Jinja paragraphs (and other snippet
/// templates) can `{% include "snippets/<path>.jinja" %}` them. Template names
/// come from the hierarchy slug path, lowercased — `Snippets/Macros/warning` →
/// `snippets/macros/warning.jinja`. Returns an empty environment when the
/// Snippets book is absent or has no Jinja paragraphs (standalone rendering still
/// works). Q2: duplicate template names are first-write-wins with a warning.
fn build_jinja_environment(
    layout: &ProjectLayout,
    hierarchy: &Hierarchy,
) -> Result<minijinja::Environment<'static>> {
    let mut env = minijinja::Environment::new();
    let Some(snippets_book) = hierarchy.iter().find(|n| {
        n.kind == NodeKind::Book
            && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_SNIPPETS)
    }) else {
        return Ok(env);
    };
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for id in hierarchy.collect_subtree(snippets_book.id) {
        let Some(n) = hierarchy.get(id) else { continue };
        if n.kind != NodeKind::Paragraph || n.content_type.as_deref() != Some("jinja") {
            continue;
        }
        let Some(rel) = &n.file else { continue };
        let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) else {
            continue;
        };
        let body = strip_leading_heading(&raw);
        let name = jinja_template_name(hierarchy, n);
        if !seen.insert(name.clone()) {
            tracing::warn!(
                target: "inkhaven::assemble",
                "jinja: duplicate template name `{name}` — keeping first, ignoring `{}`",
                n.title
            );
            continue; // Q2 — first-write-wins.
        }
        env.add_template_owned(name.clone(), body).map_err(|e| {
            Error::Store(format!(
                "jinja: failed to register snippet template `{name}` ({}): {e}",
                n.title
            ))
        })?;
    }
    Ok(env)
}

/// The `minijinja` template name for a Snippets-book paragraph: its hierarchy
/// slug path, lowercased, with a `.jinja` suffix — e.g.
/// `snippets/macros/warning.jinja`. The leading `snippets/` comes from the book
/// slug, so `{% include %}` paths match the registered names exactly.
fn jinja_template_name(hierarchy: &Hierarchy, node: &Node) -> String {
    format!("{}.jinja", hierarchy.slug_path(node).to_lowercase())
}

/// STRUCT-1 — build the render context for a Jinja paragraph. Exposes the
/// paragraph's own `title`/`slug`, its enclosing `book` + `chapter`, the project
/// `language`/`genre`, and a `linked` map of HJSON data keyed by each linked
/// paragraph's slug. Non-HJSON links are skipped — their prose isn't meaningful
/// template context (raw-text access is deferred to STRUCT-2's `linked_text`).
fn jinja_context_for_node(
    layout: &ProjectLayout,
    hierarchy: &Hierarchy,
    cfg: &Config,
    node: &Node,
) -> minijinja::Value {
    let ancestors = hierarchy.ancestors(node); // root-first, excludes `node`
    let book = ancestors.iter().find(|n| n.kind == NodeKind::Book);
    let chapter = ancestors.iter().find(|n| n.kind == NodeKind::Chapter);

    let mut linked = serde_json::Map::new();
    for lid in &node.linked_paragraphs {
        let Some(ln) = hierarchy.get(*lid) else { continue };
        if ln.content_type.as_deref() != Some("hjson") {
            continue;
        }
        let Some(rel) = &ln.file else { continue };
        let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) else {
            continue;
        };
        match serde_hjson::from_str::<serde_json::Value>(&raw) {
            Ok(v) => {
                linked.insert(ln.slug.clone(), v);
            }
            Err(e) => {
                tracing::warn!(
                    target: "inkhaven::assemble",
                    "jinja: linked HJSON `{}` did not parse — skipped: {e}",
                    ln.title
                );
            }
        }
    }

    let ctx = serde_json::json!({
        "title": node.title,
        "slug": node.slug,
        "book": book.map(|b| serde_json::json!({
            "title": b.title,
            "slug": b.slug,
            "genre": cfg.genre,
        })),
        "chapter": chapter.map(|c| serde_json::json!({
            "title": c.title,
            "slug": c.slug,
        })),
        "linked": serde_json::Value::Object(linked),
        "language": cfg.language,
        "genre": cfg.genre,
    });
    minijinja::Value::from_serialize(&ctx)
}

/// STRUCT-1 — render a `content_type: "jinja"` paragraph to Typst at `dst`
/// (always a `.typ` path). Strips the editor heading, builds the context, and
/// renders against `env` so `{% include "snippets/…" %}` resolves to the
/// pre-registered snippet templates. On render failure: abort assembly by
/// default (Q1), or — when `cfg.jinja.continue_on_error` — write a visible Typst
/// error block and continue so the author can fix templates one at a time.
fn render_jinja_paragraph(
    layout: &ProjectLayout,
    hierarchy: &Hierarchy,
    cfg: &Config,
    env: &minijinja::Environment<'static>,
    node: &Node,
    dst: &Path,
) -> Result<()> {
    let Some(rel) = &node.file else {
        return Err(Error::Store(format!(
            "assemble: jinja paragraph `{}` has no file on disk",
            node.title
        )));
    };
    let raw = std::fs::read_to_string(layout.root.join(rel)).map_err(Error::Io)?;
    let body = strip_leading_heading(&raw);
    let ctx = jinja_context_for_node(layout, hierarchy, cfg, node);
    match env.render_str(&body, ctx) {
        Ok(rendered) => {
            std::fs::write(dst, rendered.as_bytes()).map_err(Error::Io)?;
            Ok(())
        }
        Err(e) if cfg.jinja.continue_on_error => {
            // Visible in the PDF so the failure can't be silently swallowed.
            let block = format!(
                "// JINJA RENDER ERROR in {title}: {e}\n\
                 #block(fill: rgb(\"#ffdddd\"), inset: 8pt, radius: 4pt, width: 100%)[\
                 *JINJA RENDER ERROR* — {title_lit}: {err_lit}]\n",
                title = node.title,
                title_lit = escape_typst_string(&node.title),
                err_lit = escape_typst_string(&e.to_string()),
            );
            std::fs::write(dst, block.as_bytes()).map_err(Error::Io)?;
            tracing::warn!(
                target: "inkhaven::assemble",
                "jinja render error in `{}` (continuing): {e}",
                node.title
            );
            Ok(())
        }
        Err(e) => Err(Error::Store(format!(
            "jinja render failed in `{}`: {e}",
            node.title
        ))),
    }
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
    cfg: &Config,
    jinja_env: &minijinja::Environment<'static>,
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
                if child.content_type.as_deref() == Some("jinja") {
                    // STRUCT-1: render Jinja → Typst. The artefact is always
                    // `.typ` — a `.jinja` filename in the `include` would break
                    // `typst compile`, so the `ChildRef` carries the `.typ` name.
                    let out_fname = format!("{:02}-{}.typ", child.order, child.slug);
                    let dst = out_dir.join(&out_fname);
                    render_jinja_paragraph(layout, hierarchy, cfg, jinja_env, child, &dst)?;
                    *done += 1;
                    let rel = dst.strip_prefix(artefacts_root).unwrap_or(&dst);
                    progress(*done, total, rel);
                    child_refs.push(ChildRef::Paragraph { fname: out_fname });
                } else {
                    let fname = child.fs_name(); // "NN-slug.typ"
                    let dst = out_dir.join(&fname);
                    copy_paragraph_file(layout, child, &dst)?;
                    *done += 1;
                    let rel = dst.strip_prefix(artefacts_root).unwrap_or(&dst);
                    progress(*done, total, rel);
                    child_refs.push(ChildRef::Paragraph { fname });
                }
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
                    cfg,
                    jinja_env,
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

    // 1.3.0 PDF-1 — additive outline marker: a zero-size `#metadata`
    // element carrying this branch's tree node id, so the PDF outline
    // injector can correlate a chapter/subchapter to its first page.
    // `#metadata` is query-only (invisible to layout, absent from the
    // PDF itself), and carries no label so branches can't collide.
    out.push_str(&format!("#metadata((node_id: \"{}\"))\n", branch.id));

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

fn build_root_typ(
    book: &Node,
    typst_chapter_index_body: &str,
    bibliography_style: Option<&str>,
    front_matter: &str,
    index_locorum: bool,
    index_verborum: bool,
) -> String {
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
    // PAPER (1.6.15+): the front-matter title block, if configured. Empty for
    // books that don't opt in, so their assembled root is unchanged.
    if !front_matter.is_empty() {
        out.push_str(front_matter);
        out.push('\n');
    }
    out.push_str("#wrap_book(include \"book/index.typ\")\n");
    // SOURCES-1: render the bibliography from the assembled sources.bib. Typst
    // resolves @key cite tokens in the prose against this file.
    if let Some(style) = bibliography_style {
        out.push_str(&format!(
            "\n#bibliography(\"sources.bib\", style: \"{}\")\n",
            escape_typst_string(style)
        ));
    }
    // LOCI: the Index Locorum chapter, after the bibliography.
    if index_locorum {
        out.push_str("\n#include \"index_locorum.typ\"\n");
    }
    // LEXICON: the Index Verborum chapter, after the Index Locorum.
    if index_verborum {
        out.push_str("\n#include \"index_verborum.typ\"\n");
    }
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
    fn emit_snippets_writes_sidecar_and_strips_heading() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let book_id = uuid::Uuid::new_v4();
        // A Snippets book + one paragraph whose source file lives under root.
        let book = Node {
            id: book_id,
            system_tag: Some("snippets".into()),
            ..mk_node(NodeKind::Book, "Snippets", "snippets", 0)
        };
        let rel = "books/snippets/01-warn.typ".to_string();
        std::fs::create_dir_all(root.join("books/snippets")).unwrap();
        std::fs::write(root.join(&rel), "= warn\n\n#block[Careful here.]\n").unwrap();
        let para = Node {
            id: uuid::Uuid::new_v4(),
            parent_id: Some(book_id),
            file: Some(rel),
            ..mk_node(NodeKind::Paragraph, "warn", "warn", 0)
        };
        let h = Hierarchy::from_nodes_for_test(vec![book, para]);
        let out = root.join("out");
        let n = emit_snippets_directory(
            &ProjectLayout::new(root),
            &h,
            &out,
            &Config::default(),
            &minijinja::Environment::new(),
        )
        .unwrap();
        assert_eq!(n, 1);
        let written = std::fs::read_to_string(out.join("snippets/warn.typ")).unwrap();
        assert!(written.contains("#block[Careful here.]"), "{written}");
        assert!(!written.contains("= warn"), "heading must be stripped: {written}");
    }

    #[test]
    fn emit_snippets_absent_or_empty_book_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("out");
        // No Snippets book at all.
        let h = Hierarchy::from_nodes_for_test(vec![]);
        assert_eq!(
            emit_snippets_directory(
                &ProjectLayout::new(tmp.path()),
                &h,
                &out,
                &Config::default(),
                &minijinja::Environment::new(),
            )
            .unwrap(),
            0
        );
        assert!(!out.join("snippets").exists(), "no dir when no snippets");
        // An empty Snippets book (no paragraphs) is also a no-op.
        let book = Node {
            id: uuid::Uuid::new_v4(),
            system_tag: Some("snippets".into()),
            ..mk_node(NodeKind::Book, "Snippets", "snippets", 0)
        };
        let h2 = Hierarchy::from_nodes_for_test(vec![book]);
        assert_eq!(
            emit_snippets_directory(
                &ProjectLayout::new(tmp.path()),
                &h2,
                &out,
                &Config::default(),
                &minijinja::Environment::new(),
            )
            .unwrap(),
            0
        );
        assert!(!out.join("snippets").exists());
    }

    #[test]
    fn jinja_renders_includes_linked_and_metadata() {
        // A Snippets book with one `.jinja` macro, plus a user book whose chapter
        // holds a `.jinja` manuscript paragraph that includes the macro and reads
        // a linked HJSON paragraph's fields.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let l = ProjectLayout::new(root);
        std::fs::create_dir_all(root.join("books/snippets")).unwrap();
        std::fs::create_dir_all(root.join("books/mybook/intro")).unwrap();

        // Snippets/warning.jinja → template name "snippets/warning.jinja".
        let sb_id = uuid::Uuid::new_v4();
        let sb = Node {
            id: sb_id,
            system_tag: Some("snippets".into()),
            ..mk_node(NodeKind::Book, "Snippets", "snippets", 0)
        };
        let warn_rel = "books/snippets/01-warning.jinja".to_string();
        std::fs::write(root.join(&warn_rel), "= warning\n#block[Heads up.]\n").unwrap();
        let warn = Node {
            id: uuid::Uuid::new_v4(),
            parent_id: Some(sb_id),
            file: Some(warn_rel),
            content_type: Some("jinja".into()),
            ..mk_node(NodeKind::Paragraph, "warning", "warning", 0)
        };

        // User book → chapter → linked HJSON + manuscript jinja paragraph.
        let ub_id = uuid::Uuid::new_v4();
        let ub = mk_node(NodeKind::Book, "My Book", "mybook", 0);
        let ub = Node { id: ub_id, ..ub };
        let ch_id = uuid::Uuid::new_v4();
        let ch = Node {
            id: ch_id,
            parent_id: Some(ub_id),
            ..mk_node(NodeKind::Chapter, "Intro", "intro", 0)
        };
        let aria_rel = "books/mybook/intro/01-aria.hjson".to_string();
        std::fs::write(root.join(&aria_rel), "{ name: \"Aria\", species: \"fox\" }").unwrap();
        let aria_id = uuid::Uuid::new_v4();
        let aria = Node {
            id: aria_id,
            parent_id: Some(ch_id),
            file: Some(aria_rel),
            content_type: Some("hjson".into()),
            ..mk_node(NodeKind::Paragraph, "aria", "aria", 0)
        };
        let side_rel = "books/mybook/intro/02-sidebar.jinja".to_string();
        std::fs::write(
            root.join(&side_rel),
            "= Sidebar\n{% include \"snippets/warning.jinja\" %}\nName: {{ linked[\"aria\"].name }} ({{ linked[\"aria\"].species }})\nBook: {{ book.title }}\nLang: {{ language }}\n",
        )
        .unwrap();
        let side = Node {
            id: uuid::Uuid::new_v4(),
            parent_id: Some(ch_id),
            file: Some(side_rel),
            content_type: Some("jinja".into()),
            linked_paragraphs: vec![aria_id],
            ..mk_node(NodeKind::Paragraph, "sidebar", "sidebar", 1)
        };

        let side_id = side.id;
        let h = Hierarchy::from_nodes_for_test(vec![sb, warn, ub, ch, aria, side]);
        let env = build_jinja_environment(&l, &h).unwrap();
        let cfg = Config::default();

        let out = root.join("02-sidebar.typ");
        let side_node = h.get(side_id).unwrap();
        render_jinja_paragraph(&l, &h, &cfg, &env, side_node, &out).unwrap();
        let r = std::fs::read_to_string(&out).unwrap();
        assert!(r.contains("#block[Heads up.]"), "include not resolved: {r}");
        assert!(r.contains("Name: Aria (fox)"), "linked HJSON not injected: {r}");
        assert!(r.contains("Book: My Book"), "book metadata missing: {r}");
        assert!(r.contains("Lang: english"), "language missing: {r}");
        assert!(!r.contains("= Sidebar"), "heading must be stripped: {r}");
    }

    #[test]
    fn jinja_render_error_aborts_by_default_and_continues_when_opted_in() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let l = ProjectLayout::new(root);
        std::fs::create_dir_all(root.join("books/mybook")).unwrap();
        let bad_rel = "books/mybook/01-bad.jinja".to_string();
        // Unterminated expression → minijinja syntax error.
        std::fs::write(root.join(&bad_rel), "= bad\n{{ oops").unwrap();
        let node = Node {
            id: uuid::Uuid::new_v4(),
            file: Some(bad_rel),
            content_type: Some("jinja".into()),
            ..mk_node(NodeKind::Paragraph, "bad", "bad", 0)
        };
        let h = Hierarchy::from_nodes_for_test(vec![node.clone()]);
        let env = minijinja::Environment::new();
        let out = root.join("01-bad.typ");

        // Default: abort.
        let cfg = Config::default();
        assert!(cfg.jinja.continue_on_error == false);
        let err = render_jinja_paragraph(&l, &h, &cfg, &env, &node, &out).unwrap_err();
        assert!(err.to_string().contains("jinja render failed"), "{err}");

        // Opt-in: continue, writing a visible error block.
        let mut cfg2 = Config::default();
        cfg2.jinja.continue_on_error = true;
        render_jinja_paragraph(&l, &h, &cfg2, &env, &node, &out).unwrap();
        let r = std::fs::read_to_string(&out).unwrap();
        assert!(r.contains("JINJA RENDER ERROR"), "{r}");
    }

    #[test]
    fn jinja_passes_through_non_ascii_linked_values() {
        // Multilingual requirement — "does it work in Russian?". A linked HJSON
        // entry with Cyrillic values must render through the pipeline verbatim
        // (serde_hjson -> serde_json -> minijinja -> UTF-8 on disk).
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let l = ProjectLayout::new(root);
        std::fs::create_dir_all(root.join("books/kniga")).unwrap();
        // English schema keys (the realistic case), Cyrillic values — plus one
        // Cyrillic key reached by subscript (`.attr` access is ASCII-only in Jinja).
        let geroy_rel = "books/kniga/01-geroy.hjson".to_string();
        std::fs::write(
            root.join(&geroy_rel),
            "{ name: \"Ария\", species: \"лиса\", роль: \"разведчик\" }",
        )
        .unwrap();
        let geroy_id = uuid::Uuid::new_v4();
        let geroy = Node {
            id: geroy_id,
            file: Some(geroy_rel),
            content_type: Some("hjson".into()),
            ..mk_node(NodeKind::Paragraph, "geroy", "geroy", 0)
        };
        let card_rel = "books/kniga/02-card.jinja".to_string();
        std::fs::write(
            root.join(&card_rel),
            "Имя: {{ linked[\"geroy\"].name }} ({{ linked[\"geroy\"].species }}, {{ linked[\"geroy\"][\"роль\"] }})\n",
        )
        .unwrap();
        let card_id = uuid::Uuid::new_v4();
        let card = Node {
            id: card_id,
            file: Some(card_rel),
            content_type: Some("jinja".into()),
            linked_paragraphs: vec![geroy_id],
            ..mk_node(NodeKind::Paragraph, "card", "card", 1)
        };
        let h = Hierarchy::from_nodes_for_test(vec![geroy, card]);
        let env = minijinja::Environment::new();
        let cfg = Config::default();
        let out = root.join("02-card.typ");
        let card_node = h.get(card_id).unwrap();
        render_jinja_paragraph(&l, &h, &cfg, &env, card_node, &out).unwrap();
        let r = std::fs::read_to_string(&out).unwrap();
        assert!(r.contains("Имя: Ария (лиса, разведчик)"), "cyrillic mangled: {r}");
    }

    #[test]
    fn build_root_typ_bibliography_line_is_style_gated() {
        let book = mk_node(NodeKind::Book, "My Book", "my-book", 0);
        let with = build_root_typ(&book, "", Some("ieee"), "", false, false);
        assert!(
            with.contains("#bibliography(\"sources.bib\", style: \"ieee\")"),
            "{with}"
        );
        // The bibliography sits after wrap_book.
        let wrap = with.find("#wrap_book").unwrap();
        let bib = with.find("#bibliography").unwrap();
        assert!(bib > wrap, "bibliography must follow wrap_book");
        assert!(!with.contains("index_locorum.typ"), "no loci include when disabled");

        let without = build_root_typ(&book, "", None, "", false, false);
        assert!(!without.contains("#bibliography"), "{without}");

        // The Index Locorum then the Index Verborum follow the bibliography in order.
        let idx = build_root_typ(&book, "", Some("ieee"), "", true, true);
        let bib_i = idx.find("#bibliography").unwrap();
        let loci_i = idx.find("#include \"index_locorum.typ\"").expect("loci include");
        let verb_i = idx.find("#include \"index_verborum.typ\"").expect("verborum include");
        assert!(bib_i < loci_i && loci_i < verb_i, "order: bibliography → locorum → verborum");
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
