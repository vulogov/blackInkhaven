//! H-2 — `inkhaven read`: a glanceable **state of the manuscript**.
//!
//! The reader family's finding-counts grouped by reader (the finding `source`),
//! each with its severity split, plus a health line. The overview companion to
//! `inkhaven edit` (which prints the full ranked worklist) and the `Ctrl+B *`
//! reader hub. Deterministic — it reuses the same [`crate::cli::editorial::collect`].

use std::collections::BTreeMap;
use std::path::Path;

use crate::editorial::Severity;
use crate::error::Result;

pub fn run(project: &Path, book_name: Option<&str>, json: bool) -> Result<()> {
    let report = crate::cli::editorial::collect(project, book_name, None, false)?;

    // Per source: [count, errors, warnings, infos].
    let mut by_source: BTreeMap<&str, [usize; 4]> = BTreeMap::new();
    for f in &report.findings {
        let e = by_source.entry(f.source).or_default();
        e[0] += 1;
        match f.severity {
            Severity::Error => e[1] += 1,
            Severity::Warn => e[2] += 1,
            Severity::Info => e[3] += 1,
        }
    }

    if json {
        let rows: Vec<serde_json::Value> = by_source
            .iter()
            .map(|(src, c)| {
                serde_json::json!({
                    "source": src, "reader": reader_label(src),
                    "count": c[0], "errors": c[1], "warnings": c[2], "infos": c[3],
                })
            })
            .collect();
        let out = serde_json::json!({
            "findings": report.findings.len(),
            "errors": report.errors,
            "warnings": report.warnings,
            "infos": report.infos,
            "deferred": report.deferred,
            "stale": report.stale,
            "readers": by_source.len(),
            "by_source": rows,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".into()));
        return Ok(());
    }

    println!("State of the manuscript\n");
    if by_source.is_empty() {
        println!("  \u{2713} every reader is clean.");
        return Ok(());
    }
    for (src, c) in &by_source {
        let mut sev = Vec::new();
        if c[1] > 0 {
            sev.push(format!("{} \u{2717}", c[1])); // ✗
        }
        if c[2] > 0 {
            sev.push(format!("{} \u{26a0}", c[2])); // ⚠
        }
        if c[3] > 0 {
            sev.push(format!("{} \u{b7}", c[3])); // ·
        }
        println!("  {:<24} {:>3}   {}", reader_label(src), c[0], sev.join(" \u{b7} "));
    }

    let mut tail = String::new();
    if report.deferred > 0 {
        tail.push_str(&format!(" \u{b7} {} deferred", report.deferred));
    }
    if report.stale {
        tail.push_str(" \u{b7} \u{26a0} may be stale (run `inkhaven world --deep`)");
    }
    println!(
        "\n{} finding(s) across {} reader(s) \u{2014} {} error(s), {} other{tail}.",
        report.findings.len(),
        by_source.len(),
        report.errors,
        report.warnings + report.infos,
    );
    Ok(())
}

/// A friendly reader name for a finding `source`. Unknown sources pass through.
fn reader_label(source: &str) -> String {
    match source {
        "knowledge" => "Knowledge (KEN)",
        "bonds" => "Bonds",
        "continuity" => "Continuity (SENTINEL)",
        "read-through" => "Read-through (LECTOR)",
        "stylist" => "Voice (CHORUS)",
        "editor" => "Inner Editor",
        "drift" => "Drift",
        "facts" => "Facts",
        "plan" => "Structure (plan)",
        "world" => "World coverage",
        "doctor" => "Editorial (doctor)",
        other => other,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_label_maps_known_sources_and_passes_unknown_through() {
        assert_eq!(reader_label("knowledge"), "Knowledge (KEN)");
        assert_eq!(reader_label("bonds"), "Bonds");
        assert_eq!(reader_label("read-through"), "Read-through (LECTOR)");
        // An unmapped source (e.g. a future reader) shows verbatim, not lost.
        assert_eq!(reader_label("some_new_reader"), "some_new_reader");
    }
}
