//! 1.2.19+ C.3 — `inkhaven continuity` subcommand.
//!
//! Builds + inspects the continuity bible:
//!
//!   * `extract` — the AI pass.  Walks each user-book
//!     chapter, asks the LLM to extract established
//!     character facts (per-language prompt), and writes
//!     them to `<project>/.inkhaven/continuity.json`.
//!   * `list` — dump the extracted bible (per character,
//!     per attribute, per chapter).
//!
//! The `continuity-drift` doctor scan then flags
//! attributes that change across chapters.  The AI call
//! is the only non-deterministic boundary; the prompt
//! assembly + fact parsing are unit-tested in
//! `crate::continuity_bible`.

use std::path::Path;

use crate::ai::AiClient;
use crate::config::Config;
use crate::continuity_bible::{ContinuityBible, parse_extraction};
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;

use super::ContinuityCommand;

pub fn run(project: &Path, cmd: ContinuityCommand) -> Result<()> {
    match cmd {
        ContinuityCommand::Extract { provider } => {
            extract(project, provider.as_deref())
        }
        ContinuityCommand::List => list(project),
    }
}

const SYSTEM_PROMPT: &str = "You are a continuity editor for a novel. You extract \
ESTABLISHED, FACTUAL attributes of characters from prose — appearance \
(eye colour, hair, height, scars), origin (hometown), relationships, \
possessions, occupation, age. You do NOT infer mood, intentions, or \
one-off actions. Output ONE fact per line in the exact form:\n\
  Character | attribute_key | value\n\
Use a short snake_case attribute_key (eye_color, hometown, occupation, \
weapon, relationship_to_X). Keep values to a few words. Output nothing \
else — no preamble, no markdown, no commentary. If a chapter establishes \
no durable facts, output nothing.";

fn extract(project: &Path, provider: Option<&str>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)
        .map_err(|e| Error::Store(e.to_string()))?;
    let hierarchy =
        Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let bible = extract_with(&hierarchy, &cfg, &layout, provider, &|s| eprintln!("{s}"))?;
    println!(
        "continuity: extracted {} fact(s) for {} character(s) → {}",
        bible.facts.len(),
        bible.characters().len(),
        ContinuityBible::sidecar_path(&layout.root).display(),
    );
    Ok(())
}

/// 1.3.12 DEEP-1 — the continuity-extract body against an already-loaded
/// `hierarchy` (the prose is read from disk, not the DB), with progress routed
/// through `progress` (not stderr — so it's safe to call from the TUI
/// background-refresh thread). Reads `cfg.language` so the extraction prompt
/// stays in the manuscript's language. Returns the bible it saved.
pub fn extract_with(
    hierarchy: &Hierarchy,
    cfg: &Config,
    layout: &ProjectLayout,
    provider: Option<&str>,
    progress: &dyn Fn(&str),
) -> Result<ContinuityBible> {
    let ai = AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;

    let language = if cfg.language.trim().is_empty() {
        "English".to_string()
    } else {
        cfg.language.clone()
    };

    let chapters = hierarchy.user_book_chapters();
    if chapters.is_empty() {
        return Err(Error::Store(
            "continuity extract: no user-book chapters found".into(),
        ));
    }

    progress(&format!(
        "continuity extract · language: {language} · model: {model} · {} chapter(s)",
        chapters.len(),
    ));

    let mut bible = ContinuityBible {
        version: env!("CARGO_PKG_VERSION").to_string(),
        language: language.clone(),
        facts: Vec::new(),
    };

    for (idx, (chapter_id, chapter_title)) in chapters.iter().enumerate() {
        let prose = crate::cli::book_walk::chapter_raw_prose(layout, hierarchy, *chapter_id);
        let plain = crate::audiobook::typst_to_plain(&prose);
        if plain.trim().is_empty() {
            continue;
        }
        progress(&format!("continuity [{}/{}] {chapter_title}", idx + 1, chapters.len()));
        let prompt = build_extract_prompt(&language, chapter_title, &plain);
        let raw = run_blocking(&ai, model, &prompt)?;
        bible.facts.extend(parse_extraction(&raw, chapter_title));
    }

    bible
        .save(&layout.root)
        .map_err(|e| Error::Store(format!("continuity save: {e}")))?;
    Ok(bible)
}

/// Compose the per-chapter extraction prompt.  Language is
/// woven in so the model answers in the manuscript's
/// language (Tier-1 multilingual).
fn build_extract_prompt(language: &str, chapter: &str, prose: &str) -> String {
    format!(
        "Language of the manuscript: {language}.\n\
         Extract the established character facts from this chapter \
         (\"{chapter}\"). Remember: one fact per line, \
         `Character | attribute_key | value`, no other output.\n\n\
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
    let bible = ContinuityBible::load(&layout.root)
        .map_err(|e| Error::Store(format!("continuity load: {e}")))?;
    if bible.facts.is_empty() {
        println!(
            "continuity: no facts yet — run `inkhaven continuity extract` first"
        );
        return Ok(());
    }
    println!(
        "Continuity bible — {} fact(s), language {}\n",
        bible.facts.len(),
        bible.language,
    );
    for character in bible.characters() {
        println!("{character}");
        // Group this character's facts by attribute.
        let mut rows: Vec<&crate::continuity_bible::CharacterFact> = bible
            .facts
            .iter()
            .filter(|f| f.character == character)
            .collect();
        rows.sort_by(|a, b| {
            a.attribute.cmp(&b.attribute).then(a.chapter.cmp(&b.chapter))
        });
        for f in rows {
            println!("  {:<20} {:<28} [{}]", f.attribute, f.value, f.chapter);
        }
        println!();
    }
    Ok(())
}

