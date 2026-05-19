//! Combined telemetry + document-store search for [`ShardsManager`].
//!
//! [`ShardsManager::aggregationsearch`] fires two independent searches in
//! parallel (via Rayon) and merges their results into a single JSON object:
//!
//! ```json
//! {
//!   "observability": [ /* telemetry records from search_vector, score-ranked */ ],
//!   "documents":     [ /* document-store hits from doc_search_text */ ]
//! }
//! ```
//!
//! Both searches share the same plain-text `query`.  Telemetry is filtered to
//! the `duration` lookback window; the document store is global (no time
//! window).  The plain-text query is wrapped as a JSON string value before
//! being passed to `search_vector`, which fingerprints and embeds it with the
//! shared `AllMiniLML6V2` model.

use crate::common::error::Result;
use crate::shardsmanager::ShardsManager;
use serde_json::{json, Value as JsonValue};

/// Default number of document-store results returned per aggregation call.
const DEFAULT_DOC_LIMIT: usize = 10;

impl ShardsManager {
    /// Run a telemetry vector search and a document-store semantic search
    /// concurrently and return their results merged under a single JSON object.
    ///
    /// The two searches are dispatched in parallel via `rayon::join`.  Both
    /// complete before the function returns; the caller receives a single
    /// `Result` that propagates the first error (if any).
    ///
    /// Telemetry results are returned by [`search_vector`], which embeds the
    /// query with the shared `AllMiniLML6V2` model, queries the HNSW index
    /// across every shard in the lookback window, and ranks hits by cosine
    /// similarity descending.  Each result includes a `"_score"` field and an
    /// embedded `"secondaries"` array.
    ///
    /// [`search_vector`]: ShardsManager::search_vector
    ///
    /// # Parameters
    /// - `duration` — lookback window for the telemetry search in
    ///   [`humantime`] format, e.g. `"1h"`, `"30min"`, `"7days"`.
    /// - `query` — plain-text query used for both searches.
    ///
    /// # Returns
    /// ```json
    /// {
    ///   "observability": [ /* full telemetry documents, vector-ranked by _score */ ],
    ///   "documents":     [ /* doc-store hits with id/metadata/document/score */ ]
    /// }
    /// ```
    pub fn aggregationsearch(&self, duration: &str, query: &str) -> Result<JsonValue> {
        let mgr_tel = self.clone();
        let mgr_doc = self.clone();

        let duration = duration.to_owned();
        let query_vec = json!(query);   // Value::String — fingerprinted then embedded by search_vector
        let query_doc = query.to_owned();

        let (obs_result, doc_result) = rayon::join(
            move || mgr_tel.search_vector(&duration, &query_vec),
            move || mgr_doc.doc_search_text(&query_doc, DEFAULT_DOC_LIMIT),
        );

        Ok(json!({
            "observability": obs_result?,
            "documents":     doc_result?,
        }))
    }
}
