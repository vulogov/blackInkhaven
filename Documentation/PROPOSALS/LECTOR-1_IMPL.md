# LECTOR-1 — Implementation Plan (grounded, file-by-file)

*Companion to [`LECTOR-1_PLAN.md`](LECTOR-1_PLAN.md). Anchors verified against the
tree at 2.3.0-dev. Nothing built.*

## Grounded anchors (the reuse map)

**Structure substrate (SHAPE) — extend, never rebuild:**
- `planning.rs` — `Framework` enum (`src/planning.rs:52`, `parse` :71), `BeatSpec
  {name, act, target_position, expected_tension}` :20, `Beat` + `parse_beat` :181,
  `analyze_*_prompt` :195/:206, `analyze` :363. **`tension_curve` + `intensity_
  sparkline` + `plan tension rate` (LLM second opinion) already build an
  expected-vs-actual-vs-AI intensity curve** — but "actual" needs author-tagged
  tensions/threads (`has_actual` false otherwise). LECTOR feeds it a **measured**
  actual.
- `book_digest.rs` — `BookDigest` (:29), the per-chapter summary `analyze` maps
  beats over. The stateful walk reuses this.
- `cli/mod.rs::PlanCommand` (:1986 — `Init`/`Check`/…) — the CLI family to extend.

**Reader substrate (AUDIENCE) — the graph payoff:**
- `continuity_intel/introduce.rs` (SENTINEL CT-P1) — roster + `dirty_scope`-style
  first-mention-vs-first-scene. LECTOR's "who is this?" confusion finding is the
  same primitive, walked forward: an entity *used* in a chapter before its
  introduction chapter. Reuse `introduce::scan` / the roster helper (`pub(super)`).
- `tension.rs` — `TensionTag` (:59), `TensionLedger` (:75), introduce/resolve
  matching. LECTOR's "unpaid setup / still open in ch. N" reuses the ledger.
- `character/` (CHAR-1) — arcs + agency per chapter (live stakes / who's active).
- `inner_grounding.rs` — the "what the author declared" prompt prefix (cast/arcs/
  symbols/world tensions) the synthetic read is grounded on.
- `inner_socrates/personas.rs` — reader-persona `stance` + genre framing for the
  synthetic-read prompt voice.

**Intensity signals (deterministic, per chapter) — already computable:**
- Dialogue density: `dialogue/` (DIALOG-1) spans/stats.
- Sentence rhythm / readability / reading time: NARR-1 (`prose/`) + `tui/
  readability.rs` / `tui/reading_time.rs`.
- Conflict/stakes lexicon: the `continuity::built_in_lexicon(lang)` pattern
  (`continuity.rs:135`) — a new per-language stakes/conflict word list, EN/RU/DE/
  FR/ES, skipping cleanly where none ships.
- Prose read from disk: `cli::book_walk::chapter_raw_prose` + `audiobook::
  typst_to_plain` (the SENTINEL numeric adapter's pattern).

**Surface plumbing:**
- Output kind: add `pub const LECTOR: &str = "lector";` in `pane/output/types.rs`
  (+ `filter.rs` SOURCES / `message_source` + a `panes.rs` glyph — the SENTINEL
  CT-P4 checklist).
- Review pass: `run_unified_check` (`tui/app.rs:15470`) + the `run_*_check` shape
  (`run_continuity_check`); CLI mirror `cli/check.rs`.
- Dashboard: clone `Modal::ContinuityLedger` (SENTINEL CT-P6) — rows + parallel
  anchors + cursor; `draw_continuity_ledger_modal` shape; **a free `Ctrl+B` chord —
  candidates `Shift+T` (read-Through) / `Shift+M` (story Map) / `Shift+I`; VERIFY it
  isn't already bound (the concordance-shadow lesson — enumerate the WHOLE meta
  group, add a `…_still_bound` guard test).**
- Slow LLM pass: `world::fact_check_slow` + `slow_llm_call` (now `pub(crate)`,
  reused by SENTINEL CT-P7) + `BgJobKind` + a completion arm (the `ContinuitySlow`
  shape).
- Bund: the `ink.continuity.*` twin (`scripting/stdlib/continuity.rs`) + **classify
  every word in `scripting/policy.rs`** (the `every_registered_word_is_classified`
  test enforces it).

---

## Phase map

### LR-P0 — The read-through substrate (pure)
- New `src/lector/mod.rs`: `ChapterRead { chapter:u32, title, measured_intensity:
  Option<f32>, new_entities:Vec<String>, opened_threads:Vec<String>, resolved_
  threads:Vec<String>, findings:Vec<ReaderFinding> }`; `ReaderFinding { kind:
  &'static str, severity, chapter, anchor:Option<Uuid>, message, source }` (mirror
  `ContinuityFinding`); `ReadThrough { chapters:Vec<ChapterRead>, curve:Vec<(f32,
  f32)> }` with `rank`/`dedupe` primitives. Pure; tested. Module-level
  `#![allow(dead_code)]` until P1/P3 consume it.

### LR-P1 — Prose-measured intensity (SHAPE core)
- `src/lector/intensity.rs`: pure `chapter_intensity(signals) -> f32` combining
  dialogue density, stakes/conflict lexicon hits, sentence-rhythm acceleration
  (short-sentence ratio), scene-vs-summary ratio, chapter-end turn; impure
  `measure(layout, h, cfg)` per user-book chapter. New per-language stakes lexicon
  (`built_in_lexicon` pattern). Feed the sequence into `planning::tension_curve` as
  a tagging-free "actual". Tests: a high-conflict dialogue-dense chapter scores >
  a quiet summary chapter; RU stakes words match.

### LR-P2 — Scene/sequel classification (SHAPE new axis)
- `src/lector/scene_sequel.rs`: pure classifier over prose signals + the goal/
  conflict/disaster fields `planning::parse_beat` surfaces → `Scene | Sequel |
  Mixed` per scene; `rhythm(chapters) -> Vec<Kind>` + arrhythmia findings (a run of
  ≥N scenes with no sequel = breathless; ≥N sequels = sag). Tests on synthetic
  runs.

### LR-P3 — The forward reader-state walk (AUDIENCE core, deterministic)
- `src/lector/walk.rs`: `read_forward(store, cfg, layout, h) -> ReadThrough` — walk
  user-book chapters in `Hierarchy::flatten` order, accumulating introduced entities
  (via `introduce`), open threads (via `tension.rs`), arcs; per chapter derive the
  zero-AI reader findings: `confusion` (entity used before introduced — SENTINEL
  reuse), `info_dump` (≥K new entities in one chapter), `attention_dip` (measured
  intensity low + few new entities/threads), `unpaid_setup` (thread open > W
  chapters / open at end), `put_down_risk` (sustained low-intensity low-progress
  run). Tests: forward-only (a later resolution doesn't cancel an earlier dip);
  info-dump fires on a 5-new-name chapter.

### LR-P4 — The synthetic first-read (AUDIENCE, LLM, explicit)
- `src/lector/synthetic.rs`: `run(project, scope, max_cost, force) -> Vec<Reader
  Finding>` (self-contained, bg-safe, the CT-P7 `coherence::run` shape). Forward
  pass: per chapter, prompt = `inner_grounding` prefix + running-state summary +
  chapter prose, asking the model to react **as a first reader who does not know the
  ending** (clarity / stakes / engagement / page-turn / put-down), forbidden from
  referencing later chapters. `slow_llm_call` (cost-capped), reader-persona stance,
  book language. Maps to `ReaderFinding` `source:"reader"`.

### LR-P5 — The read-through report + dashboard
- `src/cli/readthrough.rs` + `Command::Readthrough { book, json, deep, max_cost }`:
  the realized-vs-intended curve (sparkline, reuse `intensity_sparkline`), the
  per-chapter reader beats, ranked findings; `--deep` folds in P4. Nonzero exit on
  a `put_down_risk` at the end (optional CI gate).
- `Modal::ReadThrough { rows, anchors, cursor }` (clone `ContinuityLedger`): the
  curve + reader beats grouped by chapter, Enter → jump-to-chapter, `k` → run the
  synthetic read. The `Ctrl+B` chord (P0 open decision).

### LR-P6 — The review-pass rails
- `kinds::LECTOR` (+ filter/glyph). `run_lector_check(store, cfg, layout) ->
  Result<usize>` (the `run_continuity_check` shape): the deterministic structural
  (`plan check` + scene/sequel + intensity flat-spots) + reader findings, emitted +
  anchored. Fold into `run_unified_check` (+`lc` total/status) AND `cli/check.rs`.
  Gated on `lector.enabled`.

### LR-P7 — Genre-aware frameworks + kishōtenketsu
- `planning::Framework::Kishotenketsu` (four-part, conflict-optional) + its
  `BeatSpec` table; framework auto-suggest from `cfg.genre` (a mapping); per-genre
  stakes/conflict lexicon variants. Multilingual.

### LR-P8 — Bund + config + docs
- `src/scripting/stdlib/lector.rs`: `ink.readthrough.report` ( -- list ),
  `ink.readthrough.curve` ( -- list ), `ink.readthrough.check` ( -- dict ). Register
  + **classify STORE_READ in policy.rs**. `lector:` config block (`enabled`,
  per-signal weights, `info_dump_threshold`, `open_thread_window`, `deep_max_cost`).
  New `Documentation/LECTOR.md`.

### LR-P9 — Capstone
- Tutorial (the read-through workflow), KEYBINDING (the dashboard chord), README
  index + `RELEASE_NOTES`, CONFIGURATION (`lector:` block), the DEVELOPING book,
  the multilingual note. Verify the forward read-through end-to-end on a fixture
  (a planted info-dump + an unpaid setup + a saggy middle all surface).

---

## Cross-cutting
- **Advisory, forward-only, deterministic-first.** The SHAPE half + the P3 reader
  findings are zero-AI and free; P4 is the one LLM pass, explicit + cost-capped.
- **Unify, don't duplicate** — the curve, `introduce`, `tension.rs`, arcs, the
  metrics, the grounding prefix, the rails, the ledger-modal + slow-pass patterns
  are all reused.
- **No new crates; warning-free; 1.2.15.**
- **Value core = P1 + P3 + P4 + P5.** The two halves ship independently: **P1+P2**
  complete the Planning Board (measured shape, no tagging); **P3+P4** deliver the
  synthetic beta read. P5 unifies them into the one report.

## Open decisions (resolve during P0/P5)
1. **The dashboard chord** — `Shift+T` / `Shift+M` / `Shift+I`; verify against the
   full meta group + add a guard test (the SENTINEL Shift+L/concordance lesson).
2. **One module or two** — `src/lector/` unifying both halves (recommended, since
   the walk is shared) vs. extending `planning.rs` for SHAPE and a separate
   `audience` module. Lean: one `lector/` that *calls* `planning`.
3. **Intensity weights** — fixed vs. `lector:` config-tunable (lean config, like
   `chorus` thresholds); calibrate on a real manuscript before fixing defaults.
4. **Put-down-risk as a CI gate** — offer a nonzero exit, but off by default (it's
   a judgment signal, unlike a hard contradiction).
