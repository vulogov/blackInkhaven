//! INNER-THEOLOGIAN-1 — the third Inner-family reader. Where Inner Socrates
//! interrogates logical structure and Inner Editor observes craft, Inner
//! Theologian reads **moral and theological seriousness**: whether the work
//! engages honestly with the weight of what it depicts and the implicit
//! cosmology it constructs.
//!
//! Tradition-neutral: it reads through eleven tradition lenses simultaneously,
//! never to judge by any of them, only to ask what each *sees*. It belongs to no
//! tradition, advocates none, and never exceeds `info` severity. It never edits
//! prose.
//!
//! IT-P0 — the language model: the tradition lenses, the fast-track signal kinds,
//! the six slow-track question categories, and the finding record the store and
//! detector (IT-P2/IT-P3) build on. Provenance glyph `⚖` (U+2696, scales) —
//! tradition-neutral, distinct from Socrates' findings, Editor's `✎`, and the
//! `✦` HAIKU-1 took.

mod corpus;
mod detect;
mod grounding;
mod lens;
mod llm;
mod pipeline;
mod prompt;
mod store;
mod vocab;

pub(crate) use corpus::auto_fire_question;
pub(crate) use detect::DetectWindows;
pub(crate) use grounding::build_grounding;
pub(crate) use llm::theologian_llm_call;
pub(crate) use pipeline::run_fast_scan;
pub(crate) use prompt::{
    THEOLOGIAN_SYSTEM, build_discovery_prompt, build_session_prompt, parse_selected_lenses,
};
pub(crate) use store::TheologianStore;

/// The eleven tradition lenses (RFC §5.2). No lens is privileged; this order is
/// presentation-grouping only and carries no weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TraditionLens {
    // Christian family
    Catholic,
    Protestant,
    Orthodox,
    Gnostic,
    Lds,
    // Abrahamic siblings
    Islam,
    Judaism,
    // Dharmic
    Hinduism,
    Buddhism,
    // Relational / philosophical
    Confucianism,
    SecularPhilosophy,
}

impl TraditionLens {
    pub(crate) const ALL: [TraditionLens; 11] = [
        TraditionLens::Catholic,
        TraditionLens::Protestant,
        TraditionLens::Orthodox,
        TraditionLens::Gnostic,
        TraditionLens::Lds,
        TraditionLens::Islam,
        TraditionLens::Judaism,
        TraditionLens::Hinduism,
        TraditionLens::Buddhism,
        TraditionLens::Confucianism,
        TraditionLens::SecularPhilosophy,
    ];

    pub(crate) fn as_code(&self) -> &'static str {
        match self {
            TraditionLens::Catholic => "catholic",
            TraditionLens::Protestant => "protestant",
            TraditionLens::Orthodox => "orthodox",
            TraditionLens::Gnostic => "gnostic",
            TraditionLens::Lds => "lds",
            TraditionLens::Islam => "islam",
            TraditionLens::Judaism => "judaism",
            TraditionLens::Hinduism => "hinduism",
            TraditionLens::Buddhism => "buddhism",
            TraditionLens::Confucianism => "confucianism",
            TraditionLens::SecularPhilosophy => "secular_philosophy",
        }
    }

    pub(crate) fn from_code(s: &str) -> Option<TraditionLens> {
        Some(match s.trim().to_lowercase().as_str() {
            "catholic" => TraditionLens::Catholic,
            "protestant" => TraditionLens::Protestant,
            "orthodox" => TraditionLens::Orthodox,
            "gnostic" => TraditionLens::Gnostic,
            "lds" => TraditionLens::Lds,
            "islam" => TraditionLens::Islam,
            "judaism" => TraditionLens::Judaism,
            "hinduism" => TraditionLens::Hinduism,
            "buddhism" => TraditionLens::Buddhism,
            "confucianism" => TraditionLens::Confucianism,
            "secular_philosophy" | "secular" => TraditionLens::SecularPhilosophy,
            _ => return None,
        })
    }

    /// The human-facing label used when the persona names which lens raises a
    /// question (it is always explicit about its source tradition, RFC §9.1).
    pub(crate) fn label(&self) -> &'static str {
        match self {
            TraditionLens::Catholic => "Catholic",
            TraditionLens::Protestant => "Protestant",
            TraditionLens::Orthodox => "Orthodox",
            TraditionLens::Gnostic => "Gnostic",
            TraditionLens::Lds => "LDS",
            TraditionLens::Islam => "Islam",
            TraditionLens::Judaism => "Judaism",
            TraditionLens::Hinduism => "Hinduism",
            TraditionLens::Buddhism => "Buddhism",
            TraditionLens::Confucianism => "Confucianism",
            TraditionLens::SecularPhilosophy => "Secular philosophy",
        }
    }
}

/// The three fast-track ethical signals (RFC §8). All deterministic, all `info`,
/// all suppressible — never `warning`/`error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SignalType {
    /// Named character harms another, no acknowledgment in the following window.
    MoralInvisibility,
    /// Lethal/severe violence with no depicted consequence in the window.
    ConsequenceGap,
    /// Sacred/ritual vocabulary alongside comic or dismissive markers.
    SacredLevity,
}

impl SignalType {
    pub(crate) fn as_code(&self) -> &'static str {
        match self {
            SignalType::MoralInvisibility => "moral_invisibility",
            SignalType::ConsequenceGap => "consequence_gap",
            SignalType::SacredLevity => "sacred_levity",
        }
    }

    pub(crate) fn from_code(s: &str) -> Option<SignalType> {
        Some(match s {
            "moral_invisibility" => SignalType::MoralInvisibility,
            "consequence_gap" => SignalType::ConsequenceGap,
            "sacred_levity" => SignalType::SacredLevity,
            _ => return None,
        })
    }

    pub(crate) fn label(&self) -> &'static str {
        match self {
            SignalType::MoralInvisibility => "moral invisibility",
            SignalType::ConsequenceGap => "consequence gap",
            SignalType::SacredLevity => "sacred vocabulary in levity-adjacent context",
        }
    }
}

/// The six slow-track question categories (RFC §9.2). Category 1 is the only one
/// raised in auto-fire; Category 6 needs book/chapter-range scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuestionCategory {
    MoralWeight,
    Theodicy,
    Redemption,
    Sacred,
    Duty,
    ImplicitTheology,
}

impl QuestionCategory {
    pub(crate) fn from_number(n: u8) -> Option<QuestionCategory> {
        Some(match n {
            1 => QuestionCategory::MoralWeight,
            2 => QuestionCategory::Theodicy,
            3 => QuestionCategory::Redemption,
            4 => QuestionCategory::Sacred,
            5 => QuestionCategory::Duty,
            6 => QuestionCategory::ImplicitTheology,
            _ => return None,
        })
    }

    pub(crate) fn number(&self) -> u8 {
        match self {
            QuestionCategory::MoralWeight => 1,
            QuestionCategory::Theodicy => 2,
            QuestionCategory::Redemption => 3,
            QuestionCategory::Sacred => 4,
            QuestionCategory::Duty => 5,
            QuestionCategory::ImplicitTheology => 6,
        }
    }

    pub(crate) fn label(&self) -> &'static str {
        match self {
            QuestionCategory::MoralWeight => "Moral weight and consequence",
            QuestionCategory::Theodicy => "Theodicy and innocent suffering",
            QuestionCategory::Redemption => "Redemption and transformation",
            QuestionCategory::Sacred => "Sacred and transcendent",
            QuestionCategory::Duty => "Duty and obligation",
            QuestionCategory::ImplicitTheology => "The author's implicit theology",
        }
    }
}

/// One fast-track signal finding (the store record, IT-P2). Mirrors the shape of
/// `SocraticFinding` but for deterministic ethical signals.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TheologianFinding {
    pub signal_type: SignalType,
    pub chapter_ord: u32,
    pub para_id: String,
    pub description: String,
    pub suppressed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tradition_lens_round_trip_all_eleven() {
        assert_eq!(TraditionLens::ALL.len(), 11);
        for lens in TraditionLens::ALL {
            assert_eq!(TraditionLens::from_code(lens.as_code()), Some(lens));
            assert!(!lens.label().is_empty());
        }
        assert_eq!(TraditionLens::from_code("secular"), Some(TraditionLens::SecularPhilosophy));
        assert_eq!(TraditionLens::from_code("nope"), None);
    }

    #[test]
    fn signal_and_category_codes() {
        for s in [SignalType::MoralInvisibility, SignalType::ConsequenceGap, SignalType::SacredLevity] {
            assert_eq!(SignalType::from_code(s.as_code()), Some(s));
        }
        for n in 1..=6u8 {
            assert_eq!(QuestionCategory::from_number(n).map(|c| c.number()), Some(n));
        }
        assert_eq!(QuestionCategory::from_number(7), None);
    }
}
