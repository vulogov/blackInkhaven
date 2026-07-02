# Stability & ripple-effect bug scan — bugfixing plan (1.5.9-dev)

_A four-track parallel audit of the sources (gate state machine · new adapters/async · core infra/config ·
broad panic sweep). The tree is well-hardened overall — pervasive `saturating_sub`, char-based parsers,
`Disconnected` handling on every polled channel, no `.lock().unwrap()`. The findings below are the real,
verified defects, ranked; each has a location, trigger, root cause, and fix approach._

## Priority 0 — fix before the next release

### BUG-1 (CRITICAL) — Esc during an in-flight `/fact` gate leaks the receiver and contaminates the *next* confirmation
- **Where:** `src/research/app.rs` — `confirmation_key` Esc branch (clears `confirmation` only), the gate
  receivers `tri_gate` / `fc_confirm` / `refute_confirm`, and their pollers `poll_tri_gate` /
  `poll_fc_confirm` / `poll_refute`.
- **Trigger:** start a gated `/fact` (e.g. `refute_gate` on), press **Esc** while the LLM pass is in
  flight, then submit a *new* `/fact`. The stale gate resolves, its poller mutates the **new**
  `ConfirmationState` (`fc_checked=true`, overwrites `fc_verdict`/`fc_detail`, appends the *old* fact's
  verdict to `prov.detail`), and on a `SOUND`/`ACCURATE`/pass verdict calls `confirm_insertion()` →
  **inserts the new fact unvetted, gate skipped, with another fact's provenance written to disk.**
- **Root cause:** Esc/discard doesn't clear the in-flight gate receivers; pollers don't verify the
  confirmation they mutate is the *same* pending insertion; `is_busy()` is cosmetic (spinner-only) so a new
  extraction can start while a gate is live.
- **Fix:** (a) on Esc/discard in `confirmation_key`, clear `tri_gate`/`fc_confirm`/`refute_confirm` too;
  (b) belt-and-suspenders — stamp the pending insertion with a monotonic id and have each poller drop its
  result if the current `confirmation` id doesn't match; (c) block new extraction while a gate receiver is
  live (consult the gate states in `start_extraction_from`). (a)+(c) are the minimal fix.
- **Regression:** the *class* is pre-existing (`fc_confirm`/`tri_gate`), but the new **refutation gate**
  widened it to the common `model`/`document` fact path.
- **Test:** unit-drive `confirmation_key(Esc)` mid-gate, assert all three gate states are `None`; sim test
  that a stale `poll_refute` with a mismatched confirmation id inserts nothing.

### BUG-2 (HIGH) — `/upgrade` (and the `/fact` triangulation gate) raise a fact's tier on *non*-corroboration
- **Where:** `src/research/app.rs` — `pick_corroborator` and `summarize_triangulation` (shared weakness).
- **Trigger:** the judge emits `"Wikidata: NOT SUPPORTED — …"` / `"UNSUPPORTED"` / `"does not support"`,
  or uses a plain hyphen instead of the em-dash `—`. `head.contains("SUPPORT")` (after `split('—')`)
  then counts it as support → `/upgrade` records `origin=wikidata "corroborated (was model)"` with **no
  real corroboration**; the same mis-parse can pass the triangulation `/fact` gate.
- **Root cause:** substring matching on `SUPPORT`, and splitting only on em-dash `—` (U+2014) so a
  hyphenated reason becomes the classified head.
- **Fix:** parse the **leading verdict token** of each source line (word-boundary, not substring); treat
  `NOT SUPPORT*` / `UNSUPPORT*` / `does not support` as not-support; split the reason on both `—` **and**
  ` - `. Add a "negated support is not support" unit test to both fns.
- **Regression:** yes — `/upgrade` (R5-E) and the triangulation verdict parser.
- **Test:** `pick_corroborator("Wikidata: NOT SUPPORTED - x")` → `None`; hyphen-reason case → correct.

### BUG-3 (HIGH) — `highlight_line` panics on length-changing lowercase (in-chat search)
- **Where:** `src/research/render.rs` `highlight_line` (~613-623): byte offsets computed on
  `text.to_lowercase()` are used to slice the original `text`.
- **Trigger:** in-chat search (`Ctrl+F`) over a row containing a char whose lowercase changes byte length
  (`ẞ`→`ß`, `İ`→`i̇`) before a match — e.g. an all-caps German word. The diverged offset slices `text` at a
  non-char boundary → **panic in the render loop** (crash-recovery restores the terminal, but the session
  dies).
- **Root cause:** search and slice on two different strings.
- **Fix:** match and slice on the *same* buffer — build spans from the lowercased text's offsets against a
  char-aligned map, or do a char-index-based case-insensitive scan. (Editor search may share this — check
  `tui::search_replace` / `tui::echo_overlay` which the sweep found already char-based.)
- **Regression:** pre-existing (RESRCH-2.x search).
- **Test:** `highlight_line("STRAẞE …", "straße", …)` doesn't panic and highlights the match.

## Priority 1 — this-release cleanup (correctness/robustness)

### BUG-4 (MEDIUM) — `poll_upgrade` parses a *partial* judge buffer as a final verdict on stream error
- **Where:** `app.rs` `poll_upgrade` Judge phase — `Done`, `Error`, and `Disconnected` all `done=true` and
  feed `buf` to `pick_corroborator`. A mid-stream error after a `"…: SUPPORTS"` line but before a later
  `CONTRADICTS` can raise the tier.
- **Fix:** only run `pick_corroborator` on a clean `Done`; on `Error`/`Disconnected` fail safe (report
  "judge interrupted — not upgraded", no tier change).

### BUG-5 (MEDIUM) — orphaned "streaming" spinner when the `/upgrade` judge provider fails
- **Where:** `app.rs` `start_upgrade_judge` error paths (and `poll_upgrade` gather-disconnect): set
  `upgrade=None` but leave the preview `ChatTurn` `streaming=true` with an empty body → a spinner that
  never resolves.
- **Fix:** finalize/replace the preview turn (`streaming=false`, an error line) on every `/upgrade`
  termination path.

### BUG-6 (MEDIUM) — CLI ingest orphans embedded vectors on a mid-loop `add_document` failure
- **Where:** `app.rs` `gutenberg_cli` and `embed_source_file` (and pre-existing `import_one_file`) use
  `?`/`return Err` inside the chunk loop: earlier chunks are committed to the vector store, but the
  function returns before `imports_store.save()` → vectors not recorded in `research-sources.json`, so
  `/forget` can never remove them. The **TUI** paths (`ingest_gutenberg`, `web_ingest`) use the tolerant
  `if let Ok(id)` pattern and don't orphan.
- **Fix:** make the CLI loops match the tolerant TUI pattern (record successful ids, save the sidecar
  regardless), or on error still persist the partial `doc_ids` before returning.

## Priority 2 — hardening (low-severity panics & UX)

- **BUG-7 (LOW/MED)** — `web_ingest` same-slug collision within one batch silently drops the first source
  (BTreeMap key = URL path-stem slug). Fix: disambiguate the name on collision (append host/index).
- **BUG-8 (LOW/MED)** — `src/config_tui/widgets.rs` `parse_hex` slices `hex[0..2]` after only a byte-length
  gate → multibyte input (e.g. two `€`) panics. Fix: add `if !hex.is_ascii() { return None; }` (the sibling
  `config.rs::parse_color` already has this guard).
- **BUG-9 (LOW)** — `/gutenberg --chapter 0` ingests chapter 1 but labels it "chapter 0/N" (`saturating_sub`
  hides the off-by-one). Fix: reject/clamp `0` to `1` with the correct label, in both `ingest_gutenberg`
  and `gutenberg_cli`.
- **BUG-10 (LOW)** — `poll_gutenberg`/`poll_geonames`/`poll_wikidata` leave a stale "Querying…" status on
  `Disconnected` (task died without sending). Fix: set an error status on the `Disconnected` arm.
- **BUG-11 (LOW)** — `poll_tri_gate` Gather-phase `Disconnected` drops the gate with no verdict/status
  (inconsistent with the Judge-phase, which finalizes). Fix: finalize with a weak verdict like the other
  gates.
- **BUG-12 (LOW)** — `src/world/utopia/grounding.rs:88` `&p[..p.len().min(8)]` byte-slices a possibly
  model-derived `para_id`. Fix: `p.chars().take(8).collect()`.

## Priority 3 — note / defer (design-level)

- **BUG-13** — provenance/verdict sidecars use whole-map read-modify-write; two concurrent *processes* on
  one project (headless CLI while the TUI is open; the project lock is advisory) can lost-update. The
  individual writes **are** atomic (temp+fsync+rename) — no corruption, only a lost insert. Deferring: a
  real fix needs per-file locking or append-merge; document as a known single-writer assumption.

## Suggested execution order & batching
1. **P0 batch** (BUG-1, BUG-2, BUG-3) — one commit each, with the named tests. These are the
   trust/stability-critical ones; do them before cutting 1.5.9.
2. **P1 batch** (BUG-4/5/6) — `/upgrade` robustness (4+5 together) and the CLI-orphan fix (6) as a second
   commit.
3. **P2 batch** (BUG-7..12) — a single "hardening" commit (small, independent one-liners + tests).
4. **P3** — a tracking note in the release; no code this cut.

Every fix ships with a regression test; full `cargo test` gate after each batch. No behavior change to the
author-facing confirm flow beyond *not* corrupting it.
