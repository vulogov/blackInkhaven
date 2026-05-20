# 14 — Document status and writing progress

Every paragraph in Inkhaven 1.1 carries an optional **status** tag
from a seven-step workflow ring: `None → Napkin → First → Second
→ Third → Final → Ready → None`. The tag is persistent metadata
stored alongside the paragraph in bdslib, surfaces in the editor
header and the tree pane, and drives a project-wide filter you can
use to walk through your batch of "what's still in first draft?"
paragraphs.

This tutorial covers the workflow ring, the visual cues, and the
filter-modal that ties it all together.

## The workflow ring

| Status   | Meaning (suggested)                            | Colour cue |
| -------- | ---------------------------------------------- | ---------- |
| `None`   | Untouched / no progress mark                   | (no badge — empty space) |
| `Napkin` | Rough draft — notes, scratched ideas           | Red (`grammar_change_fg`) |
| `First`  | First pass — coherent but not polished         | Peach (`ai_scope_fg`) |
| `Second` | Second pass — restructuring done               | Amber (`characters_fg`) |
| `Third`  | Third pass — sentence-level polish             | Cyan (`places_fg`) |
| `Final`  | Content-final — no more rewrites planned       | Green (`border_saved`) |
| `Ready`  | Shippable — proofed, ready to compile          | Green + REVERSED (strongest) |

The interpretations are deliberately suggestive, not prescriptive
— inkhaven doesn't enforce a workflow. Treat `Napkin`/`First`/etc.
as labels with consistent colours; map them to whatever stages
make sense for your book.

## Cycle status: Ctrl+B R

With a paragraph open in the editor, press `Ctrl+B` then `R`.
Each press advances one step:

```
status: `None` → `Napkin`
status: `Napkin` → `First`
status: `First` → `Second`
…
status: `Ready` → `None`     (wraps back to nothing)
```

Behind the scenes the change is written through bdslib's
`update_metadata`, so it survives the next launch.

`Ctrl+B R` previously opened the snapshot history picker. That
chord is reclaimed for status now; **F6 still opens the snapshot
picker** directly.

## Where you see the status

### Editor pane header

The header shows the current paragraph's status as a colour-coded
badge next to the title (hidden when None):

```
 Editor — Opening Scene [Napkin] · L4 C18 · 412w · ~2m · edited 3h ago
```

The badge takes the colour from the table above. The header
auto-refreshes every frame, so the cycle effect is visible
immediately.

### Tree pane

Every paragraph row has a reserved one-character gutter column
between the `¶` glyph and the title:

```
¶ R Opening Scene       (Ready — green reversed)
¶ F Closing Scene       (Final — green)
¶ 2 Bridge dialogue     (Second — amber)
¶ n Scratch idea        (Napkin — red)
¶   Untouched paragraph (None — dim space)
```

The single-character mapping:

| Status   | Letter |
| -------- | ------ |
| `Napkin` | `n` |
| `First`  | `1` |
| `Second` | `2` |
| `Third`  | `3` |
| `Final`  | `F` |
| `Ready`  | `R` |
| `None`   | (dim space — keeps the column aligned) |

Word count moved out of the tree row when this feature shipped:
the same number lives in the editor header for the open
paragraph, and the tree now gives you a project-wide progress
overview at a glance.

## Filter by status: Ctrl+B 1..7

The headline workflow. Press `Ctrl+B` then any digit `1`–`7` to
list every paragraph at that workflow stage:

| Chord       | Status |
| ----------- | ------ |
| `Ctrl+B 1`  | Ready  |
| `Ctrl+B 2`  | Final  |
| `Ctrl+B 3`  | Third  |
| `Ctrl+B 4`  | Second |
| `Ctrl+B 5`  | First  |
| `Ctrl+B 6`  | Napkin |
| `Ctrl+B 7`  | None   |

The most-advanced status maps to the lowest-effort chord — `Ctrl+B
1` answers "what's actually ready to ship?" in two keystrokes.

The modal lists matching paragraphs by **breadcrumb** (Book →
Chapter → Subchapter → Paragraph), so titles that repeat across
chapters stay disambiguatable:

```
 Paragraphs with status [Napkin] · scope: My Novel → Act Two · Ctrl+B 6
   First sketch       My Novel → Act Two → Opening   Opening scene
 › Closing line       My Novel → Act Two → Opening   The aftermath
   Bridge dialogue    My Novel → Act Two → Bridge    The conversation
   …
   ↑↓ select · Enter opens · r/R advances status · - / Backspace reverses · Esc cancel
```

### Scoped to the cursor's branch

The filter is **scoped to the tree cursor's enclosing branch**, not
the whole project:

| Cursor on … | Scope used |
| ----------- | ---------- |
| Book / Chapter / Subchapter | That node's subtree |
| Paragraph or Image | Nearest non-leaf ancestor |
| No branch ancestor (root) | Entire project |

The modal title spells out the active scope so you always know
what you're filtering ("scope: My Novel → Act Two" above). Move
the cursor onto a chapter before pressing `Ctrl+B 5` to narrow
the filter to that chapter; place the cursor on a book row to
sweep the whole book.

### Actions inside the modal

Navigation:

| Key | Action |
| --- | ------ |
| `↑` / `↓` | Move row |
| `Home` / `End` | Jump to first / last |
| `PageUp` / `PageDown` | Jump by 10 |

Per-row status changes:

| Key | Action |
| --- | ------ |
| `r` / `R` | Cycle the highlighted paragraph's status **forward** in the ring |
| `-` / `Backspace` | Step status **backward** in the ring |

After each cycle the modal re-collects entries against the same
filter target. If the paragraph no longer matches (e.g. you
advanced a `Napkin` to `First` while filtering by Napkin), the row
disappears and the next one slides up — you can **hold `r`** to
walk a whole batch through the workflow in one chord stream.

Open + cancel:

| Key | Action |
| --- | ------ |
| `Enter` | Jump the tree cursor to the row + open the paragraph |
| `Esc` | Close the modal |

## Typical workflows

### "Finish the chapter you're sitting in"

1. Move the tree cursor onto the chapter branch.
2. `Ctrl+B 3` → list every Third-stage paragraph in that chapter.
3. `Enter` on the first row → editor opens that paragraph.
4. Polish it. When done, `Ctrl+B R` to advance status to Final.
5. `Ctrl+B 3` again → next Third-stage paragraph in the chapter.
6. Repeat. The modal is empty when the chapter has no more
   Third-stage paragraphs.

### "Mass-promote a draft"

You've finished a writing burst and want to flip an entire
chapter's `Napkin` paragraphs to `First`:

1. Cursor on the chapter.
2. `Ctrl+B 6` → list all Napkin paragraphs in the chapter.
3. Hold **`r`** — each press advances the highlighted row and the
   list refreshes. The modal empties as paragraphs leave the
   Napkin stage.
4. When empty, `Esc` closes it.

### "What's actually ready to ship?"

1. Cursor on the book row (or anywhere — `Ctrl+B 1` always works).
2. `Ctrl+B 1` → list every Ready paragraph in the current scope.
3. Read the count in the status bar. When the number matches your
   target chapter / book size, run `Ctrl+B B` to build the PDF.

## What the status is *not*

It's a workflow tag, not a permissions system. The status doesn't
block edits — a `Ready` paragraph still accepts changes, you can
still re-cycle it to `Napkin` if you decide it needs rewriting.
The badge is a hint to you and your collaborators, not a lock.

## Next steps

- [`13-ai-full-screen-mode.md`](13-ai-full-screen-mode.md) — once
  you've built a chat-driven editing workflow, the AI full-screen
  layout pairs nicely with status filtering: walk through
  `Ctrl+B 6` Napkin paragraphs, ask the AI to rewrite, mark
  `First`, move on.
- [`../KEYBINDING.md`](../KEYBINDING.md) — every chord, organised
  by pane.
