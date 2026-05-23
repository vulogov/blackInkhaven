# 27 — Typst diagnostics

Inkhaven 1.2.5 added structured typst diagnostics — parse +
semantic errors surfaced at the source line where they fire.
1.2.6 built the navigation + AI surface on top: gutter markers,
a list modal, and an AI-explain chord.

This tutorial walks through every way the diagnostics surface
shows up. The underlying engine setup lives in
[`24-typst-in-process.md`](24-typst-in-process.md) — read that
first if you haven't enabled `engine = "inprocess"` yet.

## Where diagnostics come from

Two checks run independently:

1. **Parse check** — `typst-syntax` only. Always on regardless
   of engine. Catches syntax errors (unbalanced braces, malformed
   `#set` calls, etc.). Fires on save + after an idle window
   (`typst_compile.diagnostics_idle_seconds`, default 2.0).
2. **Semantic check** — full compile via the in-process
   engine. Only when `typst_compile.engine = "inprocess"`. Catches
   references to unknown variables, type errors, missing
   imports, etc. Same idle window.

Both pour into `OpenedDoc.typst_diagnostics: Vec<TypstDiagnostic>`.
Every surface in this tutorial reads from that one list.

## Gutter markers (1.2.6+)

Lines that carry a diagnostic get a red `●` in the editor's
line-number gutter:

```
   1   = Chapter 1
   2 ●
   3 ● The storm rolled in over the mountains, #invalid(arg).
   4
   5   Aerin pulled her cloak tighter.
```

The marker sits in the column between the line number and
the prose. Compatible with the current-line highlight — the
marker keeps its red colour on the cursor's line.

Costs zero visual weight on clean buffers (the slot is a
space when no diagnostic is present). On heavily-broken
files, scanning the gutter tells you exactly which lines need
attention before you even open the list.

## F8 — the diagnostics list

`F8` (Editor scope) pops a floating modal listing every
diagnostic in the open paragraph:

```
┌── Typst diagnostics (3) ────────────────────────────────────┐
│                                                              │
│  line    3:42  unknown function: `invalid`                  │
│  line    7:8   missing argument: `body`                     │
│  line   12:1   type mismatch: expected str, found int       │
│                                                              │
│   ↑↓ select · Enter jumps cursor · Esc closes               │
└──────────────────────────────────────────────────────────────┘
```

`Enter` jumps the editor cursor to the selected
diagnostic's line:column and closes the modal. Useful when
you've got a wall of errors after a paste and want to
triage them.

Bound as `editor.diagnostics_list` — rebindable via the
standard HJSON `keys.bindings`.

## Ctrl+V N / Shift+N — next / previous diagnostic

Without opening the modal, you can navigate diagnostics in
place:

| Chord            | Action |
|------------------|--------|
| `Ctrl+V N`       | Jump cursor to the next diagnostic. Wraps at the end. |
| `Ctrl+V Shift+N` | Jump cursor to the previous diagnostic. Wraps at the start. |

Both refresh the diagnostics cache up-front so the navigation
reflects the live buffer, not the last save.

## Ctrl+F12 — AI explain the diagnostic at cursor

`Ctrl+F12` finds the diagnostic closest to the cursor row,
bundles it with ±5 context lines, and sends to the AI pane
with the configured explain prompt:

```
explain-diagnostic prompt template
[…rendered context…]

── Diagnostic ──
line 3:42 — unknown function: `invalid`
── end ──

── Context (paragraph: The Storm) ──
   1     = Chapter 1
   2  
>> 3     The storm rolled in over the mountains, #invalid(arg).
   4  
   5     Aerin pulled her cloak tighter.
── end context ──
```

The `>>` marks the offending line so the model has a clear
anchor. F11 was the original binding but macOS grabs it for
Mission Control, so 1.2.6 moved the chord to **Ctrl+F12**.

Resolves the prompt through the standard chain: Prompts book
paragraph named `explain-diagnostic` → `prompts.hjson` →
embedded fallback. The seed file
`02-explain-diagnostic-example.typ` lands in your Prompts book
on `inkhaven init` ready to customise.

## hook.on_diagnostic — react from Bund

`hook.on_diagnostic ( uuid count first-message -- )` fires
when the diagnostic state of the open paragraph **changes**
(clean → errored, count change, or top-message change). The
hook is debounced — no fire on every idle tick when nothing
moved.

```bund
"hook.on_diagnostic" {
  // Stack: uuid count first-message
  swap drop swap drop                  // ( count )
  dup 5 >                              // ( count bool )
  {
    "⚠ many diagnostics: " print println
  } { drop } ifelse
} register
```

Useful for soft notifications (typewriter-sound on first error,
status counter in a custom pane, batched re-render of a side
panel, etc.).

## ink.editor.set_cursor — drive from scripts

`ink.editor.set_cursor ( row col -- )` (1-based) moves the
editor cursor. Pairs naturally with `hook.on_diagnostic` —
auto-jump to the first error when one shows up:

```bund
"hook.on_diagnostic" {
  // Stack: uuid count first-message
  drop drop drop
  // … parse the message for line:col … then:
  3 42 ink.editor.set_cursor
} register
```

Policy: `editor_write` (default-allowed; you don't need to
explicitly enable it).

## Configuration

The HJSON knobs that shape the diagnostic surface:

```hjson
typst_compile: {
  // Required for semantic-check diagnostics. Parse-only
  // diagnostics work either way.
  engine: "inprocess"

  // Seconds of editor idleness before the next idle recheck.
  // Lower = snappier feedback; higher = lighter CPU.
  diagnostics_idle_seconds: 2.0

  // Cap on emitted diagnostics. Avoids walls of repeats from a
  // single cascading error. 0 = unlimited.
  diagnostics_max: 50
}
```

When semantic checks are off, only parse-level diagnostics
fire — and they're enough to catch ~90% of typst syntax
mistakes. The semantic layer pays for itself when you start
using `#import`, custom functions, and `@preview/<pkg>`
references where the parser can't tell whether a name
resolves.

## Recap

- **Gutter `●`** — every line with a diagnostic, always visible.
- **F8** — floating list, Enter jumps cursor.
- **Ctrl+V N / Shift+N** — next / previous diagnostic in place.
- **Ctrl+F12** — AI explain the diagnostic at cursor, with
  ±5 lines of context.
- **`hook.on_diagnostic`** — Bund-side reaction on state
  change.
- **`ink.editor.set_cursor`** — 1-based cursor mover; pairs
  with the hook for auto-jump scripts.
- Engine setup lives in
  [`24-typst-in-process.md`](24-typst-in-process.md).
