//! Signal-store helpers on [`ShardsManager`].
//!
//! Signals are lightweight events emitted by bdsnode components (rules engines,
//! anomaly detectors, external integrations) and stored in a dedicated
//! [`DocumentStorage`] at `{dbpath}/signals`.
//!
//! Every signal has three required metadata fields:
//! - `"name"` — signal identifier / category
//! - `"severity"` — e.g. `"info"`, `"warning"`, `"critical"`
//! - `"timestamp"` — Unix seconds when the signal occurred
//!
//! Content is always empty (`b""`); all semantics live in the metadata.

use crate::common::error::Result;
use crate::shardsmanager::ShardsManager;
use serde_json::Value as JsonValue;
use uuid::Uuid;

impl ShardsManager {
    /// Emit a new signal and return its UUIDv7.
    ///
    /// The signal is stored as a document with empty content. `name`, `severity`,
    /// and `timestamp` are mandatory fields. `extra` may contain any additional
    /// key/value pairs to merge into the metadata; the three mandatory fields
    /// always take precedence over keys in `extra`.
    pub fn signal_emit(
        &self,
        name:      &str,
        severity:  &str,
        timestamp: u64,
        extra:     serde_json::Map<String, serde_json::Value>,
    ) -> Result<Uuid> {
        let mut meta = extra;
        meta.insert("name".to_owned(),      serde_json::json!(name));
        meta.insert("severity".to_owned(),  serde_json::json!(severity));
        meta.insert("timestamp".to_owned(), serde_json::json!(timestamp));
        self.signals.add_document(serde_json::Value::Object(meta), b"")
    }

    /// Replace the metadata for signal `id` in-place.
    ///
    /// Returns `Ok(())` even when `id` does not exist (no-op).
    pub fn signal_update(&self, id: Uuid, metadata: JsonValue) -> Result<()> {
        self.signals.update_metadata(id, metadata)
    }

    /// Return distinct signal IDs that were emitted within the humantime
    /// lookback window `duration` (e.g. `"30s"`, `"5min"`, `"1h"`).
    pub fn signals_recent(&self, duration: &str) -> Result<Vec<String>> {
        self.signals.frequencytracking_recent(duration)
    }

    /// Semantic search over signals by plain-text query.
    ///
    /// Returns the `limit` most relevant signals, each as a JSON object with
    /// keys `"id"`, `"metadata"`, `"document"`, and `"score"`.
    pub fn signals_query(&self, query: &str, limit: usize) -> Result<Vec<JsonValue>> {
        self.signals.search_document_text(query, limit)
    }

    /// Return the metadata stored under signal `id`, or `None` if not found.
    pub fn signal_get(&self, id: Uuid) -> Result<Option<JsonValue>> {
        self.signals.get_metadata(id)
    }
}
