# CANON-READER-1 — "The Ledger Reads Back" (3.16.0)

*Status: PLAN. On `3.16.0-dev`. The flagship; [`WORLD-KEEP-2`](WORLD-KEEP-2_PLAN.md)
follows it in the same release.*

## Why

Through 3.15.0 the Canon Ledger only *answers*: what breaks if I cut this, why is
this canon, how settled is it. It never *speaks first*. Nothing checks the
manuscript against the decisions it records, so the ledger and the prose can
drift apart silently — a paragraph that established a fact is rewritten and the
decision still claims it; a scene is deleted and its decisions float on with no
source; half the book rests on a choice still marked "floated".

Every other intelligence in inkhaven is a *reader*: SENTINEL, KEN, BONDS, LECTOR
each walk the book and report findings into one worklist (the Editorial Pass),
one history (CHRONICLE), one dashboard. This flagship makes canon the next
reader. It is the same shape KEN took in 2.6 — a deterministic core that costs
nothing at any book size, plus one opt-in model pass for the subtle case.

Verified in the 3.15.0 tree: `editorial.rs` already bridges one canon finding
(`from_canon_fork`, source `"canon"`); `canon check` computes the support check
but prints it only from the shell; CHRONICLE counts findings `by_source`, so a
canon reader is measured the moment it reports through `collect`.

## The findings

Deterministic — free, no model, any book size:

- **`orphaned_decision`** — a live decision whose source paragraph no longer
  exists. (Deleting the source is allowed and warned about; this is the standing
  record that it happened.)
- **`drifted_source`** — the paragraph that established a decision no longer
  carries it: none of the decision's content words (stemmed, in the project
  language — the same stems grounding inference uses) appear in the paragraph
  any more. A proxy, deliberately conservative: it fires on "the scene was
  rewritten into something else", not on a rephrase.
- **`built_on_sand`** — a decision committed above something it rests on (the
  existing support check), now a finding instead of a shell-only report.
- **`unsettled_foundation`** — a decision other decisions rest on that is still
  unmarked or floated. The more that rests on it, the higher it ranks.

Opt-in, cost-capped (`--deep`):

- **`contradicted`** — for decisions the author has marked committed or
  canonical: retrieve the passages most related to the decision (the existing
  semantic search), and ask the model whether any passage *contradicts* it. A
  finding cites the paragraph. Conservative prompt, project language, the JSON
  contract and cost reporting the other deep passes use.

## Phases (value core = P1 + P2 + P3)

- **CR-P0 — substrate.** `canon::read` module: the finding type
  (`CanonFinding { kind, decision, node, message, weight }`), reading-order
  position for sorting, a `check(store, hierarchy, language) -> Vec<CanonFinding>`
  entry point. Pure; unit-tested on a ledger built in a temp dir.
- **CR-P1 — the four deterministic detectors.** `orphaned_decision`,
  `drifted_source`, `built_on_sand` (reusing `commitment_warnings`),
  `unsettled_foundation` (reusing `impact`). Multilingual by construction
  (stemmer per project language; Unicode-aware).
- **CR-P2 — the shell.** `inkhaven canon read [<scope>] [--json] [--deep]
  [--max-cost N]` prints findings grouped by kind; exit status non-zero under
  `--strict` for CI. `canon check` keeps its meaning and output (it is one of the
  four detectors).
- **CR-P3 — the worklist and the history.** `from_canon_finding` joins `collect`,
  so the Editorial Pass (`Ctrl+V Shift+R`), `inkhaven revise` and CHRONICLE all
  see canon findings. Routing: `drifted_source` and `contradicted` are
  **Decisions** ("which is right, the scene or the ledger?"); the others are
  **Briefs** (you reconcile the ledger, not the prose). Nothing rewrites prose.
- **CR-P4 — in the editor.** The Canon dashboard gains a findings section; a
  decision with a finding is marked in the Canon pane, and its row says which
  (`⚠ drifted`, `⚠ on sand`). `Enter` on a finding jumps to the paragraph
  involved. The reader hub count includes canon.
- **CR-P5 — `--deep` contradiction pass.** Background job, single-flight, cost
  shown, findings staged into Output like the other deep passes; only decisions
  the author marked committed/canonical are checked (the cap on cost *and* on
  noise).
- **CR-P6 — Bund + docs + cut.** `ink.canon.read ( -- list )` (classified
  `store_read`); CANON.md, KEYBINDING, release notes, Manual ch19,
  Know-Your-Book ch8.

## The ~1-user gate

The ledger is now easy to fill (3.15.0). A filled ledger that nobody checks goes
stale, and a stale ledger makes `impact`, the Canon pane and grounded chat
quietly wrong — the grounded chat most of all, since it tells the model "these
are the author's decisions". This is the maintenance loop that keeps the
3.11–3.15 investment true. Accretion: one module, one bridge function, one
dashboard section; the detectors reuse `impact`, `commitment_warnings` and the
grounding stems.

## Constraints kept

Advisory (never edits prose or the ledger); deterministic core free and
unbounded; the one model path opt-in, cost-informed, never blocking;
multilingual; no new store; new Bund word classified.

## Decision to make at CR-P1

`drifted_source` as specified needs no stored state (it compares the decision's
words with the paragraph as it stands). A stricter alternative records a
fingerprint of the source paragraph when a decision is accepted and flags *any*
later change — precise, but it needs a sidecar and is noisy for ordinary
polishing. **Recommendation: ship the stateless proxy; add the fingerprint only
if it proves too quiet in use.**

## Non-goals

Auto-retconning or auto-regrounding; checking decisions nobody has committed
with the model (too noisy, too costly); replacing `canon check`.
