# 41 — POV / character chip

The status bar gains a small magenta chip showing
the cast present in the currently-open paragraph,
with the most-mentioned name highlighted as the
heuristic POV character.

```
[Editor]  POV: Anna  +Bob, Carol   • saved 2s ago …
```

`Ctrl+B Shift+P` toggles the chip on / off at
runtime.  Off by default in projects that don't have
a Characters book; on by default otherwise (via
`editor.pov_chip_enabled` in HJSON).

## The heuristic

For each open paragraph, the chip:

1. Walks every editor row, runs the project's
   lexicon scan on each.
2. Filters to `LexCategory::Character` hits.
3. Tallies occurrences per character.
4. Picks the most-mentioned name as the POV slot;
   ties broken by first-mention order.
5. Up to three additional named characters trail
   behind as the supporting cast (DIM-styled).

The rationale: in third-person limited prose the
narrator's gaze inherently centres on the POV
character, so they're the most-named entity in any
scene.  Ties go to whoever opens the scene — the
classical "scene anchor".

## Wiring

The chip is driven by the project's existing
`characters` lexicon — the paragraphs nested under
the system-tagged Characters book.  No separate
tagging mechanism, no per-paragraph frontmatter, no
POV annotation to keep current.  Add a character to
the project the way you always have; the chip starts
surfacing them.

Multilingual: the lexicon already runs every name
through every configured Snowball stemmer, so the
Russian case system handles itself —`Анна`/`Анной`/
`Анне`/`Анну` collapse to one count.  Same for
French / German / Spanish inflections.

## Toggle

`Ctrl+B Shift+P` cycles a session-local override on
top of the persisted `editor.pov_chip_enabled`
setting:

```
None          → defer to HJSON default (initial state)
Some(true)    → force chip ON regardless of HJSON
Some(false)   → force chip OFF regardless of HJSON
```

Same three-state pattern as the `Ctrl+B Shift+F`
style-warnings toggle.

## Config

```hjson
editor: {
  pov_chip_enabled: true
}
```

## Edge cases

- **No paragraph open** → no chip.
- **Lexicon empty / no Characters book** → no chip.
- **No character names mentioned in the paragraph**
  → no chip (status bar reverts to its non-1.2.9
  layout).
- **First-person POV** (the narrator is `I`, not in
  the lexicon) → chip surfaces the *other*
  prominent character — which is the contextually
  useful piece of information anyway ("scene with
  Bob from Anna's first-person POV").

## Use cases

- **Quick POV-consistency audit**.  Page through a
  chapter and watch the chip — if the POV slot
  flips between paragraphs of the same scene, your
  POV drifted.
- **Headhopping detection**.  Two POV names in the
  supporting cast can mean you slipped into
  someone else's interiority mid-scene.
- **"Whose chapter is this?"** at-a-glance answer
  when you reopen a project after a week away.

## Performance

The chip runs the lexicon scan on every status-bar
repaint, which is also every input event — that's
~~~ no measurable overhead.  Lexicon already
runs per-row for highlight purposes; the chip does
the same work paragraph-wide, then ranks.

## See also

- [`03-the-editor.md`](03-the-editor.md) — style-
  warning overlays + concordance + sentence rhythm
  + show-don't-tell (the rest of the 1.2.9 prose-
  audit set).
- [`07-places-and-characters.md`](07-places-and-characters.md)
  — how the Characters book works + the lexicon
  highlight system the chip rides on.
