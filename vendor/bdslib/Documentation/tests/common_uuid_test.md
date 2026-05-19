# common_uuid_test.rs

**File:** `tests/common_uuid_test.rs`  
**Module:** `bdslib::common::uuid` — UUIDv7 generation and timestamp extraction

Tests `generate_v7`, `generate_v7_at`, and `timestamp_from_v7`.

## Test functions

### `generate_v7`

| Test | Description |
|---|---|
| `test_generate_v7_returns_version_7` | Returns a UUID with version number 7 |
| `test_generate_v7_successive_ids_are_monotonic` | Two consecutive calls produce non-decreasing UUIDs |
| `test_generate_v7_ids_are_unique` | 100 generated IDs are all distinct |
| `test_generate_v7_timestamp_is_recent` | Embedded timestamp is within ±1 second of `now()` |

### `generate_v7_at`

| Test | Description |
|---|---|
| `test_generate_v7_at_returns_version_7` | Returns version 7 |
| `test_generate_v7_at_embeds_correct_timestamp` | Extracted timestamp matches input (±1 ms precision) |
| `test_generate_v7_at_past_time` | Past timestamps (e.g., Sep 2001) work correctly |
| `test_generate_v7_at_future_time` | Future timestamps (+1 year) work correctly |
| `test_generate_v7_at_before_epoch_uses_epoch` | Pre-UNIX_EPOCH inputs don't panic |
| `test_generate_v7_at_ordering_matches_time_ordering` | `t1 < t2 < t3` → `id1 < id2 < id3` |

### `timestamp_from_v7`

| Test | Description |
|---|---|
| `test_timestamp_from_v7_round_trips` | Timestamp embedded via `generate_v7_at` is extractable (±1 ms) |
| `test_timestamp_from_v7_on_generate_v7` | Timestamp from `generate_v7()` is within ±1s of now |
| `test_timestamp_from_non_v7_uuid_returns_none` | Non-v7 UUIDs (v4) return `None` |
| `test_timestamp_advances_with_successive_ids` | Timestamps extracted from IDs with 5 ms delay show advancement |

## Key properties verified

- **Monotonicity** — IDs are non-decreasing even at millisecond resolution
- **Uniqueness** — 100 concurrent IDs are all distinct
- **Round-trip** — timestamps survive `generate_v7_at` → `timestamp_from_v7` with ±1 ms error
- **Type safety** — `timestamp_from_v7` rejects non-v7 UUIDs rather than returning garbage
- **Ordering** — lexicographic UUID order matches chronological order
