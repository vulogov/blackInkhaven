# v2/signal.update

Replace the metadata of an existing signal in-place.

The full metadata JSON object is overwritten with the supplied value — this is *not* a partial merge. Read the current metadata first (e.g. via `v2/signals`) if you want to preserve other fields.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 session identifier. Accepted and logged; reserved for future caching. |
| `id` | string | yes | — | UUIDv7 of the signal to update. |
| `metadata` | object | yes | — | Replacement metadata document. Any field not present here is removed from storage. |

## Response

```json
{ "ok": true }
```

| Field | Type | Description |
|---|---|---|
| `ok` | bool | Always `true` on success. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/signal.update",
    "params": {
      "session":  "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "id":       "0192a3b4-c5d6-7e8f-9012-34567890abcd",
      "metadata": {
        "name":      "deploy.completed",
        "severity":  "info",
        "timestamp": 1745603600,
        "service":   "auth",
        "version":   "2.4.1",
        "result":    "success"
      }
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32011` | Signal store write failed (e.g. signal not found) |
| `-32602` | Invalid UUID in `id` |

## Notes

- **Replace, not merge.** Keys absent from the new `metadata` are dropped. Round-trip through `v2/signals` if you need to preserve untouched fields.
- **Vector index.** Re-embeddings of the metadata are not automatic; if your downstream search relies on fresh vectors, run a `tpl.reindex`-equivalent on the signal store after bulk updates (no public RPC for this yet).
