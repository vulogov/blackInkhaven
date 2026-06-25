//! INNER_EDITOR-1 — the value types: the Editor's severity vocabulary
//! (Praise/Note/Concern), the eight observation categories, a finding, and the
//! tuning-knob enums. Pure data; no I/O, no `pane` coupling (the PANE-1 severity
//! mapping lives in the output bridge).

/// The Editor's severity vocabulary — distinct from Inner Socrates'
/// Notice/Inquiry/Probe. Praise is first-class (grounded, specific). Ordered
/// `Praise < Note < Concern` for the visible-threshold filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EditorSeverity {
    /// Specific, earned observation of what is working. Lightest weight; hidden
    /// by default (maps to PANE-1 `info`).
    Praise,
    /// The bulk of substantive observation (maps to PANE-1 `warning`).
    Note,
    /// A craft issue warranting attention (maps to PANE-1 `contradiction`).
    Concern,
}

impl EditorSeverity {
    pub fn id(self) -> &'static str {
        match self {
            EditorSeverity::Praise => "praise",
            EditorSeverity::Note => "note",
            EditorSeverity::Concern => "concern",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EditorSeverity::Praise => "Praise",
            EditorSeverity::Note => "Note",
            EditorSeverity::Concern => "Concern",
        }
    }

    /// Parse an LLM/stored severity id; tolerant default `Note` for anything
    /// unrecognised (the bulk category — safest default).
    pub fn from_id(s: &str) -> EditorSeverity {
        match s.trim().to_ascii_lowercase().as_str() {
            "praise" => EditorSeverity::Praise,
            "concern" => EditorSeverity::Concern,
            _ => EditorSeverity::Note,
        }
    }

    /// `0 = Praise, 1 = Note, 2 = Concern` — for the visible-threshold compare.
    pub fn rank(self) -> u8 {
        match self {
            EditorSeverity::Praise => 0,
            EditorSeverity::Note => 1,
            EditorSeverity::Concern => 2,
        }
    }
}

/// The eight observation categories, in three modes (literary / vocabulary /
/// editorial-response). The `label` is the short Output-row tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorCategory {
    // ── literary analysis ──
    LiteraryRichness,
    Tautology,
    StyleObservation,
    StyleInstability,
    // ── vocabulary analysis ──
    DictionaryRichness,
    // ── editorial response ──
    BeliefStance,
    CraftPraise,
    EditorialSuggestions,
}

impl EditorCategory {
    pub const ALL: [EditorCategory; 8] = [
        EditorCategory::LiteraryRichness,
        EditorCategory::Tautology,
        EditorCategory::StyleObservation,
        EditorCategory::StyleInstability,
        EditorCategory::DictionaryRichness,
        EditorCategory::BeliefStance,
        EditorCategory::CraftPraise,
        EditorCategory::EditorialSuggestions,
    ];

    pub fn id(self) -> &'static str {
        match self {
            EditorCategory::LiteraryRichness => "literary_richness",
            EditorCategory::Tautology => "tautology",
            EditorCategory::StyleObservation => "style_observation",
            EditorCategory::StyleInstability => "style_instability",
            EditorCategory::DictionaryRichness => "dictionary_richness",
            EditorCategory::BeliefStance => "belief_stance",
            EditorCategory::CraftPraise => "craft_praise",
            EditorCategory::EditorialSuggestions => "editorial_suggestions",
        }
    }

    /// Short user-facing tag shown in the Output row (RFC §7.8).
    pub fn label(self) -> &'static str {
        match self {
            EditorCategory::LiteraryRichness => "Richness",
            EditorCategory::Tautology => "Tautology",
            EditorCategory::StyleObservation => "Style",
            EditorCategory::StyleInstability => "Style Drift",
            EditorCategory::DictionaryRichness => "Vocabulary",
            EditorCategory::BeliefStance => "Belief",
            EditorCategory::CraftPraise => "Craft",
            EditorCategory::EditorialSuggestions => "Suggestion",
        }
    }

    pub fn from_id(s: &str) -> Option<EditorCategory> {
        EditorCategory::ALL
            .into_iter()
            .find(|c| c.id() == s.trim())
    }
}

/// One Editor observation about a paragraph, ready to persist and emit.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorFinding {
    pub category: EditorCategory,
    pub severity: EditorSeverity,
    /// The observation in the paragraph's detected language.
    pub observation: String,
    /// English fallback, for the AI-pane bridge / `inkhaven cost` neutrality.
    pub observation_en: String,
    /// The specific textual evidence the observation grounds in (the discipline
    /// that keeps Praise earned and Concern concrete). Optional.
    pub evidence: Option<String>,
    /// Whether the observation is framed conditionally ("if intentional…").
    pub conditional: bool,
    /// When set, the intent ledger suppressed this finding; the note explains
    /// which declared intent covered it.
    pub suppressed_by: Option<String>,
}

// ── tuning-knob enums (parsed tolerantly from the HJSON strings) ────────────

/// Emphasis on critique vs. praise. HJSON `inner_editor.persona.tone`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Critical,
    Balanced,
    Encouraging,
}

impl Tone {
    pub fn from_id(s: &str) -> Tone {
        match s.trim().to_ascii_lowercase().as_str() {
            "critical" => Tone::Critical,
            "encouraging" => Tone::Encouraging,
            _ => Tone::Balanced,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Tone::Critical => "critical",
            Tone::Balanced => "balanced",
            Tone::Encouraging => "encouraging",
        }
    }
}

/// Length of finding text. HJSON `inner_editor.persona.verbosity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
    Concise,
    Standard,
    Detailed,
}

impl Verbosity {
    pub fn from_id(s: &str) -> Verbosity {
        match s.trim().to_ascii_lowercase().as_str() {
            "standard" => Verbosity::Standard,
            "detailed" => Verbosity::Detailed,
            _ => Verbosity::Concise,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Verbosity::Concise => "concise",
            Verbosity::Standard => "standard",
            Verbosity::Detailed => "detailed",
        }
    }
}

/// Rate of Praise-severity findings. HJSON `inner_editor.persona.praise_frequency`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PraiseFrequency {
    Rare,
    Moderate,
    Frequent,
}

impl PraiseFrequency {
    pub fn from_id(s: &str) -> PraiseFrequency {
        match s.trim().to_ascii_lowercase().as_str() {
            "rare" => PraiseFrequency::Rare,
            "frequent" => PraiseFrequency::Frequent,
            _ => PraiseFrequency::Moderate,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            PraiseFrequency::Rare => "rare",
            PraiseFrequency::Moderate => "moderate",
            PraiseFrequency::Frequent => "frequent",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_roundtrips_and_orders() {
        for s in [EditorSeverity::Praise, EditorSeverity::Note, EditorSeverity::Concern] {
            assert_eq!(EditorSeverity::from_id(s.id()), s);
        }
        // Unknown → Note (the bulk default).
        assert_eq!(EditorSeverity::from_id("whatever"), EditorSeverity::Note);
        assert!(EditorSeverity::Praise < EditorSeverity::Note);
        assert!(EditorSeverity::Note < EditorSeverity::Concern);
        assert_eq!(EditorSeverity::Concern.rank(), 2);
    }

    #[test]
    fn every_category_roundtrips_and_is_unique() {
        let mut ids = std::collections::HashSet::new();
        for c in EditorCategory::ALL {
            assert_eq!(EditorCategory::from_id(c.id()), Some(c));
            assert!(ids.insert(c.id()), "duplicate id {}", c.id());
            assert!(!c.label().is_empty());
        }
        assert_eq!(ids.len(), 8);
        assert_eq!(EditorCategory::from_id("nope"), None);
    }

    #[test]
    fn tuning_enums_default_tolerantly() {
        assert_eq!(Tone::from_id("CRITICAL"), Tone::Critical);
        assert_eq!(Tone::from_id("garbage"), Tone::Balanced);
        assert_eq!(Verbosity::from_id("detailed"), Verbosity::Detailed);
        assert_eq!(Verbosity::from_id(""), Verbosity::Concise);
        assert_eq!(PraiseFrequency::from_id("frequent"), PraiseFrequency::Frequent);
        assert_eq!(PraiseFrequency::from_id("x"), PraiseFrequency::Moderate);
    }
}
