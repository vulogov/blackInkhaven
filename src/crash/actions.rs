//! Recent-action ring buffer used by the crash report.
//!
//! Captures the last [`super::ACTION_RING_CAP`] user
//! actions in chronological order.  Each entry has a
//! timestamp + an action name + optional argument
//! string; the App pushes one entry per dispatched
//! action.  The ring is what the user reads in the
//! crash report to remember "what was I doing when it
//! crashed".

use serde::{Deserialize, Serialize};

use super::ACTION_RING_CAP;

/// One entry in the recent-action ring.  Serializable
/// directly into the crash-report HJSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRecord {
    /// UTC ISO 8601 with seconds resolution.  Enough
    /// granularity to reconstruct sequence; not so much
    /// that the timestamp dominates the on-disk size.
    pub at: String,
    /// Action name as recognised by `inkhaven::keybind::Action`
    /// or a free-form label for non-bound actions (panic,
    /// load, save, etc.).
    pub action: String,
    /// Optional free-form details — selection range,
    /// target paragraph slug, etc.  Kept compact;
    /// callers should not put large blobs here.
    pub detail: Option<String>,
}

impl ActionRecord {
    pub fn new(action: impl Into<String>) -> Self {
        Self {
            at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            action: action.into(),
            detail: None,
        }
    }

    pub fn with_detail(action: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            action: action.into(),
            detail: Some(detail.into()),
        }
    }
}

/// Capped chronological ring.  `push` appends to the
/// back; once the ring is at capacity, the oldest entry
/// drops off the front.  Cloning is cheap — it's
/// bounded by [`ACTION_RING_CAP`].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActionRing {
    pub entries: std::collections::VecDeque<ActionRecord>,
}

impl ActionRing {
    pub fn push(&mut self, record: ActionRecord) {
        if self.entries.len() >= ACTION_RING_CAP {
            self.entries.pop_front();
        }
        self.entries.push_back(record);
    }

    /// Consumed by tests + the recover CLI in R.2.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Consumed by tests + the recover CLI in R.2.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_starts_empty() {
        let r = ActionRing::default();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn ring_caps_at_action_ring_cap() {
        let mut r = ActionRing::default();
        for i in 0..ACTION_RING_CAP + 25 {
            r.push(ActionRecord::new(format!("a{i}")));
        }
        assert_eq!(r.len(), ACTION_RING_CAP);
        // Oldest survivor should be `a25` (we pushed 75
        // when cap is 50, so first 25 fell off).
        assert_eq!(
            r.entries.front().unwrap().action,
            format!("a{}", 25)
        );
        // Newest entry is the last we pushed.
        assert_eq!(
            r.entries.back().unwrap().action,
            format!("a{}", ACTION_RING_CAP + 25 - 1)
        );
    }

    #[test]
    fn with_detail_preserves_action_and_detail() {
        let r = ActionRecord::with_detail("save", "para=01-opening");
        assert_eq!(r.action, "save");
        assert_eq!(r.detail.as_deref(), Some("para=01-opening"));
    }

    #[test]
    fn at_field_has_iso8601_z_shape() {
        let r = ActionRecord::new("test");
        // 2026-05-31T14:23:00Z is 20 chars; we just
        // pin shape, not exact value.
        assert_eq!(r.at.len(), 20);
        assert!(r.at.ends_with('Z'));
        assert!(r.at.contains('T'));
    }
}
