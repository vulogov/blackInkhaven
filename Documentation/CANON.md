# The Canon Ledger (CANON)

*(3.11 CANON-LEDGER-1; 3.12 CANON-2 "Grounds That Hold" — the dependency graph
populates itself, so `impact`/`why` work; plus development history and the pre-cut
guard. 3.13 CANON-3 "Tend the Ledger" — `reground` backfill, `compact`, and the
`graph` view. See [`PROPOSALS/CANON-LEDGER-1_PLAN.md`](PROPOSALS/CANON-LEDGER-1_PLAN.md),
[`PROPOSALS/CANON-2_PLAN.md`](PROPOSALS/CANON-2_PLAN.md),
[`PROPOSALS/CANON-3_PLAN.md`](PROPOSALS/CANON-3_PLAN.md), and
[`PROPOSALS/SMYSL-1_RFC.md`](PROPOSALS/SMYSL-1_RFC.md). Built on the
[`smysl`](https://github.com/vulogov/smysl) crate.)*

Inkhaven has always understood your **text** (that's what git versions) and the
**current state** of your world (Facts, the semantic graph, KEN, SENTINEL). And
CHRONICLE remembers the readers' *findings* over time. But nothing remembered the
**decisions behind the fiction** — what you settled, revised, or retracted, and
what each decision rests on.

> **The Canon Ledger is the development history of your story's canon.** Each
> load-bearing decision — a world-fact, a character trait, a plot point, a reveal,
> a setup — is recorded with what it *depends on*, how *settled* it is, and where
> it came from. Then you can ask the questions a reviser actually has: **"what
> breaks if I cut this?"**, "why is this true in my world?", and "what changed in
> the canon since last draft?"

It is backed by an append-only, content-addressed store (`<project>/canon.cbor`),
one per project. The deterministic queries are **free and instant** (no model
call); the only path that consults a model is the opt-in harvest, and even that
never writes to the ledger without your confirmation.

---

## What a decision is

A canon decision has:

- a **kind** — `world-fact`, `character-trait`, `plot-point`, `reveal`, or `setup`;
- a **gist** — one sentence stating it;
- **grounds** — the decisions it rests on (e.g. a reveal rests on its setup);
- a **commitment** — how settled it is, on the canonicity ladder
  `floated → drafted → committed → canonical → retconned` (this is *authorial
  commitment*, not real-world truth — the epistemic axis stays inert for fiction);
- a **source** — the manuscript paragraph it was derived from, so you can jump
  straight back to it.

Its identity is a hash of its content, so recording the same decision twice is
idempotent, and two writing sessions or devices reconcile by **merge** with no
coordinator.

## Grounds — the dependency graph `impact` and `why` walk

`canon impact` and `canon why` are only as useful as the **grounds** edges between
decisions, and those get drawn three ways — you rarely draw them by hand:

1. **Deterministically, at no cost (3.12).** When a decision enters the ledger, its
   grounds are inferred from **kind rules + shared content words**: a *reveal* rests
   on the *setup* that shares a word with it; a *plot-point* / *reveal* rests on the
   *world-fact* whose subject it reuses (matching is stemmed and stop-word-filtered
   in your project language, so "leviathan" ~ "leviathan's" but function words never
   match). Conservative on purpose — a spurious ground would distort `impact`.
2. **By the opt-in harvest (3.12).** The harvest model, already reading a scene,
   names which sibling decisions each one rests on; those grounds are resolved on
   `canon accept` and unioned with the deterministic ones. Still author-confirmed.
3. **By hand.** `canon ground <id> --on <ground>` draws an edge the harvest missed;
   `canon unground <id> --from <ground>` removes a wrong one. In the editor, `g` in
   the Canon dashboard grounds the cursored decision on a target you pick.

Because grounds are part of a decision's content hash, editing them mints a **new**
version that *supersedes* the old one and relinks anything that rested on it — the
history is kept, and a decision's **commitment survives** a regrounding.

## How decisions get into the ledger

Three ways, in increasing model-involvement:

1. **Deterministic harvest on save (zero effort).** When you save a paragraph, its
   authored tags become decisions — a `rel:<kind>:<A>:<B>` relationship tag becomes
   a `character-trait`. No model, no cost; untagged paragraphs produce nothing.
2. **Opt-in LLM harvest.** `inkhaven canon harvest <scope>` reads the prose and
   *proposes* decisions. Nothing enters the ledger — proposals are **staged** in
   `.inkhaven/canon-staged.json`. You review (`canon staged`) and **confirm**
   (`canon accept`). The model proposes; you decide. Gists are written in your
   project language.
3. **Merge.** `inkhaven canon merge <other canon.cbor>` folds in another ledger
   (another session/device). Where two agents committed the same decision to
   different levels concurrently, the disagreement is recorded as a
   **commitment fork**, not silently resolved.

## The questions you can ask

| Ask | Command | What it does |
|---|---|---|
| **What breaks if I cut this?** | `canon impact <id>` | Every decision that transitively rests on it — the blast radius. |
| Why is this canon? | `canon why <id>` | The grounds chain the decision rests on. |
| What decisions are there? | `canon list` | Every decision, with kind, commitment, and source. |
| How did this one develop? | `canon history <id>` | Its grounds and its commitment trajectory over time. |
| How did the canon settle? | `canon log` | Every commitment event across the ledger, oldest first. |
| How settled is it? | `canon commit <id> --level <l>` | Set the canonicity (`floated…canonical…retconned`). |
| Am I building on sand? | `canon check` | Decisions committed **above** the weakest thing they rest on (SMY-W057). |
| Show me the whole shape | `canon graph [<id>]` | The grounds DAG — foundations, with what rests on them indented beneath. |
| Draw / cut a dependency | `canon ground <id> --on <g>` · `canon unground <id> --from <g>` | Ground a decision on another by hand, or remove an edge. |
| Where do agents disagree? | `canon forks` | Commitment forks — concurrent disagreement on canonicity (SMY-W058). |
| Ground an answer on canon | `canon context "<query>"` | Fit the relevant decisions (+ their grounds/rebuttals) to a token budget. |

`<id>` is the short id printed by `canon list` (a prefix is fine).

## In the editor

Open the **reader hub** (`Ctrl+B *`) and choose **Canon**: a scrollable dashboard
of every decision with its kind and `«commitment»`, plus a commitment-forks
section. `↑↓` scrolls, **`Enter` jumps to the decision's source paragraph**, **`g`
grounds** the cursored decision on a target you then pick, **`h`** shows its
development history (grounds + commitment trajectory) in the Thoughts pane, `Esc`
closes.

**The pre-cut guard.** When you delete a paragraph that *established* canon
decisions, the delete confirmation warns first — naming the decisions and how many
others rest on them ("2 canon decision(s) were established here… — 3 rest on it").
It's advisory: it informs, never blocks, and since the ledger is derived and
separate, deleting the prose keeps the decisions (they're left source-orphaned, not
pruned) — so *"what breaks if I cut this?"* is answered at the moment of cutting.

Commitment **forks** — two agents disagreeing on a decision's canonicity after a
`canon merge` — also surface in the unified worklist (`inkhaven revise` / the
Editorial Pass, `Ctrl+V Shift+R`) as advisory **Briefs**: there's no single prose
locus to rewrite, so you reconcile the *ledger* (with `canon commit` / `canon
merge`), not the manuscript. A single-author ledger never forks, so this line is
empty and free in the common case.

## Maintenance (3.12+)

Three commands keep the ledger healthy now that the graph holds real edges:

- **`canon reground [--dry-run]`** — deterministically backfill grounds across the
  *whole* ledger. A ledger recorded before 3.12 (or otherwise under-grounded) has
  no edges, so `impact`/`why` come back empty; this runs the same inference the
  harvest applies at creation over the existing decisions, so the graph fills
  without a re-harvest. `--dry-run` previews the edges it would add. Idempotent.
- **`canon compact`** — reclaim the append-only churn. Grounding a decision mints a
  new version that supersedes the old one; `compact` drops the superseded versions
  nothing live needs. Pure bookkeeping — live decisions keep their ids, their
  grounds, **and their commitment** (carried onto the live version first, so a
  regrounded canonical decision stays canonical).
- **`canon graph [<id>]`** — see the whole structure: each foundation, with what
  transitively rests on it indented beneath (or one decision's subtree).

## Configuration

The `canon:` block ([`CONFIGURATION.md`](CONFIGURATION.md#311--canon-the-decision-ledger-canon-ledger-1))
holds behavioural knobs only (the ledger is derived data, nothing to tune there):
`harvest_on_save` (default `true`) toggles the deterministic on-save tag harvest;
`context_budget` / `context_reserve` set the default token budget for `canon
context` when its `--budget` / `--reserve` flags are omitted. A Bund script reads
the ledger through `ink.canon.{list,impact,why,history,forks,graph}` (read-only —
the writes, `commit`, `ground`/`unground`, `reground`, `compact`, and the
author-confirmed harvest/`accept`, stay on the CLI and in the editor).

## Quick start

```
# tag a relationship in a paragraph's metadata, then save → it's harvested:
#   rel:mentor:Vasa:Iri

inkhaven canon list                      # see what's recorded
inkhaven canon harvest test-book/ch1     # opt-in: let a model propose more
inkhaven canon staged                    # review the proposals
inkhaven canon accept                    # confirm them into the ledger
inkhaven canon commit b3:ab… --level canonical
inkhaven canon impact b3:ab…             # what would break if you cut it
inkhaven canon why b3:cd…                # what it rests on
inkhaven canon ground b3:cd… --on b3:ab… # draw an edge the harvest missed
inkhaven canon history b3:ab…            # its grounds + commitment over time
inkhaven canon log                       # the canon settling, oldest first
inkhaven canon check                     # anything canonical resting on sand?
```

## Principles it keeps

- **Advisory.** The model never writes canon; it proposes, you confirm.
- **Deterministic + free.** `impact`/`why`/`list`/`history`/`log`/`check`/`forks`/
  `graph`, grounding, `reground`, and `compact` are pure functions over the store —
  no model call, milliseconds at book scale.
- **Off the hot path.** On-save harvest is deterministic and backgrounded; the LLM
  harvest is explicit and opt-in.
- **Multilingual.** Harvested gists are written in your project language.
- **Derived + safe.** The ledger is rebuildable from your manuscript; a lost or
  corrupt `canon.cbor` costs at most a re-harvest, never prose.

## Relationship to the other intelligences

The ledger records the **decisions**; the readers still diagnose the **draft**.
CHRONICLE trends *findings* per milestone; the Canon Ledger trends *decisions*.
SENTINEL/KEN check current-state consistency; the ledger remembers how that state
was arrived at. The `facts:` block (WORLD-1) is series-shared *current* canon; the
ledger is its *history*.
