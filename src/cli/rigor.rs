//! RIGOR — `inkhaven rigor scan`. Runs the deterministic, zero-AI reasoning-rigor
//! reader across the manuscript and prints the argument-rigor signals it finds
//! (false dichotomy, question-begging, straw man, overgeneralization,
//! non-sequitur). Advisory by default (exit 0); `--strict` makes any signal a
//! non-zero exit for a continuous-integration gate.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;

use super::RigorCommand;

pub fn run(project: &Path, cmd: RigorCommand) -> Result<()> {
    match cmd {
        RigorCommand::Scan { book, signal, json, strict } => {
            scan(project, book.as_deref(), signal.as_deref(), json, strict)
        }
    }
}

fn scan(
    project: &Path,
    book_name: Option<&str>,
    signal: Option<&str>,
    json: bool,
    strict: bool,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;
    let book = super::resolve_user_book(&h, book_name, "rigor").map_err(Error::Store)?;

    let mut findings = crate::inner_rigor::scan_book(&layout, &h, &cfg, book);
    if let Some(sig) = signal {
        if sig != "all" {
            if crate::inner_rigor::RigorSignal::from_code(sig).is_none() {
                return Err(Error::Config(format!(
                    "rigor: unknown --signal `{sig}` (use false-dichotomy | question-begging | \
                     straw-man | overgeneralization | non-sequitur | all)"
                )));
            }
            findings.retain(|f| f.signal.as_code() == sig);
        }
    }

    if json {
        let arr: Vec<serde_json::Value> = findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "signal": f.signal.as_code(),
                    "chapter": f.chapter_ord,
                    "para_id": f.para_id,
                    "description": f.description,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
    } else {
        println!("Reasoning-rigor reader — argument signals · `{}`", book.title);
        println!("{}", "─".repeat(72));
        if findings.is_empty() {
            println!("No rigor signals.");
        }
        for f in &findings {
            println!("  ⊬ [ch.{} · {}] {}", f.chapter_ord, f.signal.label(), f.description);
        }
    }

    if strict && !findings.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}
