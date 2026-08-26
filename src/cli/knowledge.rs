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
    let book = crate::cli::resolve_user_book(&h, book_name, "knowledge").map_err(Error::Store)?;

    // D-1 — the knowledge-state ledger: not the breaks, but the whole model KEN
    // reasons over — who could know what, and when. Surfaces `grants::build_grants`.
    if ledger {
        return print_ledger(&layout, &h, book, json);
    }

    let mut findings = crate::ken::check::run(&layout, &h, &cfg, book);
    // The opt-in, cost-capped LLM pass for the subtle (unnamed) cases.
    if deep {
        eprintln!("knowledge: running the LLM implied-irony pass…");
        findings.extend(crate::ken::deep::run(project, book_name, max_cost, false).map_err(Error::Store)?);
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

/// D-1 — print the knowledge ledger: the grants model (who could know what, and
/// when) grouped by character, each topic with the chapter it becomes knowable and
/// how (`presence` from a timeline event, or `declared` via a tag). Read-only.
fn print_ledger(
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &crate::store::node::Node,
    json: bool,
) -> Result<()> {
    use std::collections::BTreeMap;
    let (grants, _items, _paras) = crate::ken::grants::build_grants(layout, h, book);

    if json {
        let rows: Vec<serde_json::Value> = grants
            .iter()
            .map(|g| {
                serde_json::json!({
                    "character": g.character,
                    "topic": g.topic,
                    "chapter": g.at.chapter_ord,
                    "source": source_word(g.source),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".into()));
        return Ok(());
    }

    if grants.is_empty() {
        println!(
            "Knowledge ledger — empty. Grant knowledge with `secret:` / `know:` / `reveals:` tags, \
             or add timeline events (a character in an event's participant list knows it)."
        );
        return Ok(());
    }

    // Group by character; within a character, in reading order then topic.
    let mut by_char: BTreeMap<&str, Vec<&crate::ken::Grant>> = BTreeMap::new();
    for g in &grants {
        by_char.entry(g.character.as_str()).or_default().push(g);
    }
    for v in by_char.values_mut() {
        v.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.topic.cmp(&b.topic)));
    }

    println!("Knowledge ledger — who could know what, when\n");
    for (character, grants) in &by_char {
        println!("{character}");
        for g in grants {
            println!("  {:<30} (ch. {}, {})", g.topic, g.at.chapter_ord, source_word(g.source));
        }
    }
    Ok(())
}

/// The provenance of a grant, for display.
fn source_word(s: crate::ken::GrantSource) -> &'static str {
    match s {
        crate::ken::GrantSource::Presence => "presence",
        crate::ken::GrantSource::Declared => "declared",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ken::GrantSource;

    #[test]
    fn ledger_source_word_names_the_provenance() {
        assert_eq!(source_word(GrantSource::Presence), "presence");
        assert_eq!(source_word(GrantSource::Declared), "declared");
    }
}
