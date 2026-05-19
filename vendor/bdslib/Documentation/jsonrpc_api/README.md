# bdsnode — JSON-RPC 2.0 API

`bdsnode` is the network-facing daemon for bdslib. It exposes a JSON-RPC 2.0 HTTP server backed by the shared `ShardsManager` singleton and the BUND VM runtime.

---

## Running bdsnode

```
bdsnode [OPTIONS]
```

### Options

| Flag | Env var | Default | Description |
|---|---|---|---|
| `-c, --config <PATH>` | `BDS_CONFIG` | — | Path to the hjson configuration file |
| `--host <HOST>` | — | `127.0.0.1` | Address to bind the JSON-RPC listener |
| `-p, --port <PORT>` | — | `9000` | TCP port for the JSON-RPC listener |
| `--new` | — | false | Delete the existing data store and start with a fresh database before binding the listener |

### Example

```bash
# use a config file
bdsnode --config /etc/bdslib/config.hjson --host 0.0.0.0 --port 9944

# rely on environment variable
BDS_CONFIG=/etc/bdslib/config.hjson bdsnode --port 9944
```

On startup `bdsnode`:

1. Initialises the DuckDB-backed `ShardsManager` from the config file or `BDS_CONFIG`.
2. Initialises the BUND VM runtime (`init_adam`).
3. Binds the JSON-RPC listener on `host:port`.
4. Runs until `Ctrl-C`, then checkpoints the database (`sync_db`) before exit.

---

## Client

`bdscmd` is the dedicated command-line client for this API. It wraps every
method listed below as its own subcommand, handles the pre-flight server check,
and pretty-prints results. See [../BDSCMD.md](../BDSCMD.md) for the full
reference.

```bash
bdscmd status
bdscmd fulltext -q "kernel panic" -d 1h
bdscmd eval my_script.bund
```

---

## Protocol

All requests use **JSON-RPC 2.0** over plain HTTP `POST` to the server root (`/`).

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"<method>","params":{...},"id":1}' | jq
```

Notifications (requests without an `"id"` field) are not used; always include an `"id"`.

### Time window parameters

Several methods accept an optional time window. Exactly one of the three forms may be used; if none is provided the method queries all data.

| Parameter | Type | Description |
|---|---|---|
| `duration` | string | Lookback window from now, e.g. `"1h"`, `"30min"`, `"7d"` |
| `start_ts` | integer | Range start as Unix seconds (must be paired with `end_ts`) |
| `end_ts` | integer | Range end as Unix seconds (must be paired with `start_ts`) |

### Error codes

| Code | Meaning |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32002` | Shard index query failed |
| `-32003` | Shard open failed |
| `-32004` | Observability query failed |
| `-32005` | Relationship lookup failed |
| `-32099` | Ingest channel overloaded — back off and retry (only from `v2/add`, `v2/add.batch`, `v2/add.file`, `v2/add.file.syslog`) |
| `-32404` | Record not found |
| `-32600` | Invalid parameter (bad UUID, bad duration string, etc.) |

---

## API Reference

| Method | Description |
|---|---|
| [`v2/status`](v2_status.md) | Live process snapshot: node identity, uptime, timestamp, hostname, and ingest queue depths |
| [`v2/add`](v2_add.md) | Enqueue a single telemetry document for async persistence |
| [`v2/add.batch`](v2_add_batch.md) | Enqueue a list of telemetry documents for async persistence |
| [`v2/add.file`](v2_add_file.md) | Validate and enqueue a file of newline-delimited JSON telemetry documents for async background ingestion |
| [`v2/add.file.syslog`](v2_add_file_syslog.md) | Validate and enqueue an RFC 3164 syslog file for async background ingestion; each line is parsed and converted to a structured telemetry document |
| [`v2/timeline`](v2_timeline.md) | Earliest and latest event timestamps across all shards |
| [`v2/count`](v2_count.md) | Total number of telemetry records, optionally filtered by time window |
| [`v2/shards`](v2_shards.md) | List of shards with time boundaries, path, and primary/secondary counts |
| [`v2/keys`](v2_keys.md) | Unique sorted list of primary record keys within a duration window |
| [`v2/keys.all`](v2_keys_all.md) | Unique sorted list of primary record keys within a duration window, filtered by an optional shell-glob pattern (default `*`) |
| [`v2/keys.get`](v2_keys_get.md) | Primary record IDs and secondary ID lists for keys matching a shell-glob pattern within a duration window |
| [`v2/primaries`](v2_primaries.md) | UUIDs of all primary records, optionally filtered by time window |
| [`v2/primaries.explore`](v2_primaries_explore.md) | Keys with more than one primary record in a duration window, with counts and UUIDs |
| [`v2/primaries.explore.telemetry`](v2_primaries_explore_telemetry.md) | Keys with more than one numeric-data primary in a duration window — suitable for `v2/trends` |
| [`v2/primaries.get`](v2_primaries_get.md) | `data` payloads and timestamps for all primary records matching an exact key within a duration window |
| [`v2/primaries.get.telemetry`](v2_primaries_get_telemetry.md) | Extracted numeric values (`data` or `data["value"]`) for primary records matching an exact key within a duration window |
| [`v2/primary`](v2_primary.md) | Full document for a single primary record by UUID |
| [`v2/secondaries`](v2_secondaries.md) | UUIDs of secondary records associated with a primary |
| [`v2/secondary`](v2_secondary.md) | Full document for a single secondary record by UUID |
| [`v2/duplicates`](v2_duplicates.md) | Map of primary UUID → duplicate timestamps, optionally filtered by time window |
| [`v2/fulltext`](v2_fulltext.md) | Full-text search returning matching primary IDs and BM25 relevance scores |
| [`v2/fulltext.get`](v2_fulltext_get.md) | Full-text search returning complete primary documents with linked secondaries |
| [`v2/fulltext.recent`](v2_fulltext_recent.md) | Full-text search returning IDs, timestamps, and scores sorted by most recent first |
| [`v2/search`](v2_search.md) | Semantic vector search returning primary IDs, timestamps, and similarity scores sorted by score |
| [`v2/search.get`](v2_search_get.md) | Semantic vector search returning complete primary documents sorted by timestamp |
| [`v2/trends`](v2_trends.md) | Statistical trend summary for a single key: min, max, mean, median, std-dev, anomalies, and breakouts |
| [`v2/topics`](v2_topics.md) | LDA topic modelling over a single key's telemetry corpus within a lookback window, returning a keyword summary |
| [`v2/topics.all`](v2_topics_all.md) | LDA topic modelling over every distinct key in the window, returning one keyword summary per key |
| [`v2/rca`](v2_rca.md) | Root cause analysis: cluster non-telemetry events by co-occurrence and rank probable causes of a named failure key |
| [`v2/rca.templates`](v2_rca_templates.md) | Root cause analysis on drain3 template observations: cluster template bodies by co-occurrence and rank probable causes of a named failure template |
| [`v2/textrank.templates`](v2_textrank.templates.md) | Extractive TextRank summary of every drain3 template observed in a lookback window — fingerprints each template and returns the highest-ranked ones joined as a single string |
| [`v2/summary_for_recent`](v2_summary_for_recent.md) | Extractive TextRank summary of text-bearing primary records observed in a lookback window — skips numeric measurements, extracts bodies from `data["value"]` or `data["raw"]` |
| [`v2/summary_for_query`](v2_summary_for_query.md) | Extractive TextRank summary of primary records matching a vector query — same body-extraction rule as `v2/summary_for_recent`; default lookback is 365 days |
| [`v2/summary_lsa_for_recent`](v2_summary_lsa_for_recent.md) | Extractive LSA summary of text-bearing primary records observed in a lookback window — same body-extraction rule as `v2/summary_for_recent`; uses SVD-based Steinberger-Ježek scoring |
| [`v2/summary_lsa_for_query`](v2_summary_lsa_for_query.md) | Extractive LSA summary of primary records matching a vector query — same body-extraction and lookup as `v2/summary_for_query`; LSA backend |
| [`v2/anomaly.recent`](v2_anomaly_recent.md) | N-gram anomaly detection over recent primary records — fingerprints each record (key + `json_fingerprint(data)`) and feeds the strings to `bdslib::analysis::ngram::ngram_anomaly_with`; returns its JSON verbatim |
| [`v2/denoise.recent`](v2_denoise_recent.md) | N-gram noise removal over recent primary records — same fingerprinting as `v2/anomaly.recent`, fed to `bdslib::analysis::ngram::ngram_remove_noise_with`; splits the corpus into `kept` (signal) and `removed` (noise) |
| [`v2/knn`](v2_knn.md) | k-NN intelligence over recent primary records — same fingerprinting as `v2/anomaly.recent`, fed to `bdslib::analysis::knn::knn_summary_with`; returns clusters, density-ranked representatives, and isolated outliers as one structured JSON document |
| [`v2/tpl.add`](v2_tpl_add.md) | Manually store a template (name, body, tags, description) in the per-shard tplstorage |
| [`v2/tpl.get`](v2_tpl_get.md) | Fetch a template's metadata and body by UUID |
| [`v2/tpl.list`](v2_tpl_list.md) | List every template (manual + drain3) stored in shards overlapping a humantime window, metadata only |
| [`v2/tpl.search`](v2_tpl_search.md) | Semantic vector search over templates within a humantime window, ranked by cosine similarity |
| [`v2/tpl.update`](v2_tpl_update.md) | Update one or more fields (name, body, tags, description) of a template by UUID — partial merge |
| [`v2/tpl.delete`](v2_tpl_delete.md) | Remove a template (metadata + body + vector entry) by UUID; idempotent |
| [`v2/tpl.reindex`](v2_tpl_reindex.md) | Rebuild the tplstorage HNSW index for every shard overlapping a humantime window |
| [`v2/tpl.template_by_id`](v2_tpl_template_by_id.md) | Fetch a single drain3 template document by UUID, scanning all shards |
| [`v2/tpl.templates_by_timestamp`](v2_tpl_templates_by_timestamp.md) | List drain3 template documents whose FrequencyTracking observation falls within an explicit Unix-second range |
| [`v2/tpl.templates_recent`](v2_tpl_templates_recent.md) | List drain3 template documents whose FrequencyTracking observation falls within a humantime lookback window |
| [`v2/signal.emit`](v2_signal_emit.md) | Emit a signal — name + severity + timestamp + arbitrary metadata — into the per-shard signal store |
| [`v2/signal.update`](v2_signal_update.md) | Replace a signal's metadata in-place by UUID (full overwrite, not merge) |
| [`v2/signals`](v2_signals.md) | List signals observed within a humantime window, with full metadata resolved per signal |
| [`v2/signals_query`](v2_signals_query.md) | Semantic search over the signal store by plain-text query, ranked by cosine similarity |
| [`v2/chat.ollama`](v2_chat_ollama.md) | Send a question to a local Ollama model with retrieval-augmented context drawn from observability + document stores; supports stateful sessions via `chat_id` |
| [`v2/eval`](v2_eval.md) | Compile and evaluate a BUND VM script in a named context, returning the workbench stack as JSON |
| [`v2/eval.queued`](v2_eval_queued.md) | Submit a BUND script to the worker pool for async execution; returns a result-queue id immediately |
| [`v2/aggregationsearch`](v2_aggregationsearch.md) | Parallel vector search over time-scoped telemetry shards + semantic document store search; returns `"observability"` and `"documents"` |
| [`v2/doc.add`](v2_doc_add.md) | Store a document with JSON metadata and text content; auto-embeds both slots in the HNSW index |
| [`v2/doc.add.file`](v2_doc_add_file.md) | Load a text file, split into overlapping chunks, and store each chunk as an independently searchable record |
| [`v2/doc.get`](v2_doc_get.md) | Retrieve both metadata and content text for a document by UUID |
| [`v2/doc.get.metadata`](v2_doc_get_metadata.md) | Retrieve only the JSON metadata for a document by UUID |
| [`v2/doc.get.content`](v2_doc_get_content.md) | Retrieve only the content text for a document by UUID |
| [`v2/doc.update.metadata`](v2_doc_update_metadata.md) | Replace the metadata of a document in-place (vector index not updated automatically) |
| [`v2/doc.update.content`](v2_doc_update_content.md) | Replace the content text of a document in-place (vector index not updated automatically) |
| [`v2/doc.delete`](v2_doc_delete.md) | Remove a document from all three sub-stores (metadata, blob, HNSW); idempotent |
| [`v2/doc.search`](v2_doc_search.md) | Semantic search by plain-text query; returns ranked documents with score, metadata, and content |
| [`v2/doc.search.json`](v2_doc_search_json.md) | Semantic search by JSON query object via json_fingerprint embedding |
| [`v2/doc.search.strings`](v2_doc_search_strings.md) | Semantic search returning results as flat json_fingerprint strings |
| [`v2/doc.reindex`](v2_doc_reindex.md) | Rebuild the HNSW vector index from persisted metadata and blobs; use after unclean shutdown or bulk content updates |
| [`v2/results.len`](v2_results_len.md) | Number of result queues currently tracked, with their UUIDs |
| [`v2/results.push`](v2_results_push.md) | Push a JSON value onto the back of the result queue identified by `id`; auto-creates the queue with a fresh creation timestamp |
| [`v2/results.pull`](v2_results_pull.md) | Pop the front value from the result queue identified by `id`; returns the value as JSON plus `remaining` count |
| [`v2/results.empty`](v2_results_empty.md) | Number of elements in the result queue identified by `id`, with `empty` boolean |
| [`v2/script_add`](v2_script_add.md) | Store a new BUND script — metadata must contain `name` and `schedule` (crontab-style); returns the assigned UUIDv7 |
| [`v2/scripts`](v2_scripts.md) | List every stored BUND script with `id`, `name`, `schedule`, and the full metadata document |
| [`v2/script`](v2_script.md) | Fetch a single BUND script body and metadata by UUIDv7 |
| [`v2/script_update`](v2_script_update.md) | Replace metadata and body of an existing script (full overwrite, not merge) |
| [`v2/script_delete`](v2_script_delete.md) | Remove a script from all sub-stores; idempotent |
