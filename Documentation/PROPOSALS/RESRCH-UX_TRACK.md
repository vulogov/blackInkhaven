# RESRCH-UX — Research Assistant UI experience (track proposal)

| | |
|---|---|
| **Status** | **UX-P1..P5 shipped** (P1–P4 in 1.5.6; P5 layout/readability/quick-view + light markdown in 1.5.7-dev) — track complete |
| **Builds on** | RESRCH-1..4 (the Research Assistant TUI, `inkhaven research`) |
| **Theme** | The assistant grew a lot of *capability* (~24 `/commands`, five source tiers, a trust ladder, triangulation gate) but the **UI didn't keep pace**. The features are powerful yet **undiscoverable and invisible**. This track makes the capability *legible* — no new research capability, pure experience. |

## The problem, concretely

- **Discoverability.** ~24 slash-commands exist; nothing surfaces them. `Tab` completes *paths* only
  (`try_path_completion`, `app.rs`). A new user can't find `/wikidata`, `/triangulate`, `/world`.
- **The trust ladder is invisible.** Provenance (`computed`/`simulation`/`wikidata`/`openalex`/`arxiv`/
  `web`/`document`/`model`) drives the whole design but the user never sees a fact's tier until
  `/sources`. The `ChatTurn` already carries `sources` / `wikidata` / `paper` / `simulation` / `computed`
  / `world_detail` — the data is there, just unrendered.
- **Async feels dead.** `/web`, `/wikidata`, `/openalex`, `/triangulate` show a static "Querying…" with
  no spinner, timer, or liveness.
- **Spartan confirmation overlay.** The most important modal (nothing enters the corpus without it) shows
  a terse `dup_warning` / `fc_verdict` status line, not the actual evidence.

## Grounding (verified primitives)

- Chat rows render via `RowKind` (Header/Prompt/Response/Plain) with themed colours (`render.rs`); adding
  a styled badge `Span` per turn is a rendering-only change.
- The Facts tree already renders per-node glyphs with themed colour (`verdict_style` + the `/factcheck`
  ✓/?/✗ path) — a provenance-tier glyph reuses that exact mechanism.
- The prompt is a `tui_textarea::TextArea`; `Tab` is intercepted in `on_key` and already special-cased
  (`try_path_completion`) — a command palette hooks the same point.
- Async ops each have a poll state (`web` / `wikidata_state` / `scholarly_state` / `triangulate` /
  `tri_gate`); a spinner reads "is any Some?" + a tick counter.
- The hints bar (`show_keybind_hints`, `render_hints`) is static today — it can become input-aware.
- `split_ratio` exists but is config-only; the writing TUI already does live split resize.

## Phases

Each is shippable on its own; all are rendering / input changes with **no new crates**.

| Phase | Content |
|---|---|
| **UX-P1 — Command palette + live hints** | When the prompt begins with `/` and the cursor is in the command word, `Tab` (and/or a live overlay) offers a **filtered list of commands** with one-line descriptions + arg hints; arrow-select, narrow-as-you-type, Enter completes. Path completion (existing) still fires once past the command word. The **hints bar** becomes context-sensitive: typing `/web ` shows `/web [--ingest|--chat] <query>`. A `commands()` table (name → summary → usage) is the single source, also feeding `Ctrl+B h`. The biggest single win. |
| **UX-P2 — Trust made visible** | A **source-tier badge** on each chat turn — a themed `Span` derived from the turn's provenance fields (`[computed]` `[◆ Q937]` `[§ arxiv]` `[⚠ web]` `[? model]`). A **permanent trust glyph** in the Facts tree by each fact's recorded provenance origin (reuse the verdict-glyph render + the provenance sidecar), so the trust ladder is legible at a glance. |
| **UX-P3 — Async liveness** | A **braille spinner + elapsed seconds** in the status bar whenever any async op is in flight (`poll_*` states), naming what's running and that `Esc` cancels. Background ops (folder re-import on launch, `--batch`) emit a transient line instead of silence. |
| **UX-P4 — Richer confirmation overlay** | Render the **triangulation / fact-check verdict** in the overlay (per-source `SUPPORTS`/`CONTRADICTS`/`SILENT`, coloured) instead of a status flash; show the **near-duplicate's text** beside the pending fact when the dedup guard fires; show the exact **provenance tier + citation** that will be recorded, with field labels + a Title/Body active-field cue. **✅ Shipped 1.5.6** — `ConfirmationState.fc_detail`/`dup_body`; an evidence panel splits the overlay when a verdict/dup is present (`evidence_line_style` colours each line); the location row shows `will record: <origin> · <detail>`; Title/Body labels carry char counts. |
| **UX-P5 — Layout & readability** | Live **split resize** (widen/narrow tree vs chat) + **pane zoom** (full-screen chat or tree); light **markdown** styling in responses (bold/bullets/code via `Span`); **turn separators** and a **"▼ N more"** scroll affordance; a **yank** key for the last response / selected fact. **✅ Shipped 1.5.7-dev** — `<`/`>` resize (pane-scoped; `Ctrl+←/→` is macOS Mission Control), `Ctrl+Z` pane zoom (`zoom: Option<Focus>`), turn separators + `▼ more` indicator, `y`/`Y` yank via the existing `arboard`, plus **`Enter` fact quick-view** (`PeekState` modal) and **light markdown** in responses (reuses the editor's `highlight_markdown_lines` per response line; markers are preserved so wrap-width — and the scroll math — is unchanged; plain runs fall back to `pane_fg`). |

## Recommended first cut (for 1.5.6) — **shipped**

**UX-P1 (command completion + hints) + UX-P2 (trust badges/glyphs) + UX-P3 (async spinner).** Together
they fix the three real problems — *can't find features*, *can't see grounding*, *feels frozen* — and all
three reuse existing state with no new dependencies.

- **UX-P1 ✅** — `command::SPECS` (the single source, also feeds `Ctrl+B h`) + `hint_for`; Tab completes a
  `/command` in the command word (`try_command_completion`, before path completion); the hints bar is
  input-aware.
- **UX-P2 ✅** — `turn_badge` renders a tier badge on each chat header; `provenance_tier_glyph` +
  `fact_provenance` (reloaded on `reload_hierarchy`) render a permanent tier glyph per fact in the tree.
- **UX-P3 ✅** — `is_busy()` + `spin_tick`/`async_started` drive a braille spinner + elapsed timer in the
  status bar.

UX-P4/P5 follow as polish.

## Out of scope
- Mouse interaction (research mode is keyboard-only by design).
- New research *capability* (that's RESRCH-5/6 — synthesis, maintenance, deep research).
- Full rich-text / image rendering in the chat.

## Tests
- `commands()` table completeness (every parsed `/command` has an entry); palette filter + splice
  (token extraction after `/`, LCP, ambiguity list) — unit-tested like the path completion.
- Badge/glyph mapping (provenance origin → tier label + colour) — pure, unit-tested.
- Spinner/elapsed formatting; hints-for-input — pure helpers, unit-tested. (Rendering integration-tested.)
