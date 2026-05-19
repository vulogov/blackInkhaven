# v2/results.empty

Return the number of elements currently in the result queue identified by `id`, plus an `empty` flag for convenience.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | no | `""` | UUIDv7 session identifier. Accepted for symmetry with other v2 methods; not consulted internally. |
| `id` | string | yes | — | UUIDv7 of the queue to inspect. |

## Response

```json
{
  "id":    "0192a3b4-c5d6-7e8f-9012-34567890abcd",
  "count": 3,
  "empty": false
}
```

| Field | Type | Description |
|---|---|---|
| `id` | string | Echoes the queue id. |
| `count` | integer | Number of values currently in the queue. `0` when the queue is empty or doesn't exist. |
| `empty` | bool | `true` when `count == 0`. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/results.empty",
    "params": { "id": "0192a3b4-c5d6-7e8f-9012-34567890abcd" },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32600` | Invalid UUID in `id` |

## Notes

- **Missing queue == empty queue.** This method does not distinguish between "queue never existed" and "queue exists but is empty". Use `v2/results.len` to discover which queue ids are tracked.
- **No side effects.** Inspection only — does not pop, does not refresh the TTL.
