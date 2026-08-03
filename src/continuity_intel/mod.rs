//! SENTINEL-1 — Continuity Intelligence: "the book watches itself".
//!
//! Unifies the deterministic continuity detectors already in the tree
//! (co-location, timeline critique, numeric contradiction, per-character fact
//! drift) into one normalised finding, adds the *referenced-before-introduced*
//! invariant (CT-P1), and watches incrementally as you write (CT-P5). It does
//! not re-implement detection — it orchestrates. See
//! `Documentation/PROPOSALS/SENTINEL-1_PLAN.md`.
//!
//! CT-P0 lands the shared vocabulary: [`ContinuityFinding`] + [`Severity`] + the
//! dedup/rank primitives. CT-P1 adds the first detector — [`introduce`], the
//! referenced-before-introduced invariant. CT-P2's [`engine`] composes them:
//! it fans out to every deterministic detector, normalises, ranks, and dedupes.

use std::collections::HashSet;

use uuid::Uuid;

pub(crate) mod engine;
pub(crate) mod introduce;
pub(crate) mod watch;

/// How serious a continuity finding is. `Contradiction` is a hard clash (a
/// character in two places); `Warning` a likely problem; `Info` a nudge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    Info,
    Warning,
    Contradiction,
}

impl Severity {
    /// Higher = more severe (for ranking).
    pub(crate) fn rank(self) -> u8 {
        match self {
            Severity::Info => 0,
            Severity::Warning => 1,
            Severity::Contradiction => 2,
        }
    }
    pub(crate) fn label(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Contradiction => "contradiction",
        }
    }
}

/// One continuity finding, normalised across every detector so the ledger, the
/// review pass, and the dashboard speak one shape.
#[derive(Debug, Clone)]
pub(crate) struct ContinuityFinding {
    /// The kind of break: `co_location` | `timeline` | `numeric` | `char_fact` |
    /// `introduce` | `coherence` | `drift` | …
    pub kind: &'static str,
    pub severity: Severity,
    /// 1-based chapter (`0` = book-level / unknown).
    pub chapter: u32,
    /// The paragraph to jump to, if the detector knows one.
    pub anchor: Option<Uuid>,
    /// The entities the finding is about (character/place names) — for dedup and
    /// display.
    pub entities: Vec<String>,
    pub message: String,
    /// Which detector produced it (provenance + a per-source trust filter).
    pub source: &'static str,
    /// A stable fingerprint used to fold duplicate reports of the same break
    /// (e.g. a co-location and a travel-time complaint about the same pair).
    pub dedup_key: String,
}

impl ContinuityFinding {
    /// A conservative dedup key: kind + the entity set (order- and
    /// case-insensitive) + chapter. Widen later if two detectors describe the
    /// same break under different kinds (an open decision in the RFC).
    pub(crate) fn make_dedup_key(kind: &str, entities: &[String], chapter: u32) -> String {
        let mut es: Vec<String> = entities.iter().map(|e| e.to_lowercase()).collect();
        es.sort();
        format!("{kind}|{}|{chapter}", es.join(","))
    }
}

/// Fold findings that share a `dedup_key`, keeping the first. Callers [`rank`]
/// first, so the survivor is the most-severe of the group. Pure.
pub(crate) fn dedupe(findings: Vec<ContinuityFinding>) -> Vec<ContinuityFinding> {
    let mut seen: HashSet<String> = HashSet::new();
    findings.into_iter().filter(|f| seen.insert(f.dedup_key.clone())).collect()
}

/// Rank findings most-severe first, then by chapter ascending. Pure.
pub(crate) fn rank(findings: &mut [ContinuityFinding]) {
    findings.sort_by(|a, b| {
        b.severity.rank().cmp(&a.severity.rank()).then(a.chapter.cmp(&b.chapter))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(kind: &'static str, sev: Severity, chapter: u32, entities: &[&str]) -> ContinuityFinding {
        let entities: Vec<String> = entities.iter().map(|e| e.to_string()).collect();
        ContinuityFinding {
            kind,
            severity: sev,
            chapter,
            anchor: None,
            dedup_key: ContinuityFinding::make_dedup_key(kind, &entities, chapter),
            entities,
            message: String::new(),
            source: "test",
        }
    }

    #[test]
    fn dedup_key_is_order_and_case_insensitive() {
        let a = ContinuityFinding::make_dedup_key("co_location", &["Mara".into(), "Joren".into()], 3);
        let b = ContinuityFinding::make_dedup_key("co_location", &["joren".into(), "mara".into()], 3);
        assert_eq!(a, b);
        // A different chapter or kind is a different key.
        assert_ne!(a, ContinuityFinding::make_dedup_key("co_location", &["Mara".into(), "Joren".into()], 4));
        assert_ne!(a, ContinuityFinding::make_dedup_key("timeline", &["Mara".into(), "Joren".into()], 3));
    }

    #[test]
    fn dedupe_folds_same_break_from_two_detectors() {
        // Two detectors report the same Mara↔Joren clash in ch.3.
        let findings = vec![
            f("co_location", Severity::Contradiction, 3, &["Mara", "Joren"]),
            f("co_location", Severity::Warning, 3, &["joren", "mara"]),
            f("numeric", Severity::Info, 5, &["clock"]),
        ];
        let out = dedupe(findings);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn rank_orders_most_severe_then_chapter() {
        let mut findings = vec![
            f("numeric", Severity::Info, 2, &["a"]),
            f("co_location", Severity::Contradiction, 9, &["b"]),
            f("timeline", Severity::Warning, 1, &["c"]),
        ];
        rank(&mut findings);
        assert_eq!(findings[0].severity, Severity::Contradiction);
        assert_eq!(findings[1].severity, Severity::Warning);
        assert_eq!(findings[2].severity, Severity::Info);
    }
}
