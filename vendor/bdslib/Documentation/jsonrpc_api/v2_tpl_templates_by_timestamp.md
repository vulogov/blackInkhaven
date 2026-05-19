# v2/tpl.templates_by_timestamp

Return all drain3 log-template documents whose FrequencyTracking observation timestamp falls within an inclusive `[start_ts, end_ts]` range. Queries all shards and deduplicates results by UUID.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Accepted and logged. |
| `start_ts` | integer | yes | — | Range start as Unix seconds (inclusive). |
| `end_ts` | integer | yes | — | Range end as Unix seconds (inclusive). Must be ≥ `start_ts`. |

## Response

```json
{
  "templates": [
    {
      "id": "019682ab-1234-7000-8000-000000000001",
      "metadata": {
        "name": "user <*> logged in from <*>",
        "timestamp": 1745001234,
        "type": "tpl"
      },
      "body": "user <*> logged in from <*>"
    },
    {
      "id": "019682ab-5678-7000-8000-000000000002",
      "metadata": {
        "name": "connection to <*> on port <*> established",
        "timestamp": 1745001300,
        "type": "tpl"
      },
      "body": "connection to <*> on port <*> established"
    }
  ]
}
```

An empty array is returned when no template observations fall within the window:

```json
{ "templates": [] }
```

### `templates[]` fields

| Field | Type | Description |
|---|---|---|
| `id` | string | UUID v7 of the template document. |
| `metadata` | object | Template metadata as stored in tplstorage. Typically includes `name`, `timestamp`, and `type`. |
| `body` | string | The drain3 template pattern string, e.g. `"connection to <*> on port <*> established"`. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/tpl.templates_by_timestamp",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "start_ts": 1745000000,
      "end_ts": 1745003600
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32011` | Template query failed (shard error) |

## Notes

- The FrequencyTracking timestamp reflects when the drain3 miner stored or updated the template in tplstorage — it is the Unix second of the triggering log entry, not the wall-clock time of the storage call.
- Results are deduplicated by UUID across all shards. Each UUID appears at most once in the response.
- For humantime lookback windows (e.g. `"1h"`) use `v2/tpl.templates_recent` instead.
