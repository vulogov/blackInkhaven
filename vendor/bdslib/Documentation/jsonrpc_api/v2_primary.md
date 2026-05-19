# v2/primary

Returns the full document for a single primary record identified by its UUID.

The response includes the stored JSON document plus two computed fields: the count of associated secondary records and the list of duplicate timestamps.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `primary_id` | string | yes | UUID v7 of the primary record |

## Response

The response is the original stored JSON document with two additional fields injected. Every document always contains `id`, `timestamp`, `key`, and `data`; any extra metadata fields stored alongside the document are also present.

```json
{
  "id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
  "timestamp": 1745042000,
  "key": "server.cpu",
  "data": {"host": "web-01", "value": 87.3},
  "secondaries_count": 3,
  "duplications": [1745042010, 1745042020]
}
```

| Field | Type | Description |
|---|---|---|
| `id` | string | UUID v7 of this primary record. |
| `timestamp` | integer | Event time as Unix seconds. |
| `key` | string | Signal identifier / metric name. |
| `data` | any | Measured value as stored. |
| *(metadata fields)* | any | Any additional fields present in the original ingested document. |
| `secondaries_count` | integer | Number of secondary records linked to this primary. |
| `duplications` | array of integers | Unix seconds of each exact-match duplicate of this record (same key + data, different timestamp). Empty array if no duplicates exist. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/primary",
    "params": {"primary_id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b"},
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "id": "018f1a2b-3c4d-7e5f-8a9b-0c1d2e3f4a5b",
    "timestamp": 1745042000,
    "key": "server.cpu",
    "data": {"host": "web-01", "value": 87.3},
    "secondaries_count": 3,
    "duplications": [1745042010, 1745042020]
  },
  "id": 1
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | `ShardsManager` singleton not initialised |
| `-32004` | Observability query failed |
| `-32600` | `primary_id` is not a valid UUID |
| `-32404` | No primary record found with the given UUID |

## Notes

- Shard routing uses the UUID v7 timestamp to locate the record in O(1) without scanning all shards. A linear fallback scan across all shards is performed if the fast path misses (handles records ingested before the UUID-timestamp alignment fix).
- Use [`v2/secondaries`](v2_secondaries.md) to retrieve the IDs of the linked secondary records.
- Use [`v2/duplicates`](v2_duplicates.md) to get duplicate maps across all primaries.
