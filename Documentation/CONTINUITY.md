# Continuity Intelligence (SENTINEL)

*(2.2, RFC SENTINEL-1 — see [`PROPOSALS/SENTINEL-1_PLAN.md`](PROPOSALS/SENTINEL-1_PLAN.md)
and [`PROPOSALS/SENTINEL-1_IMPL.md`](PROPOSALS/SENTINEL-1_IMPL.md))*

Inkhaven could already *check* a manuscript's continuity — but only if you knew
which of six separate commands to run, each with its own sidecar and mental model.
SENTINEL is the layer that makes continuity **one always-watching concern**:

> **Continuity already exists in pieces. SENTINEL unifies them over the knowledge
> graph, adds the one invariant nobody had, and watches incrementally as you write.**

It is **advisory** (it flags, it never rewrites), its core is **deterministic and
free** (no LLM), and it adds **no new runtime dependencies**.

---

## What it detects

One engine runs every deterministic continuity detector already in the tree,
normalises each into a single finding shape, dedupes, and ranks (Contradiction >
Warning > Info; earlier chapters first):

| Detector | Break it catches | Source |
| -------- | ---------------- | ------ |
| `co_location` | a character in two places at overlapping times | the timeline (magic-ledger suppressed) |
| `timeline` | orphaned events · fuzzy-precision overlaps | the timeline critique |
| `numeric` | direction reversal · conflicting durations | prose quantities (EN/FR/ES) |
| `char_facts` | an established fact changed across chapters | `.inkhaven/continuity.json` |
| `introduce` | **an entity referenced before it's introduced** | the graph + prose mentions |

The **referenced-before-introduced** invariant is the one nobody had. An entity's
*introduction* is its first scene (the earliest paragraph its timeline events
touch); a *reference* is any mention in the prose. When the first reference lands
in an earlier chapter than the introduction — by more than a tolerance — SENTINEL
flags it: *"'Aldous the ferryman' is referenced in ch. 2 but not introduced until
ch. 5."*

The fuzzy detectors (LLM drift, coherence) are **not** in the deterministic sweep —
they stay explicit and cost-capped (see [Coherence](#the-slow-coherence-pass)).

---

## The command line

```
inkhaven continuity check [--only DETECTOR]… [--skip DETECTOR]… [--json]
                          [--coherence [--max-cost 8000] [--force]]
```

Runs the unified ledger and prints the ranked findings (`⊗` contradiction, `⚠`
warning, `●` info). `--only` / `--skip` name detectors (`co_location`, `timeline`,
`numeric`, `char_facts`, `introduce`); `--json` emits a machine-readable array.
The command **exits non-zero when any Contradiction survives**, so it drops into
CI. `--coherence` also runs the LLM pass (below).

```
inkhaven continuity extract    # the AI fact-extraction pass (feeds char_facts)
inkhaven continuity list       # dump the extracted continuity bible
```

---

## In the editor

- **The review pass** (`Ctrl+B Shift+C`) includes a `continuity` line — the
  deterministic ledger, minus the timeline critique (which has its own line), each
  finding anchored so you can jump to it. Findings land in the Output pane under
  the `continuity` source (filter to it there).
- **The ledger dashboard** (`Ctrl+B Shift+I`) — a scrollable modal of the ranked
  findings grouped by kind. `↑↓` to scroll, **Enter** to jump to a finding's
  paragraph, **`k`** to run the slow coherence pass, `Esc` to close.
- **The watch** — with `continuity.ambient` on, every save re-checks only what the
  edit touched (the paragraph's entities + chapter, read from the graph) and
  surfaces the delta immediately. Deterministic and free, so it runs inline.

---

## The slow coherence pass

The one fuzzy check SENTINEL will *invoke* but never run automatically: an LLM
reads a run of paragraphs and flags cross-paragraph contradictions the
deterministic detectors can't see — a fact asserted then quietly reversed, a
time-of-day that can't follow. It is **explicit, cost-capped, opt-in**:

- CLI: `inkhaven continuity check --coherence [--max-cost 8000] [--force]`.
- Editor: `k` in the ledger dashboard, over the open book (results land in Output
  as `source: coherence`).

It respects the `magic:` ledger's declared exceptions and needs a configured LLM
provider; the cost is previewed against your daily cap before it runs (cost
*informs*, it never blocks).

---

## Configuration

The `continuity:` block (all fields optional; the shown values are the defaults):

```hjson
continuity: {
  enabled: true              // the review-pass ledger master switch
  ambient: false             // re-check the edit's scope on every save
  ambient_cooldown_secs: 30  // throttle floor for the ambient watch
  co_location: true          // per-detector toggles
  timeline: true
  numeric: true
  char_facts: true
  introduce: true
  introduce_tolerance: 0     // chapters of "referenced early" tolerated (0 = strict)
}
```

Turning `enabled` off silences the review-pass line; the standalone `inkhaven
continuity check` command still runs (it's explicitly invoked). The existing
`timeline.critique` and `editor.echo_*` knobs are untouched — the engine reads
them where relevant; this block only adds.

---

## Bund

Read the ledger from a script or hook (deterministic, read-only — the coherence
pass is not exposed to Bund):

```
ink.continuity.findings  ( -- list )  the ranked, deduped findings as dicts
                                       {kind, severity, chapter, source, message, entities}
ink.continuity.check     ( -- dict )  summary counts
                                       {total, contradictions, warnings, info, by_kind}
```

---

## Multilingual

SENTINEL inherits each detector's language coverage and never claims more. The
`introduce` invariant is language-safe everywhere — names come from your own
Characters/Places books and the mention match is Unicode-aware, so Cyrillic and
accented names match exactly as Latin ones do. `co_location`, `timeline`, and
`char_facts` are multilingual as built; `numeric` is EN/FR/ES (it skips cleanly
for other languages). Every finding carries its `source`, so *"does it work in
Russian?"* answers itself per detector.

---

## What it is not

- Not a new pile of detectors — it's the unification of the ones you have, plus one.
- Not LLM-first — the fuzzy passes stay explicit and cost-capped; the core is
  deterministic and free.
- Not a corrector — it flags, it never rewrites.
