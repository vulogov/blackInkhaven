# v2/chat.ollama

Send a question to a local [Ollama](https://ollama.com) chat model with retrieval-augmented context drawn from the bdsnode observability and document stores.

The handler resolves (or creates) a chat session, builds a RAG context block from `v2/aggregationsearch` (or uses a caller-supplied context verbatim), enriches the user query with the context, and forwards everything to Ollama. The model name, base URL, and system prompt are read once at startup from `bds.hjson` (`ollama_url`, `ollama_model`, `ollama_system_prompt`); defaults are `http://localhost:11434` / `llama3.2` / a built-in SRE-analyst system prompt.

## Parameters

| Parameter | Type | Required | Default | Description |
|---|---|---|---|---|
| `chat_id` | string \| null | no | `null` | Existing session UUID. Omit (or pass `null`) to start a new session — bdsnode generates a fresh UUID and seeds it with the configured system prompt. |
| `duration` | string | yes | — | Lookback window used for the RAG retrieval (humantime: `"1h"`, `"30min"`, `"7days"`). Ignored when `context` is supplied. |
| `query` | string | yes | — | The user's natural-language question. |
| `context` | string | no | — | Pre-built RAG context. When provided, `aggregationsearch` is skipped entirely and this string is concatenated into the prompt as-is. Use this when the caller already has the relevant data on hand (e.g. from `v2/primaries.explore`). |

## RAG context shape

When `context` is omitted, bdsnode runs `aggregationsearch(duration, query)` and assembles a prompt prefix of the form:

```
Relevant observability context (last <duration>):

[telemetry 1] <json_fingerprint>
[telemetry 2] <json_fingerprint>
…
[document 1] <json_fingerprint>
[document 2] <json_fingerprint>

---

User question: <query>
```

Up to 30 telemetry hits and 10 document hits are included.

## Response

```json
{
  "chat_id":         "0192a3b4-c5d6-7e8f-9012-34567890abcd",
  "response":        "The error rate spike on auth-service started at 14:21 UTC …",
  "is_new_session":  true,
  "telemetry_count": 18,
  "document_count":  3
}
```

| Field | Type | Description |
|---|---|---|
| `chat_id` | string | Session UUID — pass back as `chat_id` on follow-up calls to maintain conversation history. |
| `response` | string | The model's reply. |
| `is_new_session` | bool | `true` when bdsnode allocated a new session for this call. |
| `telemetry_count` | integer | Number of telemetry records included in the auto-built context. `0` when caller supplied `context`. |
| `document_count` | integer | Number of documents included in the auto-built context. `0` when caller supplied `context`. |

## Example

```bash
curl -s -X POST http://127.0.0.1:9000 \
  -H 'Content-Type: application/json' \
  -d '{
    "jsonrpc": "2.0",
    "method": "v2/chat.ollama",
    "params": {
      "chat_id":  null,
      "duration": "30min",
      "query":    "What is happening with our auth service?"
    },
    "id": 1
  }' | jq
```

## Error responses

| Code | Condition |
|---|---|
| `-32000` | Internal task panic |
| `-32001` | Database unavailable, or chat-session creation failed |
| `-32004` | RAG search or Ollama call failed |
| `-32600` | Invalid `chat_id` UUID |

## Notes

- **Stateful sessions.** History is kept server-side per `chat_id`. Reuse the same `chat_id` to keep the conversation; omit to start fresh.
- **Ollama must be reachable.** If `ollama_url` cannot be contacted (e.g. the daemon isn't running), the call fails with `-32004`.
- **Configuration.** Settings come from `bds.hjson` at bdsnode startup; changing the model requires restarting bdsnode.
