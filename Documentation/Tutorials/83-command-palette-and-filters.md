# Tutorial 83 — Command palette & Output filters

*Inkhaven 1.3.33*

Two features land in 1.3.33 that make a project with a few hundred
commands and a busy Output board feel small again: a fuzzy command
palette over *every* bound command, and a set of filter keys on the
Output pane so you only see the findings you care about.

## Ctrl+V Space — the command palette

Press **`Ctrl+V Space`** (literally `Ctrl+V`, then `Space`) from
anywhere. A modal opens listing every command Inkhaven knows about:

```
┌── Run command (12/214) ───────────────────────────────────────┐
│ › book                                                        │
│  Toggle bookmark        bookmark the open paragraph   Ctrl+V B │
│  Bookmark picker        jump to a bookmarked paragraph Ctrl+V M│
│  Book info              show this book's metadata     Ctrl+B i │
│  ...                                                           │
│ ↑↓ select · Enter runs · Esc closes                           │
└────────────────────────────────────────────────────────────────┘
```

The palette doesn't hard-code a menu. It walks the keybinding
registry and collects an entry for every bound command — so each row
shows the command's **label**, its **description**, and the **chord**
that triggers it. If a command has a key, you see the key; the palette
and the keymap can never drift apart, because they read the same
source.

Type to fuzzy-filter the list; `↑` / `↓` move the selection; **Enter**
runs the highlighted command exactly as if you'd pressed its chord;
**Esc** closes without running anything.

It's bound to `Ctrl+V Space` on purpose — `Ctrl+P` is already the
editor's paste, and `Ctrl+Shift+P` is fragile inside a terminal — so
`Ctrl+V Space` is the one chord guaranteed not to collide.

Use it when you know *what* you want to do but not the chord, or when
the command has no chord at all and the palette is the only way to
reach it.

## Output-pane filters

The **Output pane** is the right-hand, deterministic-findings board —
the notice board where checkers post structured findings (see
[Tutorial 75](75-the-output-pane.md)). On a real project it fills up
fast: every checker, every severity, every paragraph. The filter keys
let you carve it down.

With the Output pane **focused**, four keys drive the filter:

- **`f`** — cycle the **source** filter: which checker's findings to
  show. Each press advances to the next source (and eventually back to
  "all sources").
- **`S`** — cycle the minimum **severity**: raise the floor so only
  findings at or above the chosen level remain.
- **`t`** — toggle **"only the open paragraph"**: show just the
  findings attached to the paragraph you currently have open, or all
  of them.
- **`c`** — **clear** the filter back to showing everything (all
  sources, all severities, every paragraph).

A one-line summary at the top of the pane reports the active filter,
so you always know what you're *not* seeing:

```
┌─ Output · 7/41 ──────────────────────────────────────────┐
│ filter: source=prose · severity≥warn · this paragraph    │
│▌⚠ repetition_check                                        │
│    "suddenly" appears 4× in this paragraph                │
│ ⚠ passive_voice                                           │
│    3 passive constructions                                │
│ ...                                                       │
└───────────────────────────────────────────────────────────┘
```

Here the count `7/41` reads as "7 findings shown of 41 on the board" —
the rest are filtered out, not gone.

### The filter is remembered

The filter state is **persisted** in the project's `.session.json`, so
it survives across sessions. Set `severity≥warn` for the open
paragraph on Friday, and Monday's session opens with the same view —
no re-filtering each time you come back to a project.

## A typical pass

```
Ctrl+B Tab    → flip the right region to the Output pane, focus it
S             → raise severity to ≥ warn (drop the info noise)
f             → narrow source to the one checker you're chasing
t             → "this paragraph only" while you work a single para
... fix, move to next paragraph ...
c             → clear, see the whole board again
```

## Quick reference overlay

Forgot a chord mid-flow? **`Ctrl+B H`** (or `?` from the Tree pane)
opens a quick-reference overlay listing the command palette, the
review pass, the cost dashboard, and the Output-pane filter keys —
the handful of surfaces you reach for most.

## See also

- [`75-the-output-pane.md`](75-the-output-pane.md) — the Output pane
  itself: every kind, action, and the `ink.io.*` surface the filters
  sit on top of.
- [`82-project-health-and-review.md`](82-project-health-and-review.md)
  — the review pass and the checkers whose findings land in the
  Output pane and feed the `f` / `S` filters.
