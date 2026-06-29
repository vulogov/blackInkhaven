//! INNER-THEOLOGIAN-1 (IT-P9) — `inkhaven theologian` handlers. Mirrors
//! `cli/utopia.rs`. `scan` is deterministic (zero-AI, exit 1 on any unsuppressed
//! signal); `session` runs the slow-track LLM over a chapter or the book and
//! prints the questions; `suppress` mutes a signal via its paragraph. Findings
//! are advisory; the cap informs, never blocks. (The RFC's `resume` is dropped —
//! the audit found no persisted-session backing, for Socrates either.)

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::inner_theologian::{
    DetectWindows, QuestionCategory, THEOLOGIAN_SYSTEM, TheologianStore, TraditionLens,
    build_grounding, build_session_prompt, run_fast_scan, theologian_llm_call,
};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::{NodeKind, Store};

use super::TheologianCommand;

pub fn run(project: &Path, cmd: TheologianCommand) -> Result<()> {
    match cmd {
        TheologianCommand::Scan { book, signal, json } => {
            scan(project, book.as_deref(), signal.as_deref(), json)
        }
        TheologianCommand::Session { book, chapter, category, lens, json } => {
            session(project, book.as_deref(), chapter, category, lens.as_deref(), json)
        }
        TheologianCommand::Suppress { para, reason, book } => {
            suppress(project, book.as_deref(), &para, &reason)
        }
    }
}

fn se(e: anyhow::Error) -> Error {
    Error::Store(e.to_string())
}

fn open(project: &Path) -> Result<(ProjectLayout, Config, Store, Hierarchy)> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;
    Ok((layout, cfg, store, h))
}

fn windows(cfg: &Config) -> DetectWindows {
    DetectWindows {
        moral_invisibility: cfg.theologian.moral_invisibility_window,
        consequence_gap: cfg.theologian.consequence_gap_window,
    }
}

fn scan(project: &Path, book_name: Option<&str>, signal: Option<&str>, json: bool) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "theologian").map_err(Error::Store)?;
    let ts = TheologianStore::open(store.project_root()).map_err(se)?;
    run_fast_scan(&ts, &layout, &h, &cfg, book, windows(&cfg), cfg.theologian.sacred_levity_signal)
        .map_err(se)?;

    let mut findings = ts.findings(&book.slug, false).map_err(se)?;
    if let Some(sig) = signal {
        if sig != "all" {
            let want = sig.replace('-', "_");
            findings.retain(|f| f.signal_type.as_code() == want);
        }
    }

    if json {
        let arr: Vec<serde_json::Value> = findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "signal_type": f.signal_type.as_code(),
                    "chapter": f.chapter_ord,
                    "para_id": f.para_id,
                    "description": f.description,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
    } else {
        println!("Inner Theologian — fast-track signals · `{}`", book.title);
        println!("{}", "─".repeat(72));
        if findings.is_empty() {
            println!("No unsuppressed signals.");
        }
        for f in &findings {
            println!("  [ch.{} · {}] {}", f.chapter_ord, f.signal_type.label(), f.description);
        }
    }

    if !findings.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

fn session(
    project: &Path,
    book_name: Option<&str>,
    chapter: Option<u32>,
    category: Option<u8>,
    lens: Option<&str>,
    json: bool,
) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "theologian").map_err(Error::Store)?;
    let cat = QuestionCategory::from_number(category.unwrap_or(6))
        .ok_or_else(|| Error::Store("category must be 1–6".into()))?;
    let (lang, _note) =
        crate::prose::resolve_prose_language(cfg.theologian.language.as_deref(), &cfg.language);

    let passage = gather_text(&layout, &h, book, chapter);
    if passage.trim().is_empty() {
        return Err(Error::Store("no prose found for that scope".into()));
    }

    // Grounding: the in-scope fast-track signals (deterministic, no recompute on
    // the read path — they were scanned by the review pass / `scan`).
    let ts = TheologianStore::open(store.project_root()).map_err(se)?;
    let scope_signals = match chapter {
        Some(c) => ts.findings_for_chapter(&book.slug, c).map_err(se)?,
        None => ts.findings(&book.slug, false).map_err(se)?,
    };
    let grounding = build_grounding(store.project_root(), &book.slug, &scope_signals);

    let mut user =
        build_session_prompt(cat, &passage, grounding.as_deref(), &lang, &cfg.theologian.disabled_lenses);
    if let Some(code) = lens {
        match TraditionLens::from_code(code) {
            Some(t) => {
                user = format!("Use ONLY the {} lens for this session.\n\n{user}", t.label());
            }
            None => {
                let valid = TraditionLens::ALL.iter().map(|l| l.as_code()).collect::<Vec<_>>().join(", ");
                return Err(Error::Store(format!("unknown lens `{code}` — valid: {valid}")));
            }
        }
    }

    let raw = theologian_llm_call(&cfg, THEOLOGIAN_SYSTEM, &user).map_err(se)?;
    if json {
        println!("{}", serde_json::json!({ "category": cat.number(), "questions": raw.trim() }));
    } else {
        println!("Inner Theologian — {} · `{}`{}", cat.label(), book.title, chapter.map(|c| format!(" · ch.{c}")).unwrap_or_default());
        println!("{}", "─".repeat(72));
        println!("{}", raw.trim());
    }
    Ok(())
}

fn suppress(project: &Path, book_name: Option<&str>, para: &str, reason: &str) -> Result<()> {
    let (_layout, _cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "theologian").map_err(Error::Store)?;
    let ts = TheologianStore::open(store.project_root()).map_err(se)?;
    let n = ts.suppress_paragraph(&book.slug, para).map_err(se)?;
    if n == 0 {
        println!("No theologian signal on paragraph `{para}` (nothing to suppress).");
    } else {
        println!("Suppressed {n} signal(s) on `{para}` — {reason}");
    }
    Ok(())
}

/// Plain prose text for a book (or one chapter), Jinja excluded, Typst stripped.
/// Capped so a whole-book session prompt stays bounded.
fn gather_text(layout: &ProjectLayout, h: &Hierarchy, book: &crate::store::node::Node, chapter: Option<u32>) -> String {
    const CAP: usize = 16_000;
    let chapters: Vec<_> = h
        .children_of(Some(book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter)
        .collect();
    let mut out = String::new();
    for (idx, ch) in chapters.iter().enumerate() {
        if let Some(c) = chapter {
            if (idx + 1) as u32 != c {
                continue;
            }
        }
        for id in h.collect_subtree(ch.id) {
            let Some(p) = h.get(id) else { continue };
            if p.kind != NodeKind::Paragraph || p.content_type.as_deref() == Some("jinja") {
                continue;
            }
            if let Some(rel) = p.file.as_ref() {
                if let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) {
                    out.push_str(&crate::audiobook::typst_to_plain(&raw));
                    out.push('\n');
                    if out.len() >= CAP {
                        return out;
                    }
                }
            }
        }
    }
    out
}
