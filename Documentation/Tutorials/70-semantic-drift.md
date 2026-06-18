# Tutorial 70 — Semantic drift: when a description quietly changes

*Inkhaven 1.3.10+*

[Tutorial 69](69-world-consistency.md) caught **hard** contradictions — a
fact that clashes with a fact, a number that clashes with the prose. But a
long book drifts in a softer way: the same tavern is "cramped and smoky" in
chapter 2 and "airy and bright" in chapter 20; a character is "soft-spoken"
early and "booming" later; a sword goes from "notched and dull" to
"mirror-bright" with no scene that polished it. No single fact is wrong —
the *descriptions* just disagree.

## Why this needs both embeddings and AI

Neither half can do it alone:

- **Embeddings** (semantic search) are great at *finding* the paragraphs
  that describe an entity — but cosine similarity measures topical
  relatedness, not contradiction. "Cramped and smoky" and "airy and bright"
  are highly *similar* (both describe the tavern's atmosphere) yet opposite.
- **Rules** (an antonym list) are hopelessly brittle across the open
  vocabulary of prose.

So Inkhaven splits the work the honest way: **embeddings retrieve, the AI
adjudicates.** It reuses the vector index that's already built as you write
(every paragraph is embedded on save — the same index `Ctrl+F` searches), so
there's no new indexing step.

## Run it

```sh
inkhaven drift scan            # needs an LLM provider; --json for a CI gate
```

For each Character, Place, and Artefact, Inkhaven retrieves the paragraphs
that describe it (keeping only the ones that actually *name* it), then asks
the model which descriptions contradict:

```
drift scan: 1 description contradiction(s):
  ⚠ The Drunken Goose (place) — [ch.2] “cramped and smoky”  ⟷  [ch.20] “airy and bright”
      ↳ the same tavern can't be both without a renovation the story never shows
```

The result is cached in `.inkhaven/drift.json` and also surfaces in the
**Editorial Pass** (`inkhaven edit`, category `drift`) alongside every other
finding — jump (`Enter`) to the *later*, divergent passage. It's jump-only:
there's no honest single-paragraph rewrite for "the atmosphere changed across
18 chapters" — you reconcile the two passages yourself.

To preview what the retriever found *without* the AI:

```sh
inkhaven drift list            # deterministic; the description snippets per entity
```

## See it in the story bible

Press **`Ctrl+V Shift+L`**. Under any entity with a drift conflict you'll see
a **⚠ drift** badge (amber), the model's reason, and the entity's full
chapter-ordered **description trail** — every passage that described it, in
order, each one a `→` jump to its source. It's the sequence at a glance, so
you can see exactly where the description turned.

## Tuning

```hjson
drift: {
  top_k: 24          // vector hits pulled per entity before filtering
  max_snippets: 8    // descriptions kept per entity (bounds the AI prompt)
}
```

## What it can and can't catch

- **Retrieval-bounded.** A description in a paragraph that never names the
  entity (pure pronouns) won't be retrieved — the name filter that kills
  topical false-positives also misses unnamed description.
- **AI-judged.** `drift scan` needs a provider and spends tokens (bounded by
  `max_snippets`); the Editorial Pass reads the cached sidecar
  deterministically, offline.
- **Needs the index.** A freshly-imported project may need a reindex (open +
  save, or search once) before retrieval returns anything.

## Where to go next

- The hard-contradiction half: [Tutorial 69](69-world-consistency.md).
- The worklist it feeds: [Tutorial 68](68-editorial-pass.md).
- Every chord: [`../KEYBINDING.md`](../KEYBINDING.md).
- The design: [WORLD-2 plan](../PROPOSALS/WORLD-2_PLAN.md).
