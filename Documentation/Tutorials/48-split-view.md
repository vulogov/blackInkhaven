# 48 — Fullscreen split view

`Shift+F4` toggles a fullscreen layout that gives
two paragraphs full editor chrome side by side.
Tree pane stays on the left; primary editor takes
the middle; secondary editor takes the right.  AI
response pane is hidden (the two editors need the
room); AI prompt input still spans the bottom so
`Ctrl+I` works from either pane.

The chord ships in 1.2.12.

## Why this exists

Inkhaven shipped two "two editor panes" features
before split view:

- **`F4` split-edit** — same paragraph in both
  halves; lower pane is a frozen snapshot.  Used
  for "see what I changed since I opened this".
  Single document, two views.
- **`Ctrl+V S` similar-paragraph mode** — two
  *different* paragraphs side-by-side; the
  secondary replaces the AI pane.  Used to land
  via the similarity picker only.

Neither covered the workflows the 1.2.11
multilingual-prompts cycle made obvious:
translation, reference-while-writing, draft
comparison against a saved snapshot, cross-book
lookup.  Split view fills that gap.

`F4` and `Ctrl+F4` keep their existing meanings.
Split view is additive.

## The natural workflow

The minimal sequence:

1. Open the paragraph you want on the **left**.
2. Press `Shift+F4`.  Layout flips.  Tree on
   the left, primary in the middle, an empty
   placeholder on the right with chord hints.
3. Pin the paragraph you want on the **right**
   via any of:
   - Tree pane navigation, then **Shift+Enter** on
     a paragraph row.
   - `Ctrl+V P` (fuzzy paragraph picker), filter,
     then **Shift+Enter** on the match.
   - `Ctrl+V Shift+P` (recent paragraphs), then
     **Shift+Enter**.
   - `Ctrl+V M` (bookmarks), then **Shift+Enter**.
   - `Ctrl+V Shift+B` (sibling-book lookup — see
     below).
   - `F6` snapshot picker, **Shift+Enter** to pin
     the chosen snapshot as a read-only historical
     view.
4. The right pane fills.  Edit both panes
   independently.  `Tab` swaps focus.
5. `Shift+F4` again to exit.  The secondary slot
   clears so the standard layout's AI pane comes
   back.

## The universal `Shift+Enter` modifier

Every paragraph picker that targets the primary
slot now also accepts `Shift+Enter` as a "pin to
secondary instead" modifier.  Same picker, same
filter, same selection — the modifier changes the
destination, not the navigation.  No new chord per
picker to remember.

The Phase B dispatch surfaces pin failures on the
status bar ("split view: that paragraph is already
in the primary pane") so attempting to pin the
current paragraph doesn't silently do nothing.

## Sibling-book lookup — `Ctrl+V Shift+B`

The chord designed for translation manuscripts.
Given the open paragraph's slug, it walks the
project's hierarchy for paragraphs with the same
slug under a *different* top-level book.

Three outcomes:

- **Zero matches** — status message names the slug
  it tried.  Useful diagnostic — maybe you forgot
  to add the translation entry, or maybe the slug
  doesn't match.
- **Exactly one match** — auto-pin to the right
  pane.  Status echoes the pinned slug-path.
- **Two or more matches** — open the fuzzy picker
  pre-scoped to those entries.  Filter further if
  needed; `Shift+Enter` to pin.

Excludes the open paragraph's own book from
candidates so you can't accidentally pin yourself.

Worked example:

- Primary: `manuscript-en/chapter-3/03-rain`.
- Press `Ctrl+V Shift+B`.
- inkhaven walks the tree.  Finds
  `manuscript-ru/chapter-3/03-rain`.  Single match
  → pinned.
- Press `Shift+F4`.  Layout flips, both versions
  appear side by side.

For mirror-book setups (`research-notes/X` vs
`scenes/X`), or any parallel-slug convention, the
same chord works.

## F12 critique-compare

When split view is on AND both panes hold distinct,
non-empty paragraphs, F12 fires the
`critique-compare` flow instead of the usual
single-paragraph critique.

The prompt asks the model to compare the two
paragraphs substantively: where they overlap,
where they diverge, which one lands the beat
harder.  Specifically covers:

- **Translation review** — left = source,
  right = translation.  Does the translation
  carry meaning, voice, register?
- **Draft comparison** — left = older snapshot,
  right = current.  Which is stronger?  Which
  fragments should the next pass carry from the
  weaker into the stronger?

`critique-compare` is the seventh embedded prompt
(eighth in 1.2.12 with the multilingual cycle's
groundwork) and ships in all five supported
languages (en / ru / es / de / fr) per the same
resolver Pass 1 → Pass 3 cascade that drives the
other named flows.

To override per-project, add a `critique-compare`
paragraph to the Prompts system book (optionally
tag `lang:<code>`) or a `critique-compare` entry to
`prompts.hjson` (optionally with the `language:`
field).  See [tutorial 47 — multilingual
prompts](47-multilingual-prompts.md) for the
resolver in detail.

## F6 snapshot pinned as secondary

Inside the F6 snapshot picker, **`Shift+Enter`** on
a snapshot row loads that snapshot's body into the
right pane as a *read-only* historical view.
Title is suffixed with the snapshot age — `My
paragraph (snapshot · 2h ago)` — so the historical-
vs-live distinction stays visible in the split-view
title bar.

This is the draft-vs-current workflow:
- Snapshot in the right pane.
- Live buffer in the left.
- Toggle `Shift+F4` to enter the split.
- F12 to fire `critique-compare` across the two
  versions.
- Edit the live buffer based on what the critique
  surfaces.  The right pane is read-only —
  refresh by picking a different snapshot in F6.

## Tab focus

In split-view layout, `Tab` swaps focus between
the left and right editor panes only — the tree
and (hidden) AI panes aren't in the cycle since
the AI response pane isn't drawn.  `Shift+Tab` is
the reverse.

Status echoes the new focus: `split: right editor
focused` / `split: left editor focused`.

## Exit semantics

`Shift+F4` to exit drops the secondary slot.  The
standard layout's AI pane reappears in its usual
right-column position.

If similar-mode (`Ctrl+V S`) is currently active
when you press `Shift+F4` to exit, the secondary
stays pinned — `Shift+F4` only toggled the
*display* layout, not the slot ownership.  Status
echoes `split view: OFF (similar-mode kept;
secondary stays pinned)`.

Otherwise: `split view: OFF (secondary cleared)`.

If the secondary was dirty when you exited, it
gets saved first (same protection similar-mode's
exit uses).  No edits lost.

## When NOT to use split view

- **Small terminals.**  At 80 cols the math is
  tree=30 + editors=25 each.  Tight but readable
  for short lines; cramped for wider prose.  At
  120 cols the math is 30 + 45 + 45 — comfortable.
  On a narrow screen, single-doc workflows might
  be easier.
- **Heavy AI usage.**  The AI response pane is
  hidden in split view; you'll have to close the
  split to read longer AI responses.  For long
  back-and-forth sessions, stick with the
  standard layout.
- **F4 split-edit covers it.**  If you just want
  to see what you changed since you opened the
  paragraph, plain `F4` is still the right tool —
  same paragraph, snapshot-on-bottom.  Split view
  is for two *different* paragraphs.

## See also

- [Tutorial 16 — similar paragraphs](16-similar-paragraphs.md)
  — the legacy two-pane mode this generalises.
- [Tutorial 20 — snapshot diff](20-snapshot-diff.md)
  — F6 picker, snapshot history, side-by-side
  diff.
- [Tutorial 28 — AI critique and memory](28-ai-critique-and-memory.md)
  — the F12 critique-edit flow that
  `critique-compare` extends.
- [Tutorial 47 — multilingual prompts](47-multilingual-prompts.md)
  — the resolver that picks the right embedded
  `critique-compare` variant for your project's
  language.
- `Documentation/PROPOSALS/SPLIT_VIEW.md` — the
  four-phase design doc with the resolver's
  state-machine and chord table.
