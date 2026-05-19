# v2/script_update

Replace both metadata and body of an existing BUND script.

The metadata is fully overwritten, so callers must include every field they
want to retain (most importantly `name` and `schedule`, which are required).
Both the metadata record and the content blob are updated atomically per
sub-store; the vector index is not rebuilt (scripts are addressed by UUID,
not searched semantically).

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `session` | string | yes | UUIDv7 transaction identifier (echoed only). |
| `id` | string | yes | UUIDv7 of the script to update. |
| `metadata` | object | yes | New metadata — must contain non-empty `name` and `schedule` strings. |
| `script` | string | yes | New BUND script body. |

## Response

```json
{ "id": "019e0c09-a151-7940-ac19-02b1fa1d0dd5" }
```

| Field | Type | Description |
|---|---|---|
| `id` | string | Echoed UUIDv7. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method":  "v2/script_update",
    "params":  {
      "session":  "-",
      "id":       "019e0c09-a151-7940-ac19-02b1fa1d0dd5",
      "metadata": { "name": "hello", "schedule": "*/10 * * * *", "version": 2 },
      "script":   "// hello v2\n\"hello v2\" println."
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32600` | `id` is not a valid UUID, or metadata is missing required fields |

## Notes

- **Idempotent for missing IDs.** Updating a non-existent script silently succeeds (the underlying `update_metadata` / `update_content` are no-ops on missing records). Use [`v2/script`](v2_script.md) first if existence verification is required.
- **Full overwrite.** Metadata is not merged — pass every field you want to keep.
