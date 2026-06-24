# Writing Goals — surfacing & extending the existing engine

Road to 1.4.0, cycle **1.3.35**. Bundles three increments.

## Premise

Writing-goals is **~90% already built** and must not be reinvented:

- **Config** (`GoalsConfig`): `daily_words`, `active_minutes_daily`,
  `streak_grace_per_week`, per-book `{target_words, deadline}`,
  `status_ladder`, `auto_promote_on_target`; plus `ProjectConfig`
  `word_count_goal` + `target_date`.
- **Engine** (`src/progress/`): append-only `writing_events` /
  `writing_baselines` in `progress.db`; `build_snapshot` →
  `ProgressSnapshot` (today/total words, daily goal, per-book required
  pace + days-to-deadline, `compute_streak` with rolling-7-day grace,
  status-ladder, 30-day sparkline, active seconds today/week).
- **Surfaces**: `Ctrl+V g` progress modal, `Ctrl+V Shift+G` goal
  projection, `Ctrl+B Shift+G` heatmap, status-bar `today X/goal`,
  goal-hit / streak-break Bund hooks, per-paragraph targets + auto-promote.

Three genuine gaps remain. This plan closes all three.

## Increments

### P1 — `inkhaven goals` CLI  *(the missing terminal surface)*
Every other analytic — `cost`, `check`, `stats`, `concordance` — has a
terminal counterpart; goals are TUI-only. Add `src/cli/goals.rs`
mirroring `cost::run`: load layout/config/store/hierarchy, walk paragraphs
for `LiveTotals` (user books only, same scope rules as `stats`), open
`progress.db`, call `build_snapshot`, render the snapshot as plain text
(project today/total vs daily goal · streak w/ grace · per-book pace +
deadline · status-ladder week · active time · 30-day sparkline). Read-only.

### P2 — persisted all-time best streak + milestones
Today's "longest" is only the longest run **inside the 60-day scan
window** — it forgets. Persist a lifetime best in `progress.db`
(`writing_days(days_back)` already exists, flagged for exactly this) and
recognise milestone crossings (7 / 30 / 100 / 365). Surface best-vs-current
in the snapshot (CLI + modal); fire a Bund hook on a new milestone, in the
same informative spirit as the existing `on_goal_hit`.

### P3 — in-app goal editing → writes back to `inkhaven.hjson`
A modal to set `daily_words` / `streak_grace_per_week` / per-book targets
without hand-editing HJSON. **Constraint:** `Config::save` full-rewrites
HJSON and drops comments — unacceptable for a hand-tuned config. P3 must
patch *only* the `goals.*` subtree, preserve every other key + comment, and
take a versioned backup before writing (permissive principle: no
destructive write without a safety net / clean consent).

## Cut criteria
All three increments land signed, each with tests; `KEYBINDING.md` +
quickref updated for P3's modal; release notes fold into the 1.3.35 cut.
