//! 1.3.11 WORLD-3 — the world report: one consolidated consistency snapshot.
//!
//! This is the pure core — the data model + the health math. It folds the
//! outputs of the world-layer checks into a single view:
//!
//! - **Facts** — established facts (Facts-book size), internal contradictions
//!   (`facts check`), and prose-vs-fact contradictions (`facts scan`).
//! - **Drift** — divergent-description contradictions (`drift scan`).
//! - **Continuity** — tracked character attributes (`continuity extract`).
//! - **Coverage** — how many entities the world defines.
//! - **Anachronisms** — deterministic era flags.
//!
//! The impure assembly (reading the sidecars + walking the store) lives in
//! `cli::world`; everything here is testable without I/O.

use std::collections::HashSet;

use serde::Serialize;

use crate::drift::{DriftConflict, EntityKind};
use crate::facts_scan::FactConflict;

/// A consolidated world-consistency snapshot. The conflict lists carry detail
/// (so `inkhaven world` can name them); the rest are counts.
#[derive(Debug, Clone, Default, Serialize)]
pub struct WorldReport {
    /// Established facts — Facts-book paragraphs.
    pub facts_total: usize,
    /// Internal fact contradictions (`facts check`).
    pub facts_conflicts: Vec<FactConflict>,
    /// Prose-vs-fact contradictions (`facts scan`).
    pub facts_prose_findings: usize,
    /// Divergent-description contradictions (`drift scan`).
    pub drift_conflicts: Vec<DriftConflict>,
    /// Tracked character attributes (`continuity extract`).
    pub continuity_attributes: usize,
    pub characters: usize,
    pub places: usize,
    pub artefacts: usize,
    /// Deterministic anachronism flags (terms postdating the setting year).
    pub anachronisms: usize,
    /// Entities defined in the books but never named in the prose — a dangling
    /// cast member / place / artefact. Coverage, not counted as an issue.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub undescribed: Vec<String>,
}

/// Entities (name + kind) whose lowercased name never appears in `appeared`
/// (the set of lowercased entity names found in the prose). Pure.
pub fn undescribed_of(
    lexicon: &[(String, EntityKind)],
    appeared: &HashSet<String>,
) -> Vec<(String, EntityKind)> {
    lexicon
        .iter()
        .filter(|(name, _)| !appeared.contains(&name.to_lowercase()))
        .cloned()
        .collect()
}

impl WorldReport {
    /// Total entities the world defines.
    pub fn entity_total(&self) -> usize {
        self.characters + self.places + self.artefacts
    }

    /// The number of *issues* — contradictions + anachronisms. Established
    /// facts, tracked attributes, and entity counts are coverage, not issues.
    pub fn issue_count(&self) -> usize {
        self.facts_conflicts.len()
            + self.facts_prose_findings
            + self.drift_conflicts.len()
            + self.anachronisms
    }

    /// The one-line health string rendered atop `inkhaven world` and the story
    /// bible — `"World: 3 issue(s) — 1 fact conflict · 2 drift"`, or a clean
    /// snapshot when there's nothing wrong.
    pub fn summary(&self) -> String {
        let n = self.issue_count();
        if n == 0 {
            return format!(
                "World: ✓ consistent — {} fact(s), {} entit{}",
                self.facts_total,
                self.entity_total(),
                if self.entity_total() == 1 { "y" } else { "ies" }
            );
        }
        let mut parts = Vec::new();
        if !self.facts_conflicts.is_empty() {
            parts.push(plural(self.facts_conflicts.len(), "fact conflict", "fact conflicts"));
        }
        if self.facts_prose_findings > 0 {
            parts.push(format!("{} prose-vs-fact", self.facts_prose_findings));
        }
        if !self.drift_conflicts.is_empty() {
            parts.push(format!("{} drift", self.drift_conflicts.len()));
        }
        if self.anachronisms > 0 {
            parts.push(plural(self.anachronisms, "anachronism", "anachronisms"));
        }
        format!("World: {n} issue(s) — {}", parts.join(" · "))
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drift::EntityKind;

    fn fact_conflict() -> FactConflict {
        FactConflict { a: "mild winters".into(), b: "harbor freezes".into(), detail: "x".into() }
    }
    fn drift_conflict() -> DriftConflict {
        DriftConflict {
            entity: "The Goose".into(),
            kind: EntityKind::Place,
            a: "smoky".into(),
            b: "airy".into(),
            chapter_a: "ch-2".into(),
            chapter_b: "ch-20".into(),
            paragraph_b: None,
            detail: "y".into(),
        }
    }

    #[test]
    fn issue_count_sums_contradictions_and_anachronisms_only() {
        let r = WorldReport {
            facts_total: 40,
            facts_conflicts: vec![fact_conflict()],
            facts_prose_findings: 2,
            drift_conflicts: vec![drift_conflict(), drift_conflict()],
            continuity_attributes: 12, // coverage, not an issue
            characters: 5,
            places: 3,
            artefacts: 1,
            anachronisms: 1,
            undescribed: vec!["Joss".into()], // coverage, not an issue
        };
        // 1 fact conflict + 2 prose + 2 drift + 1 anachronism = 6
        assert_eq!(r.issue_count(), 6);
        assert_eq!(r.entity_total(), 9);
    }

    #[test]
    fn undescribed_of_returns_unnamed_entities() {
        let lexicon = vec![
            ("Mara".to_string(), EntityKind::Character),
            ("Joss".to_string(), EntityKind::Character),
            ("The Goose".to_string(), EntityKind::Place),
        ];
        // only "mara" and "the goose" appear in the prose
        let appeared: HashSet<String> = ["mara", "the goose"].iter().map(|s| s.to_string()).collect();
        let out = undescribed_of(&lexicon, &appeared);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, "Joss", "Joss is defined but never named in prose");
    }

    #[test]
    fn summary_clean_vs_issues() {
        let clean = WorldReport { facts_total: 40, characters: 9, ..Default::default() };
        assert_eq!(clean.issue_count(), 0);
        assert!(clean.summary().contains("✓ consistent"));
        assert!(clean.summary().contains("40 fact(s)"));

        let bad = WorldReport {
            facts_conflicts: vec![fact_conflict()],
            drift_conflicts: vec![drift_conflict(), drift_conflict()],
            ..Default::default()
        };
        let s = bad.summary();
        assert!(s.starts_with("World: 3 issue(s) —"), "got: {s}");
        assert!(s.contains("1 fact conflict") && s.contains("2 drift"));
        assert!(!s.contains("prose-vs-fact"), "zero categories are omitted");
    }
}
