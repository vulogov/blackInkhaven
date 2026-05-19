# BDS — BUND Database & Document Store Words

Reference for every BUND word provided by `src/vm/stdlib/db`. Words are split into two groups:

- **Shard DB** (`db.*`) — telemetry/event storage: vector search, full-text search, and aggregated cross-store search over time-windowed shards.
- **Document Store** (`doc.*`) — persistent document store: add, update, delete, retrieve, and search documents by text, JSON, or pre-computed embedding vectors.

**Stack-effect notation:** `( before -- after )` where the top of the stack is on the right.  
`W:x` denotes a value on the **workbench** instead of the stack.  
A trailing `.` on a word name means the word reads from and writes to the **workbench** instead of the main stack.

---

## Table of Contents

1. [Shard DB — Ingest](#1-shard-db--ingest)
2. [Shard DB — Search](#2-shard-db--search)
3. [Shard DB — Aggregation Search](#3-shard-db--aggregation-search)
4. [Shard DB — Sync](#4-shard-db--sync)
5. [Document Store — Add](#5-document-store--add)
6. [Document Store — Update & Store Vectors](#6-document-store--update--store-vectors)
7. [Document Store — Delete](#7-document-store--delete)
8. [Document Store — Retrieve](#8-document-store--retrieve)
9. [Document Store — Search (full results)](#9-document-store--search-full-results)
10. [Document Store — Search (fingerprint strings)](#10-document-store--search-fingerprint-strings)
11. [Document Store — Sync & Reindex](#11-document-store--sync--reindex)

---

## 1. Shard DB — Ingest

### `db.add` / `db.add.`

```
( doc:MAP -- id:STRING )
( W:doc:MAP -- W:id:STRING )
```

Converts `doc` to JSON and stores it in the time-series shard DB. Pushes the assigned record UUID as a STRING.

```bund
{ "host" "web01" "level" "error" "msg" "connection refused" } db.add
// stack: "550e8400-e29b-41d4-a716-446655440000"
```

---

## 2. Shard DB — Search

### `db.search` / `db.search.`

```
( query:any  duration:STRING -- results:LIST )
( W:query:any  W:duration:STRING -- W:results:LIST )
```

Vector similarity search over shards within the given time window. `query` may be a STRING or a MAP — both are converted to JSON and embedded automatically. `duration` is a time-window string such as `"1h"`, `"30m"`, `"24h"`. Returns a LIST of result MAPs.

```bund
{ "level" "error" } "1h" db.search
// stack: [ {…}, {…}, … ]
```

### `db.fulltext` / `db.fulltext.`

```
( query:STRING  duration:STRING -- results:LIST )
( W:query:STRING  W:duration:STRING -- W:results:LIST )
```

Full-text (Tantivy) search over shards within the given time window. `query` is a plain search string. Returns a LIST of result MAPs.

```bund
"nginx upstream timeout" "6h" db.fulltext
// stack: [ {…}, {…}, … ]
```

---

## 3. Shard DB — Aggregation Search

### `db.aggregation.search` / `db.aggregation.search.`

```
( query:STRING  duration:STRING -- result:MAP )
( W:query:STRING  W:duration:STRING -- W:result:MAP )
```

Runs a telemetry vector search and a document-store semantic search **concurrently** (via Rayon) using the same plain-text `query`, then merges both result sets into a single MAP with two keys:

| Key | Type | Contents |
|---|---|---|
| `"observability"` | LIST of MAPs | Telemetry records from the shard DB, vector-ranked by `_score` descending. Each record includes a `"_score"` field and an embedded `"secondaries"` array. |
| `"documents"` | LIST of MAPs | Document-store hits from the semantic search (up to 10). Each hit carries `id`, `metadata`, `document`, and `score`. |

`duration` is a lookback window for the telemetry side only (e.g. `"1h"`, `"30min"`, `"7days"`). The document-store search is global — it is not filtered by time.

The query string is fingerprinted and embedded with the shared `AllMiniLML6V2` model before being passed to the HNSW indexes on both sides. If either search returns an error the word raises an error and nothing is pushed.

```bund
"connection pool exhaustion" "1h" db.aggregation.search
// stack: {
//   "observability": [ { "_score": 0.92, "host": "web01", … }, … ],
//   "documents":     [ { "id": "d2b4…", "score": 0.87, … }, … ]
// }
```

Workbench variant — useful in pipelines that pass results directly to other `.`-suffixed words:

```bund
"nginx upstream timeout" "6h" db.aggregation.search.
// workbench holds the result MAP
```

---

## 4. Shard DB — Sync

### `db.sync`

```
( -- true )
```

Flushes all pending shard-DB writes to disk (DuckDB CHECKPOINT). Pushes `true` on success; raises an error on failure. No workbench variant.

```bund
db.sync drop   // flush and discard the confirmation flag
```

---

## 5. Document Store — Add

### `doc.add` / `doc.add.`

```
( metadata:MAP  content:STRING -- id:STRING )
( W:metadata:MAP  W:content:STRING -- W:id:STRING )
```

Adds a new document. `metadata` is a MAP stored as JSON. `content` is the UTF-8 document body. The store automatically generates embeddings for both. Pushes the new document UUID as a STRING.

```bund
{ "title" "Release notes" "version" "1.2" }
"This release fixes the connection-pool exhaustion bug."
doc.add
// stack: "d2b4a1…"
```

### `doc.add.file` / `doc.add.file.`

```
( path:STRING  name:STRING  slice:INT  overlap:FLOAT -- id:STRING )
( W:path:STRING  W:name:STRING  W:slice:INT  W:overlap:FLOAT -- W:id:STRING )
```

Reads a file from `path`, slices it into chunks of `slice` characters with `overlap` fractional overlap (0.0–1.0), embeds each chunk, and stores the whole document. `name` is stored as the document title in metadata. Pushes the root document UUID.

| Argument | Type | Meaning |
|---|---|---|
| `path` | STRING | Filesystem path to the file |
| `name` | STRING | Document name stored in metadata |
| `slice` | INT | Chunk size in characters |
| `overlap` | FLOAT | Fractional overlap between chunks (0.0 = none, 0.5 = 50 %) |

```bund
"/var/log/app.log" "app.log" 512 0.1 doc.add.file
```

### `doc.add.vec` / `doc.add.vec.`

```
( metadata:MAP  content:STRING  meta_vec:LIST  content_vec:LIST -- id:STRING )
( W:metadata:MAP  W:content:STRING  W:meta_vec:LIST  W:content_vec:LIST -- W:id:STRING )
```

Adds a document with pre-computed embedding vectors, bypassing the built-in embedder. `meta_vec` and `content_vec` are LISTs of FLOATs. Useful when embeddings are computed externally or in a different pipeline stage.

```bund
{ "source" "api" }
"Error: rate limit exceeded"
[ 0.12 0.84 … ]   // meta embedding
[ 0.03 0.77 … ]   // content embedding
doc.add.vec
```

---

## 6. Document Store — Update & Store Vectors

### `doc.update.metadata` / `doc.update.metadata.`

```
( id:STRING  metadata:MAP -- true )
( W:id:STRING  W:metadata:MAP -- W:true )
```

Replaces the stored metadata JSON for document `id` with the new `metadata` MAP. Pushes `true` on success.

```bund
"d2b4a1…" { "title" "Release notes v2" "reviewed" true } doc.update.metadata
```

### `doc.update.content` / `doc.update.content.`

```
( id:STRING  content:STRING -- true )
( W:id:STRING  W:content:STRING -- W:true )
```

Replaces the stored content bytes for document `id`. The HNSW index is **not** automatically rebuilt — call `doc.reindex` afterwards if search results must reflect the new content. Pushes `true` on success.

```bund
"d2b4a1…" "Updated body text after editorial review." doc.update.content
doc.reindex drop
```

### `doc.store.meta.vec` / `doc.store.meta.vec.`

```
( id:STRING  meta_vec:LIST  metadata:MAP -- true )
( W:id:STRING  W:meta_vec:LIST  W:metadata:MAP -- W:true )
```

Stores a pre-computed metadata embedding vector together with an updated metadata MAP for document `id`. Useful for refreshing vectors without re-ingesting the full document.

```bund
"d2b4a1…" [ 0.12 0.84 … ] { "title" "Updated" } doc.store.meta.vec
```

### `doc.store.content.vec` / `doc.store.content.vec.`

```
( id:STRING  content_vec:LIST -- true )
( W:id:STRING  W:content_vec:LIST -- W:true )
```

Stores a pre-computed content embedding vector for document `id` without touching the content bytes or metadata.

```bund
"d2b4a1…" [ 0.03 0.77 … ] doc.store.content.vec
```

---

## 7. Document Store — Delete

### `doc.delete` / `doc.delete.`

```
( id:STRING -- true )
( W:id:STRING -- W:true )
```

Permanently removes document `id` from all stores (metadata, content, vectors). Pushes `true` on success. The HNSW index retains a tombstone entry until the next `doc.reindex`.

```bund
"d2b4a1…" doc.delete drop
```

---

## 8. Document Store — Retrieve

### `doc.get.metadata` / `doc.get.metadata.`

```
( id:STRING -- MAP | null )
( W:id:STRING -- W:MAP | W:null )
```

Retrieves the metadata MAP for document `id`. Pushes `null` (nodata) if the document does not exist.

```bund
"d2b4a1…" doc.get.metadata
// stack: { "title" "Release notes" "version" "1.2" }
```

### `doc.get.content` / `doc.get.content.`

```
( id:STRING -- STRING | null )
( W:id:STRING -- W:STRING | W:null )
```

Retrieves the raw content bytes for document `id`, decoded as UTF-8, and pushes them as a STRING. Pushes `null` (nodata) if the document does not exist.

```bund
"d2b4a1…" doc.get.content println
```

---

## 9. Document Store — Search (full results)

All search words in this section return a **LIST of MAPs** — each MAP is the full stored JSON document record for a matching document.

### `doc.search` / `doc.search.`

```
( query:STRING  limit:INT -- results:LIST )
( W:query:STRING  W:limit:INT -- W:results:LIST )
```

Embeds `query` using the built-in text embedder and performs HNSW vector search over document content. Returns up to `limit` result MAPs ranked by similarity.

```bund
"connection pool exhaustion" 5 doc.search
```

### `doc.search.json` / `doc.search.json.`

```
( query:MAP  limit:INT -- results:LIST )
( W:query:MAP  W:limit:INT -- W:results:LIST )
```

Converts the `query` MAP to a JSON fingerprint string, embeds it, and performs HNSW search over document metadata vectors. Returns up to `limit` result MAPs.

```bund
{ "level" "error" "host" "web01" } 10 doc.search.json
```

### `doc.search.vec` / `doc.search.vec.`

```
( query_vec:LIST  limit:INT -- results:LIST )
( W:query_vec:LIST  W:limit:INT -- W:results:LIST )
```

Performs HNSW search using a pre-computed embedding vector. `query_vec` is a LIST of FLOATs. Returns up to `limit` result MAPs.

```bund
[ 0.03 0.77 … ] 5 doc.search.vec
```

---

## 10. Document Store — Search (fingerprint strings)

These words return a **LIST of STRINGs** — the raw fingerprint string for each matching document — instead of full result MAPs. Useful for lightweight lookups or deduplication pipelines.

### `doc.search.strings` / `doc.search.strings.`

```
( query:STRING  limit:INT -- fingerprints:LIST )
( W:query:STRING  W:limit:INT -- W:fingerprints:LIST )
```

Text query → HNSW search over content vectors → list of fingerprint strings.

```bund
"nginx upstream timeout" 10 doc.search.strings
```

### `doc.search.json.strings` / `doc.search.json.strings.`

```
( query:MAP  limit:INT -- fingerprints:LIST )
( W:query:MAP  W:limit:INT -- W:fingerprints:LIST )
```

JSON MAP query → fingerprint → HNSW search over metadata vectors → list of fingerprint strings.

```bund
{ "service" "auth" } 5 doc.search.json.strings
```

### `doc.search.vec.strings` / `doc.search.vec.strings.`

```
( query_vec:LIST  limit:INT -- fingerprints:LIST )
( W:query_vec:LIST  W:limit:INT -- W:fingerprints:LIST )
```

Pre-computed vector → HNSW search → list of fingerprint strings.

```bund
[ 0.12 0.84 … ] 5 doc.search.vec.strings
```

---

## 11. Document Store — Sync & Reindex

### `doc.sync`

```
( -- true )
```

Flushes the document store HNSW index to disk. Pushes `true` on success. Call after bulk ingestion to ensure durability. No workbench variant.

```bund
doc.sync drop
```

### `doc.reindex` / `doc.reindex.`

```
( -- count:INT )
( W: -- W:count:INT )
```

Rebuilds the HNSW index from scratch using all persisted document vectors. Returns the number of documents indexed as an INT. Use after `doc.update.content`, `doc.delete`, or any bulk operation that modifies stored vectors.

```bund
doc.reindex println   // prints number of indexed documents
```

---

## Quick-Reference Table

| Word | Stack ( before -- after ) | Description |
|---|---|---|
| `db.add` | `( doc -- id )` | Ingest document into shard DB |
| `db.search` | `( query duration -- results )` | Vector search over time window |
| `db.fulltext` | `( query duration -- results )` | Full-text search over time window |
| `db.aggregation.search` | `( query duration -- result )` | Parallel telemetry + doc-store search, merged MAP |
| `db.sync` | `( -- true )` | Flush shard DB to disk |
| `doc.add` | `( meta content -- id )` | Add document, auto-embed |
| `doc.add.file` | `( path name slice overlap -- id )` | Add document from file |
| `doc.add.vec` | `( meta content meta_vec content_vec -- id )` | Add document with pre-computed vectors |
| `doc.update.metadata` | `( id meta -- true )` | Replace document metadata |
| `doc.update.content` | `( id content -- true )` | Replace document content bytes |
| `doc.store.meta.vec` | `( id meta_vec meta -- true )` | Store metadata embedding vector |
| `doc.store.content.vec` | `( id content_vec -- true )` | Store content embedding vector |
| `doc.delete` | `( id -- true )` | Delete document from all stores |
| `doc.get.metadata` | `( id -- MAP\|null )` | Retrieve metadata MAP |
| `doc.get.content` | `( id -- STRING\|null )` | Retrieve content as UTF-8 string |
| `doc.search` | `( query limit -- results )` | Text → content vector search → MAPs |
| `doc.search.json` | `( query limit -- results )` | MAP → metadata vector search → MAPs |
| `doc.search.vec` | `( vec limit -- results )` | Pre-computed vector search → MAPs |
| `doc.search.strings` | `( query limit -- fingerprints )` | Text → content vector search → strings |
| `doc.search.json.strings` | `( query limit -- fingerprints )` | MAP → metadata vector search → strings |
| `doc.search.vec.strings` | `( vec limit -- fingerprints )` | Pre-computed vector search → strings |
| `doc.sync` | `( -- true )` | Flush HNSW index to disk |
| `doc.reindex` | `( -- count )` | Rebuild HNSW index from persisted vectors |

Every word except `db.sync` and `doc.sync` has a `.`-suffixed workbench variant with identical semantics.
