# v2/results.len

Return the total number of result queues currently tracked by the [`ResultQueue`] singleton, plus the list of their UUIDs.

Result queues are short-lived per-id FIFOs created on first push and evicted by the background sweeper when their creation timestamp is older than `results_ttl_secs` (configured in `bds.hjson`).

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `session` | string | no | `""` | UUIDv7 session identifier. Accepted for symmetry with other v2 methods; not consulted internally. |

## Response

```json
{
  "count": 2,
  "ids":   [
    "0192a3b4-c5d6-7e8f-9012-34567890abcd",
    "0192a3b4-c5d6-7e8f-9012-34567890abce"
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `count` | integer | Number of distinct queues currently registered. |
| `ids[]` | array of string | UUIDs of every registered queue, in arbitrary order. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/results.len","params":{},"id":1}' | jq
```

## Error responses

| Code | Condition |
|---|---|
| _none_ | This method does not raise application-level errors. |

## Notes

- **Empty queues count.** A queue that has been fully drained still appears in `ids` until the sweeper evicts it.
- **Sweep interval.** Tune `results_sweep_secs` and `results_ttl_secs` in `bds.hjson` to control how aggressively stale queues are reclaimed.
