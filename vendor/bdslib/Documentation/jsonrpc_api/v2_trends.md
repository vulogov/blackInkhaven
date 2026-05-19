# v2/trends

Compute a statistical trend summary for a single telemetry key over a lookback window, including descriptive statistics, anomaly detection (S-H-ESD), and breakout (distribution-shift) detection.

Only primary records are used. Each document's `data` field is inspected for a numeric value: `data.value` is tried first; if absent, `data` itself is used when it is a bare number. Documents without an extractable numeric value are silently skipped.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `session` | string | yes | UUID v7 session identifier. Accepted and logged; reserved for future result caching. |
| `key` | string | yes | Exact telemetry key to query (e.g. `"server.cpu"`, `"http.latency"`). |
| `duration` | string | yes | Lookback window from now in humantime format, e.g. `"1h"`, `"30min"`, `"7days"`. Only shards whose time interval overlaps `[now − duration, now)` are queried. |

## Response

```json
{
  "key": "server.cpu",
  "start": 1745000000,
  "end": 1745003600,
  "n": 360,
  "min": 12.4,
  "max": 98.7,
  "mean": 54.2,
  "median": 52.1,
  "std_dev": 18.3,
  "variability": 0.338,
  "anomalies": [
    { "index": 241, "timestamp": 1745002410, "value": 98.7 }
  ],
  "breakouts": [
    { "index": 180, "timestamp": 1745001800, "value": 76.3 }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `key` | string | The telemetry key that was queried. |
| `start` | integer | Start of the queried window as Unix seconds (inclusive). |
| `end` | integer | End of the queried window as Unix seconds (exclusive). |
| `n` | integer | Number of numeric samples collected. |
| `min` | number \| null | Minimum observed value. `null` in JSON when `n == 0` (stored as `NaN`). |
| `max` | number \| null | Maximum observed value. `null` in JSON when `n == 0`. |
| `mean` | number \| null | Arithmetic mean. `null` in JSON when `n == 0`. |
| `median` | number \| null | Statistical median (50th percentile). `null` in JSON when `n == 0`. |
| `std_dev` | number \| null | Population standard deviation. `null` in JSON when `n == 0`. |
| `variability` | number \| null | Coefficient of variation (`std_dev / |mean|`). `0` when `mean ≈ 0`. `null` in JSON when `n == 0`. |
| `anomalies` | array | Data points flagged as statistical anomalies by the S-H-ESD algorithm. Empty when `n < 4`. |
| `anomalies[].index` | integer | Zero-based position in the time-ordered sample array. |
| `anomalies[].timestamp` | integer | Unix seconds of the anomalous document. |
| `anomalies[].value` | number | The extracted numeric value at this point. |
| `breakouts` | array | Data points where a significant distribution shift was detected (energy-based multi-breakout). Empty when `n < 4`. |
| `breakouts[].index` | integer | Zero-based position in the time-ordered sample array. |
| `breakouts[].timestamp` | integer | Unix seconds of the breakout document. |
| `breakouts[].value` | number | The extracted numeric value at this point. |

When `n == 0`, all statistical fields contain `NaN` internally. Standard JSON serialisers typically encode `NaN` as `null`; treat any `null` numeric field as "no data".

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/trends",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "key": "server.cpu",
      "duration": "1h"
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "key": "server.cpu",
    "start": 1745000000,
    "end": 1745003600,
    "n": 360,
    "min": 12.4,
    "max": 98.7,
    "mean": 54.2,
    "median": 52.1,
    "std_dev": 18.3,
    "variability": 0.338,
    "anomalies": [
      { "index": 241, "timestamp": 1745002410, "value": 98.7 }
    ],
    "breakouts": [
      { "index": 180, "timestamp": 1745001800, "value": 76.3 }
    ]
  },
  "id": 1
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32004` | Trend query failed (DB unavailable, shard error, or computation error) |
| `-32600` | Invalid `duration` string |

## Notes

- **Numeric extraction.** Only documents with an extractable numeric value contribute to the statistics. `data.value` is tried first; `data` itself is used when it is a bare JSON number. All other document shapes are skipped silently.
- **Anomaly detection.** Uses the S-H-ESD (Seasonal Hybrid Extreme Studentised Deviate) algorithm with `max_anoms = 10%`. The seasonal period is `max(2, n / 4)` capped at `n / 2`. Requires `n ≥ 4`.
- **Breakout detection.** Uses an energy-based multi-breakout algorithm. The minimum segment size is `max(2, n / 5)` capped at 30. Requires `n ≥ 4`.
- **Empty corpus.** When no numeric samples are found, `n` is `0` and all statistical fields are `null`. No error is raised.
- **Exact key match.** Only primary records whose `key` field equals `key` exactly are included. Use [`v2/keys`](v2_keys.md) to discover available keys.
- The `session` parameter is stored for future caching integration and has no current effect on results.
