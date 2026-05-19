# bdslib — BUND Data Storage

**bdslib** is a Rust library for multifunctional programmatic
data storage. It provides a unified system for ingesting, indexing, searching,
and analysing time-series telemetry, structured logs, and knowledge-base
documents. The library ships with a network daemon, two command-line clients,
and a web interface — all communicating over a single JSON-RPC 2.0 API.

---

## What It Does

| Capability | How |
|------------|-----|
| Time-series storage | DuckDB shards partitioned by time, with R2D2 connection pooling |
| Semantic search | Vector embeddings (fastembed + HNSW) for natural-language queries |
| Full-text search | Tantivy BM25 index per shard |
| Log analysis | RFC 3164 syslog parser, deduplication, LDA topic modelling |
| Document knowledge base | Metadata + blob store with per-document vector indexing |
| Statistical analysis | Trend detection, anomaly and breakout identification |
| Root cause analysis | Co-occurrence clustering and causal event ranking |
| Scripting | BUND stack-based VM with stdlib, accessible over the network |

---

## Components

```
┌───────────────────────────────────────────────────────┐
│                      Applications                     │
│                                                       │
│   bdscli          bdscmd              bdsweb          │
│   local CLI       RPC client          web UI          │
│   (direct DB)     (all v2/* methods)  (7 pages)       │
└──────────────────────────┬────────────────────────────┘
                           │  JSON-RPC 2.0 / HTTP
┌──────────────────────────▼────────────────────────────┐
│                        bdsnode                        │
│             JSON-RPC server  ·  port 9000             │
└──────────────────────────┬────────────────────────────┘
                           │  Rust API (in-process)
┌──────────────────────────▼────────────────────────────┐
│                        bdslib                         │
│                                                       │
│  ShardsManager                                        │
│    └─ Shard  (DuckDB · Tantivy FTS · VecStore HNSW)  │
│    └─ ShardsCache  (LRU open-shard pool)              │
│                                                       │
│  DocumentStorage  (metadata · blob · vector)          │
│  ObservabilityStorage  (redb dedup + secondaries)     │
│  EmbeddingEngine  (fastembed)                         │
│  BUND VM  (stack-based scripting runtime)             │
└───────────────────────────────────────────────────────┘
```

### bdsnode — network daemon

Embeds bdslib and serves all capabilities over JSON-RPC 2.0. A single long-
running process holds the `ShardsManager` singleton, the document store, and
a pool of named BUND VM contexts. All other tools talk exclusively to bdsnode.

### bdscli — local CLI

Operates directly on a DuckDB database file without a running server. Useful
for local exploration, one-off queries, and offline analysis.
→ [BDSCLI.md](BDSCLI.md)

### bdscmd — RPC command-line client

One subcommand per JSON-RPC method. Results are pretty-printed JSON;
`--raw` produces compact output for piping into `jq`. Supports shebang-based
BUND script execution.
→ [BDSCMD.md](BDSCMD.md)

### bdsweb — web interface

Dark-themed browser UI with seven pages: system dashboard, telemetry search,
log search with topic cloud, document retrieval, aggregated search,
time-series trend analysis, and an interactive BUND scripting workbench.
→ [BDSWEB.md](BDSWEB.md)

---

## How Data Flows

```
External source
      │
      ▼
  Ingest (v2/add, v2/add.batch, v2/add.file, v2/add.file.syslog)
      │
      ├─ DuckDB telemetry table  (primary records)
      ├─ Tantivy FTS index       (full-text search)
      ├─ VecStore HNSW index     (semantic search)
      └─ redb ObservabilityDB   (dedup fingerprints · secondary records)

Document ingest (v2/doc.add, v2/doc.add.file)
      │
      ├─ JSON metadata store (DuckDB)
      ├─ Blob store (DuckDB BLOB)
      └─ Per-document HNSW vector (metadata + content embedded separately)
```

Queries can target a single shard (current time window) or span all shards.
Cross-shard methods have a `duration` parameter (`15min` … `7days`).

---

## Storage Model

bdslib organises telemetry into **shards** — time-partitioned database
segments. Each shard is an independent DuckDB file with its own FTS and
vector indexes. `ShardsCache` keeps a configurable number of shards open via
an LRU cache. `ShardsManager` handles shard lifecycle, routes ingest to the
active shard, and fans out cross-shard queries.

Documents live outside the shard model in a dedicated `DocumentStorage` backed
by a single DuckDB database (metadata table + blob table) and a shared HNSW
index. The same composite type also backs the signal store and the script
store.

For the full layered architecture — every storage primitive, every search
engine, every composite store, what gets written when, the on-disk filesystem
layout, and operational notes on backup / sizing / pooling — see
[**DATABASE.md**](DATABASE.md).

### Ingest pipeline & durability

Two `bds.hjson` knobs govern the ingest-path durability/throughput trade-off:

- `sync_interval_secs` (default 60) — `bdsnode` runs a background sync
  task that calls `bdslib::sync_db()` on this cadence. Without it, a
  process killed without graceful shutdown loses every write since the
  last LRU shard eviction. Set to `0` to disable.
- `ingest_channel_capacity` (default 100000) — the `v2/add*` ingest
  channels are bounded so a producer flood can't OOM the server. When
  full, callers receive JSON-RPC `-32099` ("ingest channel
  overloaded") and should back off. Set to `0` for legacy unbounded
  behaviour.

The ingest thread defaults are tuned for batch throughput:
`pipe_batch_size = 500`, `pipe_timeout_ms = 500` — so high-volume
streams amortise the Tantivy / DuckDB / ONNX commit cost while sparse
single-record streams still flush within half a second.

For one-shot scripts that need the assigned UUID in the response,
`v2/add` accepts `"sync": true` to bypass the queue and return
`{ "id": "<uuidv7>", "synced": true }` after the record is persisted.

---

## BUND Scripting

BUND is a stack-based scripting language built into bdslib. Scripts are
evaluated by the `v2/eval` JSON-RPC method or through the `bdsweb` workbench.

```bund
// Compute and store result in workbench
2 2 + .          // → {"result": 4}

// String operation
"hello" string.upper .   // → {"result": "HELLO"}

// List processing
[ 1 2 3 ] dup len swap . // push list length to workbench
```

Named **contexts** accumulate VM state (defined words, stack contents) between
calls. A fresh context name gives a clean VM with only the stdlib loaded.
Contexts are evicted after a configurable idle timeout (default 300 s).

→ [Bund/README.md](Bund/README.md) · [Bund/SYNTAX_AND_VM.md](Bund/SYNTAX_AND_VM.md) · [Bund/BASIC_LIBRARY.md](Bund/BASIC_LIBRARY.md)

---

## Documentation Index

### Library internals

| Document | Description |
|----------|-------------|
| [DATABASE.md](DATABASE.md) | **Full storage architecture overview** — every store, what role it plays, on-disk layout, what gets written when, threading and pooling, operational notes |
| [STORAGEENGINE.md](STORAGEENGINE.md) | `StorageEngine` — DuckDB core with R2D2 connection pool |
| [SHARD.md](SHARD.md) | `Shard` — single time-partition: telemetry, FTS, vector |
| [SHARDSCACHE.md](SHARDSCACHE.md) | `ShardsCache` — LRU open-shard pool |
| [SHARDSMANAGER.md](SHARDSMANAGER.md) | `ShardsManager` — shard lifecycle, ingestion, cross-shard queries |
| [EMBEDDINGENGINE.md](EMBEDDINGENGINE.md) | `EmbeddingEngine` — fastembed vector generation |
| [FTSENGINE.md](FTSENGINE.md) | `FTSEngine` — Tantivy BM25 indexing |
| [VECTORENGINE.md](VECTORENGINE.md) | `VectorEngine` — HNSW index via VecStore |
| [DOCUMENTSENGINE.md](DOCUMENTSENGINE.md) | `DocumentStorage` — metadata, blob, and vector store |
| [OBSERVABILITYENGINE.md](OBSERVABILITYENGINE.md) | `ObservabilityStorage` — telemetry rows, dedup tracking, primary/secondary classification |
| [Algorithm/](Algorithm/README.md) | Deep-dive references for every analysis algorithm: TextRank, LSA, k-NN, RCA Jaccard, LDA, Primary/Secondary classification |
| [COMMON.md](COMMON.md) | Shared utilities: errors, JSON fingerprint, time ranges, UUID |

### Tools

| Document | Description |
|----------|-------------|
| [BDSCLI.md](BDSCLI.md) | `bdscli` — local CLI: init, generate, ingest, get, search, analyze |
| [BDSCMD.md](BDSCMD.md) | `bdscmd` — RPC client: all `v2/*` methods, eval shebang, quick reference |
| [BDSWEB.md](BDSWEB.md) | `bdsweb` — operator reference: route paths, startup flags, RPC calls behind every page |
| [BDS_UI.md](BDS_UI.md) | `bdsweb` — **user manual**: walks every UI page in task order, with controls explained, common workflows, and a troubleshooting table |

### JSON-RPC API

Full reference: [jsonrpc_api/README.md](jsonrpc_api/README.md)

**Ingestion** — `v2/add` · `v2/add.batch` · `v2/add.file` · `v2/add.file.syslog`

**Inventory** — `v2/status` · `v2/count` · `v2/timeline` · `v2/shards`

**Keys & records** — `v2/keys` · `v2/keys.all` · `v2/keys.get` · `v2/primaries` ·
`v2/primaries.get` · `v2/primaries.get.telemetry` · `v2/primaries.explore` ·
`v2/primaries.explore.telemetry` · `v2/primary` · `v2/secondaries` · `v2/secondary` ·
`v2/duplicates`

**Search** — `v2/search` · `v2/search.get` · `v2/fulltext` · `v2/fulltext.get` ·
`v2/fulltext.recent` · `v2/aggregationsearch`

**Analysis** — `v2/trends` · `v2/topics` · `v2/topics.all` · `v2/rca`

**Documents** — `v2/doc.add` · `v2/doc.add.file` · `v2/doc.get` · `v2/doc.get.metadata` ·
`v2/doc.get.content` · `v2/doc.update.metadata` · `v2/doc.update.content` ·
`v2/doc.delete` · `v2/doc.search` · `v2/doc.search.strings` · `v2/doc.search.json` ·
`v2/doc.reindex`

**BUND VM** — `v2/eval`

### BUND language

| Document | Description |
|----------|-------------|
| [Bund/README.md](Bund/README.md) | VM overview, context lifecycle, integration guide |
| [Bund/SYNTAX_AND_VM.md](Bund/SYNTAX_AND_VM.md) | Syntax, token types, stack execution model |
| [Bund/BASIC_LIBRARY.md](Bund/BASIC_LIBRARY.md) | Built-in word reference: stack, arithmetic, string, list, map, I/O |

### Scripts

[SCRIPTS.md](SCRIPTS.md) — operational shell scripts for data ingestion,
load testing, and end-to-end pipeline verification.

### Examples

[examples/README.md](examples/README.md) — 10 BUND VM tutorial scripts and
17 Rust API demos covering every major subsystem.

### Tests

[tests/README.md](tests/README.md) — 20 integration test files, each
isolated with its own in-memory or temporary database.

---

## Reference

| File | Description |
|------|-------------|
| [COMMANDS.txt](COMMANDS.txt) | Quick-reference command cheat sheet |
| [CURL.txt](CURL.txt) | `curl` one-liners for common JSON-RPC calls |
