# common_timerange_test.rs

**File:** `tests/common_timerange_test.rs`  
**Module:** `bdslib::common::time` — time range alignment functions

Tests the `minute_range`, `hour_range`, and `day_range` functions that align a Unix timestamp to a time bucket boundary.

## Test functions

### `minute_range`

| Test | Description |
|---|---|
| `test_minute_range_1_aligns_to_minute` | `minute_range(time, 1)` returns `[HH:MM:00, HH:MM+1:00)` |
| `test_minute_range_5_aligns_to_five_minute_block` | 00:07:42 aligns to `[00:05:00, 00:10:00)` |
| `test_minute_range_15_aligns_to_quarter_hour` | 15-minute blocks are correctly aligned |
| `test_minute_range_30_aligns_to_half_hour` | 30-minute blocks are correctly aligned |
| `test_minute_range_60_equals_hour_range` | `minute_range(t, 60)` equals `hour_range(t)` |
| `test_minute_range_time_on_boundary_is_own_start` | When called exactly on a boundary, `start == input` |
| `test_minute_range_interval_duration_is_correct` | All valid intervals (1,2,3,4,5,6,10,12,15,20,30,60) produce `n * 60` second ranges |
| `test_minute_range_zero_is_error` | Interval 0 returns `Err` |
| `test_minute_range_non_divisor_is_error` | Non-divisors of 60 (7,8,9,11,…) return `Err` |
| `test_minute_range_pre_epoch_is_error` | Times before UNIX_EPOCH return `Err` |

### `hour_range`

| Test | Description |
|---|---|
| `test_hour_range_aligns_to_hour` | Returns `[HH:00:00, HH+1:00:00)` |
| `test_hour_range_time_on_boundary_is_own_start` | Exact boundary: `start == input` |
| `test_hour_range_duration_is_3600_seconds` | Range duration is always 3600 seconds |
| `test_hour_range_start_has_zero_sub_hour_seconds` | `start % 3600 == 0` |
| `test_hour_range_pre_epoch_is_error` | Pre-epoch inputs return `Err` |

### `day_range`

| Test | Description |
|---|---|
| `test_day_range_aligns_to_utc_day` | Returns `[midnight UTC, midnight+1 UTC)` |
| `test_day_range_time_on_boundary_is_own_start` | Exact midnight: `start == input` |
| `test_day_range_duration_is_86400_seconds` | Range duration is always 86400 seconds |
| `test_day_range_start_has_zero_sub_day_seconds` | `start % 86400 == 0` |
| `test_day_range_pre_epoch_is_error` | Pre-epoch inputs return `Err` |

### Nesting and contiguity

| Test | Description |
|---|---|
| `test_hour_range_nested_in_day_range` | Hour range is fully contained in its day range |
| `test_minute_range_nested_in_hour_range` | Minute range is fully contained in its hour range |
| `test_successive_minute_ranges_are_contiguous` | `range_n.end == range_n+1.start` |
| `test_successive_hour_ranges_are_contiguous` | Consecutive hour ranges share a boundary |
| `test_successive_day_ranges_are_contiguous` | Consecutive day ranges share a boundary |

## Key invariants

- All ranges are half-open: `[start, end)`
- All range durations are exact multiples of their bucket size
- Nesting is strict: minute ⊆ hour ⊆ day
- Contiguity is exact: no gaps or overlaps between consecutive ranges
