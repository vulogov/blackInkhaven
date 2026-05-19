# datastorage_test.rs

**File:** `tests/datastorage_test.rs`  
**Module:** `bdslib::datastorage` — `BlobStorage` and `JsonStorage`

Tests raw blob storage and JSON document storage with key-based deduplication.

## BlobStorage tests

| Test | Description |
|---|---|
| `test_blob_add_returns_uuid` | `add_blob()` returns a non-nil UUID |
| `test_blob_add_and_get_roundtrip` | Stored blob is retrieved identically |
| `test_blob_get_nonexistent_returns_none` | Unknown UUID returns `None` |
| `test_blob_update_changes_data` | `update_blob()` replaces stored bytes |
| `test_blob_update_nonexistent_is_ok` | Updating unknown ID doesn't error |
| `test_blob_drop_removes_record` | After `drop_blob`, `get_blob` returns `None` |
| `test_blob_drop_nonexistent_is_ok` | Dropping unknown ID doesn't error |
| `test_blob_empty_payload` | Empty byte slices are stored and retrieved |
| `test_blob_binary_data_with_null_bytes` | All byte values 0–255 survive the roundtrip |
| `test_blob_add_produces_unique_uuids` | Two adds produce distinct IDs |
| `test_blob_uuids_are_time_ordered` | Later adds produce larger UUIDv7 values |
| `test_blob_clone_shares_store` | Clones see the same underlying data |
| `test_blob_update_then_drop` | Can update then delete |

## JsonStorage tests — basic operations

| Test | Description |
|---|---|
| `test_json_add_returns_uuid` | `add_json()` returns a non-nil UUID |
| `test_json_add_and_get_roundtrip` | Document stores and retrieves exactly |
| `test_json_get_nonexistent_returns_none` | Unknown UUID returns `None` |
| `test_json_update_changes_document` | `update_json()` replaces the document |
| `test_json_update_nonexistent_is_ok` | Updating unknown ID doesn't error |
| `test_json_drop_removes_record` | After `drop_json`, `get_json` returns `None` |
| `test_json_drop_nonexistent_is_ok` | Dropping unknown ID doesn't error |
| `test_json_clone_shares_store` | Clones see the same data |
| `test_json_default_key_deduplicates` | No key field → all docs share one slot; adds are upserts |
| `test_json_preserves_nested_structure` | Complex nested JSON survives the roundtrip |
| `test_json_add_produces_unique_uuids_with_different_keys` | Different keys produce different UUIDs |

## JsonStorage tests — key_field deduplication

| Test | Description |
|---|---|
| `test_json_key_field_deduplicates_by_extracted_key` | Same extracted key returns the same UUID |
| `test_json_key_field_different_keys_produce_different_uuids` | Different key values produce different UUIDs |
| `test_json_key_field_falls_back_to_default_when_missing` | Missing key field falls back to `default_key` |
| `test_json_key_field_nested_path` | Key from `"meta.id"` nested path |
| `test_json_key_field_numeric_value` | Numeric key values work |
| `test_json_key_field_bool_value` | Boolean key values work (`true ≠ false`) |
| `test_json_update_key_stays_consistent` | Updates re-extract key; same key → same UUID |
| `test_json_single_quotes_in_value_are_handled` | Single quotes in values are escaped |
| `test_json_single_quotes_in_default_key` | Single quotes in `default_key` don't cause SQL errors |

## Coverage summary

- Full CRUD for both `BlobStorage` and `JsonStorage`
- Binary data integrity (null bytes, all byte values)
- UUIDv7 generation properties (uniqueness, ordering)
- Three deduplication modes: default key, `key_field`, nested path
- SQL injection safety for single quotes in keys and values
- Clone semantics (shared underlying store)
