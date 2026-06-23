# RFC WORLD-5 — Timeline Integration with the World Fact-Checker

| | |
|---|---|
| **RFC** | WORLD-5 |
| **Title** | Timeline Integration with the World Fact-Checker (event-aware fact-checking) |
| **Status** | **In progress — building incrementally in the 1.3.x cycle** |
| **Created** | 2026-06-27 |
| **Author** | Vladimir Ulogov |
| **Target version** | author wrote 1.7.0; **pulled forward into the 1.3.x tree** (same cadence as WORLD-4 / INNER_SOCRATES-1) |
| **Depends on** | WORLD-4 (fact-checker) — **COMPLETE (1.3.25–1.3.27)** · the timeline feature (1.2.6+) — present |
| **Soft-depends on** | INNER_SOCRATES-1 — **COMPLETE (1.3.28–1.3.29)**; shares the timeline context provider |
| **External dependency** | none beyond WORLD-4 |

---

> ## Status banner (1.3.x incremental build)
>
> Author targeted 1.7.0; per Vladimir's direction we build incrementally in the
> **1.3.x** tree, one phase per signed increment. **ZERO new deps** (the RFC commits
> to this in §11). WORLD-5 is a **connector**, not a foundation — it gives the
> WORLD-4 fact-checker access to the timeline's events so several checks gain ground
> truth instead of prose inference, plus two new categories.
>
> The new module is **`src/world/timeline_context/`** — the shared
> `TimelineContextProvider` the RFC describes (§8.1). INNER_SOCRATES-1 already reads
> events (`src/inner_socrates/timeline.rs`); WORLD-5 builds the richer, calendar-aware
> provider both can consume.
>
> ### What we reuse (no reinvention)
>
> | Need | Reused from |
> |---|---|
> | Event data (start_ticks, precision, track, linked_paragraphs, characters, places) | the timeline feature (`TimelineEvent` / `EventData` on nodes) |
> | Calendar + season model | `timeline::calendar` (`Calendar`, `SeasonDef`, `decompose`) |
> | Event gathering from the hierarchy | the `inner_socrates::timeline::gather_events` shape |
> | The fact-check `Finding` + `emit_finding` + magic-ledger consult | `world::fact_check` |
> | Cost preflight / retry / multilingual baseline | WORLD-4 (`fact_check_slow`, `fact_check_lang`) |
> | Output emission (`fact_check_warning` kind) | PANE-1 |
>
> ### Purely additive (RFC §1, §7.5)
>
> The compiler, plakat, Fast/Slow architecture, magic ledger, Output kinds, CLI/TUI
> surface, and DuckDB schema are **unchanged**. WORLD-5 only adds the context the
> checker has, the two new categories, a `--timeline-aware` flag, a `📅` marker, and
> five read-only Bund words. Projects without a timeline get exactly WORLD-4.
>
> ### Phasing (RFC §12 re-cut for 1.3.x)
>
> - **P0** — the `TimelineContextProvider`: `TimelineContext` value (linked +
>   nearby events, effective date + source, effective season), calendar-aware
>   `season_for`, per-paragraph context building, the event helpers (`events_near`,
>   `events_for_character`). Pure + tested.
> - **P1** — extend the existing categories with the context (travel_time gains
>   event-derived durations; climate gains calendar-grounded season; astronomy /
>   demographics / economy gain dated grounding).
> - **P2** — the two new categories: `date_coherence` (prose date-hints vs the
>   linked event's season) and `co_location` (a character in two places in
>   overlapping event windows).
> - **P3** — the `--timeline-aware` / `--timeline-only` CLI flags + the five
>   `ink.world.fact_check.timeline.*` Bund words.
> - **P4** — multilingual date-hint tables (RU/ES/FR/DE) + per-language warning text.
> - **P5** — polish: the `📅` Output marker, coexistence-guidance docs, performance.
>
> **Discipline:** the legacy timeline critique (1.2.6+) is **not** touched (that's
> TIMELINE-2-INTEGRATION); the intent ledger is **not** consulted (that's
> INNER_SOCRATES-1's lane). The fact-checker handles *world coherence*; magic rules
> are its only exceptions.

---

## Increment log

- **P0.1 — timeline context provider core (UNRELEASED, 1.3.30-dev).** New
  `src/world/timeline_context/`: the `TimelineContext` value + `DateSource`,
  `gather_events` (richer `TlEvent` with linked_paragraphs / characters / places),
  `build_context(paragraph, events, calendar)` (linked events → effective date →
  season; nearby events within a window), and the `events_near` /
  `events_for_character` / `events_for_place` helpers. Adds
  `Calendar::season_for(point)` (calendar-aware, wraparound-safe). All pure +
  tested; degrades to empty context when the project has no events.
- **P1.1 — climate gains calendar-grounded season (UNRELEASED, 1.3.30-dev).** The
  headline P1 win: `fact_check::check_timeline(text, ctx, ledger)` flags prose
  whose weather contradicts the **timeline-dated season** — snow in a paragraph the
  timeline places in summer is a **contradiction** (dated ground truth, not prose
  inference). A localized `Msg::DateSeason` renders in all five languages; the
  magic ledger suppresses it (a `weather_control` rule covering `climate_anomaly`).
  Conservative: only the common `summer`/`winter` season names are
  temperature-mapped (custom/conlang names degrade to no finding). Wired into the
  CLI `fact-check --paragraph` (auto: runs when the project has events + a calendar;
  no-op otherwise) via `timeline_findings`. +2 tests, 1720. *Next: travel_time
  event-derived durations + the P2 cross-paragraph categories (date_coherence,
  co_location).*
- **P2.1 — the `co_location` category (UNRELEASED, 1.3.30-dev).** A character
  placed in two *different* places at overlapping event times — a contradiction
  the timeline alone reveals, no prose needed. `co_location_conflicts(events)` is
  pure + tested (overlap via event spans; ignores shared-place and no-place pairs).
  CLI `realworld co-location` gathers the events, resolves character/place names
  from the hierarchy, applies the magic ledger (a `teleportation`-style rule
  covering `co_location` suppresses to info), and emits `fact_check_warning`
  findings. +2 tests, 1722.
- **P1.2 — travel_time event-derived durations (UNRELEASED, 1.3.30-dev).** The
  RFC's flagship: `check_travel_timeline` flags a prose-stated travel duration that
  contradicts the timeline gap between the paragraph's linked event and the
  traveller's prior different-place event (prose "three days" vs a 35-day gap → a
  warning). Localized `Msg::TravelTimeline` in all five languages; magic-ledger-
  respecting; tolerant within ~3× (narrative compression). Wired into the CLI
  `timeline_findings`. +1 test, 1723. *P1's two headline extensions (climate +
  travel) ship; astronomy/demographics/economy dated grounding need a
  world-state-over-time model (out of WORLD-5's additive scope) — deferred. Next:
  `date_coherence` (P2) + the P3 flags/Bund.*
- **P2.2 — the `date_coherence` category (UNRELEASED, 1.3.30-dev).** A seasonal
  date-hint in the prose (a festival, a harvest, a solstice) that contradicts the
  timeline-dated season — a midsummer feast in a winter-dated paragraph → `warning`
  (softer than weather; a festival may be metaphorical). A canonical-season mapper
  (`summer`/`winter`/`spring`/`autumn`, custom names degrade) + an English
  date-hint table (`midsummer`, `harvest`, `yule`, …; per-language tables in P4);
  localized `Msg::DateCoherence` in all five languages; magic-ledger-respecting.
  Wired into the CLI `timeline_findings`. +1 test, 1724. **Both new WORLD-5
  categories (co_location + date_coherence) + both P1 extensions ship.** *Next: P3
  — the `--timeline-aware` flag + `ink.world.fact_check.timeline.*` Bund words.*
- **P3 — CLI flags + Bund words (UNRELEASED, 1.3.30-dev).** `fact-check` gains
  `--timeline-aware auto|on|off` (default auto — timeline checks run when a
  paragraph is identified and the project has events) and `--timeline-only` (skip
  the world checks, run only the timeline ones). Five read-only
  `ink.world.fact_check.timeline.*` Bund words (`events_near`,
  `events_for_character`, `events_for_place`, `season_for`, `effective_date`) in a
  new `scripting/stdlib/world_timeline` module. Surface-only; the checks are
  unchanged. 1724. *Next: P4 multilingual date-hints (RU/ES/FR/DE) + P5 polish/docs.*
- **P4 — multilingual date-hint tables (UNRELEASED, 1.3.30-dev).** `date_coherence`
  detection now works in all five languages: per-language hint tables (RU/ES/FR/DE
  festivals + solstitial / agricultural references — `разгар лета`, `pleno verano`,
  `plein été`, `Hochsommer`, harvests, …) selected by the detected language. The
  messages were already localized (P2.2); this completes the *detection* side.
  Verified end-to-end in all five languages. +1 test, 1725. *Next: P5 — the `📅`
  Output marker + coexistence-guidance docs.*
