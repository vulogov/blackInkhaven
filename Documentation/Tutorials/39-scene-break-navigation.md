# 39 — Scene-break navigation

Two chords for jumping the cursor between scene-break
lines in the open paragraph:

```
Ctrl+B <    jump to previous scene break
Ctrl+B >    jump to next scene break
```

Both are editor-scoped — they only fire while the
editor pane has focus.

## What counts as a scene break

The detector recognises the typographic dividers
writers actually use:

```
* * *        ***
- - -        ---
_ _ _        ___
~ ~ ~        ~~~
# # #        ###
§            (lone section sign)
```

Rule: any line consisting of 3 or more copies of one
of `*` `-` `_` `~` `#` (optionally separated by
single spaces), or a single `§`, counts as a scene
break.  Anything else doesn't.

Notably the detector **does NOT** match:

- `**` (below threshold — easily mistaken for stray
  bold markers).
- `***bold***` (mixed content — that's prose, not a
  divider).
- `= Heading` or `== Subheading` (those are typst
  structural section markers, not scene breaks —
  navigating between them is what `Ctrl+V N` and the
  diagnostics list do).

## Cursor behaviour

Both chords land the cursor at column 0 of the
matching line and let the textarea's existing
scroll-tracking pan the viewport.  When there's no
scene break in the requested direction the status
line says *"scene break: no break below"* (or
*above*) and the cursor stays put.

## Chord rationale

The chords are `Ctrl+B <` and `Ctrl+B >` — vim's
forward/backward conventions.

Originally requested as `Ctrl+B Shift+{` and
`Ctrl+B Shift+}`, but `Shift+}` was already bound to
TagSearch (since 1.2.5).  The vim-style `<` / `>`
were free and the muscle memory transfers if you
already use vim's `[[` / `]]` patterns.

## Use cases

- **Long paragraphs with multiple scenes inside one
  node**.  Some authors keep an entire chapter in
  one Paragraph node, separated internally by `* *
  *`.  `Ctrl+B >` jumps from scene to scene without
  scrolling.
- **Audit pass before assembly**.  Walk through every
  scene break in a chapter checking that the
  transitions read cleanly — `Ctrl+B >` repeatedly
  is faster than PgDn.
- **Counting scenes**.  Press `Ctrl+B >` until you
  hit the "no break below" status — the number of
  presses + 1 is your scene count for that
  paragraph.

## See also

- [`03-the-editor.md`](03-the-editor.md) — full
  catalogue of editor chords + features.
- [`13-ai-full-screen-mode.md`](13-ai-full-screen-mode.md)
  — focus mode for a distraction-free pass through
  the scenes you just navigated.
