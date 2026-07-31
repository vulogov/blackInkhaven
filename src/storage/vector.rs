//! HNSW vector index backed by the `vecstore` crate. Mirrors the
//! ergonomics of bdslib's `vectorengine.rs` — same lazy open, same
//! cosine-distance-to-similarity score flip — but with the reranker
//! pathway and unused batch/single-doc helpers removed.

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use vecstore::{Metadata, Query, VecStore};

pub use vecstore::Neighbor as SearchResult;

use crate::storage::embedding::EmbeddingEngine;
use crate::storage::fingerprint::json_fingerprint;

/// After this many consecutive background-sync failures, give up the current
/// pass (leaving `dirty` set for the next trigger) rather than spinning.
const MAX_SYNC_RETRIES: u32 = 5;

/// The background-sync retry policy after one `drain_dirty` attempt. Pure so the
/// no-spin guarantee is unit-testable without a failing store. Returns the new
/// consecutive-failure count, an optional backoff sleep, and whether to give up
/// this background pass. On success the counter resets; on failure it backs off
/// (linearly) until [`MAX_SYNC_RETRIES`], then gives up so a persistent I/O
/// fault (disk full / read-only volume) can't pin a CPU core re-attempting the
/// failing write.
fn sync_retry_step(succeeded: bool, failures: u32) -> (u32, Option<std::time::Duration>, bool) {
    if succeeded {
        return (0, None, false);
    }
    let f = failures + 1;
    if f >= MAX_SYNC_RETRIES {
        (f, None, true)
    } else {
        (f, Some(std::time::Duration::from_millis(100 * f as u64)), false)
    }
}

/// Thread-safe HNSW index wrapper. The underlying `VecStore` is opened
/// lazily on the first vector operation — important when a project is
/// opened purely to read DuckDB metadata (e.g. CLI `list`) and the
/// vector index would otherwise be deserialised for no reason.
///
/// `dirty` tracks whether the in-memory index has unpersisted writes.
/// Every successful upsert / remove flips it true; `sync()` short-
/// circuits when it's already clean. This is what lets the background
/// sync task tick at 10-minute cadence without actually rewriting the
/// index when the editor has been idle.
#[derive(Clone)]
pub struct VectorEngine {
    path: String,
    store: Arc<Mutex<Option<VecStore>>>,
    embedding: Option<Arc<EmbeddingEngine>>,
    dirty: Arc<AtomicBool>,
    /// 1.8.34 hardening — true while a background sync thread is running, so a
    /// burst of saves spawns at most ONE such thread (it coalesces later writes)
    /// instead of piling detached threads up on the store lock.
    sync_in_flight: Arc<AtomicBool>,
}

impl VectorEngine {
    /// Whether the embedding model has been lazily loaded yet (HAIKU-2 gates the
    /// semantic path on this so a cold engine never blocks the UI thread).
    pub fn embedding_is_loaded(&self) -> bool {
        self.embedding.as_ref().map(|e| e.is_loaded()).unwrap_or(false)
    }

    pub fn with_embedding(path: &str, engine: EmbeddingEngine) -> Result<Self> {
        Ok(Self {
            path: path.to_string(),
            store: Arc::new(Mutex::new(None)),
            embedding: Some(Arc::new(engine)),
            dirty: Arc::new(AtomicBool::new(false)),
            sync_in_flight: Arc::new(AtomicBool::new(false)),
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
        let dirty = self.dirty.clone();
        self.with_store(|s| {
            s.upsert(id.to_string(), vector, meta)
                .map_err(|e| anyhow!("failed to store document {id:?}: {e}"))?;
            dirty.store(true, Ordering::Release);
            Ok(())
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
        let dirty = self.dirty.clone();
        self.with_store(|s| {
            for ((id, doc), vector) in entries.iter().zip(vectors) {
                let meta = json_to_metadata(doc.clone());
                s.upsert(id.to_string(), vector, meta)
                    .map_err(|e| anyhow!("failed to store document {id:?}: {e}"))?;
            }
            dirty.store(true, Ordering::Release);
            Ok(())
        })
    }

    pub fn delete_vector(&self, id: &str) -> Result<()> {
        let dirty = self.dirty.clone();
        self.with_store(|s| {
            match s.remove(id) {
                Ok(()) => {
                    dirty.store(true, Ordering::Release);
                    Ok(())
                }
                Err(e) if e.to_string().to_lowercase().contains("not found") => Ok(()),
                Err(e) => Err(anyhow!("failed to remove vector {id:?}: {e}")),
            }
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

    /// Flush the index to disk *only when* there are writes since the
    /// last sync. The fast path (clean index) skips the mutex entirely
    /// — important because the background task ticks every 10 minutes
    /// regardless of activity, and an idle editor produces zero
    /// vector writes between ticks.
    ///
    /// On save failure `dirty` is restored to `true` so the next tick
    /// retries instead of silently dropping the unpersisted writes.
    /// 1.2.16+ Phase P.4 — total vector count in
    /// the HNSW store.  The store holds two
    /// vectors per document (`:meta` + `:content`)
    /// — the parity check accounts for this by
    /// dividing.  Cheap (single VecStore::count
    /// call); used by `Store::vector_count`.
    pub fn count(&self) -> Result<usize> {
        self.with_store(|s| Ok(s.count()))
    }

    /// Embed arbitrary texts with the configured engine — for on-demand
    /// semantic checks (e.g. conlang near-synonym detection) that aren't
    /// backed by the stored vector index. Reuses the already-loaded model.
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let Some(engine) = &self.embedding else {
            return Err(anyhow!("vector engine has no embedding model configured"));
        };
        engine.embed_batch(texts)
    }

    pub fn sync(&self) -> Result<()> {
        if !self.dirty.load(Ordering::Acquire) {
            return Ok(());
        }
        Self::drain_dirty(&self.store, &self.dirty)
    }

    /// 1.8.32+ hardening — flush the index **off the calling thread**. The vector
    /// index is a *derived* artifact (embeddings of content that is already
    /// durable via the metadata/blob stores' per-commit fsync), and its
    /// `VecStore::save` writes atomically (temp file + rename), so a crash
    /// mid-flush keeps the last good index and the pending embedding is
    /// recomputed on next access — no user data is at risk. Backgrounding it
    /// keeps a routine paragraph save from freezing the editor's render thread
    /// while the whole HNSW index is serialized to disk. Clean (`!dirty`) syncs
    /// don't spawn; the periodic background tick and the quit-path `sync()` stay
    /// synchronous, so the index always converges.
    pub fn sync_in_background(&self) {
        if !self.dirty.load(Ordering::Acquire) {
            return;
        }
        // M2 — at most one background sync thread at a time. If one is already
        // running it re-checks `dirty` after each save (the loop below), so this
        // write is coalesced into it rather than spawning another thread that
        // would just queue on the store lock.
        if self.sync_in_flight.swap(true, Ordering::AcqRel) {
            return;
        }
        let store = self.store.clone();
        let dirty = self.dirty.clone();
        let in_flight = self.sync_in_flight.clone();
        std::thread::spawn(move || {
            let mut failures: u32 = 0;
            loop {
                let (next_failures, backoff, give_up) =
                    match Self::drain_dirty(&store, &dirty) {
                        Ok(()) => sync_retry_step(true, failures),
                        Err(e) => {
                            tracing::warn!(
                                target: "inkhaven::storage::vector",
                                "background vector sync failed (attempt {}): {e}",
                                failures + 1,
                            );
                            sync_retry_step(false, failures)
                        }
                    };
                failures = next_failures;
                if let Some(delay) = backoff {
                    std::thread::sleep(delay);
                }
                // Release the flag, then re-check: a writer that set `dirty`
                // during our save is flushed on this same thread instead of
                // waiting for the next save / periodic tick. The release-then-
                // recheck (with a reclaiming swap) closes the lost-wakeup window.
                // Release BEFORE any give-up break so the next write can spawn a
                // fresh sync once a persistent I/O fault clears.
                in_flight.store(false, Ordering::Release);
                if give_up {
                    // A persistent save error (disk full / read-only / unplugged
                    // volume): `dirty` stays set, so the next write or periodic
                    // tick retries — but this thread exits instead of spinning a
                    // core re-attempting the failing write.
                    break;
                }
                if !dirty.load(Ordering::Acquire) {
                    break;
                }
                if in_flight.swap(true, Ordering::AcqRel) {
                    // Another sync_in_background reclaimed the flag first; it will
                    // handle the pending write.
                    break;
                }
            }
        });
    }

    /// The shared flush body for [`Self::sync`] and [`Self::sync_in_background`]:
    /// under the store lock, re-check the dirty flag (a racing sync may have
    /// drained it while we waited), then atomically save and clear it.
    fn drain_dirty(store: &Mutex<Option<VecStore>>, dirty: &AtomicBool) -> Result<()> {
        let mut guard = store.lock();
        if !dirty.load(Ordering::Acquire) {
            return Ok(());
        }
        let Some(s) = guard.as_mut() else {
            // Shouldn't happen — writes lazily open the store before
            // they can flip dirty — but stay defensive.
            dirty.store(false, Ordering::Release);
            return Ok(());
        };
        match s.save() {
            Ok(()) => {
                dirty.store(false, Ordering::Release);
                Ok(())
            }
            Err(e) => Err(anyhow!("failed to sync vector store: {e}")),
        }
    }

    fn with_store<R, F: FnOnce(&mut VecStore) -> Result<R>>(&self, f: F) -> Result<R> {
        let mut guard = self.store.lock();
        if guard.is_none() {
            *guard = Some(
                VecStore::open(&self.path)
                    .map_err(|e| anyhow!("failed to open vector store at {:?}: {e}", self.path))?,
            );
        }
        // 1.2.15+ Phase S.5 — `.expect()` instead of
        // `.unwrap()` so the invariant is captured in
        // the message ("set in the block immediately
        // above").  Functionally identical; the panic
        // surface only fires if the invariant is
        // broken — and now the panic message tells
        // future-us why.
        let store = guard.as_mut().expect("set immediately above when None");
        f(store)
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

#[cfg(test)]
mod tests_sync_retry {
    use super::{sync_retry_step, MAX_SYNC_RETRIES};

    #[test]
    fn success_resets_and_never_backs_off() {
        let (failures, backoff, give_up) = sync_retry_step(true, 4);
        assert_eq!(failures, 0);
        assert!(backoff.is_none());
        assert!(!give_up);
    }

    #[test]
    fn failures_back_off_linearly_then_give_up() {
        // Walk a run of consecutive failures; the background sync must stop
        // spinning once it has retried MAX_SYNC_RETRIES times.
        let mut failures = 0u32;
        let mut gave_up = false;
        for step in 1..=MAX_SYNC_RETRIES {
            let (f, backoff, give_up) = sync_retry_step(false, failures);
            failures = f;
            assert_eq!(f, step);
            if step < MAX_SYNC_RETRIES {
                // A bounded, increasing pause — not a busy spin.
                assert_eq!(backoff.unwrap().as_millis() as u64, 100 * step as u64);
                assert!(!give_up);
            } else {
                assert!(backoff.is_none());
                assert!(give_up);
                gave_up = true;
            }
        }
        assert!(gave_up, "must give up at the retry ceiling");
    }
}
