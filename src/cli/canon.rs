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

use crate::canon::CanonView;
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

fn print_view(v: &CanonView) {
    let kind = v
        .kind
        .map(|k| k.schema_str().trim_start_matches("x.narrative/").to_string())
        .unwrap_or_else(|| "?".to_string());
    let loc = v.locator.as_deref().unwrap_or("");
    let loc = if loc.is_empty() {
        String::new()
    } else {
        format!("  ({loc})")
    };
    println!("  {}  [{kind}]  {}{loc}", v.uid.short(), v.gist);
}
