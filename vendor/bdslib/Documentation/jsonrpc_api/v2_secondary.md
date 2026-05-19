# v2/secondary

Returns the full document for a single secondary record identified by its UUID.

The response includes the stored JSON document plus two computed fields: the UUID of the associated primary record and the list of duplicate timestamps.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `secondary_id` | string | yes | UUID v7 of the secondary record |

## Response

The response is the original stored JSON document with two additional fields injected. Every document always contains `id`, `timestamp`, `key`, and `data`; any extra metadata fields stored alongside the document are also present.

```json
{
  "id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4b00",
  "timestamp": 1745042005,
  "key": "server.cpu",
  "data": {"host": "web-01", "value": 88.1},
  "primary_id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
  "duplications": []
}
```

| Field | Type | Description |
|---|---|---|
| `id` | string | UUID v7 of this secondary record. |
| `timestamp` | integer | Event time as Unix seconds. |
| `key` | string | Signal identifier / metric name. |
| `data` | any | Measured value as stored. |
| *(metadata fields)* | any | Any additional fields present in the original ingested document. |
| `primary_id` | string | UUID v7 of the primary record this secondary is linked to. |
| `duplications` | array of integers | Unix seconds of each exact-match duplicate of this record. Empty array if no duplicates exist. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/secondary",
    "params": {"secondary_id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4b00"},
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4b00",
    "timestamp": 1745042005,
    "key": "server.cpu",
    "data": {"host": "web-01", "value": 88.1},
    "primary_id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
    "duplications": []
  },
  "id": 1
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `ShardsManager` singleton not initialised |
| `-32004` | Observability query failed |
| `-32005` | Primary-of relationship lookup failed |
| `-32600` | `secondary_id` is not a valid UUID |
| `-32404` | No secondary record found with the given UUID, or no primary link exists |

## Notes

- Shard routing for the secondary uses the UUID v7 timestamp fast path with a linear fallback — see [`v2/primary`](v2_primary.md#notes) for details.
- The `primary_id` field is resolved from the `primary_secondary` relationship table stored in the same shard.
