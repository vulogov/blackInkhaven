use crate::common::error::{err_msg, Result};
use crate::EmbeddingEngine;
use parking_lot::Mutex;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use vecstore::reranking::Reranker;
use vecstore::{Metadata, Query, VecStore};

pub use crate::common::jsonfingerprint::json_fingerprint;
pub use vecstore::Neighbor as SearchResult;

/// Thread-safe vector store backed by vecstore's HNSW index.
///
/// Wraps `vecstore::VecStore` behind an `Arc<Mutex<_>>` so it can be cloned
/// and shared across threads. An optional [`EmbeddingEngine`] enables
/// automatic text-to-vector conversion via [`store_document`] and
/// [`search_json`].
///
/// The underlying `VecStore` (HNSW index) is opened lazily on the first
/// vector operation, not at construction time. This avoids deserialising the
/// full binary index when a shard is opened for pure DuckDB queries.
///
/// [`store_document`]: VectorEngine::store_document
/// [`search_json`]: VectorEngine::search_json
#[derive(Clone)]
pub struct VectorEngine {
    path: String,
    store: Arc<Mutex<Option<VecStore>>>,
    embedding: Option<Arc<EmbeddingEngine>>,
}

impl VectorEngine {
    /// Create a `VectorEngine` for the store at `path`.
    ///
    /// The HNSW index is NOT opened until the first vector read or write.
    /// `store_document` and `search_json` are not available on engines created
    /// with `new`; use [`with_embedding`] instead.
    ///
    /// [`with_embedding`]: VectorEngine::with_embedding
    pub fn new(path: &str) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
            store: Arc::new(Mutex::new(None)),
            embedding: None,
        })
    }

    /// Create a `VectorEngine` for the store at `path`, with an
    /// [`EmbeddingEngine`] for automatic text embedding via [`store_document`]
    /// and [`search_json`].
    ///
    /// The HNSW index is NOT opened until the first vector read or write.
    ///
    /// [`store_document`]: VectorEngine::store_document
    /// [`search_json`]: VectorEngine::search_json
    pub fn with_embedding(path: &str, engine: EmbeddingEngine) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
            store: Arc::new(Mutex::new(None)),
            embedding: Some(Arc::new(engine)),
        })
    }

    // ── writes ────────────────────────────────────────────────────────────────

    /// Store an `id → vector` association.
    ///
    /// `metadata` is an optional JSON object whose fields are stored alongside
    /// the vector and returned in search results. Pass `None` for no metadata.
    ///
    /// If a record with the same `id` already exists it is replaced (upsert).
    pub fn store_vector(
        &self,
        id: &str,
        vector: Vec<f32>,
        metadata: Option<JsonValue>,
    ) -> Result<()> {
        let meta = json_to_metadata(metadata.unwrap_or(JsonValue::Object(Default::default())));
        self.with_store(|s| {
            s.upsert(id.to_string(), vector, meta)
                .map_err(|e| err_msg(format!("Failed to store vector {id:?}: {e}")))
        })
    }

    /// Bulk-upsert `(id, vector, metadata)` triples under a single
    /// store-lock acquisition.
    ///
    /// Equivalent in effect to calling [`store_vector`] N times, but pays
    /// the `Mutex<Option<VecStore>>` lock cost only once for the whole
    /// batch. Used by `Shard::add_batch` to coalesce per-primary HNSW
    /// upserts inside one critical section.
    ///
    /// `entries` items take ownership of their vector + metadata. Empty
    /// input is a no-op. On the first failed upsert the helper returns
    /// immediately; entries already upserted in this call are not rolled
    /// back (HNSW has no transaction primitive).
    ///
    /// [`store_vector`]: VectorEngine::store_vector
    pub fn store_vectors_batch(
        &self,
        entries: Vec<(String, Vec<f32>, Option<JsonValue>)>,
    ) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        self.with_store(|s| {
            for (id, vector, metadata) in entries {
                let meta = json_to_metadata(
                    metadata.unwrap_or(JsonValue::Object(Default::default())),
                );
                s.upsert(id.clone(), vector, meta)
                    .map_err(|e| err_msg(format!("Failed to store vector {id:?}: {e}")))?;
            }
            Ok(())
        })
    }

    /// Embed `document` using the attached [`EmbeddingEngine`] and store the
    /// resulting vector under `id`.
    ///
    /// The document is converted to a fingerprint string via [`json_fingerprint`]
    /// before embedding. The full JSON is persisted as metadata and returned in
    /// search results.
    ///
    /// Returns `Ok(())` silently when no `EmbeddingEngine` was configured (no-op).
    /// Returns `Err` only when an engine is present but embedding or storage fails.
    pub fn store_document(&self, id: &str, document: JsonValue) -> Result<()> {
        let Some(engine) = &self.embedding else { return Ok(()); };
        let fingerprint = json_fingerprint(&document);
        let vector = engine.embed(&fingerprint)?;
        let meta = json_to_metadata(document);
        self.with_store(|s| {
            s.upsert(id.to_string(), vector, meta)
                .map_err(|e| err_msg(format!("Failed to store document {id:?}: {e}")))
        })
    }

    /// Embed and store multiple `(id, document)` pairs in a single ONNX batch
    /// inference pass.
    ///
    /// All fingerprints are embedded together, then all vectors are upserted
    /// under one store-lock acquisition. Significantly faster than calling
    /// [`store_document`] N times when inserting several documents at once.
    ///
    /// Silently skips all entries when no [`EmbeddingEngine`] is configured.
    ///
    /// [`store_document`]: VectorEngine::store_document
    pub fn store_documents_batch(&self, entries: &[(&str, JsonValue)]) -> Result<()> {
        let Some(engine) = &self.embedding else { return Ok(()); };
        if entries.is_empty() {
            return Ok(());
        }
        let fingerprints: Vec<String> = entries.iter()
            .map(|(_, doc)| json_fingerprint(doc))
            .collect();
        let fp_refs: Vec<&str> = fingerprints.iter().map(String::as_str).collect();
        let vectors = engine.embed_batch(&fp_refs)?;
        self.with_store(|s| {
            for ((id, doc), vector) in entries.iter().zip(vectors) {
                let meta = json_to_metadata(doc.clone());
                s.upsert(id.to_string(), vector, meta)
                    .map_err(|e| err_msg(format!("Failed to store document {id:?}: {e}")))?;
            }
            Ok(())
        })
    }

    /// Remove the vector stored under `id`.
    ///
    /// Returns `Ok(())` silently if no record exists for `id`.
    pub fn delete_vector(&self, id: &str) -> Result<()> {
        self.with_store(|s| match s.remove(id) {
            Ok(()) => Ok(()),
            Err(e) if e.to_string().to_lowercase().contains("not found") => Ok(()),
            Err(e) => Err(err_msg(format!("Failed to remove vector {id:?}: {e}"))),
        })
    }

    // ── searches ──────────────────────────────────────────────────────────────

    /// Return the `limit` nearest neighbours to `query_vector`, ordered by
    /// descending similarity score (1.0 = identical, 0.0 = orthogonal).
    pub fn search(&self, query_vector: Vec<f32>, limit: usize) -> Result<Vec<SearchResult>> {
        let q = Query::new(query_vector).with_limit(limit);
        let mut results = self.with_store(|s| {
            s.query(q).map_err(|e| err_msg(format!("Vector search failed: {e}")))
        })?;
        distance_to_similarity(&mut results);
        Ok(results)
    }

    /// Search for the `candidate_pool` nearest neighbours, then re-rank with
    /// `reranker` and return the top `limit` results.
    ///
    /// `query_text` is forwarded to the reranker for semantic scoring (e.g.
    /// cross-encoder models). Pass an empty string for rerankers that do not
    /// use query text (e.g. MMR).
    pub fn search_reranked(
        &self,
        query_vector: Vec<f32>,
        query_text: &str,
        limit: usize,
        candidate_pool: usize,
        reranker: &dyn Reranker,
    ) -> Result<Vec<SearchResult>> {
        let pool = candidate_pool.max(limit);
        let q = Query::new(query_vector).with_limit(pool);
        let mut candidates = self.with_store(|s| {
            s.query(q).map_err(|e| err_msg(format!("Vector search failed: {e}")))
        })?;
        // Convert before reranking: rerankers treat score as similarity (higher = better).
        distance_to_similarity(&mut candidates);
        reranker
            .rerank(query_text, candidates, limit)
            .map_err(|e| err_msg(format!("Reranking failed: {e}")))
    }

    /// Fingerprint `query` using [`json_fingerprint`], embed the result, and
    /// return the `limit` nearest stored documents.
    ///
    /// Use the same JSON structure as was passed to [`store_document`] so that
    /// field paths in the query align with field paths in the index.
    ///
    /// Returns `Err` if no `EmbeddingEngine` was provided at construction time.
    pub fn search_json(&self, query: &JsonValue, limit: usize) -> Result<Vec<SearchResult>> {
        let engine = self.require_embedding("search_json")?;
        let fingerprint = json_fingerprint(query);
        let vector = engine.embed(&fingerprint)?;
        self.search(vector, limit)
    }

    /// Fingerprint `query`, embed it, search `candidate_pool` neighbours, then
    /// re-rank with `reranker` and return the top `limit` results.
    ///
    /// The fingerprint string is also passed as `query_text` to the reranker,
    /// so semantic rerankers (e.g. cross-encoder) receive meaningful input.
    ///
    /// Returns `Err` if no `EmbeddingEngine` was provided at construction time.
    pub fn search_json_reranked(
        &self,
        query: &JsonValue,
        limit: usize,
        candidate_pool: usize,
        reranker: &dyn Reranker,
    ) -> Result<Vec<SearchResult>> {
        let engine = self.require_embedding("search_json_reranked")?;
        let fingerprint = json_fingerprint(query);
        let vector = engine.embed(&fingerprint)?;
        self.search_reranked(vector, &fingerprint, limit, candidate_pool, reranker)
    }

    // ── persistence ───────────────────────────────────────────────────────────

    /// Flush the in-memory index and all records to disk.
    ///
    /// No-op when the store has never been opened (no vector writes or reads
    /// have occurred). For file-backed stores this is necessary to persist
    /// changes across process restarts.
    pub fn sync(&self) -> Result<()> {
        let mut guard = self.store.lock();
        let Some(s) = guard.as_mut() else { return Ok(()); };
        s.save().map_err(|e| err_msg(format!("Failed to sync vector store: {e}")))
    }

    // ── internal ──────────────────────────────────────────────────────────────

    /// Open the VecStore on first access, then call `f` with a mutable ref.
    fn with_store<R, F: FnOnce(&mut VecStore) -> Result<R>>(&self, f: F) -> Result<R> {
        let mut guard = self.store.lock();
        if guard.is_none() {
            *guard = Some(
                VecStore::open(&self.path).map_err(|e| {
                    err_msg(format!("Failed to open vector store at {:?}: {e}", self.path))
                })?,
            );
        }
        f(guard.as_mut().unwrap())
    }

    fn require_embedding(&self, caller: &str) -> Result<Arc<EmbeddingEngine>> {
        self.embedding.clone().ok_or_else(|| {
            err_msg(format!(
                "{caller} requires an EmbeddingEngine — use VectorEngine::with_embedding"
            ))
        })
    }
}

// ── score conversion ──────────────────────────────────────────────────────────

// vecstore returns cosine *distance* (lower = more similar). Convert in-place
// to cosine *similarity* (higher = more similar) so that callers and rerankers
// both see the natural convention: score 1.0 = identical, 0.0 = orthogonal.
fn distance_to_similarity(results: &mut Vec<SearchResult>) {
    for r in results.iter_mut() {
        r.score = 1.0 - r.score;
    }
}

// ── metadata conversion ───────────────────────────────────────────────────────

fn json_to_metadata(json: JsonValue) -> Metadata {
    let fields = match json {
        JsonValue::Object(map) => map.into_iter().collect(),
        other => {
            let mut m = HashMap::new();
            m.insert("value".to_string(), other);
            m
        }
    };
    Metadata { fields }
}
