//! INNER_SOCRATES-1 — the `inkhaven inner-socrates` CLI surface. P2 ships `check`
//! (run the deterministic Fast track over prose) and `ledger` (list declared
//! intentions). The Slow track, personas, and conversation land in later phases.

use std::path::Path;

use crate::cli::InnerSocratesCommand;
use crate::error::{Error, Result};
use crate::inner_socrates::fast;
use crate::inner_socrates::intent::{FindingContext, IntentLedger};
use crate::inner_socrates::output::emit_finding;
use crate::inner_socrates::storage::InnerSocratesStore;
use crate::inner_socrates::types::Persona;

pub fn run(project: &Path, cmd: InnerSocratesCommand) -> Result<()> {
    match cmd {
        InnerSocratesCommand::Check { text, paragraph } => check(project, text, paragraph),
        InnerSocratesCommand::Ledger => ledger(project),
    }
}

/// Run the Fast track over prose and surface its questions. When the project has
/// an Inner Socrates store, the intent ledger is consulted, findings persist (a
/// re-check replaces a paragraph's prior ones), and they emit to Output.
fn check(project: &Path, text: Option<String>, paragraph: Option<String>) -> Result<()> {
    let store = InnerSocratesStore::open_for_project(project).ok();
    let ledger = store
        .as_ref()
        .and_then(|s| s.load_ledger().ok())
        .unwrap_or_default();

    let (prose, paragraph_id) = resolve_prose(project, text, paragraph)?;
    let persona = Persona::default_inner_socrates();
    let ctx = FindingContext { paragraph_id: paragraph_id.map(|p| p.to_string()), ..Default::default() };

    let findings = fast::check_paragraph(&prose, &persona, &ledger, &ctx);

    // Persist + emit when a paragraph is identified and a store is present.
    if let (Some(s), Some(pid)) = (store.as_ref(), paragraph_id) {
        let _ = s.clear_findings_for_paragraph(pid);
        for f in &findings {
            let _ = s.insert_finding(f, Some(pid), None);
            emit_finding(f, Some(pid));
        }
    }

    if findings.is_empty() {
        println!("\u{2713} no questions raised (fast track)");
        return Ok(());
    }
    for f in &findings {
        let icon = match f.severity {
            crate::inner_socrates::types::Severity::Probe => "\u{25c6}",   // ◆
            crate::inner_socrates::types::Severity::Inquiry => "\u{25c7}", // ◇
            crate::inner_socrates::types::Severity::Notice => "\u{00b7}",  // ·
        };
        println!(
            "{icon} {} [{}] {}",
            f.severity.label(),
            f.category.label(),
            f.question
        );
    }
    println!("\n{} question(s) · persona: {}", findings.len(), persona.name);
    Ok(())
}

/// Resolve `(prose, paragraph_id)` from `--text` or `--paragraph <id>`.
fn resolve_prose(
    project: &Path,
    text: Option<String>,
    paragraph: Option<String>,
) -> Result<(String, Option<uuid::Uuid>)> {
    match (text, paragraph) {
        (Some(t), _) => Ok((t, None)),
        (None, Some(pid)) => {
            use crate::config::Config;
            use crate::project::ProjectLayout;
            use crate::store::Store;
            let id = uuid::Uuid::parse_str(&pid)
                .map_err(|e| Error::Config(format!("bad paragraph id `{pid}`: {e}")))?;
            let layout = ProjectLayout::new(project);
            layout.require_initialized()?;
            let cfg = Config::load_layered(&layout.config_path())?;
            let store = Store::open(layout, &cfg)?;
            let bytes = store
                .get_content(id)
                .map_err(|e| Error::Store(format!("reading paragraph: {e}")))?
                .ok_or_else(|| Error::Config(format!("paragraph `{pid}` not found")))?;
            Ok((String::from_utf8_lossy(&bytes).into_owned(), Some(id)))
        }
        (None, None) => Err(Error::Config("give --text \"…\" or --paragraph <id>".into())),
    }
}

/// List the intent ledger.
fn ledger(project: &Path) -> Result<()> {
    let store = InnerSocratesStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("opening inner-socrates store: {e}")))?;
    let entries = store.list_intents().map_err(|e| Error::Store(format!("listing: {e}")))?;
    if entries.is_empty() {
        println!("(no intent ledger entries yet)");
        return Ok(());
    }
    for e in &entries {
        let cats: Vec<&str> = e.coverage.iter().map(|c| c.id()).collect();
        println!("  {} [{}] · covers [{}]", e.id, e.kind.id(), cats.join(", "));
        if !e.description.is_empty() {
            println!("      {}", e.description);
        }
    }
    println!("\n{} intent entry(ies).", entries.len());
    Ok(())
}
