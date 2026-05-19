# bdslib — Operational Scripts

Shell scripts for data ingestion, node submission, and end-to-end verification of a running bdslib installation.  All scripts live in the `scripts/` directory and require `bash` 4+ (`set -euo pipefail`).

---

## Table of contents

| Script | Purpose |
|---|---|
| [`fill-store.sh`](#fill-storesh) | Populate a running bdsnode with synthetic telemetry, logs, and docstore documents in one shot |
| [`send_file_to_node.sh`](#send_file_to_nodesh) | Generate an NDJSON file, submit it to bdsnode via `v2/add.file`, wait for ingestion, then remove the file |
| [`send_logs_to_node.sh`](#send_logs_to_nodesh) | Generate mixed + log documents in memory and submit them as a single `v2/add.batch` call |
| [`send_syslog_to_node.sh`](#send_syslog_to_nodesh) | Generate an RFC 3164 syslog file, submit via `v2/add.file.syslog`, wait for ingestion, verify with `v2/fulltext*` |
| [`verify_analysis.sh`](#verify_analysissh) | End-to-end LDA topic analysis test against a live bdscli binary and database |
| [`verify_ingestion.sh`](#verify_ingestionsh) | End-to-end ingestion correctness test: primary/secondary split, exact-match dedup, vector index |
| [`verify_logs.sh`](#verify_logssh) | End-to-end log pipeline test: ingestion, deduplication, FTS, and vector search |

---

## fill-store.sh

Populate a running `bdsnode` with synthetic data in a single command.  The script generates and submits three classes of data:

- **Docstore documents** — realistic IT operational content (runbooks, incident tickets, post-mortems, KB articles, change requests) submitted one-by-one via `v2/doc.add`.
- **Telemetry** — numeric metric records for 10 infrastructure keys (`cpu.usage`, `mem.used_pct`, `http.latency_ms`, …) submitted in batches via `v2/add.batch`.
- **Logs** — syslog, HTTP, nginx, and Python traceback entries submitted per-format in batches via `v2/add.batch`.
- **Mixed batch** — an extra combined telemetry+log batch spread over an 8-hour window.

After ingestion the script queries `v2/count` and `v2/timeline` and prints a store summary.

### Dependencies

`bdscli` · `bdscmd` · `jq`

If `bdscli` or `bdscmd` are not in `PATH`, the script falls back to `target/debug/<bin>` relative to the repository root.

### Usage

```
./scripts/fill-store.sh [OPTIONS]
```

### Options

| Flag | Default | Description |
|---|---|---|
| `--addr HOST:PORT` | `http://127.0.0.1:9000` | bdsnode address.  Passed to every `bdscmd` call as `-a`. |
| `--config PATH` | — | hjson config file.  Passed to every `bdscli generate` call as `--config`.  Falls back to the `BDS_CONFIG` environment variable when omitted. |
| `--tel-count N` | `200` | Telemetry records generated **per key** (10 keys → up to `10 × N` telemetry records queued). |
| `--log-count N` | `300` | Log records generated **per format** (4 formats → up to `4 × N` log records queued). |
| `--doc-count N` | `40` | Docstore documents (runbooks, tickets, post-mortems, KB articles, change requests) added to the docstore. |
| `--no-color` | off | Suppress ANSI colour codes in output. |
| `-h`, `--help` | — | Print usage and exit. |

### Environment variables

| Variable | Purpose |
|---|---|
| `BDSCLI` | Path to the bdscli binary (default: `bdscli`). |
| `BDSCMD` | Path to the bdscmd binary (default: `bdscmd`). |
| `BDSCMD_ADDR` | bdsnode address; equivalent of `--addr`. |
| `BDS_CONFIG` | Config file path; used by `bdscli generate` when `--config` is not given. |

### Behaviour

1. **Preflight** — verifies `jq`, `bdscli`, `bdscmd` are available; confirms `bdsnode` is reachable via `v2/status`.
2. **Docstore** — calls `bdscli generate docs --count N`, reads each NDJSON line, extracts `metadata` and `content` with `jq`, and calls `bdscmd doc-add`.  Counts successes and failures.
3. **Telemetry** — for each of the 10 metric keys, pipes `bdscli generate telemetry --key K --duration D --count N` into `bdscmd add-batch` and records the `queued` count returned by the server.

   | Key | Window |
   |---|---|
   | `cpu.usage` | 6 h |
   | `mem.used_pct` | 6 h |
   | `disk.io_wait` | 3 h |
   | `disk.read_bytes` | 3 h |
   | `net.rx_bytes` | 12 h |
   | `net.tx_bytes` | 12 h |
   | `http.latency_ms` | 4 h |
   | `db.connections` | 4 h |
   | `cache.hit_ratio` | 2 h |
   | `queue.depth` | 2 h |

4. **Logs** — for each format, pipes `bdscli generate log --format F --duration D --count N` into `bdscmd add-batch`.

   | Format | Window |
   |---|---|
   | `syslog` | 24 h |
   | `http` | 12 h |
   | `http-nginx` | 12 h |
   | `traceback` | 6 h |

5. **Mixed batch** — generates `2 × tel-count` records with `--ratio 0.5` over an 8-hour window and batch-submits them.
6. **Summary** — prints total observability record count (from `v2/count`) and the oldest/newest event timestamps (from `v2/timeline`).

### Examples

```bash
# Default counts (40 docs, 200 telemetry/key, 300 logs/format) against local node:
BDS_CONFIG=/etc/bdslib/config.hjson ./scripts/fill-store.sh

# Custom counts for a quick smoke test:
./scripts/fill-store.sh --config ./bds.hjson --doc-count 10 --tel-count 50 --log-count 100

# Remote node, no colour:
./scripts/fill-store.sh --addr http://10.0.0.5:9000 --config /etc/bds/prod.hjson --no-color

# Binaries not in PATH (freshly built):
BDSCLI=./target/debug/bdscli BDSCMD=./target/debug/bdscmd \
  ./scripts/fill-store.sh --config ./bds.hjson
```

### Exit codes

| Code | Meaning |
|---|---|
| `0` | All generators ran and data was submitted without fatal errors. |
| `1` | Preflight failure (missing dependency or unreachable bdsnode). |

Individual `bdscmd doc-add` failures are counted and reported but do not abort the script.

---

## send_file_to_node.sh

Generate synthetic documents into a file, submit the file path to a running `bdsnode` via `v2/add.file`, and wait for background ingestion to complete by polling `v2/status`.  When `json_file_queue` reaches `0` and `json_file_name` becomes `null`, ingestion is finished and the file is removed (unless `--keep` or `-o` is in effect).

### Dependencies

`bdscli` · `curl` · `jq`

### Usage

```
./scripts/send_file_to_node.sh [OPTIONS]
```

### Options

| Flag | Default | Description |
|---|---|---|
| `-a`, `--address ADDR` | `127.0.0.1:9000` | bdsnode `host:port` or full URL.  `http://` is prepended automatically when absent. |
| `-n`, `--count N` | `100` | Number of mixed documents to generate. |
| `-d`, `--duration DUR` | `1h` | Timestamp window for generated data in humantime format (e.g. `30min`, `6h`, `7days`). |
| `-r`, `--ratio FLOAT` | `0.5` | Fraction of generated documents that are telemetry.  `0.0` = all log entries; `1.0` = all telemetry. |
| `--duplicate FLOAT` | `0.0` | Fraction of documents to re-emit as duplicates. |
| `-o`, `--output FILE` | — | Write the generated NDJSON to `FILE` instead of a temp file.  Implies `--keep`. |
| `--keep` | off | Keep the generated file after ingestion completes instead of removing it. |
| `--poll-interval N` | `1` | Seconds between `v2/status` polls while waiting for ingestion. |
| `--timeout N` | `300` | Maximum seconds to wait.  The script exits with code `2` if ingestion is not complete within this limit.  Set to `0` to disable. |
| `-c`, `--config FILE` | — | Optional bdscli config file, passed as `bdscli --config FILE`. |
| `--bdscli PATH` | `bdscli` | Path to the bdscli binary.  Also read from `$BDSCLI` environment variable. |
| `-h`, `--help` | — | Print usage and exit. |

### Behaviour

1. Generates `--count` mixed documents via `bdscli generate mixed` and writes them as NDJSON to the output file.
2. Submits the absolute file path to `bdsnode` via a `v2/add.file` JSON-RPC call.
3. If the response contains a JSON-RPC `error` field the script exits immediately with code `1`.
4. Polls `v2/status` every `--poll-interval` seconds, printing a live status line showing `json_file_queue` and `json_file_name`.
5. Exits the loop when `json_file_queue == 0` and `json_file_name == null`.
6. Removes the file (or keeps it if `--keep` / `-o`).

If the script exits early due to an error before ingestion monitoring completes, any temp file it created is removed by an `EXIT` trap.

### Examples

```bash
# 200 docs over a 2h window, local node, remove file when done:
./scripts/send_file_to_node.sh -n 200 -d 2h

# Remote node, keep the file:
./scripts/send_file_to_node.sh -a 10.0.0.5:9000 -n 500 --keep

# Write to a fixed path (kept automatically):
./scripts/send_file_to_node.sh -n 1000 -o /tmp/batch.jsonl

# 20% duplicates, all-telemetry mix, 10-minute timeout:
./scripts/send_file_to_node.sh --duplicate 0.2 -r 1.0 -n 300 --timeout 600

# Poll every 2 seconds:
./scripts/send_file_to_node.sh -n 5000 --poll-interval 2
```

### Exit codes

| Code | Meaning |
|---|---|
| `0` | Ingestion confirmed complete; file disposed according to `--keep`. |
| `1` | Preflight failure, generation error, or server returned an error response. |
| `2` | Timed out waiting for ingestion to complete. |

---

## send_logs_to_node.sh

Generate synthetic documents in memory — one batch of mixed telemetry+log entries and one batch of log-only entries — and submit them to a running `bdsnode` in a single `v2/add.batch` call.  No file is written to disk.

Total documents submitted = `2 × --count`.

### Dependencies

`bdscli` · `curl` · `jq`

### Usage

```
./scripts/send_logs_to_node.sh [OPTIONS]
```

### Options

| Flag | Default | Description |
|---|---|---|
| `-a`, `--address ADDR` | `127.0.0.1:9000` | bdsnode `host:port` or full URL. |
| `-n`, `--count N` | `100` | Documents per generator (mixed and log each produce `N`; total = `2N`). |
| `-d`, `--duration DUR` | `1h` | Timestamp window in humantime format. |
| `-r`, `--ratio FLOAT` | `0.5` | Telemetry fraction for the mixed generator. |
| `-f`, `--format FMT` | `random` | Log format for the log generator.  One of: `random` `syslog` `http` `http-nginx` `traceback`. |
| `--duplicate FLOAT` | `0.0` | Duplicate fraction applied to both generators. |
| `-c`, `--config FILE` | — | Optional bdscli config file. |
| `--bdscli PATH` | `bdscli` | Path to the bdscli binary or `$BDSCLI` env var. |
| `-h`, `--help` | — | Print usage and exit. |

### Behaviour

1. Runs `bdscli generate mixed` and `bdscli generate log` back-to-back.
2. Pipes both streams through `jq -s '.'` to produce a single JSON array.
3. Wraps the array in a `v2/add.batch` JSON-RPC payload and POSTs it to the node.
4. Prints the server response.

Because both generators run synchronously and the payload is delivered in a single HTTP request, this script is best suited for moderate batch sizes (up to a few thousand documents).  For larger datasets use `send_file_to_node.sh` instead, which lets the node process the file in a streaming background thread.

### Log formats

| Format | Content |
|---|---|
| `random` | Mix of all available log formats, chosen randomly per document. |
| `syslog` | RFC-3164 syslog lines (program name, PID, message). |
| `http` | Generic HTTP access log entries. |
| `http-nginx` | Nginx-style access log entries with `data.server = "nginx"`. |
| `traceback` | Python-style exception tracebacks. |

### Examples

```bash
# 200 docs (100 mixed + 100 log) to local node:
./scripts/send_logs_to_node.sh -n 100

# 500 docs over a 6h window with 20% duplicates to a remote node:
./scripts/send_logs_to_node.sh -a 10.0.0.5:9000 -n 250 -d 6h --duplicate 0.2

# Syslog-only log batch, pure telemetry mixed batch:
./scripts/send_logs_to_node.sh -f syslog -r 1.0 -n 50

# http-nginx logs for FTS / vector search testing:
./scripts/send_logs_to_node.sh -f http-nginx -n 200
```

---

## send_syslog_to_node.sh

Generate synthetic RFC 3164 syslog lines into a file via `bdscli generate syslog`, submit the file path to a running `bdsnode` via `v2/add.file.syslog`, and wait for background ingestion to complete by polling `v2/status`.  When `syslog_file_queue` reaches `0` and `syslog_file_name` becomes `null`, ingestion is finished.  The script then verifies ingestion by issuing three full-text queries (`v2/fulltext`, `v2/fulltext.get`, `v2/fulltext.recent`) and prints the results.  The file is removed unless `--keep` or `-o` is in effect.

### Dependencies

`bdscli` · `curl` · `jq`

### Usage

```
./scripts/send_syslog_to_node.sh [OPTIONS]
```

### Options

| Flag | Default | Description |
|---|---|---|
| `-a`, `--address ADDR` | `127.0.0.1:9000` | bdsnode `host:port` or full URL.  `http://` is prepended automatically when absent. |
| `-n`, `--count N` | `100` | Number of RFC 3164 syslog lines to generate. |
| `-d`, `--duration DUR` | `1h` | Timestamp window for generated lines in humantime format (e.g. `30min`, `6h`, `7days`). |
| `-q`, `--query TERM` | `kernel` | FTS term used for the post-ingestion `v2/fulltext*` verification queries.  Common syslog program names: `kernel` `sshd` `cron` `su` `nginx`. |
| `--verify-limit N` | `10` | Maximum hits returned by `v2/fulltext` and `v2/fulltext.recent`. |
| `-o`, `--output FILE` | — | Write the generated syslog to `FILE` instead of a temp file.  Implies `--keep`. |
| `--keep` | off | Keep the generated file after ingestion completes. |
| `--poll-interval N` | `1` | Seconds between `v2/status` polls while waiting for ingestion. |
| `--timeout N` | `300` | Maximum seconds to wait.  Exits with code `2` if ingestion is not complete within this limit.  Set to `0` to disable. |
| `-c`, `--config FILE` | — | Optional bdscli config file, passed as `bdscli --config FILE`. |
| `--bdscli PATH` | `bdscli` | Path to the bdscli binary.  Also read from `$BDSCLI` environment variable. |
| `-h`, `--help` | — | Print usage and exit. |

### Behaviour

1. Generates `--count` RFC 3164 syslog lines via `bdscli generate syslog` and writes them to the output file (one plain-text line per syslog entry).
2. Submits the absolute file path to `bdsnode` via a `v2/add.file.syslog` JSON-RPC call.
3. If the response contains a JSON-RPC `error` field the script exits with code `1`.
4. Polls `v2/status` every `--poll-interval` seconds, printing a live status line showing `syslog_file_queue` and `syslog_file_name`.
5. Exits the monitoring loop when `syslog_file_queue == 0` and `syslog_file_name == null`.
6. Runs three verification queries against the `--duration` window:
   - `v2/fulltext` — returns matching IDs and BM25 scores (up to `--verify-limit`).
   - `v2/fulltext.get` — returns the first 3 full documents matching the query.
   - `v2/fulltext.recent` — returns matching IDs sorted newest-first.
7. Prints a warning (but does not fail) if `v2/fulltext` returns 0 hits — this can happen when the default query term is not present in the generated data; use `--query` to choose a different term.
8. Removes the file (or keeps it if `--keep` / `-o`).

If the script exits early due to an error before ingestion monitoring completes, any temp file it created is removed by an `EXIT` trap.

### Difference from `send_file_to_node.sh`

| Aspect | `send_file_to_node.sh` | `send_syslog_to_node.sh` |
|---|---|---|
| Generator | `bdscli generate mixed` | `bdscli generate syslog` |
| File format | NDJSON (one JSON doc per line) | Plain RFC 3164 syslog (one text line per entry) |
| RPC method | `v2/add.file` | `v2/add.file.syslog` |
| Status fields watched | `json_file_queue`, `json_file_name` | `syslog_file_queue`, `syslog_file_name` |
| Post-ingestion verify | — | `v2/fulltext`, `v2/fulltext.get`, `v2/fulltext.recent` |

### Examples

```bash
# 200 syslog lines over a 2h window, local node:
./scripts/send_syslog_to_node.sh -n 200 -d 2h

# Keep the generated file, verify with "sshd" term:
./scripts/send_syslog_to_node.sh -n 500 --keep -q sshd

# Write to a specific path (kept automatically):
./scripts/send_syslog_to_node.sh -n 1000 -o /tmp/test.syslog

# Poll every 2 seconds, time out after 10 minutes:
./scripts/send_syslog_to_node.sh -n 5000 --poll-interval 2 --timeout 600

# Submit to a remote node, query with "cron":
./scripts/send_syslog_to_node.sh -a 10.0.0.5:9000 -n 300 -q cron
```

### Exit codes

| Code | Meaning |
|---|---|
| `0` | Ingestion confirmed complete; file disposed according to `--keep`. |
| `1` | Preflight failure, generation error, or server returned an error response. |
| `2` | Timed out waiting for ingestion to complete. |

---

## verify_analysis.sh

End-to-end correctness test for LDA topic analysis.  Builds `bdscli` from source, creates a fresh database, ingests a structured three-cluster corpus and a near-duplicate batch, then verifies record counts, the primary/secondary split, topic count, keyword content, and sensitivity to the `k` parameter.

**This script wipes and recreates the database** (`init --new`).  Do not run against a production database.

### Dependencies

`cargo` · `bdscli` binary at `./target/debug/bdscli`

### Usage

```bash
./scripts/verify_analysis.sh [path/to/bds.hjson]
```

The config file defaults to `./bds.hjson` when not provided.

### What it tests

| Step | Action | Assertion |
|---|---|---|
| 0 | `cargo build --bin bdscli` | Binary compiles |
| 1 | `init --new` | Clean database created |
| 2 | Ingest 20 security/auth docs (`corpus.logs`) | Cluster A ingested |
| 3 | Ingest 20 infrastructure/system docs (`corpus.logs`) | Cluster B ingested |
| 4 | Ingest 20 application/error docs (`corpus.logs`) | Cluster C ingested |
| 5 | Ingest 5 near-duplicate security docs (`corpus.near`) | Near-dup batch ingested |
| 6 | `get` (all records) | Total = 65 (60 corpus + 5 near-dup) |
| 7 | `get --primary` / `get --secondary` | `corpus.near`: exactly 1 primary, 4 secondaries |
| 8 | `analyze topics --key corpus.logs --k 3` | `n_docs=60`, `n_topics=3`, keywords non-empty |
| 9 | Keyword check per cluster | `security`/`login` (A), `system`/`disk` (B), `application`/`overflow` (C) each present |
| 10 | Repeat with `k=2` and `k=5` | Reported `n_topics` matches `k` in both cases |
| 11 | `analyze topics --key corpus.near --k 3` | `n_docs=5`, keywords contain corpus-specific terms (`security`, `login`, `deployer`, `gateway`) |

### Corpus design

Each cluster uses a template with a constant `category` field (the primary LDA discriminator) and a randomised `action` from a small vocabulary, plus a high-cardinality `$int` index field that ensures every document has a unique `data_text` fingerprint, preventing exact-match deduplication from collapsing the corpus.

The near-duplicate batch (`corpus.near`) uses a fixed `action = "login success"` and `user = "deployer"` across all five documents — they share near-identical embeddings, so the storage engine produces exactly 1 primary and 4 secondaries.

### Output

Each check prints a colour-coded `PASS` or `FAIL` line.  On first failure the script exits with code `1`.  A final `ALL CHECKS PASSED` banner is printed when all assertions succeed.

---

## verify_ingestion.sh

End-to-end correctness test for the core ingestion pipeline: record storage, primary/secondary classification, exact-match deduplication, duplication timestamps, and vector index persistence.

**This script wipes and recreates the database** (`init --new`).  Do not run against a production database.

### Dependencies

`cargo` · `bdscli` binary at `./target/debug/bdscli` · `python3`

### Usage

```bash
./scripts/verify_ingestion.sh [path/to/bds.hjson]
```

The config file defaults to `./bds.hjson` when not provided.

### What it tests

| Step | Action | Assertion |
|---|---|---|
| 0 | `cargo build --bin bdscli` | Binary compiles |
| 1 | `init --new` | Clean database created |
| 2 | Ingest 10 random telemetry docs | All 10 stored |
| 3 | Ingest 5 near-duplicate docs (`verify.secondary`) | 5 records stored; embedding dedup produces 1 primary + 4 secondaries |
| 4 | `get` (no flags) | Total = 15 |
| 5 | `get --primary` | Exactly 1 primary for key `verify.secondary` |
| 6 | `get --secondary --primary-id <UUID>` | 4 secondaries, all carrying key `verify.secondary` |
| 7 | `get --duration 1h` | Same 15 records visible in windowed query |
| 8 | `get --primary --duration 1h` | Primary count matches full scan |
| 9 | Ingest 3 exact-match duplicate docs (`verify.dedup3`) | 1 record stored; 2 extra submissions tracked in `dedup_tracking` |
| 10 | `get --duplication-timestamps` | At least 1 dedup entry; key `verify.dedup3` present |
| 11 | `get --duplication-timestamps --primary-id <UUID>` | Exactly 2 `duplicate_timestamps` entries |
| 12 | Vector index check | At least 1 file under `db/*/vec/` (index flushed to disk) |

### Near-duplicate template

The near-duplicate batch uses a fixed `value=42.0`, `unit=percent`, `host=testhost`, `env=prod` with a random high-cardinality `idx` field.  This ensures:
- Each document has a **distinct** `data_text` fingerprint → exact-match dedup is not triggered.
- All documents have **near-identical** embeddings → the engine classifies documents 2–5 as secondaries of document 1.

`--duration 1min` pins all timestamps inside the current 1-hour shard window so the embedding comparison always operates within a single shard scope.

### Output

Colour-coded `PASS` / `FAIL` lines.  Exits with code `1` on first failure; prints `ALL CHECKS PASSED` on success.

---

## verify_logs.sh

End-to-end correctness test for the full log ingestion and search pipeline: syslog and nginx log ingestion, semantic near-duplicate classification, exact-match deduplication with duplication timestamps, full-text search (Tantivy/BM25), and HNSW vector search.

**This script wipes and recreates the database** (`init --new`).  Do not run against a production database.

### Dependencies

`cargo` · `bdscli` binary at `./target/debug/bdscli` · `python3`

### Usage

```bash
./scripts/verify_logs.sh [path/to/bds.hjson]
```

The config file defaults to `./bds.hjson` when not provided.

### What it tests

| Step | Action | Assertion |
|---|---|---|
| 0 | `cargo build --bin bdscli` | Binary compiles |
| 1 | `init --new` | Clean database created |
| 2 | Ingest 20 syslog-format entries | All 20 stored |
| 3 | Ingest 10 `http-nginx` log entries | All 10 stored; these are FTS and vector search targets |
| 4 | Ingest 5 near-duplicate syslog entries (`test.sshd.auth`) | 1 primary + 4 secondaries |
| 5 | Ingest 3 exact-match duplicate entries (`test.nginx.proc`) | 1 record stored + 2 dedup timestamps |
| 6 | `get` (no flags) | Total = 36 stored records (20 + 10 + 5 + 1) |
| 7 | `get --primary` / `get --secondary` | `test.sshd.auth`: 1 primary, 4 secondaries, correct key on all |
| 8 | Dedup timestamps — global listing | Key `test.nginx.proc` present |
| 9 | Dedup timestamps — per primary | Exactly 2 `duplicate_timestamps` for `test.nginx.proc` |
| 10 | Verify dedup primary has 0 secondaries | Exact-match path uses `dedup_tracking`, not `primary_secondary` table |
| 11 | `search fts --query "nginx"` | ≥ 10 FTS hits (matches all ingested nginx docs) |
| 12 | `search fts --query "sshd OR cron OR postgres OR kernel"` | ≥ 1 FTS hit from syslog program names |
| 13 | `search vector --query "HTTP web server nginx access log request"` | ≥ 1 vector hit |
| 14 | `search vector --query "SSH authentication public key login"` | ≥ 1 vector hit |
| 15 | `get --duration 1h` | Same 36 records visible in windowed query |

### Deduplication paths tested

| Path | Trigger | Storage outcome |
|---|---|---|
| **Exact-match** | Identical `key` + `data` | 1 record in `telemetry`; extra timestamps in `dedup_tracking` |
| **Semantic (secondary)** | Same `key`; embedding similarity ≥ threshold | 1 primary in `telemetry`; remaining in `primary_secondary` |

The near-duplicate syslog batch uses a fixed SSH message with a random `idx` field, giving distinct `data_text` (bypassing exact-match) while keeping embeddings close enough to trigger secondary classification.

### Output

Colour-coded `PASS` / `FAIL` lines.  Exits with code `1` on first failure; prints `ALL CHECKS PASSED` on success.

---

## Common notes

### Required tools

| Tool | Used by |
|---|---|
| `bdscli` | all scripts |
| `bdscmd` | `fill-store.sh` |
| `curl` | `send_file_to_node.sh`, `send_logs_to_node.sh`, `send_syslog_to_node.sh` |
| `jq` | `fill-store.sh`, `send_file_to_node.sh`, `send_logs_to_node.sh`, `send_syslog_to_node.sh` |
| `cargo` | `verify_*.sh` |
| `python3` | `verify_ingestion.sh`, `verify_logs.sh` |

### BDSCLI environment variable

All scripts respect the `BDSCLI` environment variable as an alternative to `--bdscli` / `--config`:

```bash
BDSCLI=/usr/local/bin/bdscli ./scripts/send_syslog_to_node.sh -n 200
```

`fill-store.sh` additionally respects `BDSCMD` for the bdscmd binary path.

### Config file (verify scripts)

All three `verify_*.sh` scripts accept an optional positional argument pointing to the hjson config file.  When omitted they default to `./bds.hjson` in the working directory:

```bash
# Use default ./bds.hjson:
./scripts/verify_logs.sh

# Use a custom path:
./scripts/verify_logs.sh /etc/bdslib/production.hjson
```
