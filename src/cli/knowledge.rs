//! KEN-1 (KEN-P4) — `inkhaven knowledge check`.
//!
//! Runs the deterministic epistemic check (who knows what, when) and prints the
//! findings (human or `--json`). Exits non-zero when any hard break survives
//! (`premature_knowledge` / `leaked_secret`) — a CI gate, like `continuity check`.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::ken::Severity;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;

pub fn run(project: &Path, book_name: Option<&str>, json: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let book = crate::cli::resolve_user_book(&h, book_name, "knowledge").map_err(Error::Store)?;

    let findings = crate::ken::check::run(&layout, &h, &cfg, book);
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
                    "character": f.character,
                    "topic": f.topic,
                    "message": f.message,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".into()));
    } else if findings.is_empty() {
        println!("\u{2713} no epistemic breaks — nobody knows what they shouldn't.");
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
        return Err(Error::Store(format!("{breaks} epistemic break(s) — see above")));
    }
    Ok(())
}
