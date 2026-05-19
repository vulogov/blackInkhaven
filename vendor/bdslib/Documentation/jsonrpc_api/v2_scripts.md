# v2/scripts

List every stored BUND script — metadata only, no script bodies.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `session` | string | no | UUIDv7 transaction identifier (echoed only). |

## Response

```json
{
  "scripts": [
    {
      "id":       "019e0c09-a151-7940-ac19-02b1fa1d0dd5",
      "name":     "hello",
      "schedule": "*/5 * * * *",
      "metadata": { "name": "hello", "schedule": "*/5 * * * *" }
    },
    {
      "id":       "019e0c09-a15d-7302-aab0-0d565461fcfb",
      "name":     "daily_report",
      "schedule": "0 9 * * *",
      "metadata": { "name": "daily_report", "schedule": "0 9 * * *", "owner": "ops" }
    }
  ]
}
```

| Field | Type | Description |
|---|---|---|
| `scripts` | array | Each entry contains `id`, `name`, `schedule`, and the full `metadata` object. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"v2/scripts","params":{"session":"-"},"id":1}' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable |
| `-32004` | Script store query failed |

## Notes

Scripts whose metadata is missing the `schedule` field are still listed (the field appears as an empty string).
