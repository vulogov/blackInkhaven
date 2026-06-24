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
- **Extensibility** — _done._ Prepared the dashboard for more analytical threads:
  `InnerSocratesStore::llm_usage_today(day)` returns **all** recorded `(sub_budget,
  calls)` pairs, and `gather` enumerates them — the canonical slow track plus any
  other thread's sub-budget — so a new analytical thread appears automatically once
  it records via `record_llm_call(day, "<key>")`. `CostEntry.name` is now owned.
- **P1 — TUI panel.** _Done._ `Action::OpenCostDashboard` bound to **`Ctrl+B $`**
  (label "AI cost"; self-lists in the palette + quickref) → `Modal::CostDashboard`,
  a scrollable read-only panel (`draw_cost_dashboard_modal`) computing the report on
  render via the shared `cli::cost` aggregator. Binding resolve-tested. Full suite
  1801 → 1802.
- **P2 — docs + cut.** KEYBINDING / quickref rows; then cut the bundled release.

- **P3 — track every AI call + informative caps.** _Done._ Per Vladimir's
  permissive principle (inkhaven informs, doesn't gate, except for security): the
  daily slow-track caps now **warn and continue** instead of erroring
  (`DailyCapReached` → eprintln + proceed in both preflights). A new `ai::usage`
  tracker (global install + `record(category)` → `.inkhaven/ai_usage.json`, 30-day
  prune) records **every** inference by category at the `spawn_chat_stream`
  chokepoint (22 call sites tagged: chat / grammar / explain / critique /
  continuation / …; `collect_blocking` opts out so the capped slow tracks aren't
  double-counted). The dashboard now shows **daily budgets (informative)** + **other
  AI calls today** (per-category counts) + total, with explicit "informative, not
  limits · resets 00:00 UTC" framing. 4 tests. Full suite 1802 → 1804.

## Non-goals

No spend *enforcement* — caps inform, never block (the permissive principle). No
token accounting beyond call counts, no new deps. The dashboard is a read-only
aggregation; the only behavioural change is making the previously-blocking daily cap
a warning.

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
