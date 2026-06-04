//! 1.2.19+ C.4 — `inkhaven tension` subcommand.
//!
//!   * `scan` — the AI pass.  Walks each user-book
//!     chapter, asks the LLM which tensions it introduces
//!     + resolves (per-language prompt), and writes the
//!     ledger to `<project>/.inkhaven/tensions.json`.
//!   * `list` — dump the ledger.
//!
//! The opt-in `unresolved-tension` doctor scan
//! (`doctor --scan --class unresolved-tension`) then
//! flags introduced tensions with no downstream payoff.
//!
//! The AI call is the only non-deterministic boundary;
//! the matcher + parsing are unit-tested in
//! `crate::tension`.

use std::path::Path;

use crate::ai::AiClient;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::tension::{parse_tension_lines, TensionKind, TensionLedger};

use super::TensionCommand;

pub fn run(project: &Path, cmd: TensionCommand) -> Result<()> {
    match cmd {
        TensionCommand::Scan { provider } => scan(project, provider.as_deref()),
        TensionCommand::List => list(project),
    }
}

const SYSTEM_PROMPT: &str = "You are a story editor analysing narrative tension. \
For a chapter, you identify the dramatic tensions / questions / goals it \
INTRODUCES (raises for the first time) and the ones it RESOLVES (pays off). \
A tension is a dramatic promise to the reader: a mystery, a threat, a goal, \
an unanswered question, a relationship in jeopardy. Output one per line in the \
exact form:\n\
  introduce | short topic\n\
  resolve | short topic\n\
Use a short, consistent topic phrase so an introduction and its later \
resolution share key words (e.g. introduce: `the missing letter` … resolve: \
`the missing letter found`). Output nothing else — no preamble, no commentary. \
If a chapter introduces or resolves nothing, output nothing.";

fn scan(project: &Path, provider: Option<&str>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)
        .map_err(|e| Error::Store(e.to_string()))?;
    let hierarchy =
        Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;

    let ai = AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;

    let language = if cfg.language.trim().is_empty() {
        "English".to_string()
    } else {
        cfg.language.clone()
    };

    let chapters = user_book_chapters(&hierarchy);
    if chapters.is_empty() {
        return Err(Error::Store(
            "tension scan: no user-book chapters found".into(),
        ));
    }

    eprintln!(
        "inkhaven tension scan · language: {language} · model: {model} · {} chapter(s)",
        chapters.len(),
    );

    let mut ledger = TensionLedger {
        version: env!("CARGO_PKG_VERSION").to_string(),
        language: language.clone(),
        tags: Vec::new(),
    };

    for (idx, (chapter_id, chapter_title)) in chapters.iter().enumerate() {
        let prose =
            crate::cli::book_walk::chapter_raw_prose(&layout, &hierarchy, *chapter_id);
        let plain = crate::audiobook::typst_to_plain(&prose);
        if plain.trim().is_empty() {
            continue;
        }
        eprint!("  [{}/{}] {chapter_title} ", idx + 1, chapters.len());
        let prompt = build_prompt(&language, chapter_title, &plain);
        let raw = run_blocking(&ai, model, &prompt)?;
        let tags = parse_tension_lines(&raw, chapter_title, idx);
        let (intro, res) = tag_counts(&tags);
        eprintln!("→ {intro} introduced, {res} resolved");
        ledger.tags.extend(tags);
    }

    ledger
        .save(&layout.root)
        .map_err(|e| Error::Store(format!("tension save: {e}")))?;
    println!(
        "tension: tagged {} tension(s) across {} chapter(s) → {}\n\
         run `inkhaven doctor --scan --class unresolved-tension` to flag the open ones",
        ledger.tags.len(),
        chapters.len(),
        TensionLedger::sidecar_path(&layout.root).display(),
    );
    Ok(())
}

fn tag_counts(tags: &[crate::tension::TensionTag]) -> (usize, usize) {
    let intro = tags.iter().filter(|t| t.kind == TensionKind::Introduce).count();
    (intro, tags.len() - intro)
}

fn build_prompt(language: &str, chapter: &str, prose: &str) -> String {
    format!(
        "Language of the manuscript: {language}.\n\
         List the tensions introduced and resolved in this chapter \
         (\"{chapter}\"). One per line, `introduce | topic` or \
         `resolve | topic`, no other output.\n\n\
         --- CHAPTER PROSE ---\n{prose}\n--- END ---",
    )
}

fn run_blocking(ai: &AiClient, model: &str, prompt: &str) -> Result<String> {
    crate::ai::stream::collect_blocking(
        ai.client.clone(),
        model.to_string(),
        Some(SYSTEM_PROMPT.to_string()),
        prompt.to_string(),
    )
    .map_err(|e| Error::Store(format!("inference error: {e}")))
}

fn list(project: &Path) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let ledger = TensionLedger::load(&layout.root)
        .map_err(|e| Error::Store(format!("tension load: {e}")))?;
    if ledger.tags.is_empty() {
        println!("tension: no tags yet — run `inkhaven tension scan` first");
        return Ok(());
    }
    println!(
        "Tension ledger — {} tag(s), language {}\n",
        ledger.tags.len(),
        ledger.language,
    );
    for t in &ledger.tags {
        let mark = match t.kind {
            TensionKind::Introduce => "▲ introduce",
            TensionKind::Resolve => "▼ resolve  ",
        };
        println!("  {mark}  {:<32} [{}]", t.topic, t.chapter);
    }
    Ok(())
}

fn user_book_chapters(h: &Hierarchy) -> Vec<(uuid::Uuid, String)> {
    let mut out = Vec::new();
    for node in h.iter() {
        if node.kind != NodeKind::Chapter {
            continue;
        }
        let under_system = h
            .ancestors(node)
            .iter()
            .any(|a| a.kind == NodeKind::Book && a.system_tag.is_some());
        if !under_system {
            out.push((node.id, node.title.clone()));
        }
    }
    out
}

