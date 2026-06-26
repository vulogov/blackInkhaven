# AUDIENCE-1.1 — Utopian / philosophical / theological readers (plan, 1.4.7)

A small additive extension of AUDIENCE-1: three more bundled Inner-Socrates
personas and three more genres for authors whose work is neither plain fiction
nor empirical nonfiction. **No new categories, no new storage, no new modules.**
Builds directly on the AUDIENCE-1 machinery (1.4.6).

## Why these three don't fit the existing nine

| Author | Why the current personas miscalibrate |
|---|---|
| **Philosophy** | `expert-reviewer` is an *empiricist* ("evidence must support the claim, causal language for correlational evidence"). Philosophy lives on logical structure — the unstated premise, the equivocation, the unaddressed counterexample, the valid-but-unsound argument — not empirical evidence. |
| **Theology** | The sharpest case. Demanding "where's your evidence?" is category-wrong for claims grounded in revelation and tradition. The right reader probes **internal coherence, fidelity to source, and the scope of each claim** (revealed vs. reasoned) — never empirical proof. Needs explicit non-empiricist priming or it becomes a hostile skeptic. |
| **Utopia** | Genuinely **hybrid** — fiction (narrative, scenes) *and* an argument about society. The nonfiction personas mute the narrative categories, which is wrong here. The right reader keeps dramatization / temporal-density **live** while heavily weighting "what does this society assume about human nature?" and "what cost does the utopia elide?" |

The existing 5 Slow prose categories cover all three — philosophical/theological
probing is assumption-surfacing + framing + significance + implicit-comparison +
tension. So no new categories (that stays AUDIENCE-2). The genre framing is
LLM-instruction, not lexical matching → multilingual for free.

## The three personas

- **`philosophical-reader`** ("The Dialectician") — treatise. Mutes narrative.
  Heavy on assumption-surfacing (unstated premises) + framing (equivocation,
  definitional scope) + implicit-comparison (counterarguments) + tension
  (contradiction).
- **`theological-reader`** — theology. Mutes narrative. **Non-empiricist** — its
  voice explicitly respects revelation/tradition as grounds, probing coherence,
  fidelity, and scope rather than demanding proof; modal/hedging fast categories
  attenuated so it doesn't hammer conviction as overclaiming.
- **`utopian-architect`** — utopian/dystopian fiction. **Hybrid**: the narrative
  categories are left at default (NOT muted) so it still reads as fiction, while
  assumption-surfacing + implicit-comparison are weighted hard (the society as a
  designed argument; the elided cost; the foreclosed alternative).

## The three genres

Added to both `slow_genre_context()` (interrogator framing) and `genre_fragment()`
(Editor craft), same key set:
- `utopian` / `utopia` / `dystopian`
- `philosophy` / `philosophical`
- `theology` / `theological` / `religious`

## Phase map

- **B-P0 — Three personas (pure).** Add to `bundled()` (9 → 12). Tests:
  12 distinct; philosophical/theological mute the narrative categories;
  **utopian-architect does NOT mute them** (the hybrid invariant); emphasis
  sheets; theological-reader attenuates the empirical fast categories.
- **B-P1 — Three genres (pure).** Add to `slow_genre_context()` +
  `genre_fragment()`. Tests: each resolves in both functions; aliases; the
  fiction default for `None`/unknown still holds.
- **B-P2 — Docs → cut 1.4.7.** INNER_SOCRATES persona table (12), CONFIGURATION
  genre list, Tutorial 90 (or a short 91) updated. Cut 1.4.7.

## Non-goals

New categories (e.g. `logical_gap`, `citation_needed`) — AUDIENCE-2. Per-book
persona routing — AUDIENCE-2. Rewriting any of the existing 9 personas.

Test baseline 1904 (1.4.6); target ≥ ~1918.
