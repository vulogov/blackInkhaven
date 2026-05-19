# observability_demo.rs

**File:** `examples/observability_demo.rs`

Demonstrates `ObservabilityStorage`: the redb-backed key-value store used for deduplication, primary/secondary classification, time-range queries, and metadata preservation.

## What it demonstrates

### Sections

| Section | Description |
|---|---|
| 1. Deduplication | Same key+data returns the original UUID; duplicate timestamps are logged |
| 2. Mixed data types | Threshold 1.1 (above max cosine) forces all records to be primary |
| 3. Metadata extraction | Extra fields (`host`, `region`, `tags`) survive the roundtrip |
| 4. Primary/secondary split | Default threshold; similar embeddings → secondary; dissimilar → primary |
| 5. Time-range queries | `list_ids_by_time_range(start, end)` returns IDs in the window |
| 6. Delete by id / by key | `delete_by_id` and `delete_by_key` remove records and dedup state |

## Key API

| Method | Description |
|---|---|
| `add(doc)` | Ingest a document; returns UUID, primary flag, and dedup flag |
| `get_by_id(id)` | Retrieve a document by UUID |
| `get_by_key(key)` | Retrieve all documents with a given key |
| `list_primaries(start, end)` | List primary UUIDs in a time range |
| `list_secondaries(primary_id)` | List secondary UUIDs for a given primary |
| `dedup_timestamps(key, data_hash)` | Retrieve logged timestamps for a duplicate |
| `list_ids_by_time_range(start, end)` | All UUIDs with timestamps in `[start, end)` |
| `delete_by_id(id)` | Delete one record and its dedup state |
| `delete_by_key(key)` | Delete all records for a key and their dedup state |

## Key concepts

**Primary/secondary split** — when a new record's embedding is within `similarity_threshold` of an existing primary, it becomes a secondary of that primary. The threshold is a cosine similarity cutoff (default ≈ 0.85).

**Dedup tracking** — exact-content duplicates (same key + same data hash) do not create new records. Instead, the duplicate's timestamp is appended to a dedup log for the original record.

**Timestamp range** — the range `[start, end)` is half-open; records at `start` are included, records at `end` are excluded.

## Example output

```
add: uuid=019... primary=true  dedup=false
add duplicate: same uuid, dedup=true
add different data: new uuid, primary=true
primaries in window: [uuid1, uuid2]
secondaries of uuid1: [uuid3]
after delete: get_by_id returns None
```
