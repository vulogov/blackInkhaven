# telemetrytrend_demo.rs

**File:** `examples/telemetrytrend_demo.rs`

Demonstrates `TelemetryTrend`: statistical analysis, anomaly detection, and breakout detection over time-series numeric data.

## What it demonstrates

| Operation | Description |
|---|---|
| `TelemetryTrend::query(key, duration)` | Analyse all values for `key` in `duration` |
| `TelemetryTrend::query_window(key, start, end)` | Analyse values in a specific time range |
| Descriptive statistics | min, max, mean, median, std_dev, variability |
| Anomaly detection | S-H-ESD algorithm flags outlier indices |
| Breakout detection | Distribution shift detection for step functions |

## Sections

| Section | Description |
|---|---|
| 1. Ramp (0..99) | Linear sequence; verify min=0, max=99, mean≈49.5, variability is non-zero |
| 2. `query_window` | Same key with explicit start/end timestamps |
| 3. Constant series | All same value; std_dev≈0, variability≈0 |
| 4. Outlier / anomaly | 60 docs near 50.0 + one spike at 1,000,000; anomalies list contains spike index |
| 5. `SamplePoint` | Print timestamps and values from the `samples` field |
| 6. Breakout detection | 80 docs: first 40≈10.0, last 40≈100.0; breakout_at index ≈ 40 |
| 7. Generator integration | 50 synthesized telemetry docs via `Generator`; verify queryable |
| 8. Empty key | Unknown key returns n=0, all stats NaN |

## Response fields

| Field | Type | Description |
|---|---|---|
| `n` | integer | Number of data points |
| `min` | float | Minimum value |
| `max` | float | Maximum value |
| `mean` | float | Arithmetic mean |
| `median` | float | Median value |
| `std_dev` | float | Standard deviation |
| `variability` | float | Coefficient of variation (std_dev / mean) |
| `anomalies` | list of integers | Sample indices flagged as outliers (S-H-ESD) |
| `breakout_at` | integer or null | Index of detected distribution shift |
| `samples` | list of `{timestamp, value}` | All data points in chronological order |

## Example output

```
ramp: n=100 min=0.0 max=99.0 mean=49.5 median=49.5
outlier: anomalies=[30]
breakout: breakout_at=40
```
