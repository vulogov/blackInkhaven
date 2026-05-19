use crate::common::error::{err_msg, Result};
use crate::common::jsonfingerprint::json_fingerprint;
use crate::common::time::now_secs;
use crate::common::uuid::generate_v7;
use crate::datastorage::{BlobStorage, JsonStorage, JsonStorageConfig};
use crate::frequencytrackingstorage::FrequencyTracking;
use crate::EmbeddingEngine;
use crate::VectorEngine;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;

// ── directory layout ──────────────────────────────────────────────────────────
//
//  {root}/
//  ├── metadata.db    JsonStorage  — per-document JSON metadata
//  ├── blobs.db       BlobStorage  — raw document bytes
//  └── vectors/       VectorEngine — combined index; IDs: "{uuid}:meta" and
//                                   "{uuid}:content" in the same HNSW store

/// Combined document store: JSON metadata, raw byte content, and a single
/// vector index that holds both metadata-fingerprint embeddings and
/// document-content embeddings.
///
/// Every document is identified by a single UUIDv7. Two vector entries share
/// that UUID as their prefix — `"{uuid}:meta"` for the metadata embedding and
/// `"{uuid}:content"` for the content embedding — so a single
/// [`search_document`] call can surface a document via either signal.
///
/// # Search model
///
/// [`search_document`] queries the unified vector index, deduplicates raw hits
/// by UUID (keeping the best score per document), sorts by descending
/// similarity, and returns the top `limit` documents as JSON objects with
/// three keys:
///
/// ```json
/// { "id": "<uuid>", "metadata": { … }, "document": "<utf-8 content>", "score": 0.97 }
/// ```
///
/// # Graceful degradation
///
/// When created via [`new`] (no embedding engine), [`add_document`] still
/// stores metadata and blob; vector indexing is silently skipped. Call
/// [`add_document_with_vectors`] to supply pre-computed vectors explicitly, or
/// use [`with_embedding`] to enable automatic embedding on every insert.
///
/// # `Clone`-able
///
/// All internal stores are backed by `Arc`; cloning is cheap and all clones
/// share the same underlying state.
///
/// [`new`]: DocumentStorage::new
/// [`with_embedding`]: DocumentStorage::with_embedding
/// [`add_document`]: DocumentStorage::add_document
/// [`add_document_with_vectors`]: DocumentStorage::add_document_with_vectors
/// [`search_document`]: DocumentStorage::search_document
#[derive(Clone)]
pub struct DocumentStorage {
    meta:      JsonStorage,
    blobs:     BlobStorage,
    vectors:   VectorEngine,
    frequency: FrequencyTracking,
}

impl DocumentStorage {
    /// Open or create a `DocumentStorage` rooted at `root`.
    ///
    /// The directory tree is created automatically. Vector indexing requires an
    /// embedding engine; use [`with_embedding`] to enable it.
    ///
    /// [`with_embedding`]: DocumentStorage::with_embedding
    pub fn new(root: &str) -> Result<Self> {
        let paths = Paths::from(root)?;
        Ok(Self {
            meta:      JsonStorage::new(&paths.metadata_db, 4, meta_config())?,
            blobs:     BlobStorage::new(&paths.blobs_db, 4)?,
            vectors:   VectorEngine::new(&paths.vec)?,
            frequency: FrequencyTracking::new(&paths.frequency_db, 4)?,
        })
    }

    /// Open or create a `DocumentStorage` rooted at `root`, with an
    /// [`EmbeddingEngine`] for automatic vector indexing.
    ///
    /// [`add_document`] will embed the JSON metadata fingerprint as
    /// `"{uuid}:meta"` and the document text as `"{uuid}:content"` into the
    /// shared vector index.
    ///
    /// [`add_document`]: DocumentStorage::add_document
    pub fn with_embedding(root: &str, engine: EmbeddingEngine) -> Result<Self> {
        let paths = Paths::from(root)?;
        Ok(Self {
            meta:      JsonStorage::new(&paths.metadata_db, 4, meta_config())?,
            blobs:     BlobStorage::new(&paths.blobs_db, 4)?,
            vectors:   VectorEngine::with_embedding(&paths.vec, engine)?,
            frequency: FrequencyTracking::new(&paths.frequency_db, 4)?,
        })
    }

    // ── writes ────────────────────────────────────────────────────────────────

    /// Store a document.
    ///
    /// - `metadata` is stored verbatim in the JSON store.
    /// - `content` is stored as raw bytes in the blob store.
    /// - If an embedding engine is configured, the `json_fingerprint` of
    ///   `metadata` is stored as `"{uuid}:meta"` and `content` (decoded as
    ///   UTF-8) is stored as `"{uuid}:content"` in the shared vector index.
    ///   Vector indexing is silently skipped when no engine is present.
    ///
    /// Returns the generated UUIDv7 that identifies this document.
    pub fn add_document(&self, metadata: JsonValue, content: &[u8]) -> Result<Uuid> {
        let id = generate_v7();
        let id_str = id.to_string();

        self.meta.add_json_with_id(id, metadata.clone())?;
        self.blobs.add_blob_with_key(id, content)?;

        let content_text = String::from_utf8_lossy(content).into_owned();
        self.vectors.store_documents_batch(&[
            (&format!("{id_str}:meta"),     metadata.clone()),
            (&format!("{id_str}:content"),  serde_json::json!(content_text)),
        ])?;

        let ts = metadata.get("timestamp").and_then(|v| v.as_u64()).unwrap_or_else(now_secs);
        self.frequency.add_with_timestamp(ts, &id_str)?;

        Ok(id)
    }

    /// Store metadata and body to DuckDB and frequency tracking only; skip vector
    /// embedding.  Returns the generated UUIDv7.
    ///
    /// Use this inside tight loops (e.g., drain template collection) to avoid
    /// one ONNX call per document.  After the loop, call
    /// [`embed_documents_batch`] to embed all pending documents in one pass.
    ///
    /// [`embed_documents_batch`]: DocumentStorage::embed_documents_batch
    pub fn add_document_no_embed(&self, metadata: JsonValue, content: &[u8]) -> Result<Uuid> {
        let id = generate_v7();
        let id_str = id.to_string();

        self.meta.add_json_with_id(id, metadata.clone())?;
        self.blobs.add_blob_with_key(id, content)?;

        let ts = metadata.get("timestamp").and_then(|v| v.as_u64()).unwrap_or_else(now_secs);
        self.frequency.add_with_timestamp(ts, &id_str)?;

        Ok(id)
    }

    /// Batch-embed and index a set of documents previously stored via
    /// [`add_document_no_embed`].
    ///
    /// All 2 × N embeddings (one metadata fingerprint + one content text per
    /// entry) are computed in a single ONNX inference pass, then written to the
    /// vector store.  Silently no-ops when no embedding engine is configured.
    ///
    /// [`add_document_no_embed`]: DocumentStorage::add_document_no_embed
    pub fn embed_documents_batch(&self, entries: &[(Uuid, JsonValue, Vec<u8>)]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        // Build id key strings first so &str refs below live long enough.
        let keys: Vec<(String, String)> = entries
            .iter()
            .map(|(id, _, _)| {
                let s = id.to_string();
                (format!("{s}:meta"), format!("{s}:content"))
            })
            .collect();

        let mut store_entries: Vec<(&str, JsonValue)> = Vec::with_capacity(entries.len() * 2);
        for ((_, meta, content), (meta_key, content_key)) in entries.iter().zip(keys.iter()) {
            let content_text = String::from_utf8_lossy(content).into_owned();
            store_entries.push((meta_key.as_str(), meta.clone()));
            store_entries.push((content_key.as_str(), serde_json::json!(content_text)));
        }

        self.vectors.store_documents_batch(&store_entries)
    }

    /// Store a document with caller-supplied pre-computed vectors.
    ///
    /// This is the testable, embedding-free path. `meta_vec` is stored under
    /// `"{uuid}:meta"` with `metadata` as vecstore metadata; `content_vec` is
    /// stored under `"{uuid}:content"` with no extra metadata.
    ///
    /// Returns the generated UUIDv7.
    pub fn add_document_with_vectors(
        &self,
        metadata: JsonValue,
        content: &[u8],
        meta_vec: Vec<f32>,
        content_vec: Vec<f32>,
    ) -> Result<Uuid> {
        let id = generate_v7();
        let id_str = id.to_string();

        self.meta.add_json_with_id(id, metadata.clone())?;
        self.blobs.add_blob_with_key(id, content)?;
        self.vectors.store_vector(&format!("{id_str}:meta"), meta_vec, Some(metadata.clone()))?;
        self.vectors.store_vector(&format!("{id_str}:content"), content_vec, None)?;

        let ts = metadata.get("timestamp").and_then(|v| v.as_u64()).unwrap_or_else(now_secs);
        self.frequency.add_with_timestamp(ts, &id_str)?;

        Ok(id)
    }

    /// Replace the metadata for `id` and set its `updated_at` to now.
    ///
    /// Returns `Ok(())` even if `id` does not exist (no-op).
    /// The vector index is **not** updated; call [`store_metadata_vector`] or
    /// [`store_content_vector`] explicitly if needed.
    ///
    /// [`store_metadata_vector`]: DocumentStorage::store_metadata_vector
    /// [`store_content_vector`]: DocumentStorage::store_content_vector
    pub fn update_metadata(&self, id: Uuid, metadata: JsonValue) -> Result<()> {
        self.meta.update_json(id, metadata)
    }

    /// Replace the raw content for `id` and set its `updated_at` to now.
    ///
    /// Returns `Ok(())` even if `id` does not exist (no-op).
    pub fn update_content(&self, id: Uuid, content: &[u8]) -> Result<()> {
        self.blobs.update_blob(id, content)
    }

    /// Explicitly (re-)index the metadata vector for `id`.
    ///
    /// Stored under `"{id}:meta"` in the shared vector index.
    pub fn store_metadata_vector(
        &self,
        id: Uuid,
        meta_vec: Vec<f32>,
        metadata: JsonValue,
    ) -> Result<()> {
        self.vectors.store_vector(&format!("{id}:meta"), meta_vec, Some(metadata))
    }

    /// Explicitly (re-)index the content vector for `id`.
    ///
    /// Stored under `"{id}:content"` in the shared vector index.
    pub fn store_content_vector(&self, id: Uuid, content_vec: Vec<f32>) -> Result<()> {
        self.vectors.store_vector(&format!("{id}:content"), content_vec, None)
    }

    /// Remove the document from all stores (metadata, blob, vector index).
    ///
    /// Returns `Ok(())` for non-existent `id` (no-op in each sub-store).
    pub fn delete_document(&self, id: Uuid) -> Result<()> {
        let id_str = id.to_string();
        self.meta.drop_json(id)?;
        self.blobs.drop_blob(id)?;
        self.vectors.delete_vector(&format!("{id_str}:meta"))?;
        self.vectors.delete_vector(&format!("{id_str}:content"))?;
        self.frequency.delete(&id_str)?;
        Ok(())
    }

    // ── frequency tracking reads ──────────────────────────────────────────────

    /// All timestamps (Unix seconds, ascending) at which the document with
    /// `id` was added.
    pub fn frequencytracking_by_id(&self, id: &str) -> Result<Vec<u64>> {
        self.frequency.by_id(id)
    }

    /// Distinct document IDs that were added at the given exact Unix-seconds
    /// timestamp.
    pub fn frequencytracking_by_timestamp(&self, timestamp: u64) -> Result<Vec<String>> {
        self.frequency.by_timestamp(timestamp)
    }

    /// Distinct document IDs with at least one addition in the inclusive range
    /// `[start, end]` (both Unix seconds).
    pub fn frequencytracking_time_range(&self, start: u64, end: u64) -> Result<Vec<String>> {
        self.frequency.time_range(start, end)
    }

    /// Distinct document IDs added in the lookback window `[now − duration, now]`.
    ///
    /// `duration` is a human-readable string such as `"30s"`, `"5min"`, `"1h"`.
    pub fn frequencytracking_recent(&self, duration: &str) -> Result<Vec<String>> {
        self.frequency.recent(duration)
    }

    /// Record a frequency observation for `id` at the current wall-clock time.
    ///
    /// Unlike `add_document`, this does not create a new document — it only
    /// writes a `(now, id)` entry into the FrequencyTracking table so that
    /// `frequencytracking_recent` can find `id` in subsequent queries.
    pub fn frequencytracking_observe(&self, id: &str) -> Result<()> {
        self.frequency.add(id)
    }

    // ── reads ─────────────────────────────────────────────────────────────────

    /// Return the metadata stored under `id`, or `None` if no such document
    /// exists.
    pub fn get_metadata(&self, id: Uuid) -> Result<Option<JsonValue>> {
        self.meta.get_json(id)
    }

    /// Return every stored metadata document as `(uuid, metadata)` pairs.
    ///
    /// Used by callers that need a full directory of stored records without
    /// going through the vector index (e.g., the `ShardsManager` script
    /// registry).
    pub fn list_metadata(&self) -> Result<Vec<(Uuid, JsonValue)>> {
        self.meta.list_all()
    }

    /// Return the raw content stored under `id`, or `None` if no such document
    /// exists.
    pub fn get_content(&self, id: Uuid) -> Result<Option<Vec<u8>>> {
        self.blobs.get_blob(id)
    }

    // ── vector search ─────────────────────────────────────────────────────────

    /// Return the `limit` most relevant documents for a pre-computed query
    /// vector.
    ///
    /// Both `":meta"` and `":content"` entries are searched in the shared
    /// vector index. Hits are deduplicated by UUID (keeping the best score per
    /// document), sorted by descending similarity, and resolved to full
    /// documents by loading metadata from [`JsonStorage`] and content from
    /// [`BlobStorage`].
    ///
    /// Each returned element is a JSON object with four keys:
    /// - `"id"`: the document UUID string
    /// - `"metadata"`: the JSON metadata (or `null` if the document was deleted)
    /// - `"document"`: the content decoded as UTF-8 (invalid bytes replaced)
    /// - `"score"`: cosine similarity in `[0.0, 1.0]`
    pub fn search_document(&self, query_vec: Vec<f32>, limit: usize) -> Result<Vec<JsonValue>> {
        // Over-fetch so both :meta and :content slots compete fairly.
        let pool = limit.max(1) * 4;
        let candidates = self.vectors.search(query_vec, pool)?;
        self.build_results(candidates, limit)
    }

    /// Fingerprint `query` with [`json_fingerprint`], embed it, and return the
    /// `limit` most relevant documents.
    ///
    /// Returns `Err` if no embedding engine is present.
    ///
    /// [`json_fingerprint`]: crate::vectorengine::json_fingerprint
    pub fn search_document_json(
        &self,
        query: &JsonValue,
        limit: usize,
    ) -> Result<Vec<JsonValue>> {
        let pool = limit.max(1) * 4;
        let candidates = self.vectors.search_json(query, pool)?;
        self.build_results(candidates, limit)
    }

    /// Embed `query` as plain text and return the `limit` most relevant
    /// documents.
    ///
    /// Returns `Err` if no embedding engine is present.
    pub fn search_document_text(&self, query: &str, limit: usize) -> Result<Vec<JsonValue>> {
        self.search_document_json(&serde_json::json!(query), limit)
    }

    /// Like [`search_document`], but returns each result serialised to a
    /// [`json_fingerprint`] string instead of a raw `JsonValue`.
    ///
    /// Convenient for passing results directly to an embedding pipeline or
    /// full-text index without an extra mapping step.
    ///
    /// [`search_document`]: DocumentStorage::search_document
    /// [`json_fingerprint`]: crate::common::jsonfingerprint::json_fingerprint
    pub fn search_document_strings(
        &self,
        query_vec: Vec<f32>,
        limit: usize,
    ) -> Result<Vec<String>> {
        Ok(results_to_strings(&self.search_document(query_vec, limit)?))
    }

    /// Like [`search_document_json`], but returns fingerprint strings.
    ///
    /// Returns `Err` if no embedding engine is present.
    ///
    /// [`search_document_json`]: DocumentStorage::search_document_json
    pub fn search_document_json_strings(
        &self,
        query: &JsonValue,
        limit: usize,
    ) -> Result<Vec<String>> {
        Ok(results_to_strings(&self.search_document_json(query, limit)?))
    }

    /// Like [`search_document_text`], but returns fingerprint strings.
    ///
    /// Returns `Err` if no embedding engine is present.
    ///
    /// [`search_document_text`]: DocumentStorage::search_document_text
    pub fn search_document_text_strings(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<String>> {
        Ok(results_to_strings(&self.search_document_text(query, limit)?))
    }

    // ── persistence ───────────────────────────────────────────────────────────

    /// Load a text file, split it into overlapping chunks on sentence and
    /// paragraph boundaries, store each chunk as a blob, optionally embed it
    /// into the vector index, and write a metadata record that links all
    /// chunk IDs in document order.
    ///
    /// # Parameters
    ///
    /// | Name      | Type    | Meaning |
    /// |-----------|---------|---------|
    /// | `path`    | `&str`  | Filesystem path to the source file (must be valid UTF-8 text) |
    /// | `name`    | `&str`  | Human-readable document name stored in the metadata record |
    /// | `slice`   | `usize` | Maximum chunk size **in characters** |
    /// | `overlap` | `f32`   | Percentage of `slice` shared between adjacent chunks (`0.0` = none, `50.0` = half). Internally capped below `100.0`. |
    ///
    /// # Return value
    ///
    /// Returns the UUID of the **document-level metadata record**. That record
    /// contains `n_chunks` (total chunk count) and `chunks` (ordered list of
    /// per-chunk UUIDs), making it the entry-point for RAG retrieval.
    ///
    /// # Metadata layout
    ///
    /// **Document record** (stored under the returned UUID):
    /// ```json
    /// { "name": "report.txt", "path": "/data/report.txt",
    ///   "slice": 1000, "overlap": 20.0,
    ///   "n_chunks": 15, "chunks": ["<uuid-0>", …, "<uuid-14>"] }
    /// ```
    ///
    /// **Per-chunk record** (one per UUID listed in `"chunks"`):
    /// ```json
    /// { "document_name": "report.txt", "document_id": "<doc-uuid>",
    ///   "chunk_index": 0, "n_chunks": 15 }
    /// ```
    ///
    /// Each per-chunk blob is the raw UTF-8 text of that chunk, retrievable
    /// via [`get_content`].
    ///
    /// # Vector indexing
    ///
    /// For each chunk two entries are written to the shared HNSW index:
    /// - `"{chunk_uuid}:content"` — embedding of the chunk text
    /// - `"{chunk_uuid}:meta"` — embedding of the chunk metadata fingerprint
    ///
    /// The document-level metadata fingerprint is indexed as
    /// `"{doc_uuid}:meta"`. All three are best-effort: silently skipped when
    /// no [`EmbeddingEngine`] is attached (same policy as [`add_document`]).
    ///
    /// # Chunking algorithm
    ///
    /// 1. Split the text on paragraph boundaries (`\n\n`).
    /// 2. Within each paragraph, split on sentence boundaries (`.`, `!`, `?`
    ///    followed by whitespace, with abbreviation heuristics).
    /// 3. If a sentence still exceeds `slice`, split on word boundaries; if a
    ///    single word exceeds `slice`, hard-split at a UTF-8 character boundary.
    /// 4. Pack sentences greedily until adding the next sentence would exceed
    ///    `slice`, then emit the chunk and slide the window back by `overlap%`
    ///    of `slice` to create the overlap region of the next chunk.
    ///
    /// [`get_content`]: DocumentStorage::get_content
    /// [`add_document`]: DocumentStorage::add_document
    pub fn add_document_from_file(
        &self,
        path: &str,
        name: &str,
        slice: usize,
        overlap: f32,
    ) -> Result<Uuid> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| err_msg(format!("cannot read {path:?}: {e}")))?;

        let slice       = slice.max(1);
        let overlap_pct = overlap.clamp(0.0, 99.0);
        let overlap_ch  = ((slice as f32) * (overlap_pct / 100.0)).round() as usize;

        let chunks  = split_into_chunks(&text, slice, overlap_ch);
        let n_chunks = chunks.len();

        // Generate the document UUID upfront so each chunk can reference it.
        let meta_id     = generate_v7();
        let meta_id_str = meta_id.to_string();

        // ── store each chunk ──────────────────────────────────────────────────

        let mut chunk_ids: Vec<String> = Vec::with_capacity(n_chunks);
        for (i, chunk_text) in chunks.iter().enumerate() {
            let chunk_id     = generate_v7();
            let chunk_id_str = chunk_id.to_string();

            let chunk_meta = serde_json::json!({
                "document_name": name,
                "document_id":   meta_id_str,
                "chunk_index":   i,
                "n_chunks":      n_chunks,
            });

            // Blob: raw chunk bytes.
            self.blobs.add_blob_with_key(chunk_id, chunk_text.as_bytes())?;

            // Metadata: per-chunk provenance.
            self.meta.add_json_with_id(chunk_id, chunk_meta.clone())?;

            self.vectors.store_document(
                &format!("{chunk_id_str}:content"),
                serde_json::json!(chunk_text),
            )?;
            self.vectors.store_document(
                &format!("{chunk_id_str}:meta"),
                chunk_meta,
            )?;

            chunk_ids.push(chunk_id_str);
        }

        // ── store document-level metadata ─────────────────────────────────────

        let doc_meta = serde_json::json!({
            "name":     name,
            "path":     path,
            "slice":    slice,
            "overlap":  overlap,
            "n_chunks": n_chunks,
            "chunks":   chunk_ids,
        });

        self.meta.add_json_with_id(meta_id, doc_meta.clone())?;
        self.vectors.store_document(
            &format!("{meta_id_str}:meta"),
            doc_meta,
        )?;

        Ok(meta_id)
    }

    /// Flush the vector index to disk.
    ///
    /// The DuckDB-backed stores (`metadata.db`, `blobs.db`) checkpoint
    /// automatically; calling this is only necessary for the vecstore index.
    pub fn sync(&self) -> Result<()> {
        self.vectors.sync()
    }

    /// Return all `(id, metadata)` pairs stored in this DocumentStorage.
    ///
    /// Results are returned in undefined order.  Useful for listing contents
    /// or building external indexes.
    pub fn list_all(&self) -> Result<Vec<(Uuid, JsonValue)>> {
        self.meta.list_all()
    }

    /// Re-embed and re-index the stored metadata and content for a single
    /// document, updating both the `":meta"` and `":content"` HNSW slots.
    ///
    /// Call this after [`update_metadata`](Self::update_metadata) or
    /// [`update_content`](Self::update_content) to keep the vector index in
    /// sync.  Silently succeeds if no embedding engine is present.
    pub fn reembed_document(&self, id: Uuid) -> Result<()> {
        let id_str = id.to_string();
        if let Some(metadata) = self.meta.get_json(id)? {
            self.vectors.store_document(&format!("{id_str}:meta"), metadata)?;
        }
        if let Some(bytes) = self.blobs.get_blob(id)? {
            let text = String::from_utf8_lossy(&bytes).into_owned();
            self.vectors.store_document(&format!("{id_str}:content"), serde_json::json!(text))?;
        }
        Ok(())
    }

    /// Rebuild the vector index from the DuckDB metadata and blob stores.
    ///
    /// Iterates every record in `metadata.db`, embeds the JSON metadata
    /// fingerprint as `"{id}:meta"` and the blob content as `"{id}:content"`,
    /// then saves the index to disk.  Existing vector entries for a UUID are
    /// overwritten (upsert semantics in vecstore).
    ///
    /// Returns the number of documents re-indexed.  Requires an embedding
    /// engine; returns `Err` if none was configured.
    pub fn reindex(&self) -> Result<usize> {
        let all = self.meta.list_all()?;
        let count = all.len();
        for (id, metadata) in all {
            let id_str = id.to_string();
            self.vectors.store_document(&format!("{id_str}:meta"), metadata)?;
            if let Some(bytes) = self.blobs.get_blob(id)? {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                self.vectors.store_document(&format!("{id_str}:content"), serde_json::json!(text))?;
            }
        }
        self.vectors.sync()?;
        Ok(count)
    }
}

// ── internals ─────────────────────────────────────────────────────────────────

impl DocumentStorage {
    fn build_results(
        &self,
        candidates: Vec<crate::vectorengine::SearchResult>,
        limit: usize,
    ) -> Result<Vec<JsonValue>> {
        // Deduplicate by UUID; keep the highest score per document.
        let mut best: HashMap<String, f32> = HashMap::new();
        for r in &candidates {
            let uuid_str = strip_suffix(&r.id).to_string();
            let entry = best.entry(uuid_str).or_insert(f32::NEG_INFINITY);
            if r.score > *entry {
                *entry = r.score;
            }
        }

        // Sort by descending score, then truncate to the requested limit.
        let mut ranked: Vec<(String, f32)> = best.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        ranked.truncate(limit);

        // Load metadata + content for each UUID and assemble result objects.
        let mut out = Vec::with_capacity(ranked.len());
        for (uuid_str, score) in ranked {
            let uuid = Uuid::parse_str(&uuid_str)
                .map_err(|e| err_msg(format!("invalid UUID in vector index: {e}")))?;
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

/// Convert a slice of search-result JSON objects (as returned by
/// [`DocumentStorage::search_document`] and its variants) into a `Vec<String>`
/// by applying [`json_fingerprint`] to each element.
///
/// Useful for feeding results directly into an embedding pipeline or a
/// full-text index without an extra mapping step.
///
/// # Example
///
/// ```rust,no_run
/// # use bdslib::documentstorage::{DocumentStorage, results_to_strings};
/// # use tempfile::TempDir;
/// # use serde_json::json;
/// let dir = TempDir::new().unwrap();
/// let store = DocumentStorage::new(dir.path().to_str().unwrap()).unwrap();
/// let results = store.search_document(vec![1.0, 0.0, 0.0], 5).unwrap();
/// let strings = results_to_strings(&results);
/// ```
pub fn results_to_strings(results: &[JsonValue]) -> Vec<String> {
    results.iter().map(|r| json_fingerprint(r)).collect()
}

fn strip_suffix(id: &str) -> &str {
    id.strip_suffix(":meta")
        .or_else(|| id.strip_suffix(":content"))
        .unwrap_or(id)
}

// ── text chunking ─────────────────────────────────────────────────────────────

/// Split `text` into overlapping chunks that respect sentence / paragraph
/// boundaries and do not exceed `max_chars` characters each.
///
/// Each chunk beyond the first begins with the tail atoms of the previous
/// chunk whose total length is ≤ `overlap_chars` (the "overlap window").
/// Because overlap is carried at atom (sentence) granularity, the actual
/// overlap may be less than requested when sentences are long.  The first new
/// atom after the overlap window is always added to prevent stalling.
fn split_into_chunks(text: &str, max_chars: usize, overlap_chars: usize) -> Vec<String> {
    // Never let overlap consume the entire chunk — that would prevent progress.
    let overlap_chars = overlap_chars.min(max_chars.saturating_sub(1));

    let atoms = build_atoms(text, max_chars);
    if atoms.is_empty() {
        return Vec::new();
    }

    let mut chunks:      Vec<String> = Vec::new();
    let mut win_start:   usize = 0; // first atom in the current window (inclusive)
    let mut win_end:     usize = 0; // first atom NOT in the current window
    let mut win_len:     usize = 0; // char count of atoms[win_start..win_end]
    // After a slide, atoms[overlap_end..] are "new" (not from the overlap
    // region).  The first new atom is always added regardless of win_len so
    // that the window always advances.
    let mut overlap_end: usize = 0;

    loop {
        // Extend the window forward: enforce the size limit only after at
        // least one new (non-overlap) atom has been included.
        while win_end < atoms.len() {
            let atom_len = atoms[win_end].len();
            if win_end > overlap_end && win_len + atom_len > max_chars {
                break;
            }
            win_len += atom_len;
            win_end += 1;
        }

        chunks.push(atoms[win_start..win_end].concat());

        if win_end >= atoms.len() {
            break;
        }

        // Slide: walk backward from win_end-1, keeping atoms whose cumulative
        // length fits within overlap_chars.
        let mut new_start = win_end; // default: no overlap
        let mut kept_len  = 0usize;
        for i in (win_start + 1..win_end).rev() {
            let a_len = atoms[i].len();
            if kept_len + a_len <= overlap_chars {
                kept_len  += a_len;
                new_start  = i;
            } else {
                break;
            }
        }

        overlap_end = win_end; // atoms[overlap_end..] are new in the next chunk
        win_start   = new_start;
        win_len     = kept_len;
    }

    chunks
}

/// Build a flat list of atomic text segments, each guaranteed to be
/// `≤ max_chars` characters, by splitting on paragraph → sentence → word
/// boundaries in that priority order.
fn build_atoms(text: &str, max_chars: usize) -> Vec<String> {
    let mut atoms: Vec<String> = Vec::new();

    for para in text.split("\n\n") {
        let para = para.trim_end();
        if para.trim().is_empty() {
            continue;
        }
        for sent in sentence_split(para) {
            if sent.len() <= max_chars {
                atoms.push(sent);
            } else {
                atoms.extend(word_split(&sent, max_chars));
            }
        }
    }

    // Fallback: the text had no paragraph separators.
    if atoms.is_empty() && !text.trim().is_empty() {
        atoms.extend(word_split(text, max_chars));
    }

    atoms
}

/// Split a single paragraph into sentences.
///
/// A sentence boundary is detected when `.`, `!`, or `?` is followed by
/// whitespace (or end of string) and the first non-whitespace character that
/// follows is uppercase or non-alphabetic — a simple heuristic that avoids
/// splitting on most common abbreviations ("Mr.", "e.g.", etc.).
///
/// Single newlines inside the paragraph are also treated as soft boundaries.
/// Trailing whitespace is included in the segment so that concatenation
/// preserves the original spacing.
fn sentence_split(text: &str) -> Vec<String> {
    let mut out:       Vec<String> = Vec::new();
    let chars:         Vec<char>   = text.chars().collect();
    let len                        = chars.len();
    let mut seg_start: usize       = 0;
    let mut i:         usize       = 0;

    while i < len {
        let c = chars[i];

        if matches!(c, '.' | '!' | '?') {
            let next  = chars.get(i + 1).copied().unwrap_or('\0');
            let is_boundary = if next == '\0' {
                true
            } else if next.is_whitespace() {
                // Only a real sentence end when what follows (after the space)
                // starts with an uppercase letter or is not alphabetic.
                let after = chars[i + 1..]
                    .iter()
                    .find(|c| !c.is_whitespace())
                    .copied()
                    .unwrap_or('\0');
                after == '\0' || after.is_uppercase() || !after.is_alphabetic()
            } else {
                false
            };

            if is_boundary {
                // Consume the punctuation and any following horizontal whitespace.
                i += 1;
                while i < len && chars[i].is_whitespace() && chars[i] != '\n' {
                    i += 1;
                }
                let seg: String = chars[seg_start..i].iter().collect();
                let seg = seg.trim_start().to_string();
                if !seg.trim().is_empty() {
                    out.push(seg);
                }
                seg_start = i;
                continue;
            }
        }

        // Single newline → soft boundary.
        if c == '\n' {
            i += 1;
            let seg: String = chars[seg_start..i].iter().collect();
            let seg = seg.trim_start().to_string();
            if !seg.trim().is_empty() {
                out.push(seg);
            }
            seg_start = i;
            continue;
        }

        i += 1;
    }

    // Remainder (no terminating punctuation).
    if seg_start < len {
        let seg: String = chars[seg_start..].iter().collect();
        let seg = seg.trim_start().to_string();
        if !seg.trim().is_empty() {
            out.push(seg);
        }
    }

    out
}

/// Split `text` into word-boundary chunks of at most `max_chars` characters.
///
/// If a single word exceeds `max_chars`, it is hard-split at a UTF-8
/// character boundary.
fn word_split(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks:  Vec<String> = Vec::new();
    let mut current: String      = String::new();

    for word in text.split_whitespace() {
        let need = if current.is_empty() { word.len() } else { word.len() + 1 };

        if !current.is_empty() && current.len() + need > max_chars {
            chunks.push(std::mem::take(&mut current));
        }

        if word.len() > max_chars {
            // Hard-split at char boundary.
            if !current.is_empty() {
                chunks.push(std::mem::take(&mut current));
            }
            let mut remaining = word;
            while !remaining.is_empty() {
                let end = remaining
                    .char_indices()
                    .take_while(|(i, _)| *i < max_chars)
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(remaining.len());
                chunks.push(remaining[..end].to_string());
                remaining = &remaining[end..];
            }
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

fn meta_config() -> JsonStorageConfig {
    JsonStorageConfig {
        key_field:   None,
        default_key: "doc".to_string(),
    }
}

struct Paths {
    metadata_db:  String,
    blobs_db:     String,
    vec:          String,
    frequency_db: String,
}

impl Paths {
    fn from(root: &str) -> Result<Self> {
        let root = Path::new(root);
        std::fs::create_dir_all(root)
            .map_err(|e| err_msg(format!("cannot create root dir {root:?}: {e}")))?;
        std::fs::create_dir_all(root.join("vectors"))
            .map_err(|e| err_msg(format!("cannot create vectors dir: {e}")))?;
        Ok(Self {
            metadata_db:  root.join("metadata.db").to_string_lossy().into_owned(),
            blobs_db:     root.join("blobs.db").to_string_lossy().into_owned(),
            vec:          root.join("vectors").to_string_lossy().into_owned(),
            frequency_db: root.join("frequency.db").to_string_lossy().into_owned(),
        })
    }
}
