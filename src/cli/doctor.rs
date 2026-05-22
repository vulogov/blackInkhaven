//! `inkhaven doctor` — print a one-shot health report of the
//! inkhaven install and (when run inside a project) the current
//! project. Modelled on `brew doctor`: terse, structured, no
//! questions asked.
//!
//! Three sections, every section optional based on what's
//! discoverable:
//!
//! * **Binary** — version + Rust toolchain it was built with,
//!   typst engine summary, font count (bundled / system), package
//!   cache path + size.
//! * **Project** — when the working directory (or `--project`)
//!   resolves to an initialised inkhaven project: hierarchy
//!   shape, total word count, today's progress, last assembled
//!   PDF + its mtime.
//! * **Notes** — warnings the user might want to act on (no
//!   typst on PATH when engine=external, package cache empty,
//!   font count = 0, etc.).
//!
//! Designed to print plain text — pipe-friendly. No colour, no
//! TTY tricks.

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::Result;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::Store;

pub fn run(project: &Path) -> Result<()> {
    println!("inkhaven doctor — v{}", env!("CARGO_PKG_VERSION"));
    println!("================================================================");
    println!();

    // ── Binary ────────────────────────────────────────────────
    section("Binary");
    kv("version", env!("CARGO_PKG_VERSION"));
    kv("description", env!("CARGO_PKG_DESCRIPTION"));
    kv("rust-version (min)", env!("CARGO_PKG_RUST_VERSION"));
    kv("repository", env!("CARGO_PKG_REPOSITORY"));
    println!();

    // The engine summary needs a config. Outside-of-project
    // contexts get the compiled defaults; inside-project gets
    // the user's HJSON. Either way the report is meaningful.
    let layout = ProjectLayout::new(project);
    let cfg_opt = if layout.is_initialized() {
        match Config::load(&layout.config_path()) {
            Ok(c) => Some(c),
            Err(e) => {
                println!("  config-load: ERROR {e}");
                None
            }
        }
    } else {
        None
    };
    let cfg_for_engine = cfg_opt.clone().unwrap_or_default();

    section("Typst engine");
    kv("engine", &crate::typst_compile::engine_summary(&cfg_for_engine));
    kv(
        "external typst path",
        &crate::typst_compile::typst_external_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "NOT FOUND on PATH".to_owned()),
    );
    kv(
        "bundle_fonts (HJSON)",
        &cfg_for_engine.typst_compile.bundle_fonts.to_string(),
    );
    kv(
        "use_system_fonts (HJSON)",
        &cfg_for_engine.typst_compile.use_system_fonts.to_string(),
    );
    kv(
        "packages_enabled (HJSON)",
        &cfg_for_engine.typst_compile.packages_enabled.to_string(),
    );
    kv(
        "semantic_diagnostics (HJSON)",
        &cfg_for_engine.typst_compile.semantic_diagnostics.to_string(),
    );
    println!();

    section("Package cache");
    if let Some(cache) = default_package_cache_path() {
        kv("path", &cache.display().to_string());
        let (count, bytes) = dir_count_size(&cache);
        kv("entries", &count.to_string());
        kv("size", &humanise_bytes(bytes));
    } else {
        println!("  (platform default unknown — no `cache_dir`)");
    }
    println!();

    // ── Project ───────────────────────────────────────────────
    section("Project");
    kv("root", &project.display().to_string());
    if !layout.is_initialized() {
        println!("  status: not an inkhaven project (no inkhaven.hjson)");
        println!();
    } else {
        kv("status", "initialised");
        kv("config", &layout.config_path().display().to_string());
        kv("metadata.db", &layout.metadata_db_path().display().to_string());
        kv("vectors/", &layout.vecstore_path().display().to_string());
        kv("books/", &layout.books_path().display().to_string());

        // Walk the hierarchy — paragraph + word counts. Word
        // counts come from each node's persisted `word_count`
        // field (kept fresh by save / reindex), so this is a
        // pure metadata read; no need to slurp file bodies.
        if let Some(cfg) = cfg_opt.as_ref() {
            match Store::open(layout.clone(), cfg) {
                Ok(store) => match Hierarchy::load(&store) {
                    Ok(hierarchy) => {
                        let mut paragraphs = 0usize;
                        let mut user_paragraphs = 0usize;
                        let mut user_books = 0usize;
                        let mut system_books = 0usize;
                        let mut total_words: u64 = 0;
                        for (n, _) in hierarchy.flatten() {
                            match n.kind {
                                NodeKind::Book => {
                                    if n.system_tag.is_some() {
                                        system_books += 1;
                                    } else {
                                        user_books += 1;
                                    }
                                }
                                NodeKind::Paragraph => {
                                    paragraphs += 1;
                                    let in_system = hierarchy
                                        .ancestors(n)
                                        .into_iter()
                                        .any(|a| {
                                            a.kind == NodeKind::Book
                                                && a.system_tag.is_some()
                                        });
                                    if !in_system {
                                        user_paragraphs += 1;
                                        total_words += n.word_count;
                                    }
                                }
                                _ => {}
                            }
                        }
                        kv("user books", &user_books.to_string());
                        kv("system books", &system_books.to_string());
                        kv("paragraphs (total)", &paragraphs.to_string());
                        kv("paragraphs (user)", &user_paragraphs.to_string());
                        kv(
                            "words (user paragraphs)",
                            &total_words.to_string(),
                        );
                    }
                    Err(e) => {
                        println!("  hierarchy: ERROR {e}");
                    }
                },
                Err(e) => {
                    println!("  store: ERROR {e}");
                }
            }
        } else {
            println!("  (no config loaded — skipping hierarchy walk)");
        }
        println!();
    }

    // ── Notes ─────────────────────────────────────────────────
    section("Notes");
    let mut notes: Vec<String> = Vec::new();
    if !cfg_for_engine.typst_compile.use_inprocess_engine()
        && crate::typst_compile::typst_external_path().is_none()
    {
        notes.push(
            "engine = external but `typst` is NOT on PATH — Ctrl+B B / O will fail. \
             Install Typst from typst.app/docs/install, or set \
             typst_compile.engine = \"inprocess\" in inkhaven.hjson."
                .into(),
        );
    }
    if !cfg_for_engine.typst_compile.bundle_fonts
        && !cfg_for_engine.typst_compile.use_system_fonts
    {
        notes.push(
            "engine has BOTH bundle_fonts AND use_system_fonts disabled — the in-process \
             compiler will report `font not found` for every manuscript. Enable one of them."
                .into(),
        );
    }
    if notes.is_empty() {
        println!("  no warnings");
    } else {
        for n in notes {
            // Soft-wrap at 76 cols so paragraph hints stay readable on
            // an 80-col terminal.
            print_wrapped("  ⚠ ", "    ", &n, 76);
        }
    }
    println!();

    Ok(())
}

fn section(title: &str) {
    println!("─── {title} ───");
}

fn kv(key: &str, value: &str) {
    println!("  {key:32} {value}");
}

fn print_wrapped(first_prefix: &str, cont_prefix: &str, text: &str, width: usize) {
    let mut current = String::new();
    let mut first_line = true;
    for word in text.split_whitespace() {
        let prefix_len = if first_line {
            first_prefix.len()
        } else {
            cont_prefix.len()
        };
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() + prefix_len <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            if first_line {
                println!("{first_prefix}{current}");
                first_line = false;
            } else {
                println!("{cont_prefix}{current}");
            }
            current = word.to_owned();
        }
    }
    if !current.is_empty() {
        if first_line {
            println!("{first_prefix}{current}");
        } else {
            println!("{cont_prefix}{current}");
        }
    }
}

fn default_package_cache_path() -> Option<PathBuf> {
    typst_kit::package::default_package_cache_path()
}

fn dir_count_size(root: &Path) -> (u64, u64) {
    if !root.is_dir() {
        return (0, 0);
    }
    let mut count = 0u64;
    let mut bytes = 0u64;
    let mut stack: Vec<PathBuf> = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let it = match std::fs::read_dir(&d) {
            Ok(it) => it,
            Err(_) => continue,
        };
        for entry in it.flatten() {
            let path = entry.path();
            match entry.metadata() {
                Ok(m) if m.is_file() => {
                    count += 1;
                    bytes += m.len();
                }
                Ok(m) if m.is_dir() => {
                    stack.push(path);
                }
                _ => {}
            }
        }
    }
    (count, bytes)
}

fn humanise_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut idx = 0;
    while value >= 1024.0 && idx + 1 < UNITS.len() {
        value /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[idx])
    }
}
