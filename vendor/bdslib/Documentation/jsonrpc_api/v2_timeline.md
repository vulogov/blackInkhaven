# v2/timeline

Returns the earliest and latest event timestamps found across all shards. Useful for understanding the temporal span of stored data before issuing range queries.

## Parameters

None.

## Response

```json
{
  "min_ts": 1745000000,
  "max_ts": 1745086399
}
```

| Field | Type | Description |
|---|---|---|
| `min_ts` | integer \| null | Unix seconds of the earliest stored event, or `null` if no data |
| `max_ts` | integer \| null | Unix seconds of the latest stored event, or `null` if no data |

Both fields are `null` when the database contains no records.

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/timeline","params":{},"id":1}' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "min_ts": 1745000000,
    "max_ts": 1745086399
  },
  "id": 1
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `ShardsManager` singleton not initialised |
| `-32002` | Shard index query failed |
| `-32003` | Shard open failed |
| `-32004` | Timestamp range query failed |

## Notes

- Iterates every shard and aggregates `MIN(ts)` / `MAX(ts)` from the `telemetry` table of each.
- Accepts no time window — it always reflects the global dataset.
- Use `min_ts` / `max_ts` as `start_ts` / `end_ts` for bounded queries to other methods.
