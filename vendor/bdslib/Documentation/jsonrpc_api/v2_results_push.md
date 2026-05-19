# v2/results.push

Push a JSON value onto the back of the result queue identified by `id`.

If no queue exists for `id`, one is created and stamped with the current Unix-second timestamp; that timestamp drives the TTL eviction by the background sweeper. Subsequent pushes append to the same queue without resetting the timestamp.

The supplied JSON value is wrapped server-side as a `rust_dynamic::Value` of type `JSON` and stored verbatim — `v2/results.pull` returns it back unchanged.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | no | `""` | UUIDv7 session identifier. Accepted for symmetry with other v2 methods; not consulted internally. |
| `id` | string | yes | — | UUIDv7 of the target queue. |
| `value` | any JSON | yes | — | Arbitrary JSON value to enqueue (object, array, number, string, bool, or null). |

## Response

```json
{
  "id":    "0192a3b4-c5d6-7e8f-9012-34567890abcd",
  "count": 3
}
```

| Field | Type | Description |
|---|---|---|
| `id` | string | Echoes the queue id. |
| `count` | integer | New queue length immediately after the push. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/results.push",
    "params": {
      "id":    "0192a3b4-c5d6-7e8f-9012-34567890abcd",
      "value": { "kind": "alert", "severity": "warn", "code": 503 }
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32600` | Invalid UUID in `id` |
| `-32602` | Required `id` or `value` parameter missing/wrong type |

## Notes

- **TTL is set on creation.** A queue's TTL window starts at the first `push`; subsequent pushes do **not** refresh the timestamp. To extend the window, drop and re-create the queue (let the sweeper evict, or use a fresh `id`).
- **Order is FIFO.** The first value pushed is the first value returned by `v2/results.pull`.
- **Concurrency.** Push operations are atomic per queue and safe under concurrent writers.
