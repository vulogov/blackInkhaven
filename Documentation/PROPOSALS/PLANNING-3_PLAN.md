# PLANNING-3 — The Planning Board: the shape of the rise and fall (1.3.4)

_Status: planning. Target: **1.3.4**. Finishes pillar B (structure). 1.3.2
diagnosed beat **placement**; 1.3.3 made mapping **fluent**. 1.3.4 adds the
two things the Board still can't see — **intensity** (a tension curve) and
**scene-level** craft (scene cards) — plus the deferred outline extras._

## Why

The Planning Board answers *"is each beat in the right place?"* and
*"does each act carry the right word-share?"* — both about **position**. It
says nothing about **intensity**: a midpoint can land at exactly 50% and
still be flat. And it stops at the beat (chapter-ish) grain — it can't see
inside a chapter to the scene, where the actual craft of escalation lives.

1.3.4 closes both, staying true to the Board's contract: **deterministic
diagnosis first (`plan check`), AI commentary second (`plan analyze`)** —
no AI in the curve itself.

1. **Tension curve** — for every position, an *expected* intensity (the
   framework's authored dramatic shape) vs an *actual* intensity derived
   from the manuscript, so a sagging middle is an objective finding
   ("Midpoint: tension 0.3 where the framework wants 0.7") not a vibe.
2. **Scene cards** — a finer grain than beats: each scene's
   goal / conflict / disaster, with a deterministic *weak-scene* check (a
   scene that states a goal but never turns).
3. **Outline extras** — the deferred PLANNING-2 P1.2 items: link a thread
   to a beat (`t`) and jump the editor to a beat's chapter (`Enter`).

## The tension model (the crux)

**Expected tension** is authored, exactly like `target_position`: each
`BeatSpec` gains an `expected_tension: f32` (0..1), giving every framework a
canonical intensity curve (Setup low → Midpoint raised → All-Is-Lost /
Climax peak → Resolution release).

**Actual tension** is *open narrative obligation density* — the count of
debts in the air at a position — derived deterministically from data the
author already maintains, no render-time AI:

- **Primary — the tension ledger** (`src/tension.rs`,
  `.inkhaven/tensions.json`, populated by `inkhaven tension scan`): an
  `Introduce` opens an obligation; the matching `Resolve` (shared-stem, at
  `chapter_index >=` the intro — the existing matcher) closes it. At
  chapter index *i*, `open(i)` = introduced-by-*i* minus resolved-by-*i*.
- **Secondary (enrich, optional) — open threads**: a Threads-book arc is
  "open" between the first and last manuscript chapter that links it
  (the weave-view `grid[thread][chapter]` already computes this), weighted
  by the thread's `tension: i32` (0–10) field.

`actual(position)` = normalized sum of those open obligations, sampled at
each chapter's `ChapterPos.start` (the existing index→0..1 map). Graceful
degradation: **no ledger and no thread links → no actual curve**, just the
expected one, with a hint to run `inkhaven tension scan`.

The finding: where **expected is high but actual is low** (beyond a
threshold), flag the beat — the objective version of "the middle sags."

## Builds on (already in tree)

- **PLANNING-1/2** — the `Beat` / `BeatSpec` model, `beat_body` /
  `parse_beat` write-back, `analyze()` + `PlanReport`, `chapter_positions`
  (word-fraction `ChapterPos`), the `Ctrl+V Shift+K` outline modal +
  `map_plan_beat` / `cycle_plan_status` / `edit_beat` / `load_beats`.
- **`src/tension.rs`** — `TensionLedger` / `TensionTag` + the shared-stem
  Introduce↔Resolve matcher (`resolve` logic) and `.inkhaven/tensions.json`.
- **Threads** (`src/cli/thread.rs` `ThreadFull`, `threads_impl.rs` weave
  grid) — the open-span + per-thread `tension` magnitude.
- **The threads picker modal** (`open_threads_picker`) — reused for the
  outline `t` thread-link picker.
- **The structure-outline renderer** (`draw_plan_outline_modal`) — gains
  the curve rows.

## Dependencies

**None.** The curve is pure arithmetic over the existing ledger / threads /
chapter positions; scene cards reuse the HJSON-paragraph write-back; the
outline extras reuse the threads picker + the editor's load-chapter path.

## Phases

### P0 — the tension model (deterministic core)

In `src/planning.rs`: add `expected_tension: f32` to `BeatSpec` and author
the value on every beat of all five framework tables. Add a pure
`tension_curve(beats, chapters, ledger_opens, thread_spans) -> TensionCurve`
that returns, per beat, `{ expected, actual, gap }` plus a sampled
`Vec<(position, actual)>` series for the overlay. `actual` = normalized
open-obligation density at the beat's mapped position. Extend `PlanReport`
with the curve + a `tension` warning class ("Midpoint: actual 0.3 vs
expected 0.7 — flat"). Pure + fully unit-tested with synthetic ledgers.

In `src/cli/plan.rs`: `build_report` loads the ledger
(`TensionLedger::load`) + thread spans, maps `chapter_index → position`,
and feeds `tension_curve`. `plan check` grows a **TENSION** section; absent
data prints the run-`tension scan` hint. `--json` carries the curve.

### P1 — the tension overlay in the outline (`Ctrl+V Shift+K`)

`draw_plan_outline_modal` gains two sparkline rows under the position bar —
`expected` and `actual` (Unicode block-ramp `▁▂▃▄▅▆▇█`), aligned to the
same 0..1 x-axis as the beats, with sagging beats tinted (the
`theme.style_warning_*` palette). The selected beat shows its
`tension a/e` numerals. Read-only; no new keys here.

### P2 — outline extras (the deferred PLANNING-2 P1.2)

In the `Ctrl+V Shift+K` outline:

- **`t`** — open the threads picker scoped to the selected beat; toggle
  threads on/off; write back via `edit_beat` (set `beat.threads`). Reuses
  `open_threads_picker` + the P0 write-back.
- **`Enter`** — jump the editor to the selected beat's `mapped_chapter`
  (load the chapter's first paragraph, focus the editor), or a status hint
  if the beat is unmapped.

### P3 — scene cards (model + CLI)

A **scene** is a structured card finer than a beat. Model in
`src/planning.rs`: `Scene { chapter: String (slug), title: String, goal:
String, conflict: String, disaster: String, status: String }`, stored as
HJSON paragraphs under a **`Scenes`** chapter of the Planning book (reusing
`beat_body` / `parse_beat`-style render/parse + the write-back primitive).
CLI `inkhaven plan scene add|list|set|remove`:
`scene add --chapter <slug> --goal … --conflict … --disaster …`. A
deterministic **weak-scene** check (folded into `plan check` and a
`scene-no-turn` finding): a scene with a goal but an empty `disaster`
*doesn't turn*; a chapter with no scene card is *unplanned at scene grain*.

### P4 — scene cards (TUI + AI)

A scene view (`Ctrl+V Shift+S` or a sub-mode of the outline) listing each
chapter's scene cards with the goal→conflict→disaster spine and the
weak-scene flags. `plan analyze` extends to comment on scene turns over the
digest. Optional: `plan scene scaffold --chapter <slug>` proposes a card
from the chapter prose (AI, 3-tier prompt) for interactive accept — the
scene-grain analogue of `plan scaffold`.

### P5 — docs + the 1.3.4 release cut

Tutorial 67 (or a new 68) for the tension curve + scene cards; KEYBINDING
for `t` / `Enter` / the scene view; finalize `RELEASE_NOTES/1.3.4` + index
+ README (last-release-only); version bump `1.3.4-dev → 1.3.4`; signed tag
`v1.3.4`; `cargo publish`; merge to main; open the next cycle.

## Non-goals (deferred)

- **Render-time AI tension rating** — the curve stays deterministic; AI
  stays in `analyze`.
- **Per-scene reordering / drag** — scene cards are annotation, not a
  corkboard, this cycle.
- **Sequel cards** (reaction / dilemma / decision) — only the
  goal/conflict/disaster proactive scene this cycle.
- **The Whole-Book AI Editor** — still 1.4+.

## Test posture

P0 is pure and exhaustively unit-tested (synthetic ledgers + thread spans →
known curves; the expected-tension tables pinned like the pacing
proportions). P3's scene model + weak-scene check are pure and tested
end-to-end through the CLI write-back. TUI phases (P1/P2/P4) follow the
established "read modal state → drop borrow → call self methods → rebuild"
pattern; covered by keybind-regression + render-smoke tests as in 1.3.3.
