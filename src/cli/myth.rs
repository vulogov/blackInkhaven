//! MYTH-1 (M-P10) — `inkhaven myth …` commands.
//!
//! - `scan`     — refresh + deterministic scan; print the heatmap + findings.
//! - `check`    — run the LLM checks (+ deterministic); exit 1 on any finding.
//! - `profile`  — print the declared inventory (symbols / motifs / archetypes).
//! - `refresh`  — force recomputation of the deterministic caches.
//! - `suppress` — mute a finding by id.
//!
//! Reads the **declared** Mythology book only — never interprets, never edits
//! prose. Defaults here mirror the `myth:` config block (wired in M-P15).

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::myth::MythStore;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::Store;

use super::MythCommand;

pub fn run(project: &Path, cmd: MythCommand) -> Result<()> {
    match cmd {
        MythCommand::Scan { book, force, json } => scan(project, book.as_deref(), force, json),
        MythCommand::Check { book, kind, json } => check(project, book.as_deref(), kind.as_deref(), json),
        MythCommand::Profile { book, json } => profile(project, book.as_deref(), json),
        MythCommand::Refresh { book } => refresh(project, book.as_deref()),
        MythCommand::Suppress { finding, book } => suppress(project, &finding, book.as_deref()),
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

fn mstore(store: &Store) -> Result<MythStore> {
    MythStore::open(store.project_root()).map_err(|e| Error::Store(e.to_string()))
}

fn resolve<'a>(h: &'a Hierarchy, book: Option<&str>) -> Result<&'a Node> {
    super::resolve_user_book(h, book, "myth").map_err(Error::Store)
}

/// `scan` — refresh the inventory + deterministic scans, print the heatmap and
/// the deterministic findings. Zero-AI.
fn scan(project: &Path, book: Option<&str>, force: bool, json: bool) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let b = resolve(&h, book)?.clone();
    let ms = mstore(&store)?;
    let (_n, heatmap) = crate::myth::run_full_scan(
        &ms,
        &layout,
        &h,
        &b,
        cfg.myth.heatmap_buckets.max(1),
        cfg.myth.final_act_pct,
        force,
    )
    .map_err(|e| Error::Store(e.to_string()))?;
    let findings = ms.findings_with_ids(&b.slug, false).map_err(|e| Error::Store(e.to_string()))?;

    if json {
        println!("{}", findings_json(&findings));
        return Ok(());
    }
    println!("{heatmap}");
    print_findings(&findings);
    Ok(())
}

/// `check` — run the LLM checks (per `--kind`) plus the deterministic checks,
/// then print every finding and exit 1 if any are unsuppressed.
fn check(project: &Path, book: Option<&str>, kind: Option<&str>, json: bool) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let b = resolve(&h, book)?.clone();
    let ms = mstore(&store)?;
    let which = kind.unwrap_or("all").trim().to_lowercase();

    // The deterministic caches the LLM passes read from.
    crate::myth::refresh_inventory(&ms, &layout, &h, &b).map_err(|e| Error::Store(e.to_string()))?;
    crate::myth::run_density_scan(&ms, &layout, &h, &b, false).map_err(|e| Error::Store(e.to_string()))?;
    crate::myth::collect_explicit_motifs(&ms, &layout, &h, &b).map_err(|e| Error::Store(e.to_string()))?;

    if which == "all" || which == "deterministic" {
        crate::myth::run_deterministic_checks(&ms, &layout, &h, &b, cfg.myth.final_act_pct)
            .map_err(|e| Error::Store(e.to_string()))?;
    }
    if matches!(which.as_str(), "all" | "symbol" | "motif" | "archetype") {
        // The LLM pass runs all three; for a single-kind request we still run it
        // and the unrelated finding types simply come back empty / unchanged.
        crate::myth::run_llm_checks(
            &ms,
            &layout,
            &h,
            &cfg,
            &b,
            cfg.myth.consistency_min_chapters,
            cfg.myth.motif_min_occurrences,
        )
        .map_err(|e| Error::Store(e.to_string()))?;
    }

    let findings = ms.findings_with_ids(&b.slug, false).map_err(|e| Error::Store(e.to_string()))?;
    if json {
        println!("{}", findings_json(&findings));
    } else if findings.is_empty() {
        println!("myth check: no findings ✓");
    } else {
        println!("myth check: {} finding(s)\n", findings.len());
        print_findings(&findings);
    }
    if !findings.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}

/// `profile` — print the declared inventory.
fn profile(project: &Path, book: Option<&str>, json: bool) -> Result<()> {
    let (layout, _cfg, store, h) = open(project)?;
    let b = resolve(&h, book)?.clone();
    let ms = mstore(&store)?;
    crate::myth::refresh_inventory(&ms, &layout, &h, &b).map_err(|e| Error::Store(e.to_string()))?;
    let symbols = ms.symbols(&b.slug).map_err(|e| Error::Store(e.to_string()))?;
    let motifs = ms.motifs(&b.slug).map_err(|e| Error::Store(e.to_string()))?;
    let archetypes = ms.archetypes(&b.slug).map_err(|e| Error::Store(e.to_string()))?;

    if json {
        let v = serde_json::json!({
            "symbols": symbols.iter().map(|s| serde_json::json!({
                "vocabulary": s.vocabulary, "meaning": s.meaning,
                "valence": s.valence.as_code(), "traditions": s.traditions,
            })).collect::<Vec<_>>(),
            "motifs": motifs.iter().map(|m| serde_json::json!({
                "name": m.name, "description": m.description, "valence": m.valence.as_code(),
            })).collect::<Vec<_>>(),
            "archetypes": archetypes.iter().map(|a| serde_json::json!({
                "role": a.role.as_code(), "character": a.character_name, "function": a.function_desc,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        return Ok(());
    }

    println!("Mythology of `{}`\n", b.title);
    println!("⊛ symbols ({})", symbols.len());
    for s in &symbols {
        println!(
            "  {} — {} [{}]{}",
            s.vocabulary.join(", "),
            s.meaning,
            s.valence.as_code(),
            if s.traditions.is_empty() { String::new() } else { format!(" · {}", s.traditions.join(", ")) }
        );
    }
    println!("\n∿ motifs ({})", motifs.len());
    for m in &motifs {
        println!("  {} — {} [{}]", m.name, m.description, m.valence.as_code());
    }
    println!("\n⍟ archetypes ({})", archetypes.len());
    for a in &archetypes {
        let who = if a.character_name.trim().is_empty() { "—" } else { a.character_name.trim() };
        println!("  {} → {} — {}", a.role.as_code(), who, a.function_desc);
    }
    Ok(())
}

/// `refresh` — force a full deterministic recompute.
fn refresh(project: &Path, book: Option<&str>) -> Result<()> {
    let (layout, _cfg, store, h) = open(project)?;
    let b = resolve(&h, book)?.clone();
    let ms = mstore(&store)?;
    crate::myth::refresh_inventory(&ms, &layout, &h, &b).map_err(|e| Error::Store(e.to_string()))?;
    let cells = crate::myth::run_density_scan(&ms, &layout, &h, &b, true)
        .map_err(|e| Error::Store(e.to_string()))?;
    let occ = crate::myth::collect_explicit_motifs(&ms, &layout, &h, &b)
        .map_err(|e| Error::Store(e.to_string()))?;
    println!("myth refresh: {cells} density cell(s), {occ} explicit motif occurrence(s)");
    Ok(())
}

/// `suppress` — mute a finding by id.
fn suppress(project: &Path, finding: &str, book: Option<&str>) -> Result<()> {
    let (_layout, _cfg, store, h) = open(project)?;
    let b = resolve(&h, book)?.clone();
    let ms = mstore(&store)?;
    let ok = ms.suppress_finding(&b.slug, finding).map_err(|e| Error::Store(e.to_string()))?;
    if ok {
        println!("suppressed `{finding}`");
        Ok(())
    } else {
        Err(Error::Store(format!("no active finding with id `{finding}`")))
    }
}

fn print_findings(findings: &[(String, crate::myth::MythFinding)]) {
    if findings.is_empty() {
        println!("(no findings)");
        return;
    }
    for (id, f) in findings {
        println!("• [{}] {}", f.finding_type.label(), f.description);
        if let Some(ev) = &f.evidence {
            println!("    evidence: {ev}");
        }
        println!("    id: {id}  (suppress: inkhaven myth suppress --finding {id})");
    }
}

fn findings_json(findings: &[(String, crate::myth::MythFinding)]) -> String {
    let arr: Vec<serde_json::Value> = findings
        .iter()
        .map(|(id, f)| {
            serde_json::json!({
                "id": id,
                "type": f.finding_type.as_code(),
                "description": f.description,
                "evidence": f.evidence,
                "entry_para_id": f.entry_para_id,
                "chapter_ord": f.chapter_ord,
            })
        })
        .collect();
    serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".into())
}
