//! Optional LLM elaboration of pattern-detected critique findings
//! (TIMELINE-2-INTEGRATION P2).
//!
//! Orphan and fuzzy-overlap findings are detected purely (no network). When a
//! provider is configured and `timeline.critique.elaboration.enabled` is set, each
//! finding may get a short LLM elaboration explaining *why* it's worth a look — for
//! an orphan, why the event seems significant; for an overlap, which scenes most
//! likely conflict. When the LLM is absent, disabled, or the per-run cap is spent,
//! findings keep their pattern-detected reason text (graceful degradation).
//!
//! Everything here is pure and provider-free: the prompt builders + the
//! [`ElaborationBudget`] gate are testable without a model, and [`elaborate`] takes
//! the completion as a closure. The live synchronous call (cost preflight, daily
//! cap, retry) is wired where the other slow-track callers live — the CLI (P3).

use crate::config::TimelineElaborationConfig;

/// The shortened system prompt — the focused replacement for the legacy
/// `05-timeline-health-example.typ`, scoped to the two retained categories only.
pub const ELABORATION_SYSTEM: &str = "\
You are reviewing specific timeline issues for elaboration. You are given one \
already-detected issue about a fictional world's timeline. Do not invent missing \
facts. In two or three sentences:\n\
- If it is an orphaned event: explain why this event might be significant given its \
title and date, and suggest concretely how it could be integrated (linked to a \
paragraph, a character, or a place) — or whether it reads as deliberate backstory.\n\
- If it is a fuzzy-precision overlap: explain which scenes or moments most likely \
conflict, and the most natural resolution (refine one event's precision, mark them \
concurrent, or accept it as deliberate).\n\
Be specific and brief. End with one short suggested action.";

/// Per-run elaboration budget. Caps the number of LLM calls a single critique run
/// may make; `confirm_above` is the soft threshold past which the caller should ask
/// the user before continuing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElaborationBudget {
    enabled: bool,
    remaining: usize,
    pub confirm_above: usize,
}

impl ElaborationBudget {
    /// Build from config + whether a provider is actually available. With no LLM,
    /// the budget is disabled (everything falls back to pattern-only).
    pub fn from_config(cfg: &TimelineElaborationConfig, have_llm: bool) -> Self {
        ElaborationBudget {
            enabled: cfg.enabled && have_llm,
            remaining: cfg.max_calls_per_run,
            confirm_above: cfg.confirm_above_calls,
        }
    }

    /// A disabled budget (pattern-only).
    pub fn off() -> Self {
        ElaborationBudget { enabled: false, remaining: 0, confirm_above: 0 }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled && self.remaining > 0
    }

    pub fn remaining(&self) -> usize {
        self.remaining
    }

    /// Whether elaborating `n` findings would cross the confirm threshold.
    pub fn needs_confirmation(&self, n: usize) -> bool {
        self.enabled && self.confirm_above > 0 && n > self.confirm_above
    }

    /// Consume one call from the budget; `false` when disabled or exhausted.
    fn take(&mut self) -> bool {
        if self.is_enabled() {
            self.remaining -= 1;
            true
        } else {
            false
        }
    }
}

/// Build the elaboration user prompt for an orphan finding.
pub fn orphan_prompt(title: &str, date_label: &str, track: &str, reasons: &[String]) -> String {
    format!(
        "Issue: orphaned timeline event.\n\
         Event: \"{title}\" ({date_label}, \"{track}\" track).\n\
         Detected because: {}\n\
         Elaborate per the instructions.",
        reasons.join(" ")
    )
}

/// Build the elaboration user prompt for a fuzzy-overlap finding.
pub fn overlap_prompt(
    titles: &[String],
    window_label: &str,
    is_cluster: bool,
    reasons: &[String],
) -> String {
    let kind = if is_cluster { "cluster overlap" } else { "pairwise overlap" };
    format!(
        "Issue: fuzzy-precision timeline {kind}.\n\
         Events: {}.\n\
         Overlap window: {window_label}.\n\
         Detected because: {}\n\
         Elaborate per the instructions.",
        titles.join("; "),
        reasons.join(" ")
    )
}

/// Run one elaboration through the budget. Returns the model's text, or `None` when
/// the budget is spent/disabled or the completion fails (→ pattern-only fallback).
/// `complete` is the provider call (the caller injects the live or a stub one).
pub fn elaborate<F>(prompt: &str, budget: &mut ElaborationBudget, mut complete: F) -> Option<String>
where
    F: FnMut(&str) -> Option<String>,
{
    if !budget.take() {
        return None;
    }
    complete(prompt).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(enabled: bool, max: usize, confirm: usize) -> TimelineElaborationConfig {
        TimelineElaborationConfig { enabled, max_calls_per_run: max, confirm_above_calls: confirm }
    }

    #[test]
    fn budget_disabled_without_llm() {
        let b = ElaborationBudget::from_config(&cfg(true, 20, 10), false);
        assert!(!b.is_enabled());
    }

    #[test]
    fn budget_caps_calls() {
        let mut b = ElaborationBudget::from_config(&cfg(true, 2, 10), true);
        let mut calls = 0;
        let mut run = || {
            elaborate("p", &mut b, |_| {
                calls += 1;
                Some("ok".into())
            })
        };
        assert_eq!(run(), Some("ok".into()));
        assert_eq!(run(), Some("ok".into()));
        // Third request is over cap → None, closure not invoked.
        assert_eq!(run(), None);
        assert_eq!(calls, 2);
    }

    #[test]
    fn disabled_budget_is_pattern_only() {
        let mut b = ElaborationBudget::from_config(&cfg(false, 20, 10), true);
        let mut invoked = false;
        let out = elaborate("p", &mut b, |_| {
            invoked = true;
            Some("x".into())
        });
        assert_eq!(out, None);
        assert!(!invoked, "completion must not run when elaboration is disabled");
    }

    #[test]
    fn empty_completion_falls_back() {
        let mut b = ElaborationBudget::from_config(&cfg(true, 5, 10), true);
        assert_eq!(elaborate("p", &mut b, |_| Some("   ".into())), None);
        assert_eq!(elaborate("p", &mut b, |_| None), None);
    }

    #[test]
    fn needs_confirmation_past_threshold() {
        let b = ElaborationBudget::from_config(&cfg(true, 20, 10), true);
        assert!(!b.needs_confirmation(10));
        assert!(b.needs_confirmation(11));
    }

    #[test]
    fn prompts_carry_the_essentials() {
        let op = orphan_prompt("Coronation", "year 120", "main", &["Orphaned for 92 days.".into()]);
        assert!(op.contains("Coronation"));
        assert!(op.contains("year 120"));
        assert!(op.contains("92 days"));
        let fp = overlap_prompt(
            &["Training".into(), "Journey".into()],
            "summer 87",
            false,
            &["They share a character.".into()],
        );
        assert!(fp.contains("Training"));
        assert!(fp.contains("pairwise overlap"));
        assert!(fp.contains("summer 87"));
    }
}
