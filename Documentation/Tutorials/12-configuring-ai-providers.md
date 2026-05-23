# 12 — Configuring AI providers

Inkhaven 1.1 ships with six pre-configured LLM provider stanzas:
**Gemini**, **Claude**, **OpenAI**, **DeepSeek**, **Grok**, and a
local-Ollama fallback. You pick which one drives the AI pane; you
can switch live without leaving the TUI; and adding a new model is
a one-line HJSON edit.

This tutorial walks through the configuration end to end: picking a
provider, setting the API key, switching mid-session, and upgrading
a model.

## What ships out of the box

A fresh `inkhaven init <project>` writes this stanza into
`inkhaven.hjson`:

```hjson
llm: {
  default: gemini
  providers: {
    gemini:    { model: gemini-2.5-pro,    api_key_env: GEMINI_API_KEY    }
    claude:    { model: claude-sonnet-4-5, api_key_env: ANTHROPIC_API_KEY }
    openai:    { model: gpt-4o,            api_key_env: OPENAI_API_KEY    }
    deepseek:  { model: deepseek-chat,     api_key_env: DEEPSEEK_API_KEY  }
    grok:      { model: grok-2-latest,     api_key_env: XAI_API_KEY       }
    ollama:    { model: llama3.2 }                  // no key — local
  }
}
```

The underlying `genai` crate dispatches by model name, not by the
human label:

| Model-name prefix       | Adapter   |
| ----------------------- | --------- |
| `gpt-*`, `o1-*`, `chatgpt-*` | OpenAI |
| `claude-*`              | Anthropic (Claude) |
| `gemini-*`              | Gemini |
| `grok-*`                | xAI |
| `deepseek-*`            | DeepSeek |
| everything else         | Ollama (local) |

So renaming a provider entry doesn't change behaviour — only the
`model:` value does.

## Pick one and set the key

Export the env var Inkhaven expects (the names match the official
SDK conventions):

```bash
# Anthropic / Claude
$ export ANTHROPIC_API_KEY='sk-ant-…'

# OpenAI
$ export OPENAI_API_KEY='sk-…'

# Google / Gemini
$ export GEMINI_API_KEY='AI…'

# xAI / Grok
$ export XAI_API_KEY='xai-…'

# DeepSeek
$ export DEEPSEEK_API_KEY='…'
```

Then either edit `llm.default` directly:

```hjson
llm: {
  default: claude       // ← change here
  ...
}
```

…or use the live switcher (see below).

For **Ollama**, no key is needed — pull a model on the host running
Inkhaven and switch via `Ctrl+B L`:

```bash
$ ollama pull llama3.2
$ ollama list
```

## Switch providers live with Ctrl+B L

The fastest workflow is the **Ctrl+B L** floating picker. It works
from any pane:

```
 Switch LLM provider · Ctrl+B L
   gemini      gemini-2.5-pro       · GEMINI_API_KEY set       (current)
 › claude      claude-sonnet-4-5    · ANTHROPIC_API_KEY set
   openai      gpt-4o               · OPENAI_API_KEY MISSING
   deepseek    deepseek-chat        · DEEPSEEK_API_KEY MISSING
   grok        grok-2-latest        · XAI_API_KEY set
   ollama      llama3.2             · local (no key)
```

Notes on the picker:
- The third column tells you whether the env var is **set**, **missing**,
  or **local** (Ollama). You won't accidentally switch to a provider
  whose key isn't in your environment — well, you can, but the next
  inference will surface the env-var error in the AI pane and the
  status bar.
- The `(current)` tag marks `llm.default`. Pressing Enter on the
  same row is a no-op confirm; pressing Enter on a different row
  switches.

When you commit a switch, Inkhaven:

1. Rewrites **only the `default:` line** inside `llm: { … }` —
   every comment, indentation rule, and other field in your
   `inkhaven.hjson` is preserved byte-for-byte.
2. Rebuilds the in-memory `AiClient` immediately so the next prompt
   uses the new provider — no restart.
3. Reports the change in the status bar:

   ```
   LLM provider switched to `claude` · saved to inkhaven.hjson
   ```

If the picker tells you the env var is missing, set it in your
shell and **restart inkhaven** — the binary reads env vars once at
launch.

## Upgrade a model

Models move fast. Inkhaven doesn't tie you to the defaults — open
`inkhaven.hjson` and rewrite the `model:` field:

```hjson
claude: {
  model: claude-opus-4         // was claude-sonnet-4-5
  api_key_env: ANTHROPIC_API_KEY
}

openai: {
  model: gpt-5-pro             // was gpt-4o
  api_key_env: OPENAI_API_KEY
}
```

genai inspects the model prefix on the next inference and picks
the right adapter automatically. `gpt-5*` routes through OpenAI's
Responses API; `gpt-4*` through Chat Completions — no manual flag
needed.

Reload the project (close and re-open inkhaven, or just edit and
re-launch) for the change to take effect.

## Add a custom provider entry

The `providers` map is open-ended — add as many entries as you
like. A common pattern is multiple Claude rows for different
quality / cost trade-offs:

```hjson
providers: {
  claude-sonnet: {
    model: claude-sonnet-4-5
    api_key_env: ANTHROPIC_API_KEY
  }
  claude-opus: {                  // expensive but smarter
    model: claude-opus-4
    api_key_env: ANTHROPIC_API_KEY
  }
  claude-haiku: {                 // cheap + fast
    model: claude-haiku-4
    api_key_env: ANTHROPIC_API_KEY
  }
}
```

All three share the same key but the picker lists them as three
choices. Switch with `Ctrl+B L` based on the task — Opus for hard
reasoning, Haiku for batch edits.

## When something goes wrong

The AI pane's status line tells you. Common patterns:

| Message                                    | Cause                              | Fix                              |
| ------------------------------------------ | ---------------------------------- | -------------------------------- |
| `ANTHROPIC_API_KEY not set in environment` | Provider chosen but key missing    | Export the key + relaunch        |
| `unknown llm provider \`xxx\``             | `llm.default` doesn't match any provider key | Fix the typo in `inkhaven.hjson` |
| `inference error: 401` (or 429, 503, …)    | Provider-side error                | Check the provider's status page |

For typst-compile-time errors during `Ctrl+B B` build (a separate
flow), the captured stderr is auto-piped into a fresh AI chat with
the configured `typst_compile.error_system_prompt` — the model
gets enough context about inkhaven's file layout to diagnose the
problem from scratch.

## Privacy posture

Inkhaven does **not** provide inherent privacy when one of
the five cloud providers (Gemini, Claude, OpenAI, DeepSeek,
Grok) is configured. Every prompt + every RAG-attached
paragraph travels to that provider's servers under their
terms of service. They may log, train on, or otherwise
retain what you send. The chord interface
(`F9` / `F10` / `Ctrl+B L` / `Ctrl+B C`) gives you scope
and mode control, but it doesn't change that something
left your machine the moment you pressed the chord.

For **increased privacy**, set
`llm.default_provider: "ollama"` and run a local Ollama
instance. Every inkhaven AI feature (F7 grammar, F12
critique, Ctrl+F12 explain, F1 Help RAG, timeline
critique, chat, per-paragraph memory) then runs locally.
Inkhaven's other AI-adjacent subsystems are already
on-device:

- RAG embedding (fastembed → ONNX runtime, no network)
- Semantic search (HNSW vector store, no network)
- Snapshot diff, lexicon stemming (pure Rust, no network)

Ollama closes the loop on the LLM itself.

```hjson
llm: {
  default_provider: "ollama"
  ollama: { model: "qwen2.5:7b" }
}
```

You can still keep the other providers configured —
`Ctrl+B L` switches per-session — but the default
provider decides where unattended AI chords (idle hooks
firing critiques, auto-suggest-event, etc.) send their
prompts.

## Next steps

- [`13-ai-full-screen-mode.md`](13-ai-full-screen-mode.md) — the
  Ctrl+B K layout where the AI pane and chat history take the whole
  screen, with persistent history and chat search.
- [`05-ai-writing-assistant.md`](05-ai-writing-assistant.md) — the
  fundamentals of the AI pane: scopes (F9), inference modes (F10),
  chat history, the prompt picker.
