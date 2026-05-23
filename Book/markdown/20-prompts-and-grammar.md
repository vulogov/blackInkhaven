# 20 — Prompts and the F7 grammar check

Every AI feature in inkhaven uses a *prompt template* — a piece of text that gets sent to the model before your prose. Templates resolve through a three-step chain so you can override anywhere from "I want a tiny tweak" to "I want a totally different prompt".

## The prompt resolution chain

For every named prompt (`grammar-check`, `critique-edit`, `timeline-health`, etc.):

| Step | Source |
|------|--------|
| 1. Prompts book paragraph | Body of the paragraph whose title matches the name (case-insensitive). |
| 2. prompts.hjson entry | Falls back to a per-project HJSON file. |
| 3. Embedded default | The fallback compiled into the inkhaven binary. |

You override at any layer. The `.example` seeds in your Prompts book give you the embedded default verbatim, ready to customise.

## The Prompts book seeds

`inkhaven init` writes five seeds:

```
Prompts/
├── grammar-check.example
├── explain-diagnostic.example
├── critique-edit.example
├── critique-changes.example
└── timeline-health.example
```

Open one with Enter from the tree pane. Edit the body. F2 to rename and drop the `.example` suffix. From that moment inkhaven uses your version.

![figure: prompts-book-tree](images/prompts-book-tree.png) — Prompts book in the tree pane: five .example seeds. Rename (F2) drops the suffix to activate.

## F7 — the grammar check

`F7` (Editor scope) runs the grammar-check workflow:

1. Reads the open paragraph's body.
2. Resolves the `grammar-check` prompt.
3. Sends the prompt + paragraph body to the AI.
4. Focuses the AI pane so you can watch the stream.

The default prompt (English) is roughly: "Run a copy-edit pass. Check syntax, agreement, tense, punctuation. Preserve Typst markup. List issues then give the corrected paragraph between markers."

## `g` to apply (grammar-aware)

`g` (in AI pane) is the grammar-apply chord. Different from `r` (plain replace) because it **extracts only the corrected paragraph** from the response, ignoring the issue list and commentary.

Extraction tries, in order:

1. `<<<CORRECTED>>>` / `<<<END>>>` markers (the canonical form the prompt instructs the model to produce).
2. Relaxed bracket pairs — `<<>>` / `<<END>>` / Unicode `«»` / `≪≫` (deepseek and other models drift to these compressed forms).
3. The last fenced code block.
4. Text after a `Corrected:` heading.

If none match, `g` refuses with a clear hint. `r` falls through to the markdown-conversion path on the full response.

## Visual diff after apply

`g` paints the diff into the editor — added text in green, unchanged text plain. Saves automatically. The diff stays visible until you switch paragraphs (or `Ctrl+B C` clears it).

![figure: grammar-apply-diff](images/grammar-apply-diff.png) — After `g`: corrected paragraph in place, additions highlighted green. Survives saves; cleared by Ctrl+B C.

## Customising the grammar prompt

Open `Prompts/grammar-check.example` in the editor. The body is the default prompt. Rewrite it — common changes:

- **Different language** — replace "English" with your manuscript's language. Stemmer + the AI both pick it up.
- **Genre voice** — add "preserve the present-tense literary style; flag any past-tense slips".
- **Stricter / looser** — "ignore comma splices, this is a stylistic choice" or "flag every comma splice".

F2 to rename: `grammar-check.example` → `grammar-check`. The next F7 picks up your version.

> **Round-trip after editing:** After renaming, test on a paragraph you know — F7, watch the AI use your prompt, `g` to apply. If the markers aren't right (model didn't emit `<<<CORRECTED>>>`), check the prompt — your override needs the marker instructions too.

## Other prompt overrides

The same flow works for the other four seeds:

- **`explain-diagnostic`** — Ctrl+F12 (Chapter 24).
- **`critique-edit`** — F12 in plain edit mode (Chapter 21).
- **`critique-changes`** — F12 in split-edit mode.
- **`timeline-health`** — y/Y/Ctrl+Y in the timeline view (Chapter 17).

All five resolve through the same Prompts-book → HJSON → embedded chain.

## Recap

- Prompt resolution: Prompts book paragraph → prompts.hjson → embedded fallback.
- Five `.example` seeds land at init; rename (F2) to drop suffix and activate.
- `F7` runs grammar check; `g` extracts only the corrected paragraph + applies.
- Visual diff after apply persists until paragraph switch or `Ctrl+B C`.
- Customise per-prompt by editing the seed body.
