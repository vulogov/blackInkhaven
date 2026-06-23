# TIMELINE-2-INTEGRATION — implementation plan (1.3.x tree)

Adapts **RFC TIMELINE-2-INTEGRATION** ("Refactoring the Legacy Timeline
Critique") to the live 1.3.x codebase. The RFC targets a hypothetical 1.8.0; we
land it incrementally in the 1.3.x line, one signed phase per increment, the same
way WORLD-4 / INNER_SOCRATES-1 / WORLD-5 were built.

This is the final piece of the timeline-aware trilogy. WORLD-5 (shipped 1.3.30)
established the substrate; this RFC removes the now-duplicated audit items from the
legacy AI critique, keeps the two genuinely timeline-internal checks, and migrates
them from AI-pane streaming to the PANE-1 Output pane.

## What exists today (the legacy critique)

- `src/timeline/critique.rs` — `build_health_payload(events, calendar, hierarchy,
  scope_crumb, track_filter, default_track) -> String`. Flattens events + a
  five-item **audit checklist** (travel-time/co-location, paragraph mismatches,
  fuzzy overlaps, orphan signals, pacing) into an LLM prompt.
- `src/tui/app/timeline_impl.rs::timeline_start_health_critique(widen_to_book,
  widen_to_all_tracks)` — the `y` / `Y` / `Ctrl+Y` / F12 handlers. Builds the
  payload, resolves the `timeline-health` prompt template, spawns a chat stream,
  routes the result to the **AI pane**.
- `src/tui/app.rs::timeline_health_default_prompt(lang)` — embedded EN/RU/ES/FR/DE
  default prompt (the "05-timeline-health-example.typ" of the RFC; we have no such
  Typst file — the template lives in code + optional `prompts.hjson` override).
- Orphan auto-tagging: `store::reconcile_event_orphan_tag` + `EventData::is_orphan`
  (orphan = no linked paragraphs, no characters, no places). Fires
  `hook.on_event_orphaned`.
- `EventData` (`src/store/node.rs`): `start_ticks, end_ticks, precision,
  characters, places, track`. `Precision` (`src/timeline/presets.rs`):
  Tick/Hour/Day/Week/Month/Season/Year. No `summary`/`notes` field.
- `TimelineEvent` (`src/tui/timeline_state.rs`): runtime snapshot — adds
  `is_orphan`, `linked_paragraphs`, `book_prefix`, `title`, `id`.
- `inkhaven event` CLI (`src/cli/event.rs` + `cli/mod.rs`): `add` / `list` /
  `show`. **No `critique` subcommand yet.**
- Bund `ink.event.*` (`src/scripting/stdlib/ink.rs`): list, list_orphans, add,
  set_end, set_precision, set_track, link_paragraph.

## Adaptation decisions (RFC → 1.3.x)

1. **Version references.** RFC's 1.8.0 → "this cycle"; deprecation lifecycle
   (1.9.0 louder warning, 1.10.0/2.0.0 removal) → "a later 1.3.x". The `--legacy`
   flag ships with a deprecation warning from the start.
2. **Module shape.** `src/timeline/critique.rs` becomes a directory module
   `src/timeline/critique/`. The legacy payload builder moves verbatim into
   `critique/legacy.rs` (re-exported, so `timeline_impl.rs` keeps compiling).
3. **Detection input.** A critique-local `CritiqueEvent` decouples the detectors
   from `tui::timeline_state::TimelineEvent` and makes them pure + trivially
   testable. TUI and CLI both build `CritiqueEvent`s from their snapshots.
4. **Significance without `summary`/`notes`.** The data model has neither field
   (changing it is a non-goal). We adapt the RFC's significance heuristic to the
   signals we *do* have: precision concreteness (a `Day`-precision event is more
   committed than a `Year` one), title richness, and track activity (events on a
   track that carries many linked events elsewhere). Documented as an adaptation.
5. **Staleness.** "Orphaned for >N days" needs an event age. The pure detector
   takes `age_days: Option<i64>` per `CritiqueEvent`; the caller supplies it from
   the node's creation time + wall clock (or `None` when unknown → treated as
   Recent). Keeps the core deterministic.
6. **Fuzz windows.** Precision→tick window comes from the calendar
   (`ticks_per`). The pure detector takes a precomputed `FuzzWindows` so tests
   don't need a full `Calendar`.
7. **Severity.** The pure core uses a local `CritSeverity {Info, Warning,
   Contradiction}`; it maps to `pane::output::Severity` only at emission (P1),
   keeping the detectors dependency-light. A `severity_label` (Notice/Inquiry/
   Probe-style) rides in metadata for rendering parity with the siblings.
8. **No magic ledger.** Per RFC §14: the retained checks are timeline-internal;
   the ledger stays WORLD-4 territory.

## Phases

- **P0 — refactored critique core.** Restructure `critique/` into a directory.
  `CritiqueEvent`, `Significance`, `Staleness`, `Suspicion`, `CritSeverity`,
  `OrphanFinding`, `FuzzyOverlapFinding`, `Scope`. `orphan::detect` (+
  significance/staleness → severity). `fuzzy_overlap::detect` (pairwise suspicion
  scoring + 3+ event cluster detection + confidence). Legacy payload preserved in
  `critique/legacy.rs`. Pattern-detected reason text. Unit tests. **(this increment)**
- **P1 — pane integration.** `timeline_orphan_warning` +
  `timeline_fuzzy_overlap_warning` message kinds; `critique/pane.rs` emission
  (`⊗` / `⊕` glyphs, `timeline-critique` provenance); Output rendering + action
  hints in `render/panes.rs`. Wire the `y`/`Y`/`Ctrl+Y`/F12 chords to run the
  refactored critique and emit to Output instead of streaming to the AI pane.
- **P2 — optional LLM elaboration.** Per-finding elaboration via the shortened
  prompt template; cost-cap (`max_calls_per_run`, `confirm_above_calls`);
  pattern-only fallback when no LLM. `timeline.critique` config block.
- **P3 — legacy preservation + migration tooling.** `inkhaven event critique`
  (with `--track`/`--view`/`--book`), `--legacy` (old AI-pane behavior + deprecation
  warning), `--migration-check`, `--diff`. Coverage report.
- **P4 — Bund stdlib.** `ink.event.critique.{orphan_check, fuzzy_overlap_check,
  run, config, custom(reserved no-op)}`. Tests.
- **P5 — docs + polish.** Update `Documentation/Tutorials/31-story-timeline.md`;
  new `32-timeline-critique-migration.md`; multilingual finding text (EN/RU/ES/FR/DE);
  HJSON defaults; deprecation-warning wording.

## Non-goals (unchanged from RFC)

Timeline data model, timeline CLI surface (`event add/list/show`), timeline TUI
chords' *surfaces*, `ink.event.*` semantics, the calendar/TimelinePoint system —
all unchanged. No new external dependencies. WORLD-4/WORLD-5/INNER_SOCRATES-1
unmodified (they are the *replacement* for the removed items). Bund-programmatic
custom rules reserved (`ink.event.critique.custom` no-op) but not implemented.

## Increment log

- **P0** — _done._ Restructured `src/timeline/critique.rs` → `critique/` directory.
  `types.rs` (`CritiqueEvent`, `Scope`, `CritSeverity`, `Significance`/`Staleness`,
  `Suspicion`, `OrphanFinding`/`FuzzyOverlapFinding`, `FuzzWindows`). `orphan.rs`
  (significance × staleness → severity, `ScopeContext` track-activity, min-significance
  filter). `fuzzy_overlap.rs` (pairwise suspicion scoring + connected-component
  cluster detection with common-window gate + 10-event list cap). `legacy.rs` holds
  the original `build_health_payload` verbatim (re-exported). `mod.rs` adds
  `fuzz_windows(calendar)`, `CritiqueReport`, and `run()`. 18 unit tests pass.
- **P1** — _done._ Two new Output kinds (`timeline_orphan_warning`,
  `timeline_fuzzy_overlap_warning`) in `pane/output/types.rs`. `critique/pane.rs`
  emits them (`timeline-critique` provenance, `timeline:true` 📅 marker, headlines
  assembled from caller-resolved date/entity labels, `CritSeverity` →
  `pane::Severity`). `render/panes.rs`: orphan `⊘` / overlap `⧉` kind glyphs +
  a `⏎ jump to event` action-hint arm. Chord rewiring in `timeline_impl.rs`: the
  scope prologue factored into `timeline_critique_scope`; `timeline_start_health_critique`
  now projects view events → `CritiqueEvent`, runs `critique::run`, emits findings,
  and surfaces the Output pane; the original AI-pane streaming survives verbatim as
  `timeline_run_legacy_critique` (for the P3 `--legacy` flag). Orphan age is `None`
  (no creation timestamp in the model → Recent). Full suite green (1740).
</content>
</invoke>
