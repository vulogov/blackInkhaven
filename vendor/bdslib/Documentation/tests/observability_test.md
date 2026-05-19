# observability_test.rs

**File:** `tests/observability_test.rs`  
**Module:** `bdslib::ObservabilityStorage` — redb-backed dedup, primary/secondary classification

Tests ingestion validation, ID handling, retrieval, deletion, time-range queries, deduplication, and primary/secondary split.

## Validation

| Test | Description |
|---|---|
| `test_add_missing_timestamp_is_err` | Missing `timestamp` field returns `Err` |
| `test_add_missing_key_is_err` | Missing `key` field returns `Err` |
| `test_add_missing_data_is_err` | Missing `data` field returns `Err` |
| `test_add_non_string_key_is_err` | Non-string `key` returns `Err` |
| `test_add_numeric_string_timestamp_is_ok` | String `"1000"` is accepted as timestamp |
| `test_add_non_numeric_timestamp_is_err` | Non-numeric string timestamp fails |

## ID handling

| Test | Description |
|---|---|
| `test_add_generates_uuid_when_id_absent` | Missing `id` field is auto-generated as UUIDv7 |
| `test_add_uses_provided_id` | Custom `id` field is used as-is |

## `get_by_id`

| Test | Description |
|---|---|
| `test_get_by_id_returns_stored_record` | Stored document is retrieved with all fields |
| `test_get_by_id_nonexistent_returns_none` | Unknown UUID returns `None` |
| `test_get_preserves_all_data_types` | Nested structures and all JSON types survive roundtrip |

## `get_by_key`

| Test | Description |
|---|---|
| `test_get_by_key_returns_all_records` | All records with a key are returned |
| `test_get_by_key_empty_returns_empty_vec` | Unknown key returns `[]` |
| `test_get_by_key_ordered_by_timestamp` | Results are sorted by timestamp ascending |

## Delete

| Test | Description |
|---|---|
| `test_delete_by_id_removes_record` | Record is gone after `delete_by_id` |
| `test_delete_by_id_nonexistent_is_ok` | Deleting unknown ID doesn't error |
| `test_delete_by_key_removes_all_records` | All records with a key are deleted |
| `test_delete_by_key_nonexistent_is_ok` | Deleting unknown key doesn't error |
| `test_delete_by_id_clears_dedup_tracking` | Dedup state is cleaned up |
| `test_delete_by_key_clears_dedup_tracking` | Dedup state is cleaned up for all records |

## `list_ids_by_time_range`

| Test | Description |
|---|---|
| `test_list_ids_by_time_range_returns_correct_ids` | IDs in `[start, end)` are returned |
| `test_list_ids_by_time_range_half_open` | Boundary: start is included, end is excluded |
| `test_list_ids_by_time_range_empty` | Out-of-range timestamps return `[]` |

## Deduplication

| Test | Description |
|---|---|
| `test_add_duplicate_returns_existing_uuid` | Same key+data hash returns the original UUID |
| `test_add_duplicate_does_not_store_second_record` | Duplicates don't create a new record |
| `test_add_different_data_same_key_are_not_duplicates` | Different data with same key creates a new record |
| `test_dedup_timestamps_recorded` | Duplicate timestamps are logged |
| `test_dedup_timestamps_empty_for_nonexistent_key` | Unknown key has no dedup log |
| `test_dedup_timestamps_empty_when_no_duplicates` | No duplicates → empty list |
| `test_dedup_different_data_tracked_separately` | Each distinct data value tracks its own duplicates |

## Primary/secondary classification

| Test | Description |
|---|---|
| `test_first_record_is_primary` | First record for any key is always primary |
| `test_list_primaries_empty_initially` | Initially no primaries |
| `test_clearly_different_data_both_become_primaries` | Threshold 1.1 → all records are primary |
| `test_very_similar_data_assigned_as_secondary` | Threshold −1.1 → all but first are secondary |
| `test_list_secondaries_empty_for_new_primary` | New primary has no secondaries |
| `test_list_primaries_in_range` | `list_primaries(start, end)` returns primaries in range |
| `test_delete_by_id_removes_from_primary_tracking` | Deleted primary is removed from tracking |

## Clone and metadata

| Test | Description |
|---|---|
| `test_clone_shares_underlying_store` | Clones see the same underlying data |
| `test_extra_fields_stored_as_metadata` | Extra fields (`host`, `region`, `tags`) are preserved |
| `test_mandatory_fields_not_duplicated_in_metadata` | `key`, `data`, `timestamp` appear once each |
