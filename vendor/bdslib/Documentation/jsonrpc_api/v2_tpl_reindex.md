# v2/tpl.reindex

Rebuild the tplstorage HNSW vector index for every shard overlapping `[now − duration, now]`.

Use this after bulk template updates (e.g. mass `v2/tpl.update` with body changes) or unclean shutdown — the persisted metadata and body bytes are scanned and re-embedded into a fresh vector index.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | yes | — | UUIDv7 session identifier. Accepted and logged; reserved for future caching. |
| `duration` | string | no | `"24h"` | Lookback window in humantime format. Reindex covers every shard whose interval overlaps this window. |

## Response

```json
{ "indexed": 142 }
```

| Field | Type | Description |
|---|---|---|
| `indexed` | integer | Total number of templates re-embedded across every shard scanned. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/tpl.reindex",
    "params": {
      "session":  "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d",
      "duration": "7days"
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32011` | Template store reindex failed |

## Notes

- **Cost.** Reindexing re-embeds every template body via the shared fastembed model; cost scales linearly with template count.
- **Multi-shard windows rebuild in parallel** via rayon; single-shard windows take a serial path.
- **Safe to run live.** Existing tpl-search queries continue against the previous index until the rebuild completes per shard, then atomically swap.
