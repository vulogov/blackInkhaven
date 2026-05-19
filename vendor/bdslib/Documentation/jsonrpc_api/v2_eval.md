# v2/eval

Compile and evaluate a BUND VM script inside a named context (VM instance), then return the resulting workbench stack as a JSON array.

Each `context` name maps to a lazily-created, persistent BUND VM instance. Re-using the same context name across requests shares the VM's heap and stack state between calls, enabling multi-step interactive evaluation sessions.

## Parameters

| Parameter | Type | Required | Description |
|---|---|---|---|
| `context` | string | yes | Name of the BUND VM context to use. Created on first use; reused on subsequent calls with the same name. |
| `script` | string | yes | BUND source code to compile and evaluate. |

## Response

```json
{
  "result": [42, "hello", true, null]
}
```

| Field | Type | Description |
|---|---|---|
| `result` | array | Contents of the VM workbench stack after evaluation, serialised as a JSON array. Each element is the JSON representation of the corresponding stack value. Empty array if the workbench is empty after evaluation. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/eval",
    "params": {
      "context": "my-session",
      "script": "2 2 + ."
    },
    "id": 1
  }' | jq
```

```json
{
  "jsonrpc": "2.0",
  "result": {
    "result": [4]
  },
  "id": 1
}
```

## Error responses

| Code | Condition |
|---|---|
| `-32001` | Named context could not be acquired (context registry not initialised) |
| `-32002` | Script compilation or evaluation failed (syntax error, runtime error, etc.) |

## Notes

- **Stateful contexts.** VM state (heap, stack, defined words) persists for the lifetime of the `bdsnode` process within a given `context`. Use distinct context names to isolate independent sessions.
- **Thread safety.** Each context is protected by a mutex; concurrent requests to the same context name are serialised. Concurrent requests to different context names execute in parallel.
- **Stack semantics.** The `result` array reflects the workbench stack from bottom to top. Pushing multiple values in one script produces multiple elements.
