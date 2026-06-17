# PLANNING-4 — Scene craft: scaffold, sequel, second opinion (1.3.5)

_Status: planning. Target: **1.3.5**. Deepens the scene layer the Board
opened in 1.3.4 (PLANNING-3 P3/P4). Three deferrals: AI **scene scaffold**
(propose a card from the prose), **sequel cards** (the reactive counterpart
to the proactive scene), and an AI-rated **tension second opinion**._

## Why

1.3.4 gave the Board a scene grain — goal → conflict → disaster cards with a
deterministic no-turn check — but authoring is all by hand, the scene/sequel
*rhythm* is only half-modelled, and the tension curve has one reading
(deterministic obligation-density). 1.3.5 closes those three gaps, each
building directly on what shipped:

1. **Scene scaffold** — the cold-start problem. You shouldn't have to type
   the goal/conflict/disaster from scratch; the prose already implies them.
2. **Sequel cards** — Swain's structure alternates *scenes* (proactive:
   goal/conflict/disaster) with *sequels* (reactive: reaction/dilemma/
   decision). The Board models only the proactive half.
3. **Second opinion** — the deterministic curve says "few obligations are
   open here"; an AI intensity rating says "the prose *feels* flat here."
   Two independent readings catch more than one.

## Builds on (already in tree)

- **PLANNING-3** — the `Scene` model + `scene_body`/`parse_scene` +
  `analyze_scenes` (P3); the `Scenes` chapter + `plan scene add|list|set|
  remove` + `load_scenes`/`find_scene`/`save_scene` write-back; the `v`
  scene board in the `Ctrl+V Shift+K` outline (P4).
- **The scaffold pattern** — `scaffold_intentions` + `parse_scaffold`: 3-tier
  prompt resolution (`resolve_plan_prompt`) → `run_blocking` → parse a
  structured response → write back. The scene scaffold is its sibling.
- **Prose extraction** — `book_walk::chapter_paragraphs_raw` +
  `audiobook::typst_to_plain` (already used by `chapter_positions`).
- **The tension curve** (PLANNING-3 P0/P1) — `tension_curve` / the outline
  sparkline overlay gains a third (AI) line.

## Dependencies

**None.** Scaffold + the AI rating reuse the existing LLM stack
(`AiClient` + `collect_blocking`, 3-tier prompts); sequel cards reuse the
scene write-back; the second-opinion line is a sidecar + an extra sparkline.

## Phases

### P0 — `plan scene scaffold` (AI card from the prose)

`inkhaven plan scene scaffold --chapter <slug> [--all] [--provider]`: read
the chapter's prose (`chapter_paragraphs_raw` → `typst_to_plain`), send it
with a scene-analysis system prompt (new `plan-scene-scaffold` slug, 3-tier
resolved) asking for the scene's **goal / conflict / disaster**, parse the
structured reply (`goal:` / `conflict:` / `disaster:` lines —
`parse_scene_scaffold`, pure + tested), and **upsert** a scene card for the
chapter (create via the P3 write-back, or update an existing same-titled
card). `--all` scaffolds every chapter that lacks a card. Deterministic
parse, fully testable; the AI call is the only non-pure part.

### P1 — scene scaffold in the TUI (interactive accept)

From the `v` scene board, **`g`** (generate) scaffolds the selected
chapter's card: stream the proposal into the AI pane; on completion an
accept prompt (**`a`** accept → write the card via P0's upsert, **`e`** edit
in the editor, **`s`** skip). Reuses the streaming-inference + the
[[1.3.3]] `L`-lift filing pattern.

### P2 — sequel cards (the reactive half)

Unify the card model: `Scene` gains `kind: "scene" | "sequel"` + the
reactive triple `reaction` / `dilemma` / `decision` (all `#[serde(default)]`
— old cards still parse as `scene`). `scene_body` / the board / the check
branch on `kind`: a **scene** is weak when it states a goal but never turns
(no disaster); a **sequel** is weak when it reaches a dilemma but never
**decides** (no decision → the story stalls). CLI `plan sequel add|list|
set|remove` (sets `kind=sequel`, the reactive fields); the scene board
shows both with a `scene`/`sequel` tag and an alternation hint (two scenes
with no sequel between = unprocessed disaster). Scaffold (P0/P1) gains
`--kind sequel`.

### P3 — the tension second opinion (AI-rated intensity)

Opt-in `inkhaven plan tension rate [--provider]`: an AI pass that reads each
chapter's prose and rates its dramatic **intensity 0–100**, cached in
`.inkhaven/tension-ai-<book>.json` (content-hash invalidated, like the
digest). `plan check`'s TENSION section gains an `ai` column and the
`Ctrl+V Shift+K` overlay a third sparkline (`ai`, distinct colour), so the
author compares **expected** (framework) vs **actual** (obligation density)
vs **felt** (AI). The deterministic curve stays the default; the AI line is
supplementary and absent until rated.

### P4 — docs + the 1.3.5 release cut

Tutorial 67 (scene scaffold, sequels, the second-opinion line); KEYBINDING
(`g` in the scene board); finalize `RELEASE_NOTES/1.3.5` + index + README
(last-release-only); version bump `1.3.5-dev → 1.3.5`; signed tag `v1.3.5`;
`cargo publish`; merge to main; open the next cycle.

## Non-goals (deferred)

- **Multi-scene chapters** — scaffold proposes one card per chapter this
  cycle; splitting a chapter into several scene/sequel cards by scene break
  (`#`) is a later refinement.
- **AI-rated per-scene tension** — the second opinion is per-chapter; a
  per-scene-card intensity is a finer pass for later.
- **The Whole-Book AI Editor** — still 1.4+.

## Test posture

P0's parser (`parse_scene_scaffold`) and P2's branched `analyze_scenes` /
`scene_body` round-trips are pure and exhaustively unit-tested (the AI call
is mocked out of the pure path, as `parse_scaffold` is). P3's cache
shape + the curve's third line are tested deterministically. TUI phases
(P1) follow the established read-state → drop-borrow → call-self pattern,
covered by keybind-regression + render-smoke tests.
