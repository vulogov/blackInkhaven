# 6 — Grammar check

`F7` runs a grammar check on the currently-open paragraph and streams
the result into the AI pane. `g` (in the AI pane) lifts only the
**corrected** text out of the response and overwrites the editor
buffer with it — Typst markup preserved, changes highlighted in red.

This tutorial walks the flow end to end and explains the moving
parts: prompt resolution, the `<<<CORRECTED>>>` markers, the apply
pipeline, and the highlight lifetime.

## Prerequisites

- A configured AI provider with its API key set (see
  [`05-ai-writing-assistant.md`](05-ai-writing-assistant.md)).
- The `language` field in `inkhaven.hjson` set to the language of
  your prose (`english`, `russian`, `german`, …). Defaults to
  `english`. The built-in grammar prompt uses this to ask for the
  right grammar rules.

## Run a grammar check

1. Open the paragraph you want to check in the Editor pane.
2. Press **`F7`**.

The status bar reports:

```
Grammar check: streaming from gemini (english)…
```

Focus moves to the **AI pane** so you can watch the response stream
in:

```
┌── AI — gemini · streaming… · infer=Full ─────────────────────┐
│ 3 grammar issues, 1 punctuation issue, otherwise clean.       │
│                                                                │
│ Issues:                                                        │
│ - "sharte:" — likely a typo; should be "short:".              │
│ - "cuntext" — should be "context".                            │
│ - "dont" — missing apostrophe; should be "don't".             │
│ - Comma splice in sentence 4: split into two sentences.       │
│                                                                │
│ <<<CORRECTED>>>                                                │
│ = Opening Scene                                                │
│                                                                │
│ The thunderstruck mariner stood at the rail. The deck         │
│ was slick. Rain had been falling for three days, and he       │
│ couldn't tell where the sky ended and the sea began.          │
│ <<<END>>>                                                      │
└────────────────────────────────────────────────────────────────┘
```

When streaming finishes, the action chips light up:

```
 r replace  i insert  t top  b bottom  c copy  g grammar
```

## What the prompt looks like

The grammar-check system prompt instructs the model to:

- Check grammar / syntax / punctuation **only** — not style.
- Preserve every Typst markup token verbatim (`= Heading`, `*bold*`,
  `_italic_`, `#link(…)[…]`, raw blocks).
- Output a summary line, then a list of issues, then the corrected
  paragraph **between `<<<CORRECTED>>>` and `<<<END>>>` markers**.

The markers are the key to the apply step — they let Inkhaven extract
only the corrected text without dragging the summary or issue list
along.

The prompt is grounded on the configured `language`. With
`language: russian`, the built-in prompt asks for Russian grammar
checking; with `language: french`, French grammar; and so on.

## Apply the correction

Press **`g`** (lowercase or `Shift+g`) in the AI pane while the
inference is done. Inkhaven:

1. Looks for the `<<<CORRECTED>>>` … `<<<END>>>` block in the
   response.
2. If found, lifts the inner text. If markers are missing, falls
   back to the last fenced code block (```` ``` …``` ````). Still
   nothing? Falls back to "text after the last line containing
   `Corrected`".
3. If none of those patterns match, **refuses** — status:
   `couldn't find corrected text in the response (expected
   <<<CORRECTED>>> block or fenced code)`. No buffer changes.
4. On match, captures the **pre-correction** lines as a baseline,
   then **swaps the whole editor buffer** for the corrected text. No
   markdown→Typst conversion runs here; the grammar prompt already
   keeps Typst markup verbatim.
5. Sets the editor dirty.

The status reports:

```
applied AI result (replaced with corrected text) — changes highlighted; Ctrl+B C dismisses
```

Focus stays on the editor.

## The change highlight

After the apply you see the corrected paragraph in the editor. Every
character that differs from the pre-correction baseline is rendered
in `theme.grammar_change_fg` + bold — **red** by default.

So if the original was:

```
The thunderstruck mariner stood at the rail. The deck were slick. Rain had been falling for three days, and we cant tell where the sky end and the sea began.
```

…and the corrected version is:

```
The thunderstruck mariner stood at the rail. The deck was slick. Rain had been falling for three days, and we can't tell where the sky ended and the sea began.
```

…then `was`, `'`, and `ed` show up in red. Untouched prose is in your
normal pane foreground.

### Highlight lifetime

The change overlay persists across many things; it goes away on these:

| Event | Highlight cleared? |
| ----- | ------------------ |
| `Ctrl+S` (manual save) | Yes — your explicit "I've reviewed" signal. |
| Idle autosave | **No** — autosave is **suspended** while the highlight is active so it doesn't disappear under you. |
| `Ctrl+B C` (clear chat) | Yes — the source inference is being discarded. |
| Editor focus loss (Tab away) | Yes — focus return means a fresh visit. |
| Paragraph switch | Yes — every paragraph starts clean. |
| Buffer edits | No, the highlight stays until one of the above. |

This gives you time to read through and edit the corrected text
without losing the visual cue.

## Override the prompt

Want a different grammar style guide (e.g. AP style, MLA, your own
house rules)? Inkhaven resolves the grammar-check prompt by
precedence:

1. **Paragraph slugged `grammar-check`** under the **Prompts** system
   book.
2. **Entry named `grammar-check`** (or `grammar check`) in
   `prompts.hjson`.
3. **Built-in fallback** with the configured language.

To override #1, add a paragraph titled `Grammar check` to the Prompts
book and write your custom prompt body. To use #2, add to
`prompts.hjson`:

```hjson
{
  name: "grammar-check"
  description: "House-style grammar pass"
  template: '''
    Run a copy-edit pass on the paragraph below, following our house
    style guide (Oxford comma, single quotes for dialogue, em-dash with
    no surrounding spaces). Preserve every Typst markup token. After
    listing issues, emit the corrected paragraph between
    <<<CORRECTED>>> and <<<END>>> markers — these are required.

    {{selection}}
  '''
}
```

**Important:** your custom prompt must instruct the model to emit
the `<<<CORRECTED>>>` / `<<<END>>>` markers (or wrap the result in a
fenced code block) — that's how the `g` apply chord knows what to
extract.

Without those markers `g` may pick up commentary along with the
correction (the heading fallback grabs everything after the last line
containing "corrected"), which is rarely what you want.

## Workflow tips

### Run grammar before save

Habit-form to press F7 then `g` before each `Ctrl+S` — clears the
typos that creep in during fast drafting.

### Run grammar after a translation pass

Useful when working in a non-native language. Set `language: russian`
(or whichever) and the model checks against that language's grammar
rather than English.

### Don't enable scope alongside F7

F7 ignores the scope cycle (F9). The grammar prompt explicitly works
on the open paragraph; setting scope wouldn't help and would confuse
the model. The Inkhaven flow guarantees `scope=None` for grammar
inferences regardless of your current F9 setting.

### Don't blindly accept

`g` overwrites the buffer wholesale. The change highlight is there
specifically so you can scan red regions and undo (`Ctrl+U`) any that
look wrong.

If the model is producing too many changes you don't want, switch
provider (`llm.default`), tighten the prompt to "minimum necessary
changes", or just edit the changes by hand after apply.

### What about the issue list?

It's in the AI pane after `g` apply too. Scroll the AI pane (focus
it, Page Up / arrows) to revisit the issues. The list helps you
understand *why* the model made each change.

## Troubleshooting

### "couldn't find corrected text in the response"

The model didn't emit markers, a fence, or a "Corrected …" line.
Either:
- Re-prompt with explicit instructions ("emit the corrected text
  between <<<CORRECTED>>> and <<<END>>>");
- Adjust your custom `grammar-check` prompt to ask for markers;
- Or fall back to `r` (Replace) which takes the entire response (you
  manually delete the issue list).

### Corrections too aggressive

The default prompt says "Check grammar / syntax / punctuation only —
not style". If the model is rewriting voice or restructuring, either:
- Switch provider (some are more aggressive editors).
- Override with a stricter custom prompt.

### Corrections too conservative

Opposite — the model says "no issues" on prose you know has errors.
Try:
- A more capable model (`gemini-2.5-pro` instead of a small Ollama
  local model).
- Explicit instructions in the prompt: "Be thorough; tone is
  literary, but typos are still typos."

### Russian (or other language) check is poor

Use a provider that handles your language well. Gemini and DeepSeek
do well on Russian; small local models may struggle. Confirm
`language: russian` is set in the config so the prompt asks for
Russian grammar.

## What you have learned

- F7 runs a grammar check on the open paragraph.
- The model is instructed to emit corrections between
  `<<<CORRECTED>>>` / `<<<END>>>` markers.
- `g` in the AI pane lifts that block and overwrites the editor.
- Changed characters are highlighted in red; the highlight stays
  visible until manual save or focus loss; autosave is suspended
  during this window.
- You can override the grammar prompt via the Prompts system book or
  `prompts.hjson`; just keep the markers (or a fenced block) so `g`
  still extracts cleanly.

## Next steps

- [`07-places-and-characters.md`](07-places-and-characters.md) —
  Place / Character RAG flows.
- [`../PROMPTS.md`](../PROMPTS.md) — writing your own grammar prompt.
