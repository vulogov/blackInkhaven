# bdslib — Database and Storage Architecture

bdslib is not a single database — it's a **layered composition of
specialised stores**, each chosen for a specific job, all coordinated
by a thin shard-and-cache layer above. This document explains every
storage layer in the project, what role it plays, what file lives
where, and how the pieces fit together.

If the storage stack feels surprising at first, it's because most
"databases" are general-purpose engines wrapped in a single API.
bdslib is the opposite: every storage primitive does one thing well,
and the higher-level types compose them. The result is that a single
record arriving over JSON-RPC may end up in three or four physical
stores at once — each tuned for the access pattern that store will
serve.

This document covers:

1. [The storage stack at a glance](#1-the-storage-stack-at-a-glance)
2. [The DuckDB foundation](#2-the-duckdb-foundation)
3. [Primitive stores](#3-primitive-stores)
4. [Search engines](#4-search-engines)
5. [Composite stores](#5-composite-stores)
6. [The shard layer](#6-the-shard-layer)
7. [The on-disk filesystem layout](#7-the-on-disk-filesystem-layout)
8. [What gets written when, and why](#8-what-gets-written-when-and-why)
9. [Threading, pooling, and persistence](#9-threading-pooling-and-persistence)
10. [Operational notes](#10-operational-notes)

---

## 1. The storage stack at a glance

Everything in bdslib that persists data is built on one of three
backends:

| Backend | Used for | Crate |
|---|---|---|
| **DuckDB** (with R2D2 connection pool) | Structured rows: telemetry, blobs, JSON documents, frequency tracking, shard metadata | `duckdb` + `r2d2` |
| **Tantivy** | Full-text search index per shard | `tantivy` |
| **VecStore** (HNSW) | Vector / semantic-similarity search | `vecstore` |

Around those, four families of types layer specialisation:

```
┌─────────────────────────────────────────────────────────────────────┐
│                       Composite stores                              │
│   DocumentStorage    (metadata + blob + vector + frequency)         │
│   ObservabilityStorage  (telemetry + dedup + primary/secondary)     │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ built on
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     Primitive stores                                │
│   BlobStorage   JsonStorage   FrequencyTracking   ShardInfoEngine   │
│   FTSEngine     VectorEngine                                        │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ built on
                       ▼
┌─────────────────────────────────────────────────────────────────────┐
│                  StorageEngine (DuckDB pool)                        │
│         + Tantivy (FTSEngine)  + VecStore (VectorEngine)            │
└─────────────────────────────────────────────────────────────────────┘
```

And one orchestration layer turns a stream of records into time-partitioned shards:

```
┌─────────────────────────────────────────────────────────────────────┐
│                       ShardsManager                                 │
│  - hjson config                                                     │
│  - routes incoming records to time-partitioned Shards               │
│  - cross-shard queries (FTS, vector, RCA, LDA, TextRank, …)         │
│  - additional non-shard stores: docstore, signals, scripts          │
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    ShardsCache (LRU)                          │  │
│  │  ┌───────────────────────────────────────────────────────┐    │  │
│  │  │                  Shard (one per time bucket)          │    │  │
│  │  │  ObservabilityStorage + FTSEngine + VectorEngine +    │    │  │
│  │  │  tplstorage (DocumentStorage)                         │    │  │
│  │  └───────────────────────────────────────────────────────┘    │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

The rest of this document walks each layer in detail.

---

## 2. The DuckDB foundation

### `StorageEngine` — the DuckDB connection pool

**File:** `src/storageengine.rs`

`StorageEngine` is the single low-level wrapper around DuckDB. Every
DuckDB-backed primitive in bdslib goes through it. It owns a
`r2d2::Pool<DuckdbConnectionManager>` so concurrent threads can
borrow connections without contention.

Key responsibilities:

- **Pool configuration.** `pool_size` is configurable per-engine
  (typically 4–8 connections). Pools larger than necessary waste file
  descriptors; smaller pools serialise concurrent access.
- **Schema initialisation.** A `&'static str` of `CREATE TABLE …
  CREATE INDEX …` is executed on construction, idempotently — so
  opening an existing database doesn't reinitialise it.
- **Type bridge.** DuckDB rows are converted to
  `rust_dynamic::value::Value` (a polymorphic value type used
  throughout the rest of the codebase). `cast_int`, `cast_string`,
  `cast_bin` extract concrete types.
- **Shared maintenance pool.** All DuckDB pools share a single
  `ScheduledThreadPool` (configured by the top-level `pool_size` /
  `r2d2_thread_pool_size` config) so the per-pool background
  maintenance thread count stays bounded — important when 50+ shards
  are open at once.

API surface:

```rust
pub fn new(path: &str, init_sql: &str, pool_size: u32) -> Result<Self>;
pub fn execute(&self, sql: &str) -> Result<()>;
pub fn execute_many(&self, stmts: &[String]) -> Result<()>;     // single transaction
pub fn select_all(&self, sql: &str) -> Result<Vec<Vec<Value>>>;
pub fn select_foreach<F>(&self, sql: &str, callback: F) -> Result<()>;
pub fn sync(&self) -> Result<()>;                                // CHECKPOINT
```

### Why DuckDB?

bdslib uses DuckDB rather than SQLite or a key-value store for three
reasons:

- **Columnar.** Telemetry queries are overwhelmingly time-range scans,
  often with aggregations — DuckDB's columnar layout makes
  `SELECT count(*) FROM telemetry WHERE ts BETWEEN ? AND ?` orders of
  magnitude faster than the same query in row-store engines.
- **Embedded JSON.** The `JSON` column type is first-class:
  `json_extract`, `json_keys`, and so on are usable in SQL without
  parsing strings on the application side.
- **Single-file, no daemon.** Each shard is one file (`obs.db`), no
  network layer, no separate process. Backup is `cp`.
- **Modern feature set.** Window functions, CTEs, BLOB columns,
  `from_hex` / `to_hex` for binary, type inference. The full SQL
  surface bdslib needs is in one engine.

---

## 3. Primitive stores

### `BlobStorage` — keyed binary blobs

**File:** `src/datastorage.rs`

A simple keyed binary-blob store. Schema:

```sql
CREATE TABLE blobs (
    id         TEXT   NOT NULL PRIMARY KEY,    -- UUIDv7 or arbitrary string
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    data       BLOB   NOT NULL
);
```

Operations: `add_blob` / `get_blob` / `update_blob` / `drop_blob`,
plus string-key variants. `created_at` is set once; `updated_at` is
refreshed on every update.

**Used by**: `DocumentStorage` (raw document content), the script
store (BUND source code), and several internal caches.

### `JsonStorage` — keyed JSON documents

**File:** `src/datastorage.rs`

A keyed JSON document store. Schema:

```sql
CREATE TABLE json_docs (
    id         TEXT   NOT NULL PRIMARY KEY,     -- UUIDv7
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    key        TEXT   NOT NULL,                 -- logical key extracted from doc
    document   JSON   NOT NULL                  -- DuckDB JSON column
);
CREATE INDEX idx_json_docs_key ON json_docs (key);
```

`add_json` is **upsert-by-logical-key**: if a document with the same
extracted key already exists it is updated in-place and the original
UUID is returned, preserving `created_at`. The logical key is
extracted via dot-notation path (`config.key_field`), with a
configurable fallback when extraction fails.

**Used by**: `DocumentStorage` (metadata), every store that wants
"latest version of this thing under this name" semantics.

### `FrequencyTracking` — observation rate log

**File:** `src/frequencytrackingstorage.rs`

A simple `(timestamp, id)` observation store. Same `id` recorded many
times means the rate is `count / window`. Schema:

```sql
CREATE TABLE observations (
    ts BIGINT NOT NULL,                         -- Unix seconds
    id TEXT   NOT NULL                          -- arbitrary label
);
CREATE INDEX idx_obs_ts ON observations (ts);
CREATE INDEX idx_obs_id ON observations (id);
```

Operations:

- `add(id)` / `add_with_timestamp(ts, id)` — record an observation.
- `by_id(id)` — every timestamp this id was observed at.
- `by_timestamp(ts)` — every distinct id seen at exactly this second.
- `time_range(start, end)` — distinct ids in `[start, end]`.
- `recent(duration)` — distinct ids in the lookback window.

**Used by**: `DocumentStorage` (every doc add records a frequency
observation, so per-doc cadence analysis is free), the per-shard
template store (drain3 template firing rates), the signal store.

### `ShardInfoEngine` — shard catalog

**File:** `src/shardsinfo.rs`

The catalog of every shard the system knows about. Schema:

```sql
CREATE TABLE shards (
    shard_id TEXT   NOT NULL PRIMARY KEY,       -- UUIDv7
    path     TEXT   NOT NULL,                   -- filesystem location
    start_ts BIGINT NOT NULL,                   -- Unix seconds, inclusive
    end_ts   BIGINT NOT NULL                    -- Unix seconds, exclusive
);
CREATE INDEX idx_shards_start_ts ON shards (start_ts);
CREATE INDEX idx_shards_end_ts   ON shards (end_ts);
```

Each shard covers `[start_time, end_time)`. Catalog entries are added
when a new time bucket needs a shard; they're removed only by an
explicit drop call. This is the source of truth for "what shards
exist on disk" — even if the in-memory cache is empty.

**Used by**: `ShardsCache` (catalog lookup before opening from disk).

---

## 4. Search engines

### `FTSEngine` — Tantivy full-text search

**File:** `src/fts.rs`

A thin wrapper around a Tantivy index, with two fields per document:

- `id` — UUIDv7, stored as `STRING`.
- `body` — full-text indexed `TEXT`.

Operations: `add_document(id, body)`, `drop_document(id)`,
`search(query, limit)`, `search_scored(query, limit)` — the last
returns BM25 scores so callers can merge across shards.

Important properties:

- **Lazy initialisation.** The Tantivy index isn't opened on
  construction — only on the first read or write. Shards opened for
  pure DuckDB queries (e.g., a `count(*)` time-range query) pay no
  Tantivy startup cost.
- **Immediate consistency.** Every write is followed by `commit()` +
  reader reload, so a search immediately after an insert finds the
  new document. (Tantivy's "writer heap" is 50 MB — large enough that
  bursts don't trigger mid-batch flushes.)
- **Primary records only.** Per the [primary/secondary
  algorithm](Algorithm/PRIMARY_SECONDARY.md), the FTS index never sees
  secondary records.

### `VectorEngine` — HNSW vector store

**File:** `src/vectorengine.rs`

A `vecstore::VecStore`-backed HNSW index, optionally paired with an
`EmbeddingEngine` for automatic text→vector conversion.

Operations: `store_vector(id, vec, metadata)`,
`store_document(id, json_value)` (auto-embeds the JSON fingerprint),
`search(query_vec, limit)`, `search_json(query, limit)`,
`delete_vector(id)`. The `with_embedding` constructor enables the
automatic-embedding paths.

Important properties:

- **Lazy initialisation.** Same as FTS — the binary HNSW index isn't
  deserialised until first use.
- **Reranking.** The `search_*` family supports `MMR` (Maximum
  Marginal Relevance) and custom rerankers — important when the raw
  HNSW results have many near-duplicates that should be diversified.
- **Primary records only** (when used inside a `Shard`).
- **Document-store mode.** When backing `DocumentStorage`, two
  vector entries per document share the same UUID prefix:
  `"{uuid}:meta"` (metadata fingerprint) and `"{uuid}:content"`
  (content text). One search hits both signals.

### `EmbeddingEngine` — fastembed text→vector

**File:** `src/embedding.rs`

The text-embedding model wrapper. Defaults to `AllMiniLML6V2` from
`fastembed`, producing 384-dim cosine-friendly vectors. Used by:

- `ObservabilityStorage` for primary/secondary classification.
- `VectorEngine` (when `with_embedding` is used) for automatic
  document embedding.
- The cross-shard semantic search in `ShardsManager`.

Single-instance per process: cloning is cheap (`Arc`-backed) and
shared across all stores.

---

## 5. Composite stores

### `ObservabilityStorage` — telemetry + dedup + primary/secondary

**File:** `src/observability.rs`

The most consequential store in bdslib. Holds every telemetry event,
deduplicates exact matches, and classifies new patterns as primary or
secondary. Tables:

- `telemetry` — every accepted record (primary or secondary).
- `primary_embeddings` — one row per primary, holding the 384-dim
  embedding used for downstream classification.
- `primary_secondary` — link table from primary UUID to secondary UUID.
- `dedup_tracking` — per-pattern timestamp log of byte-identical
  duplicates.

The full classification algorithm has its own document:
[`Documentation/Algorithm/PRIMARY_SECONDARY.md`](Algorithm/PRIMARY_SECONDARY.md).

Lives inside every `Shard`.

### `DocumentStorage` — metadata + blob + vector + frequency

**File:** `src/documentstorage.rs`

The general-purpose "I want to store some structured stuff with a
search index" type. One UUIDv7 identifies every document; each
document has four storage facets:

| Facet | Backed by | Stores |
|---|---|---|
| Metadata | `JsonStorage` (`metadata.db`) | The `metadata: JsonValue` field |
| Content | `BlobStorage` (`blobs.db`) | The raw bytes of the document |
| Vector | `VectorEngine` (`vectors/`) | Two HNSW entries: `"{uuid}:meta"` (metadata fingerprint) and `"{uuid}:content"` (content text) |
| Frequency | `FrequencyTracking` (`frequency.db`) | One observation per add |

Operations: `add_document(metadata, content)`, `update_metadata`,
`update_content`, `get_metadata`, `get_content`, `delete_document`,
`search_document` (semantic), and the file-chunking ingester
`add_document_from_file` for splitting large text files into
context-window-sized chunks.

**Used everywhere** the system needs "stored stuff with a search
index". The `ShardsManager` holds three `DocumentStorage` instances
side-by-side:

| Instance | Purpose | Path |
|---|---|---|
| `docstore` | User-managed knowledge base, RAG context | `{dbpath}/docstore` |
| `signals` | Named severity events (`v2/signal.emit`) | `{dbpath}/signals` |
| `scripts` | Stored BUND scripts with cron schedules | `{dbpath}/scripts` |

…plus one `tplstorage` per shard for drain3-mined templates
(`{shard_path}/tplstorage`).

---

## 6. The shard layer

### `Shard` — one time bucket

**File:** `src/shard.rs`

A `Shard` is **one self-contained time bucket** of telemetry data.
Each shard covers a half-open interval `[start_time, end_time)` aligned
to `shard_duration` boundaries (default 1 day).

Composition:

```
Shard {
    observability:  ObservabilityStorage,   // {path}/obs.db
    fts:            Arc<FTSEngine>,         // {path}/fts/
    vector:         VectorEngine,           // {path}/vec/
    tplstorage:     DocumentStorage,        // {path}/tplstorage/
}
```

The four stores share **the same UUID namespace**: a record stored in
`observability` with UUID `0192…` will also appear in `fts` and
`vector` under the same UUID (if it became a primary). Deletion via
`Shard::delete(id)` removes it from all four atomically (per-store —
not transactional across stores, but always best-effort).

### `ShardsCache` — LRU pool of open shards

**File:** `src/shardscache.rs`

An in-memory cache of open `Shard` instances, keyed by
`(start_time, end_time)`. Lookup order:

1. **Cache hit** — O(1) return.
2. **Catalog hit** — query `ShardInfoEngine` for a shard whose
   interval contains the requested timestamp. Open from the recorded
   path; insert into cache.
3. **Auto-create** — if neither hit, provision a new shard at
   `{root_path}/{start_ts}_{end_ts}/`, register in the catalog, open,
   cache.

When the cache grows beyond `max_open_shards` (default 16), the
least-recently-used shard is `sync()`'d to disk and dropped to reclaim
file descriptors. This is critical when ingesting historical data
that spans hundreds of shard intervals — without LRU eviction every
shard's connection pool would stay open and exhaust the file-descriptor
limit.

### `ShardsManager` — top-level orchestrator

**File:** `src/shardsmanager.rs` (plus the
`shardsmanager_*.rs` mixins)

The user-facing entry point. Loaded from an hjson config file:

```hjson
{
  dbpath: "/var/lib/bdslib"
  shard_duration: "1day"
  pool_size: 4
  similarity_threshold: 0.85
  drain_enabled: true
  drain_load_duration: "7days"
  jsoncache_capacity: 10000
  jsoncache_ttl_secs: 300
  max_open_shards: 16
}
```

What it owns:

| Component | Path | Role |
|---|---|---|
| `cache: ShardsCache` | `{dbpath}/{start}_{end}/` per shard | Time-partitioned telemetry |
| `docstore: DocumentStorage` | `{dbpath}/docstore` | RAG document knowledge base |
| `signals: DocumentStorage` | `{dbpath}/signals` | Named severity events |
| `scripts: DocumentStorage` | `{dbpath}/scripts` | Stored BUND scripts |
| `drain: Option<Arc<Mutex<DrainParser>>>` | none (in-memory) | Drain3 template miner |
| `drain_cluster_map` | none (in-memory) | Drain cluster ID → tplstorage UUID |
| `jsoncache: JsonCache` | none (in-memory) | LRU cache of recent records |

Public API surface (selected):

- **Writes**: `add(doc)`, `add_batch(docs)`, `update(id, doc)`,
  `delete_by_id(id)` — route by `timestamp` to the right shard.
- **Search**: `search_fts`, `search_vector`, `vectorsearch`,
  `vectorsearch_recent` — fan out to every relevant shard, merge,
  return.
- **Inventory**: `keys`, `keys_all`, `primaries`, `primaries_explore`,
  `primaries_get`, `primaries_get_telemetry`, `duplicates` — windowed
  metadata queries.
- **Analysis** — extension methods in `shardsmanager_*.rs` mixins:
  `summary_for_recent`, `summary_for_query`, `summary_lsa_for_recent`,
  `summary_lsa_for_query`, `textrank_templates`, `template_by_id`,
  `templates_recent`, `aggregationsearch`, …
- **Signal store**: `signal_emit`, `signal_update`, `signals`,
  `signals_query`.
- **Script store**: `script_add`, `scripts`, `script`,
  `update_script`, `script_delete`.

This is the type that is wrapped in `globals::get_db()` and shared
across every JSON-RPC handler in `bdsnode`.

---

## 7. The on-disk filesystem layout

A typical bdslib data directory after some activity:

```
/var/lib/bdslib/
├── shards_info.db                         ← ShardInfoEngine catalog
│
├── 1745000000_1745086400/                 ← Shard for one day
│   ├── obs.db                             ← ObservabilityStorage
│   ├── fts/                               ← Tantivy index files
│   │   ├── meta.json
│   │   └── *.term, *.idx, *.pos, …
│   ├── vec/                               ← VecStore HNSW index files
│   │   └── *.bin, *.meta
│   └── tplstorage/                        ← Per-shard drain3 templates
│       ├── metadata.db
│       ├── blobs.db
│       ├── vectors/
│       └── frequency.db
│
├── 1745086400_1745172800/                 ← Next day's shard
│   └── … (same layout as above)
│
├── docstore/                              ← User RAG knowledge base
│   ├── metadata.db
│   ├── blobs.db
│   ├── vectors/
│   └── frequency.db
│
├── signals/                               ← v2/signal.emit store
│   ├── metadata.db
│   ├── blobs.db
│   ├── vectors/
│   └── frequency.db
│
└── scripts/                               ← Stored BUND scripts
    ├── metadata.db
    ├── blobs.db
    ├── vectors/
    └── frequency.db
```

Every `*.db` file is a DuckDB database. Every `vectors/` directory is
a VecStore HNSW index. Every `fts/` directory is a Tantivy index. The
top-level `shards_info.db` and the per-shard `obs.db` are
schema-different DuckDB databases — they do not share rows.

---

## 8. What gets written when, and why

A single record arriving via JSON-RPC traverses up to four stores.
Tracing through `ShardsManager::add(doc)`:

```
1. Record { key, data, timestamp, … } arrives.
2. ShardsManager extracts `timestamp`, picks the right Shard from
   ShardsCache (open or auto-create).
3. (Optional, if drain_enabled)
   DrainParser.parse_json_with_callback(doc, |meta, body| {
       shard.tplstorage.add_document(meta, body)   ← TEMPLATE STORED
   });
4. Shard.add(doc) →
   ObservabilityStorage.add(doc):
       - Validate, build data_text, dedup-check.
       - If duplicate: append to dedup_tracking. STOP.
       - Else embed, classify primary/secondary.
       - Insert into telemetry  ← ROW STORED
       - If primary: insert into primary_embeddings  ← EMBEDDING STORED
                     return (uuid, true, Some(emb))
         If secondary: insert into primary_secondary  ← LINK STORED
                       return (uuid, false, None)
5. (Only if step 4 returned is_primary=true)
   Shard.fts.add_document(uuid, body)              ← FTS STORED
   Shard.vector.store_document(uuid, doc)          ← VECTOR STORED
6. ShardsManager.jsoncache.insert(uuid, ts, doc)   ← LRU CACHE
```

So a primary record touches: 1 DuckDB `telemetry` row + 1 DuckDB
`primary_embeddings` row + 1 Tantivy doc + 2 vector entries (meta +
content) + 1 in-memory cache entry. Plus, if drain is enabled, a
`tplstorage` document containing the inferred template body.

A secondary record touches: 1 DuckDB `telemetry` row + 1 DuckDB
`primary_secondary` row + 1 in-memory cache entry. **No FTS, no
vector, no embedding work.** This is the ~5,000× compression that
makes bdslib viable on noisy log streams (see
[PRIMARY_SECONDARY.md § 9C](Algorithm/PRIMARY_SECONDARY.md)).

A duplicate record touches: 1 JSON UPDATE on `dedup_tracking` (and
nothing else). The fastest possible "we've seen this already" path.

---

## 9. Threading, pooling, and persistence

### Thread safety

Every storage type in bdslib is `Clone + Send + Sync`. Cloning is
cheap — internal state is `Arc`-backed and shared:

- DuckDB pools are `r2d2::Pool` instances; `Pool: Clone` shares the
  pool.
- Tantivy `IndexWriter` is wrapped in `parking_lot::Mutex`.
- `VecStore` is wrapped in `parking_lot::Mutex<Option<VecStore>>` for
  lazy initialisation.

Concurrent reads always work. Concurrent writes work for
`ObservabilityStorage` (the primary/secondary classifier holds a
single lock on the in-memory primary cache during classification, so
no two threads can both classify the same near-duplicate as primary).
Tantivy and VecStore serialise writes internally.

### Pooling

| Pool | Where configured | Default | Effect |
|---|---|---|---|
| Per-store DuckDB pool | `pool_size` config | 4 | Concurrent connection count per `StorageEngine` |
| Shared r2d2 maintenance | `r2d2_thread_pool_size` config | 3 | Background threads for *all* DuckDB pools |
| `ShardsCache` LRU | `max_open_shards` config | 16 | Maximum simultaneously-open shards |

Pool sizing rule of thumb: `pool_size` should match your
ingestion-thread count, not your read-thread count (DuckDB releases
read locks aggressively). The LRU bound exists to protect against
file-descriptor exhaustion during historical backfills.

### Persistence

DuckDB writes are durable on COMMIT, but uses a write-ahead log (WAL)
for ongoing transactions. To force a checkpoint, call
`StorageEngine::sync()` (which executes `CHECKPOINT`).

bdslib calls `sync` in three places:

- **Periodic background sync** — `bdsnode` spawns a tokio task on
  startup that calls `bdslib::sync_db()` every `sync_interval_secs`
  (default 60). The task iterates every open shard and runs DuckDB
  CHECKPOINT, Tantivy commit, VecStore flush, and tplstorage HNSW
  save. Bounds WAL recovery time after an unclean exit. Set
  `sync_interval_secs: 0` in `bds.hjson` to disable.
- **LRU shard eviction** — before dropping a shard from the cache.
- **Process shutdown** — `sync_db()` is called from `bdsnode`'s
  shutdown handler before the process exits.

For unclean shutdowns (process killed without `sync_db`), DuckDB
recovers from the WAL on next open. No data is lost; the recovery
just takes a fraction of a second longer. With the periodic sync
running every 60 s the WAL stays bounded — without it, the active
shard's WAL could grow for hours between LRU evictions.

Tantivy writes are durable after the explicit `commit()` that
follows every `add_document` / `drop_document`. VecStore writes are
durable after the per-store flush, which happens automatically.

### Ingest backpressure

`bdsnode`'s `v2/add` / `v2/add.batch` / `v2/add.file` /
`v2/add.file.syslog` handlers push records onto named crossbeam
channels (`ingest`, `ingest_file`, `ingest_file_syslog`) drained by
background threads. The channels are **bounded** by
`ingest_channel_capacity` (default 100000); when at capacity the
handler returns JSON-RPC error `-32099` ("ingest channel
overloaded") so clients can apply backoff + retry rather than the
server OOMing on an unbounded queue. Set
`ingest_channel_capacity: 0` to revert to the legacy unbounded
behaviour.

Inside the `bds-add` thread, records are batched up to
`pipe_batch_size` (default 500) before flushing to
`ShardsManager::add_batch`, or whenever `pipe_timeout_ms` (default
500) of idle elapses. This trades sub-second visibility for any
single record against amortising the Tantivy commit / DuckDB
transaction / ONNX embedding cost across hundreds of records per
flush — the dominant perf optimisation on the ingest path.

---

## 10. Operational notes

### Backups

Every shard directory and every top-level `docstore`/`signals`/`scripts`
directory is fully self-contained — copy the directory, you have a
backup. The shard catalog (`shards_info.db`) records absolute paths,
so if you restore to a different filesystem location you must also
update the `path` column to match (or, equivalently, rewrite the
catalog from a directory walk).

### Wipe

`bdsnode --new` removes the configured `dbpath` directory tree before
opening the database. There is no "drop a single shard" API beyond
`std::fs::remove_dir_all` on the shard directory followed by deleting
the catalog entry — typically not needed; let the LRU naturally evict
old shards.

### Choosing the embedding model

The fastembed `EmbeddingModel` variant is selected via two `bds.hjson`
keys, both optional:

```hjson
embedding_model:     "AllMiniLML6V2"          // default
embedding_cache_dir: "/var/lib/bdslib/models" // optional, fastembed default otherwise
```

The resolved name appears in `v2/status` and on the bdsweb Dashboard
so operators can confirm which model is loaded.

**Pick the model at deployment time.** The HNSW vector index dimension
is fixed at first vector insert, so changing `embedding_model` later
will break vector search on the existing data. To switch models,
rebuild the dbpath: `bdsnode --new --config bds.hjson`. The full
explanation of the dimension-lock-in constraint and the list of common
variants live in
[`EMBEDDINGENGINE.md`](EMBEDDINGENGINE.md#configuring-the-model-via-bdshjson).

### Sizing

Rough storage cost per primary record:

- `telemetry` row: ~200 bytes (UUID, timestamps, key, JSON metadata,
  is_primary).
- `primary_embeddings` row: ~1.5 KB (UUID + 384 × 4-byte floats).
- Tantivy index: ~40 bytes per token, amortised across all docs.
- VecStore HNSW: ~3 KB per primary (graph layer, payload, two
  entries).

Per secondary record: ~200 bytes (`telemetry` row only — no
embedding, no FTS, no vector).

So a million primaries plus ten million secondaries is roughly
`(200 + 1500 + 3000) × 10⁶ + 200 × 10⁷ ≈ 6.7 GB` of structured data
plus the Tantivy index. In practice indices compress well; expect
the on-disk total to be ~50% of the in-memory peak.

### Where to read more

- [`Documentation/Algorithm/PRIMARY_SECONDARY.md`](Algorithm/PRIMARY_SECONDARY.md)
  — the deduplication algorithm, in detail, including its impact on
  storage costs.
- [`Documentation/Algorithm/README.md`](Algorithm/README.md) — every
  query-time analysis algorithm and how it consumes the stored data.
- [`Documentation/STORAGEENGINE.md`](STORAGEENGINE.md) — the DuckDB
  wrapper's full API surface.
- [`Documentation/SHARD.md`](SHARD.md),
  [`Documentation/SHARDSCACHE.md`](SHARDSCACHE.md),
  [`Documentation/SHARDSMANAGER.md`](SHARDSMANAGER.md) — per-component
  reference docs.
- [`Documentation/DOCUMENTSENGINE.md`](DOCUMENTSENGINE.md) — the
  composite document store's full API.
- [`Documentation/OBSERVABILITYENGINE.md`](OBSERVABILITYENGINE.md) —
  per-method reference for `ObservabilityStorage`.
- `src/storageengine.rs`, `src/datastorage.rs`,
  `src/observability.rs`, `src/documentstorage.rs`, `src/shard.rs`,
  `src/shardscache.rs`, `src/shardsmanager.rs` — the implementations
  themselves, ~3500 lines total.
