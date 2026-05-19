use crate::common::error::{err_msg, Result};
use crate::documentstorage::DocumentStorage;
use crate::fts::FTSEngine;
use crate::observability::{ObservabilityStorage, ObservabilityStorageConfig};
use crate::vectorengine::{json_fingerprint, SearchResult, VectorEngine};
use crate::EmbeddingEngine;
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use vecstore::reranking::MMRReranker;

/// Combined observability, full-text search, and vector shard.
///
/// `Shard` stores every telemetry event in [`ObservabilityStorage`] and
/// maintains two search indexes that cover **primary records only**:
///
/// | Index | Engine | Scope |
/// |---|---|---|
/// | Full-text | [`FTSEngine`] | Primary records only |
/// | Vector | [`VectorEngine`] | Primary records only |
///
/// Secondary records are stored in `ObservabilityStorage` but are **not**
/// added to the FTS or vector indexes. They are returned as embedded
/// `"secondaries"` arrays inside their parent primary in search results.
///
/// All three indexes share the same UUID namespace: the UUIDv7 returned by
/// [`add`] is the identifier used in all stores.
///
/// `Shard` is `Clone`; all clones share the same underlying connection pool,
/// FTS writer, and vector index.
///
/// [`add`]: Shard::add
#[derive(Clone)]
pub struct Shard {
    observability: ObservabilityStorage,
    fts: Arc<FTSEngine>,
    vector: VectorEngine,
    /// Template store for this shard's time interval; lives at `{path}/tplstorage`.
    pub(crate) tplstorage: DocumentStorage,
}

impl Shard {
    /// Open or create a shard rooted at `path` with default config.
    ///
    /// Three sub-paths are created automatically:
    ///
    /// | Sub-path | Engine |
    /// |---|---|
    /// | `{path}/obs.db` | ObservabilityStorage (DuckDB) |
    /// | `{path}/fts/` | FTSEngine (Tantivy) |
    /// | `{path}/vec/` | VectorEngine (HNSW) |
    ///
    /// `pool_size` is forwarded to `ObservabilityStorage`.
    pub fn new(path: &str, pool_size: u32, embedding: EmbeddingEngine) -> Result<Self> {
        Self::with_config(path, pool_size, embedding, ObservabilityStorageConfig::default())
    }

    /// Open or create a shard at `path` with a custom `ObservabilityStorageConfig`.
    pub fn with_config(
        path: &str,
        pool_size: u32,
        embedding: EmbeddingEngine,
        config: ObservabilityStorageConfig,
    ) -> Result<Self> {
        std::fs::create_dir_all(path)
            .map_err(|e| err_msg(format!("cannot create shard directory '{path}': {e}")))?;

        let obs_path = format!("{path}/obs.db");
        let fts_path = format!("{path}/fts");
        let vec_path = format!("{path}/vec");
        let tpl_path = format!("{path}/tplstorage");

        let observability =
            ObservabilityStorage::with_config(&obs_path, pool_size, embedding.clone(), config)?;
        let fts = FTSEngine::new(&fts_path)?;
        let vector = VectorEngine::with_embedding(&vec_path, embedding.clone())?;
        let tplstorage = DocumentStorage::with_embedding(&tpl_path, embedding)?;

        Ok(Self {
            observability,
            fts: Arc::new(fts),
            vector,
            tplstorage,
        })
    }

    // ── writes ────────────────────────────────────────────────────────────────

    /// Store a telemetry event and, if the record is classified as primary,
    /// index it in the FTS and vector engines.
    ///
    /// The document must satisfy `ObservabilityStorage::add` requirements
    /// (`timestamp`, `key`, `data` mandatory fields). The JSON fingerprint of
    /// the full document is used as the FTS body and as the embedding input for
    /// the vector index.
    ///
    /// Secondary records are stored in `ObservabilityStorage` only; they are
    /// not added to the FTS or vector indexes. They are accessible via their
    /// parent primary through [`search_fts`] and [`search_vector`] results.
    ///
    /// Duplicate `(key, data)` pairs return the existing record's UUID and
    /// update the deduplication log without touching the search indexes.
    ///
    /// [`search_fts`]: Shard::search_fts
    /// [`search_vector`]: Shard::search_vector
    pub fn add(&self, doc: JsonValue) -> Result<Uuid> {
        let fingerprint = json_fingerprint(&doc);
        let (id, is_primary, opt_emb) = self.observability.add(doc.clone())?;
        if is_primary {
            self.fts.add_document_with_id(id, &fingerprint)?;
            // Reuse the embedding already computed by observability — no second embed.
            self.vector
                .store_vector(&id.to_string(), opt_emb.unwrap(), Some(doc))?;
        }
        Ok(id)
    }

    /// Store a batch of telemetry events with a single embedding pass, a single
    /// FTS commit, and a single DuckDB write transaction for all primaries.
    ///
    /// Documents classified as duplicates or secondaries are written to
    /// `ObservabilityStorage` but are not staged for FTS/vector indexing.
    /// Returns UUIDs in the same order as the input documents.
    pub fn add_batch(&self, docs: Vec<JsonValue>) -> Result<Vec<Uuid>> {
        let fingerprints: Vec<String> = docs.iter().map(|d| json_fingerprint(d)).collect();

        // observability.add_batch handles batch embedding + single transaction.
        let results = self.observability.add_batch(&docs)?;

        let mut ids = Vec::with_capacity(results.len());
        let mut fts_batch:    Vec<(Uuid, String)>                        = Vec::new();
        let mut vector_batch: Vec<(String, Vec<f32>, Option<JsonValue>)> = Vec::new();

        for (i, (id, is_primary, opt_emb)) in results.into_iter().enumerate() {
            ids.push(id);
            if is_primary {
                fts_batch.push((id, fingerprints[i].clone()));
                // Reuse embedding from observability — no re-embed.
                vector_batch.push((id.to_string(), opt_emb.unwrap(), Some(docs[i].clone())));
            }
        }

        // One vector-store mutex acquisition + one Tantivy commit per
        // batch (instead of one per primary). This is the largest single
        // perf win for high-volume primary-heavy ingestion.
        self.vector.store_vectors_batch(vector_batch)?;
        self.fts.add_documents_batch(&fts_batch)?;
        Ok(ids)
    }

    /// Delete a record from `ObservabilityStorage` and, if it was a primary,
    /// also remove it from the FTS and vector indexes.
    ///
    /// Deleting a primary leaves its linked secondaries in `ObservabilityStorage`
    /// as unlinked records (they are not automatically promoted or removed).
    /// Returns `Ok(())` for unknown IDs.
    pub fn delete(&self, id: Uuid) -> Result<()> {
        let was_primary = self.observability.is_primary(id)?;
        self.observability.delete_by_id(id)?;
        if was_primary {
            self.fts.drop_document(id)?;
            self.vector.delete_vector(&id.to_string())?;
        }
        Ok(())
    }

    // ── reads ─────────────────────────────────────────────────────────────────

    /// Return the full JSON record for `id`, or `None` if not found.
    pub fn get(&self, id: Uuid) -> Result<Option<JsonValue>> {
        self.observability.get_by_id(id)
    }

    /// Return all records whose `key` matches, ordered by timestamp ascending.
    pub fn get_by_key(&self, key: &str) -> Result<Vec<JsonValue>> {
        self.observability.get_by_key(key)
    }

    /// Return only primary records whose `key` matches, ordered by timestamp ascending.
    pub fn get_primaries_by_key(&self, key: &str) -> Result<Vec<JsonValue>> {
        self.observability.get_primaries_by_key(key)
    }

    /// Flush all engines to disk.
    ///
    /// Calls `ObservabilityStorage::sync` (DuckDB CHECKPOINT),
    /// `FTSEngine::sync` (Tantivy commit + reload), `VectorEngine::sync`
    /// (HNSW save), and `DocumentStorage::sync` (tplstorage HNSW save).
    pub fn sync(&self) -> Result<()> {
        self.observability.sync()?;
        self.fts.sync()?;
        self.vector.sync()?;
        self.tplstorage.sync()?;
        Ok(())
    }

    // ── template storage ──────────────────────────────────────────────────────

    /// Store a template in this shard's tplstorage and index it.
    ///
    /// `metadata` should carry at least `"name"` and `"timestamp"` fields.
    /// The body is stored as UTF-8 bytes and both metadata and body are
    /// embedded automatically via the shared [`EmbeddingEngine`].
    /// Returns the UUIDv7 assigned to the template.
    pub fn tpl_add(&self, metadata: JsonValue, body: &[u8]) -> Result<Uuid> {
        self.tplstorage.add_document(metadata, body)
    }

    /// Store a template to DuckDB and frequency tracking without embedding.
    ///
    /// Pair with [`DocumentStorage::embed_documents_batch`] to amortize ONNX
    /// overhead across an entire ingest batch rather than paying it per template.
    pub fn tpl_add_no_embed(&self, metadata: JsonValue, body: &[u8]) -> Result<Uuid> {
        self.tplstorage.add_document_no_embed(metadata, body)
    }

    /// Return the JSON metadata for template `id`, or `None`.
    pub fn tpl_get_metadata(&self, id: Uuid) -> Result<Option<JsonValue>> {
        self.tplstorage.get_metadata(id)
    }

    /// Return the raw body bytes for template `id`, or `None`.
    pub fn tpl_get_body(&self, id: Uuid) -> Result<Option<Vec<u8>>> {
        self.tplstorage.get_content(id)
    }

    /// Replace the metadata for template `id` and re-embed it.
    pub fn tpl_update_metadata(&self, id: Uuid, metadata: JsonValue) -> Result<()> {
        self.tplstorage.update_metadata(id, metadata)?;
        self.tplstorage.reembed_document(id)
    }

    /// Replace the body for template `id` and re-embed it.
    pub fn tpl_update_body(&self, id: Uuid, body: &[u8]) -> Result<()> {
        self.tplstorage.update_content(id, body)?;
        self.tplstorage.reembed_document(id)
    }

    /// Remove template `id` from all sub-stores of tplstorage.
    pub fn tpl_delete(&self, id: Uuid) -> Result<()> {
        self.tplstorage.delete_document(id)
    }

    /// Return all `(id, metadata)` pairs stored in this shard's tplstorage.
    pub fn tpl_list(&self) -> Result<Vec<(Uuid, JsonValue)>> {
        self.tplstorage.list_all()
    }

    /// Semantic search over templates in this shard by plain-text query.
    pub fn tpl_search_text(&self, query: &str, limit: usize) -> Result<Vec<JsonValue>> {
        self.tplstorage.search_document_text(query, limit)
    }

    /// Semantic search over templates in this shard by JSON query.
    pub fn tpl_search_json(&self, query: &JsonValue, limit: usize) -> Result<Vec<JsonValue>> {
        self.tplstorage.search_document_json(query, limit)
    }

    /// Rebuild the tplstorage vector index from persisted metadata and blobs.
    ///
    /// Returns the number of templates re-indexed.
    pub fn tpl_reindex(&self) -> Result<usize> {
        self.tplstorage.reindex()
    }

    // ── passthrough accessors ─────────────────────────────────────────────────

    /// Borrow the underlying `ObservabilityStorage` for direct access to
    /// deduplication, primary/secondary, and time-range APIs.
    pub fn observability(&self) -> &ObservabilityStorage {
        &self.observability
    }

    // ── search ────────────────────────────────────────────────────────────────

    /// Full-text search over the JSON fingerprints of primary records.
    ///
    /// `query` uses Tantivy query syntax (e.g. `cpu AND usage`, `"disk full"`).
    /// Results are returned in Tantivy relevance order.
    ///
    /// Each returned document is the full JSON of the matching primary with a
    /// `"secondaries"` field containing the full JSON of every secondary linked
    /// to that primary.
    pub fn search_fts(&self, query: &str, limit: usize) -> Result<Vec<JsonValue>> {
        let ids = self.fts.search(query, limit)?;
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let mut doc_map: HashMap<Uuid, JsonValue> = self.observability
            .get_by_ids(&ids)?
            .into_iter()
            .filter_map(|doc| {
                doc["id"].as_str()
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .map(|uuid| (uuid, doc))
            })
            .collect();
        let present: Vec<Uuid> = ids.iter().copied().filter(|id| doc_map.contains_key(id)).collect();
        let mut sec_map = self.observability.get_secondaries_batch(&present)?;
        let mut results = Vec::with_capacity(present.len());
        for id in ids {
            if let Some(mut doc) = doc_map.remove(&id) {
                let secondaries = sec_map.remove(&id).unwrap_or_default();
                if let JsonValue::Object(ref mut map) = doc {
                    map.insert("secondaries".to_string(), json!(secondaries));
                }
                results.push(doc);
            }
        }
        Ok(results)
    }

    /// Full-text search returning `(primary_id, BM25_score)` pairs only — no
    /// document body is fetched. Use this when you only need IDs and relevance
    /// scores without the overhead of retrieving and deserialising full records.
    pub fn search_fts_scored(&self, query: &str, limit: usize) -> Result<Vec<(Uuid, f32)>> {
        self.fts.search_with_scores(query, limit)
    }

    /// Full-text search returning `(primary_id, unix_ts, BM25_score)` triples.
    ///
    /// The timestamp is fetched from `ObservabilityStorage` for each hit via a
    /// single indexed PK lookup. Records whose IDs have been deleted between
    /// the FTS search and the timestamp lookup are silently skipped.
    pub fn search_fts_with_ts(&self, query: &str, limit: usize) -> Result<Vec<(Uuid, i64, f32)>> {
        let hits = self.fts.search_with_scores(query, limit)?;
        let mut results = Vec::with_capacity(hits.len());
        for (id, score) in hits {
            if let Some(ts) = self.observability.get_ts_by_id(id)? {
                results.push((id, ts, score));
            }
        }
        Ok(results)
    }

    /// Semantic vector search with MMR reranking over primary records.
    ///
    /// `query` is fingerprinted, embedded, and used to search the HNSW index.
    /// A candidate pool of `max(limit * 2, 10)` nearest neighbours is fetched
    /// and reranked with `MMRReranker(λ = 0.7)` before the top `limit` results
    /// are selected.
    ///
    /// Each returned document is the full JSON of the matching primary with a
    /// `"_score"` field (cosine similarity, higher = more similar) and a
    /// `"secondaries"` field containing the full JSON of every linked secondary.
    /// Results are ordered by descending score.
    pub fn search_vector(&self, query: &JsonValue, limit: usize) -> Result<Vec<JsonValue>> {
        let candidate_pool = (limit * 2).max(10);
        let reranker = MMRReranker::new(0.7);
        let neighbors = self.vector.search_json_reranked(query, limit, candidate_pool, &reranker)?;
        self.neighbors_to_docs(neighbors)
    }

    /// Like [`search_vector`] but uses a pre-computed embedding vector and
    /// fingerprint string, avoiding a redundant ONNX inference call.
    ///
    /// Use this when searching across multiple shards with the same query —
    /// embed once in the caller and pass the result here.
    pub fn search_vector_precomputed(
        &self,
        query_vec: &[f32],
        fingerprint: &str,
        limit: usize,
    ) -> Result<Vec<JsonValue>> {
        let candidate_pool = (limit * 2).max(10);
        let reranker = MMRReranker::new(0.7);
        let neighbors = self.vector.search_reranked(
            query_vec.to_vec(),
            fingerprint,
            limit,
            candidate_pool,
            &reranker,
        )?;
        self.neighbors_to_docs(neighbors)
    }

    /// Vector search returning `(uuid, unix_ts, score)` triples only — no
    /// document body or secondaries are fetched.
    ///
    /// Uses a pre-computed embedding vector and fingerprint string.
    /// Prefer this over [`search_vector_precomputed`] when the caller only
    /// needs IDs, timestamps, and scores (e.g. for global merge + truncate).
    pub fn search_vector_scored_precomputed(
        &self,
        query_vec: &[f32],
        fingerprint: &str,
        limit: usize,
    ) -> Result<Vec<(Uuid, i64, f32)>> {
        let candidate_pool = (limit * 2).max(10);
        let reranker = MMRReranker::new(0.7);
        let neighbors = self.vector.search_reranked(
            query_vec.to_vec(),
            fingerprint,
            limit,
            candidate_pool,
            &reranker,
        )?;
        let mut results = Vec::with_capacity(neighbors.len());
        for n in neighbors {
            if let Ok(id) = Uuid::parse_str(&n.id) {
                let ts = self.observability.get_ts_by_id(id)?.unwrap_or(0);
                results.push((id, ts, n.score));
            }
        }
        Ok(results)
    }

    // ── internal ──────────────────────────────────────────────────────────────

    /// Batch-fetch full documents and secondaries for a list of vector search
    /// neighbors, preserving neighbor order and attaching `_score` and
    /// `secondaries` fields.
    fn neighbors_to_docs(
        &self,
        neighbors: Vec<SearchResult>,
    ) -> Result<Vec<JsonValue>> {
        if neighbors.is_empty() {
            return Ok(vec![]);
        }
        let mut id_order: Vec<Uuid> = Vec::with_capacity(neighbors.len());
        let mut score_map: HashMap<Uuid, f32> = HashMap::with_capacity(neighbors.len());
        for n in &neighbors {
            let id = Uuid::parse_str(&n.id).map_err(|e| {
                err_msg(format!("vector index contains invalid UUID '{}': {e}", n.id))
            })?;
            id_order.push(id);
            score_map.insert(id, n.score);
        }
        let mut doc_map: HashMap<Uuid, JsonValue> = self.observability
            .get_by_ids(&id_order)?
            .into_iter()
            .filter_map(|doc| {
                doc["id"].as_str()
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .map(|uuid| (uuid, doc))
            })
            .collect();
        let present: Vec<Uuid> = id_order.iter().copied().filter(|id| doc_map.contains_key(id)).collect();
        let mut sec_map = self.observability.get_secondaries_batch(&present)?;
        let mut results = Vec::with_capacity(present.len());
        for id in id_order {
            if let Some(mut doc) = doc_map.remove(&id) {
                if let JsonValue::Object(ref mut map) = doc {
                    if let Some(&score) = score_map.get(&id) {
                        map.insert("_score".to_string(), json!(score));
                    }
                    let secondaries = sec_map.remove(&id).unwrap_or_default();
                    map.insert("secondaries".to_string(), json!(secondaries));
                }
                results.push(doc);
            }
        }
        Ok(results)
    }
}
