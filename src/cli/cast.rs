//! ENSEMBLE (EN-P4) — `inkhaven cast`: the book's Dramatis Personae.
//!
//! Prints the cast joined with their declared BONDS relationships and their
//! CHAR-1 arc state (human or `--json`). Read-only; deterministic. Mirrors
//! [`crate::cli::bonds`].

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;

pub fn run(project: &Path, book_name: Option<&str>, json: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let book = crate::cli::resolve_user_book(&h, book_name, "cast").map_err(Error::Store)?;

    let cast = crate::cast::build_cast(&layout, &h, &cfg, book);

    if json {
        let members: Vec<serde_json::Value> = cast
            .members
            .iter()
            .map(|m| {
                serde_json::json!({
                    "name": m.name,
                    "node": m.node.map(|n| n.to_string()),
                    "arc": m.arc.as_ref().map(|a| serde_json::json!({
                        "arc": a.arc_code,
                        "state": a.current_state,
                        "chapter": a.current_chapter,
                        "changes": a.changes,
                        "agency": a.latest_agency,
                    })),
                    "bonds": m.bonds.iter().map(|t| serde_json::json!({
                        "other": t.other, "kind": t.kind, "chapter": t.chapter,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let out = serde_json::json!({
            "book": cast.book,
            "members": members,
            "findings": cast.findings.iter().map(|f| serde_json::json!({
                "kind": f.kind, "severity": f.severity.label(), "a": f.a, "b": f.b, "message": f.message,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".into()));
        return Ok(());
    }

    println!("Dramatis Personae — {} ({} character(s))\n", cast.book, cast.members.len());
    if cast.members.is_empty() {
        println!("  (no cast — declare characters in the Characters book, or tag `rel:` bonds)");
        return Ok(());
    }
    for m in &cast.members {
        let arc = m
            .arc
            .as_ref()
            .map(|a| {
                let shape = a.arc_code.as_deref().unwrap_or("—");
                let state = a.current_state.as_deref().unwrap_or("—");
                let ch = if a.current_chapter > 0 { format!(" (ch. {})", a.current_chapter) } else { String::new() };
                format!("  [{shape} · {state}{ch} · ✦{}]", a.changes)
            })
            .unwrap_or_default();
        println!("{}{arc}", m.name);
        for t in &m.bonds {
            println!("  ⇄ {} — {} (ch. {})", t.other, t.kind, t.chapter);
        }
    }
    if !cast.findings.is_empty() {
        println!("\n{} bond finding(s) — see `inkhaven bonds`.", cast.findings.len());
    }
    Ok(())
}
