# Output pane filtering (road to 1.4.0)

Roadmap item after the command palette ([ROADMAP-1.4.0.md](ROADMAP-1.4.0.md)).
Bundled into the current 1.3.33 cycle (palette + filtering ship together). The
Output pane now collects findings from ≥5 subsystems on one surface; this lets the
user narrow it by **source**, **severity**, and **the open paragraph**.

## The provenance problem

There is no single provenance field on `Message` — it's scattered:

- `fact_check_warning` / `socratic_inquiry` are identified by their `kind`.
- the timeline kinds carry `metadata.provenance = "timeline-critique"`.
- socratic carries `metadata.persona_id`; fact-check carries `metadata.category`.

So the human filter must not be raw `kind` strings. We define a **source
classifier** `message_source(&Message) -> &'static str` mapping every kind to a
clean group (`fact-check`, `socrates`, `timeline-critique`, `translation`,
`lexicon`, `variety`, `world`, `ai`, `bund`, `other`). The filter operates on that.

## Phases

- **P0 — pure filter core** (`src/pane/output/filter.rs`). `message_source`,
  `OutputFilter { source, min_severity, only_open_paragraph }`, `matches(msg,
  open_paragraph)`, `is_active`, a one-line `summary()` for the header, and the
  `SOURCES` cycle list. Pure → unit-tested. **(this increment)**
- **P1 — wire into the pane.** Filter in-memory in `draw_output` (the pane already
  fetches `active()` each frame — no SQL change), a **filter header** showing the
  active filter + `shown/total`, and keys in `handle_output_key` (cycle source,
  cycle min-severity, toggle "this paragraph only", clear). Keep `output_selected`
  clamped to the filtered list.
- **P2 — persistence.** Persist the active filter in `.session.json` next to
  `right_pane`. Optional saved-filter presets.
- **P3 — stability rider + docs.** Filter `matches` proptest; KEYBINDING/quickref
  rows. Then cut the bundled release.

## Non-goals

No change to the `Message` schema, the store SQL, or how subsystems emit. Filtering
is a read-side view over `active()`. No new deps.

## Increment log

- **P0** — _done._ `src/pane/output/filter.rs`: `message_source(&Message)` (kind →
  one of 10 human source groups), `OutputFilter { source, min_severity,
  only_open_paragraph }` (Serialize/Deserialize for P2) with `matches(msg,
  open_paragraph)`, `is_active`, `clear`, `summary()`, and `cycle_source` /
  `cycle_min_severity` (Progress ranks lowest so "≥Warning" hides task ticks).
  Pure; 8 unit tests. Re-exported from `pane::output`. Full suite 1782 → 1790.
</content>
