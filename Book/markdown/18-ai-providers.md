# 18 — Configuring AI providers

Six LLM providers ship inside the inkhaven binary via the `genai` crate. Configuration is one stanza in `inkhaven.hjson`; switching between providers at runtime is `Ctrl+B L`.

## The full set

| Provider | Env var | Notes |
|----------|---------|-------|
| gemini | `GEMINI_API_KEY` | Google. |
| claude | `ANTHROPIC_API_KEY` | Anthropic. |
| openai | `OPENAI_API_KEY` | OpenAI. |
| deepseek | `DEEPSEEK_API_KEY` | DeepSeek. |
| grok | `XAI_API_KEY` | xAI Grok. |
| ollama | (no key) | Local — set `host` if not on localhost. |

Pick one as default; the others stay configurable for `Ctrl+B L` live-switching.

## Minimal configuration

```hjson
llm: {
  default_provider: "ollama"

  ollama: {
    model: "qwen2.5:7b"
    host:  "http://localhost:11434"
  }

  claude: {
    model: "claude-sonnet-4-6"
  }

  openai: {
    model: "gpt-4o-mini"
  }
}
```

Inkhaven reads the API key from the matching environment variable. No keys in the HJSON, no keys in git. The config holds only the provider name + the model preference.

## Live switching — Ctrl+B L

`Ctrl+B L` opens a small picker over every configured provider:

![figure: ctrl-b-l-llm-picker](images/ctrl-b-l-llm-picker.png) — Ctrl+B L: provider picker. Current provider marked. Enter switches.

The switch is per-session; the default in HJSON survives a restart. Useful for picking the cheap fast model for grammar checks and the heavy model for critique.

## Inference mode — Ctrl+B M / F10

| Mode | What it does |
|------|--------------|
| Local | The model is constrained to the supplied RAG context. Won't draw on outside knowledge. |
| Full | The model can augment context with general knowledge (still treats context as ground truth). |

`F10` toggles the mode globally. `Ctrl+B M` shows the current state in the status bar. Help RAG (`F1`) is always pinned to Local — the help-answer flow refuses to confabulate features that don't exist.

## AI scope — F9

`F9` cycles the inference scope:

| Scope | What it sends |
|-------|---------------|
| None | Just the user's typed query. |
| Selection | + the current editor selection. |
| Paragraph | + the whole open paragraph. |
| Subchapter | + every paragraph in the current subchapter. |
| Chapter | + every paragraph in the current chapter. |
| Book | + every paragraph in the current book. |

Cycle forward with F9; backward with Shift+F9. The current scope shows in the AI pane's title bar. Auto-resets to None after every send so you don't accidentally re-attach a huge context to your next casual question.

## Cost discipline

The default mode for cloud providers is Full + Paragraph scope — substantial but not enormous. A typical 500-word-paragraph + 1500-token response is a fraction of a cent on every provider except Claude Opus.

If you're using a cloud provider heavily, set `ai.per_paragraph_memory_max_turns` to something modest (default 10 → consider 4-6) so paragraph-scoped chats don't grow unbounded.

> **If you want totally local:** Set `llm.default_provider: "ollama"` + run any local model. Nothing leaves your machine. Inkhaven's RAG pipelines, embedding, and search are all already local; the LLM is the last network call to silence.

## Configuring per-provider

Each provider has its own block under `llm.<provider>:`. Common fields:

| Field | Role |
|-------|------|
| `model` | Which model name to use. |
| `host` (ollama) | URL of the Ollama API. |
| `base_url` (openai) | Override for OpenAI-compatible endpoints (e.g. local proxies). |
| `max_tokens` | Cap for the response length (default varies by provider). |

The full reference is in Appendix B + `Documentation/CONFIGURATION.md`.

## Recap

- Six providers bundled: Gemini, Claude, OpenAI, DeepSeek, Grok, Ollama.
- API keys live in environment variables; HJSON only names the model + default provider.
- `Ctrl+B L` switches provider live (per session).
- `F10` toggles Local vs Full inference mode; `F9` cycles RAG scope.
- Ollama for fully-local; cloud providers for heavier critique.
