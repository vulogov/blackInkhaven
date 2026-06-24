# 81 — Migrating the timeline critique

*1.3.31+ · RFC TIMELINE-2-INTEGRATION*

If you used the timeline's AI health critique (the `y` / `Y` /
`Ctrl+Y` / `F12` chords) before 1.3.31, this is what changed and
how to move over. If you're new to Inkhaven, you don't need this
tutorial — the critique just works the way
[Tutorial 31](31-story-timeline.md) describes.

## What changed, in one paragraph

The old critique sent every event to the LLM with a five-item
audit checklist and streamed prose back into the AI pane. Four of
those five items now have a better home, built since the timeline
critique shipped:

| Legacy item | Now done by | Command |
|---|---|---|
| Travel-time conflicts | world fact-checker `travel_time` | `inkhaven realworld fact-check --timeline-aware` |
| Co-location conflicts | world fact-checker `co_location` | `inkhaven realworld co-location` |
| Paragraph-date mismatches | world fact-checker `date_coherence` | `inkhaven realworld fact-check --timeline-aware` |
| Pacing (gaps / rushes) | INNER_SOCRATES `temporal_density` | `inkhaven inner-socrates check` |

The fifth and sixth — **orphan events** and **fuzzy-precision
overlaps** — are genuinely timeline-internal (no other system
understands precision windows or event-link coherence), so they
*stay*, now stronger and structured. They emit to the **Output
pane** like every other finding, instead of streaming prose.

Nothing about your timeline **data** changed: events, dates,
precision, tracks, links, the `event add/list/show` CLI, the
`Ctrl+V e` picker and swim-lane view — all identical.

## Step 1 — see your coverage

Run the migration check against your project:

```
inkhaven event critique --migration-check
```

It runs the two retained checks live (counting your orphans and
overlaps) and prints where the four removed categories now live,
with the command for each. It deliberately does **not** invent
counts for the migrated categories — those depend on a world
definition / prose pass the timeline critique no longer performs.
Run the listed commands to see those findings.

## Step 2 — see what moved

```
inkhaven event critique --diff
```

shows each legacy category and the command that now owns it, plus
what the refactor made *stronger*:

- **Orphans** are graded by significance × staleness (the legacy
  audit treated every orphan the same).
- **Overlaps** detect multi-event **clusters** (the legacy audit
  only ever flagged pairs).

## Step 3 — use the new critique

From the TUI, the chords are unchanged — `y` / `Y` / `Ctrl+Y` /
`F12` — but findings now appear in the Output pane (`Ctrl+B Tab`).
From the CLI:

```
inkhaven event critique                 # whole project
inkhaven event critique --track main    # one track
inkhaven event critique --book-name "Velmaron"
inkhaven event critique --no-elaborate  # pattern-only, no LLM
```

Each orphan (`⊘`) and overlap (`⧉`) prints with a severity icon,
the pattern-detected reason, and — when an LLM is configured and
`timeline.critique.elaboration.enabled` is set — a short `↳`
elaboration explaining why it's worth a look.

## Tuning

`inkhaven.hjson`, under `timeline.critique`:

```hjson
timeline: {
  critique: {
    enabled: true
    orphan: {
      enabled: true
      min_orphan_age_days: 0          // emit immediately
      min_significance: "low"         // 'low' | 'moderate' | 'high'
    }
    fuzzy_overlap: {
      enabled: true
      min_suspicion: "moderate"       // 'low' | 'moderate' | 'high'
      cluster_min_size: 3
    }
    elaboration: {
      enabled: true                   // LLM elaboration when a provider exists
      max_calls_per_run: 20
      confirm_above_calls: 10
    }
  }
}
```

Too many orphan notes? Raise `orphan.min_significance` to
`"moderate"`. Want broader overlap coverage? Drop
`fuzzy_overlap.min_suspicion` to `"low"`. Saving cost? Set
`elaboration.enabled: false` (or pass `--no-elaborate`).

## If you really need the old behaviour

```
inkhaven event critique --legacy
```

runs the original five-item AI audit, one LLM call, prose to
stdout. It prints a deprecation notice and **will be removed in a
later release** — it's a transition crutch, not a destination. If
the new infrastructure misses something you relied on, the
`--diff` output names where each category went; tune that system
(e.g. lower `inner_socrates` severity for more pacing coverage)
rather than leaning on `--legacy`.

## Scripting

The retained checks are also Bund words (read-only):

```bund
ink.event.critique.run    ( -- dict )   { orphans overlaps total }
ink.event.critique.config ( -- dict )   the active thresholds
```

See [Tutorial 31](31-story-timeline.md) for the full word list.

---

*Related: [31 — The story timeline](31-story-timeline.md) ·
[80 — Timeline-aware fact-checking](80-timeline-aware-fact-checking.md).*
