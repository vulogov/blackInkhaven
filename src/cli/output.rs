//! 1.3.24 PANE-1 — CLI surface over the Output message store (RFC §10.1).
//!
//! The pane itself is a TUI feature; this is the headless surface — list, emit
//! (for testing/scripting), dismiss, clear — usable over ssh or in a pipeline.

use std::path::Path;

use crate::cli::OutputCommand;
use crate::error::{Error, Result};
use crate::pane::output::{Message, OutputStore, Severity};

pub fn run(project: &Path, cmd: OutputCommand) -> Result<()> {
    let store = OutputStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("opening Output store: {e}")))?;
    // Cleanup on open (RFC §8.15): drop expired / over-quota messages.
    let _ = store.cleanup();

    match cmd {
        OutputCommand::Show { kind, severity, limit, json } => {
            show(&store, kind.as_deref(), severity.as_deref(), limit, json)
        }
        OutputCommand::Emit { kind, metadata, severity } => emit(&store, &kind, &metadata, &severity),
        OutputCommand::Dismiss { id } => {
            let uuid = uuid::Uuid::parse_str(&id)
                .map_err(|e| Error::Config(format!("bad message id `{id}`: {e}")))?;
            store.dismiss(uuid).map_err(|e| Error::Store(format!("dismiss: {e}")))?;
            println!("dismissed {id}");
            Ok(())
        }
        OutputCommand::Clear { kind, all } => clear(&store, kind.as_deref(), all),
    }
}

fn show(
    store: &OutputStore,
    kind: Option<&str>,
    severity: Option<&str>,
    limit: Option<usize>,
    json: bool,
) -> Result<()> {
    let mut msgs = match kind {
        Some(k) => store.by_kind(k),
        None => store.active(),
    }
    .map_err(|e| Error::Store(format!("query: {e}")))?;

    if let Some(sev) = severity {
        let want = Severity::parse(sev);
        msgs.retain(|m| m.severity == want);
    }
    if let Some(n) = limit {
        msgs.truncate(n);
    }

    if json {
        let arr: Vec<serde_json::Value> = msgs.iter().map(message_json).collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&arr)
                .map_err(|e| Error::Store(format!("serializing: {e}")))?
        );
        return Ok(());
    }

    if msgs.is_empty() {
        println!("(no active Output messages)");
        return Ok(());
    }
    for m in &msgs {
        let summary = m
            .metadata
            .get("text")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| m.metadata.to_string());
        let pin = if m.pinned { " 📌" } else { "" };
        println!("{} [{}] {}{}", m.severity.icon(), m.kind, summary, pin);
        println!("   {}  {}", m.id, m.severity.as_str());
    }
    Ok(())
}

fn emit(store: &OutputStore, kind: &str, metadata: &str, severity: &str) -> Result<()> {
    let meta: serde_json::Value = serde_json::from_str(metadata)
        .map_err(|e| Error::Config(format!("--metadata is not valid JSON: {e}")))?;
    let msg = Message::new(
        kind.to_string(),
        Severity::parse(severity),
        crate::pane::output::Lifetime::Session(200),
        meta,
    );
    let id = store.emit(&msg).map_err(|e| Error::Store(format!("emit: {e}")))?;
    println!("emitted {id} ({kind})");
    Ok(())
}

fn clear(store: &OutputStore, kind: Option<&str>, all: bool) -> Result<()> {
    if kind.is_none() && !all {
        return Err(Error::Config("give a --kind or --all".into()));
    }
    let msgs = match kind {
        Some(k) => store.by_kind(k),
        None => store.active(),
    }
    .map_err(|e| Error::Store(format!("query: {e}")))?;
    let n = msgs.len();
    for m in &msgs {
        store.dismiss(m.id).map_err(|e| Error::Store(format!("dismiss: {e}")))?;
    }
    println!("cleared {n} message(s)");
    Ok(())
}

fn message_json(m: &Message) -> serde_json::Value {
    serde_json::json!({
        "id": m.id.to_string(),
        "kind": m.kind,
        "timestamp": m.timestamp,
        "severity": m.severity.as_str(),
        "pinned": m.pinned,
        "metadata": m.metadata,
    })
}
