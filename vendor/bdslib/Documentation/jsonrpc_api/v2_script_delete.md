# v2/script_delete

Remove a BUND script from all sub-stores (metadata, blob, vector, frequency tracking).

Idempotent — succeeds even when `id` does not exist.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `session` | string | yes | UUIDv7 transaction identifier (echoed only). |
| `id` | string | yes | UUIDv7 of the script to delete. |

## Response

```json
{ "id": "019e0c09-a151-7940-ac19-02b1fa1d0dd5", "deleted": true }
```

| Field | Type | Description |
|---|---|---|
| `id` | string | Echoed UUIDv7. |
| `deleted` | boolean | Always `true` (delete is idempotent — non-existent IDs do not raise an error). |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method":  "v2/script_delete",
    "params":  { "session": "-", "id": "019e0c09-a151-7940-ac19-02b1fa1d0dd5" },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32004` | Delete failed in one of the sub-stores |
| `-32600` | `id` is not a valid UUID |
