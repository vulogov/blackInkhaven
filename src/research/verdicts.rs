//! RESRCH-2.4 (RE-P5) — `/factcheck` per-fact verdicts. The truth pass already
//! grades every fact `ACCURATE | DUBIOUS | INACCURATE — <reason>`; this captures
//! that grade per Facts node so the **Facts tree** can glyph each fact (✓ / ? / ✗)
//! and the author can navigate to a flagged paragraph and run `/whatswrong`.
//!
//! A `.inkhaven/fact-verdicts.json` sidecar (the established pattern, like
//! `provenance.rs`) so the marks survive a restart until the next `/factcheck`.

use std::collections::BTreeMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::project::ProjectLayout;

/// A fact's fact-check level (ordered worst-last for sorting/severity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Level {
    Accurate,
    Dubious,
    Inaccurate,
}

impl Level {
    /// The Facts-tree glyph for this level.
    pub(super) fn glyph(self) -> &'static str {
        match self {
            Level::Accurate => "✓",
            Level::Dubious => "?",
            Level::Inaccurate => "✗",
        }
    }

    /// The verdict word, for prompts / summaries.
    pub(super) fn label(self) -> &'static str {
        match self {
            Level::Accurate => "ACCURATE",
            Level::Dubious => "DUBIOUS",
            Level::Inaccurate => "INACCURATE",
        }
    }

    fn parse(word: &str) -> Option<Level> {
        match word.to_ascii_uppercase().as_str() {
            "ACCURATE" => Some(Level::Accurate),
            "DUBIOUS" => Some(Level::Dubious),
            "INACCURATE" => Some(Level::Inaccurate),
            _ => None,
        }
    }
}

/// One fact's verdict: its level and the model's short reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Verdict {
    pub level: Level,
    #[serde(default)]
    pub reason: String,
    pub checked_at: String,
}

/// The verdict sidecar: fact-node-id → verdict.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct Verdicts {
    #[serde(default)]
    pub facts: BTreeMap<String, Verdict>,
}

/// The `/factcheck` verdict sidecar.
const VERDICTS_FILE: &str = "fact-verdicts.json";
/// UD-P4 — the `/undisputed` common-sense verdict sidecar (separate — undisputed
/// facts never carry a `/factcheck` verdict, and the tree renders them via `※`).
const UNDISPUTED_FILE: &str = "fact-undisputed.json";

impl Verdicts {
    fn path(layout: &ProjectLayout, file: &str) -> std::path::PathBuf {
        layout.root.join(".inkhaven").join(file)
    }

    fn load_file(layout: &ProjectLayout, file: &str) -> Verdicts {
        match std::fs::read_to_string(Verdicts::path(layout, file)) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Verdicts::default(),
        }
    }

    fn save_file(&self, layout: &ProjectLayout, file: &str) -> Result<()> {
        let dir = layout.root.join(".inkhaven");
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        let json = serde_json::to_string_pretty(self).context("serialise verdicts")?;
        crate::io_atomic::write(&Verdicts::path(layout, file), json.as_bytes())
            .with_context(|| format!("write {file}"))?;
        Ok(())
    }

    pub(super) fn load(layout: &ProjectLayout) -> Verdicts {
        Verdicts::load_file(layout, VERDICTS_FILE)
    }

    pub(super) fn save(&self, layout: &ProjectLayout) -> Result<()> {
        self.save_file(layout, VERDICTS_FILE)
    }

    pub(super) fn load_undisputed(layout: &ProjectLayout) -> Verdicts {
        Verdicts::load_file(layout, UNDISPUTED_FILE)
    }

    pub(super) fn save_undisputed(&self, layout: &ProjectLayout) -> Result<()> {
        self.save_file(layout, UNDISPUTED_FILE)
    }

    pub(super) fn level_for(&self, node_id: Uuid) -> Option<Level> {
        self.facts.get(&node_id.to_string()).map(|v| v.level)
    }

    pub(super) fn get(&self, node_id: Uuid) -> Option<&Verdict> {
        self.facts.get(&node_id.to_string())
    }
}

/// Parse a numbered per-statement report into per-node verdicts. `level_of` maps
/// the verdict word to a `Level`; the statement number `n` (1-based) indexes the
/// ordered `fact_ids`. `now` stamps every verdict.
fn parse_report(
    report: &str,
    fact_ids: &[Uuid],
    now: &str,
    level_of: impl Fn(&str) -> Option<Level>,
) -> BTreeMap<String, Verdict> {
    let mut out = BTreeMap::new();
    for line in report.lines() {
        let line = line.trim();
        let Some((num_str, rest)) = line.split_once('.') else { continue };
        let Ok(num) = num_str.trim().parse::<usize>() else { continue };
        if num == 0 || num > fact_ids.len() {
            continue;
        }
        let rest = rest.trim();
        let word = rest.split([' ', '\t', ':', '—', '-']).find(|s| !s.is_empty()).unwrap_or("");
        let Some(level) = level_of(word) else { continue };
        let reason = rest
            .split_once('—')
            .or_else(|| rest.split_once(" - "))
            .map(|(_, r)| r.trim().to_string())
            .unwrap_or_default();
        out.insert(
            fact_ids[num - 1].to_string(),
            Verdict { level, reason, checked_at: now.to_string() },
        );
    }
    out
}

/// `/factcheck` truth-pass report → verdicts (`ACCURATE | DUBIOUS | INACCURATE`).
pub(super) fn parse_truth_report(report: &str, fact_ids: &[Uuid], now: &str) -> BTreeMap<String, Verdict> {
    parse_report(report, fact_ids, now, Level::parse)
}

/// UD-P4 — `/undisputed` common-sense report → verdicts, mapping
/// `PLAUSIBLE → Accurate`, `ODD → Dubious`, `INCOHERENT → Inaccurate` so the
/// tree can colour the `※` glyph by the common-sense result.
pub(super) fn parse_undisputed_report(report: &str, fact_ids: &[Uuid], now: &str) -> BTreeMap<String, Verdict> {
    parse_report(report, fact_ids, now, |word| match word.to_ascii_uppercase().as_str() {
        "PLAUSIBLE" => Some(Level::Accurate),
        "ODD" => Some(Level::Dubious),
        "INCOHERENT" => Some(Level::Inaccurate),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_levels_and_reasons() {
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
        let report = "1. ACCURATE — checks out\n2. DUBIOUS — date is uncertain\n3. INACCURATE — Rome fell in 476, not 410";
        let m = parse_truth_report(report, &ids, "now");
        assert_eq!(m.len(), 3);
        assert_eq!(m[&ids[0].to_string()].level, Level::Accurate);
        assert_eq!(m[&ids[1].to_string()].level, Level::Dubious);
        let bad = &m[&ids[2].to_string()];
        assert_eq!(bad.level, Level::Inaccurate);
        assert!(bad.reason.contains("476"));
    }

    #[test]
    fn ignores_garbage_and_out_of_range() {
        let ids = vec![Uuid::new_v4()];
        let report = "preamble line\n1. ACCURATE — ok\n9. INACCURATE — out of range\nnonsense";
        let m = parse_truth_report(report, &ids, "now");
        assert_eq!(m.len(), 1);
        assert_eq!(m[&ids[0].to_string()].level, Level::Accurate);
    }

    #[test]
    fn glyphs_distinct() {
        assert_eq!(Level::Accurate.glyph(), "✓");
        assert_eq!(Level::Dubious.glyph(), "?");
        assert_eq!(Level::Inaccurate.glyph(), "✗");
    }

    #[test]
    fn undisputed_maps_common_sense_words() {
        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();
        let report = "1. PLAUSIBLE — fits the world\n2. ODD — a bit strange\n3. INCOHERENT — contradicts itself";
        let m = parse_undisputed_report(report, &ids, "now");
        assert_eq!(m[&ids[0].to_string()].level, Level::Accurate);
        assert_eq!(m[&ids[1].to_string()].level, Level::Dubious);
        assert_eq!(m[&ids[2].to_string()].level, Level::Inaccurate);
        // Real-world verdict words are NOT accepted by the common-sense parser.
        assert!(parse_undisputed_report("1. ACCURATE — x", &ids, "now").is_empty());
    }
}
