# shard_test.rs

**File:** `tests/shard_test.rs`  
**Module:** `bdslib::Shard` — single time-partition storage

Tests document storage, FTS search, vector search, primary/secondary classification, deduplication, and deletion within a single shard.

## Add / Get / Delete

| Test | Description |
|---|---|
| `test_add_returns_uuid_and_get_retrieves_doc` | Document is stored and retrieved by UUID |
| `test_get_nonexistent_returns_none` | Unknown UUID returns `None` |
| `test_get_by_key_returns_all_records` | All records with a key are returned |
| `test_delete_primary_removes_from_all_indexes` | Deleting primary removes record from FTS, vector, and observability |
| `test_delete_secondary_leaves_primary_in_indexes` | Deleting a secondary doesn't affect the primary's indexes |
| `test_delete_nonexistent_is_ok` | Deleting unknown ID doesn't error |

## Secondary not indexed

| Test | Description |
|---|---|
| `test_secondary_not_in_fts` | Secondary records are not searchable via FTS |
| `test_secondary_not_in_vector` | Secondary records are not in the vector index |

## Search results embed secondaries

| Test | Description |
|---|---|
| `test_search_fts_result_has_secondaries_field` | FTS results include a `secondaries` array |
| `test_search_vector_result_has_secondaries_field` | Vector results include `secondaries` with `_score` |
| `test_search_fts_no_secondaries_when_primary_has_none` | Empty `secondaries` when none exist |
| `test_search_vector_no_secondaries_when_primary_has_none` | Empty `secondaries` in vector result |
| `test_search_fts_multiple_secondaries_per_primary` | All secondaries are embedded in the result |

## `search_fts`

| Test | Description |
|---|---|
| `test_search_fts_finds_keyword_in_data` | Keyword search works across primary documents |
| `test_search_fts_no_match_returns_empty` | No matching documents returns `[]` |
| `test_search_fts_respects_limit` | Result count capped at requested limit |
| `test_search_fts_returns_full_documents_with_metadata` | Results include full document payload and metadata |

## `search_vector`

| Test | Description |
|---|---|
| `test_search_vector_returns_score_and_secondaries` | Results have `_score` field and `secondaries` |
| `test_search_vector_top_result_is_most_similar` | Most similar record ranks first |
| `test_search_vector_scores_descending` | Scores are in descending order |
| `test_search_vector_respects_limit` | Result count capped at requested limit |

## Deduplication

| Test | Description |
|---|---|
| `test_add_duplicate_returns_same_uuid` | Same key+data returns the original UUID |

## Clone

| Test | Description |
|---|---|
| `test_clone_shares_all_indexes` | Cloned shards see the same data across all three indexes |

## Custom configuration

| Test | Description |
|---|---|
| `test_with_config_all_primaries_all_indexed` | Similarity threshold 1.1 → all records are primary and indexed |
| `test_with_config_secondaries_not_indexed` | Threshold −1.1 → only first record is indexed |

## Key concepts tested

- **Three-index consistency** — add/delete must update the telemetry table, FTS index, and vector index atomically
- **Secondary isolation** — secondaries are stored in observability but never added to FTS or vector
- **Embedded secondaries** — search results include secondaries inline so callers don't need a second query
- **Threshold effects** — similarity threshold controls the primary/secondary split rate
