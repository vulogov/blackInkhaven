# bdscli — BDS Command-Line Interface

`bdscli` is the primary command-line tool for managing and querying a BDS
(multifunctional programmatic data storage) database.  It covers the full
lifecycle: initialisation, synthetic data generation, ingestion, full-text and
vector search, analytical computation, and raw document retrieval.

---

## Table of Contents

1. [Global Options](#1-global-options)
2. [Configuration File](#2-configuration-file)
3. [Commands Overview](#3-commands-overview)
4. [init](#4-init)
5. [sync](#5-sync)
6. [generate](#6-generate)
   - [generate log](#61-generate-log)
   - [generate telemetry](#62-generate-telemetry)
   - [generate mixed](#63-generate-mixed)
   - [generate templated](#64-generate-templated)
7. [get](#7-get)
8. [search](#8-search)
   - [search fts](#81-search-fts)
   - [search vector](#82-search-vector)
9. [analyze](#9-analyze)
   - [analyze trend](#91-analyze-trend)
   - [analyze topics](#92-analyze-topics)
10. [eval](#10-eval)
11. [Deduplication Concepts](#11-deduplication-concepts)
12. [Exit Codes](#12-exit-codes)

---

## 1. Global Options

These flags apply to every subcommand and must be placed **before** the
subcommand name.

| Flag | Short | Env var | Default | Description |
|------|-------|---------|---------|-------------|
| `--config <PATH>` | `-c` | `BDS_CONFIG` | — | Path to the hjson configuration file. Required by all commands that open the database. |
| `--nocolor` | — | — | `false` | Suppress ANSI colour codes in error output. |
| `--help` | `-h` | — | — | Print help and exit. |
| `--version` | `-V` | — | — | Print version and exit. |

```bash
# Explicit config path
bdscli -c ./bds.hjson <subcommand>

# Config via environment variable
export BDS_CONFIG=./bds.hjson
bdscli <subcommand>
```

---

## 2. Configuration File

The config is an [HJSON](https://hjson.github.io/) file (JSON with comments
and relaxed syntax).  A minimal working example:

```hjson
{
  dbpath: "./db"
  shard_duration: "1h"
  pool_size: 4
  similarity_threshold: 0.85
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `dbpath` | string | Yes | — | Root directory for time-partitioned shards. |
| `shard_duration` | humantime | Yes | — | Width of each shard window, e.g. `"1h"`, `"6h"`, `"1day"`. |
| `pool_size` | integer | No | `4` | Max concurrent DuckDB connections per shard. |
| `similarity_threshold` | float | No | `0.85` | Cosine-similarity threshold for secondary classification (0.0–1.0). |

---

## 3. Commands Overview

```
bdscli
├── init          Open or (re)create the database
├── sync          Flush all open shards to disk
├── generate
│   ├── log       Synthetic syslog / HTTP / traceback entries
│   ├── telemetry Synthetic metric documents
│   ├── mixed     Mix of telemetry and log entries
│   └── templated Documents from a custom JSON template
├── get           Retrieve stored documents
├── search
│   ├── fts       Full-text keyword search (Tantivy)
│   └── vector    Semantic vector search (HNSW)
├── analyze
│   ├── trend     Descriptive statistics and anomaly detection
│   └── topics    LDA topic modelling
└── eval          Execute a BUND scripting-language snippet
```

---

## 4. init

Open (or create) the BDS database described by the config file.  Without
`--new` this is a no-op when the database already exists — safe to call on
every startup.

```
bdscli [GLOBAL] init [--new]
```

| Flag | Description |
|------|-------------|
| `--new` | Remove the existing database directory first, then create a fresh empty store. **Destructive — all stored data is lost.** |

### Examples

```bash
# Open existing database (or create if absent)
bdscli -c bds.hjson init

# Wipe everything and start fresh
bdscli -c bds.hjson init --new
```

### Output

```
removed: ./db          # only when --new and the directory existed
init: OK
```

---

## 5. sync

Flush all open shard write-ahead logs to disk and persist the HNSW vector
index.  Called automatically after every `generate … --ingest`, but must be
called explicitly when using the library API directly.

```
bdscli [GLOBAL] sync
```

### Example

```bash
bdscli -c bds.hjson sync
# sync: OK
```

---

## 6. generate

Generate synthetic documents and either print them as newline-delimited JSON
(stdout) or ingest them directly into the database.

All `generate` subcommands accept:

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--duration <WINDOW>` | `-d` | `1h` | Spread generated timestamps uniformly across the trailing window. Uses humantime notation: `30min`, `1h`, `6h`, `1day`. |
| `--count <N>` | `-n` | `100` | Number of documents to generate. |
| `--ingest` | — | `false` | Ingest into the DB and print a count instead of raw JSON. |

When `--ingest` is used the config (`-c` / `BDS_CONFIG`) is required and
`sync` is called automatically after the batch.

---

### 6.1 generate log

Generate structured log-entry documents in one of several real-world formats.

```
bdscli [GLOBAL] generate log [-d WINDOW] [-n COUNT] [-f FORMAT] [--ingest]
```

| Flag | Short | Default | Values | Description |
|------|-------|---------|--------|-------------|
| `--format <FMT>` | `-f` | `random` | `random`, `syslog`, `http`, `http-nginx`, `traceback` | Log format to produce per document. `random` picks a different format for each document. |

#### Formats

| Value | Description |
|-------|-------------|
| `random` | Each document independently picks one of the formats below. |
| `syslog` | RFC-3164 syslog line — facility, severity, hostname, program, PID, message. |
| `http` | Apache Combined Log Format — method, path, status, bytes, referer, user-agent. |
| `http-nginx` | Nginx access log — similar to `http` with nginx-specific fields. |
| `traceback` | Python-style exception traceback with module, exception type, and message. |

#### Document shape (`syslog` example)

```json
{
  "timestamp": 1776978500,
  "key": "log.syslog",
  "data": {
    "raw": "Apr 23 14:15:00 web-01 sshd[1234]: Accepted publickey for deploy",
    "host": "web-01",
    "program": "sshd",
    "pid": 1234,
    "message": "Accepted publickey for deploy"
  }
}
```

#### Examples

```bash
# Print 10 random log entries to stdout
bdscli generate log -n 10

# Generate 50 nginx access log entries and ingest
bdscli -c bds.hjson generate log -n 50 --format http-nginx --ingest

# Generate syslog entries spread over the last 6 hours and ingest
bdscli -c bds.hjson generate log -n 200 -d 6h --format syslog --ingest
```

---

### 6.2 generate telemetry

Generate numeric metric documents with dotted-namespace keys and floating-point
values.

```
bdscli [GLOBAL] generate telemetry [-d WINDOW] [-n COUNT] [-k KEY] [--ingest]
```

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--key <KEY>` | `-k` | random | Restrict all documents to a single metric key, e.g. `cpu.usage`. When omitted a different metric key is chosen for each document. |

#### Document shape

```json
{
  "timestamp": 1776978900,
  "key": "cpu.usage",
  "data": {
    "value": 67.42,
    "unit": "percent",
    "host": "web-01",
    "env": "prod",
    "region": "us-east-1"
  }
}
```

#### Examples

```bash
# Print 20 telemetry documents with random keys
bdscli generate telemetry -n 20

# Ingest 500 cpu.usage samples over the last day
bdscli -c bds.hjson generate telemetry -n 500 -d 1day -k cpu.usage --ingest

# Generate memory.used metrics for trend analysis
bdscli -c bds.hjson generate telemetry -n 100 -k memory.used --ingest
```

---

### 6.3 generate mixed

Generate a blend of telemetry and log-entry documents in a single pass.

```
bdscli [GLOBAL] generate mixed [-d WINDOW] [-n COUNT] [-r RATIO] [--ingest]
```

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--ratio <F>` | `-r` | `0.5` | Fraction of documents that are telemetry. `0.0` = all logs; `1.0` = all telemetry. |

#### Examples

```bash
# 100 documents, 70 % telemetry / 30 % logs, printed to stdout
bdscli generate mixed -n 100 -r 0.7

# Ingest 1000 mixed documents spread over last 24 hours
bdscli -c bds.hjson generate mixed -n 1000 -d 1day -r 0.5 --ingest
```

---

### 6.4 generate templated

Generate documents from a custom JSON template using `$placeholder`
substitutions.  Every generated document must contain at minimum the
`timestamp` and `key` top-level fields.

```
bdscli [GLOBAL] generate templated [-d WINDOW] [-n COUNT]
    (--template JSON | --template-file PATH)
    [--ingest]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--template <JSON>` | — | Inline JSON template string. Mutually exclusive with `--template-file`. |
| `--template-file <PATH>` | — | Path to a file containing the template. Mutually exclusive with `--template`. |

#### Placeholder Reference

| Placeholder | Description |
|-------------|-------------|
| `$timestamp` | Unix timestamp within the `--duration` window. |
| `$int(min,max)` | Random integer in `[min, max]`. |
| `$float(min,max)` | Random float in `[min, max]`. |
| `$choice(a,b,c,…)` | Pick one value from the comma-separated list. |
| `$bool` | `true` or `false`. |
| `$uuid` | Random UUID v4. |
| `$ip` | Random IPv4 address. |
| `$word` | Random English word. |
| `$name` | Random person name. |

#### Examples

```bash
# Inline template: security events with random actions
bdscli -c bds.hjson generate templated -n 20 \
  --template '{"timestamp":"$timestamp","key":"sec.events","data":{"action":"$choice(login,logout,reset)","user":"$choice(alice,bob,carol)","severity":"$choice(info,warn,error)","idx":"$int(1,1000000)"}}' \
  --ingest

# Near-duplicate batch for dedup testing (idx field keeps each doc unique)
bdscli -c bds.hjson generate templated -n 5 --duration 1min \
  --template '{"timestamp":"$timestamp","key":"test.sshd","data":{"message":"Accepted publickey for deploy from 10.0.0.1","host":"web-01","idx":"$int(1,100000)"}}' \
  --ingest

# Template from a file
bdscli -c bds.hjson generate templated -n 100 \
  --template-file ./templates/event.json \
  --ingest
```

> **Deduplication note:** When multiple documents share the same `key` and
> `data`, only the first is stored (exact-match dedup).  Add a high-cardinality
> `$int` field to make each document's content unique while keeping their
> embeddings similar — this triggers the embedding-based secondary
> classification path instead.

---

## 7. get

Retrieve stored documents from the database.  Output is newline-delimited JSON
(one document per line) on stdout; summary counts go to stderr.

```
bdscli [GLOBAL] get [-d WINDOW]
    [--primary | --secondary --primary-id UUID | --duplication-timestamps [--primary-id UUID]]
```

### Flags

| Flag | Description |
|------|-------------|
| `--duration <WINDOW>` | Restrict results to documents whose event timestamp falls within the trailing window (e.g. `1h`, `30min`). When omitted, all shards are scanned. |
| `--primary` | Return only primary records (documents that are not near-duplicates of anything already stored). Mutually exclusive with `--secondary` and `--duplication-timestamps`. |
| `--secondary` | Return all secondary records (near-duplicates) that belong to the primary identified by `--primary-id`. Requires `--primary-id`. Mutually exclusive with `--primary` and `--duplication-timestamps`. |
| `--primary-id <UUID>` | UUID of the primary record. Required by `--secondary`; optional for `--duplication-timestamps`. |
| `--duplication-timestamps` | Show exact-match deduplication entries. Without `--primary-id`, lists every primary that has duplicates (UUID, key, and timestamps). With `--primary-id`, shows only the timestamps for that record. Mutually exclusive with `--primary` and `--secondary`. |

### Modes at a glance

| Invocation | What is returned |
|------------|-----------------|
| `get` | Every stored record (primaries and secondaries). |
| `get -d 1h` | Every record with an event timestamp in the last hour. |
| `get --primary` | Only primary records (all shards). |
| `get --primary -d 1h` | Only primary records from the last hour. |
| `get --secondary --primary-id UUID` | All secondary records linked to the given primary. |
| `get --duplication-timestamps` | All primaries that have exact-match duplicates. |
| `get --duplication-timestamps --primary-id UUID` | Duplicate timestamps for one specific primary. |

### Output format

**Document output** (stdout) — one JSON object per line:

```json
{"id":"019dbca3-179c-7e72-...","timestamp":1776978500,"key":"log.syslog","data":{...},"metadata":{}}
```

**Duplication-timestamps (global)** — one JSON object per line:

```json
{"primary_id":"019dbca3-...","key":"test.nginx.proc","duplicate_timestamps":[1776978600,1776978660]}
```

**Duplication-timestamps (scoped)** — single JSON object:

```json
{"primary_id":"019dbca3-...","duplicate_timestamps":[1776978600,1776978660]}
```

**Stderr summary:**

```
total: 65
```
or
```
secondaries: 4
```

### Examples

```bash
# All stored records
bdscli -c bds.hjson get

# Records from the last 30 minutes only
bdscli -c bds.hjson get -d 30min

# Only primary records
bdscli -c bds.hjson get --primary

# Primary records from the last hour
bdscli -c bds.hjson get --primary -d 1h

# Secondaries for a known primary
bdscli -c bds.hjson get --secondary \
  --primary-id 019dbca3-179c-7e72-bb86-000cd452e2d8

# All primaries that have exact-match duplicates
bdscli -c bds.hjson get --duplication-timestamps

# Duplicate timestamps for one primary
bdscli -c bds.hjson get --duplication-timestamps \
  --primary-id 019dbca3-179c-7e72-bb86-000cd452e2d8

# Pipe all primaries into jq to extract keys
bdscli -c bds.hjson get --primary | jq -r '.key' | sort | uniq -c
```

---

## 8. search

Search across the documents stored in a time window.

---

### 8.1 search fts

Full-text keyword search powered by [Tantivy](https://github.com/quickwit-oss/tantivy).
The index covers all `"field: value"` pairs extracted from each document's
`data` object.

```
bdscli [GLOBAL] search fts -q QUERY [-d WINDOW] [-l LIMIT]
```

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--query <QUERY>` | `-q` | required | Tantivy query string. Supports boolean operators (`AND`, `OR`, `NOT`), phrase queries (`"exact phrase"`), and field queries. |
| `--duration <WINDOW>` | `-d` | `1h` | Lookback window. |
| `--limit <N>` | `-l` | `20` | Maximum number of results to display. |

#### Query syntax highlights

| Pattern | Meaning |
|---------|---------|
| `nginx` | Any document containing the token `nginx`. |
| `nginx AND 200` | Both tokens present. |
| `sshd OR cron` | Either token. |
| `"login failure"` | Exact phrase. |
| `NOT timeout` | Token absent. |

#### Output

```
fts query  : "nginx"
duration   : 1h
hits       : 10  (showing 10)
  [1776978500]  score=0.9231  key=log.http-nginx
  [1776978520]  score=0.8847  key=log.http-nginx
  ...
```

#### Examples

```bash
# Find all nginx access log entries in the last hour
bdscli -c bds.hjson search fts -q "nginx"

# Boolean query for syslog program names
bdscli -c bds.hjson search fts -q "sshd OR cron OR postgres OR kernel"

# Phrase search over the last 6 hours, up to 50 results
bdscli -c bds.hjson search fts -q '"login failure"' -d 6h -l 50

# Find documents mentioning disk issues
bdscli -c bds.hjson search fts -q "disk AND (warning OR critical)"
```

---

### 8.2 search vector

Semantic nearest-neighbour search using an HNSW vector index.  The query is
embedded with the same model used at ingest time; results are ranked by cosine
similarity.

```
bdscli [GLOBAL] search vector -q QUERY [-d WINDOW] [-l LIMIT]
```

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--query <QUERY>` | `-q` | required | Free-form natural-language description of what you are looking for. |
| `--duration <WINDOW>` | `-d` | `1h` | Lookback window. |
| `--limit <N>` | `-l` | `10` | Maximum number of results to display. |

#### Output

```
vector query : "HTTP web server nginx access log request"
duration     : 1h
hits         : 10  (showing 10)
  [1776978500]  score=0.9712  key=log.http-nginx
  [1776978480]  score=0.9634  key=log.http-nginx
  ...
```

#### Examples

```bash
# Find documents semantically related to HTTP web-server access
bdscli -c bds.hjson search vector -q "HTTP web server nginx access log request"

# Find SSH authentication events
bdscli -c bds.hjson search vector -q "SSH authentication public key login"

# Semantic search over the last 24 hours, top 20
bdscli -c bds.hjson search vector \
  -q "out of memory kernel panic crash" -d 1day -l 20

# Combine with jq to extract matched keys
bdscli -c bds.hjson search vector -q "disk pressure storage warning" \
  | grep key=
```

---

## 9. analyze

Run analytical computations over a corpus identified by a key and time window.

---

### 9.1 analyze trend

Compute descriptive statistics over numeric telemetry values for a given key,
with automatic anomaly and change-point (breakout) detection.

```
bdscli [GLOBAL] analyze trend -k KEY [-d WINDOW] [--start SECS --end SECS]
```

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--key <KEY>` | `-k` | required | Metric key to analyse (e.g. `cpu.usage`). |
| `--duration <WINDOW>` | `-d` | `1h` | Lookback window. Ignored when `--start` and `--end` are both supplied. |
| `--start <SECS>` | — | — | Absolute window start as Unix seconds. Must be paired with `--end`. |
| `--end <SECS>` | — | — | Absolute window end as Unix seconds. Must be paired with `--start`. |

#### Output

```
key        : cpu.usage
window     : [1776975000, 1776978600)
samples    : 100
min / max  : 12.340000 / 98.710000
mean       : 54.823100
median     : 53.200000
std_dev    : 21.456000
variability: 0.391300  (CV)
anomalies  : 3 flagged
  [7]   ts=1776975700  value=98.710000
  [42]  ts=1776977100  value=11.230000
  [89]  ts=1776978400  value=97.500000
breakouts  : 1 detected
  [50]  ts=1776977300  value=72.000000
```

#### Examples

```bash
# Trend for cpu.usage over the last hour
bdscli -c bds.hjson analyze trend -k cpu.usage

# Trend for memory.used over the last day
bdscli -c bds.hjson analyze trend -k memory.used -d 1day

# Absolute time window
bdscli -c bds.hjson analyze trend -k cpu.usage \
  --start 1776960000 --end 1776978600
```

---

### 9.2 analyze topics

Run [Latent Dirichlet Allocation (LDA)](https://en.wikipedia.org/wiki/Latent_Dirichlet_allocation)
over the document corpus for a given key and extract the most informative
keywords per discovered topic.  Documents include both primary and secondary
records.

```
bdscli [GLOBAL] analyze topics -k KEY [-d WINDOW] [--start SECS --end SECS]
    [--k N] [--iters N] [--top-n N] [--alpha F] [--beta F] [--seed N]
```

#### Flags

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--key <KEY>` | `-k` | required | Corpus key to analyse. |
| `--duration <WINDOW>` | `-d` | `1h` | Lookback window. Ignored when `--start` and `--end` are both supplied. |
| `--start <SECS>` | — | — | Absolute window start as Unix seconds. Must be paired with `--end`. |
| `--end <SECS>` | — | — | Absolute window end as Unix seconds. Must be paired with `--start`. |
| `--k <N>` | — | `3` | Number of topics to discover. Clamped to `min(k, n_docs)` automatically. |
| `--iters <N>` | — | `200` | Gibbs sampling iterations. More iterations → more stable topics, longer runtime. |
| `--top-n <N>` | — | `10` | Number of top keywords to extract per topic. |
| `--alpha <F>` | — | `0.1` | Dirichlet prior for document-topic distributions. Lower values produce sparser (more focussed) topic assignments. |
| `--beta <F>` | — | `0.01` | Dirichlet prior for topic-word distributions. Lower values produce sparser per-topic vocabularies. |
| `--seed <N>` | — | `42` | RNG seed for reproducible topic assignments. |

#### Output

```
key      : corpus.logs
window   : [1776978167, 1776981767)
docs     : 60
topics   : 3
keywords : account, action, application, auth, category, connection, error,
           host, idx, logs, queue, refused, security, service, system, timeout
```

#### Examples

```bash
# 3-topic analysis of corpus.logs over the last hour
bdscli -c bds.hjson analyze topics -k corpus.logs --k 3

# 5-topic analysis with more iterations for stability
bdscli -c bds.hjson analyze topics -k corpus.logs \
  --k 5 --iters 500 --top-n 15 --seed 42

# Topic analysis over a specific absolute window
bdscli -c bds.hjson analyze topics -k sec.events \
  --start 1776960000 --end 1776978600 --k 4

# Check log topics over the last week
bdscli -c bds.hjson analyze topics -k log.syslog -d 7days --k 5 --iters 300
```

---

## 10. eval

Evaluate a BUND scripting-language snippet.  BUND is an embedded
stack-based language with access to BDS storage primitives.  Exactly one
source must be provided.

```
bdscli [GLOBAL] eval (--stdin | --eval EXPR | --file PATH | --url URL)
```

| Flag | Short | Description |
|------|-------|-------------|
| `--stdin` | — | Read script from stdin. |
| `--eval <EXPR>` | `-e` | Evaluate an inline expression. |
| `--file <PATH>` | `-f` | Read script from a local file. |
| `--url <URL>` | `-u` | Fetch and evaluate a script from a URL (`http`, `https`, `ftp`, `file`). |

### Examples

```bash
# Inline expression
bdscli eval --eval '2 2 + .'

# Script from a file
bdscli eval --file ./scripts/report.bund

# Script from stdin
echo '2 2 + .' | bdscli eval --stdin

# Script from a URL
bdscli eval --url https://example.com/scripts/daily-report.bund
```

---

## 11. Deduplication Concepts

BDS applies two deduplication strategies at ingest time.

### Exact-match deduplication

When an incoming document has the same `key` **and** the same serialised `data`
as a document already in the store, the existing record is returned without
creating a new row.  The submission timestamp is appended to a
`dedup_tracking` table instead.

- Use `get --duplication-timestamps` to inspect these entries.
- The stored record count is **not** incremented.

### Embedding-based (secondary) classification

When the `data` content differs but the cosine similarity between the new
document's embedding and an existing primary's embedding is ≥
`similarity_threshold` (default `0.85`), the new document is stored as a
**secondary** record linked to that primary.

- Secondaries are stored and counted in the total record set.
- Use `get --secondary --primary-id UUID` to retrieve them.
- The `similarity_threshold` is configured in `bds.hjson`.

### Reliable secondary creation in tests

Use a high-cardinality field (e.g. `"idx":"$int(1,1000000)"`) in templates
to ensure each document has unique content (bypassing exact-match dedup)
while keeping the embeddings nearly identical (triggering secondary classification).
Use `--duration 1min` to pin all documents to the same shard so the
embedding comparison always operates within a single shard.

```bash
bdscli -c bds.hjson generate templated -n 5 --duration 1min \
  --template '{"timestamp":"$timestamp","key":"test.auth","data":{"message":"login success","host":"gw-01","idx":"$int(1,1000000)"}}' \
  --ingest
```

---

## 12. Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success. |
| `1` | Any error (config not found, DB failure, invalid argument, query error). Error details are printed to stderr. |
