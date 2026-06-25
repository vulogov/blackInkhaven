//! IE-P2 — consult the (shared) intent ledger to suppress Editor findings the
//! author has already declared deliberate. Reuses INNER_SOCRATES-1's ledger:
//! the raw rows (coverage as strings) + `IntentScope::applies_to`, matched
//! against the Editor's own category ids. Pure given the rows + context.

use crate::inner_socrates::intent::FindingContext;
use crate::inner_socrates::storage::RawIntentRow;

use super::types::EditorFinding;

/// Partition findings into (kept, suppressed). A finding is suppressed when some
/// ledger entry's `coverage` names its category id AND the entry's scope applies
/// to `ctx`; the suppressed finding carries the declaring entry's description as
/// its note (lazy consultation, same shape as Inner Socrates / WORLD-4).
pub fn consult(
    findings: Vec<EditorFinding>,
    rows: &[RawIntentRow],
    ctx: &FindingContext,
) -> (Vec<EditorFinding>, Vec<EditorFinding>) {
    let mut kept = Vec::new();
    let mut suppressed = Vec::new();
    for mut f in findings {
        let hit = rows.iter().find(|r| {
            r.coverage.iter().any(|c| c == f.category.id()) && r.scope.applies_to(ctx)
        });
        match hit {
            Some(r) => {
                f.suppressed_by =
                    Some(format!("consistent with declared intent: {}", r.description));
                suppressed.push(f);
            }
            None => kept.push(f),
        }
    }
    (kept, suppressed)
}

/// A compact human summary of the ledger entries that *could* apply, for the
/// prompt (so the model doesn't re-raise declared choices in the first place).
pub fn intent_summary(rows: &[RawIntentRow]) -> String {
    if rows.is_empty() {
        return "None.".to_string();
    }
    rows.iter()
        .map(|r| format!("- {}: {} (covers: {})", r.kind, r.description, r.coverage.join(", ")))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inner_socrates::intent::IntentScope;
    use crate::inner_editor::types::{EditorCategory, EditorSeverity};

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

    fn row(coverage: &[&str], scope: IntentScope) -> RawIntentRow {
        RawIntentRow {
            kind: "deliberate_repetition".into(),
            description: "the silence motif is intentional".into(),
            scope,
            coverage: coverage.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn suppresses_covered_category_in_scope() {
        let rows = vec![row(&["dictionary_richness"], IntentScope::Chapter("ch07".into()))];
        let ctx = FindingContext { chapter_id: Some("ch07".into()), ..Default::default() };
        let (kept, supp) = consult(
            vec![finding(EditorCategory::DictionaryRichness), finding(EditorCategory::Tautology)],
            &rows,
            &ctx,
        );
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].category, EditorCategory::Tautology);
        assert_eq!(supp.len(), 1);
        assert!(supp[0].suppressed_by.as_deref().unwrap().contains("silence motif"));
    }

    #[test]
    fn does_not_suppress_out_of_scope() {
        let rows = vec![row(&["dictionary_richness"], IntentScope::Chapter("ch07".into()))];
        // Different chapter → scope doesn't apply.
        let ctx = FindingContext { chapter_id: Some("ch09".into()), ..Default::default() };
        let (kept, supp) = consult(vec![finding(EditorCategory::DictionaryRichness)], &rows, &ctx);
        assert_eq!(kept.len(), 1);
        assert!(supp.is_empty());
    }

    #[test]
    fn project_scope_applies_everywhere() {
        let rows = vec![row(&["style_instability"], IntentScope::Project)];
        let ctx = FindingContext::default();
        let (kept, supp) = consult(vec![finding(EditorCategory::StyleInstability)], &rows, &ctx);
        assert!(kept.is_empty());
        assert_eq!(supp.len(), 1);
    }
}
