# shardsmanager_test.rs

**File:** `tests/shardsmanager_test.rs`  
**Module:** `bdslib::ShardsManager` — config-driven top-level API

Tests config loading, document routing by timestamp, cross-shard search, updates, and clone semantics.

## Construction

| Test | Description |
|---|---|
| `test_new_reads_config` | Config file is parsed and the manager initializes |
| `test_new_missing_config` | Missing config file returns `Err` |
| `test_new_invalid_hjson` | Malformed hjson returns `Err` |
| `test_new_missing_required_field` | Missing `dbpath` returns `Err` |
| `test_new_similarity_threshold_respected` | Custom `similarity_threshold` is applied |

## `add`

| Test | Description |
|---|---|
| `test_add_returns_uuid` | Returns a non-nil UUID |
| `test_add_missing_timestamp` | Missing `timestamp` returns `Err` |
| `test_add_string_timestamp_rejected` | Non-numeric string `timestamp` fails |
| `test_add_routes_to_correct_shard` | Records with timestamps 2 hours apart go to different shards |

## `add_batch`

| Test | Description |
|---|---|
| `test_add_batch_empty` | Empty batch returns empty vector |
| `test_add_batch_returns_ordered_uuids` | Returns a UUID for each document |
| `test_add_batch_error_propagates` | An invalid document fails the entire batch |

## `delete_by_id`

| Test | Description |
|---|---|
| `test_delete_by_id_removes_record` | Record is gone after deletion |
| `test_delete_by_id_unknown_id_ok` | Deleting unknown ID doesn't error |

## `update`

| Test | Description |
|---|---|
| `test_update_returns_new_uuid` | Update returns a new UUID for the replacement |
| `test_update_cross_shard` | Updating to a different timestamp moves the record to a different shard |

## `search_fts`

| Test | Description |
|---|---|
| `test_search_fts_finds_added_record` | FTS search finds records across all shards |
| `test_search_fts_no_results_on_miss` | No matches return `[]` |
| `test_search_fts_invalid_duration` | Invalid duration string returns `Err` |
| `test_search_fts_spans_multiple_shards` | Search works across records in multiple hour shards |

## `search_vector`

| Test | Description |
|---|---|
| `test_search_vector_finds_added_record` | Vector search works across all shards |
| `test_search_vector_results_have_score` | Results include `_score` field |
| `test_search_vector_sorted_by_score_desc` | Results are ordered by descending score |
| `test_search_vector_invalid_duration` | Invalid duration returns `Err` |

## Accessors and clone

| Test | Description |
|---|---|
| `test_cache_accessor` | `cache()` returns the underlying `ShardsCache` |
| `test_clone_shares_state` | Clones share the same cache, catalog, and data |

## Key concepts tested

- **Timestamp routing** — `add` reads the `timestamp` field and routes each document to the correct time-partitioned shard
- **Cross-shard search** — `search_fts` and `search_vector` fan out across all shards covering the requested duration and merge results
- **Cross-shard update** — updating a document's timestamp may move it to a different shard; the old record is deleted and the new one is inserted
- **Config validation** — missing or malformed config fields fail at construction time, not at query time
