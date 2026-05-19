# v2/tpl.add

Manually store a template document — name, body text, optional tags and description — in the per-shard tplstorage.

This is the explicit-write counterpart to drain3, which auto-creates templates from log streams. Use this when you want to seed runbooks, alert templates, or other named text fragments that should be searchable alongside drain3-discovered patterns.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 session identifier. Accepted and logged; reserved for future caching. |
| `name` | string | yes | — | Human-readable template name. |
| `body` | string | yes | — | Template body text. Both `name` and `body` are embedded into the tplstorage vector index for semantic search. |
| `timestamp` | integer | no | wall-clock now | Unix seconds. Determines which time shard the template lands in. |
| `tags` | array of string | no | `[]` | Optional tag list, stored verbatim in metadata. |
| `description` | string | no | `""` | Optional human-readable description. |

The metadata document stored alongside the body is automatically populated with `{ name, tags, description, type: "template", created_at, timestamp }`.

## Response

```json
{ "id": "0192a3b4-c5d6-7e8f-9012-34567890abcd" }
```

| Field | Type | Description |
|---|---|---|
| `id` | string | UUIDv7 of the stored template. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/tpl.add",
    "params": {
      "session":     "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "name":        "runbook.disk_full",
      "body":        "1. Identify the volume via `df -h`. 2. Rotate logs in /var/log. 3. Restart filesystem.",
      "tags":        ["runbook", "disk"],
      "description": "Step-by-step recovery for a full disk."
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

## Notes

- **Drain3 vs. manual.** Templates created here have `type: "template"` in metadata; templates auto-discovered by drain3 have `type: "drain_template"`. Both live in the same tplstorage and are returned together by `v2/tpl.list` / `v2/tpl.search`.
- **Time routing.** A `timestamp` outside any existing shard auto-creates a new shard, just like primary record ingestion.
