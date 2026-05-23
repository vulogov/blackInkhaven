#import "../design.typ": *

#chapter(number: 22, part: "Part VI — Working with AI",
  title: "AI for diagnostics and the timeline")

#dropcap("T")wo specialised AI surfaces sit at the
intersection of writing and the technical layers: explain
a Typst diagnostic in plain English, and audit a story
timeline for consistency.

#section("Ctrl+F12 — explain the diagnostic at cursor")

When the gutter shows a red `●` and you can't tell what
the error means, `Ctrl+F12` (Editor scope) bundles:

- the diagnostic message,
- the offending line + ±5 lines of context,
- the `explain-diagnostic` prompt template,

and streams the explanation into the AI pane.

The chord moved from `F11` in 1.2.6 because macOS grabs F11
globally (Mission Control / Show Desktop). Linux + Windows
users can rebind F11 back via the standard HJSON
`keys.bindings` overlay.

#figure_slot(
  id: "ctrl-f12-explain",
  caption: "Ctrl+F12 — diagnostic message + ±5 lines of context sent to AI, which explains the cause + suggests a fix.",
  height: 55mm,
)

The default prompt is roughly: "A Typst compiler diagnostic
is shown below with the surrounding source. Explain in plain
English what the diagnostic means, why it likely fired in
this context, and the most plausible one-line fix."

#section("Customising the explain prompt")

Open `Prompts/explain-diagnostic.example`. Rename to drop
`.example` once you've edited. Useful additions:

- "When the error references a custom function, check the
  book's globals.typ first."
- "Be terse. Two sentences maximum."
- "Always quote the offending phrase."

#section("Timeline health critique (y / Y / Ctrl+Y)")

Inside the swim-lane view (`Ctrl+V t`):

#chord_table((
  chord_row("y", "Critique events in the current view scope, current track only."),
  chord_row("Y", "Critique events in the current view scope, all tracks."),
  chord_row("Ctrl+Y", "Critique events in the whole book, all tracks (widens regardless of view scope)."),
))

The payload — covered fully in Chapter 17 — is a flat
prose summary the model can read without parsing
calendar config. Events appear as bullet lines with
calendar-formatted start, optional end, title, track,
precision, orphan flag, and resolved character / place /
paragraph link names.

#section("What the audit asks for")

The default `timeline-health` prompt instructs the model
to surface:

- #strong[Travel-time conflicts] — a character at two events
  with insufficient time between them.
- #strong[Paragraph mismatches] — a manuscript paragraph
  saying "the day after the Storm" while the Storm is
  three months earlier.
- #strong[Fuzzy-precision overlaps] — two season-precision
  events whose 90-day windows collide suspiciously.
- #strong[Orphan signals] — events tagged orphan that look
  like they want a scene attached.
- #strong[Pacing] — long unexplained gaps or rushed sequences.

#section("Hook on diagnostic (Bund)")

The diagnostic surface also fires a Bund hook when the
state changes:

```bund
"hook.on_diagnostic" {
  // ( uuid count first-message -- )
  swap drop swap drop      // ( count )
  dup 5 >                   // ( count bool )
  { "⚠ many diagnostics: " print println }
  { drop } ifelse
} register
```

Debounced — fires only on state transitions (clean → errored,
count change, top-message change). Doesn't fire on every
idle tick when nothing moved.

#section("`ink.editor.set_cursor` for auto-jump")

Pairs with the diagnostic hook for automated cursor
positioning:

```bund
"hook.on_diagnostic" {
  // … parse the message → get line + col …
  drop drop drop
  3 42 ink.editor.set_cursor     // ( row col -- )
} register
```

1-based row/col (matches the diagnostic format). Policy:
`editor_write` (default-allowed).

#recap((
  [`Ctrl+F12` — AI explain for the diagnostic at cursor.],
  [Custom prompt: `Prompts/explain-diagnostic` (rename from `.example`).],
  [`y` / `Y` / `Ctrl+Y` inside Ctrl+V t — timeline health critique.],
  [`hook.on_diagnostic` — Bund reaction to diagnostic state changes.],
  [`ink.editor.set_cursor` — 1-based cursor mover for hook-driven scripts.],
))
