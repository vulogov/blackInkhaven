# v2/script_add

Store a new BUND script with metadata and source body.

The script is persisted in the dedicated script `DocumentStorage` reachable
via `ShardsManager::script_add`. The metadata JSON object **must** contain
`name` (human-readable label) and `schedule` (crontab-style execution
schedule). Any additional fields are preserved verbatim. The body is stored
as the document content blob.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `session` | string | yes | UUIDv7 transaction identifier (echoed only). |
| `metadata` | object | yes | Metadata object — must contain non-empty `name` and `schedule` strings. |
| `script` | string | yes | Raw BUND script source code. |

## Response

```json
{ "id": "019e0c09-a151-7940-ac19-02b1fa1d0dd5" }
```

| Field | Type | Description |
|---|---|---|
| `id` | string | UUIDv7 assigned to the new script. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/script_add",
    "params": {
      "session":  "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "metadata": { "name": "hello", "schedule": "*/5 * * * *" },
      "script":   "// say hello\n\"hello\" println."
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32600` | Invalid metadata (missing/empty `name`, missing/empty `schedule`, or not a JSON object) |
