# Unified AI cost dashboard (road to 1.4.0)

Bundled into the 1.3.34 cycle. The LLM-using subsystems each track their own daily
call budget in their own store; there's no single place to see AI spend. This adds
one — `inkhaven cost` + a TUI panel.

## What's tracked today

| Budget | Store method | Daily cap |
|---|---|---|
| World slow track (fact-check) | `WorldStore::llm_calls_today(day)` | 200 |
| Inner Socrates slow track | `InnerSocratesStore::llm_calls_today(day, "slow_track")` | 150 |
| Timeline critique elaboration | per-run `ElaborationBudget` (not persisted) | (per run) |

The two daily caps are currently function-local `const`s in the CLI handlers. To
keep the dashboard from drifting, they become shared `pub const`s on the stores,
referenced by both the handlers and the dashboard.

## Phases

- **P0 — cost core + `inkhaven cost` CLI.** Promote the caps to shared consts.
  `src/cli/cost.rs`: `CostEntry { name, calls_today, daily_cap }`, `CostReport`,
  `gather(project)` (reads both stores, gracefully zero when absent), a formatter
  (per-budget bar + today's totals + the per-run elaboration note), and the `Cost`
  command. Tested. **(this increment)**
- **P1 — TUI panel.** `Action::OpenCostDashboard` (self-lists in the palette /
  quickref) → a scrollable modal showing the same report. Reachable by a chord.
- **P2 — docs + cut.** KEYBINDING / quickref rows; then cut the bundled release.

## Non-goals

No new spend *enforcement* (the existing preflights/caps are unchanged), no token
accounting beyond what the stores record (call counts), no new deps. The dashboard
is a read-only aggregation.

## Increment log

- **P0** — _done._ Promoted the daily caps to shared consts (`WorldStore::DAILY_CALL_CAP`
  = 200; `InnerSocratesStore::DAILY_CALL_CAP` = 150 + `SLOW_SUB_BUDGET`), now
  referenced by both the CLI slow-track preflights and the dashboard (no drift).
  `src/cli/cost.rs`: `CostEntry` / `CostReport` / `gather(project, day)` (reads both
  stores, zero when absent) / `render_lines` (per-budget bar + % + totals + the
  per-run elaboration note) / `bar`. `Command::Cost` → `inkhaven cost`. Over-cap
  shows a full bar with the true (>100%) percentage. 2 unit tests + a live smoke.
  Full suite 1799 → 1801.
</content>
