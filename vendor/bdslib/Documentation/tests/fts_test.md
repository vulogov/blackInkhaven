# fts_test.rs

**File:** `tests/fts_test.rs`  
**Module:** `bdslib::FTSEngine` — Tantivy full-text search

Tests document indexing, BM25 search, deletion, sync, and concurrent access.

## Construction

| Test | Description |
|---|---|
| `test_new_memory` | Creates an in-memory Tantivy index |
| `test_new_file_backed` | Creates a file-backed persistent index |

## `add_document`

| Test | Description |
|---|---|
| `test_add_returns_uuidv7` | Returns a UUIDv7 |
| `test_add_same_text_produces_distinct_ids` | Duplicate content gets distinct IDs |
| `test_uuidv7_ids_are_monotonically_increasing` | Later adds produce larger UUIDs |

## `search`

| Test | Description |
|---|---|
| `test_search_empty_index_returns_empty` | Empty index returns no results |
| `test_search_finds_added_document` | Keyword search finds a matching document |
| `test_search_no_match_returns_empty` | No matching documents returns `[]` |
| `test_search_is_selective` | Only documents containing the queried term are returned |
| `test_search_returns_all_matching_documents` | All matching documents are returned |
| `test_search_respects_limit` | Results capped at requested limit |
| `test_search_returns_correct_ids_not_aliases` | Results contain exact UUIDs from `add_document` |

## `drop_document`

| Test | Description |
|---|---|
| `test_drop_removes_document_from_search` | Deleted document doesn't appear in subsequent searches |
| `test_drop_nonexistent_uuid_is_silent` | Deleting unknown ID doesn't error |
| `test_drop_only_removes_target_document` | Other documents are unaffected by the deletion |

## Error handling

| Test | Description |
|---|---|
| `test_invalid_query_returns_error` | Malformed Tantivy query (e.g., unclosed phrase) returns `Err` |

## File-backed lifecycle

| Test | Description |
|---|---|
| `test_file_backed_add_search_drop` | Full add / search / drop cycle on disk-backed index |

## `sync`

| Test | Description |
|---|---|
| `test_sync_memory_does_not_error` | Syncing in-memory index succeeds |
| `test_sync_file_backed_does_not_error` | Syncing file-backed index succeeds |
| `test_sync_data_readable_after_sync` | Data added and synced is immediately searchable |
| `test_sync_on_empty_index_does_not_error` | Syncing empty index is safe |

## Concurrency

| Test | Description |
|---|---|
| `test_concurrent_adds_produce_unique_ids` | 8 threads adding documents concurrently produce 8 distinct UUIDs |
