//! The value types of Inner Socrates — severity, the category set, the finding,
//! and the reader persona. All pure; no storage / I/O here.

use std::collections::HashMap;

/// Which track produced a finding — the deterministic pattern pass (no LLM) or
/// the semantic LLM pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Track {
    Fast,
    Slow,
}

/// The three severities, ordered `Notice < Inquiry < Probe` so a visibility
/// threshold is a simple `>=`. They map onto PANE-1's `info` / `warning` /
/// `contradiction` envelope levels (the labels are renamed for the Socratic mood;
/// the underlying pane mechanics are unchanged).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Surface observation, hidden by default.
    Notice,
    /// A question warranting consideration — the bulk of output, default visible.
    Inquiry,
    /// A structural question about the work — rare, always visible.
    Probe,
}

impl Severity {
    /// The PANE-1 Output severity this maps onto.
    pub fn pane_level(self) -> &'static str {
        match self {
            Severity::Notice => "info",
            Severity::Inquiry => "warning",
            Severity::Probe => "contradiction",
        }
    }

    /// The user-facing label.
    pub fn label(self) -> &'static str {
        match self {
            Severity::Notice => "Notice",
            Severity::Inquiry => "Inquiry",
            Severity::Probe => "Probe",
        }
    }
}

/// Every Socratic category — 7 deterministic (Fast track) + 8 semantic (Slow
/// track, of which 3 are timeline-aware). The `id` is a stable snake_case key;
/// the `label` is the evocative user-facing name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    // ── Fast (deterministic) ──
    ModalClaims,
    HedgedUncertainty,
    StructuralPatterns,
    UnattributedDialogue,
    PronounAmbiguity,
    TenseVoiceShifts,
    SentenceLengthAnomalies,
    // ── Slow (LLM) — prose ──
    AssumptionSurfacing,
    TensionDetection,
    FramingInterrogation,
    SignificanceProbing,
    ImplicitComparison,
    // ── Slow (LLM) — timeline ──
    DramatizationGap,
    ImplicationTracing,
    TemporalDensity,
}

impl Category {
    /// The 7 Fast-track categories, in declaration order.
    pub const FAST: [Category; 7] = [
        Category::ModalClaims,
        Category::HedgedUncertainty,
        Category::StructuralPatterns,
        Category::UnattributedDialogue,
        Category::PronounAmbiguity,
        Category::TenseVoiceShifts,
        Category::SentenceLengthAnomalies,
    ];

    /// The 8 Slow-track categories (5 prose + 3 timeline), in declaration order.
    pub const SLOW: [Category; 8] = [
        Category::AssumptionSurfacing,
        Category::TensionDetection,
        Category::FramingInterrogation,
        Category::SignificanceProbing,
        Category::ImplicitComparison,
        Category::DramatizationGap,
        Category::ImplicationTracing,
        Category::TemporalDensity,
    ];

    /// The stable, machine-readable id.
    pub fn id(self) -> &'static str {
        match self {
            Category::ModalClaims => "modal_claims",
            Category::HedgedUncertainty => "hedged_uncertainty",
            Category::StructuralPatterns => "structural_patterns",
            Category::UnattributedDialogue => "unattributed_dialogue",
            Category::PronounAmbiguity => "pronoun_ambiguity",
            Category::TenseVoiceShifts => "tense_voice_shifts",
            Category::SentenceLengthAnomalies => "sentence_length_anomalies",
            Category::AssumptionSurfacing => "assumption_surfacing",
            Category::TensionDetection => "tension_detection",
            Category::FramingInterrogation => "framing_interrogation",
            Category::SignificanceProbing => "significance_probing",
            Category::ImplicitComparison => "implicit_comparison",
            Category::DramatizationGap => "dramatization_gap",
            Category::ImplicationTracing => "implication_tracing",
            Category::TemporalDensity => "temporal_density",
        }
    }

    /// The evocative user-facing label.
    pub fn label(self) -> &'static str {
        match self {
            Category::ModalClaims => "Asserted Necessity",
            Category::HedgedUncertainty => "Hedging",
            Category::StructuralPatterns => "Pattern",
            Category::UnattributedDialogue => "Speaker",
            Category::PronounAmbiguity => "Reference",
            Category::TenseVoiceShifts => "Tense Shift",
            Category::SentenceLengthAnomalies => "Length",
            Category::AssumptionSurfacing => "Hidden Assumption",
            Category::TensionDetection => "Internal Tension",
            Category::FramingInterrogation => "Framing",
            Category::SignificanceProbing => "Significance",
            Category::ImplicitComparison => "Echo",
            Category::DramatizationGap => "Dramatization Gap",
            Category::ImplicationTracing => "Implication",
            Category::TemporalDensity => "Temporal Density",
        }
    }

    /// Which track this category belongs to.
    pub fn track(self) -> Track {
        if Category::FAST.contains(&self) {
            Track::Fast
        } else {
            Track::Slow
        }
    }

    /// Parse from a stable id (the inverse of [`Category::id`]).
    pub fn from_id(s: &str) -> Option<Category> {
        Category::FAST
            .into_iter()
            .chain(Category::SLOW)
            .find(|c| c.id() == s)
    }
}

/// A Socratic finding — a question, never a correction. `question` is in the
/// paragraph's language; `question_en` is the English fallback the AI-bridge
/// consumes. `suppressed_by` is set (informationally) only when a finding was
/// considered against the intent ledger and a note is attached.
#[derive(Debug, Clone, PartialEq)]
pub struct SocraticFinding {
    pub category: Category,
    pub severity: Severity,
    /// The id of the persona that produced this finding.
    pub persona_id: String,
    /// The question, in the paragraph's detected language.
    pub question: String,
    /// English fallback (for the AI-bridge / metadata).
    pub question_en: String,
    /// A note when a declared intent applies (kept for the snapshot log).
    pub suppressed_by: Option<String>,
}

impl SocraticFinding {
    /// A finding's severity clears a visibility threshold.
    pub fn visible_at(&self, threshold: Severity) -> bool {
        self.severity >= threshold
    }
}

/// How a persona delivers its Slow-track findings. The default is the classical
/// Socratic discipline — questions only, never praise, never prescribe. The
/// **verdict** stances (1.4.7) deliberately relax that for the two adversarial
/// bundled personas: the **Defender** speaks only praise, the **Prosecutor**
/// only concern. User-authored personas may opt in via `stance:` in HJSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stance {
    /// Questions only — the Socratic spine shared by every other persona.
    #[default]
    Question,
    /// Praise only — "counsel for the defense" (the Defender).
    Praise,
    /// Concern only — "the prosecution" (the Prosecutor).
    Concern,
}

impl Stance {
    /// A verdict stance *states* findings (praise / concern); the default *asks*.
    pub fn is_verdict(self) -> bool {
        !matches!(self, Stance::Question)
    }

    /// Parse a `stance:` id from a persona HJSON file. Accepts the canonical
    /// `question` / `praise` / `concern` plus the role aliases.
    pub fn from_id(s: &str) -> Option<Stance> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "question" | "socratic" => Some(Stance::Question),
            "praise" | "defend" | "defender" | "defence" | "defense" => Some(Stance::Praise),
            "concern" | "prosecute" | "prosecutor" | "prosecution" => Some(Stance::Concern),
            _ => None,
        }
    }
}

/// A reader persona — a distinct careful-reader perspective. Category **emphasis
/// weights** scale a category's salience for this persona; `0.0` mutes the
/// category entirely. Voice notes are the LLM-facing character (used by the Slow
/// track in later phases).
#[derive(Debug, Clone)]
pub struct Persona {
    pub id: String,
    pub name: String,
    pub description: String,
    /// A short (<100 char) UI tagline.
    pub voice_summary: String,
    /// 200–400 words of LLM-facing character (Slow track).
    pub voice_notes: String,
    /// Per-category emphasis; absent categories default to `1.0`.
    pub emphasis: HashMap<Category, f32>,
    /// How the Slow track delivers findings (questions by default; praise /
    /// concern for the verdict personas). 1.4.7 AUDIENCE-1.
    pub stance: Stance,
}

impl Persona {
    /// The emphasis weight for a category (default `1.0`).
    pub fn emphasis_for(&self, c: Category) -> f32 {
        self.emphasis.get(&c).copied().unwrap_or(1.0)
    }

    /// A persona is *muted* for a category when its emphasis is non-positive.
    pub fn mutes(&self, c: Category) -> bool {
        self.emphasis_for(c) <= 0.0
    }

    /// The bundled default persona (Inner Socrates) — used until storage + the
    /// bundled-persona files land (P2). Emphasis mirrors the RFC's character sheet.
    pub fn default_inner_socrates() -> Persona {
        let mut emphasis = HashMap::new();
        emphasis.insert(Category::AssumptionSurfacing, 1.2);
        emphasis.insert(Category::FramingInterrogation, 1.1);
        emphasis.insert(Category::SignificanceProbing, 1.1);
        Persona {
            id: "inner-socrates".into(),
            name: "Inner Socrates".into(),
            description: "The classical interrogator: brief, direct, never reassuring, never \
                          prescriptive. Asks what the prose presupposes and what it leaves out."
                .into(),
            voice_summary: "Every question opens what the prose has closed.".into(),
            voice_notes: String::new(),
            emphasis,
            stance: Stance::Question,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_orders_and_maps() {
        assert!(Severity::Notice < Severity::Inquiry);
        assert!(Severity::Inquiry < Severity::Probe);
        assert_eq!(Severity::Notice.pane_level(), "info");
        assert_eq!(Severity::Inquiry.pane_level(), "warning");
        assert_eq!(Severity::Probe.pane_level(), "contradiction");
    }

    #[test]
    fn categories_partition_into_tracks() {
        assert_eq!(Category::FAST.len(), 7);
        assert_eq!(Category::SLOW.len(), 8);
        for c in Category::FAST {
            assert_eq!(c.track(), Track::Fast, "{}", c.id());
        }
        for c in Category::SLOW {
            assert_eq!(c.track(), Track::Slow, "{}", c.id());
        }
    }

    #[test]
    fn category_ids_roundtrip_and_are_unique() {
        let all: Vec<Category> = Category::FAST.into_iter().chain(Category::SLOW).collect();
        let mut ids: Vec<&str> = all.iter().map(|c| c.id()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 15, "all 15 ids distinct");
        for c in all {
            assert_eq!(Category::from_id(c.id()), Some(c));
        }
        assert_eq!(Category::from_id("not_a_category"), None);
    }

    #[test]
    fn persona_emphasis_defaults_and_mutes() {
        let p = Persona::default_inner_socrates();
        assert_eq!(p.emphasis_for(Category::AssumptionSurfacing), 1.2);
        assert_eq!(p.emphasis_for(Category::TenseVoiceShifts), 1.0); // default
        assert!(!p.mutes(Category::AssumptionSurfacing));
        let mut muted = p.clone();
        muted.emphasis.insert(Category::HedgedUncertainty, 0.0);
        assert!(muted.mutes(Category::HedgedUncertainty));
    }

    #[test]
    fn visibility_threshold() {
        let f = SocraticFinding {
            category: Category::ModalClaims,
            severity: Severity::Notice,
            persona_id: "inner-socrates".into(),
            question: "?".into(),
            question_en: "?".into(),
            suppressed_by: None,
        };
        assert!(f.visible_at(Severity::Notice));
        assert!(!f.visible_at(Severity::Inquiry));
    }
}
