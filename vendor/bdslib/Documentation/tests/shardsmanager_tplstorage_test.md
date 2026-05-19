# shardsmanager_tplstorage_test.rs

**File:** `tests/shardsmanager_tplstorage_test.rs`

Integration tests for the `ShardsManager` template FrequencyTracking query API: `template_by_id`, `templates_by_timestamp`, and `templates_recent`. All tests use `tpl_add` to inject templates directly without drain3.

## Test fixture

`tmp_manager(duration)` creates a fresh `(TempDir, ShardsManager)` with the given shard duration. The embedding engine is initialised once via `OnceLock`.

`store_tpl(mgr, body, ts)` calls `mgr.tpl_add` with `{"name": body, "timestamp": ts, "type": "tpl"}`.

## template_by_id tests

| Test | Description |
|---|---|
| `test_template_by_id_found` | Store one template; `template_by_id` with its UUID returns `Some` with matching `id`, `body`, and `metadata`. |
| `test_template_by_id_not_found` | A random UUID v7 not stored in any shard returns `None`. |
| `test_template_by_id_invalid_uuid` | A non-UUID string returns an error (not `None`). |
| `test_template_by_id_multiple_shards` | Templates stored in two separate shards (different timestamps); each is found by its respective UUID. |
| `test_template_by_id_metadata_shape` | Confirms `metadata["name"]`, `metadata["timestamp"]`, and `metadata["type"]` are present and correct. |

## templates_by_timestamp tests

| Test | Description |
|---|---|
| `test_templates_by_timestamp_empty` | No templates stored → returns empty list. |
| `test_templates_by_timestamp_single` | One template within range → returned; same template outside range → not returned. |
| `test_templates_by_timestamp_range` | Three templates at distinct timestamps; queries over sub-windows return the expected subsets. |
| `test_templates_by_timestamp_deduplication` | Two templates with the same body but different UUIDs both appear independently (no deduplication by body). |
| `test_templates_by_timestamp_inclusive_bounds` | Verifies boundary timestamps are inclusive: `start_ts == ts` and `end_ts == ts` both include the template. |
| `test_templates_by_timestamp_cross_shard` | Templates in two separate shards (different `shard_duration` boundaries) are both returned by a range spanning both shards. |

## templates_recent tests

| Test | Description |
|---|---|
| `test_templates_recent_empty` | No templates → empty list. |
| `test_templates_recent_within_window` | Template stored at `now - 30 min`; `templates_recent("1h")` returns it. |
| `test_templates_recent_outside_window` | Template stored at `now - 90 min`; `templates_recent("1h")` excludes it. |
| `test_templates_recent_multiple` | Three templates in window, one outside; confirms only the three in-window templates are returned. |
| `test_templates_recent_deduplication` | Two distinct UUIDs with same body both returned. |
| `test_templates_recent_result_shape` | Each result entry has `id` (valid UUID string), `metadata` (object), and `body` (non-empty string). |
| `test_templates_recent_invalid_duration` | `templates_recent("bad")` returns an error. |
