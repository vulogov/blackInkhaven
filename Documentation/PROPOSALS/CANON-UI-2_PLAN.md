# CANON-UI-2 — "Canon in Hand" (3.15.0)

*Status: SHIPPED in 3.15.0 "Canon in Hand" (CU2-P1 83d82c28 · CU2-P2 24031292 ·
CU2-P3 75a2473b · CU2-P4 6c3ad9de · CU2-P5 docs + cut). Follows [`CANON-UI-1_PLAN.md`](CANON-UI-1_PLAN.md)
(3.14.0 "Canon at Hand") and closes its two deferred proposals (C, D) as riders.*

## Why

3.14.0 put the ledger *in view* while writing: the `◈` glyph, the paragraph-aware
Canon pane, Book-scope chat grounded on the decisions. All three show nothing
until the ledger is populated — and populating it is still a shell job. Verified
in the 3.14.0 tree: the editor can *ground* a decision (dashboard `g`) and
nothing else. `canon harvest`, `canon staged`, `canon accept` and `canon commit`
have no editor surface at all. The author has to leave the manuscript to feed the
thing that is supposed to sit beside it.

3.15.0 makes the Canon pane the place the ledger is *tended*, not only read:
propose decisions for the open paragraph, confirm or discard each one, and say
how settled it is — without leaving the scene. No new store, no new dependency;
everything drives the 3.11–3.13 API (`harvest_llm` staging, `record_grounded_batch`,
`commit`) that the CLI already uses.

## Phases (cheapest first; value core = P2 + P3)

- **CU2-P1 — commitment in the pane.** `c` on the cursored decision opens a small
  level picker (floated → drafted → committed → canonical → retconned) and calls
  `CanonLedger::commit` as agent `author`. The row's `«commitment»` updates in
  place; if the new level trips the support check (canonical resting on floated —
  `commitment_warnings`), the status line says so. Deterministic, free. Also in
  the dashboard, so both surfaces agree.

- **CU2-P2 — staged proposals in the pane.** The pane gains a *Proposed* section
  under the paragraph's decisions: the entries of `.inkhaven/canon-staged.json`
  whose `node` is the open paragraph (`[kind] gist`, `↳ rests on: …`). `a`
  accepts the cursored proposal, `x` discards it, `A` accepts all of this
  paragraph's. Accept goes through the same `record_grounded_batch` path as
  `canon accept` — refactored into one shared function that takes a subset, so
  the CLI's all-at-once accept and the pane's per-proposal accept cannot drift —
  and removes exactly the accepted entries from staging (atomic rewrite). This
  is useful on its own: proposals staged from the shell become reviewable where
  the prose is. The pane title shows the pending count.

- **CU2-P3 — harvest the open paragraph.** `H` in the pane asks the model to
  propose decisions for the open paragraph: one call, the existing
  `harvest_llm::system_prompt` in the project language, `parse_proposals`, and
  the result is **appended to staging only** — nothing enters the ledger until
  `a`. Runs off the UI thread (single-flight, panic-isolated like the F1 help
  search), with a spinner in the pane title and `Esc` to cancel; cost is shown,
  never blocks. Harvests the *saved* text; a dirty buffer prompts "save first".
  Proposals identical to an existing live decision or an already-staged entry of
  the same paragraph are dropped before staging.

- **CU2-P4 — the riders (CANON-UI-1 proposals C and D).**
  - *Impact-on-edit (C).* Saving a `◈` paragraph whose decisions have dependents
    posts one advisory status line — *"canon: N decision(s) rest on what this
    paragraph established — check they still hold (Canon pane)"* — once per
    paragraph per session, so it informs without nagging. Extends the pre-cut
    delete guard from *deleting* to *changing*.
  - *Commitment at a glance (D).* The `◈` in the Tree and Outline is tinted by the
    strongest commitment among the paragraph's decisions (dim = unmarked/floated,
    normal = drafted/committed, bold = canonical, struck = retconned), computed
    in the same throttled pass that builds the source-node set.

- **CU2-P5 — docs + cut.** CANON.md ("In the editor" → tending from the pane),
  KEYBINDING §4.2 (the new keys), RELEASE_NOTES/3.15.0.md, Manual ch19 +
  Know-Your-Book ch8. Then cut 3.15.0 (README regen per the rule; publish only on
  an explicit "publish").

## The ~1-user gate

Would the author use it? The pane is already open beside the scene; `H` then `a`
is two keys against a shell round-trip with three commands, so the ledger gets fed
at the moment the decision is fresh. Accretion: P1 and P4 are small; P2 is a
refactor of an existing path plus a list section; P3 is the one new background job
and reuses the harvest prompt, parser and staging file as they are.

## Constraints kept

- **AI-advisory.** The model only ever writes to the staging sidecar. A decision
  enters the ledger on the author's per-proposal `a` — the same contract as
  `canon accept`, at finer grain. Prose is never touched.
- **Multilingual.** Harvest uses the existing project-language prompt
  (en/ru/fr/de/es); grounding inference stays stemmer-aware.
- **Stability.** Background harvest is single-flight and panic-contained; staging
  and ledger writes stay atomic (`io_atomic`, `sync`).
- **Permissive.** Cost is shown, never capped into a refusal.
- **No resizable panes.** Still one cycled pane mode.

## Non-goals

Harvesting a whole chapter or book from the editor (stays `canon harvest <scope>`
in the shell); remembering rejected proposals across harvests (a discard just
removes the staged entry); auto-accept of any kind; checking prose *against* the
ledger (the "canon as a reader" candidate — a natural 3.16 follow-on once the
ledger is easy to fill).

## What follows

[`WORLD-KEEP-1`](WORLD-KEEP-1_PLAN.md) — "A World That Keeps Its Word" — is
sequenced directly after this flagship.
