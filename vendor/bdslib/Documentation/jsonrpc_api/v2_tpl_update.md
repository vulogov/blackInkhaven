# v2/tpl.update

Update one or more fields of an existing template document.

Each updatable field (`name`, `body`, `tags`, `description`) is independent — omit a field to leave it unchanged. When any of `name`, `tags`, or `description` is supplied, the metadata is loaded, merged, and written back; when `body` is supplied, the body is overwritten directly.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 session identifier. Accepted and logged; reserved for future caching. |
| `id` | string | yes | — | UUIDv7 of the template to update. |
| `name` | string | no | — | New human-readable name. Omit to leave unchanged. |
| `body` | string | no | — | New body text. Omit to leave unchanged. |
| `tags` | array of string | no | — | New tag list. Omit to leave unchanged. |
| `description` | string | no | — | New description. Omit to leave unchanged. |

## Response

```json
{ "updated": true }
```

| Field | Type | Description |
|---|---|---|
| `updated` | bool | Always `true` on success. |

## Example

```bash
# Rename a template and append a tag.
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/tpl.update",
    "params": {
      "session": "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "id":      "0192a3b4-c5d6-7e8f-9012-34567890abcd",
      "name":    "runbook.disk_full_v2",
      "tags":    ["runbook", "disk", "filesystem"]
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32010` | Template not found |
| `-32011` | Template store write failed (incl. malformed stored metadata) |
| `-32600` | Invalid UUID in `id` |

## Notes

- **Partial updates are merge, not replace.** Supplying just `tags` will rewrite tags and keep `name`, `description`, etc. untouched.
- **Body and metadata are written separately.** If both `body` and any metadata field are present, the metadata write happens first; on a body-write failure the metadata change persists.
- **Vector index is not auto-rebuilt** after a body or name change. Run `v2/tpl.reindex` when fresh embeddings matter (e.g. before a search-heavy workload).
