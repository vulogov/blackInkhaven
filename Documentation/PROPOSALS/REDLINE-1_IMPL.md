# REDLINE-1 — Implementation Plan (grounded, file-by-file)

*Companion to [`REDLINE-1_PLAN.md`](REDLINE-1_PLAN.md). Anchors verified against the
tree at 2.4.0-dev (Editorial-Pass inventory). Nothing built.*

## Grounded anchors (the reuse map)

**The aggregation — extend, never rebuild:**
- `EditorialFinding { category:String, severity, location{chapter, paragraph:
  Option<Uuid>, char_range:Option<(usize,usize)>, path}, message, hint, source }`
  — `src/editorial.rs:98`. `fingerprint()` `:114` (= `"{category}\u1{message}"`,
  drives defer). `rewritable() = location.paragraph.is_some() && fix_spec(category)
  .is_some()` — `:120` (**the one gate REDLINE widens**).
- `fix_spec(category) -> Option<FixSpec{slug,builtin,label,scope}>` — `:150`; four
  categories only (echo/pacing/show-tell/filter). `FixScope::{Paragraph,Span}` `:127`.
- `batch_fix_queue(findings, filter) -> Vec<BatchFix=(Uuid,String,Option<(usize,
  usize)>)>` `:197`; `splice_span` `:216`, `extract_phrase` `:230`.
- Converters `from_scan_finding` `:305` / `from_fact_finding` `:335` /
  `from_fact_conflict` `:349` / `from_drift_conflict` `:365` (has `paragraph_b`) /
  `from_plan_warning` `:388`. `ScanClass::editorial_category` — `cli/doctor_scan.rs:287`.
- **The hub:** `cli::editorial::collect(project, book_name, only, include_deferred)
  -> EditorialReport` — `src/cli/editorial.rs:22` (pulls doctor · facts · drift ·
  plan · prose-style; `resolve_locations` :94; staleness :68; defer-hide :104).
  `prose_style_findings` `:126` is the only source emitting a `char_range`.
- `Dismissed` (defer sidecar `.inkhaven/editorial-dismissed.json`) — `editorial.rs:262`.

**The rewrite → diff → snapshot contract (reused VERBATIM):**
- `start_editorial_rewrite(&mut self, category, span)` — `src/tui/app/ai_impl.rs:1592`
  (looks up `fix_spec`, resolves the prompt via `resolve_prompt(slug, lang, builtin)`,
  marks the `« »` phrase for a Span fix, `spawn_chat_stream(…, "editorial-rewrite")`,
  sets `pending_rewrite_diff`/`pending_rewrite_span`; **never writes the buffer**).
- `pump_inference` routes the completed stream → `open_ai_diff_review_with_snapshot`
  (whole) / `open_ai_diff_review_final` (spliced span) — `src/tui/app.rs:4746`.
- `Modal::AiDiffReview { before_lines, after_lines, action, scroll, post_accept_
  snapshot:Option<String>, wrapped_total }` — `src/tui/modal.rs:663`.
- `ai_diff_review_handle_key` — `src/tui/app/ai_impl.rs:2205`: accept (`a`/`e`/`Enter`)
  `:2289` calls `snapshot_open_paragraph_with_annotation` **iff** `post_accept_
  snapshot.is_some()`, **then** `apply_ai_diff_accepted` (`src/tui/app.rs:29034`);
  reject (`r`) `:2329` leaves the buffer untouched. **The prose buffer is mutated
  nowhere else.**
- Snapshots — `src/tui/app/snapshot_impl.rs`: `snapshot_open_paragraph_with_
  annotation` `:168`, `commit_snapshot_annotation` `:119`; `F6` picker `open_snapshot_
  picker` `:188` / `commit_snapshot_load` `:519` (restore snapshots first, `:566`).
- `EditorialBatch{queue:VecDeque<BatchFix>,total,applied,skipped}` — `app.rs:7432`;
  `advance_editorial_batch` `:13446`.

**The surface:**
- `Modal::EditorialPass{findings, cursor, scroll, filter}` — `src/tui/modal.rs:855`;
  chord `Ctrl+V Shift+R` = `A::OpenEditorialPass` (`keybind.rs:159`, entry `:1879`
  under the view-sub prefix); `open_editorial_pass` `app.rs:13308`;
  `editorial_pass_handle_key` `:13336` (`f` `:13394`, `F` `:13409`, `Enter` jump).

**The judgment sources to convert (anchor → `location.paragraph`):**
- Inner Stylist `Finding{severity,kind,key,message}` — `src/inner_stylist/mod.rs:44`
  (book/voice-level → mostly Brief; a per-chapter voice finding → Rewrite/Decision).
- SENTINEL `ContinuityFinding{kind,severity,chapter,anchor:Option<Uuid>,entities,
  message,source}` — `src/continuity_intel/mod.rs:54` (co_location/char_facts →
  Decision; introduce → Decision; numeric single-para → Rewrite).
- LECTOR `ReaderFinding{kind,severity,chapter,anchor:Option<Uuid>,entities,message,
  source}` — `src/lector/mod.rs:76` (unpaid_setup/confusion → Decision; info_dump/
  attention_dip → Rewrite/Brief; put_down_risk/shape_sag/arrhythmia → Brief).
- Inner Editor `EditorFinding{category,severity,observation,evidence:Option<String>,
  …}` — `src/inner_editor/types.rs:122` (**strongest Rewrite candidate** — open
  paragraph + an `evidence` phrase → a Span).
- drift `DriftConflict{chapter_b, paragraph_b:Uuid}` (already `from_drift_conflict`)
  → Rewrite/Brief (reconcile `paragraph_b`). tension `TensionTag` (`tension.rs:59`)
  → Decision/Brief. Socrates `SocraticFinding` (`inner_socrates/types.rs:162`) →
  Brief (interrogative).

**LLM plumbing:** `spawn_chat_stream(client,model,system,history,prompt,category)`
`src/ai/stream.rs:53` (records cost via `ai::usage::record`); `resolve_prompt(name,
lang,fallback)` `app.rs:6797` (3-pass override); `start_bg_job(kind,label,work)`
`app.rs:4191` + `BgJobKind` `:7458`; cost-capped `realworld::slow_llm_call(project,
label,system,prompt,soft_cap,force)` `src/cli/realworld.rs:287`.

---

## Phase map

### RD-P0 — The response-kind substrate (pure)
- `src/editorial.rs`: `enum ResponseKind { Rewrite, Decision, Brief }` + `response_
  kind(category) -> ResponseKind` (echo/pacing/show-tell/filter/editor/drift-fix/
  voice/numeric → Rewrite; co_location/char_facts/introduce/unpaid_setup/confusion →
  Decision; shape_sag/put_down_risk/arrhythmia/distinctiveness/register/tension →
  Brief; default → Brief). Add `response: ResponseKind` to `EditorialFinding` (or a
  method). Pure; tested for the mapping. No behaviour change.

### RD-P1 — Bring the judgment readers into the queue (value)
- `src/editorial.rs`: `from_stylist_finding` / `from_continuity_finding` /
  `from_lector_finding` / `from_editor_finding` — each maps the source finding →
  `EditorialFinding` with `location.paragraph` from its anchor (chapter → first
  paragraph when only a chapter is known, reusing LECTOR's `first_paragraph_of`
  pattern) and the `response_kind`.
- `src/cli/editorial.rs::collect`: pull SENTINEL (`continuity_intel::engine::run`),
  LECTOR (`lector::deterministic_findings`), Stylist (`inner_stylist::pipeline::
  gather`) into the report. The Editorial Pass now shows the whole diagnosis, each
  item tagged Rewrite/Decision/Brief. +tests.

### RD-P2 — More rewrites (widen `fix_spec`)
- `src/editorial.rs::fix_spec`: add slugs + built-in prompts for `editor`
  (Span, from `evidence`), `drift-fix` (Paragraph, reconcile description), `voice`
  (Paragraph, sharpen a character's flat dialogue), `numeric` (Paragraph). Each new
  slug is `Prompts`-book-overridable via the existing `resolve_prompt`. These flow
  through `start_editorial_rewrite` **unchanged** — no new prose-write path. +tests
  (`rewritable()` true for the new categories with a paragraph).

### RD-P3 — The decision flow
- `Modal::RevisionDecision { finding, options:Vec<DecisionOption>, cursor }`
  (`src/tui/modal.rs`) — presents the choice (which scene/value/placement is right).
  On pick, build a targeted prompt ("rewrite paragraph P to be consistent with
  {chosen}") and call `start_editorial_rewrite` with a synthetic `FixSpec`
  (`decision-resolve` slug) + the chosen paragraph opened → the normal confirmed
  diff. Handlers per Decision category (co_location, char_facts, introduce,
  unpaid_setup, confusion). The **choice is the author's**; the write is confirmed.

### RD-P4 — The brief flow
- `redline::brief(project, finding, max_cost, force) -> String` (self-contained, the
  LECTOR `coherence::run` shape): grounds on the finding + the relevant scope, asks
  the LLM for a specific, actionable revision brief (no rewrite), runs under
  `slow_llm_call`. A `BgJobKind::RedlineBrief` spawner emits it to the **Thoughts**
  pane; the buffer is never touched. For structural findings (shape_sag,
  put_down_risk, arrhythmia, book-level stylist, tension).

### RD-P5 — The editorial letter (`inkhaven revise`)
- `src/cli/revise.rs` + `Command::Revise{book, deep, max_cost, json}`: run `collect`
  (+ the RD-P1 sources), then one synthesis LLM pass over the ranked findings → a
  prioritized, thematically-grouped developmental letter (big picture → continuity →
  structure → voice → line), with each item's response kind noted. `--json` for
  tooling; default is the letter. Reuses the report + `slow_llm_call`.

### RD-P6 — The revision-pass surface
- `editorial_pass_handle_key`: `f` on a **Rewrite** → the existing path; on a
  **Decision** → open `Modal::RevisionDecision`; on a **Brief** → spawn `brief`.
  Row rendering shows the response glyph (✎ rewrite · ⇄ decision · ✉ brief). The
  Editorial Pass is now the revision pass. (Chord stays `Ctrl+V Shift+R`.)

### RD-P7 — Batch + reversibility polish
- Batch (`F`) applies **Rewrite** items only (Decision/Brief are inherently
  per-item); confirm this in `advance_editorial_batch`. Audit: every accepted change
  carries a `post_accept_snapshot`; the letter/brief never write. Defer works across
  all three kinds (fingerprint-keyed).

### RD-P8 — Bund + config + docs
- `src/scripting/stdlib/revise.rs`: `ink.revise.findings` ( -- list, each with its
  response kind ), `ink.revise.check` ( -- dict, counts by kind ). Register +
  **classify STORE_READ in policy.rs**. `redline:`/`revise:` config (`enabled`,
  per-source toggles, `brief_max_cost`). New `Documentation/REDLINE.md`.

### RD-P9 — Capstone
- Tutorial (the revision workflow), KEYBINDING (`Ctrl+V Shift+R` re-described),
  CONFIGURATION, README index, RELEASE_NOTES, the DEVELOPING book. Verify the loop
  end-to-end on a fixture (a rewrite, a decision→rewrite, a brief), confirming a
  snapshot lands before every accepted change.

---

## Cross-cutting
- **Confirm-then-snapshot is structural, not conventional.** REDLINE adds no prose-
  write path; every fix routes through the reused `start_editorial_rewrite` →
  diff-review → accept contract.
- **Deterministic-first.** The queue + classification are free; rewrite/decision use
  the existing streaming path; brief + letter are explicit, cost-capped.
- **No new crates; warning-free; 1.2.15.**
- **Value core = P1 + P2 + P3 + P5.** P4 (briefs) is the honest structural handling;
  P6–P9 are surface/docs.

## Open decisions (resolve during RD-P0/P3)
1. **`response` on `EditorialFinding` vs a lookup** — a stored field (set by each
   `from_*`) vs `response_kind(&category)` at render. Lean stored (a source may
   override the category default, e.g. a numeric break local to one paragraph =
   Rewrite, cross-paragraph = Decision).
2. **Decision → rewrite prompt shape** — one generic `decision-resolve` slug
   parameterised by the choice, vs per-category slugs. Lean generic + a per-category
   preamble.
3. **Which continuity/LECTOR kinds are Rewrite vs Decision vs Brief** — calibrate on
   a real manuscript; start conservative (Decision/Brief) and promote to Rewrite only
   where a single-paragraph fix is genuinely honest.
4. **The name** — REDLINE (the red-pen/diff metaphor) vs REVISE (the user's word);
   trivially renamed before P1.
