# bdslib — Examples

Reference documentation for the files in the `examples/` directory. Each example is self-contained and runnable with `cargo run --example <name>`.

---

## BUND VM examples

Ten progressive tutorials for the BUND stack-based scripting VM. Run with:

```bash
bdscli bund examples/<name>.bund
```

| Example | File | Description |
|---|---|---|
| [Hello World](01_hello_world.md) | `01_hello_world.bund` | Push literals, print with `println` |
| [Arithmetic](02_arithmetic.md) | `02_arithmetic.bund` | Postfix math, `float.sqrt`, `float.Pi`, `*+` bulk sum |
| [Named Functions](03_named_functions.md) | `03_named_functions.bund` | `:name { body } register`, `alias`, recursion |
| [Conditionals](04_conditionals.md) | `04_conditionals.bund` | `if`, `if.false`, `ifthenelse`, boolean combinators |
| [Loops](05_loops.md) | `05_loops.bund` | `times`, `do`, `map`, `while`, `for`, fibonacci |
| [Lists](06_lists.md) | `06_lists.bund` | `car`/`cdr`, `head`/`tail`, `at`, `len`, `push`, `map`, recursive sum |
| [Strings](07_strings.md) | `07_strings.bund` | Case conversion, `wildmatch`, `regex`, `tokenize` |
| [Maps and Types](08_maps_and_types.md) | `08_maps_and_types.bund` | `set`/`get`/`has_key`, `type`, `convert.*` |
| [Stack and Workbench](09_stack_and_workbench.md) | `09_stack_and_workbench.bund` | Workbench (`.`), named stacks (`@name`), function pointers |
| [Full Program](10_full_program.md) | `10_full_program.bund` | Statistics tool combining all BUND features |

---

## Rust API examples

Run with:

```bash
cargo run --example <name>
```

### Storage layer

| Example | File | Description |
|---|---|---|
| [StorageEngine](storage_engine_demo.md) | `storage_engine_demo.rs` | Low-level DuckDB SQL engine with R2D2 pool and rust_dynamic type bridge |
| [DataStorage](datastorage_demo.md) | `datastorage_demo.rs` | `BlobStorage` and `JsonStorage` with key-based deduplication |
| [FrequencyTracking](frequencytracking_demo.md) | `frequencytracking_demo.rs` | `FrequencyTracking`: record `(timestamp, id)` observations; query by id, exact timestamp, time range, and humantime lookback duration |
| [DocumentStorage](documentstorage_demo.md) | `documentstorage_demo.rs` | `DocumentStorage`: metadata + blob + unified HNSW vector store — add, search, update, delete, string output, persistence |
| [LargeDocument](large_document_demo.md) | `large_document_demo.rs` | `add_document_from_file`: file chunking, overlap inspection, RAG context-window expansion, semantic chunk search via `EmbeddingEngine` |
| [ObservabilityStorage](observability_demo.md) | `observability_demo.rs` | redb-backed dedup, primary/secondary classification, time-range queries |

### Search engines

| Example | File | Description |
|---|---|---|
| [EmbeddingEngine](embedding_engine_demo.md) | `embedding_engine_demo.rs` | fastembed vector embeddings, cosine similarity, nearest-neighbour |
| [FTSEngine](fts_engine_demo.md) | `fts_engine_demo.rs` | Tantivy BM25 full-text search: add, query, drop, sync |
| [VectorEngine](vectorengine_demo.md) | `vectorengine_demo.rs` | HNSW vector storage, reranking (MMR, custom), JSON fingerprinting |

### Shard management

| Example | File | Description |
|---|---|---|
| [Shard](shard_demo.md) | `shard_demo.rs` | Single time-partition: telemetry table, FTS, vector search, delete |
| [ShardsCache](shardscache_demo.md) | `shardscache_demo.rs` | LRU shard cache, time-aligned buckets, cross-shard span queries |
| [ShardsManager](shardsmanager_demo.md) | `shardsmanager_demo.rs` | Config-driven top-level API: bulk ingest, cross-shard FTS and vector |
| [ShardsManager+DocumentStore](shardsmanager_documentstore.md) | `shardsmanager_documentstore.rs` | Telemetry + runbooks: RAG pattern combining shard FTS/vector search with semantic chunk retrieval and context-window expansion |
| [AggregationSearch](aggregationsearch_demo.md) | `aggregationsearch_demo.rs` | `aggregationsearch`: parallel vector search over time-scoped telemetry shards + semantic document store search in one call; duration-scoping behaviour; result structure |

### Analytics

| Example | File | Description |
|---|---|---|
| [TelemetryTrend](telemetrytrend_demo.md) | `telemetrytrend_demo.rs` | Statistics, S-H-ESD anomaly detection, breakout detection |
| [RCA](rca_demo.md) | `rca_demo.rs` | Co-occurrence clustering and causal ranking for root cause analysis |
| [RCATemplates](rca_templates_demo.md) | `rca_templates_demo.rs` | G-Forest RCA on drain3 template observations: cluster detection and causal ranking by template lead time |
| [TextRank](textrank_demo.md) | `textrank_demo.rs` | Extractive summarisation of sentence/log/JSON-fingerprint lists via cosine-similarity TextRank |
| [LSA](lsa_demo.md) | `lsa_demo.rs` | Extractive summarisation via Latent Semantic Analysis: TF-IDF → centred Gram → truncated SVD → Steinberger-Ježek scoring |
| [k-NN](knn_demo.md) | `knn_demo.rs` | k-Nearest-Neighbour intelligence: TF-IDF + cosine similarity → top-k neighbours → cluster discovery (union-find on k-NN graph) → anomaly detection (low top-1 similarity); structured JSON output |
| [N-gram](ngram_demo.md) | `ngram_demo.rs` | N-gram anomaly detection (`ngram_anomaly`) + noise removal (`ngram_remove_noise`): document-frequency over bigrams/trigrams → mean rarity / commonness per line → threshold cut; structured JSON with `anomalies` / `kept` / `removed` arrays |
| [PrimaryTextRank](primary_textrank_demo.md) | `primary_textrank_demo.rs` | `summary_for_recent` and `summary_for_query`: TextRank over primary observability records, skipping numeric measurements |
| [PrimaryLSA](lsa_primary_textrank_demo.md) | `lsa_primary_textrank_demo.rs` | `summary_lsa_for_recent` and `summary_lsa_for_query`: LSA over primary observability records, numeric exclusion, `n_concepts` and `max_sentences` knobs |
| [Scripts](shardsmanager_scripts_demo.md) | `shardsmanager_scripts_demo.rs` | `script_add` / `scripts` / `script` / `update_script` / `script_delete`: BUND script registry with `name` + `schedule` metadata validation |
| [ResultQueue](result_queue_demo.md) | `result_queue_demo.rs` | Per-id FIFO queues of `rust_dynamic` values with creation timestamps and TTL eviction; backs `v2/results.*` |
| [ShardsManagerTplFrequency](shardsmanager_tpl_frequency_demo.md) | `shardsmanager_tpl_frequency_demo.rs` | Drain3 template discovery and FrequencyTracking query API: `templates_recent`, `template_by_id`, `templates_by_timestamp` |

### BUND worker pools

| Example | File | Description |
|---|---|---|
| [WorkersDemo](workers_demo.md) | `workers_demo.rs` | `BundWorkerPool`: submit Bund scripts to a shared worker pool; poll results by UUIDv7 job handle; concurrent submissions |
| [EphemeralDemo](ephemeral_demo.md) | `ephemeral_demo.rs` | `WorkerPool` (ephemeral): per-job fresh Bund VM, zero cross-job state leakage; independent `EPHEMERAL_PIPE` channel |

### Data generation and globals

| Example | File | Description |
|---|---|---|
| [Generator](generator_demo.md) | `generator_demo.rs` | Synthetic telemetry, logs, mixed, and template-driven documents |
| [Globals](globals_demo.md) | `globals_demo.md` | Process-wide `ShardsManager` singleton: `init_db`, `get_db`, `sync_db` |
