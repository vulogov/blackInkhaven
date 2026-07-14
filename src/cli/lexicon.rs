//! LEXICON — `inkhaven lexicon list`. Prints the scholarly-lexicon terms held in
//! the Glossary book: each term's original-language forms, its distinct senses,
//! and whether it is watched for equivocation (≥2 senses + `watch_equivocation`,
//! which the reasoning-rigor reader then polices).

use std::path::Path;

use crate::config::Config;
use crate::error::Result;
use crate::glossary::{self, GlossaryEntry};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;

use super::LexiconCommand;

pub fn run(project: &Path, cmd: LexiconCommand) -> Result<()> {
    match cmd {
        LexiconCommand::List { book, watched, json } => list(project, book.as_deref(), watched, json),
    }
}

fn list(project: &Path, book_name: Option<&str>, watched_only: bool, json: bool) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;

    let mut entries = glossary::glossary_entries_from_store(&store, &h, book_name);
    entries.retain(GlossaryEntry::is_valid);
    if watched_only {
        entries.retain(GlossaryEntry::is_equivocation_watched);
    }
    entries.sort_by(|a, b| a.term.to_lowercase().cmp(&b.term.to_lowercase()));

    if json {
        let arr: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "term": e.term,
                    "original_forms": e.original_forms,
                    "senses": e.senses.iter().map(|s| serde_json::json!({
                        "label": s.label,
                        "gloss": s.gloss,
                    })).collect::<Vec<_>>(),
                    "watch_equivocation": e.is_equivocation_watched(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
        return Ok(());
    }

    println!("Scholarly lexicon — {} term(s)", entries.len());
    println!("{}", "─".repeat(72));
    if entries.is_empty() {
        println!("No lexicon terms. Add senses + `watch_equivocation` to a Glossary entry.");
    }
    for e in &entries {
        let forms = if e.original_forms.is_empty() {
            String::new()
        } else {
            format!("  ⟨{}⟩", e.original_forms.join(", "))
        };
        let watch = if e.is_equivocation_watched() { "  ⊬ watched" } else { "" };
        println!("\n{}{forms}{watch}", e.term);
        if e.senses.is_empty() && !e.definition.trim().is_empty() {
            println!("    {}", e.definition.trim());
        }
        for (i, s) in e.senses.iter().enumerate() {
            let label = if s.label.trim().is_empty() {
                String::new()
            } else {
                format!(" {}", s.label.trim())
            };
            println!("    [{}]{label} — {}", i + 1, s.gloss.trim());
        }
    }
    Ok(())
}
