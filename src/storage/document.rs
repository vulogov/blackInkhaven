//! Combined document store. Holds one JSON record (`json_docs`) and
//! one binary blob (`blobs`) per UUID, plus two entries in an HNSW
//! vector index per UUID — `"{uuid}:meta"` (embedded metadata
//! fingerprint) and `"{uuid}:content"` (embedded body text). Both
//! vector entries collapse to the same UUID at search time.
//!
//! This is the equivalent of bdslib's `documentstorage.rs`, trimmed
//! to the 11 methods inkhaven actually called. The `frequency.db`
//! sub-store that bdslib wrote on every insert (and that inkhaven
//! never read) is gone — projects that already have one on disk are
//! simply ignored.

use anyhow::{anyhow, Result};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;

use crate::storage::engine::{BlobStorage, JsonStorage};
use crate::storage::embedding::EmbeddingEngine;
use crate::storage::vector::{SearchResult, VectorEngine};

/// One document = one JSON metadata row + one blob + two HNSW
/// entries. Cloneable; every internal store is `Arc`-backed.
#[derive(Clone)]
pub struct DocumentStorage {
    meta:    JsonStorage,
    blobs:   BlobStorage,
    vectors: VectorEngine,
}

impl DocumentStorage {
    /// Open or create the document store rooted at `root`.
    ///
    /// Inkhaven's call site (`Store::open`) always supplies an
    /// embedding engine; the bdslib-style no-embedding constructor
    /// is gone.
    pub fn with_embedding(root: &str, engine: EmbeddingEngine) -> Result<Self> {
        let paths = Paths::from(root)?;
        Ok(Self {
            meta:    JsonStorage::new(&paths.metadata_db, 4, "doc")?,
            blobs:   BlobStorage::new(&paths.blobs_db, 4)?,
            vectors: VectorEngine::with_embedding(&paths.vec, engine)?,
        })
    }

    // ── writes ─────────────────────────────────────────────────────

    /// Generate a fresh UUIDv7, persist metadata + content + two
    /// vector entries, and return the new id.
    pub fn add_document(&self, metadata: JsonValue, content: &[u8]) -> Result<Uuid> {
        let id = Uuid::now_v7();
        let id_str = id.to_string();

        self.meta.add_json_with_id(id, metadata.clone())?;
        self.blobs.add_blob_with_key(id, content)?;

        let content_text = String::from_utf8_lossy(content).into_owned();
        self.vectors.store_documents_batch(&[
            (&format!("{id_str}:meta"), metadata),
            (&format!("{id_str}:content"), serde_json::json!(content_text)),
        ])?;
        Ok(id)
    }

    /// Variant that skips embedding. Used for snapshots and image
    /// blobs — both are stored in bdslib for backup-round-trip but
    /// shouldn't surface in semantic search.
    pub fn add_document_no_embed(&self, metadata: JsonValue, content: &[u8]) -> Result<Uuid> {
        let id = Uuid::now_v7();
        self.meta.add_json_with_id(id, metadata)?;
        self.blobs.add_blob_with_key(id, content)?;
        Ok(id)
    }

    pub fn update_metadata(&self, id: Uuid, metadata: JsonValue) -> Result<()> {
        self.meta.update_json(id, metadata)
    }

    pub fn update_content(&self, id: Uuid, content: &[u8]) -> Result<()> {
        self.blobs.update_blob(id, content)
    }

    /// Remove every trace of `id`: metadata, blob, both vector slots.
    pub fn delete_document(&self, id: Uuid) -> Result<()> {
        let id_str = id.to_string();
        self.meta.drop_json(id)?;
        self.blobs.drop_blob(id)?;
        self.vectors.delete_vector(&format!("{id_str}:meta"))?;
        self.vectors.delete_vector(&format!("{id_str}:content"))?;
        Ok(())
    }

    /// Re-embed both vector slots from the current metadata + blob.
    /// Called after `update_metadata` or `update_content` so the
    /// index stays in lockstep with the source of truth.
    pub fn reembed_document(&self, id: Uuid) -> Result<()> {
        let id_str = id.to_string();
        if let Some(metadata) = self.meta.get_json(id)? {
            self.vectors
                .store_document(&format!("{id_str}:meta"), metadata)?;
        }
        if let Some(bytes) = self.blobs.get_blob(id)? {
            let text = String::from_utf8_lossy(&bytes).into_owned();
            self.vectors.store_document(
                &format!("{id_str}:content"),
                serde_json::json!(text),
            )?;
        }
        Ok(())
    }

    // ── reads ──────────────────────────────────────────────────────

    pub fn get_content(&self, id: Uuid) -> Result<Option<Vec<u8>>> {
        self.blobs.get_blob(id)
    }

    pub fn list_metadata(&self) -> Result<Vec<(Uuid, JsonValue)>> {
        self.meta.list_all()
    }

    // ── search ─────────────────────────────────────────────────────

    /// Embed `query` as plain text and return the top `limit`
    /// documents. The HNSW index holds two slots per document; the
    /// dedup pass in [`build_results`] keeps only the higher-scoring
    /// slot per UUID so each document appears at most once.
    pub fn search_document_text(&self, query: &str, limit: usize) -> Result<Vec<JsonValue>> {
        // Over-fetch so :meta and :content slots compete fairly
        // (matches bdslib's behaviour: pool of 4×limit candidates).
        let pool = limit.max(1) * 4;
        let candidates = self
            .vectors
            .search_json(&serde_json::json!(query), pool)?;
        self.build_results(candidates, limit)
    }

    // ── persistence ────────────────────────────────────────────────

    /// Flush the vector index to disk. DuckDB's blob + json stores
    /// checkpoint themselves; only vecstore needs an explicit save.
    pub fn sync(&self) -> Result<()> {
        self.vectors.sync()
    }

    // ── internals ──────────────────────────────────────────────────

    fn build_results(
        &self,
        candidates: Vec<SearchResult>,
        limit: usize,
    ) -> Result<Vec<JsonValue>> {
        let mut best: HashMap<String, f32> = HashMap::new();
        for r in &candidates {
            let uuid_str = strip_suffix(&r.id).to_string();
            let entry = best.entry(uuid_str).or_insert(f32::NEG_INFINITY);
            if r.score > *entry {
                *entry = r.score;
            }
        }

        let mut ranked: Vec<(String, f32)> = best.into_iter().collect();
        ranked.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });
        ranked.truncate(limit);

        let mut out = Vec::with_capacity(ranked.len());
        for (uuid_str, score) in ranked {
            let uuid = Uuid::parse_str(&uuid_str)
                .map_err(|e| anyhow!("invalid UUID in vector index: {e}"))?;
            let metadata = self.meta.get_json(uuid)?.unwrap_or(JsonValue::Null);
            let content_bytes = self.blobs.get_blob(uuid)?.unwrap_or_default();
            let document = String::from_utf8_lossy(&content_bytes).into_owned();
            out.push(serde_json::json!({
                "id":       uuid_str,
                "metadata": metadata,
                "document": document,
                "score":    score,
            }));
        }
        Ok(out)
    }
}

fn strip_suffix(id: &str) -> &str {
    id.strip_suffix(":meta")
        .or_else(|| id.strip_suffix(":content"))
        .unwrap_or(id)
}

// ── directory layout ────────────────────────────────────────────────

struct Paths {
    metadata_db: String,
    blobs_db:    String,
    vec:         String,
}

impl Paths {
    fn from(root: &str) -> Result<Self> {
        let root = Path::new(root);
        std::fs::create_dir_all(root)
            .map_err(|e| anyhow!("cannot create root dir {root:?}: {e}"))?;
        std::fs::create_dir_all(root.join("vectors"))
            .map_err(|e| anyhow!("cannot create vectors dir: {e}"))?;
        Ok(Self {
            metadata_db: root.join("metadata.db").to_string_lossy().into_owned(),
            blobs_db:    root.join("blobs.db").to_string_lossy().into_owned(),
            vec:         root.join("vectors").to_string_lossy().into_owned(),
        })
    }
}
