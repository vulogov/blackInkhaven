# v2/script

Fetch a single BUND script by UUIDv7 — body and metadata.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `session` | string | no | UUIDv7 transaction identifier (echoed only). |
| `id` | string | yes | UUIDv7 of the script. |

## Response

```json
{
  "id":       "019e0c09-a151-7940-ac19-02b1fa1d0dd5",
  "script":   "// say hello\n\"hello\" println.",
  "metadata": { "name": "hello", "schedule": "*/5 * * * *" }
}
```

| Field | Type | Description |
|---|---|---|
| `id` | string | Echoed UUIDv7. |
| `script` | string | BUND source code (UTF-8). |
| `metadata` | object | Metadata document; empty `{}` if missing. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method":  "v2/script",
    "params":  { "session": "-", "id": "019e0c09-a151-7940-ac19-02b1fa1d0dd5" },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32004` | Script lookup failed |
| `-32404` | Script with that `id` does not exist |
| `-32600` | `id` is not a valid UUID |
