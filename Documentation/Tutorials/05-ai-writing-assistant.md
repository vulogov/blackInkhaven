# 5 — The AI writing assistant

The AI pane is where Inkhaven streams answers from an LLM (Gemini,
DeepSeek, Ollama, …) and where you decide how to apply them. This
tutorial covers the workflow: prompts, scopes, modes, chat history,
and applying results to the editor buffer.

## What an LLM is, briefly

A **Large Language Model** is a neural network trained on text. You
send it a question or instruction (the **prompt**); it generates a
response one token at a time. Inkhaven streams those tokens into the
AI pane as they arrive — typing isn't simulated, the model is
literally producing characters left-to-right.

You will hear three terms used loosely:

- **Provider** — the company or runtime that hosts the model (Google
  for Gemini, DeepSeek for DeepSeek, Ollama if you run a model locally).
- **Model** — a specific named version (`gemini-2.5-pro`,
  `deepseek-chat`, `llama3.2`).
- **API key** — a secret string you set in an environment variable
  that authenticates your account with the provider. Local providers
  (Ollama) need no key.

Inkhaven uses the [genai](https://github.com/jeremychone/rust-genai)
crate, which picks the right adapter from the model name. See
[`../CONFIGURATION.md`](../CONFIGURATION.md#llm) for the provider
block.

## Set up a provider

Pick one and set its key. For Gemini:

```bash
$ export GEMINI_API_KEY='your-key-here'
```

For DeepSeek:

```bash
$ export DEEPSEEK_API_KEY='your-key-here'
```

For Ollama (no key needed; just install Ollama and pull a model):

```bash
$ ollama pull llama3.2
```

Then in `inkhaven.hjson` set the default:

```hjson
llm: { default: ollama }
```

You can keep multiple providers configured and switch the default
when you want; or use the CLI with `--provider name`:

```bash
$ inkhaven ai "summarise this passage" --provider deepseek
```

## First inference

Inside the TUI:

1. Open a paragraph in the editor.
2. Press `Ctrl+I`. Focus moves to the **AI prompt** bar at the
   bottom.
3. Type a question: `What's a darker phrase for 'thunderstruck mariner'?`
4. Press `Enter`.

Streaming starts immediately in the AI pane:

```
┌── AI — gemini · streaming… · infer=Full ─────────────────────┐
│ Several alternatives:                                         │
│                                                               │
│ - **stunned sailor** — keeps the alliteration                 │
│ - **shaken helmsman** — slightly nautical                     │
│ - **dazed deckhand** — focuses on disorientation             │
│ - **rattled bosun** — period appropriate                     │
│                                                               │
│ "Thunderstruck" reads pre-20th century; if the manuscript…   │
└───────────────────────────────────────────────────────────────┘
```

The title strip shows `streaming…` while tokens arrive, then `done`
when complete. The status bar at the bottom reports elapsed time.

Focus stays on the AI prompt bar throughout — you can type a
follow-up question and press Enter again to continue the conversation
(see [Chat history](#chat-history) below).

## Pressing Esc

`Esc` from the AI prompt bar **bounces focus** to the AI pane so you
can read and scroll. Press `Esc` from the AI pane and it bounces
back to the AI prompt. This Ai ↔ AiPrompt pairing is independent of
the Editor / Tree / Search rotation.

## Markdown rendering

The AI pane renders the response as markdown — bold (`**foo**`),
italic (`*foo*`), inline code (`` `foo` ``), headings (`# Foo`),
lists, code fences, blockquotes. So the response above with bullet
points actually shows up as a proper list. This is purely a display
thing — the raw markdown is what gets stored in `inference.response`
and what flows through the apply pipeline.

## Applying the result to the editor

When the inference is **done** and has non-empty content, action chips
appear in the AI pane's footer:

```
 r replace  i insert  t top  b bottom  c copy  g grammar
```

Each key applies the response to the editor in a different way:

| Key | Action |
| --- | ------ |
| `r` / `R` | **Replace** — overwrite the editor selection with the AI text. With no selection, replaces the whole paragraph. |
| `i` / `I` | **Insert** at cursor. |
| `t` / `T` | **Top** — prepend to the top of the paragraph (blank line separator). |
| `b` / `B` | **Bottom** — append. |
| `c` / `C` | **Copy** to clipboard only (no edit). |
| `g` / `G` | **Grammar** — only valid for F7 grammar-check output; lifts the corrected text between markers (see [`06-grammar-check.md`](06-grammar-check.md)). |

Three things to know about apply:

1. **Markdown → Typst** — when applying to the editor (`r`, `i`, `t`,
   `b`), the markdown is converted to Typst syntax (`#` → `=`,
   `**bold**` → `*bold*`, `1.` → `+`, etc.). `c` keeps the raw
   markdown so you can paste it elsewhere.
2. **Dirty marker** — applies set the buffer dirty; `Ctrl+S` to
   commit.
3. **Focus moves to the editor** — after `r`/`i`/`t`/`b`, focus
   jumps to the editor pane so you can review.

## Scope (F9): tell the AI what to look at

By default, the AI sees only the prompt you type plus the chat
history. To attach a chunk of your manuscript, cycle the **scope**
with `F9`:

```
None → Selection → Paragraph → Subchapter → Chapter → Book → None
```

Each non-None scope prepends matching content to the next prompt and
**auto-resets to None** after submission so you don't accidentally
re-attach.

| Scope | What it sends |
| ----- | ------------- |
| **None** | Nothing extra. Just the prompt and prior chat turns. |
| **Selection** | The current editor selection. Errors out if no selection is active. |
| **Paragraph** | The full open paragraph. In split-edit, both the snapshot and the live buffer are sent. |
| **Subchapter** | Every paragraph nested under the cursor's enclosing subchapter. |
| **Chapter** | Same but for the enclosing chapter. |
| **Book** | Same but for the enclosing book. |

The scope is visible in two places:

- **Status bar** while armed: `AI scope: Paragraph (will prepend
  matching context to next prompt)`.
- **AI pane title chip**: `… · scope=Paragraph · …` (peach by default
  — `theme.ai_scope_fg`).

After Enter, the scope resets to None and the chip disappears.

### Choosing a scope

- **Brainstorm a single sentence** — scope=Selection (or None).
- **Tighten one paragraph** — scope=Paragraph.
- **Check consistency within a scene** — scope=Subchapter.
- **Plot continuity across a chapter** — scope=Chapter.
- **Whole-book reviews** — scope=Book (very long; mostly Gemini
  handles it).

## Inference mode (F10): Local vs Full

Two modes, toggled by F10:

| Mode | System prompt | Use when… |
| ---- | ------------- | --------- |
| **Local** | Model is told to use **only** the supplied context (and prior chat) — refuse rather than fall back on general knowledge. | You're asking the model to summarise your own canon, fact-check against your worldbuilding, or work strictly inside the manuscript. |
| **Full** | Context is ground truth where present, but general knowledge is fair game. | Default for chat / brainstorm — the model can pull in references, suggest tropes, mention craft books. |

Default is **Full**. The mode chip is always shown in the AI title
(`infer=Local` or `infer=Full`, teal by default —
`theme.ai_infer_fg`) so an accidentally-armed Local mode is obvious.

Help inferences (`F1` / `Help!` prefix) and Grammar inferences (`F7`)
pin to Local regardless of the toggle — they have dedicated strict
system prompts.

## Chat history

Every non-Help / non-Grammar inference appends a `(user, assistant)`
turn to in-memory chat history. The next prompt replays the whole
history so the conversation is **continuous** — the model knows what
you and it said earlier in the session.

The AI title shows the current depth:

```
AI — gemini · done · infer=Full · 3 turn(s)
```

Clear the history with:

- **`Ctrl+B C`** — drops every turn + the current inference.
- **F9** (no — F9 cycles scope; was repurposed from the old "clear"
  semantics).

A typical multi-turn flow:

1. Set scope=Paragraph, F9 once.
2. Prompt: "Tighten this." Send.
3. Apply `r` to replace the paragraph.
4. Prompt: "Now do another tightening pass, especially on dialogue
   tags." Send. (History knows what "this" was — the previous
   turn replayed it.)
5. `g` to apply. Or `Ctrl+B C` to start fresh.

## The prompt picker (/)

For reusable prompt templates, type `/` in the AI prompt bar. The
picker opens:

```
┌── Prompts ─────────────────────────────────────────────────┐
│  system  /tighten                                          │
│         Tighten the prose without changing meaning         │
│  system  /darker                                           │
│         Make the tone darker, keep facts                   │
│   book   /worldbuilding-pass                               │
│         Run a worldbuilding consistency pass               │
└────────────────────────────────────────────────────────────┘
```

Two sources of prompts:

- **System** — `prompts.hjson` in the project root.
- **Book** — paragraphs under the **Prompts** system book.

See [`../PROMPTS.md`](../PROMPTS.md) for the full schema, substitutions
(`{{selection}}` and `{{context}}`), and patterns. Highlights here:

- Arrow keys move; `Enter` or `Tab` expands the selected template into
  the bar (with substitutions applied). You can edit the expanded text
  before sending.
- Direct invocation works too: type `/tighten` and Enter — no picker
  needed.
- `Esc` closes without expanding.

## Help! prefix

In the AI prompt bar, typing `Help! …` runs the rest of the line
through the F1 help-manual flow (RAG over the Help system book) —
identical to pressing F1 and typing the question into the floating
pane. Useful when you don't want to leave the AI prompt.

The case matters: `Help!` only, not `help!` or `HELP!`. The Help book
must be populated first (see
[`08-importing-existing-docs.md`](08-importing-existing-docs.md)).

## Multi-provider workflows

Different inferences sometimes want different providers. You can:

- Change `llm.default` in `inkhaven.hjson` and restart the TUI.
- For CLI one-shots: `inkhaven ai "prompt" --provider deepseek`.
- For the TUI: switch the default in the config and restart between
  sessions — there's no per-inference provider override inside the
  TUI today.

A common pattern: `default: ollama` (local, free, no token cost) for
brainstorming, switch to `default: gemini` when you want a manuscript-
wide review.

## Status bar messaging during inference

While streaming you see things like:

```
streaming from gemini (chat turn #3 · scope=Paragraph)…
```

Once done:

```
gemini responded in 1.8s
```

If something fails (API key missing, model name wrong, network
hiccup), the status flips to an error and the AI pane title carries
`error`. The error text is in the AI pane body. Fix the issue
(e.g. set the env var) and re-send.

## What you have learned

- An LLM streams tokens into the AI pane in response to your prompt.
- `Ctrl+I` focuses the AI prompt. `Enter` sends. `Esc` bounces focus.
- The AI pane renders markdown.
- After done, `r`/`i`/`t`/`b` apply to the editor (with
  markdown→Typst), `c` copies, `g` is the grammar-check apply.
- F9 cycles **scope**; F10 toggles **inference mode**.
- Chat history accumulates; `Ctrl+B C` clears it.
- `/` opens the prompt picker; `Help!` prefix routes to the
  help-manual flow.

## Next steps

- [`06-grammar-check.md`](06-grammar-check.md) — the F7 / `g` apply
  flow with change highlighting.
- [`07-places-and-characters.md`](07-places-and-characters.md) —
  `Ctrl+B P` and `Ctrl+B C` for worldbuilding lookups.
