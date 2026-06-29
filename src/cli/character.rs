//! CHAR-1 (C-P7) — `inkhaven character …` handlers. Character arc tracking from
//! the command line:
//!
//! - `arc <name>` — a read-only report (declaration · state chain · agency ·
//!   stalls · checks · planning gaps) over the cached `char.duckdb`.
//! - `refresh`    — recompute agency (deterministic) + re-extract the state
//!   chain (LLM, lazy) for declared characters.
//! - `check`      — run the arc-completeness checks (LLM); exit 1 on any gap or
//!   stall, 2 if the ending / earned-arc check fails (pre-submission gate).
//! - `plan`       — Planning-Board coverage gaps (deterministic); exit 1 on any.
//!
//! Findings are advisory; the cap informs, never blocks.

use std::path::Path;

use crate::character::{
    ArcCheckType, ArcDeclaration, CharStore, read_arc_declarations, run_agency, run_arc_checks,
    run_extraction, run_planning,
};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;

use super::CharacterCommand;

/// RFC §15 defaults until the `char:` config block lands (C-P10).
const STALL_THRESHOLD: u32 = 4;
const MIN_CHAPTERS: usize = 3;

pub fn run(project: &Path, cmd: CharacterCommand) -> Result<()> {
    match cmd {
        CharacterCommand::Arc { name, book } => arc(project, &name, book.as_deref()),
        CharacterCommand::Check { book, json } => check(project, book.as_deref(), json),
        CharacterCommand::Refresh { book, name } => {
            refresh(project, book.as_deref(), name.as_deref())
        }
        CharacterCommand::Plan { book, json } => plan(project, book.as_deref(), json),
    }
}

fn open(project: &Path) -> Result<(ProjectLayout, Config, Store, Hierarchy)> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;
    Ok((layout, cfg, store, h))
}

fn cstore(store: &Store) -> Result<CharStore> {
    CharStore::open(store.project_root()).map_err(se)
}

/// Map an `anyhow::Error` from the character module to the CLI error type.
fn se(e: anyhow::Error) -> Error {
    Error::Store(e.to_string())
}

/// Sync the author-declared arcs from the Characters book into `char.duckdb` so
/// the cached store reflects the current `character_arc` blocks.
fn sync_declarations(cs: &CharStore, h: &Hierarchy, layout: &ProjectLayout, book_slug: &str) -> Result<()> {
    for (decl, hash) in read_arc_declarations(h, layout) {
        cs.upsert_declaration(book_slug, &decl, hash).map_err(se)?;
    }
    Ok(())
}

fn fmt_opt_f32(v: Option<f32>) -> String {
    v.map(|x| format!("{x:.2}")).unwrap_or_else(|| "—".to_string())
}

// ── arc (read-only report) ────────────────────────────────────────────────────

fn arc(project: &Path, name: &str, book_name: Option<&str>) -> Result<()> {
    let (layout, _cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "character").map_err(Error::Store)?;
    let cs = cstore(&store)?;
    sync_declarations(&cs, &h, &layout, &book.slug)?;

    // Resolve the canonical roster spelling (case-insensitive).
    let roster = crate::character::character_names(&h);
    let canonical = roster
        .iter()
        .find(|n| n.eq_ignore_ascii_case(name))
        .cloned()
        .unwrap_or_else(|| name.to_string());

    let decl = cs.declaration(&book.slug, &canonical).map_err(se)?;
    let states = cs.states_for_character(&book.slug, &canonical).map_err(se)?;
    let checks = cs.checks_for_character(&book.slug, &canonical).map_err(se)?;
    let planning: Vec<_> = cs
        .planning_findings(&book.slug)
        .map_err(se)?
        .into_iter()
        .filter(|(n, _, _)| n.eq_ignore_ascii_case(&canonical))
        .collect();

    println!("Character arc — {canonical} · `{}`", book.title);
    println!("{}", "─".repeat(72));

    match &decl {
        Some(d) => {
            println!("Declared arc: {}", d.arc_type.as_code());
            println!("  start    : {}", d.desired_state_start);
            if let Some(m) = &d.desired_midpoint_state {
                println!("  midpoint : {m}");
            }
            println!("  end      : {}", d.desired_state_end);
        }
        None => println!("Declared arc: (none — add a `character_arc` block in the Characters book)"),
    }
    println!();

    if states.is_empty() {
        println!("State chain: (empty — run `inkhaven character refresh` to extract)");
    } else {
        println!("State chain ({} chapters):", states.len());
        for s in &states {
            let mark = if s.changed { "✦" } else { "·" };
            println!(
                "  {mark} ch.{:>2}  agency {}  ({}a/{}p)  {}",
                s.chapter_ord,
                fmt_opt_f32(s.agency_score),
                s.active_count,
                s.passive_count,
                s.state_summary,
            );
            if let Some(cd) = &s.change_description {
                println!("           change: {cd}");
            }
        }
    }
    println!();

    if checks.is_empty() {
        println!("Arc checks: (none — run `inkhaven character check`)");
    } else {
        println!("Arc checks:");
        for c in &checks {
            let loc = c.chapter_ord.map(|o| format!(" (ch.{o})")).unwrap_or_default();
            println!("  [{}] {}{loc} — {}", c.verdict.as_code(), c.check_type.label(), c.description);
        }
    }

    if !planning.is_empty() {
        println!();
        println!("Planning gaps:");
        for (_, ft, desc) in &planning {
            println!("  [{ft}] {desc}");
        }
    }
    println!("{}", "─".repeat(72));
    Ok(())
}

// ── refresh (recompute) ───────────────────────────────────────────────────────

fn refresh(project: &Path, book_name: Option<&str>, only: Option<&str>) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "character").map_err(Error::Store)?;
    let cs = cstore(&store)?;
    sync_declarations(&cs, &h, &layout, &book.slug)?;

    let cells = run_agency(&cs, &layout, &h, &cfg, book).map_err(se)?;
    println!("Agency: {cells} (character, chapter) cells computed.");

    let decls: Vec<ArcDeclaration> = cs
        .all_declarations(&book.slug)
        .map_err(se)?
        .into_iter()
        .filter(|d| only.is_none_or(|n| d.character_name.eq_ignore_ascii_case(n)))
        .collect();
    if decls.is_empty() {
        println!("No declared arcs to extract. (Add a `character_arc` block in the Characters book.)");
        return Ok(());
    }
    for d in &decls {
        let n = run_extraction(&cs, &cfg, &layout, &h, book, &d.character_name, Some(d)).map_err(se)?;
        println!("  {} — {n} chapter(s) (re)extracted.", d.character_name);
    }
    Ok(())
}

// ── check (LLM arc completeness + exit gate) ──────────────────────────────────

fn check(project: &Path, book_name: Option<&str>, json: bool) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "character").map_err(Error::Store)?;
    let cs = cstore(&store)?;
    sync_declarations(&cs, &h, &layout, &book.slug)?;

    // Keep agency fresh (cheap, deterministic) so stall averages are current.
    run_agency(&cs, &layout, &h, &cfg, book).map_err(se)?;

    let decls = cs.all_declarations(&book.slug).map_err(se)?;
    let mut all = Vec::new();
    for d in &decls {
        // Ensure the state chain exists (lazy; only re-extracts on edits).
        run_extraction(&cs, &cfg, &layout, &h, book, &d.character_name, Some(d)).map_err(se)?;
        let states = cs.states_for_character(&book.slug, &d.character_name).map_err(se)?;
        let checks = run_arc_checks(&cs, &cfg, &book.slug, d, &states, STALL_THRESHOLD, MIN_CHAPTERS)
            .map_err(se)?;
        all.extend(checks);
    }

    if json {
        let arr: Vec<serde_json::Value> = all
            .iter()
            .map(|c| {
                serde_json::json!({
                    "character": c.character_name,
                    "check": c.check_type.as_code(),
                    "verdict": c.verdict.as_code(),
                    "description": c.description,
                    "chapter": c.chapter_ord,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
    } else {
        println!("Character arc checks — `{}` [{}]", book.title, cfg.language);
        println!("{}", "─".repeat(72));
        if decls.is_empty() {
            println!("No declared arcs. (Add a `character_arc` block in the Characters book.)");
        }
        for c in &all {
            let loc = c.chapter_ord.map(|o| format!(" (ch.{o})")).unwrap_or_default();
            let flag = if c.verdict.is_problem() { "⚠" } else { " " };
            println!(
                "{flag} {} · [{}] {}{loc} — {}",
                c.character_name,
                c.verdict.as_code(),
                c.check_type.label(),
                loc_trim(&c.description),
            );
        }
        let problems = all.iter().filter(|c| c.verdict.is_problem()).count();
        println!("{}", "─".repeat(72));
        println!("{problems} problem(s) across {} check(s).", all.len());
    }

    // Exit gate: 2 if the ending or earned-arc fails, 1 on any other problem.
    let serious = all.iter().any(|c| {
        c.verdict.is_problem()
            && matches!(c.check_type, ArcCheckType::EndAlignment | ArcCheckType::ArcEarned)
    });
    let any_problem = all.iter().any(|c| c.verdict.is_problem());
    if serious {
        std::process::exit(2);
    }
    if any_problem {
        std::process::exit(1);
    }
    Ok(())
}

/// Collapse internal whitespace runs so a multi-line LLM description prints on
/// one tidy line.
fn loc_trim(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ── plan (deterministic Planning-Board gaps + exit gate) ──────────────────────

fn plan(project: &Path, book_name: Option<&str>, json: bool) -> Result<()> {
    let (layout, _cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "character").map_err(Error::Store)?;
    let cs = cstore(&store)?;
    sync_declarations(&cs, &h, &layout, &book.slug)?;

    run_planning(&cs, &store, &h, book).map_err(se)?;
    let findings = cs.planning_findings(&book.slug).map_err(se)?;

    if json {
        let arr: Vec<serde_json::Value> = findings
            .iter()
            .map(|(name, ft, desc)| {
                serde_json::json!({ "character": name, "type": ft, "description": desc })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
    } else {
        println!("Planning-Board arc gaps — `{}`", book.title);
        println!("{}", "─".repeat(72));
        if findings.is_empty() {
            println!("No planning gaps. Every declared arc has scene-card coverage through the book.");
        }
        for (name, ft, desc) in &findings {
            println!("  {name} · [{ft}] {desc}");
        }
    }

    if !findings.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}
