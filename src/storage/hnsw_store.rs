//! Owned HNSW vector store — inkhaven's in-tree replacement for the
//! abandoned `vecstore` crate. Backed directly by `hnsw_rs`.
//!
//! The one thing vecstore got wrong and this fixes: it **rebuilt** the whole
//! HNSW graph from raw vectors on every `open` (~15–30 s for inkhaven's
//! corpus, landing on the first F1 search), because its generic
//! distance-metric enum made the persisted graph awkward to reload. Inkhaven
//! only ever uses **cosine** distance, so that polymorphism never arises — we
//! reload the graph dump directly via `HnswIo::load_hnsw` (measured ~50 ms for
//! ~10k points) and skip the reconstruction entirely.
//!
//! Design (vecstore's proven shape, plus load-on-open):
//!   - `records: id -> vector` is the **source of truth**. Persisted with
//!     bincode; a paragraph save updates it and re-persists.
//!   - the HNSW graph is a rebuildable **query accelerator** over those
//!     records. Loaded from the dump on open; rebuilt from records only when
//!     the dump is missing/inconsistent, or compacted on save when orphaned.
//!
//! Writes stay incremental: `upsert` inserts straight into the live (reloaded)
//! graph — verified that a reloaded `hnsw_rs` graph accepts new points and can
//! be re-dumped — so a paragraph save re-indexes just that paragraph.
//! `remove` is a tombstone (drops the id↔idx mapping so the point is filtered
//! out of every search) and a re-`upsert` orphans the old point; both leave
//! dead points in the graph. `query` over-searches in proportion to the orphan
//! fraction so filtering still yields a full result set, and `save` compacts
//! (rebuilds the graph from live records, resetting DataIds) once orphans pass
//! [`ORPHAN_COMPACT_THRESHOLD`], so the graph can't bloat without bound across
//! a long editing session.
//!
//! On-disk layout, all inside the project's `vectors/` dir:
//!   - `records.bin` — bincode `{ dim, records: [(id, vector)] }`, written
//!     first on save (the durable source of truth).
//!   - `graph.hnsw.graph` + `graph.hnsw.data` — the `hnsw_rs` file dump.
//!   - `vecmap.json` — `{ next_idx, entries: [(id, DataId)] }`, written last so
//!     it doubles as the accelerator's commit marker: `open` trusts the dump
//!     only when the map is present and its point count matches the graph.

use anyhow::{anyhow, Result};
use hnsw_rs::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Per-document metadata bag. Accepted on `upsert` for source-compatibility
/// with the old vecstore surface, but **not persisted**: inkhaven reads a
/// document's metadata back from the DuckDB metadata store (see
/// `DocumentStorage::build_results`), never from the vector index, so the
/// bag would be write-only dead weight on disk.
#[derive(Debug, Clone, Default)]
pub struct Metadata {
    // Populated by the caller (`json_to_metadata`) but intentionally never read
    // back: the vector index doesn't persist metadata, so this bag is inert.
    // Kept public so the construction site in `vector.rs` compiles unchanged.
    #[allow(dead_code)]
    pub fields: HashMap<String, JsonValue>,
}

/// A search request: the embedded query vector plus a neighbour limit.
#[derive(Debug, Clone)]
pub struct Query {
    vector: Vec<f32>,
    limit: usize,
}

impl Query {
    pub fn new(vector: Vec<f32>) -> Self {
        Self { vector, limit: 10 }
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

/// One search hit: the document id and its cosine **distance** (lower = more
/// similar). `VectorEngine::search` flips this into a similarity before it
/// reaches callers, matching the old vecstore convention.
#[derive(Debug, Clone)]
pub struct Neighbor {
    pub id: String,
    pub score: f32,
}

/// hnsw_rs graph construction parameters — mirror the values vecstore used so
/// recall characteristics are unchanged.
const MAX_NB_CONNECTION: usize = 16;
const MAX_ELEMENTS: usize = 100_000;
const MAX_LAYER: usize = 16;
const EF_CONSTRUCTION: usize = 200;

/// `hnsw_rs` `file_dump` basename → `graph.hnsw.graph` / `graph.hnsw.data`.
const GRAPH_BASENAME: &str = "graph";
/// Temp basename used while re-dumping, renamed over `GRAPH_BASENAME` on save.
const GRAPH_TMP_BASENAME: &str = "graph.next";
const MAP_FILE: &str = "vecmap.json";
const RECORDS_FILE: &str = "records.bin";

/// Compact (rebuild the graph from live records) on save once dead points —
/// tombstoned removals plus the orphans left by re-`upsert`s — exceed this
/// fraction of the graph. Keeps query over-search bounded and the dump small.
const ORPHAN_COMPACT_THRESHOLD: f64 = 0.5;

/// Never over-search more than this many neighbours, however orphaned the graph
/// — a backstop so a pathological (uncompacted) graph can't turn one query into
/// a near-full scan.
const MAX_OVERSEARCH: usize = 4096;

#[derive(Serialize, Deserialize)]
struct VecMap {
    next_idx: usize,
    /// `(document id, internal hnsw DataId)` pairs — the live (non-tombstoned)
    /// mappings only.
    entries: Vec<(String, usize)>,
}

#[derive(Serialize, Deserialize, Default)]
struct RecordsFile {
    /// Embedding dimension (0 when empty); a mismatch on reload rejects the dump.
    dim: usize,
    records: Vec<(String, Vec<f32>)>,
}

/// An owned cosine-distance HNSW index over string-keyed vectors.
pub struct VecStore {
    dir: PathBuf,
    hnsw: Hnsw<'static, f32, DistCosine>,
    /// Source of truth: id -> vector. `save` persists this; the graph is
    /// rebuildable from it.
    records: HashMap<String, Vec<f32>>,
    id_to_idx: HashMap<String, usize>,
    idx_to_id: HashMap<usize, String>,
    /// Monotonic DataId counter. Never reused, so a re-`upsert` of an existing
    /// id gets a fresh DataId and the old point becomes a graph orphan until the
    /// next compaction.
    next_idx: usize,
    dim: usize,
}

impl VecStore {
    /// Open the store at `path` (the project's `vectors/` dir). Loads the
    /// records, then reloads the persisted graph when its dump + map are
    /// present and consistent; otherwise rebuilds the graph from the records
    /// (no re-embedding needed). Starts empty only when there are no records.
    pub fn open(path: &str) -> Result<Self> {
        let dir = PathBuf::from(path);
        fs::create_dir_all(&dir).map_err(|e| anyhow!("create vector dir {dir:?}: {e}"))?;

        let records = Self::load_records(&dir).unwrap_or_else(|e| {
            tracing::warn!(
                target: "inkhaven::storage::hnsw",
                "vector records unreadable ({e}); starting empty — a reindex will repopulate"
            );
            RecordsFile::default()
        });

        if records.records.is_empty() {
            return Ok(Self::empty(dir));
        }

        let record_map: HashMap<String, Vec<f32>> = records.records.into_iter().collect();
        let dim = records.dim;

        // Fast path: reload the persisted graph if it matches the records.
        match Self::try_load_graph(&dir, &record_map) {
            Ok(Some(store)) => return Ok(store),
            Ok(None) => {} // no/inconsistent dump — fall through to rebuild
            Err(e) => tracing::warn!(
                target: "inkhaven::storage::hnsw",
                "graph reload failed ({e}); rebuilding from records"
            ),
        }

        // Slow path (rare: first open after a format change, or a lost/corrupt
        // dump). Rebuild the accelerator from the durable records.
        Ok(Self::from_records(dir, record_map, dim))
    }

    fn empty(dir: PathBuf) -> Self {
        Self {
            dir,
            hnsw: Self::new_hnsw(),
            records: HashMap::new(),
            id_to_idx: HashMap::new(),
            idx_to_id: HashMap::new(),
            next_idx: 0,
            dim: 0,
        }
    }

    fn new_hnsw() -> Hnsw<'static, f32, DistCosine> {
        Hnsw::<f32, DistCosine>::new(
            MAX_NB_CONNECTION,
            MAX_ELEMENTS,
            MAX_LAYER,
            EF_CONSTRUCTION,
            DistCosine,
        )
    }

    fn load_records(dir: &Path) -> Result<RecordsFile> {
        let path = dir.join(RECORDS_FILE);
        if !path.is_file() {
            return Ok(RecordsFile::default());
        }
        let bytes = fs::read(&path).map_err(|e| anyhow!("read {path:?}: {e}"))?;
        bincode::deserialize(&bytes).map_err(|e| anyhow!("parse {path:?}: {e}"))
    }

    /// Reload the graph dump and its id↔idx map, validating both against the
    /// records. Returns `Ok(None)` when the dump is absent or inconsistent (so
    /// the caller rebuilds), `Err` only on an unexpected reload failure.
    fn try_load_graph(
        dir: &Path,
        record_map: &HashMap<String, Vec<f32>>,
    ) -> Result<Option<Self>> {
        let graph = dir.join(format!("{GRAPH_BASENAME}.hnsw.graph"));
        let data = dir.join(format!("{GRAPH_BASENAME}.hnsw.data"));
        let mapf = dir.join(MAP_FILE);
        if !(graph.is_file() && data.is_file() && mapf.is_file()) {
            return Ok(None);
        }

        let raw = fs::read(&mapf).map_err(|e| anyhow!("read {mapf:?}: {e}"))?;
        let map: VecMap =
            serde_json::from_slice(&raw).map_err(|e| anyhow!("parse {mapf:?}: {e}"))?;

        // The live map must describe exactly the records we hold; otherwise the
        // dump is stale relative to records.bin and we can't trust it.
        if map.entries.len() != record_map.len()
            || !map.entries.iter().all(|(id, _)| record_map.contains_key(id))
        {
            return Ok(None);
        }

        // `load_hnsw` returns an `Hnsw` that borrows its `HnswIo` loader. We
        // leak the loader so the reloaded graph is `'static` and can be owned +
        // mutated: exactly one leak per store open, and the store lives for the
        // whole session (CLI process or TUI), so this is a bounded, one-time
        // cost — not a growing leak. The default (non-mmap) reload copies the
        // point data into memory, so re-dumping over the files on `save` is safe.
        let io: &'static mut HnswIo = Box::leak(Box::new(HnswIo::new(dir, GRAPH_BASENAME)));
        let hnsw: Hnsw<'static, f32, DistCosine> = io
            .load_hnsw::<f32, DistCosine>()
            .map_err(|e| anyhow!("reload hnsw graph: {e}"))?;

        // The graph holds every point ever inserted (orphans included); that
        // must equal next_idx, or the dump and map disagree.
        if hnsw.get_nb_point() != map.next_idx {
            return Ok(None);
        }

        let mut id_to_idx = HashMap::with_capacity(map.entries.len());
        let mut idx_to_id = HashMap::with_capacity(map.entries.len());
        for (id, idx) in map.entries {
            id_to_idx.insert(id.clone(), idx);
            idx_to_id.insert(idx, id);
        }

        let dim = record_map.values().next().map(|v| v.len()).unwrap_or(0);
        Ok(Some(Self {
            dir: dir.to_path_buf(),
            hnsw,
            records: record_map.clone(),
            id_to_idx,
            idx_to_id,
            next_idx: map.next_idx,
            dim,
        }))
    }

    /// Build a fresh, compact graph (DataIds `0..n`, zero orphans) from records.
    fn from_records(dir: PathBuf, records: HashMap<String, Vec<f32>>, dim: usize) -> Self {
        let hnsw = Self::new_hnsw();
        let mut id_to_idx = HashMap::with_capacity(records.len());
        let mut idx_to_id = HashMap::with_capacity(records.len());
        let items: Vec<(&Vec<f32>, usize)> = records
            .iter()
            .enumerate()
            .map(|(idx, (id, vec))| {
                id_to_idx.insert(id.clone(), idx);
                idx_to_id.insert(idx, id.clone());
                (vec, idx)
            })
            .collect();
        if !items.is_empty() {
            hnsw.parallel_insert(&items);
        }
        let next_idx = items.len();
        let dim = if dim > 0 {
            dim
        } else {
            records.values().next().map(|v| v.len()).unwrap_or(0)
        };
        Self {
            dir,
            hnsw,
            records,
            id_to_idx,
            idx_to_id,
            next_idx,
            dim,
        }
    }

    /// Insert or replace the vector stored under `id`. A replace tombstones the
    /// old DataId (dropped from the reverse map, orphaned in the graph) and
    /// inserts the new vector under a fresh DataId. `metadata` is accepted for
    /// API parity and intentionally discarded (see [`Metadata`]).
    pub fn upsert(&mut self, id: String, vector: Vec<f32>, _metadata: Metadata) -> Result<()> {
        if self.dim == 0 {
            self.dim = vector.len();
        }
        if let Some(old_idx) = self.id_to_idx.remove(&id) {
            self.idx_to_id.remove(&old_idx);
        }
        let idx = self.next_idx;
        self.next_idx += 1;
        self.hnsw.insert((vector.as_slice(), idx));
        self.records.insert(id.clone(), vector);
        self.id_to_idx.insert(id.clone(), idx);
        self.idx_to_id.insert(idx, id);
        Ok(())
    }

    /// Tombstone the vector stored under `id`. The point stays in the graph but
    /// is filtered out of every future search. Errors with a "not found"
    /// message when the id is absent (callers tolerate that case).
    pub fn remove(&mut self, id: &str) -> Result<()> {
        match self.id_to_idx.remove(id) {
            Some(idx) => {
                self.idx_to_id.remove(&idx);
                self.records.remove(id);
                Ok(())
            }
            None => Err(anyhow!("id not found: {id}")),
        }
    }

    /// k-nearest search. Returns up to `limit` live neighbours ordered nearest
    /// first, each carrying its raw cosine distance in `score`. Over-searches in
    /// proportion to the orphan fraction so tombstone filtering still yields a
    /// full result set.
    pub fn query(&self, q: Query) -> Result<Vec<Neighbor>> {
        let live = self.id_to_idx.len();
        if live == 0 {
            return Ok(Vec::new());
        }
        let k = q.limit.max(1);

        // Inflate the neighbour count so that, after filtering out ~(1 - live
        // ratio) orphans, roughly k live hits remain. Clamp between k and a
        // hard backstop so a pathological graph can't become a full scan.
        let total = self.next_idx.max(live);
        let live_ratio = live as f64 / total as f64;
        let inflated = if live_ratio > 0.0 {
            ((k as f64) / live_ratio).ceil() as usize
        } else {
            k
        };
        let knbn = inflated.clamp(k, MAX_OVERSEARCH).min(total);
        let ef = knbn.max(64);

        let hits = self.hnsw.search(&q.vector, knbn, ef);
        let mut out = Vec::with_capacity(k);
        for h in hits {
            if let Some(id) = self.idx_to_id.get(&h.d_id) {
                out.push(Neighbor {
                    id: id.clone(),
                    score: h.distance,
                });
                if out.len() == k {
                    break;
                }
            }
        }
        Ok(out)
    }

    /// Number of live (non-tombstoned) vectors.
    pub fn count(&self) -> usize {
        self.records.len()
    }

    fn orphan_fraction(&self) -> f64 {
        if self.next_idx == 0 {
            return 0.0;
        }
        let live = self.id_to_idx.len();
        (self.next_idx - live) as f64 / self.next_idx as f64
    }

    /// Persist the records (source of truth) and the graph accelerator.
    ///
    /// Compacts first when the graph is more than [`ORPHAN_COMPACT_THRESHOLD`]
    /// dead points, so a long editing session's re-`upsert` orphans can't
    /// accumulate without bound. Writes `records.bin` first (durable), then the
    /// graph dump, then `vecmap.json` last as the accelerator's commit marker —
    /// so a crash mid-save leaves `open` to rebuild the graph from records
    /// rather than trust a half-written dump.
    pub fn save(&mut self) -> Result<()> {
        if self.orphan_fraction() > ORPHAN_COMPACT_THRESHOLD && !self.records.is_empty() {
            self.compact();
        }

        // 1. records.bin — the source of truth, written first.
        let rf = RecordsFile {
            dim: self.dim,
            records: self
                .records
                .iter()
                .map(|(id, v)| (id.clone(), v.clone()))
                .collect(),
        };
        let bytes = bincode::serialize(&rf).map_err(|e| anyhow!("serialize records: {e}"))?;
        crate::io_atomic::write(&self.dir.join(RECORDS_FILE), &bytes)
            .map_err(|e| anyhow!("write records: {e}"))?;

        // 2. graph dump — temp basename, then rename into place.
        let tmp_graph = self.dir.join(format!("{GRAPH_TMP_BASENAME}.hnsw.graph"));
        let tmp_data = self.dir.join(format!("{GRAPH_TMP_BASENAME}.hnsw.data"));
        let _ = fs::remove_file(&tmp_graph);
        let _ = fs::remove_file(&tmp_data);
        self.hnsw
            .file_dump(&self.dir, GRAPH_TMP_BASENAME)
            .map_err(|e| anyhow!("dump hnsw graph: {e}"))?;
        let final_graph = self.dir.join(format!("{GRAPH_BASENAME}.hnsw.graph"));
        let final_data = self.dir.join(format!("{GRAPH_BASENAME}.hnsw.data"));
        fs::rename(&tmp_graph, &final_graph)
            .map_err(|e| anyhow!("commit graph file {final_graph:?}: {e}"))?;
        fs::rename(&tmp_data, &final_data)
            .map_err(|e| anyhow!("commit graph data {final_data:?}: {e}"))?;

        // 3. vecmap.json — the commit marker, written last (atomically).
        let map = VecMap {
            next_idx: self.next_idx,
            entries: self
                .id_to_idx
                .iter()
                .map(|(id, idx)| (id.clone(), *idx))
                .collect(),
        };
        let map_bytes = serde_json::to_vec(&map).map_err(|e| anyhow!("serialize vecmap: {e}"))?;
        crate::io_atomic::write(&self.dir.join(MAP_FILE), &map_bytes)
            .map_err(|e| anyhow!("write vecmap: {e}"))?;
        Ok(())
    }

    /// Rebuild the graph from live records, resetting DataIds to `0..n` so the
    /// orphan count returns to zero. In-memory only; `save` persists the result.
    fn compact(&mut self) {
        let rebuilt = Self::from_records(
            self.dir.clone(),
            std::mem::take(&mut self.records),
            self.dim,
        );
        self.hnsw = rebuilt.hnsw;
        self.records = rebuilt.records;
        self.id_to_idx = rebuilt.id_to_idx;
        self.idx_to_id = rebuilt.idx_to_id;
        self.next_idx = rebuilt.next_idx;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vec_for(seed: usize, dim: usize) -> Vec<f32> {
        (0..dim)
            .map(|i| (((seed * 131 + i * 17) % 1000) as f32) / 1000.0)
            .collect()
    }

    #[test]
    fn upsert_query_remove_and_reload_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        let dim = 32;

        {
            let mut s = VecStore::open(&path).unwrap();
            for i in 0..100usize {
                s.upsert(format!("id{i}"), vec_for(i, dim), Metadata::default())
                    .unwrap();
            }
            assert_eq!(s.count(), 100);
            let hits = s.query(Query::new(vec_for(5, dim)).with_limit(3)).unwrap();
            assert_eq!(hits.first().map(|n| n.id.as_str()), Some("id5"));
            s.save().unwrap();
        }

        // Reload (no rebuild — the dump matches records) and verify search.
        {
            let mut s = VecStore::open(&path).unwrap();
            assert_eq!(s.count(), 100, "reloaded records preserve live count");
            let hits = s.query(Query::new(vec_for(42, dim)).with_limit(3)).unwrap();
            assert_eq!(hits.first().map(|n| n.id.as_str()), Some("id42"));

            // Incremental write into the reloaded graph (re-index on save).
            s.upsert("id42".to_string(), vec_for(999, dim), Metadata::default())
                .unwrap();
            assert_eq!(s.count(), 100, "replace keeps count stable");
            let hits = s.query(Query::new(vec_for(999, dim)).with_limit(1)).unwrap();
            assert_eq!(hits.first().map(|n| n.id.as_str()), Some("id42"));

            s.remove("id7").unwrap();
            assert_eq!(s.count(), 99);
            let hits = s.query(Query::new(vec_for(7, dim)).with_limit(5)).unwrap();
            assert!(hits.iter().all(|n| n.id != "id7"), "removed id must not surface");
            assert!(s.remove("id7").is_err(), "second remove reports not found");

            s.save().unwrap();
        }

        {
            let s = VecStore::open(&path).unwrap();
            assert_eq!(s.count(), 99);
            let hits = s.query(Query::new(vec_for(999, dim)).with_limit(1)).unwrap();
            assert_eq!(hits.first().map(|n| n.id.as_str()), Some("id42"));
        }
    }

    #[test]
    fn heavy_orphaning_never_starves_results_and_compacts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        let dim = 24;

        let mut s = VecStore::open(&path).unwrap();
        for i in 0..60usize {
            s.upsert(format!("id{i}"), vec_for(i, dim), Metadata::default())
                .unwrap();
        }
        // Re-upsert every id many times → each leaves an orphan in the graph.
        for _round in 0..6 {
            for i in 0..60usize {
                s.upsert(format!("id{i}"), vec_for(i, dim), Metadata::default())
                    .unwrap();
            }
        }
        assert!(
            s.orphan_fraction() > ORPHAN_COMPACT_THRESHOLD,
            "test should have driven orphan fraction high (was {:.2})",
            s.orphan_fraction()
        );

        // Despite ~86% orphans, a small-k query must still return live hits.
        // The guarantee over-search provides is *non-starvation* — never the
        // empty "No results" that filtering exactly-k top hits produced. (Exact
        // top-k rank is an approximate-search property and would flake here, so
        // we assert the invariant, not the ranking.)
        let hits = s.query(Query::new(vec_for(3, dim)).with_limit(3)).unwrap();
        assert!(
            !hits.is_empty(),
            "over-search must not starve a heavily-orphaned graph to zero results"
        );
        assert!(
            hits.iter().all(|n| s.records.contains_key(&n.id)),
            "every returned id must be a live record"
        );

        // Saving compacts the graph back to zero orphans.
        s.save().unwrap();
        assert_eq!(s.orphan_fraction(), 0.0, "save compacts away the orphans");
        assert_eq!(s.count(), 60);
        let hits = s.query(Query::new(vec_for(50, dim)).with_limit(1)).unwrap();
        assert!(!hits.is_empty(), "compacted graph still searchable");
    }
}
