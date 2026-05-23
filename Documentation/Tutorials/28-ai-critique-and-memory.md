# 28 — AI critique, per-paragraph memory, diff review

Inkhaven 1.2.6 expanded the AI surface with three additions
that compound: an F12 critique chord, opt-in per-paragraph
chat memory, and a diff-review modal that gates every
buffer-replacing apply.

This tutorial walks the three features as one workflow
because in practice they're used together.

## F12 — critique (mode-aware)

`F12` (Editor scope) sends the open paragraph to the AI pane
with a configurable critique prompt. The prompt **varies by
editor mode**:

| Mode             | Prompt resolved from   | What the AI sees |
|------------------|------------------------|------------------|
| **Plain edit**   | `critique-edit`        | The full paragraph body. |
| **Split-edit** (F4) | `critique-changes`   | The snapshot body (`Before`) + the live buffer (`After`). |

Both names resolve through the standard precedence chain:
Prompts book paragraph → `prompts.hjson` → embedded fallback.
The seed files `03-critique-edit-example.typ` and
`04-critique-changes-example.typ` land in the Prompts book on
`inkhaven init`; rename either to drop the `.example` suffix
to take effect.

Sample default for `critique-edit`:

> Read the paragraph below as a draft. Point out the weakest
> two or three elements: vague verbs, abstract nouns where the
> concrete would land harder, sentences that lose the reader,
> rhythm that flattens, claims that wobble, imagery that doesn't
> earn its place. Be specific — quote the exact phrase and
> propose a tighter alternative. Do NOT rewrite the whole
> paragraph; critique it. Be honest, not destructive.

The response streams into the AI pane like F7 grammar check
or any other one-shot. No chat history replay; no turn
recorded into the visible chat. Pure consult.

## Per-paragraph AI memory (1.2.6+)

When `ai.per_paragraph_memory: true` is set in HJSON, every
**Paragraph-scoped** AI prompt records its turn onto the open
paragraph's `Node.ai_memory` field. Subsequent
Paragraph-scoped prompts prepend that memory to the chat
history sent to the model — so the AI sees prior context
without polluting the project-wide visible chat.

```hjson
ai: {
  per_paragraph_memory:           true   # default false
  per_paragraph_memory_max_turns: 10     # oldest pair evicts first
}
```

Workflow:

1. Open paragraph `A`. Cycle AI scope to **Paragraph** with
   `F9`.
2. Type a question about `A`, send. The exchange records onto
   `A.ai_memory`.
3. Send another question (still Paragraph scope on `A`) — the
   model sees turn 1's content automatically prepended.
4. Switch to paragraph `B`. The model starts fresh — `B` has
   its own memory.
5. Come back to `A` days later. Send another question. The
   stored memory replays, you continue the conversation.

Mechanics:

- Storage: `Node.ai_memory: Vec<AiMemoryTurn>` where each
  turn is `{ role: "user"|"assistant", text: String }`.
- Recording fires after the stream completes successfully
  and the user message was non-empty. One-shots (F7 grammar,
  F12 critique, Ctrl+F12 explain, Help RAG) skip recording.
- Trimming is pair-aligned: when length exceeds
  `max_turns`, the oldest user+assistant pair evicts as a
  unit.
- Memory comes BEFORE the visible session chat history in
  the payload — it's the prologue, current-session turns
  follow.

When OFF (default), the AI surface stays project-wide as it
was pre-1.2.6.

## Diff review on apply (1.2.6+)

`ai.diff_review_on_apply: true` (default) routes every
buffer-replacing AI apply through a **side-by-side diff
modal** before any bytes hit the paragraph.

Affected chords:

| Chord | What it applies | Goes through diff? |
|-------|-----------------|---------------------|
| `r` / `R` | Replace whole buffer | **Yes** |
| `g` / `G` | Replace with grammar-corrected text only | **Yes** |
| `i` / `I` | Insert at cursor | No (additive) |
| `t` / `T` | Prepend (top) | No (additive) |
| `b` / `B` | Append (bottom) | No (additive) |
| `c` / `C` | Copy to clipboard | No (clipboard only) |

The modal lays out the current buffer on the left and the
proposed result on the right, with line-level diff markers:

```
┌── AI diff review — a accept · r reject ─────────────────────┐
│  = Chapter 1                  │  = Chapter 1                │
│                               │                              │
│- The storm came in fast.      │+ Storm came in fast — the   │
│- It was loud.                 │+ wind a high keen against   │
│                               │+ the eaves.                 │
│   Aerin pulled her cloak…     │   Aerin pulled her cloak…   │
│                               │                              │
│  before (left) ─ after (right) · scroll 0/24 · ↑↓ PgUp PgDn │
└──────────────────────────────────────────────────────────────┘
```

| Chord     | Effect |
|-----------|--------|
| `a` / `A` / Enter | Accept — apply the rewrite, refocus the editor. |
| `r` / `R`         | Reject — close, buffer unchanged. |
| `e` / `E`         | Accept + edit (alias for `a` since 1.2.6). |
| `↑` / `↓`         | Scroll one line. |
| `PgUp` / `PgDn`   | Scroll ten lines. |
| `Home` / `End`    | Jump to top / bottom. |
| `Esc`             | Same as reject. |

The modal status bar reports `✂ extracted X/Y chars` when
the extraction layer (see below) trimmed commentary from
the response.

## Smart extraction (1.2.6+)

For Replace-style applies (`r` and `g`), inkhaven scans the
AI's reply for a **discrete corrected block** before
applying. Falls through in order:

1. `<<<CORRECTED>>>` … `<<<END>>>` markers (the canonical
   grammar-prompt format).
2. **Relaxed bracket pairs** — ASCII `<<>>` / `<<END>>` /
   `<<<corrected>>>` (any 2+ `<`, optional word chars, 2+ `>`)
   or Unicode `«»` / `≪≫` / `〈〉` / `⟨⟩` / `《》`.
3. The last fenced code block.
4. Text after a `Corrected:` heading.

If any pattern matches, only the discrete block lands; the
issue list / commentary / chrome the model wrote around it is
trimmed.

If none match AND the action is `g` (force_extract), the apply
refuses with a clear hint. If none match AND the action is
`r`, the full reply gets converted through `markdown_to_typst`
and lands as the rewrite (legacy behaviour).

## Disabling the diff modal

Some workflows (tightly looping a single paragraph, quick
typing-into-AI critique) feel slower with the modal. Turn it
off:

```hjson
ai: {
  diff_review_on_apply: false
}
```

`r` / `g` revert to immediate apply.

## The full HJSON block

```hjson
ai: {
  # 1.2.6+ knobs covered in this tutorial.
  per_paragraph_memory:           false   # default
  per_paragraph_memory_max_turns: 10
  diff_review_on_apply:           true    # default

  # Existing knobs (see 12-configuring-ai-providers).
  reseed_prompt_examples:         true
}
```

## Recap

- **F12** — critique chord; reads buffer in edit mode, reads
  diff vs snapshot in split-edit mode.
- **`ai.per_paragraph_memory: true`** — opt-in per-paragraph
  chat continuity (Paragraph-scope prompts only).
- **`ai.diff_review_on_apply: true`** (default) — `r`/`g`
  open a side-by-side review before any bytes change.
- **Smart extraction** — multi-tier pattern that lifts a
  corrected block out of the response, trimming commentary.
- **`a`** accepts and refocuses editor; **`r`** rejects;
  **`e`** is an alias for `a`.
