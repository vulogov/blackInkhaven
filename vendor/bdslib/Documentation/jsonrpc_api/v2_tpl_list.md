# v2/tpl.list

List every template document stored in shards overlapping `[now − duration, now]`, with metadata.

This is the bulk-read counterpart to `v2/tpl.get`. Bodies are not returned — call `v2/tpl.get` to fetch a body for a specific UUID.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 session identifier. Accepted and logged; reserved for future caching. |
| `duration` | string | no | `"1h"` | Lookback window in humantime format (e.g. `"30min"`, `"6h"`, `"7days"`). |

## Response

```json
{
  "templates": [
    {
      "id":       "0192a3b4-c5d6-7e8f-9012-34567890abcd",
      "metadata": {
        "name":        "runbook.disk_full",
        "tags":        ["runbook", "disk"],
        "description": "Step-by-step recovery for a full disk.",
        "type":        "template",
        "created_at":  1745603600,
        "timestamp":   1745603600
      }
    },
    {
      "id":       "0192a3b4-c5d6-7e8f-9012-34567890abce",
      "metadata": {
        "name":       "user <*> logged in from <*>",
        "type":       "drain_template",
        "cluster_id": 17,
        "timestamp":  1745603620,
        "created_at": 1745603620
      }
    }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `templates[]` | array | One entry per template stored in any shard overlapping the window. |
| `templates[].id` | string | UUIDv7 of the template. |
| `templates[].metadata` | object | Full stored metadata. Bodies are not included. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/tpl.list",
    "params": {
      "session":  "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "24h"
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32011` | Template store query failed |

## Notes

- **Drain3 + manual templates appear together.** Distinguish via `metadata.type` (`"drain_template"` vs `"template"`).
- **Order is per-shard.** Templates are merged from each overlapping shard in catalog order; sort client-side by `metadata.timestamp` if you need a global chronology.
- **Multi-shard windows are scanned in parallel** via rayon when more than one shard overlaps the window.
