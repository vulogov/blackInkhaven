# `inkhaven check` — the unified review pass (road to 1.4.0)

The consolidation centrepiece ([ROADMAP-1.4.0.md](ROADMAP-1.4.0.md)). Today each
checker is a separate command/chord: `realworld fact-check`, `inner-socrates check`,
`event critique`. `inkhaven check` runs **every applicable fast/deterministic
checker over one scope** in a single gesture and reports a consolidated summary —
the "examined authorship" promise as one pass.

## The three checkers (all reuse existing per-scope entry points)

| Checker | Function | Scope | Gate |
|---|---|---|---|
| **fact-check** (WORLD-4/5 fast) | `world::fact_check::check_paragraph(text, ledger, roles, ctx)` | per paragraph | `world.hjson` exists |
| **socrates** (INNER_SOCRATES fast) | `inner_socrates::fast::check_paragraph(text, persona, ledger, ctx)` | per paragraph | always (deterministic, no LLM) |
| **timeline-critique** (TIMELINE-2) | `timeline::critique::run(events, …)` | whole project | `timeline.enabled` |

The fast tracks are deterministic and LLM-free, so a default `check` is instant and
free. (A `--slow` opt-in for the LLM tracks is a later enhancement.)

## Phases

- **P0 — the CLI `inkhaven check`.** `src/cli/check.rs` + a `Check` command. Resolve
  scope (`--paragraph` / `--book` / default project) → paragraphs; build each
  checker's context once (skip the gated ones cleanly); run fact-check + socrates
  per paragraph and the timeline critique once; print findings grouped by checker +
  a **summary table** (per-checker counts, paragraphs scanned). Each native finding
  also emits to the Output store (a no-op headless), so a future `--watch`/TUI path
  is free. **(this increment)**
- **P1 — the TUI chord.** _Done._ `Action::RunCheck` bound to **`Ctrl+B Shift+C`**
  (label "review pass"; self-lists in the palette + quickref). `run_unified_check`
  runs fact-check + Inner Socrates over the **open paragraph** (reusing
  `build_fact_check_context` / `collect_socratic_findings` / `persist_and_emit_socratic`)
  and the timeline critique over the **project** (new `collect_and_emit_timeline_critique`,
  which clears prior critique findings then re-emits), all into the Output pane, then
  surfaces it with a `fact N · socrates N · timeline N` status summary. Binding
  resolve-tested. Full suite 1796 → 1797.
- **P2 — tree report-card badges.** _Done._ Each tree node shows a badge
  (`⊗`/`⚠`/`●` + count) for the open Output findings under it — `compute_tree_badges`
  walks up from each finding's source paragraph, tallying a count + worst severity
  onto the paragraph and every ancestor (so a chapter/book shows the subtree sum,
  colored by its worst). Cached in `tree_badges`, refreshed on a ~900 ms throttle
  (`tick_tree_badges`) to avoid per-frame DB queries, and force-refreshed right
  after a review pass + on dismiss. Rendered as a trailing pip in `tree_row_lines`.
  2 aggregation unit tests. Full suite 1797 → 1799.
- **P3 — stability + docs.** _Done._ Warning-clean build (the orchestration's pure
  pieces — scope helpers, badge aggregation, binding resolve — are unit-tested in
  P0–P2). Docs: KEYBINDING §1.1 gains an **"Any pane"** meta table with `Ctrl+B Shift+C`
  (review pass + the `inkhaven check` CLI equivalent + the tree-badge note); the
  quickref global section lists the review pass. Full suite stable (1799).

## Design notes

- **Normalized finding.** The three native finding types differ; the orchestration
  collects a `CheckFinding { checker, severity, category, body, paragraph }` for the
  summary + CLI display, *and* emits the native finding (so the Output pane keeps
  full per-kind metadata + actions). Dual collect+emit.
- **No new subsystem.** `check` is pure orchestration over existing checkers; no new
  checker logic, no schema change, no new deps.

## Increment log

- **P0** — _done._ `src/cli/check.rs` + `Command::Check` (`--paragraph` / `--book-name`
  / `--no-fact` / `--no-socrates` / `--no-timeline`). Scope resolution (one paragraph,
  or every non-event prose paragraph under a book / all user books — system books
  skipped via `node_under_user_book`). Per-paragraph fact-check + socrates; project-wide
  timeline critique once; each native finding emits to the Output store (no-op headless)
  + collects a normalized `CheckFinding`. Prints findings grouped + a per-checker summary
  table. Extracted `realworld::build_world_context` as the reusable world-context helper.
  Smoke-tested end to end (socratic `modal_claims` caught, summary table, gating). 1 unit
  test for the scope helpers. Full suite 1795 → 1796.
</content>
