//! HNSW vector index backed by the `vecstore` crate. Mirrors the
//! ergonomics of bdslib's `vectorengine.rs` — same lazy open, same
//! cosine-distance-to-similarity score flip — but with the reranker
//! pathway and unused batch/single-doc helpers removed.

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use vecstore::{Metadata, Query, VecStore};

pub use vecstore::Neighbor as SearchResult;

use crate::storage::embedding::EmbeddingEngine;
use crate::storage::fingerprint::json_fingerprint;

/// Thread-safe HNSW index wrapper. The underlying `VecStore` is opened
/// lazily on the first vector operation — important when a project is
/// opened purely to read DuckDB metadata (e.g. CLI `list`) and the
/// vector index would otherwise be deserialised for no reason.
#[derive(Clone)]
pub struct VectorEngine {
    path: String,
    store: Arc<Mutex<Option<VecStore>>>,
    embedding: Option<Arc<EmbeddingEngine>>,
}

impl VectorEngine {
    pub fn with_embedding(path: &str, engine: EmbeddingEngine) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
            store: Arc::new(Mutex::new(None)),
            embedding: Some(Arc::new(engine)),
        })
    }

    /// Fingerprint `document`, embed the result via the attached
    /// engine, and upsert under `id`. No-op when no engine is set
    /// (kept to match bdslib's behaviour even though inkhaven always
    /// constructs with one).
    pub fn store_document(&self, id: &str, document: JsonValue) -> Result<()> {
        let Some(engine) = &self.embedding else {
            return Ok(());
        };
        let fingerprint = json_fingerprint(&document);
        let vector = engine.embed(&fingerprint)?;
        let meta = json_to_metadata(document);
        self.with_store(|s| {
            s.upsert(id.to_string(), vector, meta)
                .map_err(|e| anyhow!("failed to store document {id:?}: {e}"))
        })
    }

    /// Batch variant — embed N documents in one ONNX pass, then upsert
    /// each one. Used by `DocumentStorage::add_document` (two entries
    /// per call: `:meta` + `:content`).
    pub fn store_documents_batch(&self, entries: &[(&str, JsonValue)]) -> Result<()> {
        let Some(engine) = &self.embedding else {
            return Ok(());
        };
        if entries.is_empty() {
            return Ok(());
        }
        let fingerprints: Vec<String> = entries
            .iter()
            .map(|(_, doc)| json_fingerprint(doc))
            .collect();
        let fp_refs: Vec<&str> = fingerprints.iter().map(String::as_str).collect();
        let vectors = engine.embed_batch(&fp_refs)?;
        self.with_store(|s| {
            for ((id, doc), vector) in entries.iter().zip(vectors) {
                let meta = json_to_metadata(doc.clone());
                s.upsert(id.to_string(), vector, meta)
                    .map_err(|e| anyhow!("failed to store document {id:?}: {e}"))?;
            }
            Ok(())
        })
    }

    pub fn delete_vector(&self, id: &str) -> Result<()> {
        self.with_store(|s| match s.remove(id) {
            Ok(()) => Ok(()),
            Err(e) if e.to_string().to_lowercase().contains("not found") => Ok(()),
            Err(e) => Err(anyhow!("failed to remove vector {id:?}: {e}")),
        })
    }

    /// Search by a pre-computed query vector. Returns up to `limit`
    /// neighbours with `score` already flipped from cosine distance
    /// (lower-is-closer) to cosine similarity (higher-is-closer) so
    /// callers downstream can compare against a natural threshold.
    pub fn search(&self, query_vector: Vec<f32>, limit: usize) -> Result<Vec<SearchResult>> {
        let q = Query::new(query_vector).with_limit(limit);
        let mut results = self
            .with_store(|s| s.query(q).map_err(|e| anyhow!("vector search failed: {e}")))?;
        distance_to_similarity(&mut results);
        Ok(results)
    }

    /// Fingerprint `query`, embed it, then [`search`].
    pub fn search_json(&self, query: &JsonValue, limit: usize) -> Result<Vec<SearchResult>> {
        let engine = self
            .embedding
            .clone()
            .ok_or_else(|| anyhow!("search_json requires an EmbeddingEngine"))?;
        let fingerprint = json_fingerprint(query);
        let vector = engine.embed(&fingerprint)?;
        self.search(vector, limit)
    }

    pub fn sync(&self) -> Result<()> {
        let mut guard = self.store.lock();
        let Some(s) = guard.as_mut() else {
            return Ok(());
        };
        s.save()
            .map_err(|e| anyhow!("failed to sync vector store: {e}"))
    }

    fn with_store<R, F: FnOnce(&mut VecStore) -> Result<R>>(&self, f: F) -> Result<R> {
        let mut guard = self.store.lock();
        if guard.is_none() {
            *guard = Some(
                VecStore::open(&self.path)
                    .map_err(|e| anyhow!("failed to open vector store at {:?}: {e}", self.path))?,
            );
        }
        f(guard.as_mut().unwrap())
    }
}

// vecstore returns cosine *distance* (lower = more similar). Convert
// in-place to cosine *similarity* so callers see the natural
// convention: 1.0 = identical, 0.0 = orthogonal.
fn distance_to_similarity(results: &mut [SearchResult]) {
    for r in results.iter_mut() {
        r.score = 1.0 - r.score;
    }
}

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
