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

impl Verdicts {
    fn path(layout: &ProjectLayout) -> std::path::PathBuf {
        layout.root.join(".inkhaven").join("fact-verdicts.json")
    }

    pub(super) fn load(layout: &ProjectLayout) -> Verdicts {
        match std::fs::read_to_string(Verdicts::path(layout)) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Verdicts::default(),
        }
    }

    pub(super) fn save(&self, layout: &ProjectLayout) -> Result<()> {
        let dir = layout.root.join(".inkhaven");
        std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
        let json = serde_json::to_string_pretty(self).context("serialise verdicts")?;
        crate::io_atomic::write(&Verdicts::path(layout), json.as_bytes())
            .context("write fact-verdicts.json")?;
        Ok(())
    }

    pub(super) fn level_for(&self, node_id: Uuid) -> Option<Level> {
        self.facts.get(&node_id.to_string()).map(|v| v.level)
    }

    pub(super) fn get(&self, node_id: Uuid) -> Option<&Verdict> {
        self.facts.get(&node_id.to_string())
    }
}

/// Parse the truth-pass report into per-node verdicts. The report has one line
/// per statement — `<n>. ACCURATE | DUBIOUS | INACCURATE — <reason>` — where the
/// statement number `n` (1-based) indexes the ordered `fact_ids` passed to the
/// truth calls. `now` stamps every parsed verdict.
pub(super) fn parse_truth_report(report: &str, fact_ids: &[Uuid], now: &str) -> BTreeMap<String, Verdict> {
    let mut out = BTreeMap::new();
    for line in report.lines() {
        let line = line.trim();
        let Some((num_str, rest)) = line.split_once('.') else { continue };
        let Ok(num) = num_str.trim().parse::<usize>() else { continue };
        if num == 0 || num > fact_ids.len() {
            continue;
        }
        let rest = rest.trim();
        // First whitespace-delimited token is the verdict word.
        let word = rest.split([' ', '\t', ':', '—', '-']).find(|s| !s.is_empty()).unwrap_or("");
        let Some(level) = Level::parse(word) else { continue };
        // Reason: text after the first em-dash / hyphen separator, if any.
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
}
