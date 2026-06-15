# PLANNING-1 — The Planning Board (1.3.2) implementation plan

_Status: planning. Target: **1.3.2**. Pillar B of the 1.3 theme — the
**front** of the lifecycle, structure. First slice leads with **analyze +
pacing**: diagnose an existing draft's structure, not organize cards._

## Thesis

Authors don't struggle to *organize* — corkboards and index cards are
everywhere. They struggle to **diagnose**: *"something's wrong with my
middle but I can't see it."* The Planning Board's job is to turn that vague
unease into a **specific, located, objective finding** — which no other
tool does well. So 1.3.2 leads with the diagnosis, not the card wall.

## The third axis

Inkhaven already has **Timeline** (*when* things happen) and **Threads**
(*whether each arc pays off*). The Planning Board is the orthogonal third:
**structure** — acts, beats, turning points, the shape of the rise and
fall. A beat sheet is the spine; threads weave along it; the timeline is
its calendar. The Board **references** threads/timeline, it never
re-implements them — that's what keeps it from becoming "Threads 2."

## Builds on (already in tree)

- **The 1.3.1 book digest** (`src/book_digest.rs`) — the compact whole-book
  context the AI analyze pass (P3) maps against. Direct synergy with the
  cycle just shipped.
- **Word counts** (`src/progress`) — the deterministic pacing/proportion
  engine (P1) is almost free over data already tracked.
- **`SYSTEM_BOOKS` + `ensure_system_books`** — a new `Planning` book
  auto-seeds on open like `Submissions` did.
- **Threads** (HJSON-fronted paragraphs) — the model pattern beats reuse,
  and the link target for "which arc advances here".
- **`inkhaven tension scan`** (1.2.19) — the curve for the tension overlay
  (P4), when the author has run it.

## Dependencies

**None.** Pure software — frameworks are data, the pacing engine is
arithmetic over word counts, the analyze pass reuses the existing LLM
stack + the digest.

## Data model

A new **`Planning`** system book (joins `SYSTEM_BOOKS`, after Threads).
Beats live as **HJSON-fronted paragraphs** (the Threads pattern):

```hjson
{ framework: "save_the_cat", beat: "Midpoint", act: 2,
  target_position: 0.50, mapped_chapter: "<chapter-slug-or-null>",
  threads: [], status: "planned" }
```

Beats need their own existence (you have beats before chapters, and
several beats per chapter), so they're nodes with a `mapped_chapter` link —
not chapter tags. **Frameworks** ship as built-in templates — each just an
ordered list of `{ beat, act, target_position }`:

- `three_act`, `save_the_cat` (15), `story_circle` (8), `hero_journey`
  (12), `seven_point`. Pick one per book; custom is a later add.

## Phases

### P0 — frameworks + the structure model (landed)

`src/planning/` (or `src/planning.rs`): the `Framework` enum + the
built-in beat tables (`{ beat, act, target_position }`), the `Beat` record
(serde), and the `Planning` system-book seeding. `inkhaven plan
init [--framework save_the_cat]` scaffolds the chosen framework's beats as
paragraphs in the Planning book. Tests: every built-in framework has
monotonic target positions in `[0,1]`, act boundaries consistent, distinct
beat names; round-trip of a beat through HJSON.

### P1 — coverage + pacing engine (deterministic — the sleeper feature) (landed)

The diagnosis, no AI:

- **Coverage** — which beats are mapped to a chapter, which are gaps.
- **Position drift** — each mapped beat's *actual* position (its chapter's
  cumulative word fraction) vs `target_position`; flag drift past a
  threshold ("Midpoint lands at 0.65, target 0.50").
- **Pacing / proportion** — word-count fraction per **act** vs the
  framework's expected shape ("Act 2 is 30% of your words — saggy or
  rushed").

`inkhaven plan check [--book] [--json]` prints the report. Tests:
synthetic books with known word distributions → expected drift / pacing
findings; gap detection; act proportions.

### P2 — the structure-outline view (TUI) (landed)

A new `Ctrl+V` chord opens the **structure outline** (not a 2-D corkboard —
terminal-hostile, lower value): beats down the page, each with its target
position, mapped chapter, status, linked threads, and a **position /
coverage bar** that shows drift + gaps + act pacing at a glance. Navigate /
map a beat to the chapter under the cursor / jump to a beat's chapter.

### P3 — AI analyze (the headline) (landed)

`inkhaven plan analyze [--book] [--provider]`: over the **1.3.1 digest** +
the chosen framework, the LLM proposes a beat→chapter mapping and names the
structural problems ("your 'bad guys close in' stretch is doing the work
of a midpoint; the real midpoint is missing"). Writes the suggested
mappings into the Planning book (the author confirms); surfaced in the P2
view + a TUI chord that streams the analysis into the AI pane. Prompt
resolves through the 3-tier resolver (`plan-analyze` slug → Prompts book →
`prompts.hjson` → built-in).

### P4 — insight integrations (threads landed; tension overlay deferred)

- **Tension overlay** — plot the actual `tension scan` curve against the
  framework's *expected* tension shape; flag where they diverge ("tension
  dips where the framework peaks"). Conditional on a tension scan existing.
- **Threads / Timeline links** — each beat references which thread advances
  + the timeline event(s) it covers, so a beat *does narrative work*. The
  view shows them; `plan check` flags beats that advance no thread.

> **P4 status:** thread-links landed (beats surface + validate their `threads`; `plan check`/outline show them, unknown refs + a no-thread nudge warn). The **tension-curve overlay is deferred** — `tension scan` tracks introduced/resolved threads, not a positional curve; a faithful overlay needs a tension model (expected-tension per beat + derivation from linked-thread tensions), a P4.2.

### P5 — docs + release

Tutorial 67 (planning a book's structure); KEYBINDING (the outline chord);
CONFIGURATION (`planning:` defaults — framework, drift threshold);
RELEASE_NOTES/1.3.2 finalize, README, version bump, signed tag, `cargo
publish`, merge to main.

## Risks / decisions

1. **Frameworks are subjective.** Ship a handful, make the positions
   configurable, and frame findings as *prompts to think*, not verdicts —
   the deterministic ones ("Act 2 is 30%") are factual; the AI ones are
   suggestions.
2. **Beat→chapter mapping is ambiguous** (several beats per chapter, a beat
   spanning chapters). v1: one chapter per beat, many beats per chapter
   allowed; richer spans later.
3. **Tension overlay depends on an opt-in scan** — degrade cleanly to "run
   `tension scan` to see the curve" when absent.
4. **Don't duplicate Threads.** Beats *link* threads; they don't restate
   arc state. Pin this with the link model in P4.

## Out of scope (1.3.2)

- **Plan-first scaffolding** (premise → AI beat outline → scaffold
  chapters) — the blank-page mode; a strong 1.3.3 once analyze proves the
  framework model.
- **Scene cards** (per-scene goal / conflict / disaster) — a *finer* grain
  than beats; a separate later layer, not conflated with the skeleton.
- **A 2-D draggable corkboard** — the outline view is the better TUI
  surface.
- **Facts-push / Research-pull** — genre-specific; defer.

## Sequencing

P0→P1 is a shippable cut on its own (the framework model + the deterministic
diagnosis — the part authors feel most, no AI, no risk). P2 makes it
visible; P3 adds the qualitative AI map; P4 the insight overlays. A partial
1.3.2 (P0–P2) is still a real release.
