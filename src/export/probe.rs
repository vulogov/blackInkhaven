//! Export probe (3.22) — every export format against one seeded project,
//! checked for what it promises: every paragraph present in reading order,
//! the `--book-name`, `--status` and `--tag` scopes honoured, the EPUB
//! reimportable, the HTML site complete, the DOCX readable. Runs the real CLI
//! entry points (`cli::export::run`, `cli::docx::run`,
//! `cli::import_epub::run`) over a temp project; the PDF step runs only when
//! `typst` is on `PATH` and is reported either way. `#[ignore]`d because it
//! opens real stores and shells out:
//!
//! ```text
//! cargo test --bin inkhaven -- --ignored --nocapture export_probe
//! ```
#![cfg(test)]

use std::path::{Path, PathBuf};

use crate::cli::ExportFormat;
use crate::config::Config;
use crate::probe_kit::run_probe;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::{InsertPosition, NodeKind, Store};

/// A seeded paragraph: title, body sentinel (a word that appears nowhere
/// else), status, tags.
struct Para {
    title: &'static str,
    sentinel: &'static str,
    status: Option<&'static str>,
    tags: &'static [&'static str],
}

/// Reading order of the main book: Arrival / Quay, Ledger, Night / Lantern;
/// Thaw / March.
const MAIN: [Para; 4] = [
    Para { title: "Quay", sentinel: "QUAYSTONE", status: Some("ready"), tags: &["keep"] },
    Para { title: "Ledger", sentinel: "LEDGERLINE", status: Some("napkin"), tags: &[] },
    Para { title: "Lantern", sentinel: "LANTERNGLOW", status: None, tags: &[] },
    Para { title: "March", sentinel: "MARCHTHAW", status: Some("ready"), tags: &[] },
];
const OTHER_SENTINEL: &str = "BOOTSMUD";

fn body(title: &str, sentinel: &str) -> String {
    format!("= {title}\n\nThe {sentinel} sentence stands in for prose. A second sentence follows it.\n")
}

fn seed(store: &Store, cfg: &Config, root: &Path, parent: &Node, p: &Para) -> Node {
    let h = Hierarchy::load(store).unwrap();
    let mut node = store
        .create_paragraph_with_body(
            cfg,
            &h,
            p.title,
            Some(parent),
            InsertPosition::End,
            body(p.title, p.sentinel).as_bytes(),
            None,
        )
        .expect("create paragraph");
    if let Some(rel) = &node.file {
        std::fs::write(root.join(rel), body(p.title, p.sentinel)).unwrap();
    }
    if p.status.is_some() || !p.tags.is_empty() {
        node.status = p.status.map(String::from);
        node.tags = p.tags.iter().map(|t| t.to_string()).collect();
        store.raw().update_metadata(node.id, node.to_json()).expect("tags/status");
    }
    node
}

struct Project {
    #[allow(dead_code)] // keeps the TempDir alive for the probe's duration
    tmp: tempfile::TempDir,
    root: PathBuf,
}

/// Two user books: the main one (two chapters, a subchapter) and a one-paragraph
/// second book that must never leak into a scoped export.
fn project() -> Project {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut cfg = Config::default();
    cfg.images.preview_enabled = false;
    cfg.sync_interval_seconds = 0;
    let layout = ProjectLayout::new(tmp.path());
    layout.create_layout().unwrap();
    cfg.save(&layout.config_path()).unwrap();
    let store = Store::open(layout.clone(), &cfg).expect("open store");
    let mk = |kind: NodeKind, title: &str, parent: Option<&Node>| -> Node {
        let h = Hierarchy::load(&store).unwrap();
        store.create_node(&cfg, &h, kind, title, parent, None, InsertPosition::End).unwrap()
    };
    let book = mk(NodeKind::Book, "Harbor Lights", None);
    let arrival = mk(NodeKind::Chapter, "Arrival", Some(&book));
    seed(&store, &cfg, tmp.path(), &arrival, &MAIN[0]);
    seed(&store, &cfg, tmp.path(), &arrival, &MAIN[1]);
    let night = mk(NodeKind::Subchapter, "Night", Some(&arrival));
    seed(&store, &cfg, tmp.path(), &night, &MAIN[2]);
    let thaw = mk(NodeKind::Chapter, "Thaw", Some(&book));
    seed(&store, &cfg, tmp.path(), &thaw, &MAIN[3]);
    let other = mk(NodeKind::Book, "Field Notes", None);
    let walk = mk(NodeKind::Chapter, "Walk", Some(&other));
    seed(
        &store,
        &cfg,
        tmp.path(),
        &walk,
        &Para { title: "Boots", sentinel: OTHER_SENTINEL, status: None, tags: &[] },
    );
    drop(store);
    Project { root: tmp.path().to_path_buf(), tmp }
}

#[allow(clippy::too_many_arguments)]
fn export(
    root: &Path,
    format: ExportFormat,
    output: &Path,
    book: Option<&str>,
    status: Option<&str>,
    tag: Option<&str>,
    bundle: Option<&Path>,
) -> Result<(), String> {
    crate::cli::export::run(root, format, Some(output), book, status, tag, &[], None, None, false, bundle)
        .map_err(|e| format!("{e}"))
}

/// Read every file under `dir` (recursively) with one of `exts` and join
/// their text in path order.
fn read_tree(dir: &Path, exts: &[&str]) -> String {
    let mut files: Vec<PathBuf> = Vec::new();
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else {
                    out.push(p);
                }
            }
        }
    }
    walk(dir, &mut files);
    files.sort();
    files
        .iter()
        .filter(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| exts.contains(&e)))
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every text entry of a zip (EPUB / DOCX), concatenated, plus the entry names.
fn zip_text(path: &Path) -> Result<(Vec<String>, String), String> {
    let f = std::fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("zip {}: {e}", path.display()))?;
    let mut names = Vec::new();
    let mut text = String::new();
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| format!("entry {i}: {e}"))?;
        names.push(e.name().to_string());
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut e, &mut buf).map_err(|e| format!("read entry {i}: {e}"))?;
        if let Ok(s) = String::from_utf8(buf) {
            text.push_str(&s);
            text.push('\n');
        }
    }
    Ok((names, text))
}

/// Positions of each sentinel in `text`, in MAIN order; `None` when absent.
fn positions(text: &str) -> Vec<Option<usize>> {
    MAIN.iter().map(|p| text.find(p.sentinel)).collect()
}

fn check_full_book(label: &str, text: &str, problems: &mut Vec<String>, out: &mut Vec<String>) {
    let pos = positions(text);
    for (p, at) in MAIN.iter().zip(&pos) {
        if at.is_none() {
            problems.push(format!("[{label}] paragraph `{}` ({}) is missing", p.title, p.sentinel));
        }
    }
    let present: Vec<usize> = pos.iter().flatten().copied().collect();
    if present.len() == MAIN.len() && present.windows(2).any(|w| w[0] > w[1]) {
        problems.push(format!("[{label}] paragraphs are out of reading order: {pos:?}"));
    }
    if text.contains(OTHER_SENTINEL) {
        problems.push(format!("[{label}] the other book's paragraph leaked into a `--book-name` export"));
    }
    // Two documented structures: the flat exports (Typst/Markdown/TeX/bundle)
    // emit paragraphs only — branch nodes carry no text and the paragraphs'
    // `= Title` headings shape the document (`assemble_typst_source`); the
    // chapter-built ones (EPUB/DOCX/HTML) use the chapter titles as headings
    // and strip each paragraph's leading `= Title` (`epub.rs`).
    if matches!(label, "EPUB" | "DOCX" | "HTML") {
        // Standard manuscript format (DOCX) sets chapter headings in capitals.
        let haystack = if label == "DOCX" { text.to_uppercase() } else { text.to_string() };
        for title in ["Arrival", "Thaw"] {
            let needle = if label == "DOCX" { title.to_uppercase() } else { title.to_string() };
            if !haystack.contains(&needle) {
                problems.push(format!("[{label}] chapter title `{title}` is missing"));
            }
        }
    } else {
        for p in MAIN {
            if !text.contains(p.title) {
                problems.push(format!("[{label}] paragraph heading `{}` is missing", p.title));
            }
        }
    }
    out.push(format!("  [{label}] {} chars, every paragraph present and in order", text.len()));
}

#[test]
#[ignore = "probe: opens real stores, writes every export format, shells out to typst; run with --ignored --nocapture"]
fn export_probe() {
    run_probe("export", |out| {
        let mut problems: Vec<String> = Vec::new();
        let proj = project();
        let root = proj.root.clone();
        let outdir = root.join("exports-probe");
        std::fs::create_dir_all(&outdir).unwrap();
        let book = Some("Harbor Lights");

        // ---- Refusals the CLI promises -----------------------------------
        match export(&root, ExportFormat::Typst, &outdir.join("x.typ"), None, None, None, None) {
            Ok(()) => problems.push("[SCOPE] two user books and no --book-name was accepted".into()),
            Err(e) if e.contains("--book-name") && e.contains("Harbor Lights") && e.contains("Field Notes") => {
                out.push("  [SCOPE] two books without --book-name refused, both listed".into())
            }
            Err(e) => problems.push(format!("[SCOPE] refusal does not list the books: {e}")),
        }
        match export(&root, ExportFormat::Typst, &outdir.join("x.typ"), Some("Nope"), None, None, None) {
            Err(e) if e.contains("no book matches") => out.push("  [SCOPE] unknown --book-name refused".into()),
            other => problems.push(format!("[SCOPE] unknown --book-name: {other:?}")),
        }
        match export(&root, ExportFormat::Typst, &outdir.join("x.typ"), book, Some("polished"), None, None) {
            Err(e) if e.contains("unknown --status") => out.push("  [SCOPE] unknown --status refused".into()),
            other => problems.push(format!("[SCOPE] unknown --status: {other:?}")),
        }

        // ---- Typst / Markdown / TeX: the combined pipeline ------------------
        for (fmt, name, label) in [
            (ExportFormat::Typst, "book.typ", "TYPST"),
            (ExportFormat::Markdown, "book.md", "MARKDOWN"),
            (ExportFormat::Tex, "book.tex", "TEX"),
        ] {
            let path = outdir.join(name);
            match export(&root, fmt, &path, book, None, None, None) {
                Err(e) => problems.push(format!("[{label}] export failed: {e}")),
                Ok(()) => match std::fs::read_to_string(&path) {
                    Err(e) => problems.push(format!("[{label}] {} not written: {e}", path.display())),
                    Ok(text) => check_full_book(label, &text, &mut problems, out),
                },
            }
        }
        // The typst source keeps the headings as Typst headings; the markdown
        // turns them into `#` headings; the TeX into sectioning commands.
        if let Ok(t) = std::fs::read_to_string(outdir.join("book.md")) {
            if !t.lines().any(|l| l.starts_with('#') && l.contains("Quay")) {
                problems.push("[MARKDOWN] `Quay` is not a markdown heading".into());
            }
        }
        if let Ok(t) = std::fs::read_to_string(outdir.join("book.tex")) {
            if !(t.contains("\\chapter") || t.contains("\\section")) {
                problems.push("[TEX] no sectioning command in the LaTeX".into());
            }
        }

        // ---- --status and --tag scopes -----------------------------------
        let scoped = outdir.join("ready.typ");
        match export(&root, ExportFormat::Typst, &scoped, book, Some("ready"), None, None) {
            Err(e) => problems.push(format!("[STATUS] export failed: {e}")),
            Ok(()) => {
                let t = std::fs::read_to_string(&scoped).unwrap_or_default();
                let pos = positions(&t);
                // ready: Quay and March; napkin and unset are below the floor.
                let want = [true, false, false, true];
                for ((p, at), w) in MAIN.iter().zip(&pos).zip(want) {
                    if at.is_some() != w {
                        problems.push(format!(
                            "[STATUS] --status ready: `{}` ({:?}) {}",
                            p.title,
                            p.status,
                            if w { "missing" } else { "present" }
                        ));
                    }
                }
                out.push(format!("  [STATUS] --status ready keeps {} of {} paragraphs", pos.iter().flatten().count(), MAIN.len()));
            }
        }
        let tagged = outdir.join("keep.typ");
        match export(&root, ExportFormat::Typst, &tagged, book, None, Some("keep"), None) {
            Err(e) => problems.push(format!("[TAG] export failed: {e}")),
            Ok(()) => {
                let t = std::fs::read_to_string(&tagged).unwrap_or_default();
                let pos = positions(&t);
                if pos[0].is_none() || pos[1..].iter().any(|p| p.is_some()) {
                    problems.push(format!("[TAG] --tag keep should keep only `Quay`: {pos:?}"));
                } else {
                    out.push("  [TAG] --tag keep keeps only the tagged paragraph".into());
                }
            }
        }

        // ---- PDF (typst on PATH) -------------------------------------------
        let have_typst = std::process::Command::new("typst")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if have_typst {
            let pdf = outdir.join("book.pdf");
            match export(&root, ExportFormat::Pdf, &pdf, book, None, None, None) {
                Err(e) => problems.push(format!("[PDF] export failed: {e}")),
                Ok(()) => match std::fs::read(&pdf) {
                    Ok(bytes) if bytes.starts_with(b"%PDF") && bytes.len() > 1024 => {
                        out.push(format!("  [PDF] {} bytes, %PDF header", bytes.len()))
                    }
                    Ok(bytes) => problems.push(format!("[PDF] not a PDF ({} bytes)", bytes.len())),
                    Err(e) => problems.push(format!("[PDF] not written: {e}")),
                },
            }
        } else {
            out.push("  [PDF] skipped — `typst` is not on PATH".into());
        }

        // ---- EPUB: build, inspect, reimport --------------------------------
        let epub = outdir.join("book.epub");
        match export(&root, ExportFormat::Epub, &epub, book, None, None, None) {
            Err(e) => problems.push(format!("[EPUB] export failed: {e}")),
            Ok(()) => match zip_text(&epub) {
                Err(e) => problems.push(format!("[EPUB] {e}")),
                Ok((names, text)) => {
                    if names.first().map(String::as_str) != Some("mimetype") {
                        problems.push(format!("[EPUB] first entry must be `mimetype`, got {:?}", names.first()));
                    }
                    if !names.iter().any(|n| n.ends_with(".opf")) {
                        problems.push("[EPUB] no .opf package document".into());
                    }
                    if !names.iter().any(|n| n.contains("nav") || n.contains("toc")) {
                        problems.push(format!("[EPUB] no nav/toc entry in {names:?}"));
                    }
                    check_full_book("EPUB", &text, &mut problems, out);
                    out.push(format!("  [EPUB] {} entries", names.len()));
                }
            },
        }
        if epub.exists() {
            let tmp2 = tempfile::tempdir().unwrap();
            let layout2 = ProjectLayout::new(tmp2.path());
            layout2.create_layout().unwrap();
            let mut cfg2 = Config::default();
            cfg2.images.preview_enabled = false;
            cfg2.sync_interval_seconds = 0;
            cfg2.save(&layout2.config_path()).unwrap();
            match crate::cli::import_epub::run(tmp2.path(), &epub, None, false) {
                Err(e) => problems.push(format!("[REIMPORT] import-epub failed: {e}")),
                Ok(()) => {
                    let store2 = Store::open(layout2.clone(), &cfg2).unwrap();
                    let h2 = Hierarchy::load(&store2).unwrap();
                    let books: Vec<&Node> = h2
                        .children_of(None)
                        .into_iter()
                        .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
                        .collect();
                    match books.as_slice() {
                        [b] => {
                            if b.title != "Harbor Lights" {
                                problems.push(format!("[REIMPORT] book title `{}`", b.title));
                            }
                            let chapters: Vec<String> = h2
                                .children_of(Some(b.id))
                                .into_iter()
                                .filter(|n| n.kind == NodeKind::Chapter)
                                .map(|n| n.title.clone())
                                .collect();
                            for want in ["Arrival", "Thaw"] {
                                if !chapters.iter().any(|c| c == want) {
                                    problems.push(format!("[REIMPORT] chapter `{want}` missing from {chapters:?}"));
                                }
                            }
                            // Every sentinel comes back as paragraph text.
                            let mut text = String::new();
                            for id in h2.collect_subtree(b.id) {
                                if let Some(n) = h2.get(id) {
                                    if n.kind == NodeKind::Paragraph {
                                        if let Some(rel) = &n.file {
                                            text.push_str(&std::fs::read_to_string(tmp2.path().join(rel)).unwrap_or_default());
                                        }
                                    }
                                }
                            }
                            let pos = positions(&text);
                            for (p, at) in MAIN.iter().zip(&pos) {
                                if at.is_none() {
                                    problems.push(format!("[REIMPORT] `{}` ({}) did not survive the round trip", p.title, p.sentinel));
                                }
                            }
                            let present: Vec<usize> = pos.iter().flatten().copied().collect();
                            if present.windows(2).any(|w| w[0] > w[1]) {
                                problems.push(format!("[REIMPORT] reading order changed: {pos:?}"));
                            }
                            out.push(format!("  [REIMPORT] book `{}`, chapters {chapters:?}, every paragraph back", b.title));
                        }
                        other => problems.push(format!("[REIMPORT] expected one user book, found {}", other.len())),
                    }
                }
            }
        }

        // ---- HTML site -----------------------------------------------------
        let site = outdir.join("site");
        match export(&root, ExportFormat::Html, &site, book, None, None, None) {
            Err(e) => problems.push(format!("[HTML] export failed: {e}")),
            Ok(()) => {
                for f in ["index.html", "theme.css"] {
                    if !site.join(f).exists() {
                        problems.push(format!("[HTML] {f} missing"));
                    }
                }
                let text = read_tree(&site, &["html"]);
                check_full_book("HTML", &text, &mut problems, out);
                let n = std::fs::read_dir(&site).map(|d| d.count()).unwrap_or(0);
                out.push(format!("  [HTML] {n} files in the site"));
            }
        }

        // ---- DOCX ------------------------------------------------------------
        let docx = outdir.join("book.docx");
        match crate::cli::docx::run(&root, book, Some(&docx), None, Some("Probe Author"), None, None) {
            Err(e) => problems.push(format!("[DOCX] export failed: {e}")),
            Ok(()) => match zip_text(&docx) {
                Err(e) => problems.push(format!("[DOCX] {e}")),
                Ok((names, text)) => {
                    if !names.iter().any(|n| n == "word/document.xml") {
                        problems.push(format!("[DOCX] no word/document.xml in {names:?}"));
                    }
                    check_full_book("DOCX", &text, &mut problems, out);
                    if !text.contains("Probe Author") {
                        problems.push("[DOCX] --author did not reach the document".into());
                    }
                }
            },
        }

        // ---- arXiv bundle ----------------------------------------------------
        let bundle = outdir.join("bundle");
        match export(&root, ExportFormat::Tex, &outdir.join("bundle.tex"), book, None, None, Some(&bundle)) {
            Err(e) => problems.push(format!("[BUNDLE] export failed: {e}")),
            Ok(()) => {
                let text = read_tree(&bundle, &["tex", "bib"]);
                if text.is_empty() && !bundle.with_extension("zip").exists() {
                    problems.push(format!("[BUNDLE] nothing written under {}", bundle.display()));
                } else if !text.is_empty() {
                    check_full_book("BUNDLE", &text, &mut problems, out);
                } else {
                    out.push("  [BUNDLE] zipped bundle written".into());
                }
            }
        }

        // ---- Re-export overwrites ------------------------------------------
        let md = outdir.join("book.md");
        let before = std::fs::metadata(&md).map(|m| m.len()).unwrap_or(0);
        if export(&root, ExportFormat::Markdown, &md, book, None, None, None).is_ok() {
            let after = std::fs::metadata(&md).map(|m| m.len()).unwrap_or(0);
            if before != after {
                problems.push(format!("[REEXPORT] markdown size changed on an unchanged project: {before} → {after}"));
            } else {
                out.push("  [REEXPORT] a second export reproduces the file".into());
            }
        }

        out.push(String::new());
        out.push(format!("PROBLEMS ({})", problems.len()));
        for p in &problems {
            out.push(format!("  - {p}"));
        }
        assert!(problems.is_empty(), "{} export problem(s):\n{}", problems.len(), problems.join("\n"));
    });
}
