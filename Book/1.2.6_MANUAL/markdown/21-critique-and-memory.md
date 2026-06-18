# 21 — Critique, memory, and the diff modal

Three 1.2.6 additions work together: an F12 critique chord, opt-in per-paragraph chat memory, and a diff-review modal that gates buffer-replacing applies. In combination they make AI feel like a writing partner you trust rather than a fire-and-forget tool.

## F12 — critique (mode-aware)

`F12` (Editor scope) sends the open paragraph to the AI with a configurable critique prompt. The prompt depends on editor mode:

| Mode | Prompt | What the AI sees |
|------|--------|------------------|
| Plain edit | `critique-edit` | The paragraph body. |
| Split-edit (F4) | `critique-changes` | The snapshot body (Before) + the live buffer (After). |

The default `critique-edit` prompt is roughly: "Point out the weakest two or three elements. Be specific — quote the phrase and propose a tighter alternative. Don't rewrite the paragraph; critique it. Be honest, not destructive."

`critique-changes` is roughly: "Identify what the revision changed. Evaluate each change — improvement, regression, or neutral. Suggest one focus for the next pass."

## Per-paragraph AI memory

Opt in:

```hjson
ai: {
  per_paragraph_memory:           true
  per_paragraph_memory_max_turns: 10
}
```

When on AND `F9` scope is Paragraph AND a paragraph is open, each AI turn records onto the paragraph's `Node.ai_memory` field. Subsequent Paragraph-scoped prompts prepend that memory to the chat history sent to the model.

Effect: a paragraph carries its own conversation with the AI. Come back days later, the model sees the prior context automatically.

![figure: per-paragraph-memory-flow](images/per-paragraph-memory-flow.png) — Per-paragraph memory: three Paragraph-scope prompts on the same paragraph. The model sees turn 1 + turn 2 as prologue to turn 3.

The visible chat history stays project-wide and shows just the current session's turns. Memory is the invisible prologue.

Trimming is pair-aligned: when length exceeds `max_turns`, the oldest user+assistant pair evicts as a unit.

Recording only fires when the user's message wasn't empty. One-shots (F7 grammar, F12 critique, F1 Help, Ctrl+F12 explain) skip recording — they have no carrying-over story.

## The diff modal — accept / reject

`ai.diff_review_on_apply: true` (default) routes every buffer-replacing apply through a side-by-side diff modal:

![figure: ai-diff-modal](images/ai-diff-modal.png) — AI diff modal: left is current buffer, right is the proposed replacement. Removed lines marked -, added lines marked +.

| Chord | What it does |
|-------|--------------|
| a / A / Enter | Accept — apply the rewrite, refocus the editor. |
| r / R | Reject — close, buffer unchanged. |
| e / E | Accept + edit (alias for `a` since 1.2.6). |
| ↑ / ↓ / PgUp / PgDn | Scroll the diff. |
| Home / End | Jump top / bottom. |
| Esc | Same as reject. |

Modal status reports `✂ extracted X/Y chars` when the extraction layer trimmed commentary from the response. See Chapter 20 for the extraction logic.

## Smart extraction — what lands in the buffer

For `r` and `g`, inkhaven scans the AI reply for a discrete corrected block before applying:

1. `<<<CORRECTED>>>` / `<<<END>>>` markers (canonical).
2. Relaxed bracket pairs (ASCII `<<>>`, Unicode `«»` / `≪≫`).
3. The last fenced code block.
4. Text after a `Corrected:` heading.

If any matches, only that block lands. If none match AND the action is `g`, the apply refuses. If none match AND the action is `r`, the full reply gets converted through markdown→typst and lands.

## Disabling the diff modal

Some workflows feel slower with the modal. Turn it off:

```hjson
ai: {
  diff_review_on_apply: false
}
```

`r` / `g` revert to immediate apply (the pre-1.2.6 behaviour).

## Configuration recap

```hjson
ai: {
  per_paragraph_memory:           false   # default
  per_paragraph_memory_max_turns: 10
  diff_review_on_apply:           true    # default
  reseed_prompt_examples:         true
}
```

The first two opt into stateful AI; the third gates the diff modal; the fourth controls whether the Prompts book gets re-seeded with `.example` files on every project open (idempotent — skips existing entries).

## Recap

- `F12` critique — `critique-edit` in plain edit; `critique-changes` in split-edit.
- `ai.per_paragraph_memory: true` — paragraph-scoped chat continuity (prepend to history).
- `ai.diff_review_on_apply: true` (default) — side-by-side before `r`/`g` lands.
- Smart extraction picks the corrected block from the reply, trimming commentary.
- `a` accepts and refocuses editor; `r` rejects; `e` is an alias for `a`.
