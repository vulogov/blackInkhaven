//! CL-P3 — the `inkhaven canon` CLI: run the ledger's read-side queries.
//!
//!   inkhaven canon list                 — the recorded canon decisions
//!   inkhaven canon impact <id>          — what breaks if a decision is cut
//!   inkhaven canon why <id>             — the grounds a decision rests on
//!
//! `<id>` is a canonical or short id prefix as printed by `list` (resolved to a
//! unique decision). The ledger is populated deterministically from authored
//! tags on save (CL-P2); LLM harvest of prose is a later, opt-in phase.

use std::path::Path;

use smysl::Commitment;

use crate::canon::{CanonView, CommitmentForkView, CommitmentWarning, MergeSummary, PackedContext};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;

fn open(project: &Path) -> Result<Store> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    Store::open(layout, &cfg)
}

fn store_err(e: anyhow::Error) -> Error {
    Error::Store(e.to_string())
}

/// `inkhaven canon list`
pub fn list(project: &Path) -> Result<()> {
    let store = open(project)?;
    let decisions = store.raw().canon().all_decisions().map_err(store_err)?;
    if decisions.is_empty() {
        eprintln!(
            "No canon decisions recorded yet. They are harvested from authored tags \
             (e.g. `rel:<kind>:<A>:<B>`) when a paragraph is saved."
        );
        return Ok(());
    }
    eprintln!("{} canon decision(s):", decisions.len());
    for v in &decisions {
        print_view(v);
    }
    Ok(())
}

/// `inkhaven canon impact <id>`
pub fn impact(project: &Path, id: &str) -> Result<()> {
    let store = open(project)?;
    let canon = store.raw().canon();
    let uid = canon.resolve(id).map_err(store_err)?;
    let target = canon.view(uid).map_err(store_err)?;
    if let Some(t) = &target {
        eprint!("If cut: ");
        print_view(t);
    }
    let hits = canon.impact(uid).map_err(store_err)?;
    if hits.is_empty() {
        eprintln!("  nothing else rests on it — safe to cut.");
        return Ok(());
    }
    eprintln!("  {} decision(s) would dangle:", hits.len());
    for v in &hits {
        print_view(v);
    }
    Ok(())
}

/// `inkhaven canon why <id>`
pub fn why(project: &Path, id: &str) -> Result<()> {
    let store = open(project)?;
    let canon = store.raw().canon();
    let uid = canon.resolve(id).map_err(store_err)?;
    if let Some(t) = canon.view(uid).map_err(store_err)? {
        eprint!("Decision: ");
        print_view(&t);
    }
    let grounds = canon.why(uid).map_err(store_err)?;
    if grounds.is_empty() {
        eprintln!("  rests on nothing recorded — a base decision.");
        return Ok(());
    }
    eprintln!("  rests on {} decision(s):", grounds.len());
    for v in &grounds {
        print_view(v);
    }
    Ok(())
}

/// `inkhaven canon commit <id> --level <level> [--as <agent>]`
pub fn commit(project: &Path, id: &str, level: &str, agent: Option<String>) -> Result<()> {
    let store = open(project)?;
    let canon = store.raw().canon();
    let uid = canon.resolve(id).map_err(store_err)?;
    let level = Commitment::parse(level).ok_or_else(|| {
        let valid: Vec<String> = Commitment::ALL.iter().map(|c| c.to_string()).collect();
        Error::Store(format!("unknown commitment level {level:?} — valid: {}", valid.join(", ")))
    })?;
    let agent = agent.unwrap_or_else(|| "author".to_string());
    canon.commit(uid, level, &agent).map_err(store_err)?;
    eprintln!("committed {} → {level}", uid.short());
    Ok(())
}

/// `inkhaven canon check` — the SMY-W057 advisory.
pub fn check(project: &Path) -> Result<()> {
    let store = open(project)?;
    let warnings: Vec<CommitmentWarning> =
        store.raw().canon().commitment_warnings().map_err(store_err)?;
    if warnings.is_empty() {
        eprintln!("No commitment issues — nothing is committed above what it rests on (SMY-W057).");
        return Ok(());
    }
    eprintln!(
        "{} decision(s) committed above their foundation (SMY-W057 — \"built on sand\"):",
        warnings.len()
    );
    for w in &warnings {
        println!(
            "  {} [{}]  rests on  {} [{}]",
            w.unit.uid.short(),
            w.level,
            w.weakest_ground.uid.short(),
            w.ground_level
        );
        println!("      {}", w.unit.gist);
        println!("        ⟵ needs: {}", w.weakest_ground.gist);
    }
    Ok(())
}

/// `inkhaven canon merge <path>`
pub fn merge(project: &Path, other: &str) -> Result<()> {
    let store = open(project)?;
    let summary: MergeSummary = store.raw().canon().merge_from(other).map_err(store_err)?;
    eprintln!(
        "merged {other}: {} added, {} duplicate(s), {} commitment fork(s)",
        summary.added, summary.duplicates, summary.forks
    );
    if summary.forks > 0 {
        eprintln!("  run `inkhaven canon forks` to review them.");
    }
    Ok(())
}

/// `inkhaven canon forks`
pub fn forks(project: &Path) -> Result<()> {
    let store = open(project)?;
    let forks: Vec<CommitmentForkView> = store.raw().canon().commitment_forks().map_err(store_err)?;
    if forks.is_empty() {
        eprintln!("No commitment forks — no decision has agents disagreeing on how settled it is.");
        return Ok(());
    }
    eprintln!(
        "{} commitment fork(s) (SMY-W058 — agents disagree on canonicity):",
        forks.len()
    );
    for f in &forks {
        let status = if f.resolved { "  (resolved)" } else { "" };
        println!("  {}  {}{status}", f.unit.uid.short(), f.unit.gist);
        for (agent, level) in &f.positions {
            println!("      {agent} → {level}");
        }
    }
    Ok(())
}

/// `inkhaven canon context <query> [--budget N] [--reserve N] [--limit N]`
pub fn context(project: &Path, query: &str, limit: usize, budget: usize, reserve: usize) -> Result<()> {
    let store = open(project)?;
    let ctx: PackedContext = store
        .raw()
        .canon_context_for_query(query, limit, budget, reserve)
        .map_err(store_err)?;
    if ctx.views.is_empty() {
        eprintln!("No canon context for {query:?} (empty ledger, or nothing relevant).");
        return Ok(());
    }
    eprintln!(
        "canon context for {query:?} — {} decision(s), {}/{} tokens ({} reserved, {} dropped):",
        ctx.views.len(),
        ctx.used,
        ctx.budget,
        ctx.reserved,
        ctx.dropped
    );
    print!("{}", ctx.to_prompt());
    Ok(())
}

fn print_view(v: &CanonView) {
    let kind = v
        .kind
        .map(|k| k.schema_str().trim_start_matches("x.narrative/").to_string())
        .unwrap_or_else(|| "?".to_string());
    let commitment = v
        .commitment
        .map(|c| format!(" «{c}»"))
        .unwrap_or_default();
    let loc = v.locator.as_deref().unwrap_or("");
    let loc = if loc.is_empty() {
        String::new()
    } else {
        format!("  ({loc})")
    };
    println!("  {}  [{kind}]{commitment}  {}{loc}", v.uid.short(), v.gist);
}
