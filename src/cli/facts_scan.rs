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
use std::io::{BufRead, Write};
use std::path::Path;

use rust_stemmers::Stemmer;
use uuid::Uuid;

use crate::ai::AiClient;
use crate::config::{Config, parse_stemmer_language};
use crate::error::{Error, Result};
use crate::facts_scan::{
    FactScanReport, near_duplicate, normalise_tokens, parse_candidates, parse_findings,
};
use crate::project::ProjectLayout;
use crate::store::InsertPosition;
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

const EXTRACT_SYSTEM_PROMPT: &str = "You extract ESTABLISHED world facts — the invariants a \
story relies on: climate, geography, seasons, distances / travel-times, chronology / dates, \
and recurring rules (magic, technology, law, custom). You do NOT extract plot events, \
character actions, emotions, dialogue, or one-off details. Output ONE fact per line in the \
exact form:\n\
  category | statement\n\
where category is one of: climate, geography, seasons, distances, chronology, culture, rules, \
other; and statement is a short, self-contained factual sentence. Output nothing else — no \
preamble, no commentary, no markdown. If a chapter establishes no durable world facts, output \
nothing.";

pub fn run(project: &Path, cmd: FactsCommand) -> Result<()> {
    match cmd {
        FactsCommand::Scan { provider, json } => scan(project, provider.as_deref(), json),
        FactsCommand::Check { provider, json } => check(project, provider.as_deref(), json),
        FactsCommand::Import { from, yes } => import(project, from.as_deref(), yes),
        FactsCommand::List { json } => list(project, json),
        FactsCommand::Extract {
            provider,
            yes,
            dry_run,
        } => extract(project, provider.as_deref(), yes, dry_run),
        FactsCommand::Init { force, genre } => {
            init(project, force, genre.as_deref())
        }
    }
}

/// 1.2.21+ FF.4c — starter skeleton for the Facts book: the categories
/// a world's invariants usually fall into, each a paragraph seeded with
/// a one-line prompt the author replaces.  Turns an empty Facts book
/// into fill-in-the-blanks.  The general set applies to every project;
/// `--genre` appends genre-specific categories (FF.4c genre-aware).
const GENERAL_SKELETON: &[(&str, &str)] = &[
    (
        "Climate",
        "Temperature bands, rainfall / monsoon, extremes — and what's impossible here (snow? drought?).",
    ),
    (
        "Geography",
        "Key places and the distances / travel-times between them; terrain, borders, the capital.",
    ),
    (
        "Seasons",
        "The seasonal cycle: names, lengths, and what each season brings.",
    ),
    (
        "Chronology",
        "The calendar and fixed dates: when the story sits, and what happened how long ago.",
    ),
    (
        "Culture",
        "Customs, social structure, religion, and language facts the prose relies on.",
    ),
    (
        "Rules",
        "The hard limits of the world — magic / technology / law the plot can't break.",
    ),
];

/// The genre-specific categories appended to the general set by
/// `--genre`.  Each is the genre's recurring continuity hot-spots.
fn genre_extras(genre: &str) -> Option<&'static [(&'static str, &'static str)]> {
    match genre.trim().to_lowercase().as_str() {
        "" | "general" => Some(&[]),
        "fantasy" => Some(&[
            (
                "Magic system",
                "What magic can and can't do, who can use it, and its costs / limits — the rules a clever reader will hold you to.",
            ),
            (
                "Cosmology & religion",
                "Gods, planes, afterlife, prophecy — the metaphysics the plot leans on.",
            ),
            (
                "Peoples & lineages",
                "The races / peoples / bloodlines, their traits, lifespans, and relations.",
            ),
        ]),
        "scifi" | "sci-fi" | "science-fiction" => Some(&[
            (
                "Technology",
                "The tech baseline: what exists, what doesn't, and the hard limits (energy, comms, weapons).",
            ),
            (
                "Physics & travel",
                "FTL / sublight rules, travel-times between worlds, gravity, and any bent physics — kept consistent.",
            ),
            (
                "Polity & factions",
                "Governments, corporations, alliances, and who controls what.",
            ),
        ]),
        "mystery" => Some(&[
            (
                "Timeline of events",
                "The true sequence of what happened, with times — the spine the clues must fit.",
            ),
            (
                "Suspects & alibis",
                "Each suspect's whereabouts, motive, and means — what's verifiably true.",
            ),
            (
                "Physical evidence",
                "The clues, where they were, and what they actually prove (fair-play: no cheating).",
            ),
        ]),
        "historical" => Some(&[
            (
                "Period & setting",
                "The era's material facts: dress, technology, money, travel, daily life.",
            ),
            (
                "Real events & figures",
                "The historical events / people the story touches, and their real dates.",
            ),
            (
                "Anachronism watch",
                "Words, objects, and ideas that would be out of period — the things to never let slip in.",
            ),
        ]),
        _ => None,
    }
}

/// Genres `--genre` accepts (for help + error messages).
const GENRES: &[&str] = &["general", "fantasy", "scifi", "mystery", "historical"];

/// `inkhaven facts init` — scaffold the starter category paragraphs in
/// the Facts book.  Idempotent: a category already present is left
/// untouched unless `--force` adds a second copy.  `--genre` appends
/// genre-specific categories to the general set.
fn init(project: &Path, force: bool, genre: Option<&str>) -> Result<()> {
    let genre = genre.unwrap_or("general");
    let Some(extras) = genre_extras(genre) else {
        return Err(Error::Store(format!(
            "facts init: unknown genre `{genre}` — try one of: {}",
            GENRES.join(", "),
        )));
    };
    let skeleton: Vec<(&str, &str)> =
        GENERAL_SKELETON.iter().chain(extras.iter()).copied().collect();

    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let hierarchy = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;

    let Some(facts_id) = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_FACTS)
        })
        .map(|n| n.id)
    else {
        return Err(Error::Store(
            "facts init: this project has no Facts book".into(),
        ));
    };

    // Titles already present under Facts (case-insensitive), for idempotence.
    let existing: HashSet<String> = hierarchy
        .collect_subtree(facts_id)
        .into_iter()
        .filter_map(|id| hierarchy.get(id))
        .filter(|n| n.kind == NodeKind::Paragraph)
        .map(|n| n.title.trim().to_lowercase())
        .collect();

    let mut added = 0usize;
    let mut skipped = 0usize;
    for (title, hint) in &skeleton {
        if !force && existing.contains(&title.to_lowercase()) {
            skipped += 1;
            continue;
        }
        // Fresh hierarchy each create for correct sibling ordering.
        let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
        let facts_node = h
            .iter()
            .find(|n| n.id == facts_id)
            .cloned()
            .ok_or_else(|| Error::Store("facts init: Facts book vanished".into()))?;
        let mut node = store.create_node(
            &cfg,
            &h,
            NodeKind::Paragraph,
            title,
            Some(&facts_node),
            None,
            InsertPosition::End,
        )?;
        let body = format!("= {title}\n\n{hint}\n");
        if let Some(rel) = &node.file {
            let abs = store.project_root().join(rel);
            std::fs::write(&abs, body.as_bytes()).map_err(Error::Io)?;
            store.update_paragraph_content(&mut node, body.as_bytes())?;
        }
        added += 1;
    }

    println!(
        "facts init: added {added} categor{}{} to the Facts book",
        if added == 1 { "y" } else { "ies" },
        if skipped > 0 {
            format!(" ({skipped} already present, kept)")
        } else {
            String::new()
        },
    );
    Ok(())
}

/// 1.2.21+ FF.3 — propose world-facts from the manuscript and (after an
/// interactive accept) add them to the Facts book.  Solves the
/// cold-start: an empty Facts book does nothing until it's populated.
/// Mirrors `continuity extract`'s AI walk; the new parts are dedup
/// against the existing Facts entries and writing accepted candidates
/// in as paragraphs.
fn extract(project: &Path, provider: Option<&str>, yes: bool, dry_run: bool) -> Result<()> {
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

    // The Facts book must exist (we add into it) — but it may be EMPTY
    // (that's the whole point of cold-start extraction).
    let Some(facts_id) = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_FACTS)
        })
        .map(|n| n.id)
    else {
        return Err(Error::Store(
            "facts extract: this project has no Facts book".into(),
        ));
    };
    let facts_ids: HashSet<Uuid> = hierarchy.collect_subtree(facts_id).into_iter().collect();

    let chapters = hierarchy.user_book_chapters();
    if chapters.is_empty() {
        return Err(Error::Store(
            "facts extract: no user-book chapters found".into(),
        ));
    }

    // Existing facts (normalised token sets) for dedup.
    let stemmer: Option<Stemmer> = parse_stemmer_language(&cfg.language).map(Stemmer::create);
    let mut existing_sets: Vec<std::collections::BTreeSet<String>> = Vec::new();
    for &id in &facts_ids {
        let Some(node) = hierarchy.get(id) else {
            continue;
        };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        if let Ok(Some(bytes)) = store.get_content(id) {
            let body = crate::audiobook::typst_to_plain(&String::from_utf8_lossy(&bytes));
            existing_sets.push(normalise_tokens(&body, &stemmer));
        }
    }

    let ai = AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;

    eprintln!(
        "inkhaven facts extract · language: {language} · model: {model} · {} chapter(s)",
        chapters.len(),
    );

    // Walk + extract candidates, deduping against existing facts and
    // against candidates already kept (two chapters may state the same).
    let mut kept: Vec<crate::facts_scan::FactCandidate> = Vec::new();
    let mut kept_sets: Vec<std::collections::BTreeSet<String>> = Vec::new();
    for (idx, (chapter_id, chapter_title)) in chapters.iter().enumerate() {
        let prose = crate::cli::book_walk::chapter_raw_prose(&layout, &hierarchy, *chapter_id);
        let plain = crate::audiobook::typst_to_plain(&prose);
        if plain.trim().is_empty() {
            continue;
        }
        eprint!("  [{}/{}] {chapter_title} ", idx + 1, chapters.len());
        let prompt = build_extract_prompt(&language, chapter_title, &plain);
        let raw = run_blocking(&ai, model, EXTRACT_SYSTEM_PROMPT, &prompt)?;
        let candidates = parse_candidates(&raw, chapter_title);
        let mut new_here = 0;
        for cand in candidates {
            let toks = normalise_tokens(&cand.statement, &stemmer);
            if toks.is_empty() {
                continue;
            }
            if existing_sets.iter().any(|e| near_duplicate(&toks, e, 0.7)) {
                continue;
            }
            if kept_sets.iter().any(|k| near_duplicate(&toks, k, 0.7)) {
                continue;
            }
            kept_sets.push(toks);
            kept.push(cand);
            new_here += 1;
        }
        eprintln!("→ {new_here} new candidate(s)");
    }

    if kept.is_empty() {
        println!("facts extract: no new world-facts proposed (the Facts book already covers what the prose establishes)");
        return Ok(());
    }

    if dry_run {
        println!(
            "facts extract: {} candidate(s) (dry run — nothing added):\n",
            kept.len()
        );
        for (i, c) in kept.iter().enumerate() {
            println!("  {:>3}. [{}] {}", i + 1, c.category, c.statement);
            println!("       (from: {})", c.chapter);
        }
        return Ok(());
    }

    // Interactive accept (y / N / a=all / q=quit), or --yes.
    let stdin = std::io::stdin();
    let mut accept_all = yes;
    let mut added = 0usize;
    'review: for (i, cand) in kept.iter().enumerate() {
        println!("\n[{}/{}] {} — {}", i + 1, kept.len(), cand.category, cand.statement);
        println!("       (established in: {})", cand.chapter);
        let accept = if accept_all {
            true
        } else {
            loop {
                print!("       add to Facts? [y/N/a=all/q=quit]: ");
                std::io::stdout().flush().ok();
                let mut line = String::new();
                stdin.lock().read_line(&mut line).map_err(Error::Io)?;
                match line.trim() {
                    "y" | "Y" => break true,
                    "" | "n" | "N" => break false,
                    "a" | "A" => {
                        accept_all = true;
                        break true;
                    }
                    "q" | "Q" => break 'review,
                    other => {
                        println!("       (didn't understand `{other}` — y, N, a, or q)");
                    }
                }
            }
        };
        if accept {
            add_fact(&store, &cfg, &layout, facts_id, cand)?;
            added += 1;
        }
    }

    println!(
        "\nfacts extract: added {added} fact(s) to the Facts book{}",
        if added < kept.len() {
            format!(" ({} left unaccepted)", kept.len() - added)
        } else {
            String::new()
        },
    );
    Ok(())
}

/// Create one Facts-book paragraph from an accepted candidate.  Reloads
/// the hierarchy first so sibling ordering is fresh (the immutable-
/// hierarchy contract), then writes `= Category\n\nstatement` as the
/// body.
fn add_fact(
    store: &Store,
    cfg: &Config,
    layout: &ProjectLayout,
    facts_id: Uuid,
    cand: &crate::facts_scan::FactCandidate,
) -> Result<()> {
    let h = Hierarchy::load(store).map_err(|e| Error::Store(e.to_string()))?;
    let facts_node = h
        .iter()
        .find(|n| n.id == facts_id)
        .cloned()
        .ok_or_else(|| Error::Store("facts extract: Facts book vanished".into()))?;
    let title = title_case(&cand.category);
    let mut node = store.create_node(
        cfg,
        &h,
        NodeKind::Paragraph,
        &title,
        Some(&facts_node),
        None,
        InsertPosition::End,
    )?;
    let body = format!("= {title}\n\n{}\n", cand.statement.trim());
    if let Some(rel) = &node.file {
        let abs = layout.root.join(rel);
        std::fs::write(&abs, body.as_bytes()).map_err(Error::Io)?;
        store.update_paragraph_content(&mut node, body.as_bytes())?;
    }
    Ok(())
}

/// Title-case a one-word category (`climate` → `Climate`).
fn title_case(s: &str) -> String {
    let s = s.trim();
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => "Fact".to_string(),
    }
}

fn build_extract_prompt(language: &str, chapter: &str, prose: &str) -> String {
    format!(
        "Language of the manuscript: {language}.\n\
         Extract the established world facts from this chapter (\"{chapter}\"). \
         One fact per line, `category | statement`, no other output.\n\n\
         --- CHAPTER PROSE ---\n{prose}\n--- END ---",
    )
}

fn scan(project: &Path, provider: Option<&str>, json: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let hierarchy = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let never = std::sync::atomic::AtomicBool::new(false);
    let report = scan_with(&store, &hierarchy, &cfg, &layout, provider, &never, &|s| eprintln!("{s}"))?;

    if json {
        let rendered = serde_json::to_string_pretty(&report)
            .map_err(|e| Error::Store(format!("facts_scan JSON: {e}")))?;
        println!("{rendered}");
    } else {
        println!(
            "facts scan: {} contradiction(s) across {} chapter(s) → {}",
            report.findings.len(),
            hierarchy.user_book_chapters().len(),
            FactScanReport::sidecar_path(&layout.root).display(),
        );
    }
    Ok(())
}

/// 1.3.12 DEEP-1 — the prose-vs-fact scan against an **already-open** store,
/// progress through `progress` (not stderr), returning the saved report. Reads
/// `cfg.language` so the per-chapter prompt stays in the manuscript's language.
/// Safe to call from the TUI background-refresh thread.
#[allow(clippy::too_many_arguments)]
pub fn scan_with(
    store: &Store,
    hierarchy: &Hierarchy,
    cfg: &Config,
    layout: &ProjectLayout,
    provider: Option<&str>,
    cancel: &std::sync::atomic::AtomicBool,
    progress: &dyn Fn(&str),
) -> Result<FactScanReport> {
    let language = if cfg.language.trim().is_empty() {
        "English".to_string()
    } else {
        cfg.language.clone()
    };

    // Cheap preconditions first (a Facts book + chapters), so a project with
    // nothing to check fails clearly without needing an API key.
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

    let chapters = hierarchy.user_book_chapters();
    if chapters.is_empty() {
        return Err(Error::Store("facts scan: no user-book chapters found".into()));
    }

    let ai = AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    progress(&format!(
        "facts scan · language: {language} · model: {model} · {} chapter(s) · {total_facts} fact(s)",
        chapters.len(),
    ));

    let mut report = FactScanReport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        language: language.clone(),
        findings: Vec::new(),
        manuscript_fingerprint: crate::world_report::manuscript_fingerprint(hierarchy),
    };

    for (idx, (chapter_id, chapter_title)) in chapters.iter().enumerate() {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(Error::Store("cancelled".into())); // partial report not saved
        }
        let prose = crate::cli::book_walk::chapter_raw_prose(layout, hierarchy, *chapter_id);
        let plain = crate::audiobook::typst_to_plain(&prose);
        if plain.trim().is_empty() {
            continue;
        }
        let facts_ctx = relevant_facts(store, hierarchy, &facts_ids, &plain, 12);
        if facts_ctx.is_empty() {
            progress(&format!("facts [{}/{}] {chapter_title} → no relevant facts", idx + 1, chapters.len()));
            continue;
        }
        let prompt = build_check_prompt(&language, chapter_title, &plain, &facts_ctx);
        let raw = run_blocking(&ai, model, SYSTEM_PROMPT, &prompt)?;
        let findings = parse_findings(&raw, chapter_title, idx);
        progress(&format!(
            "facts [{}/{}] {chapter_title} → {} contradiction(s)",
            idx + 1,
            chapters.len(),
            findings.len()
        ));
        report.findings.extend(findings);
    }

    report
        .save(&layout.root)
        .map_err(|e| Error::Store(format!("facts_scan save: {e}")))?;
    Ok(report)
}

const CHECK_SYSTEM_PROMPT: &str = "You are a world-consistency editor for a work of fiction. \
Below are a story world's established facts. Using your knowledge of planetary science, physics, \
astronomy, geology, hydrology, climate, ecology, biology, and human culture / history, find pairs \
of facts that CANNOT COEXIST — either they directly contradict, or one makes the other \
physically or causally impossible given how worlds actually work. Reason beyond the wording: a \
tidally-locked planet can't have an ordinary day–night cycle; a mild climate can't sit where the \
geography forces extremes; a harbour that freezes each winter can't have warm currents; a city's \
population can't exceed what its stated food / water supply sustains; a travel time can't fit the \
stated distance and terrain. Treat any explicitly-stated speculative rule (magic, invented \
physics, non-Earth biology) as AUTHORITATIVE — only flag what those rules don't already permit; \
do not flag mere deviation from Earth where the story has set its own rule. Output ONE problem \
per line as `fact A | fact B | why`, quoting each fact briefly; `why` names the domain reason \
(e.g. \"tidal locking\", \"thermohaline circulation\", \"carrying capacity\"). If everything is \
consistent, output nothing. No preamble, no header row.";

/// 1.3.8 WORLD-1 P0 — internal-consistency check: flag fact pairs that
/// contradict each other *within* the Facts book.
fn check(project: &Path, provider: Option<&str>, json: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let hierarchy = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let report = check_with(&store, &hierarchy, &cfg, &layout, provider, &|s| eprintln!("{s}"))?;

    if json {
        let rendered = serde_json::to_string_pretty(&report)
            .map_err(|e| Error::Store(format!("facts_check JSON: {e}")))?;
        println!("{rendered}");
    } else if report.conflicts.is_empty() {
        println!("facts check: ✓ no internal contradictions");
    } else {
        println!(
            "facts check: {} internal contradiction(s):",
            report.conflicts.len()
        );
        for c in &report.conflicts {
            println!("  ⚠ {}  ⟷  {}\n      ↳ {}", c.a, c.b, c.detail);
        }
        eprintln!("  (also surfaced in `inkhaven edit`)");
    }
    Ok(())
}

/// 1.3.12 DEEP-1 — the internal fact-consistency check against an
/// **already-open** store, progress through `progress` (not stderr), returning
/// the saved report. Reads `cfg.language` so the prompt stays in the
/// manuscript's language. Safe to call from the TUI background-refresh thread.
pub fn check_with(
    store: &Store,
    hierarchy: &Hierarchy,
    cfg: &Config,
    layout: &ProjectLayout,
    provider: Option<&str>,
    progress: &dyn Fn(&str),
) -> Result<crate::facts_scan::FactCheckReport> {
    let language = if cfg.language.trim().is_empty() {
        "English".to_string()
    } else {
        cfg.language.clone()
    };

    let Some(facts_id) = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_FACTS)
        })
        .map(|n| n.id)
    else {
        return Err(Error::Store("facts check: this project has no Facts book".into()));
    };
    let facts_ids: HashSet<Uuid> = hierarchy.collect_subtree(facts_id).into_iter().collect();
    let local_titles: HashSet<String> = facts_ids
        .iter()
        .filter_map(|id| hierarchy.get(*id))
        .filter(|n| n.kind == NodeKind::Paragraph)
        .map(|n| n.title.trim().to_lowercase())
        .collect();
    let mut facts = all_facts(store, hierarchy, &facts_ids);
    // Layer series-shared canon (local wins on a title clash).
    if let Some(dir) = cfg.facts.shared_path.as_deref() {
        let added: Vec<String> = crate::facts_scan::shared_facts(std::path::Path::new(dir))
            .into_iter()
            .filter(|(t, _)| !local_titles.contains(&t.to_lowercase()))
            .map(|(t, b)| format!("- {t} (shared): {b}"))
            .collect();
        if !added.is_empty() {
            progress(&format!("+ {} shared fact(s) from {dir}", added.len()));
            facts.extend(added);
            facts.sort();
        }
    }
    if facts.len() < 2 {
        return Err(Error::Store(
            "facts check: need at least two facts to compare — `inkhaven facts extract` first".into(),
        ));
    }

    let ai = AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    progress(&format!(
        "facts check · language: {language} · model: {model} · {} fact(s)",
        facts.len()
    ));

    let prompt = format!(
        "Language of the manuscript: {language}. Quote each fact verbatim, and write every `why` \
explanation in {language}.\n--- ESTABLISHED FACTS ---\n{}\n--- END ---",
        facts.join("\n")
    );
    let raw = run_blocking(&ai, model, CHECK_SYSTEM_PROMPT, &prompt)?;
    let report = crate::facts_scan::FactCheckReport {
        version: env!("CARGO_PKG_VERSION").to_string(),
        content_hash: crate::facts_scan::FactCheckReport::compute_hash(&facts),
        conflicts: crate::facts_scan::parse_conflicts(&raw),
        manuscript_fingerprint: crate::world_report::manuscript_fingerprint(hierarchy),
    };
    report
        .save(&layout.root)
        .map_err(|e| Error::Store(format!("facts_check save: {e}")))?;
    Ok(report)
}

/// Every Facts-book entry as a `- title: body` line, sorted for a stable
/// prompt + cache hash.
fn all_facts(store: &Store, hierarchy: &Hierarchy, facts_ids: &HashSet<Uuid>) -> Vec<String> {
    let mut out = Vec::new();
    for id in facts_ids {
        let Some(node) = hierarchy.get(*id) else {
            continue;
        };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        if let Ok(Some(bytes)) = store.get_content(*id) {
            let body = crate::audiobook::typst_to_plain(&String::from_utf8_lossy(&bytes));
            let body = body.trim();
            if !body.is_empty() {
                out.push(format!("- {}: {}", node.title.trim(), body));
            }
        }
    }
    out.sort();
    out
}

/// 1.3.8 WORLD-1 P3 — copy series-shared facts (a directory of plain-text
/// fact files) into this project's Facts book as paragraphs. A hard
/// snapshot: after import, `scan` / fact-check see them as local facts.
fn import(project: &Path, from: Option<&str>, yes: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let hierarchy = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;

    let dir = from
        .map(String::from)
        .or_else(|| cfg.facts.shared_path.clone())
        .ok_or_else(|| {
            Error::Store("facts import: pass --from <dir> or set `facts.shared_path`".into())
        })?;
    let shared = crate::facts_scan::shared_facts(std::path::Path::new(&dir));
    if shared.is_empty() {
        return Err(Error::Store(format!(
            "facts import: no fact files (.txt / .md / .typ) in {dir}"
        )));
    }

    let Some(facts_id) = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_FACTS)
        })
        .map(|n| n.id)
    else {
        return Err(Error::Store("facts import: this project has no Facts book".into()));
    };
    let existing: HashSet<String> = hierarchy
        .collect_subtree(facts_id)
        .into_iter()
        .filter_map(|id| hierarchy.get(id))
        .filter(|n| n.kind == NodeKind::Paragraph)
        .map(|n| n.title.trim().to_lowercase())
        .collect();
    let to_add: Vec<(String, String)> = shared
        .into_iter()
        .filter(|(t, _)| !existing.contains(&t.to_lowercase()))
        .collect();
    if to_add.is_empty() {
        println!("facts import: every shared fact is already in the Facts book");
        return Ok(());
    }
    if !yes {
        println!("facts import: would add {} fact(s) from {dir}:", to_add.len());
        for (t, _) in &to_add {
            println!("  + {t}");
        }
        eprintln!("  pass --yes to write them");
        return Ok(());
    }

    for (title, body) in &to_add {
        let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
        let facts_node = h
            .iter()
            .find(|n| n.id == facts_id)
            .cloned()
            .ok_or_else(|| Error::Store("facts import: Facts book vanished".into()))?;
        let mut node = store.create_node(
            &cfg,
            &h,
            NodeKind::Paragraph,
            title,
            Some(&facts_node),
            None,
            InsertPosition::End,
        )?;
        let para_body = format!("= {title}\n\n{}\n", body.trim());
        if let Some(rel) = &node.file {
            let abs = store.project_root().join(rel);
            std::fs::write(&abs, para_body.as_bytes()).map_err(Error::Io)?;
            store.update_paragraph_content(&mut node, para_body.as_bytes())?;
        }
    }
    println!(
        "facts import: added {} fact(s) from {dir} → the Facts book",
        to_add.len()
    );
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
         One contradiction per line, `claim | fact | detail`, no other output. \
         Quote `claim` and `fact` verbatim from the text; write each `detail` in {language}.\n\n\
         --- ESTABLISHED FACTS ---\n{}\n--- END ---\n\n\
         --- CHAPTER PROSE ---\n{prose}\n--- END ---",
        facts.join("\n"),
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

