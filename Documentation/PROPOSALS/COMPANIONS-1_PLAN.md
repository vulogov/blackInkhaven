# COMPANIONS-1 — unify the examined-authorship companions (1.4.4)

The examined-authorship triad is complete — **WORLD** (consistency), **Inner
Socrates** (structure), **Inner Editor** (craft) — but they're three *parallel*
features. 1.4.4 makes them feel like one system: the Editor joins the unified
review pass, its intent loop closes (promotion-from-dismissal), and a single
**companions** surface shows all their findings at once. Almost entirely reuse;
no new dependencies.

## Grounding (verified against the 1.4.4-dev tree)

| Assumption | Reality | Consequence |
|---|---|---|
| "Add Editor to the tree badges" | `compute_tree_badges` is **kind-agnostic** — it counts every Output message by `source_paragraph_id` | Editor findings **already** badge the tree. Verify + test + document; no new code. |
| "Add Editor to `inkhaven check`" | `check` / `run_unified_check` run the **Fast track** (deterministic fact-check + Socratic + timeline) — instant, LLM-free | Editor is **LLM-only**; it must join **asynchronously** (RunCheck spawns the engage as a bg job) and **opt-in** on the CLI (`--editor`), preserving the instant/free pass. |
| "Reuse Socrates' promotion" | `record_socratic_dismissal` (on `d`) → `InnerSocratesStore::record_dismissal(category, chapter)` → `promotion_candidates(5)`; `socratic_dismissals` table is typed to Socratic `Category` | Editor needs its **own** `editor_dismissals` table (string category ids) + a parallel `promotion_candidates` + the `d`-key hook for `inner_editor_observation` rows. |
| "One companions filter" | the Output filter cycles **individual** sources (`fact-check` / `socrates` / `inner-editor` / `timeline-critique` / …) | Add a **`companions`** meta-source (matches all examined-authorship kinds) + a cockpit report. |

Background-runner caveat (carried from 1.4.3): there is **one** `bg_job` slot.
RunCheck spawning the Editor engage must respect it (skip with a clear note if a
job is already running) — concurrent bg jobs are a separate theme (deferred).

## Locked direction

Theme 1 — **unify the companions**. Single release (1.4.4). Reuse the existing
Output envelope, intent ledger, cost store, and check pass; the only new storage
is the `editor_dismissals` tally.

## Phase map

- **CU-P0 — Editor in the review pass.** `run_unified_check` (Ctrl+B Shift+C)
  spawns the Inner Editor engage on the **open paragraph** as a background job
  (alongside the instant fact/socrates/timeline checkers), guarded on the single
  bg slot + `inner_editor.enabled`; the "clean" message waits if the Editor is
  still running, and its findings re-badge the tree on completion. Tree badges
  already include the Editor (kind-agnostic) — pinned by a test.
  *CLI `check --editor` is deferred:* the CLI `check` runs project-**wide**
  (every paragraph), so engaging the LLM Editor there would be N calls — wrong;
  the per-paragraph `inkhaven inner-editor engage` already covers targeted CLI
  use.
- **CU-P1 — Editor promotion-from-dismissal.** `editor_dismissals` table on
  `InnerEditorStore` (string category + chapter id) + `record_dismissal` /
  `promotion_candidates`; hook the Output `d` key for `inner_editor_observation`
  rows (mirrors `record_socratic_dismissal`); surface candidates via
  `inkhaven inner-editor suggestions list/promote/dismiss` (promote reuses
  `intent_declare`) + a status nudge when a pattern crosses the threshold.
- **CU-P2 — the companions surface.** A `companions` Output meta-source (the `f`
  filter gains a "companions" stop that matches fact-check + socrates +
  inner-editor + timeline) and a cockpit: `inkhaven companions` (terminal report)
  — today's findings by companion + severity + cost, reusing the cost gather and
  the per-feature stores. (TUI: surface it under an existing chord; no new modal
  if avoidable.)
- **CU-P3 — polish + docs.** CONFIGURATION notes, a short tutorial / update to
  the existing ones, edge tests. → **cut 1.4.4**.

## Non-goals (1.4.4)

- Concurrent background jobs / a job queue (separate hardening theme).
- A new Inner-family member (Inner Sentinel — a future RFC).
- Editor editable-settings overlay, findings export (Editor polish leftovers —
  fold in only if cheap).
- Any change to WORLD / Inner Socrates surfaces beyond the additive
  `editor_dismissals` parallel and the shared filter.
