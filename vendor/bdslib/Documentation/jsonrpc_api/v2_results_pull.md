# v2/results.pull

Pop the front value from the result queue identified by `id`.

The popped `rust_dynamic::Value` is converted to JSON and returned. Values pushed via `v2/results.push` (which carry JSON natively) round-trip exactly; other Value types fall through `cast_value_to_json` (numbers, strings, lists, dicts).

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | no | `""` | UUIDv7 session identifier. Accepted for symmetry with other v2 methods; not consulted internally. |
| `id` | string | yes | — | UUIDv7 of the queue to pop from. |

## Response

```json
{
  "id":        "0192a3b4-c5d6-7e8f-9012-34567890abcd",
  "value":     { "kind": "alert", "severity": "warn", "code": 503 },
  "remaining": 2
}
```

| Field | Type | Description |
|---|---|---|
| `id` | string | Echoes the queue id. |
| `value` | any JSON | The popped value, or `null` when the queue was missing/empty. |
| `remaining` | integer | Number of values still in the queue after the pop. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/results.pull",
    "params": { "id": "0192a3b4-c5d6-7e8f-9012-34567890abcd" },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32600` | Invalid UUID in `id` |

## Notes

- **Empty / missing queue is not an error.** Pulling from an empty or unknown queue returns `value: null`, `remaining: 0`. Distinguish "queue missing" from "queue exists but empty" via `v2/results.empty`.
- **Empty queues are kept.** A drained queue is **not** auto-removed — its creation timestamp persists so subsequent pushes share the same TTL window.
- **Type loss is uncommon.** Anything pushed via `v2/results.push` round-trips identically; values written by other paths (e.g. internal callers) may degrade to `null` if the underlying `rust_dynamic` type cannot be expressed in JSON.
