# shardsmanager_scripts_test.rs

**File:** `tests/shardsmanager_scripts_test.rs`
**Module:** `bdslib::shardsmanager_scripts` — BUND script registry on `ShardsManager`

Verifies the contract of `ShardsManager::script_add`, `scripts`, `script`, `update_script`, `script_delete`, plus persistence across reopen.

## Test functions

### script_add

| Test | What it verifies |
|---|---|
| `script_add_succeeds_with_valid_metadata` | Valid metadata returns a non-empty UUIDv7 |
| `script_add_preserves_extra_metadata` | Additional metadata fields (e.g., `owner`, `tags`) are retained verbatim |
| `script_add_rejects_missing_name` | Metadata without `name` → error mentioning "name" |
| `script_add_rejects_missing_schedule` | Metadata without `schedule` → error mentioning "schedule" |
| `script_add_rejects_empty_name` | Whitespace-only `name` is rejected |
| `script_add_rejects_non_object_metadata` | A non-object metadata value (e.g., a string) is rejected |

### scripts (list)

| Test | What it verifies |
|---|---|
| `scripts_returns_id_schedule_pairs` | Each stored script appears in the list with the correct `(id, schedule)` |
| `scripts_with_metadata_returns_full_metadata` | Convenience helper exposes the full metadata document |
| `scripts_empty_when_none_added` | Empty store → empty list |

### script (get)

| Test | What it verifies |
|---|---|
| `script_returns_body_verbatim` | The stored body is returned byte-for-byte (UTF-8 round-trip) |
| `script_returns_none_for_missing_id` | Unknown UUID → `None` (not an error) |

### update_script

| Test | What it verifies |
|---|---|
| `update_script_replaces_metadata_and_body` | Both metadata and body are replaced; `script_metadata` reflects the new values |
| `update_script_validates_metadata` | Missing `schedule` is rejected and the original record is left untouched |

### script_delete

| Test | What it verifies |
|---|---|
| `script_delete_removes_record` | Both `script` and `script_metadata` return `None` after delete; the listing is empty |
| `script_delete_is_idempotent` | Deleting a non-existent UUID is a no-op (no error) |

### Persistence

| Test | What it verifies |
|---|---|
| `scripts_persist_across_reopen` | A script added in one `ShardsManager` instance survives drop + reopen |

## Key properties verified

- **Metadata validation.** `script_add` and `update_script` both reject metadata without non-empty `name` and `schedule` strings.
- **Idempotent delete.** Deleting an unknown UUID is not an error.
- **Persistence.** Scripts survive `ShardsManager` reopen (DocumentStorage on disk).
- **No vector indexing required.** Tests pass with no semantic search; the script registry is identity-keyed.

## Run

```bash
cargo test --test shardsmanager_scripts_test -- --show-output
```

## Notes

A single shared `EmbeddingEngine` is held in a `OnceLock` to avoid per-test model load. Each test creates a fresh `TempDir` config so tests are independent and parallelisable.
