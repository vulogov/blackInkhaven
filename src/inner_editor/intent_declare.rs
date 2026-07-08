//! IE-P6 — record-as-intent: declare an Editor observation deliberate, so the
//! ledger suppresses that category going forward. Writes into the **shared**
//! intent ledger (`inner_socrates.db`) with the Editor's category id as raw
//! string coverage (via `InnerSocratesStore::add_intent_raw`), under a style
//! intent kind. The consultation (`intent_consult::consult`) then suppresses
//! matching findings — the loop is symmetric with Inner Socrates' promotion.

use std::path::Path;

use anyhow::Result;
use uuid::Uuid;

use crate::intent::{IntentKind, IntentScope, ScopeLevel};
use crate::inner_socrates::storage::InnerSocratesStore;

use super::types::EditorCategory;

/// The intent kind that naturally declares a category deliberate.
pub fn proposed_intent_kind(category: EditorCategory) -> IntentKind {
    match category {
        EditorCategory::DictionaryRichness => IntentKind::DeliberateRepetition,
        EditorCategory::Tautology => IntentKind::DeliberateTautology,
        EditorCategory::StyleInstability => IntentKind::VoiceInstabilityIntentional,
        EditorCategory::BeliefStance => IntentKind::ProseBeliefIntentionalDistance,
        // literary_richness / style_observation / craft_praise / editorial_suggestions
        _ => IntentKind::StylisticPatternDeliberate,
    }
}

/// Declare `category` a deliberate choice — project-wide, or scoped to a chapter
/// when `chapter` is given. Future Editor findings of that category in scope are
/// suppressed (with the declaring entry's note).
pub fn declare_intent(
    project: &Path,
    category: EditorCategory,
    chapter: Option<&str>,
    description: Option<&str>,
) -> Result<()> {
    let store = InnerSocratesStore::open_for_project(project)?;
    let scope = match chapter {
        Some(c) => IntentScope::Chapter(c.to_string()),
        None => IntentScope::Project,
    };
    let desc = description
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{} is a deliberate choice here", category.label()));
    store.add_intent_raw(
        &Uuid::new_v4().to_string(),
        &proposed_intent_kind(category),
        &desc,
        &scope,
        &[category.id().to_string()],
        ScopeLevel::Project,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inner_editor::intent_consult::consult;
    use crate::inner_editor::types::{EditorFinding, EditorSeverity};
    use crate::intent::FindingContext;

    fn finding(cat: EditorCategory) -> EditorFinding {
        EditorFinding {
            category: cat,
            severity: EditorSeverity::Note,
            observation: "x".into(),
            observation_en: "x".into(),
            evidence: None,
            conditional: true,
            suppressed_by: None,
        }
    }

    #[test]
    fn declared_intent_suppresses_the_category_in_scope() {
        let dir = tempfile::tempdir().unwrap();
        // Declaring writes into the shared inner_socrates.db ledger.
        declare_intent(dir.path(), EditorCategory::Tautology, Some("ch04"), None).unwrap();

        let store = InnerSocratesStore::open_for_project(dir.path()).unwrap();
        let rows = store.list_intent_rows_raw().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "deliberate_tautology");
        assert!(rows[0].coverage.iter().any(|c| c == "tautology"));

        // In ch04, a tautology finding is now suppressed; elsewhere it isn't.
        let ctx04 = FindingContext { chapter_id: Some("ch04".into()), ..Default::default() };
        let (kept, supp) = consult(vec![finding(EditorCategory::Tautology)], &rows, &ctx04);
        assert!(kept.is_empty());
        assert_eq!(supp.len(), 1);

        let ctx09 = FindingContext { chapter_id: Some("ch09".into()), ..Default::default() };
        let (kept, _) = consult(vec![finding(EditorCategory::Tautology)], &rows, &ctx09);
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn category_kind_mapping_is_stable() {
        assert_eq!(proposed_intent_kind(EditorCategory::DictionaryRichness).id(), "deliberate_repetition");
        assert_eq!(proposed_intent_kind(EditorCategory::StyleInstability).id(), "voice_instability_intentional");
        assert_eq!(proposed_intent_kind(EditorCategory::BeliefStance).id(), "prose_belief_intentional_distance");
        assert_eq!(proposed_intent_kind(EditorCategory::StyleObservation).id(), "stylistic_pattern_deliberate");
    }
}
