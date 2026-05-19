//! TextRank summarisation over drain3 templates stored in shard tplstorage.
//!
//! [`ShardsManager::textrank_templates`] gathers every template observed in
//! the lookback window, converts each one into a stable JSON fingerprint
//! string, and feeds the list into the
//! [`textrank_summary_with`](crate::textrank_summary_with) extractive
//! summariser.  The output is a single human-readable string built from the
//! highest-ranked fingerprints, suitable for rendering in dashboards or
//! piping into a follow-up LLM prompt.
//!
//! The lookback `duration` is taken as a [`std::time::Duration`] (typically
//! produced via [`humantime::parse_duration`]); the function computes
//! `[now − duration, now]` internally and queries only shards that overlap
//! that window — the same routing rule used by
//! [`ShardsManager::templates_by_timestamp`].

use crate::common::error::Result;
use crate::common::jsonfingerprint::json_fingerprint;
use crate::shardsmanager::ShardsManager;
use crate::{textrank_summary_with, TextRankConfig};
use serde_json::{Map, Value as JsonValue};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

impl ShardsManager {
    /// Summarise drain3 templates observed in the last `lookback` window.
    ///
    /// Every template observed within `[now − lookback, now]` is retrieved
    /// (metadata + body), serialised into a JSON object, and fingerprinted
    /// via [`json_fingerprint`].  The resulting list of strings is fed to
    /// [`textrank_summary_with`] using the supplied [`TextRankConfig`], and
    /// the joined summary string is returned.
    ///
    /// # Parameters
    /// - `_session_id` — UUIDv7 of the calling session.  Currently accepted
    ///   for parity with other `ShardsManager` query methods; not consulted
    ///   internally (templates are global within the lookback window).
    /// - `lookback` — how far back to look for template observations.
    ///   Convert humantime strings such as `"1h"` via
    ///   [`humantime::parse_duration`].
    /// - `cfg` — TextRank tuning knobs (max sentences, damping, etc.).
    ///
    /// # Returns
    /// The TextRank summary string.  Empty when no templates were observed
    /// in the window.
    pub fn textrank_templates(
        &self,
        _session_id: Uuid,
        lookback: Duration,
        cfg: &TextRankConfig,
    ) -> Result<String> {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let start_secs = now_secs.saturating_sub(lookback.as_secs());

        let templates = self.templates_by_timestamp(start_secs, now_secs)?;
        if templates.is_empty() {
            return Ok(String::new());
        }

        let fingerprints: Vec<String> = templates
            .iter()
            .map(template_to_fingerprint)
            .collect();

        Ok(textrank_summary_with(&fingerprints, cfg))
    }
}

/// Top-level metadata keys that carry time / observation-cycle data and
/// therefore vary per record even when the underlying template content is
/// identical.  Excluded from fingerprinting so that templates differing only
/// in observation timestamp produce identical fingerprints, which is what
/// the cosine-similarity graph in TextRank actually wants to see.
const TIME_KEYS: &[&str] = &["timestamp", "created_at", "updated_at", "ts", "time"];

/// Build a stable fingerprint string from a single `{id, metadata, body}`
/// template record.
///
/// The metadata object (if present) is stripped of timestamp-style keys,
/// merged with the body string under a dedicated `"body"` key, and run
/// through [`json_fingerprint`].  Removing the timestamp tokens lets two
/// otherwise-identical templates produce identical fingerprints — the
/// signal TextRank needs to surface recurring patterns instead of treating
/// every observation as unique.
fn template_to_fingerprint(tpl: &JsonValue) -> String {
    let mut obj: Map<String, JsonValue> = match tpl.get("metadata") {
        Some(JsonValue::Object(m)) => m.clone(),
        _ => Map::new(),
    };
    for k in TIME_KEYS {
        obj.remove(*k);
    }
    if let Some(body) = tpl.get("body").and_then(|v| v.as_str()) {
        if !body.is_empty() {
            obj.insert("body".to_owned(), JsonValue::String(body.to_owned()));
        }
    }
    if obj.is_empty() {
        // Fallback: fingerprint the entire record so we don't lose the input.
        return json_fingerprint(tpl);
    }
    json_fingerprint(&JsonValue::Object(obj))
}
