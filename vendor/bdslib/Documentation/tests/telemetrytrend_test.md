# telemetrytrend_test.rs

**File:** `tests/telemetrytrend_test.rs`  
**Module:** `bdslib::analysis::telemetrytrend` — statistical trend analysis

Tests descriptive statistics, anomaly detection (S-H-ESD), and breakout detection over numeric telemetry corpora.

## Test function

### `test_telemetrytrend_lifecycle`

A single comprehensive test covering the full trend analysis lifecycle:

| Step | Description |
|---|---|
| 1 | Query before `init_db` returns `Err("not initialized")` |
| 2 | `init_db()` succeeds |
| 3 | Empty result: unknown key returns `n=0`, all statistics are `NaN` |
| 4 | Deterministic ramp (0..99): verifies `min=0`, `max=99`, `mean≈49.5`, variability is non-zero |
| 5 | `query_window(key, start, end)` works on the ramp data |
| 6 | Constant series: all same value; `std_dev≈0`, `variability≈0` |
| 7 | Outlier detection: 60 docs near 50.0 + one spike at 1,000,000; `anomalies` contains the spike index |
| 8 | `SamplePoint` fields: each sample has `timestamp` and `value` |
| 9 | Breakout detection: 80 docs with step function (first 40≈10.0, last 40≈100.0); `breakout_at≈40` |
| 10 | Generator integration: 50 synthesized telemetry docs ingested via `Generator`; queryable |

## Response fields verified

| Field | Verification |
|---|---|
| `n` | Equals the number of data points ingested |
| `min` | Minimum of all values |
| `max` | Maximum of all values |
| `mean` | Arithmetic mean, verified for ramp (49.5) |
| `median` | Median, verified for ramp |
| `std_dev` | Near-zero for constant series; non-zero for ramp |
| `variability` | `std_dev / mean`; zero for constant series |
| `anomalies` | Contains index of the 1,000,000 spike in outlier test |
| `breakout_at` | Near index 40 for the step-function dataset |
| `samples` | Each element has `timestamp` (number) and `value` (number) |

## Key invariants

- **NaN safety** — unknown keys and empty corpora return `NaN` statistics without panicking
- **Anomaly accuracy** — S-H-ESD detects a single extreme outlier within a near-constant series
- **Breakout accuracy** — a step function (abrupt distribution shift) is detected near the transition point
- **`query_window` consistency** — explicit start/end range returns the same results as `query(key, duration)` for the same data

## Notes

Like other singleton-dependent tests, this uses a single `#[test]` function because the global `ShardsManager` cannot be reset between runs.
