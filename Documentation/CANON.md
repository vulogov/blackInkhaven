# The Canon Ledger (CANON)

*(3.11, CANON-LEDGER-1 — see [`PROPOSALS/CANON-LEDGER-1_PLAN.md`](PROPOSALS/CANON-LEDGER-1_PLAN.md)
and [`PROPOSALS/SMYSL-1_RFC.md`](PROPOSALS/SMYSL-1_RFC.md). Built on the
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
| How settled is it? | `canon commit <id> --level <l>` | Set the canonicity (`floated…canonical…retconned`). |
| Am I building on sand? | `canon check` | Decisions committed **above** the weakest thing they rest on (SMY-W057). |
| Where do agents disagree? | `canon forks` | Commitment forks — concurrent disagreement on canonicity (SMY-W058). |
| Ground an answer on canon | `canon context "<query>"` | Fit the relevant decisions (+ their grounds/rebuttals) to a token budget. |

`<id>` is the short id printed by `canon list` (a prefix is fine).

## In the editor

Open the **reader hub** (`Ctrl+B *`) and choose **Canon**: a scrollable dashboard
of every decision with its kind and `«commitment»`, plus a commitment-forks
section. `↑↓` scrolls, **`Enter` jumps to the decision's source paragraph**, `Esc`
closes.

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
inkhaven canon check                     # anything canonical resting on sand?
```

## Principles it keeps

- **Advisory.** The model never writes canon; it proposes, you confirm.
- **Deterministic + free.** `impact`/`why`/`list`/`diff`/`check`/`forks` are pure
  functions over the store — no model call, milliseconds at book scale.
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
