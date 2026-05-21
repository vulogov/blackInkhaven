# 18 — Bund output pane, script picker, input modal

Three Bund-side additions in 1.2.3 make scripted workflows
feel native to the editor:

- **The floating output pane** (`ink.pane.*`) — multi-line
  script output stops clobbering the status bar.
- **The script picker** (`Ctrl+Z` `?`) — discover and run
  scripts from the current branch or the `Scripts` system book.
- **The input modal** (`ink.input`) — prompt the user for a
  string mid-script; the answer flows back through a hook.

Read [`../Bund/BUND_TUTORIAL.md`](../Bund/BUND_TUTORIAL.md) first
if you haven't met the Bund VM yet. This tutorial assumes you
already know what a lambda + hook looks like.

## The Bund output pane

Before 1.2.3, `print` and `println` from a Bund script accumulated
into a status-bar buffer that was drained once on eval-return. A
single line: fine. Twenty lines of structured output: useless.

The new `ink.pane.show` word opens a **floating pane** that
captures every subsequent `print` / `println` until you close it:

```bund
"My report" ink.pane.show

"Header line"           println
""                      println
"Row 1: " 42 conv->STRING concat println
"Row 2: " 99 conv->STRING concat println
```

While the pane is open, `print` / `println` route there instead
of the status bar. Close with `Esc` (or `ink.pane.close`).

The four pane words:

| Word              | Stack                | Notes |
| ----------------- | -------------------- | ----- |
| `ink.pane.show`   | `( title -- )`       | Open or reset. Title shows in the modal header. |
| `ink.pane.close`  | `( -- )`             | Closes. No-op when not open. |
| `ink.pane.clear`  | `( -- cleared )`     | Empties the buffer without closing. Returns false if no pane is open. |
| `ink.pane.line`   | `( text -- routed )` | Append one line. Returns false if no pane is open (so scripts can branch on pane visibility). |

All four live under the `editor_read` policy category — opening
a pane is non-destructive UI state, recoverable with `Esc`,
never touches the project.

### When to use the pane vs the status bar

- **Single-line result** → leave it on the status bar. Don't
  open the pane for `42 println`.
- **Multi-line table or report** → `ink.pane.show` first.
- **Long-running script with progress updates** → open the pane,
  emit `ink.pane.line` periodically.

The pane scrolls itself when content grows past the visible
area; `Ctrl+C` inside the pane clears its buffer.

## The script picker: Ctrl+Z ?

Scripts live as `NodeKind::Script` leaves in the hierarchy.
Before 1.2.3 you ran them by opening the `.bund` node in the
editor and pressing `Ctrl+Z` `R`. That's still there. The new
discovery chord is `Ctrl+Z` `?`:

```
┌── Bund · pick a script (current branch) ──────────────────┐
│  λ rename-paragraph        story/01-arrival/scripts        │
│  λ split-on-sentences      story/01-arrival/scripts        │
│  λ trim-trailing-space     story/01-arrival/scripts        │
│                                                            │
│ ↑↓ select · Enter run · A toggle scope · Esc close  (1/3)  │
└────────────────────────────────────────────────────────────┘
```

Two scopes:

- **`Branch`** (default) — scripts under the cursor's nearest
  enclosing branch (Book / Chapter / Subchapter). What you're
  most likely to want from the editor.
- **`ScriptsBook`** — scripts under the project's `Scripts`
  system book.

`A` (or `a`) toggles between them. If the branch scope is empty,
inkhaven automatically falls back to the Scripts book on first
open — saves one keystroke.

`Enter` loads the chosen script's body via `Store::get_content`
and evaluates it through the Adam VM. Output lands in the
floating pane when one is open, otherwise the status bar.

### Custom Bund chord bindings

`Ctrl+Z` `?` is a default binding. Like every chord in
`bund_sub`, you can rebind it in `inkhaven.hjson`:

```hjson
keys: {
  bindings: [
    // Move the script picker to Ctrl+Z P
    { chord: "Ctrl+z p", action: "bund.open_script_picker" }
    { chord: "Ctrl+z ?", action: "none" }   // disable the default
  ]
}
```

See [`../KEYS_REASSIGNMENT.md`](../KEYS_REASSIGNMENT.md) for the
full overlay grammar.

## The input modal: ink.input

You want a script that asks for a filename / chapter title /
status. Before 1.2.3 you'd hard-code the value and re-eval.
1.2.3 adds `ink.input`:

```bund
:hook.greet_user ( name -- )
  "Hello, " swap concat println
;

"What's your name?" "greet_user" ink.input
```

The flow:

1. `ink.input` takes two stack values: the **prompt** string and
   the name of a **hook lambda** to fire on Enter.
2. A modal opens showing the prompt + a text input.
3. The user types and presses Enter (or Esc to cancel).
4. The typed string is pushed onto Adam's workbench and the
   lambda named `greet_user` runs.

Why hook-driven instead of synchronous? A blocking modal inside
`scripting_eval` would freeze every other timer (autosave, AI
inference polling, Typst build) until the user types. Hooks
keep the editor responsive — the prompt is just another modal,
the rest of the world keeps ticking.

### Wiring an input modal from a script node

Stash the hook lambda in your `Scripts` system book or in a
chapter-local Scripts sub-tree:

```bund
( File: rename-paragraph.bund — a Scripts/ leaf )

:hook.rename_paragraph_with_input ( new-title -- )
  ink.editor.cursor          ( row col )
  drop drop                  ( ignore, just need editor active )
  ink.tree.rename
;

"New title for this paragraph:"
"rename_paragraph_with_input"
ink.input
```

Press `Ctrl+Z` `?`, pick the script, hit Enter, type the new
title in the modal that pops up. The hook fires with your
input on the workbench.

### Cancellation

Press `Esc` in the modal: the hook does NOT fire. No way to
distinguish "Esc" from "Enter on empty string" inside the hook —
if you need that, prompt with an empty-string guard:

```bund
:hook.do_rename ( name -- )
  dup len 0 = if
    drop
    "rename cancelled — empty title" println
  else
    ( ... actually rename ... )
  endif
;
```

### Policy

`ink.input` lives under `editor_read` — opening the modal is
non-destructive UI. The hook the user names is unrelated:
**that** lambda's words go through normal policy gating when
they run. If the hook calls `ink.tree.rename` (in `store_write`),
the user needs `store_write` enabled.

## Putting it together

A small example that combines all three new features:

```bund
:hook.run_word_count ( title -- )
  "Word counts" ink.pane.show
  "" ink.pane.line drop
  
  ( Iterate project paragraphs and emit one line per paragraph.
    `ink.node.list` returns a list of node IDs. )
  ink.node.list
  ( ... loop body that calls ink.paragraph.text + counts words ... )
  
  drop
;

"Hit Enter to compute word counts" "run_word_count" ink.input
```

Press `Ctrl+Z` `?`, pick the script, see the input modal, hit
Enter, watch the report stream into the floating output pane.

## New `ink.*` words in 1.2.4

Six words landed in 1.2.4 that close the most common
scripting gaps. They pair well with the pane + input + script
picker:

| Word | What | Policy |
|------|------|--------|
| `ink.editor.replace_all ( old new -- count )` | Find/replace on the open editor buffer. | `editor_write` |
| `ink.search.load ( query -- )` | Run semantic search, load top hit into editor (autosaves prev). | `editor_write` |
| `ink.ai.send_blocking ( prompt -- response )` | Sync AI send — blocks the script, UI keeps repainting. | `ai_write` |
| `ink.ai.poll ( -- string )` | Non-blocking poll of the async AI slot. Empty when none. | `ai_read` |
| `ink.fs.read ( path -- string )` | Read a file. | `fs_read` (default-allowed) |
| `ink.fs.write ( path content -- )` | Write a file. | `fs_write` (**default-denied** — opt in via `enabled_categories: ["fs_write"]`) |

Worked combo — "export the open paragraph's body and pipe it
through an AI for a one-shot summary, then drop the summary
into a sibling file":

```bund
:hook.summarize ( -- )
  ink.paragraph.text                  ( -- body )
  "Summarize the following paragraph in 50 words:\n" swap concat
  ink.ai.send_blocking                ( -- summary )
  "summary.md" swap ink.fs.write      ( -- )
;

"Summarize open paragraph?" "summarize" ink.input
```

This script needs `enabled_categories: ["ai_write", "fs_write"]`
because writing files is in the deny-by-default bucket. The
read direction (`ink.fs.read`) is allowed out of the box —
the asymmetry is deliberate.

## See also

- [`../Bund/BUND_TUTORIAL.md`](../Bund/BUND_TUTORIAL.md) — the
  full Bund language tutorial, including stack semantics,
  hooks, the `ink.*` stdlib catalogue, and the sandbox.
- [`../KEYS_REASSIGNMENT.md`](../KEYS_REASSIGNMENT.md) — rebinding
  any chord including `Ctrl+Z` `?`.
