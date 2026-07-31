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

use std::collections::HashSet;

use crate::storage::edge_store::{Edge, EdgeKind, EdgeOrigin, EdgeStore, EndpointRef};
use crate::storage::engine::{BlobStorage, JsonStorage};
use crate::storage::embedding::EmbeddingEngine;
use crate::storage::vector::{SearchResult, VectorEngine};

/// Current on-disk metadata schema version (M9). Bump when the node
/// JSON shape changes incompatibly, and add a migration in
/// `JsonStorage::ensure_schema_version`. v1 = the 1.3.37 baseline
/// (every pre-existing project is stamped v1 on first open).
const CURRENT_SCHEMA_VERSION: i64 = 1;

/// One document = one JSON metadata row + one blob + two HNSW
/// entries. Cloneable; every internal store is `Arc`-backed.
#[derive(Clone)]
pub struct DocumentStorage {
    meta:    JsonStorage,
    blobs:   BlobStorage,
    vectors: VectorEngine,
    /// SEMNET-P0 — the typed-edge layer (`edges.db`). A separate DuckDB store
    /// beside metadata/blobs/vectors; no cross-store transaction (same doctrine
    /// as the vector index — durable edges are source-of-truth, derived ones a
    /// rebuildable cache).
    edges:   EdgeStore,
}

impl DocumentStorage {
    /// Open or create the document store rooted at `root`.
    ///
    /// Inkhaven's call site (`Store::open`) always supplies an
    /// embedding engine; the bdslib-style no-embedding constructor
    /// is gone.
    pub fn with_embedding(root: &str, engine: EmbeddingEngine, pool_size: usize) -> Result<Self> {
        let paths = Paths::from(root)?;
        let pool = pool_size as u32;
        let meta = JsonStorage::new(&paths.metadata_db, pool, "doc")?;
        // M9 — verify (or stamp) the metadata schema version. A store
        // written by a newer inkhaven fails loudly here instead of
        // silently dropping nodes whose JSON shape this binary can't
        // parse.
        meta.ensure_schema_version(CURRENT_SCHEMA_VERSION)?;
        Ok(Self {
            meta,
            blobs:   BlobStorage::new(&paths.blobs_db, pool)?,
            vectors: VectorEngine::with_embedding(&paths.vec, engine)?,
            edges:   EdgeStore::new(&paths.edges_db, pool)?,
        })
    }
}

/// SEMNET-P0 graph pass-throughs. Several are consumed by the Store graph API /
/// P1+ migrations ahead of a P0 caller; the block-level allow holds the
/// warning-free bar without peppering per-method attributes. Remove once P1
/// wires the traversal surface.
#[allow(dead_code)]
impl DocumentStorage {
    // ── graph / edges (SEMNET-P0) ──────────────────────────────────

    /// Insert one edge. Durable immediately (DuckDB autocommit).
    pub fn add_edge(&self, edge: &Edge) -> Result<()> {
        self.edges.insert(edge)
    }

    /// Insert many edges atomically (all or none).
    pub fn add_edges(&self, edges: &[Edge]) -> Result<()> {
        self.edges.insert_batch(edges)
    }

    pub fn edge(&self, id: Uuid) -> Result<Option<Edge>> {
        self.edges.by_id(id)
    }

    /// Edges leaving a node, filtered to `kinds` (empty = any).
    pub fn edges_out(&self, node: Uuid, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        self.edges.outgoing(&EndpointRef::Node(node), kinds)
    }

    /// Edges arriving at a node — the reverse-index query.
    pub fn edges_in(&self, node: Uuid, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        self.edges.incoming(&EndpointRef::Node(node), kinds)
    }

    /// Every edge touching a node on either side (deduped).
    pub fn edges_around(&self, node: Uuid, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        self.edges.neighbors(&EndpointRef::Node(node), kinds)
    }

    pub fn delete_edge(&self, id: Uuid) -> Result<()> {
        self.edges.delete(id)
    }

    /// Cascade-GC every edge touching any of `nodes` (either endpoint). Returns
    /// the count removed. Called from `Store::delete_subtree`.
    pub fn gc_edges_for_nodes(&self, nodes: &HashSet<Uuid>) -> Result<usize> {
        self.edges.delete_nodes(nodes)
    }

    /// Drop the rebuildable-cache edges of one origin (`graph rebuild`).
    pub fn delete_edges_by_origin(&self, origin: EdgeOrigin) -> Result<usize> {
        self.edges.delete_by_origin(origin)
    }

    pub fn edge_count(&self) -> Result<usize> {
        self.edges.count()
    }

    pub fn edges_by_kind(&self) -> Result<Vec<(String, usize)>> {
        self.edges.count_by_kind()
    }

    pub fn all_edges(&self) -> Result<Vec<Edge>> {
        self.edges.all()
    }

    /// Edge-store integrity (kept separate from the meta/blob tuple so existing
    /// callers of `integrity_check` don't change shape).
    pub fn edges_integrity_check(&self) -> Result<String> {
        self.edges.integrity_check()
    }
}

impl DocumentStorage {
    // ── writes ─────────────────────────────────────────────────────

    /// Generate a fresh UUIDv7, persist metadata + content + two
    /// vector entries, and return the new id.
    pub fn add_document(&self, metadata: JsonValue, content: &[u8]) -> Result<Uuid> {
        let id = Uuid::now_v7();
        let id_str = id.to_string();

        // H2 — meta, blob, and the HNSW index are three separate stores
        // with no shared transaction (a cross-DB transaction isn't
        // possible). Order the writes so a crash between steps leaves
        // the least-harmful state: blob first, then metadata, so a
        // partial write is an orphan blob (invisible, swept by
        // `inkhaven reindex`) rather than a metadata row pointing at
        // missing content (a node that fails when opened). The vector
        // slots are the rebuildable cache, written last.
        self.blobs.add_blob_with_key(id, content)?;
        self.meta.add_json_with_id(id, metadata.clone())?;

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
        // H2 — blob before metadata (see `add_document`).
        self.blobs.add_blob_with_key(id, content)?;
        self.meta.add_json_with_id(id, metadata)?;
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
        // H2 — drop the vector slots FIRST so a crash mid-delete can't
        // leave stale entries that surface deleted content in search;
        // then metadata (the node vanishes), then the blob last (a
        // partial delete leaves only an orphan blob, swept by reindex).
        self.vectors.delete_vector(&format!("{id_str}:meta"))?;
        self.vectors.delete_vector(&format!("{id_str}:content"))?;
        self.meta.drop_json(id)?;
        self.blobs.drop_blob(id)?;
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

    /// Embed arbitrary texts on demand via the loaded engine (conlang
    /// near-synonym detection); not backed by the stored index.
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.vectors.embed_batch(texts)
    }

    /// Whether the embedding model is already warm (HAIKU-2 warmth gate).
    pub fn embedding_is_loaded(&self) -> bool {
        self.vectors.embedding_is_loaded()
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

    /// Flush the vector index to disk. Called on every save and by
    /// the background tick. Cheap when the index is clean (dirty
    /// flag short-circuit inside `VectorEngine::sync`).
    pub fn sync(&self) -> Result<()> {
        self.vectors.sync()
    }

    /// 1.8.32+ hardening — flush the vector index off the calling thread. Use
    /// this on the render/UI thread's routine save paths so serializing the HNSW
    /// index to disk doesn't freeze the editor; the underlying content is already
    /// durable, and the index is a rebuildable derived artifact. See
    /// [`crate::storage::vector::VectorEngine::sync_in_background`].
    pub fn sync_in_background(&self) {
        self.vectors.sync_in_background()
    }

    /// Issue a DuckDB `CHECKPOINT` against both sub-stores
    /// (`metadata.db` + `blobs.db`). Drains WAL into the main `.db`
    /// files; cheap when there's nothing to drain (DuckDB short-
    /// circuits). Called from the background sync tick and the TUI
    /// shutdown path — not on every save, since per-commit fsync
    /// already makes writes durable.
    pub fn checkpoint(&self) -> Result<()> {
        self.meta.checkpoint()?;
        self.blobs.checkpoint()?;
        self.edges.checkpoint()?;
        Ok(())
    }

    /// 1.2.16+ Phase P.4 — DuckDB integrity check
    /// across both sub-stores.  Errors on the
    /// first non-`"ok"` result; otherwise returns
    /// `Ok(())`.  The `which` argument is the
    /// sub-store identifier exposed in the error
    /// so callers can name the failing layer.
    pub fn integrity_check(&self) -> Result<(String, String)> {
        let meta_ok = self.meta.integrity_check()?;
        let blobs_ok = self.blobs.integrity_check()?;
        Ok((meta_ok, blobs_ok))
    }

    /// 1.2.16+ Phase P.4 — total paragraph row
    /// count.  Just `list_metadata().len()`;
    /// shortcut for the vector-parity check.
    pub fn row_count(&self) -> Result<usize> {
        Ok(self.meta.list_all()?.len())
    }

    /// 1.2.16+ Phase P.4 — total HNSW vector
    /// count.  Note: the store holds two vectors
    /// per document (`:meta` + `:content`); the
    /// parity check divides by 2.
    pub fn vector_count(&self) -> Result<usize> {
        self.vectors.count()
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
    edges_db:    String,
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
            edges_db:    root.join("edges.db").to_string_lossy().into_owned(),
        })
    }
}
