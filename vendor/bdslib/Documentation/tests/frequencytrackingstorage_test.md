# frequencytrackingstorage_test.rs

**File:** `tests/frequencytrackingstorage_test.rs`  
**Module:** `bdslib::FrequencyTracking` — event-frequency tracking store

Tests the full public API of `FrequencyTracking`: recording observations, querying by ID, by exact timestamp, by time range, by lookback duration, clone semantics, sync, and persistence.

## Fixtures

| Fixture | Description |
|---|---|
| `memory_ft()` | In-process `":memory:"` store; isolated per test |
| `file_ft()` | Persistent `TempDir`-backed store for sync and reopen tests |

## `add` / `add_with_timestamp`

| Test | Description |
|---|---|
| `test_add_records_observation` | `add()` creates one entry; `by_id` returns a single timestamp |
| `test_add_with_timestamp_explicit` | Explicit timestamp is stored verbatim and returned by `by_id` |
| `test_add_same_id_multiple_times` | Three `add_with_timestamp` calls at different times → three rows, returned ascending |
| `test_add_same_timestamp_same_id_duplicate` | Duplicate `(ts, id)` pairs each produce a separate row (no dedup on write) |
| `test_add_id_with_special_characters` | Single-quoted IDs and spaces are stored and retrieved correctly |

## `by_id`

| Test | Description |
|---|---|
| `test_by_id_unknown_returns_empty` | Unknown ID returns an empty `Vec` |
| `test_by_id_returns_ascending_order` | Out-of-order inserts are returned sorted ascending by timestamp |
| `test_by_id_does_not_return_other_ids` | Query for ID `"a"` never returns timestamps recorded under `"b"` |

## `by_timestamp`

| Test | Description |
|---|---|
| `test_by_timestamp_returns_ids_at_exact_second` | Two IDs at the same second are returned; a third at a different second is not |
| `test_by_timestamp_empty_returns_empty_vec` | Querying an unrecorded second returns an empty `Vec` |
| `test_by_timestamp_deduplicates_ids` | Same ID inserted twice at the same timestamp appears once in the result |
| `test_by_timestamp_sorted_alphabetically` | Returned IDs are sorted alphabetically |

## `time_range`

| Test | Description |
|---|---|
| `test_time_range_returns_ids_in_window` | IDs inside the window are returned; those outside are not |
| `test_time_range_inclusive_on_both_ends` | Records at exactly `start` and exactly `end` are included |
| `test_time_range_no_match_returns_empty` | Range containing no records returns an empty `Vec` |
| `test_time_range_deduplicates_ids` | An ID firing three times in the window appears once in the result |
| `test_time_range_point_interval` | `time_range(t, t)` (zero-width interval) returns the ID recorded at `t` |

## `recent`

| Test | Description |
|---|---|
| `test_recent_returns_freshly_added_id` | ID added via `add()` appears in `recent("1min")` |
| `test_recent_excludes_old_record` | Record 2 hours old is absent from `recent("1h")`; a fresh record is present |
| `test_recent_empty_store_returns_empty` | `recent("1h")` on an empty store returns an empty `Vec` |
| `test_recent_invalid_duration_returns_err` | Unparseable duration string returns `Err` |
| `test_recent_various_duration_formats` | `"30s"`, `"5min"`, `"1h"`, `"7days"` all parse and return non-empty results |

## Clone / shared state

| Test | Description |
|---|---|
| `test_clone_shares_underlying_store` | Data written through one clone is immediately visible through another |

## Sync

| Test | Description |
|---|---|
| `test_sync_on_file_db` | `sync()` completes without error; data is still readable afterwards |

## Persistence

| Test | Description |
|---|---|
| `test_data_persists_across_reopen` | Record written and sync'd in one `FrequencyTracking` instance is readable after reopening the same file |

## Coverage summary

- All five public read methods (`by_id`, `by_timestamp`, `time_range`, `recent`, and their edge cases)
- Write semantics: explicit and implicit timestamps; duplicate `(ts, id)` pairs stored separately
- `DISTINCT` correctness for `by_timestamp`, `time_range`, and `recent`
- Ascending ordering of `by_id` results; alphabetical ordering of ID lists
- Inclusive boundary semantics for `time_range`
- humantime duration parsing (valid and invalid strings)
- SQL injection safety for single-quoted and space-containing IDs
- Clone-shared state
- `sync()` and WAL persistence across process-boundary reopen
