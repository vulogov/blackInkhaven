# Tutorial 98 — Utopian/Dystopian Coherence

*Inkhaven 1.4.15*

Inkhaven's world-consistency stack (WORLD-4/5) checks *physical* and *temporal*
facts. It does not check the **logical coherence of a social premise**. A
utopia or dystopia makes a structured claim — *society organised this way
produces these outcomes* — and utopian fiction fails not from plot holes but
from **coherence holes**: the mechanism doesn't scale to enforce the premise,
two consequences contradict, or a scene uses something the premise eliminated.

WORLD-6 adds a three-stage checker for exactly that. It reads **only what you
declare**, reasons about **logical/systemic** structure only (never morality —
that's a future Inner Theologian), and grounds the `utopian-architect` Inner
Socrates persona with what it finds.

## Declare your premise in the World book

In the **World** system book, write your premise set as paragraphs tagged with
the four new `para:utopia-*` values (add a tag with **`Ctrl+B ]`**):

| Tag | Glyph | Means |
|---|:--:|---|
| `para:utopia-premise` | ⊢ | what's different from the real world, and why |
| `para:utopia-mechanism` | ⚙ | how the premise is enforced / maintained |
| `para:utopia-consequence` | ⇒ | what the premise produces |
| `para:utopia-elimination` | ∅ | what the premise rules out |

The checker reads these four paragraph kinds — not your novel's prose chapters.
Consecutive tagged paragraphs form one **premise group**; a gap breaks the
group (a `// world:group:<name>` comment names the next one), so a book with a
dominant society *and* a resistance can declare both.

## Run the check

```sh
inkhaven world utopia-check            # Stage 1 (extraction) — cheap
inkhaven world utopia-check --stage 2  # + pairing (chain logic) — explicit
inkhaven world utopia-check --stage all  # + entailment scan of the prose
```

- **Stage 1** extracts your claims (cheap, one call per group).
- **Stage 2** checks claim *pairs* for logical compatibility (premise→mechanism,
  mechanism→consequence, consequence↔consequence) and raises **CHAIN BREAK**,
  **CONSEQUENCE GAP**, and **INTERNAL CONFLICT** findings. It's the expensive
  one (O(pairs) calls), so it's **explicit-only** — never silent.
- **Stage 3** scans each chapter against the **elimination inventory** and flags
  **ENTAILMENT VIOLATION**s — prose that uses something declared eliminated.

`utopia-check` exits **1** on any chain-logic finding and **2** on any
entailment violation, so it doubles as a pre-submission gate. `inkhaven world
utopia-model` prints the extracted claims without running any checks.

After Stage 1, a deterministic (no-LLM) pass cross-references the elimination
inventory against your **Facts** book — if a documented fact is also declared
eliminated, you get a `Factual` finding to reconcile.

## In the editor

The cached findings surface in the Output pane via the **`Ctrl+B Shift+C`**
review pass (read-only — the LLM stages run only when you ask, via the CLI). The
**`utopian-architect`** Inner Socrates persona (`Ctrl+B J → S` to select it)
opens its Slow session grounded by these findings: it names the unresolved
tensions before it begins its questions.

## Deliberate tensions

If a contradiction is the point (surveillance *and* flourishing — the novel's
central irony), suppress it: `inkhaven world utopia-suppress --finding <id>
--reason "..."`, or add a `deliberate_tension` entry covering
`utopia_coherence` to your magic ledger. Suppressed findings leave the Output
pane but stay recorded.

## Tune & script

The optional `utopia:` config block sets the Stage-2 cost-warning threshold,
the Stage-3 batch size and minimum chapter length, and the group-gap threshold.
Bund: `ink.utopia.{model, findings, violations, suppress}`. Profiles live in
`.inkhaven/utopia.duckdb`.

It checks logical coherence; it never tells you what to write.
