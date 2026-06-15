//! 1.3.1 SUBMISSION-1 — the submission tracker.
//!
//! Generated drafts (query letter / synopsis / …) live as prose in the
//! `Submissions` system book; this module is the **structured** half — a
//! project-wide `.inkhaven/submissions.json` log of where the manuscript
//! went, when, and what came back.  Project-wide (not per-file like
//! comments), atomic writes via `io_atomic`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionStatus {
    Drafting,
    Sent,
    Rejected,
    Offer,
    Withdrawn,
}

impl SubmissionStatus {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "drafting" | "draft" => Some(Self::Drafting),
            "sent" | "submitted" => Some(Self::Sent),
            "rejected" | "reject" | "pass" => Some(Self::Rejected),
            "offer" | "accepted" | "accept" => Some(Self::Offer),
            "withdrawn" | "withdraw" => Some(Self::Withdrawn),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Drafting => "drafting",
            Self::Sent => "sent",
            Self::Rejected => "rejected",
            Self::Offer => "offer",
            Self::Withdrawn => "withdrawn",
        }
    }
    /// Whether the submission is still live (awaiting a response).
    pub fn is_open(self) -> bool {
        matches!(self, Self::Sent)
    }
    /// Used by the P2.2 TUI tracker modal (status cycling) + the tests.
    #[allow(dead_code)]
    pub const ALL: [Self; 5] = [
        Self::Drafting,
        Self::Sent,
        Self::Rejected,
        Self::Offer,
        Self::Withdrawn,
    ];
}

/// One timestamped entry in a submission's running note log — the event
/// trail (got a call, requested edits, moving to round two, …).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteEntry {
    /// ISO `YYYY-MM-DD` the note was added.
    pub date: String,
    pub text: String,
}

/// One submission to one market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionRecord {
    /// Sequential `S<n>` id.
    pub id: String,
    /// Agency / publication / contest.
    pub market: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    /// Paragraph slug of the draft used, in the `Submissions` book.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_ref: Option<String>,
    /// ISO `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_sent: Option<String>,
    pub status: SubmissionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_action_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Append-only timestamped event trail (`submissions add-note`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub log: Vec<NoteEntry>,
}

impl SubmissionRecord {
    /// Append a timestamped note to the event trail.
    pub fn add_note(&mut self, text: impl Into<String>) {
        self.log.push(NoteEntry {
            date: today(),
            text: text.into(),
        });
    }
}

/// The whole tracker.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubmissionLog {
    #[serde(default)]
    pub records: Vec<SubmissionRecord>,
}

/// Today as ISO `YYYY-MM-DD` (local).
pub fn today() -> String {
    chrono::Local::now().date_naive().format("%Y-%m-%d").to_string()
}

impl SubmissionLog {
    pub fn sidecar_path(project_root: &Path) -> PathBuf {
        project_root.join(".inkhaven").join("submissions.json")
    }

    pub fn load(project_root: &Path) -> Result<Self, String> {
        let path = Self::sidecar_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| format!("submissions: read {}: {e}", path.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        serde_json::from_str(&raw)
            .map_err(|e| format!("submissions: parse {}: {e}", path.display()))
    }

    pub fn save(&self, project_root: &Path) -> Result<(), String> {
        let dir = project_root.join(".inkhaven");
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("submissions: mkdir {}: {e}", dir.display()))?;
        let path = Self::sidecar_path(project_root);
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| format!("submissions: serialize: {e}"))?;
        crate::io_atomic::write(&path, raw.as_bytes())
            .map_err(|e| format!("submissions: write {}: {e}", path.display()))
    }

    /// Next sequential id, `S<n>` where n is one past the highest existing.
    pub fn next_id(&self) -> String {
        let max = self
            .records
            .iter()
            .filter_map(|r| r.id.strip_prefix('S').and_then(|n| n.parse::<u32>().ok()))
            .max()
            .unwrap_or(0);
        format!("S{}", max + 1)
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut SubmissionRecord> {
        self.records.iter_mut().find(|r| r.id.eq_ignore_ascii_case(id))
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.records.len();
        self.records.retain(|r| !r.id.eq_ignore_ascii_case(id));
        self.records.len() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trips_and_parses_aliases() {
        for s in SubmissionStatus::ALL {
            assert_eq!(SubmissionStatus::parse(s.label()), Some(s));
        }
        assert_eq!(SubmissionStatus::parse("PASS"), Some(SubmissionStatus::Rejected));
        assert_eq!(SubmissionStatus::parse("accepted"), Some(SubmissionStatus::Offer));
        assert!(SubmissionStatus::parse("nope").is_none());
        assert!(SubmissionStatus::Sent.is_open() && !SubmissionStatus::Offer.is_open());
    }

    #[test]
    fn ids_are_sequential_past_the_max() {
        let mut log = SubmissionLog::default();
        assert_eq!(log.next_id(), "S1");
        log.records.push(SubmissionRecord {
            id: "S1".into(),
            market: "Agency A".into(),
            agent: None,
            draft_ref: None,
            date_sent: None,
            status: SubmissionStatus::Drafting,
            response_date: None,
            next_action_date: None,
            notes: None,
            log: vec![],
        });
        log.records.push(SubmissionRecord {
            id: "S7".into(),
            market: "Agency B".into(),
            agent: None,
            draft_ref: None,
            date_sent: None,
            status: SubmissionStatus::Sent,
            response_date: None,
            next_action_date: None,
            notes: None,
            log: vec![],
        });
        assert_eq!(log.next_id(), "S8", "one past the highest, not the count");
        assert!(log.find_mut("s1").is_some(), "id match is case-insensitive");
        assert!(log.remove("S7") && log.records.len() == 1);
    }

    #[test]
    fn add_note_appends_timestamped_entries() {
        let mut rec = SubmissionRecord {
            id: "S1".into(),
            market: "Dream Lit".into(),
            agent: None,
            draft_ref: None,
            date_sent: None,
            status: SubmissionStatus::Sent,
            response_date: None,
            next_action_date: None,
            notes: None,
            log: vec![],
        };
        rec.add_note("got a call");
        rec.add_note("requested first 50 pages");
        assert_eq!(rec.log.len(), 2);
        assert_eq!(rec.log[0].text, "got a call");
        assert_eq!(rec.log[1].text, "requested first 50 pages");
        assert!(!rec.log[0].date.is_empty(), "each note is dated");
    }

    #[test]
    fn round_trips_through_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(SubmissionLog::load(root).unwrap().records.is_empty(), "missing → empty");
        let mut log = SubmissionLog::default();
        log.records.push(SubmissionRecord {
            id: "S1".into(),
            market: "Dream Lit".into(),
            agent: Some("A. Reader".into()),
            draft_ref: Some("query-letter".into()),
            date_sent: Some("2026-06-15".into()),
            status: SubmissionStatus::Sent,
            response_date: None,
            next_action_date: Some("2026-08-15".into()),
            notes: None,
            log: vec![],
        });
        log.save(root).unwrap();
        let back = SubmissionLog::load(root).unwrap();
        assert_eq!(back.records.len(), 1);
        assert_eq!(back.records[0].market, "Dream Lit");
        assert_eq!(back.records[0].status, SubmissionStatus::Sent);
        assert!(SubmissionLog::sidecar_path(root).ends_with(".inkhaven/submissions.json"));
    }
}
