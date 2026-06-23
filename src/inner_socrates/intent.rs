//! The **intent ledger** — the author's declared deliberate choices about their
//! prose (ambiguity, framing, structural echo, style, temporality). It is the
//! direct counterpart of WORLD-4's *magic ledger*: same vocabulary (Entry, Kind,
//! Coverage, Scope, lazy Consultation, Suppression, Promotion), same shape. Where
//! the magic ledger excuses physical impossibilities in a simulated world, the
//! intent ledger excuses Socratic findings the author has consciously chosen.
//!
//! Consultation is **lazy**: a candidate finding is checked against the ledger
//! only after it is generated; a matching entry suppresses it with a note.

use super::types::Category;

/// The typed kinds of declared intent (controlled vocabulary). `CustomBund` is
/// reserved for a future RFC and is never constructed in the MVP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentKind {
    DeliberateAmbiguity,
    FramingChoice,
    StructuralEcho,
    StylisticChoice,
    DeliberateTemporalAmbiguity,
}

impl IntentKind {
    pub fn id(&self) -> &'static str {
        match self {
            IntentKind::DeliberateAmbiguity => "deliberate_ambiguity",
            IntentKind::FramingChoice => "framing_choice",
            IntentKind::StructuralEcho => "structural_echo",
            IntentKind::StylisticChoice => "stylistic_choice",
            IntentKind::DeliberateTemporalAmbiguity => "deliberate_temporal_ambiguity",
        }
    }

    pub fn from_id(s: &str) -> Option<IntentKind> {
        Some(match s {
            "deliberate_ambiguity" => IntentKind::DeliberateAmbiguity,
            "framing_choice" => IntentKind::FramingChoice,
            "structural_echo" => IntentKind::StructuralEcho,
            "stylistic_choice" => IntentKind::StylisticChoice,
            "deliberate_temporal_ambiguity" => IntentKind::DeliberateTemporalAmbiguity,
            _ => return None,
        })
    }
}

/// Whether an entry stays in this project or is exportable to a `.isl` series
/// bundle (forward-compatible from MVP).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeLevel {
    Project,
    Series,
}

/// Where an intent entry applies. Matched against a candidate finding's
/// [`FindingContext`]. Paragraph and timeline ranges compare lexicographically, so
/// ids/points must be zero-padded comparably (e.g. `ch07-p042`, `1A.092`).
#[derive(Debug, Clone, PartialEq)]
pub enum IntentScope {
    /// Applies project-wide.
    Project,
    Chapter(String),
    ParagraphRange { from: String, to: String },
    Character(String),
    Scene(String),
    /// New in this RFC — a span of world-time (consumed by the timeline categories).
    TimelineRange { from: String, to: String },
}

/// The locus a candidate finding carries, matched against an entry's scope. All
/// fields optional — a finding need only populate what it knows.
#[derive(Debug, Clone, Default)]
pub struct FindingContext {
    pub paragraph_id: Option<String>,
    pub chapter_id: Option<String>,
    pub character_ids: Vec<String>,
    pub scene: Option<String>,
    /// A world-time point (e.g. `"1A.092"`), for timeline-scoped entries.
    pub timeline_point: Option<String>,
}

impl IntentScope {
    /// Does this scope cover the given finding context?
    pub fn applies_to(&self, ctx: &FindingContext) -> bool {
        match self {
            IntentScope::Project => true,
            IntentScope::Chapter(c) => ctx.chapter_id.as_deref() == Some(c.as_str()),
            IntentScope::ParagraphRange { from, to } => ctx
                .paragraph_id
                .as_deref()
                .is_some_and(|p| in_range(p, from, to)),
            IntentScope::Character(id) => ctx.character_ids.iter().any(|c| c == id),
            IntentScope::Scene(s) => ctx.scene.as_deref() == Some(s.as_str()),
            IntentScope::TimelineRange { from, to } => ctx
                .timeline_point
                .as_deref()
                .is_some_and(|t| in_range(t, from, to)),
        }
    }
}

/// Inclusive lexicographic range test (ids must be comparably padded).
fn in_range(value: &str, from: &str, to: &str) -> bool {
    let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
    value >= lo && value <= hi
}

/// One declared intention.
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

/// The outcome of consulting the ledger for a candidate finding.
#[derive(Debug, Clone, PartialEq)]
pub enum ConsultationResult {
    /// No declared intent applies — let the finding through.
    Emit,
    /// A declared intent covers it — suppress, with a note for the snapshot log.
    Suppress { entry_id: String, note: String },
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
    fn project_scope_applies_everywhere() {
        let s = IntentScope::Project;
        assert!(s.applies_to(&FindingContext::default()));
    }

    #[test]
    fn chapter_scope_matches_chapter() {
        let s = IntentScope::Chapter("ch07".into());
        let yes = FindingContext { chapter_id: Some("ch07".into()), ..Default::default() };
        let no = FindingContext { chapter_id: Some("ch08".into()), ..Default::default() };
        assert!(s.applies_to(&yes));
        assert!(!s.applies_to(&no));
    }

    #[test]
    fn paragraph_range_is_inclusive() {
        let s = IntentScope::ParagraphRange { from: "ch07-p042".into(), to: "ch07-p051".into() };
        let inside = FindingContext { paragraph_id: Some("ch07-p045".into()), ..Default::default() };
        let edge = FindingContext { paragraph_id: Some("ch07-p051".into()), ..Default::default() };
        let outside = FindingContext { paragraph_id: Some("ch07-p052".into()), ..Default::default() };
        assert!(s.applies_to(&inside));
        assert!(s.applies_to(&edge));
        assert!(!s.applies_to(&outside));
    }

    #[test]
    fn timeline_range_matches() {
        let s = IntentScope::TimelineRange { from: "1A.090".into(), to: "1A.095".into() };
        let inside = FindingContext { timeline_point: Some("1A.092".into()), ..Default::default() };
        let outside = FindingContext { timeline_point: Some("1A.096".into()), ..Default::default() };
        assert!(s.applies_to(&inside));
        assert!(!s.applies_to(&outside));
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

        // Covered category, in scope → suppressed with a note.
        match ledger.consult(Category::AssumptionSurfacing, &in_ch07) {
            ConsultationResult::Suppress { entry_id, note } => {
                assert_eq!(entry_id, "e1");
                assert!(note.contains("declared intent"));
            }
            other => panic!("expected Suppress, got {other:?}"),
        }
        // Covered category, out of scope → emit.
        assert_eq!(ledger.consult(Category::AssumptionSurfacing, &in_ch08), ConsultationResult::Emit);
        // In scope but category not covered → emit.
        assert_eq!(ledger.consult(Category::FramingInterrogation, &in_ch07), ConsultationResult::Emit);
    }

    #[test]
    fn intent_kind_roundtrips() {
        for k in [
            IntentKind::DeliberateAmbiguity,
            IntentKind::FramingChoice,
            IntentKind::StructuralEcho,
            IntentKind::StylisticChoice,
            IntentKind::DeliberateTemporalAmbiguity,
        ] {
            assert_eq!(IntentKind::from_id(k.id()), Some(k.clone()));
        }
        assert_eq!(IntentKind::from_id("nope"), None);
    }
}
