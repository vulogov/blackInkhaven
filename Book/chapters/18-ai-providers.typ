#import "../design.typ": *

#chapter(number: 18, part: "Part VI — Working with AI",
  title: "Configuring AI providers")

#dropcap("S")ix LLM providers ship inside the inkhaven binary
via the `genai` crate. Configuration is one stanza in
`inkhaven.hjson`; switching between providers at runtime
is `Ctrl+B L`.

#section("The full set")

#chord_table((
  chord_row("gemini", "Google. Set `GEMINI_API_KEY`."),
  chord_row("claude", "Anthropic. Set `ANTHROPIC_API_KEY`."),
  chord_row("openai", "OpenAI. Set `OPENAI_API_KEY`."),
  chord_row("deepseek", "DeepSeek. Set `DEEPSEEK_API_KEY`."),
  chord_row("grok", "xAI Grok. Set `XAI_API_KEY`."),
  chord_row("ollama", "Local — no API key. Set `host` if not on localhost."),
))

Pick one as default; the others stay configurable for
`Ctrl+B L` live-switching.

#section("Minimal configuration")

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

Inkhaven reads the API key from the matching environment
variable. No keys in the HJSON, no keys in git. The
config holds only the provider name + the model preference.

#section("Live switching — Ctrl+B L")

`Ctrl+B L` opens a small picker over every configured
provider:

#figure_slot(
  id: "ctrl-b-l-llm-picker",
  caption: "Ctrl+B L — provider picker. Current provider marked. Enter switches.",
  height: 40mm,
)

The switch is per-session; the default in HJSON survives a
restart. Useful for picking the cheap fast model for grammar
checks and the heavy model for critique.

#section("Inference mode — Ctrl+B M / F10")

#chord_table((
  chord_row("Local", "The model is constrained to the supplied RAG context. Won't draw on outside knowledge."),
  chord_row("Full", "The model can augment context with general knowledge (still treats context as ground truth)."),
))

`F10` toggles the mode globally. `Ctrl+B M` shows the current
state in the status bar. Help RAG (`F1`) is always pinned
to Local — the help-answer flow refuses to confabulate
features that don't exist.

#section("AI scope — F9")

`F9` cycles the inference scope:

#chord_table((
  chord_row("None", "Just the user's typed query."),
  chord_row("Selection", "+ the current editor selection."),
  chord_row("Paragraph", "+ the whole open paragraph."),
  chord_row("Subchapter", "+ every paragraph in the current subchapter."),
  chord_row("Chapter", "+ every paragraph in the current chapter."),
  chord_row("Book", "+ every paragraph in the current book."),
))

Cycle forward with F9; backward with Shift+F9. The current
scope shows in the AI pane's title bar. Auto-resets to
None after every send so you don't accidentally re-attach
a huge context to your next casual question.

#section("Cost discipline")

The default mode for cloud providers is Full + Paragraph
scope — substantial but not enormous. A typical
500-word-paragraph + 1500-token response is a fraction of
a cent on every provider except Claude Opus.

If you're using a cloud provider heavily, set
`ai.per_paragraph_memory_max_turns` to something modest
(default 10 → consider 4-6) so paragraph-scoped chats
don't grow unbounded.

#callout(label: "If you want totally local")[
  Set `llm.default_provider: "ollama"` + run any local
  model. Nothing leaves your machine. Inkhaven's RAG
  pipelines, embedding, and search are all already local;
  the LLM is the last network call to silence.
]

#section("Configuring per-provider")

Each provider has its own block under `llm.<provider>:`. Common
fields:

#chord_table((
  chord_row("model", "Which model name to use."),
  chord_row("host (ollama)", "URL of the Ollama API."),
  chord_row("base_url (openai)", "Override for OpenAI-compatible endpoints (e.g. local proxies)."),
  chord_row("max_tokens", "Cap for the response length (default varies by provider)."),
))

The full reference is in Appendix B + `Documentation/CONFIGURATION.md`.

#recap((
  [Six providers bundled: Gemini, Claude, OpenAI, DeepSeek, Grok, Ollama.],
  [API keys live in environment variables; HJSON only names the model + default provider.],
  [`Ctrl+B L` switches provider live (per session).],
  [`F10` toggles Local vs Full inference mode; `F9` cycles RAG scope.],
  [Ollama for fully-local; cloud providers for heavier critique.],
))
