# v2/tpl.delete

Delete a template document from the tplstorage by UUID.

The handler removes the metadata, body, and vector index entry. Idempotent — deleting an already-removed template returns `{ "deleted": true }` without raising an error.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 session identifier. Accepted and logged; reserved for future caching. |
| `id` | string | yes | — | UUIDv7 of the template to delete. |

## Response

```json
{ "deleted": true }
```

| Field | Type | Description |
|---|---|---|
| `deleted` | bool | Always `true` on success. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/tpl.delete",
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
| `-32011` | Template store write failed |
| `-32600` | Invalid UUID in `id` |

## Notes

- **Idempotent.** Deleting a non-existent UUID is not an error.
- **Frequency-tracking history.** The deletion does not retroactively scrub FrequencyTracking observation rows in older shards — `v2/tpl.template_by_id` will still report the template as not found, but historical observation IDs are not pruned.
