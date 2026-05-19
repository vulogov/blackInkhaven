# v2/tpl.template_by_id

Look up a single drain3 log-template document by its UUID. Scans all shards until the template is found and returns its metadata and body, or `null` when no template with that UUID exists in any shard.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUID v7 session identifier. Accepted and logged. |
| `id` | string | yes | — | UUID v7 of the template document to retrieve. |

## Response

```json
{
  "template": {
    "id": "019682ab-1234-7000-8000-000000000001",
    "metadata": {
      "name": "user <*> logged in from <*>",
      "timestamp": 1745001234,
      "type": "tpl"
    },
    "body": "user <*> logged in from <*>"
  }
}
```

When the UUID is not found in any shard the `template` field is `null`:

```json
{ "template": null }
```

### `template` fields

| Field | Type | Description |
|---|---|---|
| `id` | string | UUID v7 of the template document. |
| `metadata` | object | Template metadata as stored in tplstorage. Typically includes `name`, `timestamp`, and `type`. |
| `body` | string | The drain3 template pattern string, e.g. `"user <*> logged in from <*>"`. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/tpl.template_by_id",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "id": "019682ab-1234-7000-8000-000000000001"
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32011` | Template lookup failed (shard error) |
| `-32600` | Malformed UUID in `id` |

## Notes

- The UUID is parsed before any shard is queried; a malformed `id` returns a `-32600` error immediately.
- Shard order is not defined. All shards are scanned until a match is found.
- A `null` result is not an error. Use `v2/tpl.templates_recent` or `v2/tpl.templates_by_timestamp` to discover valid template UUIDs.
