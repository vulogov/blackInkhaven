//! REDLINE-1 (RD-P5) — `inkhaven revise`: the editorial letter.
//!
//! Runs the same unified worklist the Editorial Pass shows (`collect`), then
//! synthesises it into one prioritized, thematically-grouped developmental letter —
//! the overview a writer opens a revision with. `--json` dumps the findings (each
//! tagged with how it can be acted on) for tooling. The letter advises; it never
//! rewrites (that's the Editorial Pass's confirmed-diff loop).

use std::path::Path;

use crate::editorial::Severity;
use crate::error::{Error, Result};

/// The editorial letter (or `--json` findings) over the whole worklist.
pub fn run(project: &Path, book_name: Option<&str>, json: bool) -> Result<()> {
    let report = crate::cli::editorial::collect(project, book_name, None, false)?;

    if json {
        let rows: Vec<serde_json::Value> = report
            .findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "category": f.category,
                    "severity": severity_word(f.severity),
                    "response": f.response().label(),
                    "location": f.location.label(),
                    "message": f.message,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".into()));
        return Ok(());
    }

    if report.findings.is_empty() {
        println!("\u{2713} no issues found — nothing to revise.");
        return Ok(());
    }

    // One line per finding: severity · category · location · response-kind — message.
    let block = report
        .findings
        .iter()
        .map(|f| {
            format!(
                "- [{}] {} · {} ({}) — {}",
                severity_word(f.severity),
                f.category,
                f.location.label(),
                f.response().label(),
                f.message,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    eprintln!(
        "revise: synthesising the editorial letter over {} finding(s)…",
        report.findings.len()
    );
    let letter = crate::redline::letter(project, &block).map_err(Error::Store)?;
    println!("{letter}");
    Ok(())
}

fn severity_word(s: Severity) -> &'static str {
    match s {
        Severity::Error => "high",
        Severity::Warn => "med",
        Severity::Info => "low",
    }
}
