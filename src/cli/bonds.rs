//! BONDS-1 (BD-P4) — `inkhaven bonds`.
//!
//! Runs the deterministic relationship-continuity check (declared `rel:` bonds
//! vs. the scenes that earn them) and prints the findings (human or `--json`).
//! `--deep` adds the opt-in, cost-capped LLM `implied_cooling` pass. `--ledger`
//! prints the declared-bond model instead (the BONDS analog of the knowledge
//! ledger). Exits non-zero when any hard break survives (`unearned_shift`) — a CI
//! gate, like `knowledge` / `continuity check`. Mirrors [`crate::cli::knowledge`].

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
    ledger: bool,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let book = crate::cli::resolve_user_book(&h, book_name, "bonds").map_err(Error::Store)?;

    // E1 — the bond ledger: not the breaks, but the declared `rel:` model BONDS
    // reasons over (who is bonded to whom, and when), the analog of the KEN ledger.
    if ledger {
        return print_ledger(&layout, &h, book, json);
    }

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

/// E1 — print the bond ledger: the declared `rel:` bonds (the model BONDS reasons
/// over) grouped by character pair, each state with the chapter it's declared in.
/// The BONDS analog of `knowledge --ledger`. Read-only.
fn print_ledger(
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &crate::store::node::Node,
    json: bool,
) -> Result<()> {
    let ties = crate::bonds::ties(layout, h, book);

    if json {
        let rows: Vec<serde_json::Value> = ties
            .iter()
            .map(|t| {
                serde_json::json!({
                    "a": t.a,
                    "b": t.b,
                    "kind": t.kind,
                    "chapter": t.at.chapter_ord,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".into()));
        return Ok(());
    }

    if ties.is_empty() {
        println!(
            "Bond ledger — empty. Declare bonds with `rel:<kind>:<A>:<B>` tags \
             (e.g. `rel:ally:mara:kell`)."
        );
        return Ok(());
    }

    print!("{}", ledger_lines(&ties));
    Ok(())
}

/// Pure — render the bond ledger's human lines: declared bonds grouped by
/// canonical pair, in reading order within a pair, each state `kind (ch. N)`.
fn ledger_lines(ties: &[crate::bonds::Declared]) -> String {
    use std::collections::BTreeMap;
    let mut by_pair: BTreeMap<(&str, &str), Vec<&crate::bonds::Declared>> = BTreeMap::new();
    for t in ties {
        by_pair.entry((t.a.as_str(), t.b.as_str())).or_default().push(t);
    }
    let mut out = String::from("Bond ledger — declared relationships, in reading order\n\n");
    for (pair, states) in &mut by_pair {
        states.sort_by(|x, y| x.at.cmp(&y.at));
        out.push_str(&format!("{} \u{2014} {}\n", pair.0, pair.1));
        for t in states {
            out.push_str(&format!("  {:<20} (ch. {})\n", t.kind, t.at.chapter_ord));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bonds::Declared;
    use crate::ken::ScenePos;
    use uuid::Uuid;

    #[test]
    fn ledger_groups_by_pair_in_reading_order() {
        // E1 — two states of the mara—kell bond, declared out of order, group under
        // one pair heading and sort by reading position.
        let mk = |kind: &str, ch: u32| {
            Declared::new(kind, "Kell", "Mara", ScenePos { chapter_ord: ch, ..Default::default() }, Uuid::nil())
        };
        let ties = vec![mk("rivals", 5), mk("allies", 2)];
        let out = ledger_lines(&ties);
        // One pair heading (canonical order), both states, allies (ch.2) before rivals (ch.5).
        assert_eq!(out.matches("Kell \u{2014} Mara").count(), 1, "one pair heading: {out}");
        let allies = out.find("allies").unwrap();
        let rivals = out.find("rivals").unwrap();
        assert!(allies < rivals, "reading order (ch.2 before ch.5): {out}");
        assert!(out.contains("(ch. 2)") && out.contains("(ch. 5)"), "{out}");
    }
}
