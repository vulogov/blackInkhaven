# v2/tpl.get

Fetch a template document by UUID from the tplstorage.

The handler does a direct shard lookup using the UUIDv7 creation timestamp first (O(1)); if that misses (e.g. backdated ingestion where the template's `timestamp` differs from wall-clock time), it falls back to a full catalog scan.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 session identifier. Accepted and logged; reserved for future caching. |
| `id` | string | yes | — | UUIDv7 of the template. |

## Response

```json
{
  "id":       "0192a3b4-c5d6-7e8f-9012-34567890abcd",
  "metadata": {
    "name":        "runbook.disk_full",
    "tags":        ["runbook", "disk"],
    "description": "Step-by-step recovery for a full disk.",
    "type":        "template",
    "created_at":  1745603600,
    "timestamp":   1745603600
  },
  "body": "1. Identify the volume via `df -h`. 2. Rotate logs in /var/log. 3. Restart filesystem."
}
```

| Field | Type | Description |
|---|---|---|
| `id` | string | UUIDv7 of the template (echoes the request). |
| `metadata` | object | Stored metadata document. |
| `body` | string | Template body text decoded as UTF-8. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/tpl.get",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "id":      "0192a3b4-c5d6-7e8f-9012-34567890abcd"
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32010` | Template not found in any shard |
| `-32011` | Template store query failed |
| `-32600` | Invalid UUID in `id` |

## Notes

- **Returns both halves.** Use `v2/tpl.get` when you need the full template; this saves a round-trip compared to fetching metadata and body separately.
- **Body encoding.** Bodies are stored as raw bytes; this method decodes them via `from_utf8_lossy`, so non-UTF-8 sequences are replaced with `U+FFFD`.
