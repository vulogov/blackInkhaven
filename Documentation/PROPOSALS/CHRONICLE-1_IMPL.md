# CHRONICLE-1 — implementation plan (grounded, file-by-file)

*Companion to `CHRONICLE-1_PLAN.md`. Every anchor below was verified against the
tree on 2026-08-04. Phases CH-P0→P6; value core P1+P2+P3.*

---

## Grounded substrate (the pieces CHRONICLE builds on)

**Snapshot source — the whole diagnosis in one headless call**
- `crate::cli::editorial::collect(project: &Path, book_name: Option<&str>, only:
  Option<&[String]>, include_deferred: bool) -> Result<EditorialReport>` —
  `src/cli/editorial.rs:22`. Headless (opens its own `Store`/`Hierarchy`). Runs
  every reader.
- `editorial::EditorialReport { findings: Vec<EditorialFinding>, errors, warnings,
  infos: usize, deferred: usize, stale: bool }` — `src/editorial.rs:356`.
- `EditorialFinding { category: String, severity: Severity, location: Location,
  message, hint, source: &'static str, autofixable }` — `src/editorial.rs:97`;
  `severity: Severity{Error,Warn,Info}` (`:20`); `response() -> ResponseKind`
  (`:128`) → `ResponseKind{Rewrite,Decision,Brief}.label()`; **`fingerprint() ->
  String` = `category ⟂ message`** (`:114`) — the identity for cleared/introduced.

**Optional raw enrichment (LECTOR shape, P2+)**
- `crate::lector::walk::read_forward(store, cfg, layout, h) -> ReadThrough` —
  `src/lector/walk.rs:38`. `ReadThrough { chapters: Vec<ChapterRead>, curve:
  Vec<(f32,f32)> }` (`src/lector/mod.rs:129`); `ChapterRead.measured_intensity:
  Option<f32>` (`:109`). Mean intensity + sag count (expected−measured ≥ `SAG_GAP
  0.28`, floor `0.5`, `src/lector/shape.rs:20-22`) are the two numbers `collect`'s
  findings don't carry.

**Persistence template to mirror — `progress`**
- `ProgressStore { engine: Arc<StorageEngine> }`, `open(path)` →
  `StorageEngine::new(path, INIT_SQL, 2)` — `src/progress/store.rs:53-64`. Sibling
  convention: `OutputStore::open_for_project(root){ Self::open(&root.join(
  "output.db")) }` — `src/pane/output/store.rs:86`. → `ChronicleStore::
  open_for_project(root)` opens `<project>/chronicle.db`.
- `StorageEngine::new(path, init_sql, pool_size)` (`src/storage/engine.rs:39`),
  `execute_with(sql, &[&dyn ToSql])` (`:139`), `select_all_with(sql, args) ->
  Vec<Vec<duckdb::types::Value>>` (`:96`). DuckDB value unwrap handles
  `Int|BigInt|HugeInt|Double` (pattern at `src/progress/store.rs:321`).
- Day key: `crate::dayclock::today_days()` / `today_key() -> "YYYY-MM-DD"`
  (`src/dayclock.rs:49,61`).

**Draft marker** — git surface is *only* `git::uncommitted_count(root)`
(`src/git.rs:20`) and `docs.rs:git_changed_files` (`--name-only`,
`src/cli/docs.rs:125`). **No tag listing / ref resolution exists.** So a milestone
is an explicit `chronicle mark`; `--ref` is a stored string, never resolved.

**Free chords** — only `Ctrl+B Shift+U` and `Ctrl+B Shift+Z` are unbound across all
scopes in the `meta_sub` table (`src/tui/keybind.rs:1580-1770`; resolve_in is
first-match, `Scope::Any` shadows later scopes). Use **Shift+U**, `Scope::Any`.

**Bund policy sync** — `every_registered_word_is_classified` test
(`src/scripting/policy.rs:757`) fails until each `ink.chronicle.*` word is in
`WORD_CATEGORIES` (`:117`) as `category::STORE_READ` (`:58`, default-allowed).

---

## CH-P0 — substrate (pure types + store)

New `src/chronicle/mod.rs` (+ `mod chronicle;` in `main.rs`).

- `pub struct MetricVector` (`#[derive(Serialize, Deserialize, Clone, Default)]`):
  `total: usize`, `errors/warnings/infos: usize`, `by_category:
  BTreeMap<String,usize>`, `by_response: BTreeMap<String,usize>` (rewrite/decision/
  brief), `by_source: BTreeMap<String,usize>`, `deferred: usize`, `stale: bool`,
  and enrichment `mean_intensity: Option<f32>`, `sag_count: usize`.
- `pub struct Milestone { id: Uuid, label: String, day: i64, ts: i64, book_slug:
  Option<String>, git_ref: Option<String>, metrics: MetricVector }`.
- `pub struct FingerprintSet(BTreeSet<String>)` with `{severity, category}` kept
  alongside each fp (a small `FindingRef{fingerprint, category, severity, chapter:
  Option<String>, paragraph: Option<Uuid>}`) so the dashboard can jump to an
  introduced finding and `check` can gate on severity.
- New `src/chronicle/store.rs` — `ChronicleStore { engine: Arc<StorageEngine> }`,
  `open`/`open_for_project` mirroring ProgressStore. `INIT_SQL`: two tables —
  `milestones(id TEXT PK, label TEXT, day BIGINT, ts BIGINT, book_slug TEXT,
  git_ref TEXT, metrics_json TEXT)` + `milestone_findings(milestone_id TEXT, fp
  TEXT, category TEXT, severity TEXT, chapter TEXT, paragraph TEXT)` (index on
  milestone_id). Methods: `insert_milestone(&Milestone, &[FindingRef])`,
  `list_milestones(book_slug) -> Vec<Milestone>`, `findings_for(id) ->
  Vec<FindingRef>`, `latest(book_slug) -> Option<Milestone>`, `by_label(label)`.
- All `#[allow(dead_code)]` until P1 consumes (the allow-until-consumer idiom).
- Tests: MetricVector serde round-trip; store insert→list→findings_for round-trip
  on a tempdir.

## CH-P1 — capture (value: the milestone) 

- `chronicle::capture(project: &Path, book_name: Option<&str>) -> Result<(
  MetricVector, Vec<FindingRef>)>` — ONE `collect(project, book_name, None, false)`
  call, tally the report: `total = findings.len()`, err/warn/info from the report,
  `by_category`/`by_source` folded from findings, `by_response` from
  `f.response().label()`, `deferred`/`stale` from the report; `FindingRef` per
  finding from `fingerprint()` + category + severity + `location.label()` +
  `location.paragraph`. (P2 adds the `read_forward` enrichment; P1 leaves
  `mean_intensity=None, sag_count = by_category["shape_sag"]`.)
- New `src/cli/chronicle.rs` + `Command::Chronicle{ ChronicleCmd }` in
  `src/cli/mod.rs` (subcommands, like `graph`/`continuity`): `mark { label,
  ref: Option<String>, book_name }`, `list { book_name, json }`. `mark` =
  capture → `ChronicleStore::open_for_project` → `insert_milestone`; prints the
  headline. `list` = `list_milestones` → table.
- Tests: `capture` on a fixture returns a vector whose `total == findings.len()`;
  `mark` then `list` shows the milestone (CLI smoke, no LLM — `collect` core is
  deterministic).

## CH-P2 — trend + diff report (value: the arrows)

- Pure `diff_vectors(old: &MetricVector, new: &MetricVector) -> Vec<MetricDelta>`
  where `MetricDelta { key: String, old: i64, new: i64, direction: Better|Worse|
  Same }`. **Direction polarity is per-metric**: fewer findings/errors/sags = Better
  (▼); a rising `mean_intensity` toward the curve is *not* scored (report as
  neutral) — keep polarity honest and only score counts where "fewer is better".
- `chronicle` (bare) = capture live → diff vs `latest()` → render the ▲/▼/= table
  (`Severity::icon`-style arrows). `chronicle diff <a> <b>` = `by_label` both →
  diff. `--json` dumps deltas.
- P2 enrichment: `capture` now also calls `read_forward` once for `mean_intensity`
  (mean of `Some` `measured_intensity`) + `sag_count` (findings already have it;
  keep for parity). Guard: read_forward is cheap/deterministic but opens the store
  — reuse the store handle collect-style if trivial, else a second open (the TUI
  already double-opens; acceptable).
- Tests: `diff_vectors` polarity (errors 4→2 = Better; confusion 0→1 = Worse;
  info 5→5 = Same); bare `chronicle` on a project with one prior mark.

## CH-P3 — the REDLINE hook: cleared / introduced (THE value core)

- Pure `diff_findings(old: &[FindingRef], new: &[FindingRef]) -> FindingDiff {
  cleared: Vec<FindingRef>, introduced: Vec<FindingRef>, persisted: Vec<FindingRef>
  }` — set difference on `fingerprint`. (Old = the stored milestone's
  `milestone_findings`; new = the live capture's FindingRefs.)
- Fold into the P2 report: the `✓ N cleared · ▲ M introduced · K unchanged` line +
  an itemised "introduced" list (category · location · message-head), sorted
  severity-first. `--json` includes the three lists.
- This is the flagship's payload — it's what makes CHRONICLE close REDLINE's loop.
- Tests: cleared/introduced/persisted partition is correct + disjoint; an
  introduced error surfaces in the itemised list.

## CH-P4 — the dashboard chord

- `Ctrl+B Shift+U` → `Action::OpenChronicle` → `Modal::Chronicle{ deltas,
  finding_diff, since_label }`. Renders the P2 table + the P3 cleared/introduced
  split; `Enter` on an introduced finding with a `paragraph` → `open_paragraph_by_
  uuid` (the editorial-pass jump pattern). `m` in the modal = quick `chronicle mark`
  (prompt a label). `Esc` closes.
- Wire: `keybind.rs` meta_sub entry (`Scope::Any`, Shift+U), `modal.rs` variant,
  `render.rs` dispatch, `render/modals.rs` `draw_chronicle_modal`, key router.
- **Guard test** (the SENTINEL/LECTOR lesson): assert `resolve_in` maps `Ctrl+B
  Shift+U` → `OpenChronicle` and that the existing Shift+U-adjacent chords
  (`Ctrl+B u` = UndoLastDelete Any; Shift+S = SearchFacts; Shift+I =
  OpenContinuityLedger) are unshadowed — enumerate the whole group.

## CH-P5 — Bund + policy

- New `src/scripting/stdlib/chronicle.rs` (mirror `stdlib/revise.rs`): `ink.
  chronicle.marks` ( -- list ) the milestones as dicts; `ink.chronicle.trend`
  ( -- dict ) the live deltas vs latest {key, old, new, direction}; `ink.chronicle.
  check` ( -- dict ) {introduced, cleared, clean} where `clean` = no introduced
  error-severity finding since the last mark. All wrap `capture` + the stored
  latest milestone. Register in `stdlib/mod.rs`.
- Policy: add the three words to `WORD_CATEGORIES` as `category::STORE_READ`
  (`src/scripting/policy.rs`); `every_registered_word_is_classified` then passes.
- Tests: a module unit test on the dict shape (like revise's); the policy test
  already guards classification.

## CH-P6 — capstone (docs + e2e, the last phase before release)

- `Documentation/CHRONICLE.md` (mirror REDLINE.md/LECTOR.md): thesis, the metric
  vector, the trend, cleared/introduced, surfaces, the safety framing (pure
  measurement), Bund, multilingual, what-it-is-not.
- `Documentation/Tutorials/114-the-draft-chronicle.md` + index row.
- `KEYBINDING.md` (Ctrl+B Shift+U), `CONFIGURATION.md` (only if a `chronicle:` block
  is added — likely just a retention/enabled knob; decide in P1, else note none),
  top-level `README.md` "Latest release" refresh, `RELEASE_NOTES/2.5.0.md` draft +
  index row, a DEVELOPING-book audit (the fiction "revision" section gains the
  did-it-get-better beat).
- e2e: `chronicle mark` → edit → `chronicle` shows a delta + an introduced finding;
  `ink.chronicle.check` gate; dashboard jump. Suite green + warning-free.

---

## Open decisions (resolve as we build)

- **`chronicle:` config** — probably one knob (`enabled` for a review-pass line?
  `retention` cap on milestones). Or none (mark is explicit). Decide at P1/P5.
- **Auto-capture** — deliberately *out* (no git-tag enumeration exists; marks are
  decisions). Revisit only if a `hook` naturally fits.
- **LLM "what changed" letter** — an optional, cost-capped synthesis over the diff
  (reuse `redline::call`), deferred past the value core; nice-to-have, not the trend.
- **Review-pass line** — a `chronicle` line on `Ctrl+B Shift+C` (drift-since-last-
  mark) is a natural P4/P5 add if cheap.
