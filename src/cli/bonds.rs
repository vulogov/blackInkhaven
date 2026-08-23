//! BONDS-1 (BD-P4) — `inkhaven bonds`.
//!
//! Runs the deterministic relationship-continuity check (declared `rel:` bonds
//! vs. the scenes that earn them) and prints the findings (human or `--json`).
//! `--deep` adds the opt-in, cost-capped LLM `implied_cooling` pass. Exits
//! non-zero when any hard break survives (`unearned_shift`) — a CI gate, like
//! `knowledge` / `continuity check`. Mirrors [`crate::cli::knowledge`].

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::ken::Severity;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;

pub fn run(
    project: &Path,
    book_name: Option<&str>,
    json: bool,
    deep: bool,
    max_cost: usize,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let book = crate::cli::resolve_user_book(&h, book_name, "bonds").map_err(Error::Store)?;

    let mut findings = crate::bonds::check::run(&layout, &h, &cfg, book);
    // The opt-in, cost-capped LLM pass for the subtle (undeclared) shifts.
    if deep {
        eprintln!("bonds: running the LLM implied-cooling pass…");
        findings.extend(crate::bonds::deep::run(project, book_name, max_cost, false).map_err(Error::Store)?);
    }
    let breaks = findings.iter().filter(|f| f.severity == Severity::Break).count();

    if json {
        let rows: Vec<serde_json::Value> = findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "kind": f.kind,
                    "severity": f.severity.label(),
                    "chapter": f.chapter,
                    "anchor": f.anchor.map(|a| a.to_string()),
                    "a": f.a,
                    "b": f.b,
                    "message": f.message,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".into()));
    } else if findings.is_empty() {
        println!("\u{2713} no relationship breaks — every declared bond is earned on the page.");
    } else {
        for f in &findings {
            let icon = match f.severity {
                Severity::Break => "\u{2297}",  // ⊗
                Severity::Notice => "\u{25cf}", // ●
                Severity::Info => "\u{b7}",     // ·
            };
            println!("{icon} [{}] {}", f.kind, f.message);
        }
        println!(
            "\n{} finding(s): {breaks} break(s), {} other.",
            findings.len(),
            findings.len() - breaks
        );
    }

    if breaks > 0 {
        return Err(Error::Store(format!("{breaks} relationship break(s) — see above")));
    }
    Ok(())
}
