//! 1.2.21+ FF.2 — `inkhaven facts scan` subcommand.
//!
//! Walks each user-book chapter, semantically searches the Facts book
//! for the entries relevant to that chapter, and asks the LLM to flag
//! prose that contradicts them.  Findings →
//! `<project>/.inkhaven/facts_scan.json`.  `scan` runs the AI pass;
//! `list` re-dumps the last result.  `--json` emits the report for CI.
//!
//! A fact-check is semantic, so this is a standalone AI command (the
//! `continuity` / `tension` pattern), not a deterministic `doctor
//! --scan` ScanClass.  Facts are RAG-filtered per chapter so each
//! prompt stays small even for a large Facts book.

use std::collections::HashSet;
use std::path::Path;

use uuid::Uuid;

use crate::ai::AiClient;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::facts_scan::{FactScanReport, parse_findings};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;

use super::FactsCommand;

const SYSTEM_PROMPT: &str = "You are a fact-checker for a work of fiction. You receive a set \
of ESTABLISHED facts about the story's world (climate, geography, seasons, distances, \
chronology) and a chapter's prose. Flag any claim in the prose that CONTRADICTS an \
established fact — snow in a region established as tropical, a three-day ride done overnight, \
an event dated before something it must follow. Treat the established facts as ground truth; \
do not flag things merely unmentioned by them. Output ONE contradiction per line in the exact \
form:\n\
  claim | fact | detail\n\
where `claim` is the exact contradicting phrase from the prose, `fact` is the established \
fact it violates, and `detail` is a one-line explanation. Output nothing else — no preamble, \
no commentary, no markdown. If the chapter contradicts no facts, output nothing.";

pub fn run(project: &Path, cmd: FactsCommand) -> Result<()> {
    match cmd {
        FactsCommand::Scan { provider, json } => scan(project, provider.as_deref(), json),
        FactsCommand::List { json } => list(project, json),
    }
}

fn scan(project: &Path, provider: Option<&str>, json: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let hierarchy = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;

    let language = if cfg.language.trim().is_empty() {
        "English".to_string()
    } else {
        cfg.language.clone()
    };

    // Cheap preconditions first (a Facts book + chapters), so a project
    // with nothing to check fails clearly without needing an API key.
    // Resolve the Facts book + its paragraph subtree.
    let Some(facts_id) = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_FACTS)
        })
        .map(|n| n.id)
    else {
        return Err(Error::Store(
            "facts scan: this project has no Facts book".into(),
        ));
    };
    let facts_ids: HashSet<Uuid> = hierarchy.collect_subtree(facts_id).into_iter().collect();
    let total_facts = facts_ids
        .iter()
        .filter(|id| {
            hierarchy
                .get(**id)
                .map(|n| n.kind == NodeKind::Paragraph)
                .unwrap_or(false)
        })
        .count();
    if total_facts == 0 {
        return Err(Error::Store(
            "facts scan: the Facts book is empty — collect some facts first".into(),
        ));
    }

    let chapters = user_book_chapters(&hierarchy);
    if chapters.is_empty() {
        return Err(Error::Store(
            "facts scan: no user-book chapters found".into(),
        ));
    }

    // Preconditions met — now require the LLM.
    let ai = AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;

    eprintln!(
        "inkhaven facts scan · language: {language} · model: {model} · {} chapter(s) · {total_facts} fact(s)",
        chapters.len(),
    );

    let mut report = FactScanReport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        language: language.clone(),
        findings: Vec::new(),
    };

    for (idx, (chapter_id, chapter_title)) in chapters.iter().enumerate() {
        let prose = crate::cli::book_walk::chapter_raw_prose(&layout, &hierarchy, *chapter_id);
        let plain = crate::audiobook::typst_to_plain(&prose);
        if plain.trim().is_empty() {
            continue;
        }
        eprint!("  [{}/{}] {chapter_title} ", idx + 1, chapters.len());

        let facts_ctx = relevant_facts(&store, &hierarchy, &facts_ids, &plain, 12);
        if facts_ctx.is_empty() {
            eprintln!("→ no relevant facts");
            continue;
        }
        let prompt = build_check_prompt(&language, chapter_title, &plain, &facts_ctx);
        let raw = run_blocking(&ai, model, &prompt)?;
        let findings = parse_findings(&raw, chapter_title, idx);
        eprintln!("→ {} contradiction(s)", findings.len());
        report.findings.extend(findings);
    }

    report
        .save(&layout.root)
        .map_err(|e| Error::Store(format!("facts_scan save: {e}")))?;

    if json {
        let rendered = serde_json::to_string_pretty(&report)
            .map_err(|e| Error::Store(format!("facts_scan JSON: {e}")))?;
        println!("{rendered}");
    } else {
        println!(
            "facts scan: {} contradiction(s) across {} chapter(s) → {}",
            report.findings.len(),
            chapters.len(),
            FactScanReport::sidecar_path(&layout.root).display(),
        );
    }
    Ok(())
}

/// Semantically retrieve the Facts-book entries relevant to `prose`, as
/// plain-text bullet lines.  Whole-project `search_text` post-filtered
/// to the Facts subtree; full bodies read from the store and flattened
/// to plain text.  Keeps each chapter's prompt small even for a large
/// Facts book (the FF.1 RAG idea, applied to the scan).
fn relevant_facts(
    store: &Store,
    hierarchy: &Hierarchy,
    facts_ids: &HashSet<Uuid>,
    prose: &str,
    limit: usize,
) -> Vec<String> {
    // Cap the query — `search_text` embeds it, and a chapter's opening
    // establishes most of its setting anyway.
    let query: String = prose.chars().take(2000).collect();
    let raw = match store.search_text(&query, (limit + 12).max(24)) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for v in raw.iter() {
        let Some(id) = v
            .get("id")
            .and_then(|x| x.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            continue;
        };
        if !facts_ids.contains(&id) || !seen.insert(id) {
            continue;
        }
        let Some(node) = hierarchy.get(id) else {
            continue;
        };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        if let Ok(Some(bytes)) = store.get_content(id) {
            let body = crate::audiobook::typst_to_plain(&String::from_utf8_lossy(&bytes));
            let body = body.trim();
            if body.is_empty() {
                continue;
            }
            out.push(format!("- {}: {}", node.title.trim(), body));
        }
        if out.len() >= limit {
            break;
        }
    }
    out
}

fn build_check_prompt(language: &str, chapter: &str, prose: &str, facts: &[String]) -> String {
    format!(
        "Language of the manuscript: {language}.\n\
         Fact-check this chapter (\"{chapter}\") against the established facts below. \
         One contradiction per line, `claim | fact | detail`, no other output.\n\n\
         --- ESTABLISHED FACTS ---\n{}\n--- END ---\n\n\
         --- CHAPTER PROSE ---\n{prose}\n--- END ---",
        facts.join("\n"),
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

fn list(project: &Path, json: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let report = FactScanReport::load(&layout.root)
        .map_err(|e| Error::Store(format!("facts_scan load: {e}")))?;
    if json {
        let rendered = serde_json::to_string_pretty(&report)
            .map_err(|e| Error::Store(format!("facts_scan JSON: {e}")))?;
        println!("{rendered}");
        return Ok(());
    }
    if report.findings.is_empty() {
        println!("facts scan: no contradictions recorded — run `inkhaven facts scan` first");
        return Ok(());
    }
    println!(
        "Fact-check findings — {} contradiction(s), language {}\n",
        report.findings.len(),
        report.language,
    );
    let mut last = String::new();
    for f in &report.findings {
        if f.chapter != last {
            println!("{}", f.chapter);
            last = f.chapter.clone();
        }
        println!("  claim:  {}", f.claim);
        println!("  fact:   {}", f.fact);
        println!("  → {}\n", f.detail);
    }
    Ok(())
}

/// Top-level (non-system) book chapters in display order — mirrors the
/// `continuity` / `tension` walk.
fn user_book_chapters(h: &Hierarchy) -> Vec<(Uuid, String)> {
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
