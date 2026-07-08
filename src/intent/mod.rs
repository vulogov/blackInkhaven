//! INTENT-1 — the shared **intent ledger** core: the author's declared deliberate
//! choices about their prose, and the lazy-consultation rule that suppresses a
//! finding a declared intent covers.
//!
//! This is the neutral home for the model that Inner Socrates, the Inner Editor,
//! and the Terms/glossary overlay all share. It carries no dependency on any one
//! reader — coverage is expressed as plain category-id strings ([`RawIntentRow`]),
//! and [`consult_raw`] is the single suppression predicate they all consult. Inner
//! Socrates layers its typed `Category` ledger on top of these types.
//!
//! The counterpart of WORLD-4's *magic ledger*: same vocabulary (Entry, Kind,
//! Coverage, Scope, lazy Consultation, Suppression). Where the magic ledger
//! excuses physical impossibilities in a simulated world, the intent ledger
//! excuses findings the author has consciously chosen.

/// The typed kinds of declared intent (controlled vocabulary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentKind {
    DeliberateAmbiguity,
    FramingChoice,
    StructuralEcho,
    StylisticChoice,
    DeliberateTemporalAmbiguity,
    // INNER_EDITOR-1 — additive kinds for declared *style* intentions the Editor
    // should respect (the ledger is the shared examined-authorship surface).
    DeliberateRepetition,
    DeliberateTautology,
    VoiceInstabilityIntentional,
    ProseBeliefIntentionalDistance,
    StylisticPatternDeliberate,
    // TERMS-1 (1.4.8+) — a deliberately-kept terminology variant (suppresses the
    // banned-synonym overlay for one canonical term).
    DeliberateVariant,
}

impl IntentKind {
    pub fn id(&self) -> &'static str {
        match self {
            IntentKind::DeliberateAmbiguity => "deliberate_ambiguity",
            IntentKind::FramingChoice => "framing_choice",
            IntentKind::StructuralEcho => "structural_echo",
            IntentKind::StylisticChoice => "stylistic_choice",
            IntentKind::DeliberateTemporalAmbiguity => "deliberate_temporal_ambiguity",
            IntentKind::DeliberateRepetition => "deliberate_repetition",
            IntentKind::DeliberateTautology => "deliberate_tautology",
            IntentKind::VoiceInstabilityIntentional => "voice_instability_intentional",
            IntentKind::ProseBeliefIntentionalDistance => "prose_belief_intentional_distance",
            IntentKind::StylisticPatternDeliberate => "stylistic_pattern_deliberate",
            IntentKind::DeliberateVariant => "deliberate_variant",
        }
    }

    pub fn from_id(s: &str) -> Option<IntentKind> {
        Some(match s {
            "deliberate_ambiguity" => IntentKind::DeliberateAmbiguity,
            "framing_choice" => IntentKind::FramingChoice,
            "structural_echo" => IntentKind::StructuralEcho,
            "stylistic_choice" => IntentKind::StylisticChoice,
            "deliberate_temporal_ambiguity" => IntentKind::DeliberateTemporalAmbiguity,
            "deliberate_repetition" => IntentKind::DeliberateRepetition,
            "deliberate_tautology" => IntentKind::DeliberateTautology,
            "voice_instability_intentional" => IntentKind::VoiceInstabilityIntentional,
            "prose_belief_intentional_distance" => IntentKind::ProseBeliefIntentionalDistance,
            "stylistic_pattern_deliberate" => IntentKind::StylisticPatternDeliberate,
            "deliberate_variant" => IntentKind::DeliberateVariant,
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
    /// A span of world-time (consumed by the timeline categories).
    TimelineRange { from: String, to: String },
}

impl IntentScope {
    /// Does this scope cover the given finding context?
    pub fn applies_to(&self, ctx: &FindingContext) -> bool {
        match self {
            IntentScope::Project => true,
            IntentScope::Chapter(c) => ctx.chapter_id.as_deref() == Some(c.as_str()),
            IntentScope::ParagraphRange { from, to } => {
                ctx.paragraph_id.as_deref().is_some_and(|p| in_range(p, from, to))
            }
            IntentScope::Character(id) => ctx.character_ids.iter().any(|c| c == id),
            IntentScope::Scene(s) => ctx.scene.as_deref() == Some(s.as_str()),
            IntentScope::TimelineRange { from, to } => {
                ctx.timeline_point.as_deref().is_some_and(|t| in_range(t, from, to))
            }
        }
    }
}

/// Inclusive lexicographic range test (ids must be comparably padded).
fn in_range(value: &str, from: &str, to: &str) -> bool {
    let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
    value >= lo && value <= hi
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

/// A ledger entry with `coverage` left as raw category-id strings — the
/// cross-feature form every reader consults (Socrates lists its typed entries as
/// these too).
#[derive(Debug, Clone)]
pub struct RawIntentRow {
    pub kind: String,
    pub description: String,
    pub scope: IntentScope,
    pub coverage: Vec<String>,
}

/// The outcome of consulting the ledger for a candidate finding.
#[derive(Debug, Clone, PartialEq)]
pub enum ConsultationResult {
    /// No declared intent applies — let the finding through.
    Emit,
    /// A declared intent covers it — suppress, with a note for the snapshot log.
    Suppress { entry_id: String, note: String },
}

/// The single shared suppression predicate: the first row whose `coverage` names
/// `coverage_id` **and** whose scope applies to `ctx`, else `None`. Every reader's
/// consultation funnels through this — Socrates' typed ledger, the Inner Editor,
/// and the Terms overlay — so the rule lives in exactly one place. Pure; no I/O.
pub fn consult_raw<'a>(
    rows: &'a [RawIntentRow],
    coverage_id: &str,
    ctx: &FindingContext,
) -> Option<&'a RawIntentRow> {
    rows.iter()
        .find(|r| r.coverage.iter().any(|c| c == coverage_id) && r.scope.applies_to(ctx))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(coverage: &[&str], scope: IntentScope) -> RawIntentRow {
        RawIntentRow {
            kind: "deliberate_repetition".into(),
            description: "the silence motif is intentional".into(),
            scope,
            coverage: coverage.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn scope_applies_to_matches_ranges_and_dimensions() {
        let ctx = FindingContext {
            paragraph_id: Some("ch07-p042".into()),
            chapter_id: Some("ch07".into()),
            ..Default::default()
        };
        assert!(IntentScope::Project.applies_to(&ctx));
        assert!(IntentScope::Chapter("ch07".into()).applies_to(&ctx));
        assert!(!IntentScope::Chapter("ch09".into()).applies_to(&ctx));
        assert!(IntentScope::ParagraphRange { from: "ch07-p001".into(), to: "ch07-p099".into() }
            .applies_to(&ctx));
        assert!(!IntentScope::ParagraphRange { from: "ch07-p043".into(), to: "ch07-p099".into() }
            .applies_to(&ctx));
    }

    #[test]
    fn consult_raw_finds_covered_in_scope_only() {
        let rows = vec![row(&["dictionary_richness"], IntentScope::Chapter("ch07".into()))];
        let in_scope = FindingContext { chapter_id: Some("ch07".into()), ..Default::default() };
        let out_scope = FindingContext { chapter_id: Some("ch09".into()), ..Default::default() };
        assert!(consult_raw(&rows, "dictionary_richness", &in_scope).is_some());
        assert!(consult_raw(&rows, "dictionary_richness", &out_scope).is_none());
        // Not covered → no hit even in scope.
        assert!(consult_raw(&rows, "tautology", &in_scope).is_none());
    }

    #[test]
    fn intent_kind_id_round_trips() {
        for k in [IntentKind::DeliberateAmbiguity, IntentKind::DeliberateVariant, IntentKind::StructuralEcho] {
            assert_eq!(IntentKind::from_id(k.id()), Some(k));
        }
        assert!(IntentKind::from_id("nonsense").is_none());
    }
}
