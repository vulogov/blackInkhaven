# EDITORIAL-1 — The Editorial Pass (1.3.6)

_Status: planning. Target: **1.3.6**. The capstone the 1.3 line has pointed
at since 1.2.22 ("the substrate for the 1.3 editorial-pass cycle"): unify
every revision detector — the prose ones AND the Planning Board's structure
findings — into one ranked, walkable, jump-to-location worklist._

## Why

Inkhaven has ~12 detectors, but a writer doing a revision pass runs each
separately and aggregates in their head: `doctor --scan` (a dozen classes),
`plan check` (gaps / drift / pacing / flat tension / weak scenes / sequels),
`facts scan`, `tension scan`, `thread doctor`, echo / numeric-contradiction
/ continuity-drift / naming, fact-check. They point at different things
(file paths, chapter titles, paragraph numbers) in different shapes
(`ScanFinding`, `FactFinding`, `Unresolved`, `EchoFinding`, plain strings).

The Editorial Pass collapses that into **one worklist**: every finding,
ranked by severity, each one you can **jump to**, act on, skip, or defer —
the pre-submission sweep that turns "something's off, somewhere" into a
checklist.

## Builds on (already in tree)

- **`doctor_scan`** — `ScanFinding { class, severity: ScanSeverity, path:
  Option<String>, detail }` + `scan_project(project, selected)` + the
  `apply_fix` dispatch. The author-judgment classes (DroppedCharacter,
  PacingCollapse, StalledThread, NamingInconsistency, EchoRepetition,
  NumericContradiction, ContinuityDrift, UnresolvedTension,
  ParagraphTooLong) ARE editorial findings — already normalized.
- **Sidecar detectors** — `facts_scan.json` (`FactFinding`), the tension
  ledger (`detect_unresolved` → `Unresolved`), the continuity bible. Read
  what's already computed; no live AI in the default pass.
- **`plan check`** — the structural findings (`PlanReport.warnings` + the
  structured beats / scenes / tension). Needs a small structured surface.
- **The cockpit substrate** — `open_paragraph_by_uuid` + `load_paragraph`
  (cursor restore); the `DoctorPanel` walk pattern; the `CommentsPanel`
  jump-to-location (open by UUID + position cursor at a char offset); the
  snapshot API (`create_snapshot_annotated`).

## The crux: location resolution

Today findings point at **file paths** (doctor) or **chapter titles /
1-based paragraph numbers** (facts, echo, tension). The cockpit needs a
**paragraph node id (+ optional char range)** to jump. So the unifying
work is a `Location` that the aggregator *resolves*:

- doctor `path` → the node whose `file` matches (the `apply_fix` path walk).
- chapter title / index → the chapter node (the `ChapterPos` order).
- echo `para_start` (1-based) → the Nth paragraph node under its chapter.

`EditorialFinding { category, severity, location, message, hint, source,
autofixable }`; `Location { book, chapter, paragraph: Option<Uuid>,
char_range: Option<(usize,usize)>, path }`. Findings that don't resolve to a
paragraph still list (jump lands on the chapter).

## Dependencies

**None.** The deterministic pass aggregates existing scan outputs + sidecars
(no live LLM); fixes reuse `apply_fix` + the snapshot API; the cockpit
reuses the open-by-uuid + cursor machinery. The AI tier (P3) reuses the
existing scan commands.

## Phases

### P0 — the unified model + aggregator + `inkhaven edit`

`EditorialFinding` + `Location` (in a new `src/editorial.rs`). A pure
`aggregate(findings from each source) -> Vec<EditorialFinding>` that
**maps + ranks** (severity desc, then category, then position) + dedups. The
CLI `inkhaven edit [--json] [--only <cats>] [--book <name>]` runs the
editorial doctor classes (via `scan_project`), reads the facts / tension /
continuity sidecars, and folds in `plan check`'s structural findings; maps
each native finding into `EditorialFinding`, resolving `Location`. Text
worklist + `--json` (CI gate). Mapping is pure + unit-tested per source;
the underlying scans are already tested.

### P1 — the cockpit (walk + jump-to-location)

A TUI modal on a free `Ctrl+V` chord (e.g. `Ctrl+V Shift+E`, **E**ditorial):
the ranked worklist with a severity icon + category tag + one-line message
per finding, the selected one's detail + hint expanded; `↑↓` navigate,
category/severity filters, **`Enter`** jumps to the finding's location
(`open_paragraph_by_uuid` + position the cursor at `char_range` when
present, else the chapter's first paragraph). Read + navigate.

### P2 — actions (fix / skip / defer)

In the cockpit: **`f`** applies the autofix where one exists (the doctor
`apply_fix` classes — snapshot-gated for any content write via
`create_snapshot_annotated`); **`s`** skips for the session; **`d`** defers
— persisted to a `.inkhaven/editorial-dismissed.json` sidecar (keyed by a
stable finding fingerprint, like `ScanFinding.detail`'s stability) so
accepted / not-now findings don't resurface until the prose changes.
Navigator-first: most editorial findings are author-judgment (the "fix" is
you editing at the jumped-to location); autofix only where mechanical.

### P3 — the AI deep tier (`--deep`)

`inkhaven edit --deep [--provider]` first **refreshes the AI sidecars** —
runs `facts scan`, `tension scan`, the continuity extract, fact-check — then
aggregates, so the worklist includes the semantic findings. Opt-in (needs a
provider); the deterministic pass stays the default and CI-able. The cockpit
gains a `D` "deep refresh" trigger mirroring the CLI.

### P4 — docs + the 1.3.6 release cut

A new tutorial (68, *The Editorial Pass*); KEYBINDING (`Ctrl+V Shift+E`);
finalize `RELEASE_NOTES/1.3.6` + index + README; version bump
`1.3.6-dev → 1.3.6`; signed tag `v1.3.6`; `cargo publish`; merge to main;
open the next cycle.

## Non-goals (deferred)

- **AI rewrite-in-place** — "fix this echo / flat scene for me" (lift an AI
  rewrite into the buffer from a finding). A natural follow-up, not this cut.
- **Merging doctor + editorial** — `doctor` stays *project integrity*
  (zero-byte files, orphan rows, bdslib drift); `edit` is *manuscript
  readiness*. They share the author-judgment classes; they're not merged.
- **New detectors** — this cycle unifies what exists; it doesn't add
  detection.

## Test posture

P0's per-source mappers + the rank/dedup + the `Location` resolver are pure
and exhaustively unit-tested (synthetic `ScanFinding` / `FactFinding` /
`Unresolved` → expected `EditorialFinding`s; a path/title/para-number →
node-id resolution test over a synthetic hierarchy). The CLI `--json` shape
is pinned. TUI phases (P1/P2) follow the read-state → drop-borrow → call-self
pattern, covered by keybind-regression + render-smoke tests; the defer
sidecar round-trips.
