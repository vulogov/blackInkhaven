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
        ContinuityCommand::Check { only, skip, json } => check(project, &only, &skip, json),
    }
}

/// SENTINEL — the unified deterministic continuity ledger. Runs the engine over
/// the selected detectors, prints the ranked findings (human or JSON), and exits
/// non-zero when any Contradiction survives.
fn check(project: &Path, only: &[String], skip: &[String], json: bool) -> Result<()> {
    use crate::continuity_intel::engine;
    use crate::continuity_intel::Severity;

    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let hierarchy = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;

    // Nudge on a mistyped detector name rather than silently running nothing.
    for key in only.iter().chain(skip.iter()) {
        if !engine::DETECTORS.contains(&key.to_ascii_lowercase().as_str()) {
            eprintln!(
                "continuity check: unknown detector `{key}` (known: {})",
                engine::DETECTORS.join(", "),
            );
        }
    }

    let sel = engine::selector(only, skip);
    let findings = engine::run(&store, &cfg, &layout, &hierarchy, &sel);
    let contradictions = findings.iter().filter(|f| f.severity == Severity::Contradiction).count();

    if json {
        let rows: Vec<serde_json::Value> = findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "kind": f.kind,
                    "severity": f.severity.label(),
                    "chapter": f.chapter,
                    "anchor": f.anchor.map(|a| a.to_string()),
                    "entities": f.entities,
                    "message": f.message,
                    "source": f.source,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".into()));
    } else if findings.is_empty() {
        println!("\u{2713} no continuity breaks detected");
    } else {
        for f in &findings {
            let icon = match f.severity {
                Severity::Contradiction => "\u{2297}", // ⊗
                Severity::Warning => "\u{26a0}",       // ⚠
                Severity::Info => "\u{25cf}",          // ●
            };
            let where_ = if f.chapter == 0 { String::new() } else { format!(" (ch. {})", f.chapter) };
            println!("{icon} [{}] {}{where_}", f.source, f.message);
        }
        println!(
            "\n{} finding(s): {contradictions} contradiction(s), {} other.",
            findings.len(),
            findings.len() - contradictions,
        );
    }

    if contradictions > 0 {
        return Err(Error::Store(format!(
            "{contradictions} continuity contradiction(s) — see above"
        )));
    }
    Ok(())
}

// The continuity-extract system prompt now lives, localized, in
// `cli::world_prompts` (slug `continuity`, 1.3.13).

fn extract(project: &Path, provider: Option<&str>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)
        .map_err(|e| Error::Store(e.to_string()))?;
    let hierarchy =
        Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let never = std::sync::atomic::AtomicBool::new(false);
    let bible = extract_with(&store, &hierarchy, &cfg, &layout, provider, &never, &|s| eprintln!("{s}"))?;
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
#[allow(clippy::too_many_arguments)]
pub fn extract_with(
    store: &Store,
    hierarchy: &Hierarchy,
    cfg: &Config,
    layout: &ProjectLayout,
    provider: Option<&str>,
    cancel: &std::sync::atomic::AtomicBool,
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
    let (system, fell_back) =
        super::world_prompts::resolve(store, hierarchy, layout, "continuity", &language);
    if fell_back {
        progress(&format!("continuity extract: no {language} prompt — using English"));
    }

    let mut bible = ContinuityBible {
        version: env!("CARGO_PKG_VERSION").to_string(),
        language: language.clone(),
        facts: Vec::new(),
        manuscript_fingerprint: crate::world_report::manuscript_fingerprint(hierarchy),
    };

    for (idx, (chapter_id, chapter_title)) in chapters.iter().enumerate() {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(Error::Store("cancelled".into())); // partial bible not saved
        }
        let prose = crate::cli::book_walk::chapter_raw_prose(layout, hierarchy, *chapter_id);
        let plain = crate::audiobook::typst_to_plain(&prose);
        if plain.trim().is_empty() {
            continue;
        }
        progress(&format!("continuity [{}/{}] {chapter_title}", idx + 1, chapters.len()));
        let prompt = build_extract_prompt(&language, chapter_title, &plain);
        let raw = run_blocking(&ai, model, &system, &prompt)?;
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
         `Character | attribute_key | value`, no other output. \
         Keep `attribute_key` a short lowercase English identifier (eye_color, hometown) so it \
         matches across chapters; write `Character` and `value` in {language} as they appear in \
         the prose.\n\n\
         --- CHAPTER PROSE ---\n{prose}\n--- END ---",
    )
}

fn run_blocking(ai: &AiClient, model: &str, system: &str, prompt: &str) -> Result<String> {
    crate::ai::stream::collect_blocking(
        ai.client.clone(),
        model.to_string(),
        Some(system.to_string()),
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

