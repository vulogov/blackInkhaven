# Primary / Secondary record classification (`bdslib::observability`)

When a telemetry record arrives at bdslib it is **classified as primary
or secondary** before being stored. This is the deduplication mechanism
that makes the storage subsystem viable on real-world log streams: a
naïve "store everything" approach would have indexes choke on the
millions of near-duplicate lines a single noisy service can emit per
hour.

The classifier sits inside [`ObservabilityStorage::add`] and is the
*first* substantive thing that happens to a record after timestamp
parsing. Every downstream subsystem — full-text search, vector search,
template mining, RCA — sees only **primary records as first-class
citizens**, with secondaries available on demand as embedded arrays
inside their parent primary.

This document covers:

1. [What problem primary/secondary solves](#1-what-problem-primarysecondary-solves)
2. [The classification rules](#2-the-classification-rules)
3. [How bdslib implements it](#3-how-bdslib-implements-it)
4. [The full pipeline, step by step](#4-the-full-pipeline-step-by-step)
5. [Schema and on-disk layout](#5-schema-and-on-disk-layout)
6. [Configuration knobs](#6-configuration-knobs)
7. [Complexity and scaling](#7-complexity-and-scaling)
8. [Determinism and idempotency](#8-determinism-and-idempotency)
9. [Worked examples](#9-worked-examples)
10. [Failure modes and edge cases](#10-failure-modes-and-edge-cases)
11. [References](#11-references)

[`ObservabilityStorage::add`]: ../../src/observability.rs

---

## 1. What problem primary/secondary solves

Production log streams have one defining property: **they're heavily
redundant**. The same nginx 502 message fires 1,000 times in a minute.
The same Python traceback repeats with only the timestamp changing. The
same Kubernetes pod evicts itself, comes back, and emits the same boot
sequence verbatim. If you store every record as an independent first-class
entity:

- Vector indexes (HNSW) bloat by 100×–1000× without learning anything
  new about the corpus.
- Full-text indexes (Tantivy) get hammered with insertion writes that
  return the same document body over and over.
- Search results are dominated by the loudest log line. The user types
  "upstream timeout" and the top 100 hits are all the same message
  with different timestamps.
- The "what is this batch about?" algorithms (TextRank, LSA, k-NN)
  surface the noisiest line as the most central, defeating the
  summarisation goal.

The fix is to recognise that there are really only **two relevant
questions** about any incoming record:

1. **Is this exactly the same as something we already have?** If yes,
   record the new timestamp on the deduplication log and don't index
   it at all. The existing UUID is returned.
2. **Is this *similar enough* to something we already have?** If yes,
   store it as a *secondary* — a leaf attached to an existing
   *primary* — and skip the search indexes.

Otherwise it's a genuinely new pattern and it becomes a new primary
that *does* get indexed.

This is the contract the rest of bdslib relies on. Search returns
primaries with secondaries embedded; analysis algorithms ingest
primaries and treat the secondary count as a "popularity" signal;
storage costs scale with *distinct patterns*, not raw event volume.

---

## 2. The classification rules

For every incoming record `(key, data, timestamp, …)`:

```
if exact (key, data_text) already exists in the store:
    → DUPLICATE
       - append timestamp to dedup_tracking[(key, data_text)].timestamps
       - return existing UUID, is_primary=false
       - do NOT re-index in FTS or vector store

else:
    embedding = embed("key: " + key + " " + data_text)
    best_sim, best_primary_id = nearest_primary(embedding)

    if best_sim >= similarity_threshold:
        → SECONDARY
           - store row in `telemetry` with is_primary=0
           - link to best_primary_id in `primary_secondary`
           - return new UUID, is_primary=false
           - do NOT add to FTS or vector index

    else:
        → PRIMARY
           - store row in `telemetry` with is_primary=1
           - store embedding in `primary_embeddings`
           - return new UUID, is_primary=true
           - caller adds to FTS and vector index
```

Three distinct outcomes, three distinct downstream behaviours:

| Outcome | DB rows touched | Embedding stored? | FTS / Vector indexed? | UUID returned |
|---|---|---|---|---|
| Duplicate | `dedup_tracking` only | No | No | existing primary's UUID |
| Secondary | `telemetry` + `primary_secondary` | No | No | freshly minted UUID |
| Primary | `telemetry` + `primary_embeddings` | Yes | Yes (by caller) | freshly minted UUID |

The asymmetry is intentional: secondaries are cheap (one INSERT, no
embedding work after the fact), primaries are expensive (embedding +
two index writes), duplicates are cheapest of all (one UPDATE on a
JSON array).

---

## 3. How bdslib implements it

[`ObservabilityStorage`] holds:

- A [`StorageEngine`] DuckDB pool with the schema in
  [§ 5](#5-schema-and-on-disk-layout).
- An [`EmbeddingEngine`] (fastembed AllMiniLML6V2 by default — 384-dim
  cosine-friendly vectors).
- A configurable `similarity_threshold` (cosine, default 0.85).
- A **lazy-loaded in-memory cache of every primary's embedding** —
  `Arc<Mutex<Option<Vec<(Uuid, Vec<f32>)>>>>`. The first classification
  call loads the cache from `primary_embeddings`; subsequent calls
  classify entirely in memory. New primaries are appended to the cache
  in the same lock acquisition.

Everything pluggable about the classifier is one of these three
components: change the similarity metric (currently cosine), change the
embedding model, or change the threshold.

The default `0.85` cosine threshold was chosen by tuning against real
log streams: lower (0.7) lumps together messages that mention different
services with similar wording; higher (0.95) treats every minor
variation (a different IP, a different transaction id) as a new
primary, defeating the deduplication goal.

[`ObservabilityStorage`]: ../../src/observability.rs
[`StorageEngine`]: ../../src/storageengine.rs
[`EmbeddingEngine`]: ../../src/embedding.rs

---

## 4. The full pipeline, step by step

### Step 1 — Validate mandatory fields

Every record must contain `timestamp`, `key`, and `data`. The `id`
field is optional; when absent a UUIDv7 is generated **at the
record's timestamp**, so v7's monotonic-time prefix lines up with the
event time it represents.

```rust
let ts = parse_timestamp(doc.get("timestamp")?)?;
let key = doc.get("key").and_then(|v| v.as_str())
    .ok_or_else(|| err_msg("missing or non-string mandatory field 'key'"))?
    .to_string();
let data = doc.get("data")
    .ok_or_else(|| err_msg("missing mandatory field 'data'"))?
    .clone();

let id = if let Some(s) = doc.get("id").and_then(|v| v.as_str()) {
    Uuid::parse_str(s)?
} else {
    generate_v7_at(UNIX_EPOCH + Duration::from_secs(ts as u64))
};
```

### Step 2 — Build the deduplication signature

`data_to_text` flattens the `data` payload into a canonical string so
`(key, data_text)` is a stable equality signature regardless of JSON
quoting:

```rust
fn data_to_text(data: &JsonValue) -> String {
    match data {
        JsonValue::String(s) => s.clone(),
        JsonValue::Number(n) => n.to_string(),
        JsonValue::Bool(b)   => b.to_string(),
        JsonValue::Null      => String::new(),
        other                => json_fingerprint(other),
    }
}
```

For object/array shapes, `json_fingerprint` produces the same
`"field: value"` flattening used by the LDA pipeline — sorted by key
within each level, so two structurally-identical payloads always
produce the same fingerprint regardless of HashMap iteration order.

The `data_text` column then becomes the dedup key. The `(key,
data_text)` composite is stored verbatim in the `telemetry` table and
indexed (see § 5) so duplicate detection is a single SELECT.

### Step 3 — Exact-match dedup

A `SELECT id FROM telemetry WHERE key = ? AND data_text = ?` decides
whether this exact record body has been seen before:

```rust
let existing = self.engine.select_all(&format!(
    "SELECT id FROM telemetry WHERE key = '{}' AND data_text = '{}'",
    sql_escape(&key),
    sql_escape(&data_text),
))?;

if let Some(row) = existing.into_iter().next() {
    let existing_id = parse_uuid_field(row, 0, "telemetry.id")?;
    self.record_duplicate(&key, &data_text, ts)?;
    return Ok((existing_id, false, None));
}
```

If found, the new timestamp is appended to the
`dedup_tracking[(key, data_text)].timestamps` JSON array — the
"how many times has this fired, when?" log — and the existing UUID is
returned without further work. **The caller must not re-index it**:
the third return value `Option<Vec<f32>>` is `None`, signalling
"already classified, take no action".

### Step 4 — Embed the data for similarity classification

A novel `(key, data_text)` triggers an embedding pass. The embedding
input deliberately includes the key:

```rust
let embed_input = format!("key: {key} {data_text}");
let embedding = self.embedding.embed(&embed_input)?;
```

This prevents two structurally-similar but semantically-different
records from collapsing — `key: cpu.usage 42` and `key: mem.free 42`
have the same `data_text` but should never be considered the same
pattern. By prepending `"key: <key>"` we anchor the embedding to the
metric / event identity.

### Step 5 — Cosine-classify against existing primaries

The classifier scans the in-memory primary embedding cache:

```rust
fn classify_in_memory(
    entries: &[(Uuid, Vec<f32>)],
    embedding: &[f32],
    threshold: f32,
) -> Result<(bool, Option<Uuid>)> {
    if entries.is_empty() {
        return Ok((true, None));   // first record ever — is_primary = true
    }
    let mut best_sim = f32::NEG_INFINITY;
    let mut best_id: Option<Uuid> = None;
    for (pid, prim_emb) in entries {
        let sim = cosine_similarity(embedding, prim_emb)?;
        if sim > best_sim {
            best_sim = sim;
            best_id  = Some(*pid);
        }
    }
    if best_sim >= threshold {
        Ok((false, best_id))      // SECONDARY — link to best_id
    } else {
        Ok((true, None))          // PRIMARY — new pattern
    }
}
```

Cosine is the right metric here for two reasons:

1. **Magnitude invariance.** A long log line and a short log line that
   share the same gist should match.
2. **Numerically stable for high-dim sparse vectors.** The 384-dim
   AllMiniLML6V2 outputs cluster on the unit sphere by construction,
   so cosine and dot product are numerically equivalent.

The lock is held for the full scan and the cache update, ensuring no
two concurrent `add` calls can both classify a near-identical record
as primary and then race to insert distinct primaries.

### Step 6 — Insert the row

Build the INSERT statement based on the classification:

```rust
self.engine.execute(&format!(
    "INSERT INTO telemetry (id, ts, key, data, metadata, data_text, is_primary) \
     VALUES ('{id}', {ts}, '{}', '{}'::JSON, '{}'::JSON, '{}', {})",
    sql_escape(&key),
    sql_escape(&data_s),
    sql_escape(&meta_s),
    sql_escape(&data_text),
    if is_primary { 1 } else { 0 },
))?;

if is_primary {
    let hex = to_hex(&embedding_to_bytes(&embedding));
    self.engine.execute(&format!(
        "INSERT INTO primary_embeddings VALUES ('{id}', from_hex('{hex}'))"
    ))?;
    Ok((id, true, Some(embedding)))   // caller will index in FTS + Vector
} else {
    let primary_id = similar_primary.unwrap();
    self.engine.execute(&format!(
        "INSERT INTO primary_secondary VALUES ('{primary_id}', '{id}', {ts})"
    ))?;
    Ok((id, false, None))             // caller does NOT index
}
```

Note the third tuple element: `Some(embedding)` for primaries, `None`
otherwise. The caller (typically [`Shard::add`]) uses this signal to
decide whether to push the record into the FTS and vector indexes.
This avoids re-embedding the same text in two places.

[`Shard::add`]: ../../src/shard.rs

### Step 7 — The batch-add fast path

`add_batch` is the same algorithm with three optimisations:

1. **One ONNX pass for every new record's embedding.** All `embed`
   calls are coalesced into one `embed_batch`, which is dramatically
   faster than per-record inference because of how transformer models
   are batch-friendly on CPU and GPU.
2. **Intra-batch dedup map** so two records in the same batch with the
   same `(key, data_text)` collapse without round-tripping the DB.
3. **Single transaction for all inserts.** All telemetry rows,
   primary-embedding rows, and primary-secondary links go in one
   `BEGIN … COMMIT`. The classification result for each record was
   pre-computed under one cache lock; no I/O during the lock window.

The result type is unchanged — `Vec<(Uuid, bool, Option<Vec<f32>>)>` —
so the caller can treat batch results identically to single results.

---

## 5. Schema and on-disk layout

The observability schema lives in three tables plus one auxiliary:

### `telemetry` — every accepted record

```sql
CREATE TABLE telemetry (
    id         TEXT    NOT NULL PRIMARY KEY,   -- UUIDv7
    ts         BIGINT  NOT NULL,                -- Unix seconds
    key        TEXT    NOT NULL,                -- signal / metric name
    data       JSON    NOT NULL,                -- the original `data` payload
    metadata   JSON    NOT NULL,                -- everything else from the document
    data_text  TEXT    NOT NULL,                -- canonicalised dedup signature
    is_primary INTEGER NOT NULL                 -- 0 or 1
);
CREATE INDEX idx_tel_ts         ON telemetry (ts);
CREATE INDEX idx_tel_key_data   ON telemetry (key, data_text);   -- dedup lookup
CREATE INDEX idx_tel_primary_ts ON telemetry (is_primary, ts);   -- primary timeline scans
CREATE INDEX idx_tel_key_ts     ON telemetry (key, ts);          -- per-key range queries
```

`is_primary` is the bit that everything else keys off. The
`(is_primary, ts)` index lets cross-shard primary scans filter to just
primaries without touching secondary rows.

### `primary_embeddings` — one row per primary

```sql
CREATE TABLE primary_embeddings (
    primary_id TEXT NOT NULL PRIMARY KEY,
    embedding  BLOB NOT NULL                    -- f32 LE bytes, 384 floats
);
```

The embedding is stored as raw little-endian bytes (4 × 384 = 1536
bytes per row). It's used solely as the source-of-truth for the
in-memory primary cache; runtime classification is from the cache, not
from this table.

### `primary_secondary` — link table

```sql
CREATE TABLE primary_secondary (
    primary_id   TEXT   NOT NULL,
    secondary_id TEXT   NOT NULL,
    ts           BIGINT NOT NULL,
    PRIMARY KEY (primary_id, secondary_id)
);
CREATE INDEX idx_ps_primary_ts ON primary_secondary (primary_id, ts);
CREATE INDEX idx_ps_secondary  ON primary_secondary (secondary_id);
```

One row per secondary, pointing at its parent primary. The
`(primary_id, ts)` index supports the "give me all secondaries for
this primary, time-ordered" query that bdsweb's drill-down view uses.
The `(secondary_id)` index supports the inverse lookup.

### `dedup_tracking` — per-pattern timestamp log

```sql
CREATE TABLE dedup_tracking (
    key       TEXT NOT NULL,
    data_text TEXT NOT NULL,
    timestamps JSON NOT NULL,                   -- JSON array of i64 Unix seconds
    PRIMARY KEY (key, data_text)
);
CREATE INDEX idx_dedup_key ON dedup_tracking (key);
```

Every duplicate event appends its timestamp to this JSON array. This
is the "how many times has this exact pattern fired, when?" log — the
backbone of cadence analysis (`v2/duplicates`) and the input to the
template-frequency tracker.

---

## 6. Configuration knobs

```rust
pub struct ObservabilityStorageConfig {
    pub similarity_threshold: f32,    // default: 0.85
}
```

| Knob | Effect |
|---|---|
| **`similarity_threshold`** | The cosine cutoff for primary-vs-secondary classification. Higher (0.95) ⇒ more primaries, secondaries only for very-near-duplicates; lower (0.70) ⇒ fewer primaries, broader secondaries. Range `[0, 1]`. The default 0.85 balances "merge near-duplicate log lines" against "don't merge two genuinely different signals". |

Tuning advice:

- **For binary-numeric telemetry** (`cpu.usage`, `mem.free`, …) the
  threshold barely matters — the data fingerprints are tiny strings
  ("`0.72`", "`81.5`") and the embedding model treats numeric
  fingerprints as nearly orthogonal anyway. Almost everything ends up
  primary.
- **For free-text logs** the threshold matters a lot. 0.85 catches
  "same template, different parameters"; 0.95 only catches
  byte-identical messages (which the exact-match dedup already
  handles).
- **For drain3-template observations** the threshold is irrelevant —
  drain3 emits canonical template bodies, so two observations of the
  same template have `data_text` equality and never reach the
  embedding step.

---

## 7. Complexity and scaling

For each `add` call:

| Phase | Cost |
|---|---|
| Validate + extract fields | `O(|doc|)` |
| Build `data_text` (json_fingerprint) | `O(|data|)` linear in tree size |
| Exact-match dedup query | `O(log n)` index lookup on `(key, data_text)` |
| Embed (when novel) | one ONNX inference call, ~5–15 ms on CPU |
| Classify against primary cache | `O(P)` where `P` is the number of primaries |
| INSERT row | `O(1)` |
| INSERT primary_secondary or primary_embeddings | `O(1)` |

The cosine scan dominates when primaries are many. With 10⁴
primaries (~10 MB cache), a 384-dim cosine scan is ~5 ms — comparable
to a single embedding call. With 10⁵+ primaries the linear scan
becomes the bottleneck and you'd want an HNSW-backed primary index
instead — but bdslib's shard architecture caps a single shard's
primary count at "what fits in one time bucket", so this rarely bites.

For `add_batch` of `B` documents:

| Phase | Cost |
|---|---|
| Validate + intra-batch dedup | `O(B)` |
| DB dedup queries | `O(B · log n)` |
| Single batched embed call | one ONNX pass over `B` inputs (much cheaper than `B` separate calls) |
| Classify all under one cache lock | `O(B · P)` |
| Single transaction insert | `O(B)` |

Memory:

| Structure | Size |
|---|---|
| In-memory primary cache | `O(P × 384 × 4 bytes)` ≈ 1.5 KB per primary |
| `dedup_tracking.timestamps` JSON arrays | unbounded — grows with duplicate count |

The unbounded growth of dedup-tracking arrays is intentional: the
whole point is to know how often each pattern has fired. For
operationally bounded retention, time-partition the data via shards
(each shard has its own `dedup_tracking` table) and let old shards
drop off naturally.

---

## 8. Determinism and idempotency

The classifier is **idempotent** in the practical sense: storing the
same record twice is a no-op except for an extra timestamp in the
dedup log. This is the single most important property the rest of
bdslib depends on — replays are safe, retries are safe, double-deliveries
are safe.

It is **deterministic** in the more technical sense given:

- A deterministic embedding model (yes — fastembed inference is
  reproducible per-model-revision).
- A deterministic order of inserts (i.e. a single-threaded ingestor or
  a stream pre-sorted by timestamp).

Concurrent inserts are *correct* but not bit-exact deterministic: when
two threads simultaneously add records that would each be the first
"new pattern", whichever wins the cache lock becomes the primary and
the other becomes a secondary linking to it. Both orderings produce a
valid dedup tree, but the specific primary UUIDs differ.

If exact reproducibility across concurrent runs matters (rare —
usually only test fixtures), pin pool size to 1 or use the
single-threaded `add` path with explicit timestamps.

---

## 9. Worked examples

### Example A — three records, three outcomes

Given an empty store and these three records arriving in order:

```json
{ "key": "log.web", "timestamp": 1745000000, "data": { "raw": "nginx upstream timeout" } }
{ "key": "log.web", "timestamp": 1745000010, "data": { "raw": "nginx upstream timeout" } }
{ "key": "log.web", "timestamp": 1745000020, "data": { "raw": "nginx upstream connection refused" } }
```

Step through the classifier:

1. **Record 1.**
   - No exact match (store empty).
   - Embedding is computed; no existing primaries, so classification
     trivially returns `(is_primary=true, similar=None)`.
   - Inserted to `telemetry` with `is_primary=1`. Embedding stored in
     `primary_embeddings`. Returns `(uuid_1, true, Some(emb_1))`.
2. **Record 2.**
   - Exact `(key, data_text)` match — same `"nginx upstream timeout"`.
   - Timestamp `1745000010` appended to
     `dedup_tracking[("log.web", "nginx upstream timeout")].timestamps`.
   - Returns `(uuid_1, false, None)` — same UUID as record 1, no
     re-indexing.
3. **Record 3.**
   - No exact match (different `data_text`).
   - Embedding computed. Cosine similarity to record 1's primary is
     ~0.92 (nginx + upstream + connection-related vocabulary in common).
   - 0.92 ≥ 0.85, so secondary. Inserted to `telemetry` with
     `is_primary=0`, linked to `uuid_1` in `primary_secondary`.
     Returns `(uuid_3, false, None)`.

Final state:

```
telemetry:
  uuid_1, ts=1745000000, key=log.web, data="...timeout",  is_primary=1
  uuid_3, ts=1745000020, key=log.web, data="...refused",  is_primary=0

primary_embeddings:
  uuid_1, <384-float blob>

primary_secondary:
  uuid_1, uuid_3, ts=1745000020

dedup_tracking:
  ("log.web", "nginx upstream timeout"): [1745000010]
```

A search for `"nginx"` finds `uuid_1` (the only primary) and returns
it with `secondaries: [uuid_3]` embedded.

### Example B — distinguishing keys with identical data

```json
{ "key": "cpu.usage", "timestamp": 1745000000, "data": 0.72 }
{ "key": "mem.free",  "timestamp": 1745000000, "data": 0.72 }
```

Both records have `data_text = "0.72"`, so the exact-match dedup
**does not fire** — the dedup index is keyed on `(key, data_text)`, not
on `data_text` alone. Both proceed to the embedding step.

The embedding inputs are:

```
"key: cpu.usage 0.72"
"key: mem.free 0.72"
```

These are textually distinct strings; their embeddings are well
below threshold. Both records become primaries, attached to two
different rows of `primary_embeddings`. This is the right outcome —
they're genuinely different signals that just happen to share a numeric
value.

### Example C — a noisy nginx log over an hour

A service emits 50,000 nginx access-log lines in an hour. After bdslib
ingest:

- ~10 distinct primaries (one per HTTP-status / route-pattern combo).
- ~40,000 secondaries (the structurally-similar variations within each
  pattern).
- ~10,000 dedup hits (byte-identical lines with different timestamps).

Vector and FTS indexes contain only the 10 primaries. The
`v2/fulltext` query for `502` returns the single primary that
represents the 502-error pattern, with the timestamps of every
secondary instance available via `secondary_ids` on the result.

This 5,000× compression is what makes bdslib's storage costs grow with
*pattern count*, not raw event volume.

---

## 10. Failure modes and edge cases

| Input | Behaviour |
|---|---|
| Missing `timestamp`/`key`/`data` | `Err` from `add`/`add_batch`. The record is rejected; nothing is stored. |
| `id` field present but invalid UUID | `Err`; rejected. |
| `id` field present and valid | Used verbatim — caller-supplied UUIDs override v7 generation. Useful for replay scenarios. |
| `data` is `null` | Allowed; `data_text = ""`. Two `null`-data records under the same key dedup against each other. |
| `data` is a structurally-different but textually-equivalent JSON object | The `json_fingerprint` flattening is order-stable, so the dedup signature is the same. They dedup. |
| Empty store, first record | Trivially classified as primary. |
| Very high `similarity_threshold` (≥ 0.99) | Almost everything becomes a primary; vector and FTS indexes grow ~linearly with input. |
| Very low `similarity_threshold` (≤ 0.5) | Almost everything becomes a secondary of the first-ever primary; the system stops discriminating between distinct patterns. |
| Concurrent inserts of near-identical records | Both eventually settle to "one primary, the other a secondary"; which one is primary depends on lock ordering. The primary cache is updated under the lock so no race produces two duplicate primaries. |
| Embedding model unavailable | `Err` from `embed`; the record is rejected. The store remains in a consistent state — no row was inserted. |

The `add`/`add_batch` calls return `Result<…>`; transient errors propagate as `Err`. The classifier never panics on user-supplied input.

---

## 11. References

- Goldberg, J., et al. (2007). *Stable distributions, pseudorandom
  generators, embeddings, and data stream computation.* Journal of the
  ACM, 53(3) — for the embedding-then-classify pattern used here as a
  Locality-Sensitive Hashing (LSH) approximation.
- Indyk, P., & Motwani, R. (1998). *Approximate nearest neighbors:
  towards removing the curse of dimensionality.* Proceedings of STOC '98 —
  the foundational LSH paper that motivates threshold-based clustering
  in high-dimensional spaces.
- Mikolov, T., Chen, K., Corrado, G., & Dean, J. (2013). *Efficient
  estimation of word representations in vector space.* arXiv:1301.3781 —
  the embedding-as-classifier intuition that underpins the cosine
  threshold here.
- Sentence-Transformers (Reimers & Gurevych, 2019). *Sentence-BERT:
  Sentence Embeddings using Siamese BERT-Networks.* EMNLP 2019 — the
  family of models that AllMiniLML6V2 (used by fastembed) belongs to;
  they are tuned to make cosine similarity meaningful.
- Drain3 (He, P., et al., 2017). *Drain: An online log parsing approach
  with fixed depth tree.* IEEE ICWS 2017 — bdslib's complementary log
  template miner; primary/secondary handles "is this the same string?",
  drain3 handles "is this the same template?".

## See also

- [`Documentation/DATABASE.md`](../DATABASE.md) — the complete database
  layout. `ObservabilityStorage` is one of seven distinct stores;
  primary/secondary classification is the most consequential algorithm
  in the data path but it sits inside a larger context.
- [`Documentation/Algorithm/KNN.md`](KNN.md) — the same cosine
  similarity primitive used differently: per-batch clustering rather
  than store-wide deduplication.
- [`Documentation/jsonrpc_api/v2_duplicates.md`](../jsonrpc_api/v2_duplicates.md)
  — the public API for reading the `dedup_tracking` log.
- [`Documentation/jsonrpc_api/v2_secondaries.md`](../jsonrpc_api/v2_secondaries.md),
  [`Documentation/jsonrpc_api/v2_secondary.md`](../jsonrpc_api/v2_secondary.md)
  — the public API for traversing the primary→secondary tree.
- `src/observability.rs` — the implementation itself, ~1320 lines.
- `src/common/jsonfingerprint.rs` — the canonical-fingerprint helper
  used to build `data_text` for dedup.
