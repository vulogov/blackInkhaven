![bdslib logo](logo.jpeg)

# bdslib — BUND Data Storage

A Rust library (Edition 2024) for multifunctional programmatic data storage.
bdslib combines time-series telemetry, full-text and semantic search, log
analysis, extractive text summarisation, root cause analysis, a document
knowledge base, statistical trend analysis, and a stack-based scripting
runtime into a single cohesive system backed by DuckDB.

---

## Capabilities

### Storage

| Capability | Description |
|---|---|
| **Time-series shards** | DuckDB partitioned by configurable time windows; LRU shard cache; R2D2 connection pool |
| **Observability records** | Primary / secondary record model with redb-backed deduplication fingerprinting |
| **Document knowledge base** | Metadata (JSON) + raw content (BLOB) + per-document HNSW vector index; chunked file ingestion |
| **Frequency tracking** | `(timestamp, id)` observation store for event-rate analysis over time |
| **Signal store** | Named severity signals with arbitrary metadata and semantic search |
| **Result queues** | Per-id FIFO queues of `rust_dynamic` values with TTL eviction; backs async BUND job results |

### Search

| Capability | Description |
|---|---|
| **Semantic vector search** | fastembed AllMiniLML6V2 embeddings stored in per-shard HNSW indexes (VecStore) |
| **Full-text search** | Tantivy BM25 index per shard |
| **Aggregation search** | Single call combining cross-shard vector search over telemetry + semantic document store search |

### Log analysis

| Capability | Description |
|---|---|
| **Syslog ingestion** | RFC 3164 parser — timestamp, host, facility, severity, message; bulk file ingest |
| **Drain3 template mining** | Prefix-tree log clustering into drain3 templates; per-shard template store with HNSW search |
| **LDA topic modelling** | Latent Dirichlet Allocation over a key's corpus; per-key and all-keys variants |

### Extractive summarisation

| Capability | Description |
|---|---|
| **TextRank** | PageRank over pairwise cosine similarity; summarises sentences, log lines, or JSON fingerprints |
| **LSA** | Latent Semantic Analysis (Steinberger-Ježek 2004): TF-IDF → centred Gram → truncated SVD → concept-space scoring |
| **Template TextRank** | TextRank over drain3 template bodies observed in a time window |
| **Primary TextRank** | TextRank over primary record text bodies (`data["value"]` / `data["raw"]`), skipping numeric measurements |
| **Primary LSA** | LSA variant of primary summarisation — same body extraction rule, SVD-based ranking |

### Statistical analysis & RCA

| Capability | Description |
|---|---|
| **Telemetry trends** | Min, max, mean, median, std-dev, S-H-ESD anomaly detection, breakout detection |
| **Root cause analysis** | G-Forest co-occurrence clustering over non-telemetry events; causal ranking by lead time |
| **Template RCA** | RCA on drain3 template observations — cluster template bodies by co-occurrence |

### Scripting runtime

| Capability | Description |
|---|---|
| **BUND VM** | Stack-based scripting language with full stdlib; stateful named contexts; `v2/eval` RPC |
| **BUND worker pool** | Process-wide pool of threads each running an independent Bund VM; jobs submitted via crossbeam MPMC channel; results written to global result queues |
| **Async eval** | `v2/eval.queued` — submit a BUND script, get a UUIDv7 job handle immediately, poll results via `v2/results.*` |

### AI integration

| Capability | Description |
|---|---|
| **Ollama chat (RAG)** | `v2/chat.ollama` — retrieval-augmented generation combining observability + document store context with a local Ollama model; stateful sessions |

---

## Components

```
┌─────────────────────────────────────────────────────────────────┐
│                         Applications                            │
│                                                                 │
│   bdscli              bdscmd                 bdsweb             │
│   local CLI           RPC client             web UI             │
│   (direct DB)         (all v2/* methods)      (HTMX / Tailwind) │
└────────────────────────────┬────────────────────────────────────┘
                             │  JSON-RPC 2.0 over HTTP
┌────────────────────────────▼────────────────────────────────────┐
│                           bdsnode                               │
│              JSON-RPC 2.0 server  ·  default port 9000          │
│              BundWorkerPool  ·  Ollama chat sessions            │
└────────────────────────────┬────────────────────────────────────┘
                             │  Rust API (in-process)
┌────────────────────────────▼────────────────────────────────────┐
│                           bdslib                                │
│                                                                 │
│  ShardsManager                                                  │
│    └─ Shard  (DuckDB · Tantivy FTS · VecStore HNSW · tplstore) │
│    └─ ShardsCache  (LRU open-shard pool)                        │
│    └─ TextRank / LSA summarisation                              │
│    └─ LDA · RCA · TelemetryTrend · Drain3                       │
│                                                                 │
│  DocumentStorage  (metadata · blob · HNSW)                      │
│  ObservabilityStorage  (redb dedup · secondaries)               │
│  FrequencyTracking  (event-rate observations)                   │
│  EmbeddingEngine  (fastembed AllMiniLML6V2)                     │
│  BUND VM  (stack-based scripting · worker pools · result queues)│
└─────────────────────────────────────────────────────────────────┘
```

### bdsnode — network daemon

Embeds bdslib and exposes all capabilities over JSON-RPC 2.0. Holds the
`ShardsManager` singleton, the document store, Ollama chat sessions, and a
configurable `BundWorkerPool`. All other tools talk exclusively to bdsnode.
→ [Documentation/jsonrpc_api/README.md](Documentation/jsonrpc_api/README.md)

### bdscli — local CLI

Operates directly on a DuckDB database file without a running server. Useful
for local exploration, one-off queries, and offline analysis.
→ [Documentation/BDSCLI.md](Documentation/BDSCLI.md)

### bdscmd — RPC command-line client

One subcommand per JSON-RPC method. Results are pretty-printed JSON;
`--raw` produces compact output for piping into `jq`. Supports shebang-based
BUND script execution.
→ [Documentation/BDSCMD.md](Documentation/BDSCMD.md)

### bdsweb — web interface

Dark-themed browser UI (HTMX + Tailwind) with grouped navigation and live
HTMX partial updates. No JavaScript framework required.
→ [Documentation/BDSWEB.md](Documentation/BDSWEB.md)

**Navigation groups:**

| Group | Pages |
|---|---|
| Dashboard | System snapshot: uptime, shard count, queue depth |
| **Telemetry** | Metrics, Logs, Templates |
| **Analysis** | Agg. Search, Trends, Templates Summary, Primary Summary, Primary Query Summary, Primary LSA Summary, Primary LSA Query Summary |
| Documents | Semantic document search |
| **RCA** | Telemetry RCA, Template RCA |
| Signals | Signal timeline and semantic search |
| Chat | Ollama RAG chat |
| Bund | Interactive BUND scripting workbench |

---

## Build

```bash
make all        # cargo build
make rebuild    # clean + build
make test       # cargo test -- --show-output
make clean      # clean artifacts and update deps
```

Run a single test:

```bash
cargo test test_storage_engine_full_lifecycle -- --show-output
```

---

## Quick Start

**1. Configure**

```hjson
// bds.hjson
{
  dbpath: "/var/lib/bdslib"
  shard_duration: "24h"
  pool_size: 8
  similarity_threshold: 0.85
  drain_enabled: true
  drain_load_duration: "7days"
  n_workers: 4           // BundWorkerPool threads
  ollama_model: "llama3.2"
}
```

**2. Start the server**

```bash
bdsnode --config bds.hjson
```

**3. Verify**

```bash
bdscmd status
```

**4. Ingest data**

```bash
# Single record
bdscmd add --key cpu.usage --data '{"value": 0.72}'

# Batch from NDJSON file
bdscmd add-file /path/to/records.ndjson

# Syslog file
bdscmd add-file-syslog /var/log/syslog
```

**5. Search and summarise**

```bash
# Semantic search
bdscmd search-get -q "high cpu memory pressure" --duration 1h

# TextRank summary of recent text records
bdscmd summary-for-recent --duration 1h

# LSA summary of records matching a query
bdscmd summary-lsa-for-query --query "nginx upstream timeout"
```

**6. Analyse**

```bash
# Statistical trends for a metric key
bdscmd trends --key cpu.usage --duration 6h

# Root cause analysis
bdscmd rca --key service.error --duration 1h

# LDA topics for a key's corpus
bdscmd topics --key log.app --duration 24h
```

**7. BUND scripting**

```bash
# Evaluate inline
bdscmd eval --script '2 2 + .'

# Run a script file (shebang supported)
bdscmd eval my_script.bund

# Async job — submit and poll
bdscmd eval-queued my_script.bund
# → { "id": "019f2a3b-..." }
bdscmd results-pull --id 019f2a3b-...
```

**8. Open the web UI**

```bash
bdsweb --node http://127.0.0.1:9000
# → http://127.0.0.1:8080
```

---

## JSON-RPC API Summary

All methods use JSON-RPC 2.0 over HTTP POST to `/`. Full reference:
[Documentation/jsonrpc_api/README.md](Documentation/jsonrpc_api/README.md)

| Group | Methods |
|---|---|
| **Ingestion** | `v2/add` · `v2/add.batch` · `v2/add.file` · `v2/add.file.syslog` |
| **Inventory** | `v2/status` · `v2/count` · `v2/timeline` · `v2/shards` |
| **Keys & records** | `v2/keys` · `v2/keys.all` · `v2/keys.get` · `v2/primaries` · `v2/primaries.explore` · `v2/primaries.explore.telemetry` · `v2/primaries.get` · `v2/primaries.get.telemetry` · `v2/primary` · `v2/secondaries` · `v2/secondary` · `v2/duplicates` |
| **Search** | `v2/fulltext` · `v2/fulltext.get` · `v2/fulltext.recent` · `v2/search` · `v2/search.get` · `v2/aggregationsearch` |
| **Analysis** | `v2/trends` · `v2/topics` · `v2/topics.all` · `v2/rca` · `v2/rca.templates` |
| **Summarisation** | `v2/textrank.templates` · `v2/summary_for_recent` · `v2/summary_for_query` · `v2/summary_lsa_for_recent` · `v2/summary_lsa_for_query` |
| **Templates** | `v2/tpl.add` · `v2/tpl.get` · `v2/tpl.list` · `v2/tpl.search` · `v2/tpl.update` · `v2/tpl.delete` · `v2/tpl.reindex` · `v2/tpl.template_by_id` · `v2/tpl.templates_by_timestamp` · `v2/tpl.templates_recent` |
| **Documents** | `v2/doc.add` · `v2/doc.add.file` · `v2/doc.get` · `v2/doc.get.metadata` · `v2/doc.get.content` · `v2/doc.update.metadata` · `v2/doc.update.content` · `v2/doc.delete` · `v2/doc.search` · `v2/doc.search.json` · `v2/doc.search.strings` · `v2/doc.reindex` |
| **Signals** | `v2/signal.emit` · `v2/signal.update` · `v2/signals` · `v2/signals_query` |
| **BUND VM** | `v2/eval` · `v2/eval.queued` |
| **Result queues** | `v2/results.len` · `v2/results.push` · `v2/results.pull` · `v2/results.empty` |
| **Chat** | `v2/chat.ollama` |

---

## Documentation

| Document | Description |
|---|---|
| [Documentation/README.md](Documentation/README.md) | Architecture, data flow, storage model, BUND overview, full doc index |
| [Documentation/BDSCLI.md](Documentation/BDSCLI.md) | `bdscli` local CLI — all subcommands |
| [Documentation/BDSCMD.md](Documentation/BDSCMD.md) | `bdscmd` RPC client — all subcommands and quick reference |
| [Documentation/BDSWEB.md](Documentation/BDSWEB.md) | `bdsweb` web interface — all pages, startup flags |
| [Documentation/SCRIPTS.md](Documentation/SCRIPTS.md) | Operational shell scripts for ingest, load testing, and pipeline verification |
| [Documentation/jsonrpc_api/README.md](Documentation/jsonrpc_api/README.md) | All `v2/*` JSON-RPC methods with parameters, response shapes, and examples |
| [Documentation/Bund/README.md](Documentation/Bund/README.md) | BUND VM overview, context lifecycle, integration guide |
| [Documentation/Bund/SYNTAX_AND_VM.md](Documentation/Bund/SYNTAX_AND_VM.md) | BUND language syntax and stack execution model |
| [Documentation/Bund/BASIC_LIBRARY.md](Documentation/Bund/BASIC_LIBRARY.md) | BUND built-in word reference |
| [Documentation/examples/README.md](Documentation/examples/README.md) | 10 BUND tutorials + Rust API demos for every subsystem |
| [Documentation/tests/README.md](Documentation/tests/README.md) | All integration test files — what each covers |

### Library internals

| Document | Description |
|---|---|
| [Documentation/STORAGEENGINE.md](Documentation/STORAGEENGINE.md) | `StorageEngine` — DuckDB core with R2D2 connection pool and `rust_dynamic` type bridge |
| [Documentation/SHARD.md](Documentation/SHARD.md) | `Shard` — single time-partition: telemetry table, FTS, vector, template store |
| [Documentation/SHARDSCACHE.md](Documentation/SHARDSCACHE.md) | `ShardsCache` — LRU open-shard pool with time-aligned interval keys |
| [Documentation/SHARDSMANAGER.md](Documentation/SHARDSMANAGER.md) | `ShardsManager` — shard lifecycle, ingest routing, cross-shard queries |
| [Documentation/EMBEDDINGENGINE.md](Documentation/EMBEDDINGENGINE.md) | `EmbeddingEngine` — fastembed vector generation |
| [Documentation/FTSENGINE.md](Documentation/FTSENGINE.md) | `FTSEngine` — Tantivy BM25 indexing |
| [Documentation/VECTORENGINE.md](Documentation/VECTORENGINE.md) | `VectorEngine` — HNSW index via VecStore |
| [Documentation/DOCUMENTSENGINE.md](Documentation/DOCUMENTSENGINE.md) | `DocumentStorage` — metadata, blob, and vector store |
| [Documentation/OBSERVABILITYENGINE.md](Documentation/OBSERVABILITYENGINE.md) | `ObservabilityStorage` — redb dedup and secondary records |
| [Documentation/COMMON.md](Documentation/COMMON.md) | Shared utilities: errors, JSON fingerprint, time ranges, UUID |

---

## License

See [LICENSE](LICENSE).
