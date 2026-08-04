//! CHRONICLE-1 (2.5) — the draft-history intelligence.
//!
//! CHRONICLE persists the readers' collective verdict at each draft milestone and
//! trends it over time — measuring whether revision is working (did the sag lift,
//! did the voices separate, did the continuity breaks close) and, the signature
//! move, which findings a revision *cleared* versus which *new* ones it
//! *introduced*.
//!
//! It is pure measurement: there is **no** prose-write surface anywhere in this
//! module. The whole diagnostic state is captured from one headless
//! [`crate::cli::editorial::collect`] call (CH-P1), and the cleared/introduced diff
//! is a set difference over each [`EditorialFinding`](crate::editorial::EditorialFinding)'s
//! stable `fingerprint()` (CH-P3).
//!
//! CH-P0 lays the substrate: the metric vector, the milestone, and the DuckDB
//! store (mirroring `progress`). The `#![allow(dead_code)]` below is the
//! allow-until-consumer idiom — dropped as CH-P1..P3 wire these in.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::editorial::{EditorialFinding, EditorialReport, Severity};

pub mod store;

/// The editorial severity word CHRONICLE stores (matches `EditorialReport`'s own
/// `errors`/`warnings`/`infos` naming).
fn sev_word(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warn => "warn",
        Severity::Info => "info",
    }
}

/// Tally an [`EditorialReport`] into a [`MetricVector`] + the finding set — pure,
/// so the capture logic is unit-testable without a project. The report's own
/// severity/deferred/stale tallies are reused verbatim.
pub fn summarise(report: &EditorialReport) -> (MetricVector, Vec<FindingRef>) {
    let mut m = MetricVector {
        total: report.findings.len(),
        errors: report.errors,
        warnings: report.warnings,
        infos: report.infos,
        deferred: report.deferred,
        stale: report.stale,
        ..Default::default()
    };
    for f in &report.findings {
        *m.by_category.entry(f.category.clone()).or_insert(0) += 1;
        *m.by_response.entry(f.response().label().to_string()).or_insert(0) += 1;
        *m.by_source.entry(f.source.to_string()).or_insert(0) += 1;
    }
    m.sag_count = m.by_category.get("shape_sag").copied().unwrap_or(0);
    let refs = report.findings.iter().map(finding_ref).collect();
    (m, refs)
}

fn finding_ref(f: &EditorialFinding) -> FindingRef {
    FindingRef {
        fingerprint: f.fingerprint(),
        category: f.category.clone(),
        severity: sev_word(f.severity).to_string(),
        location: Some(f.location.label()).filter(|s| s.as_str() != "—"),
        paragraph: f.location.paragraph,
    }
}

/// Capture the current draft state — one headless
/// [`crate::cli::editorial::collect`] over the whole worklist, tallied into a
/// [`MetricVector`] + finding set. Deterministic (no live AI); the LECTOR
/// `mean_intensity` enrichment is folded in at CH-P2.
pub fn capture(project: &Path, book_name: Option<&str>) -> crate::error::Result<(MetricVector, Vec<FindingRef>)> {
    let report = crate::cli::editorial::collect(project, book_name, None, false)?;
    Ok(summarise(&report))
}

/// The readers' collective verdict at one draft milestone, tallied from a single
/// [`crate::cli::editorial::collect`] report. Every count is "fewer is better"
/// (findings the readers raised), so a falling number is an improvement — except
/// [`mean_intensity`](Self::mean_intensity), which is a raw shape reading, not a
/// score. Serialised to the milestone's `metrics_json` column.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MetricVector {
    /// Total findings across every reader.
    pub total: usize,
    /// Findings by editorial severity (the report's own tallies).
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    /// Findings grouped by their category (`echo`, `shape_sag`, `co_location`, …).
    #[serde(default)]
    pub by_category: BTreeMap<String, usize>,
    /// Findings grouped by REDLINE response kind (`rewrite` / `decision` / `brief`).
    #[serde(default)]
    pub by_response: BTreeMap<String, usize>,
    /// Findings grouped by the detector that raised them (`continuity`,
    /// `read-through`, `stylist`, `doctor`, …).
    #[serde(default)]
    pub by_source: BTreeMap<String, usize>,
    /// Findings hidden by the defer sidecar at capture time.
    pub deferred: usize,
    /// Whether an AI sidecar predated the latest edits (some findings may be stale).
    pub stale: bool,
    /// LECTOR enrichment (CH-P2): mean measured dramatic intensity across chapters
    /// (0..=1), or `None` before the shape read is folded in. A raw reading, never
    /// scored as better/worse.
    #[serde(default)]
    pub mean_intensity: Option<f32>,
    /// Count of `shape_sag` findings — a convenience mirror of `by_category`.
    #[serde(default)]
    pub sag_count: usize,
}

/// One captured draft milestone: its metric vector plus enough identity to trend
/// and diff it. The `label` is the writer's ("draft-3"); `git_ref` is a string
/// recorded verbatim for the writer's own bookkeeping — CHRONICLE never resolves
/// or enumerates git refs.
#[derive(Debug, Clone)]
pub struct Milestone {
    pub id: Uuid,
    pub label: String,
    /// Days since the epoch (via [`crate::dayclock::today_days`]).
    pub day: i64,
    /// Unix seconds at capture — the trend/ordering key.
    pub ts: i64,
    /// The user book this milestone scopes to, or `None` for the whole project.
    pub book_slug: Option<String>,
    /// A git ref string the writer passed (`--ref`), stored verbatim, never resolved.
    pub git_ref: Option<String>,
    pub metrics: MetricVector,
}

/// A single finding's identity within a milestone — enough to diff two milestones'
/// finding sets (the cleared/introduced hook, CH-P3) and to jump to the paragraph
/// (the dashboard, CH-P4). `fingerprint` is the finding's own stable
/// `category ⟂ message` identity (matching REDLINE's defer key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingRef {
    pub fingerprint: String,
    pub category: String,
    /// The editorial severity word: `"error"` | `"warn"` | `"info"`.
    pub severity: String,
    /// A short display label for the finding's location (chapter, else file).
    pub location: Option<String>,
    /// The anchored paragraph, when the finding resolved to one (the jump target).
    pub paragraph: Option<Uuid>,
}

impl FindingRef {
    /// Whether this is an error-severity finding (the gate `check` keys off, CH-P5).
    pub fn is_error(&self) -> bool {
        self.severity == "error"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_vector_json_round_trips() {
        let mut mv = MetricVector { total: 5, errors: 1, warnings: 2, infos: 2, ..Default::default() };
        mv.by_category.insert("echo".into(), 2);
        mv.by_response.insert("rewrite".into(), 3);
        mv.by_source.insert("continuity".into(), 1);
        mv.mean_intensity = Some(0.5);
        mv.sag_count = 1;
        let s = serde_json::to_string(&mv).unwrap();
        let back: MetricVector = serde_json::from_str(&s).unwrap();
        assert_eq!(mv, back);
    }

    #[test]
    fn summarise_tallies_the_report_by_every_axis() {
        use crate::editorial::{Location, Severity};
        let mk = |cat: &str, sev: Severity, src: &'static str, para: bool| EditorialFinding {
            category: cat.into(),
            severity: sev,
            location: Location {
                chapter: Some("ch. 3".into()),
                paragraph: para.then(uuid::Uuid::now_v7),
                ..Default::default()
            },
            message: format!("{cat} msg"),
            hint: None,
            source: src,
            autofixable: false,
        };
        let report = EditorialReport {
            findings: vec![
                mk("echo", Severity::Warn, "doctor", true),      // rewrite
                mk("shape_sag", Severity::Info, "read-through", false), // brief
                mk("co_location", Severity::Error, "continuity", true), // decision
            ],
            errors: 1,
            warnings: 1,
            infos: 1,
            deferred: 2,
            stale: true,
        };
        let (m, refs) = summarise(&report);
        assert_eq!(m.total, 3);
        assert_eq!((m.errors, m.warnings, m.infos), (1, 1, 1));
        assert_eq!(m.deferred, 2);
        assert!(m.stale);
        assert_eq!(m.by_category.get("echo"), Some(&1));
        assert_eq!(m.by_source.get("continuity"), Some(&1));
        assert_eq!(m.by_response.get("rewrite"), Some(&1)); // echo
        assert_eq!(m.by_response.get("decision"), Some(&1)); // co_location
        assert_eq!(m.by_response.get("brief"), Some(&1)); // shape_sag
        assert_eq!(m.sag_count, 1);
        // finding set: fingerprints + severity words + jump targets.
        assert_eq!(refs.len(), 3);
        let co = refs.iter().find(|r| r.category == "co_location").unwrap();
        assert_eq!(co.severity, "error");
        assert!(co.paragraph.is_some());
        assert_eq!(co.location.as_deref(), Some("ch. 3"));
    }

    #[test]
    fn finding_ref_error_gate() {
        let mk = |sev: &str| FindingRef {
            fingerprint: "k".into(),
            category: "c".into(),
            severity: sev.into(),
            location: None,
            paragraph: None,
        };
        assert!(mk("error").is_error());
        assert!(!mk("warn").is_error());
        assert!(!mk("info").is_error());
    }
}
