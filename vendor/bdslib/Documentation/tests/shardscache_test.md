# shardscache_test.rs

**File:** `tests/shardscache_test.rs`  
**Module:** `bdslib::ShardsCache` — time-aligned shard management with LRU cache

Tests auto-creation, interval alignment, memory cache, catalog persistence, span queries, and lifecycle management.

## Construction

| Test | Description |
|---|---|
| `test_new_creates_root_directory` | Root directory is created on first open |
| `test_new_creates_catalog_db` | `shards_info.db` catalog is created |
| `test_invalid_duration_string_is_error` | Invalid humantime string returns `Err` |
| `test_zero_duration_is_error` | Zero duration returns `Err` |
| `test_with_config_accepts_custom_threshold` | Custom similarity threshold is accepted |
| `test_various_duration_formats_are_accepted` | `"1h"`, `"30min"`, `"1day"`, `"7days"`, `"3600s"` all work |

## `shard()` — auto-creation

| Test | Description |
|---|---|
| `test_shard_auto_creates_for_uncovered_timestamp` | Accessing an uncovered timestamp creates the shard |
| `test_shard_auto_create_registers_in_catalog` | Auto-created shards are registered in the catalog |
| `test_shard_auto_create_increments_cached_count` | `cached_count()` increases after creation |
| `test_shard_creates_subdirectory_on_disk` | A subdirectory is created for each new shard |

## `shard()` — interval alignment

| Test | Description |
|---|---|
| `test_shard_same_interval_same_instance` | Same 1-hour bucket returns the same `Shard` instance |
| `test_shard_different_intervals_different_instances` | Different hours return different instances |
| `test_shard_two_auto_creates_each_has_own_catalog_entry` | Each shard gets its own catalog entry |

## `shard()` — cache and catalog hits

| Test | Description |
|---|---|
| `test_second_call_hits_cache_not_catalog` | Same bucket is cached in memory after first access |
| `test_cache_hit_returns_same_shared_data` | Cached shards share data across calls |
| `test_shard_catalog_hit_after_close` | Shard is reopened from catalog after `close()` |
| `test_shard_catalog_hit_does_not_duplicate_catalog_entry` | No duplicate catalog entries on reopen |

## Primary/secondary with threshold

| Test | Description |
|---|---|
| `test_shard_with_all_primary_threshold_indexes_all` | Threshold 1.1 → all records indexed |
| `test_shard_with_all_secondary_threshold_indexes_only_first` | Threshold −1.1 → only first record indexed |

## `shards_span`

| Test | Description |
|---|---|
| `test_shards_span_empty_for_inverted_range` | Reversed bounds return empty |
| `test_shards_span_empty_for_equal_bounds` | Equal bounds return empty |
| `test_shards_span_single_interval` | Single bucket returns one shard |
| `test_shards_span_exact_boundary_is_single` | Boundary-aligned range returns one shard |
| `test_shards_span_crosses_two_intervals` | Range crossing two buckets returns two shards |
| `test_shards_span_crosses_three_intervals` | 3-hour range returns three 1-hour shards |
| `test_shards_span_populates_cache` | `shards_span` populates the memory cache |
| `test_shards_span_data_visible_across_returned_shards` | Records in different shards are isolated to their shard |

## `current`

| Test | Description |
|---|---|
| `test_current_returns_at_least_one_shard` | `current("1s")` returns ≥1 shard |
| `test_current_longer_span_may_return_multiple_shards` | `current("3h")` may return ≥3 shards |
| `test_current_invalid_duration_is_error` | Invalid duration string returns `Err` |

## `sync` and `close`

| Test | Description |
|---|---|
| `test_sync_empty_cache_is_ok` | Syncing empty cache succeeds |
| `test_sync_populated_cache_is_ok` | Syncing populated cache succeeds |
| `test_close_empty_cache_is_ok` | Closing empty cache succeeds |
| `test_close_clears_cache` | `cached_count()` drops to 0 after `close()` |
| `test_close_data_persists_on_disk` | Data survives `close()` and reopen |

## Clone and accessors

| Test | Description |
|---|---|
| `test_clone_shares_cache` | Clones share the same in-memory LRU cache |
| `test_clone_shares_shard_data` | Clones see the same shard data |
| `test_cached_count_starts_at_zero` | Initially no shards cached |
| `test_info_accessor_reflects_auto_created_shards` | `info()` catalog reflects all created shards |
