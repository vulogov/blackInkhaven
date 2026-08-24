# Tutorial 111 — Continuity Intelligence

*Inkhaven 2.2 (SENTINEL)*

Tutorial 59 introduced continuity checking one detector at a time. SENTINEL makes
it **one always-watching concern**: a single engine unifies every deterministic
continuity detector, adds the one invariant nobody had — *an entity referenced
before it's introduced* — and watches incrementally as you write. It is
**advisory** (it flags, it never rewrites) and its core is **deterministic and
free** (no LLM).

## The unified check

```sh
inkhaven continuity check
```

```
⊗ [co_location] Mara is in Velmaril ("Council") and Ashford ("Ambush") at overlapping times.
⚠ [numeric] direction reversal: "five miles north" then "five miles south" (ch. 4)
⚠ [introduce] 'Aldous the ferryman' is referenced in ch. 2 but not introduced until ch. 5.

3 finding(s): 1 contradiction(s), 2 other.
```

One engine runs five deterministic detectors, normalises each into one finding
shape, dedupes, and ranks (`⊗` contradiction > `⚠` warning > `●` info; earlier
chapters first):

| Detector | The break it catches |
| -------- | -------------------- |
| `co_location` | a character in two places at overlapping times (from the timeline) |
| `timeline` | orphaned events · fuzzy-precision overlaps |
| `numeric` | a direction reversed or a duration that contradicts itself (EN/FR/ES/DE/RU) |
| `char_facts` | an established fact changed across chapters (over `continuity extract`) |
| `introduce` | **an entity named before it's introduced** |

Narrow it with `--only` / `--skip` (by detector name), machine-read it with
`--json`, and drop it into CI — the command **exits non-zero when any
Contradiction survives**:

```sh
inkhaven continuity check --only introduce --json
inkhaven continuity check --skip timeline        # e.g. before the timeline is built
```

## The invariant nobody had

An entity's *introduction* is its **first scene** — the earliest paragraph its
timeline events touch. A *reference* is any mention in the prose. When you name a
character chapters before they step on stage, `introduce` catches it:

> *'Aldous the ferryman' is referenced in ch. 2 but not introduced until ch. 5.*

Same-chapter foreshadowing never flags; raise `continuity.introduce_tolerance` to
allow a chapter or two of lead-in. Names come from your own Characters / Places
books and the match is Unicode-aware, so it works the same in Cyrillic or accented
scripts as in Latin.

## In the editor

- **The review pass** (`Ctrl+B Shift+C`) now includes a `continuity` line — each
  finding anchored so you can jump to it, in the Output pane's `continuity`
  category (filter to it there).
- **The ledger dashboard** (`Ctrl+B Shift+I`) — a scrollable modal of the ranked
  findings grouped by kind. `↑↓` to scroll, **Enter** jumps to a finding's
  paragraph, `Esc` closes.
- **The watch** — turn on `continuity.ambient` and every save re-checks only what
  the edit touched (the paragraph's entities and chapter, read from the graph) and
  shows the delta immediately. Deterministic and free, so it runs inline — the
  book watches itself as you write.

## When you want the model's eyes too

The deterministic detectors can't see a fact asserted then quietly reversed three
paragraphs later. The **coherence pass** can — an LLM reads a run of paragraphs
and flags the cross-paragraph contradictions. It is **explicit and cost-capped**,
never automatic:

```sh
inkhaven continuity check --coherence            # merged into the ledger
```

or press **`k`** in the ledger dashboard to run it over the open book (results
land in Output as `source: coherence`). The cost is previewed against your daily
cap before it runs — cost *informs*, it never blocks.

## Configure it

The `continuity:` block (all optional; defaults shown):

```hjson
continuity: {
  enabled: true              // the review-pass ledger master switch
  ambient: false             // re-check the edit's scope on every save
  ambient_cooldown_secs: 30
  co_location: true, timeline: true, numeric: true, char_facts: true, introduce: true
  introduce_tolerance: 0     // chapters of "referenced early" tolerated
}
```

Turning `enabled` off silences the review-pass line; the standalone `continuity
check` still runs (it's explicitly invoked).

## From a script

```
ink.continuity.check     ( -- dict )  { total, contradictions, warnings, info, by_kind }
ink.continuity.findings  ( -- list )  { kind, severity, chapter, source, message, entities }
```

Read-only — the coherence pass is not exposed to Bund.

---

SENTINEL doesn't replace your judgment; it makes sure nothing slips past it. The
full reference is [`CONTINUITY.md`](../CONTINUITY.md); the older detector-by-detector
walkthrough is [Tutorial 59](59-revision-and-continuity.md).
