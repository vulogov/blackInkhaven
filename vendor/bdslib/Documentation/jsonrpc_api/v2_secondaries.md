# v2/secondaries

Returns the UUIDs of all secondary records associated with a given primary record.

Secondary records are near-duplicate documents linked to a primary by embedding similarity (cosine similarity ≥ 0.85). Use [`v2/secondary`](v2_secondary.md) to fetch a full document for any of the returned IDs.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `primary_id` | string | yes | UUID v7 of the primary record |

## Response

```json
{
  "ids": [
    "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4b00",
    "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4b01"
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `ids` | array of strings | UUID v7 strings of all secondary records linked to this primary |

Returns `{"ids": []}` if the primary exists but has no associated secondaries.

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/secondaries",
    "params": {"primary_id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b"},
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "ids": [
      "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4b00",
      "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4b01"
    ]
  },
  "id": 1
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `ShardsManager` singleton not initialised |
| `-32004` | Secondary listing query failed |
| `-32600` | `primary_id` is not a valid UUID |
| `-32404` | No primary record found with the given UUID |

## Notes

- Shard lookup uses the UUID v7 timestamp fast path with a linear fallback — see [`v2/primary`](v2_primary.md#notes) for details.
- The `secondaries_count` field in a `v2/primary` response equals `len(ids)` from this method.
