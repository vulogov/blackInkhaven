//! CHAR-1 — character arc tracking. Per named character: a chapter-ordered
//! observable state chain (LLM-extracted), a deterministic agency score, and
//! completeness checks against an author-declared arc. Read-only / advisory.
//! Imports NARR-1's `ProseLanguage` + passive detection and reads DIALOG-1's
//! dialogue tables; never writes to either.
//!
//! C-P0 — the language model: the arc taxonomy, the core state/check types, and
//! the per-language action-verb lists the agency score (C-P3) builds on.

mod agency;
mod declarations;
mod llm;
mod pipeline;
mod store;
mod verbs;

pub(crate) use declarations::read_arc_declarations;
pub(crate) use pipeline::{character_names, run_agency, run_extraction};
pub(crate) use store::CharStore;
pub(crate) use verbs::{ActionVerbs, is_action_verb, verbs_for};

/// Arc structural taxonomy (RFC §4). `Other` accepts any author string and
/// falls the checker back to the generic arc-completeness probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArcType {
    /// Overcomes a false belief, reaches a growth truth (Scrooge).
    PositiveChange,
    /// Already holds the truth; changes the world, not themselves (Atticus).
    Flat,
    /// Starts near truth, chooses a lie, degrades (Michael Corleone).
    Corruption,
    /// Starts in a lie, embraces a worse one; no redemption (Macbeth).
    Fall,
    /// Starts in a lie, discovers a painful truth and accepts it (Javert).
    Disillusionment,
    /// Any other declared value — generic probe.
    Other(String),
}

impl ArcType {
    pub(crate) fn from_label(s: &str) -> ArcType {
        match s.trim().to_lowercase().replace([' ', '-'], "_").as_str() {
            "positive_change" | "positive" | "growth" | "redemption" => ArcType::PositiveChange,
            "flat" | "steadfast" => ArcType::Flat,
            "corruption" => ArcType::Corruption,
            "fall" => ArcType::Fall,
            "disillusionment" => ArcType::Disillusionment,
            other => ArcType::Other(other.to_string()),
        }
    }

    pub(crate) fn as_code(&self) -> &str {
        match self {
            ArcType::PositiveChange => "positive_change",
            ArcType::Flat => "flat",
            ArcType::Corruption => "corruption",
            ArcType::Fall => "fall",
            ArcType::Disillusionment => "disillusionment",
            ArcType::Other(s) => s,
        }
    }

    /// Arc-type-specific framing injected into the earned-check prompt (§10.2).
    pub(crate) fn earned_framing(&self) -> &'static str {
        match self {
            ArcType::PositiveChange => "look for moments where the false belief is challenged and \
                the character responds, even if they resist",
            ArcType::Flat => "look for moments where the character's core belief is tested but \
                holds; the world should be changing around them, not the character",
            ArcType::Corruption => "look for incremental moral compromises, each slightly larger \
                than the last; the arc fails if the largest compromise has no smaller ones before it",
            ArcType::Fall => "look for moments where a better choice was available and refused; \
                the arc fails if the character never had a real chance to escape the lie",
            ArcType::Disillusionment => "look for moments of growing contact with the truth the \
                character resists; the acceptance at the end should feel reluctant, not sudden",
            ArcType::Other(_) => "look for gradual shifts, moments of pressure, and small \
                reversals that prepare the character's final state",
        }
    }
}

/// The five arc-check kinds (RFC §9.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArcCheckType {
    StartAlignment,
    MidpointAlignment,
    EndAlignment,
    ArcEarned,
    StallLocation,
}

impl ArcCheckType {
    pub(crate) fn as_code(&self) -> &'static str {
        match self {
            ArcCheckType::StartAlignment => "start_alignment",
            ArcCheckType::MidpointAlignment => "midpoint_alignment",
            ArcCheckType::EndAlignment => "end_alignment",
            ArcCheckType::ArcEarned => "arc_earned",
            ArcCheckType::StallLocation => "stall_location",
        }
    }

    pub(crate) fn from_code(s: &str) -> Option<ArcCheckType> {
        match s {
            "start_alignment" => Some(ArcCheckType::StartAlignment),
            "midpoint_alignment" => Some(ArcCheckType::MidpointAlignment),
            "end_alignment" => Some(ArcCheckType::EndAlignment),
            "arc_earned" => Some(ArcCheckType::ArcEarned),
            "stall_location" => Some(ArcCheckType::StallLocation),
            _ => None,
        }
    }

    pub(crate) fn label(&self) -> &'static str {
        match self {
            ArcCheckType::StartAlignment => "Start alignment",
            ArcCheckType::MidpointAlignment => "Midpoint alignment",
            ArcCheckType::EndAlignment => "End alignment",
            ArcCheckType::ArcEarned => "Arc earned",
            ArcCheckType::StallLocation => "Stall detected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArcVerdict {
    Aligned,
    Gap,
    Stalled,
    Earned,
}

impl ArcVerdict {
    pub(crate) fn as_code(&self) -> &'static str {
        match self {
            ArcVerdict::Aligned => "aligned",
            ArcVerdict::Gap => "gap",
            ArcVerdict::Stalled => "stalled",
            ArcVerdict::Earned => "earned",
        }
    }

    pub(crate) fn from_code(s: &str) -> ArcVerdict {
        match s {
            "gap" => ArcVerdict::Gap,
            "stalled" => ArcVerdict::Stalled,
            "earned" => ArcVerdict::Earned,
            _ => ArcVerdict::Aligned,
        }
    }

    /// Whether this verdict reads as a problem (drives CLI exit codes).
    pub(crate) fn is_problem(&self) -> bool {
        matches!(self, ArcVerdict::Gap | ArcVerdict::Stalled)
    }
}

/// One chapter's extracted observable state for a character (RFC §9.3).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CharacterState {
    pub character_name: String,
    pub chapter_ord: u32,
    pub state_summary: String,
    pub changed: bool,
    pub change_description: Option<String>,
    pub agency_score: Option<f32>,
    pub active_count: u32,
    pub passive_count: u32,
    pub utterance_count: Option<u32>,
    pub chapter_hedge_density: Option<f32>,
    pub chapter_interiority_ratio: Option<f32>,
}

/// An author-declared arc from the Characters book `character_arc` block.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ArcDeclaration {
    pub character_name: String,
    pub arc_type: ArcType,
    pub desired_state_start: String,
    pub desired_midpoint_state: Option<String>,
    pub desired_state_end: String,
}

/// One arc-check result.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CharacterArcCheck {
    pub character_name: String,
    pub check_type: ArcCheckType,
    pub verdict: ArcVerdict,
    pub description: String,
    pub chapter_ord: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_type_round_trip_and_fallback() {
        for t in [
            ArcType::PositiveChange,
            ArcType::Flat,
            ArcType::Corruption,
            ArcType::Fall,
            ArcType::Disillusionment,
        ] {
            assert_eq!(ArcType::from_label(t.as_code()), t);
        }
        // Aliases + open string.
        assert_eq!(ArcType::from_label("redemption"), ArcType::PositiveChange);
        assert_eq!(ArcType::from_label("positive-change"), ArcType::PositiveChange);
        assert_eq!(ArcType::from_label("my custom arc"), ArcType::Other("my_custom_arc".into()));
        // Every type yields earned framing.
        assert!(!ArcType::Other("x".into()).earned_framing().is_empty());
        assert!(ArcType::Corruption.earned_framing().contains("compromise"));
    }

    #[test]
    fn check_type_and_verdict_codes() {
        for c in [
            ArcCheckType::StartAlignment,
            ArcCheckType::MidpointAlignment,
            ArcCheckType::EndAlignment,
            ArcCheckType::ArcEarned,
            ArcCheckType::StallLocation,
        ] {
            assert_eq!(ArcCheckType::from_code(c.as_code()), Some(c));
        }
        assert!(ArcVerdict::Gap.is_problem());
        assert!(ArcVerdict::Stalled.is_problem());
        assert!(!ArcVerdict::Aligned.is_problem());
        assert!(!ArcVerdict::Earned.is_problem());
        assert_eq!(ArcVerdict::from_code("earned"), ArcVerdict::Earned);
    }
}
