# shardsinfo_test.rs

**File:** `tests/shardsinfo_test.rs`  
**Module:** `bdslib::common::shardsinfo` — `ShardInfoEngine` shard catalog

Tests the redb-backed shard registry: adding shard records, querying by timestamp, and concurrent access.

## Construction

| Test | Description |
|---|---|
| `test_new_in_memory_succeeds` | In-memory catalog creation works |
| `test_new_file_backed_creates_db` | File-backed catalog database is created |
| `test_new_file_backed_survives_reopen` | Data persists across close and reopen |

## `add_shard`

| Test | Description |
|---|---|
| `test_add_shard_returns_uuid` | Returns a UUIDv7 |
| `test_add_shard_ids_are_unique` | Two adds produce distinct IDs |
| `test_add_shard_start_equal_end_is_error` | `start == end` returns `Err` |
| `test_add_shard_start_after_end_is_error` | `start > end` returns `Err` |
| `test_add_shard_path_with_single_quote_is_safe` | Paths containing `'` are escaped correctly |

## `shards_at`

| Test | Description |
|---|---|
| `test_shards_at_empty_returns_empty_vec` | Empty catalog returns `[]` |
| `test_shards_at_timestamp_inside_interval` | Timestamp inside `[start, end)` returns the shard |
| `test_shards_at_timestamp_on_start_boundary_inclusive` | `start` is included in the range |
| `test_shards_at_timestamp_on_end_boundary_exclusive` | `end` is excluded from the range |
| `test_shards_at_timestamp_before_interval` | Timestamp before `start` returns `[]` |
| `test_shards_at_timestamp_after_interval` | Timestamp at or after `end` returns `[]` |
| `test_shards_at_returns_multiple_overlapping_shards` | Overlapping shard intervals are all returned |
| `test_shards_at_results_ordered_by_start_time` | Results are sorted by `start_time` ascending |
| `test_shards_at_returned_fields_round_trip` | `shard_id`, `path`, `start_time`, `end_time` are correct |

## `shard_exists_at`

| Test | Description |
|---|---|
| `test_shard_exists_at_empty_is_false` | Empty catalog returns `false` |
| `test_shard_exists_at_inside_interval_is_true` | Timestamp inside returns `true` |
| `test_shard_exists_at_start_boundary_is_true` | `start` is included |
| `test_shard_exists_at_end_boundary_is_false` | `end` is excluded |
| `test_shard_exists_at_outside_interval_is_false` | Outside range returns `false` |

## Thread safety

| Test | Description |
|---|---|
| `test_clone_shares_state` | Clones see the same catalog data |
| `test_concurrent_add_and_query` | 8 concurrent `add_shard` calls succeed; all shards are queryable |

## Key properties

- **Half-open interval** — the range `[start_time, end_time)` is start-inclusive, end-exclusive throughout
- **SQL safety** — paths with single quotes are escaped, preventing SQL injection
- **Ordering** — `shards_at` results are always sorted by `start_time` ascending
- **Concurrency** — the catalog supports concurrent writes from multiple threads
