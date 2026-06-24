# Tutorial 82 — Project health and review

*Inkhaven 1.3.32–1.3.34*

Three road-to-1.4.0 tools share one job: keep the project sound
without leaving the keyboard or waiting on a model. One scans the
data for structural rot, one runs every fast checker at once over
your prose and timeline, and one shows what the day's AI usage has
cost. None of them mutate your manuscript unless you ask.

## `inkhaven doctor --scan` — referential integrity

`inkhaven doctor` is a small family of unrelated checks. Its
project-integrity mode is the scan: it walks the whole project and
reports references that point at nothing, slugs that collide, and
duplicated system books — the kind of damage that accumulates from
crashes, half-finished imports, or hand-edited data.

```sh
$ inkhaven doctor --scan
doctor: scanning project…
  ⊗ BrokenParentRef        (Critical)  chapter 'Tides' → missing book 7c2f…
  ⚠ DanglingParagraphLink  (Warning)   paragraph 'scene 3' → 4 dead link(s)
  ⚠ DanglingEventRef       (Warning)   timeline event 'the duel' → missing paragraph
  ⚠ SiblingSlugCollision   (Warning)   two children share slug 'the-coast'
  ⚠ DuplicateSystemBook    (Warning)   2× system book 'Characters'

5 finding(s): 1 Critical, 4 Warning
```

The finding classes and their severities are fixed:

| Class | Severity | What it means |
|-------|----------|----------------|
| `BrokenParentRef` | Critical | A node names a parent that doesn't exist — the node is orphaned. |
| `DanglingParagraphLink` | Warning | A paragraph links to a target paragraph that's gone. |
| `DanglingEventRef` | Warning | A timeline event references a paragraph that no longer exists. |
| `SiblingSlugCollision` | Warning | Two siblings resolve to the same slug — on-disk paths collide. |
| `DuplicateSystemBook` | Warning | A system book (Characters, Places, …) exists more than once. |

### Filtering and machine-readable output

`--json` emits the findings as structured records instead of the
human table — for piping into a script or a test:

```sh
$ inkhaven doctor --scan --json | jq '.findings[].class' | sort | uniq -c
```

`--class <slug>` narrows the scan to a single finding class, which
is the quick way to confirm one specific repair landed:

```sh
$ inkhaven doctor --scan --class SiblingSlugCollision
doctor: scanning project…
  ⚠ SiblingSlugCollision   (Warning)   two children share slug 'the-coast'

1 finding(s): 1 Warning
```

### Fixing, and the CI exit code

`--autofix` applies the *safe* repairs and prompts before it
writes. Add `--yes` to skip that confirmation in a script:

```sh
$ inkhaven doctor --scan --autofix --yes
doctor: scanning project…
  fixed: DuplicateSystemBook — merged 2× 'Characters' into one
  fixed: SiblingSlugCollision — re-slugged 'the-coast' → 'the-coast-2'
2 finding(s) repaired
```

The scan's exit code is **2** whenever any finding at Warning
severity or above shipped — so a build can gate on project health:

```sh
$ inkhaven doctor --scan || echo "project has findings; failing build"
```

A clean project exits 0; anything Warning-or-worse exits 2. (Note
`doctor` also carries unrelated modes — `--voices` and `--tts-test`
for the speech subsystem. Those aren't the integrity scan; reach
for `--scan` when you mean project health.)

## The unified review pass — `Ctrl+B Shift+C`

The review pass runs every **FAST, deterministic** checker the
editor has, all at once, and posts the combined result to the
Output pane (Tutorial 75). It is **instant and LLM-free** — nothing
here calls a model, so you can run it on every save without
spending a token or waiting on the network.

Press **`Ctrl+B Shift+C`** in the editor. Three checkers fire:

- the **world fact-checker** over the **open paragraph**,
- **Inner Socrates** over the **open paragraph**,
- the **timeline critique** over the **whole project**.

Everything lands in the Output pane with a one-line summary:

```
┌─ Output · review ────────────────────────────────────┐
│  fact 2 · socrates 1 · timeline 3                     │
│ ⚠ fact: 'the northern sea' is described as frozen …   │
│ ● socrates: this paragraph asserts X but never shows… │
│ ⚠ timeline: 'the duel' precedes 'the journey' yet …   │
└───────────────────────────────────────────────────────┘
```

Because it all goes to the Output pane, the **`f` / `S` / `t`**
filters from Tutorial 75 slice the combined board down to just the
fact, Socrates, or timeline findings — handy when one checker is
noisy and you want to clear another class first.

### Report-card badges in the tree

The tree shows a per-node **report-card badge** aggregated from the
findings on that node's source paragraphs:

| Badge | Meaning |
|-------|---------|
| `⊗ N` | N critical-class findings under this node |
| `⚠ N` | N warnings under this node |
| `● N` | N informational findings under this node |

The count is live: as you **dismiss** findings in the Output pane,
the badge count drops. A node with no remaining findings shows no
badge.

### From the command line — `inkhaven check`

The same pass runs headlessly:

```sh
$ inkhaven check
fact 2 · socrates 1 · timeline 3
```

```sh
$ inkhaven check --paragraph 9f3a… --no-timeline
fact 1 · socrates 0
```

| Flag | What |
|------|------|
| `--paragraph <id>` | Run the paragraph-scoped checkers on this paragraph instead of the open one. |
| `--book-name <name>` | Scope the run to a named book. |
| `--no-fact` | Skip the world fact-checker. |
| `--no-socrates` | Skip Inner Socrates. |
| `--no-timeline` | Skip the project timeline critique. |

## The AI cost dashboard — `Ctrl+B $`

Press **`Ctrl+B $`** for one view of today's LLM usage. It shows
the two **capped daily budgets** — the world slow-track and the
Inner Socrates slow-track — each with a usage bar, then the **other
AI calls today** grouped by category, then a total.

```
┌─ AI cost · today ────────────────────────────────────┐
│  world slow-track      ███████░░░  142 / 200          │
│  Inner Socrates s-t    █████░░░░░   88 / 150          │
│                                                       │
│  other AI calls today                                 │
│    chat          37                                   │
│    grammar       12                                   │
│    explain        8                                   │
│    critique       5                                   │
│    continuation   4                                   │
│    translation    3                                   │
│                                                       │
│  total: 299 calls                                     │
└───────────────────────────────────────────────────────┘
```

The "other" categories — chat / grammar / explain / critique /
continuation / translation / … — are tracked at the single
inference chokepoint, so every model call is counted exactly once
regardless of which feature made it.

**The caps are informative, not gates.** Past a budget the slow
tracks **warn and continue** — Inkhaven's permissive principle:
the dashboard tells you you've gone over, it never blocks the work.

The same numbers print from the CLI:

```sh
$ inkhaven cost
world slow-track     142 / 200
Inner Socrates s-t    88 / 150
other: chat 37 · grammar 12 · explain 8 · critique 5 · …
total: 299
```

### Configuring caps and retention

Caps and history live in the `cost:` HJSON block:

```hjson
cost: {
  world_daily_call_cap: 200          // default 200
  inner_socrates_daily_call_cap: 150 // default 150
  usage_retention_days: 30           // default 30
}
```

The "day" the dashboard counts against resets per
`goals.day_boundary` — `utc` by default, or set it to `local` to
roll over on your own midnight.

## See also

- [`75-the-output-pane.md`](75-the-output-pane.md) — where the
  review pass posts its findings, and the `f` / `S` / `t` filters
  that slice them.
- [`24-typst-diagnostics.md`](24-typst-diagnostics.md) — `doctor`'s
  other face: the Typst-compilation diagnostics, separate from the
  referential-integrity scan covered here.
