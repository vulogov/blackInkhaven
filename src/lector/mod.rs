//! LECTOR-1 — The Read-Through: the book reads itself, end to end.
//!
//! The zoom-out complement to the per-paragraph inner readers and the per-break
//! SENTINEL: a **forward walk** of the whole manuscript that reports both the
//! *shape* of the read (structure & pacing, measured from the prose — LR-P1/P2)
//! and the *experience* of the read (clarity, attention, stakes, payoff — LR-P3
//! deterministic + LR-P4 the synthetic first-read). See
//! `Documentation/PROPOSALS/LECTOR-1_PLAN.md`.
//!
//! LR-P0 lands the shared vocabulary: [`ChapterRead`] (what one chapter looks like
//! on the read-through), [`ReaderFinding`] + [`Severity`], and the [`ReadThrough`]
//! container with the dedup/rank primitives the walk (LR-P3) composes.
//!
//! The defining discipline is **forward-only**: a `ChapterRead` is computed with
//! the accumulated state of every chapter *before* it and never with knowledge of
//! what comes after — that is what makes it a *reader* rather than an analyst.

// The type surface is consumed by LR-P1 (intensity), LR-P3 (the walk), and LR-P5
// (the report); until they land it is scaffolding. Drop this allow at LR-P3, when
// the walk constructs findings and composes dedupe/rank.
#![allow(dead_code)]

use std::collections::HashSet;

use uuid::Uuid;

pub(crate) mod intensity;

/// How much a reader finding matters. `Concern` is a real problem (a stretch a
/// reader would likely put down); `Notice` is worth a look; `Info` is a nudge.
/// A local enum (mapped to the Output pane's severity only at emit time) keeps the
/// walk decoupled from PANE-1, the way SENTINEL's severity does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    Info,
    Notice,
    Concern,
}

impl Severity {
    /// Higher = more serious (for ranking).
    pub(crate) fn rank(self) -> u8 {
        match self {
            Severity::Info => 0,
            Severity::Notice => 1,
            Severity::Concern => 2,
        }
    }
    pub(crate) fn label(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Notice => "notice",
            Severity::Concern => "concern",
        }
    }
}

/// One thing a first reader would flag on the read-through, normalised so the
/// report, the review pass, and the dashboard speak one shape.
#[derive(Debug, Clone)]
pub(crate) struct ReaderFinding {
    /// The kind of reader problem: `confusion` | `info_dump` | `attention_dip` |
    /// `unpaid_setup` | `stakes_gap` | `put_down_risk` | `reader` (the synthetic
    /// first-read) | …
    pub kind: &'static str,
    pub severity: Severity,
    /// 1-based chapter ordinal the reader hits it at (0 = book-level).
    pub chapter: u32,
    /// The paragraph to jump to, if the walk knows one.
    pub anchor: Option<Uuid>,
    /// The entities the finding is about (character/place/thread names) — for
    /// dedup and display.
    pub entities: Vec<String>,
    pub message: String,
    /// Which pass produced it (`walk` for the deterministic reader-state findings,
    /// `reader` for the LLM synthetic first-read).
    pub source: &'static str,
    /// A stable fingerprint used to fold duplicate reports of the same problem.
    pub dedup_key: String,
}

impl ReaderFinding {
    /// A conservative dedup key: kind + the entity set (order- and
    /// case-insensitive) + chapter — one problem per (kind, entities, chapter).
    pub(crate) fn make_dedup_key(kind: &str, entities: &[String], chapter: u32) -> String {
        let mut es: Vec<String> = entities.iter().map(|e| e.to_lowercase()).collect();
        es.sort();
        format!("{kind}|{}|{chapter}", es.join(","))
    }
}

/// What one chapter looks like on the read-through, computed with the accumulated
/// state of every prior chapter (never a later one).
#[derive(Debug, Clone, Default)]
pub(crate) struct ChapterRead {
    /// 1-based chapter ordinal in reading order.
    pub chapter: u32,
    pub title: String,
    /// The dramatic intensity measured from this chapter's prose (LR-P1), 0..=1;
    /// `None` when it can't be measured (empty chapter / unsupported language).
    pub measured_intensity: Option<f32>,
    /// Entities the reader meets *for the first time* here.
    pub new_entities: Vec<String>,
    /// Threads (setups / questions / goals) opened here.
    pub opened_threads: Vec<String>,
    /// Threads paid off here.
    pub resolved_threads: Vec<String>,
    /// The reader findings that land in this chapter.
    pub findings: Vec<ReaderFinding>,
}

/// The whole forward read: the per-chapter reads plus the shape curve (one
/// `(expected, measured)` intensity pair per chapter, for the sparkline).
#[derive(Debug, Clone, Default)]
pub(crate) struct ReadThrough {
    pub chapters: Vec<ChapterRead>,
    pub curve: Vec<(f32, f32)>,
}

impl ReadThrough {
    /// Every finding across every chapter, ranked most-serious-first (then by
    /// chapter) and deduped. The report / dashboard / review pass consume this.
    pub(crate) fn ranked_findings(&self) -> Vec<ReaderFinding> {
        let mut all: Vec<ReaderFinding> =
            self.chapters.iter().flat_map(|c| c.findings.iter().cloned()).collect();
        rank(&mut all);
        dedupe(all)
    }
}

/// Fold findings that share a `dedup_key`, keeping the first. Callers [`rank`]
/// first, so the survivor is the most-serious of the group. Pure.
pub(crate) fn dedupe(findings: Vec<ReaderFinding>) -> Vec<ReaderFinding> {
    let mut seen: HashSet<String> = HashSet::new();
    findings.into_iter().filter(|f| seen.insert(f.dedup_key.clone())).collect()
}

/// Rank findings most-serious first, then by chapter ascending. Pure.
pub(crate) fn rank(findings: &mut [ReaderFinding]) {
    findings.sort_by(|a, b| {
        b.severity.rank().cmp(&a.severity.rank()).then(a.chapter.cmp(&b.chapter))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(kind: &'static str, sev: Severity, chapter: u32, entities: &[&str]) -> ReaderFinding {
        let entities: Vec<String> = entities.iter().map(|e| e.to_string()).collect();
        ReaderFinding {
            kind,
            severity: sev,
            chapter,
            anchor: None,
            dedup_key: ReaderFinding::make_dedup_key(kind, &entities, chapter),
            entities,
            message: String::new(),
            source: "walk",
        }
    }

    #[test]
    fn dedup_key_is_order_and_case_insensitive() {
        let a = ReaderFinding::make_dedup_key("confusion", &["Mara".into(), "Joren".into()], 3);
        let b = ReaderFinding::make_dedup_key("confusion", &["joren".into(), "mara".into()], 3);
        assert_eq!(a, b);
        assert_ne!(a, ReaderFinding::make_dedup_key("confusion", &["Mara".into()], 3));
        assert_ne!(a, ReaderFinding::make_dedup_key("info_dump", &["Mara".into(), "Joren".into()], 3));
    }

    #[test]
    fn dedupe_folds_same_problem() {
        let findings = vec![
            f("confusion", Severity::Concern, 3, &["Aldous"]),
            f("confusion", Severity::Info, 3, &["aldous"]),
            f("attention_dip", Severity::Info, 5, &[]),
        ];
        assert_eq!(dedupe(findings).len(), 2);
    }

    #[test]
    fn rank_orders_most_serious_then_chapter() {
        let mut findings = vec![
            f("attention_dip", Severity::Info, 2, &["a"]),
            f("put_down_risk", Severity::Concern, 9, &["b"]),
            f("unpaid_setup", Severity::Notice, 1, &["c"]),
        ];
        rank(&mut findings);
        assert_eq!(findings[0].severity, Severity::Concern);
        assert_eq!(findings[1].severity, Severity::Notice);
        assert_eq!(findings[2].severity, Severity::Info);
    }

    #[test]
    fn read_through_ranks_and_dedupes_across_chapters() {
        let rt = ReadThrough {
            chapters: vec![
                ChapterRead {
                    chapter: 2,
                    findings: vec![f("attention_dip", Severity::Info, 2, &[])],
                    ..Default::default()
                },
                ChapterRead {
                    chapter: 5,
                    findings: vec![
                        f("put_down_risk", Severity::Concern, 5, &[]),
                        // a duplicate report of the same dip, folded away
                        f("attention_dip", Severity::Info, 2, &[]),
                    ],
                    ..Default::default()
                },
            ],
            curve: Vec::new(),
        };
        let ranked = rt.ranked_findings();
        assert_eq!(ranked.len(), 2, "the duplicate attention_dip is folded");
        assert_eq!(ranked[0].kind, "put_down_risk", "most serious first");
    }
}
