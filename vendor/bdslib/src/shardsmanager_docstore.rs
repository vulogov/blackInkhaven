//! `DocumentStorage` helpers on [`ShardsManager`].
//!
//! Every method here is a thin delegation to the embedded `DocumentStorage`
//! instance that lives at `{dbpath}/docstore`.  The `doc_` prefix keeps the
//! document-store API distinct from the shard-oriented API defined in
//! `shardsmanager.rs`.
//!
//! The document store shares the same [`EmbeddingEngine`] as the shard cache,
//! so all embedding-based methods (`doc_search_text`, `doc_search_json`, …)
//! are available without any extra setup.

use crate::common::error::Result;
use crate::shardsmanager::ShardsManager;
use serde_json::Value as JsonValue;
use uuid::Uuid;

impl ShardsManager {
    // ── writes ────────────────────────────────────────────────────────────────

    /// Store a document with JSON `metadata` and raw byte `content`.
    ///
    /// Both the metadata fingerprint and the content text are embedded and
    /// indexed automatically (the shared [`EmbeddingEngine`] is always present).
    /// Returns the UUIDv7 assigned to the stored document.
    pub fn doc_add(&self, metadata: JsonValue, content: &[u8]) -> Result<Uuid> {
        self.docstore.add_document(metadata, content)
    }

    /// Store a document with caller-supplied pre-computed vectors.
    ///
    /// Use this when vectors are generated externally (GPU pipelines, batch
    /// jobs).  No embedding engine call is made; all four stores are written.
    pub fn doc_add_with_vectors(
        &self,
        metadata: JsonValue,
        content: &[u8],
        meta_vec: Vec<f32>,
        content_vec: Vec<f32>,
    ) -> Result<Uuid> {
        self.docstore
            .add_document_with_vectors(metadata, content, meta_vec, content_vec)
    }

    /// Load a text file, split it into overlapping chunks on sentence/paragraph
    /// boundaries, and store every chunk as an independently searchable record.
    ///
    /// Returns the UUIDv7 of the document-level metadata record.  Each chunk
    /// gets its own UUID, blob entry, metadata entry, and HNSW vector pair
    /// (`":content"` + `":meta"`).  The document-level record holds the ordered
    /// chunk UUID list under `"chunks"`.
    ///
    /// # Parameters
    /// - `path` — filesystem path to read
    /// - `name` — human-readable document name stored in all metadata records
    /// - `slice` — maximum character count per chunk (clamped to `≥ 1`)
    /// - `overlap` — overlap as a percentage of `slice` (`[0.0, 99.0]`)
    pub fn doc_add_from_file(
        &self,
        path: &str,
        name: &str,
        slice: usize,
        overlap: f32,
    ) -> Result<Uuid> {
        self.docstore
            .add_document_from_file(path, name, slice, overlap)
    }

    /// Replace the metadata for `id` in-place.
    ///
    /// The vector index is **not** updated automatically.  Call
    /// [`doc_store_metadata_vector`](Self::doc_store_metadata_vector) afterwards
    /// if the new metadata should be re-embedded.
    pub fn doc_update_metadata(&self, id: Uuid, metadata: JsonValue) -> Result<()> {
        self.docstore.update_metadata(id, metadata)
    }

    /// Replace the raw content bytes for `id` in-place.
    pub fn doc_update_content(&self, id: Uuid, content: &[u8]) -> Result<()> {
        self.docstore.update_content(id, content)
    }

    /// Remove a document from all three sub-stores (metadata, blob, vector index).
    ///
    /// Both `"{id}:meta"` and `"{id}:content"` HNSW entries are deleted.
    /// Returns `Ok(())` for unknown UUIDs (no-op in each sub-store).
    pub fn doc_delete(&self, id: Uuid) -> Result<()> {
        self.docstore.delete_document(id)
    }

    /// Explicitly (re-)index the `":meta"` vector slot for `id`.
    ///
    /// Use after [`doc_update_metadata`](Self::doc_update_metadata) to keep
    /// the HNSW index in sync with the updated metadata.
    pub fn doc_store_metadata_vector(
        &self,
        id: Uuid,
        meta_vec: Vec<f32>,
        metadata: JsonValue,
    ) -> Result<()> {
        self.docstore.store_metadata_vector(id, meta_vec, metadata)
    }

    /// Explicitly (re-)index the `":content"` vector slot for `id`.
    ///
    /// Use after [`doc_update_content`](Self::doc_update_content) to keep
    /// the HNSW index in sync with the updated content.
    pub fn doc_store_content_vector(&self, id: Uuid, content_vec: Vec<f32>) -> Result<()> {
        self.docstore.store_content_vector(id, content_vec)
    }

    /// Flush the document store's HNSW vector index to disk.
    ///
    /// The DuckDB-backed stores (`metadata.db`, `blobs.db`) checkpoint
    /// automatically; this call is only necessary for the vecstore index.
    pub fn doc_sync(&self) -> Result<()> {
        self.docstore.sync()
    }

    /// Rebuild the HNSW vector index from the persisted metadata and blob stores.
    ///
    /// Useful after a crash or an unclean shutdown where DuckDB survived but the
    /// vecstore index was not flushed.  Returns the number of documents indexed.
    pub fn doc_reindex(&self) -> Result<usize> {
        self.docstore.reindex()
    }

    // ── reads ─────────────────────────────────────────────────────────────────

    /// Return the JSON metadata for `id`, or `None` if no such document exists.
    pub fn doc_get_metadata(&self, id: Uuid) -> Result<Option<JsonValue>> {
        self.docstore.get_metadata(id)
    }

    /// Return the raw content bytes for `id`, or `None` if no such document exists.
    pub fn doc_get_content(&self, id: Uuid) -> Result<Option<Vec<u8>>> {
        self.docstore.get_content(id)
    }

    // ── search ────────────────────────────────────────────────────────────────

    /// Search the document store with a pre-computed query vector.
    ///
    /// Searches the shared HNSW index (both `":meta"` and `":content"` slots),
    /// deduplicates hits by UUID, and returns the `limit` most relevant
    /// documents sorted by cosine similarity descending.
    ///
    /// Each result is a JSON object with keys `"id"`, `"metadata"`,
    /// `"document"` (UTF-8 decoded content), and `"score"`.
    pub fn doc_search(&self, query_vec: Vec<f32>, limit: usize) -> Result<Vec<JsonValue>> {
        self.docstore.search_document(query_vec, limit)
    }

    /// Search the document store by embedding a JSON query object.
    ///
    /// The query is converted to a `"path: value"` string via `json_fingerprint`
    /// before embedding, so field names contribute to the semantic signal.
    pub fn doc_search_json(&self, query: &JsonValue, limit: usize) -> Result<Vec<JsonValue>> {
        self.docstore.search_document_json(query, limit)
    }

    /// Search the document store by embedding a plain-text query string.
    pub fn doc_search_text(&self, query: &str, limit: usize) -> Result<Vec<JsonValue>> {
        self.docstore.search_document_text(query, limit)
    }

    /// Like [`doc_search`](Self::doc_search), but returns each result
    /// serialised to a `json_fingerprint` string.
    pub fn doc_search_strings(&self, query_vec: Vec<f32>, limit: usize) -> Result<Vec<String>> {
        self.docstore.search_document_strings(query_vec, limit)
    }

    /// Like [`doc_search_json`](Self::doc_search_json), but returns each
    /// result serialised to a `json_fingerprint` string.
    pub fn doc_search_json_strings(&self, query: &JsonValue, limit: usize) -> Result<Vec<String>> {
        self.docstore.search_document_json_strings(query, limit)
    }

    /// Like [`doc_search_text`](Self::doc_search_text), but returns each
    /// result serialised to a `json_fingerprint` string.
    pub fn doc_search_text_strings(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        self.docstore.search_document_text_strings(query, limit)
    }
}
