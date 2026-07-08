//! Inner Socrates' **typed** intent layer, over the shared [`crate::intent`] core.
//!
//! The neutral model (kinds, scopes, contexts, the raw-row form, and the
//! `consult_raw` suppression rule) now lives in `crate::intent` and is shared with
//! the Inner Editor and the Terms overlay. This module adds only the pieces that
//! are Socrates-specific because they speak in Socratic [`Category`]s: the typed
//! [`IntentEntry`] / [`IntentLedger`], and the promotion mapping. The shared types
//! are re-exported so existing `inner_socrates::intent::*` paths keep resolving.

use super::types::Category;

pub use crate::intent::{
    consult_raw, ConsultationResult, FindingContext, IntentKind, IntentScope, RawIntentRow,
    ScopeLevel,
};

impl IntentKind {
    /// The intent kind a promotion suggestion proposes for a dismissed category —
    /// the natural declaration that would suppress that category.
    pub fn proposed_for(category: Category) -> IntentKind {
        match category {
            Category::FramingInterrogation => IntentKind::FramingChoice,
            Category::StructuralPatterns
            | Category::ImplicitComparison
            | Category::SentenceLengthAnomalies => IntentKind::StructuralEcho,
            Category::DramatizationGap
            | Category::ImplicationTracing
            | Category::TemporalDensity => IntentKind::DeliberateTemporalAmbiguity,
            Category::AssumptionSurfacing | Category::TensionDetection => {
                IntentKind::DeliberateAmbiguity
            }
            _ => IntentKind::StylisticChoice,
        }
    }
}

/// One declared intention, with coverage as typed Socratic categories.
#[derive(Debug, Clone)]
pub struct IntentEntry {
    pub id: String,
    pub kind: IntentKind,
    /// The author's prose explanation (shown in the suppression note).
    pub description: String,
    pub scope: IntentScope,
    /// Which Socratic categories this entry may suppress.
    pub coverage: Vec<Category>,
    pub scope_level: ScopeLevel,
}

impl IntentEntry {
    /// Does this entry cover `category` *and* apply to `ctx`?
    pub fn matches(&self, category: Category, ctx: &FindingContext) -> bool {
        self.coverage.contains(&category) && self.scope.applies_to(ctx)
    }
}

/// The project's declared intentions.
#[derive(Debug, Clone, Default)]
pub struct IntentLedger {
    pub entries: Vec<IntentEntry>,
}

impl IntentLedger {
    /// Lazily consult the ledger for a candidate `(category, context)`. Returns the
    /// first matching entry's suppression, else `Emit`. Pure; no I/O.
    pub fn consult(&self, category: Category, ctx: &FindingContext) -> ConsultationResult {
        match self.entries.iter().find(|e| e.matches(category, ctx)) {
            Some(e) => ConsultationResult::Suppress {
                entry_id: e.id.clone(),
                note: format!("consistent with declared intent: {}", e.description),
            },
            None => ConsultationResult::Emit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(scope: IntentScope, coverage: Vec<Category>) -> IntentEntry {
        IntentEntry {
            id: "e1".into(),
            kind: IntentKind::DeliberateAmbiguity,
            description: "Mara's loyalty is intentionally unresolved".into(),
            scope,
            coverage,
            scope_level: ScopeLevel::Project,
        }
    }

    #[test]
    fn consult_suppresses_covered_in_scope_else_emits() {
        let ledger = IntentLedger {
            entries: vec![entry(
                IntentScope::Chapter("ch07".into()),
                vec![Category::AssumptionSurfacing, Category::TensionDetection],
            )],
        };
        let in_ch07 = FindingContext { chapter_id: Some("ch07".into()), ..Default::default() };
        let in_ch08 = FindingContext { chapter_id: Some("ch08".into()), ..Default::default() };

        match ledger.consult(Category::AssumptionSurfacing, &in_ch07) {
            ConsultationResult::Suppress { entry_id, note } => {
                assert_eq!(entry_id, "e1");
                assert!(note.contains("declared intent"));
            }
            other => panic!("expected Suppress, got {other:?}"),
        }
        // Out of scope → emit.
        assert_eq!(ledger.consult(Category::AssumptionSurfacing, &in_ch08), ConsultationResult::Emit);
        // In scope but category not covered → emit.
        assert_eq!(ledger.consult(Category::FramingInterrogation, &in_ch07), ConsultationResult::Emit);
    }
}
