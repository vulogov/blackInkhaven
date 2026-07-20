//! HYP-1 (Wave 4, Track C) — the hypothesis register.
//!
//! Historical conlanging is an accumulation of *claims*: "Proto-X had \*k, which
//! became tʃ before front vowels in the Northern branch"; "these two daughter
//! forms are cognate"; "this word is a loan from Y". The diachronic tools
//! (`sound-change`, `cognates`, `reconstruct`, the Consequence Tracer) let you
//! *test* such claims one at a time but never *record* them. This is the record:
//! each hypothesis carries its kind, the claim itself, a rationale, its evidence,
//! and a status that moves from proposed → supported / refuted as the evidence
//! comes in. The type is pure; persistence (a `Hypotheses` chapter of the language
//! book) and the tracer-assisted consequence check are the CLI's job.

use serde::{Deserialize, Serialize};

/// What kind of diachronic/comparative claim a hypothesis makes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A regular sound change (an SPE-style rule).
    SoundChange,
    /// A cognacy claim — forms descending from one proto-root.
    Cognacy,
    /// A borrowing — a form taken from another language.
    Borrowing,
    Other,
}

/// Where a hypothesis stands as evidence accumulates.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Proposed,
    Supported,
    Refuted,
    Retired,
}

/// One recorded diachronic/comparative hypothesis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hypothesis {
    /// Short handle used to refer to it.
    pub id: String,
    pub kind: Kind,
    /// The claim itself — a rewrite rule (`k > tʃ / _ i`), a cognate set, a loan.
    pub claim: String,
    /// Free rationale / discussion.
    #[serde(default)]
    pub note: String,
    /// Supporting (or contradicting) forms and examples.
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub status: Status,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        match s.trim().to_ascii_lowercase().replace(['_', ' '], "-").as_str() {
            "sound-change" | "sound" | "change" | "rule" => Some(Kind::SoundChange),
            "cognacy" | "cognate" | "cognates" => Some(Kind::Cognacy),
            "borrowing" | "loan" | "borrow" => Some(Kind::Borrowing),
            "other" => Some(Kind::Other),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::SoundChange => "sound-change",
            Kind::Cognacy => "cognacy",
            Kind::Borrowing => "borrowing",
            Kind::Other => "other",
        }
    }
}

impl Status {
    pub fn parse(s: &str) -> Option<Status> {
        match s.trim().to_ascii_lowercase().as_str() {
            "proposed" | "propose" => Some(Status::Proposed),
            "supported" | "support" | "confirmed" => Some(Status::Supported),
            "refuted" | "refute" | "rejected" => Some(Status::Refuted),
            "retired" | "retire" | "withdrawn" => Some(Status::Retired),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Proposed => "proposed",
            Status::Supported => "supported",
            Status::Refuted => "refuted",
            Status::Retired => "retired",
        }
    }

    /// A glyph for the status, mirroring the Output pane's convention.
    pub fn icon(self) -> char {
        match self {
            Status::Proposed => '?',
            Status::Supported => '✓',
            Status::Refuted => '✗',
            Status::Retired => '—',
        }
    }
}

impl Hypothesis {
    /// A one-line summary: `? [sound-change] palatalization — k > tʃ / _ i`.
    pub fn summary(&self) -> String {
        format!("{} [{}] {} — {}", self.status.icon(), self.kind.label(), self.id, self.claim)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_and_status_parse_flexibly() {
        assert_eq!(Kind::parse("sound_change"), Some(Kind::SoundChange));
        assert_eq!(Kind::parse("Loan"), Some(Kind::Borrowing));
        assert_eq!(Kind::parse("cognates"), Some(Kind::Cognacy));
        assert_eq!(Kind::parse("nonsense"), None);
        assert_eq!(Status::parse("Confirmed"), Some(Status::Supported));
        assert_eq!(Status::parse("refute"), Some(Status::Refuted));
    }

    #[test]
    fn a_hypothesis_round_trips_through_json() {
        // The stored form (a Hypotheses-chapter paragraph) must reload identically.
        let h = Hypothesis {
            id: "palatalization".into(),
            kind: Kind::SoundChange,
            claim: "k > tʃ / _ i".into(),
            note: "Northern branch only.".into(),
            evidence: vec!["kina → tʃina".into()],
            status: Status::Supported,
        };
        let json = serde_json::to_string(&h).unwrap();
        let back: Hypothesis = serde_json::from_str(&json).unwrap();
        assert_eq!(h, back);
    }

    #[test]
    fn defaults_fill_in_for_a_minimal_record() {
        // A record with only the required fields loads, defaulting the rest.
        let h: Hypothesis =
            serde_json::from_str(r#"{"id":"x","kind":"borrowing","claim":"tea < Y"}"#).unwrap();
        assert_eq!(h.status, Status::Proposed);
        assert!(h.evidence.is_empty());
        assert!(h.note.is_empty());
    }

    #[test]
    fn summary_reads_as_a_register_line() {
        let h = Hypothesis {
            id: "p1".into(),
            kind: Kind::SoundChange,
            claim: "s > h / #_".into(),
            note: String::new(),
            evidence: vec![],
            status: Status::Proposed,
        };
        assert_eq!(h.summary(), "? [sound-change] p1 — s > h / #_");
    }
}
