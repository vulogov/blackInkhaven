# WORLD-2 — Semantic drift: the soft-consistency layer (1.3.10)

_1.3.8 (WORLD-1) caught **hard** contradictions — a fact that clashes with a
fact, a number that clashes with the prose. 1.3.10 catches **soft** drift: two
descriptions of the **same** entity that diverge without a clean factual
clash. A tavern "cramped and smoky" in ch.2, "airy and bright" in ch.20; a
character "soft-spoken" who's later "booming"; a sword "notched and dull" that
becomes "mirror-bright" with no scene that polished it. It surfaces as a new
`drift` category in the now-finished Editorial Pass cockpit._

## The core idea: embeddings retrieve, AI adjudicates

Neither half alone works:

- **Pure embeddings can't detect contradiction.** "Cramped and smoky" and
  "airy and bright" are *topically* very similar (both describe the tavern's
  atmosphere) yet contradictory. Cosine similarity finds same-dimension
  descriptions; it can't tell you they disagree.
- **Pure rules can't either.** An antonym lexicon ("smoky" ⟂ "airy") is
  hopelessly brittle across the open vocabulary of prose.

So we split the work the honest way:

1. **Retrieval (embeddings).** For each entity, pull the handful of paragraphs
   that actually *describe* it — bounded, focused, cheap.
2. **Judgment (AI).** A small AI pass reads those snippets and reports which
   pairs contradict — exactly the `facts check` shape from 1.3.8, scoped by
   retrieval so the prompt stays small.

This is also the **1.4 bridge**: it builds and proves the whole-book
retrieve-then-reason loop the Whole-Book AI Editor needs — without being the
full editor.

## What already exists (reused, not rebuilt)

- **The retrieval index.** `EmbeddingEngine` embeds every paragraph's content
  into the `VectorEngine` *on save*; `VectorEngine::search` already does cosine
  top-k (it backs `Ctrl+F` semantic search). Drift retrieval queries that
  existing index — **no new index, no bulk re-embed.** (`Book` AI scope, by
  contrast, concatenates every paragraph into the prompt — full-context, not
  retrieval; drift does not touch it.)
- **Entity enumeration.** The Characters / Places / Artefacts system books,
  via the same `SYSTEM_TAG_*` walk the 1.3.8 story bible uses.
- **The AI-scan template.** `facts_scan::check` — `ai.resolve_provider` +
  `run_blocking(system, prompt)` + a `parse_conflicts`-style parser + a
  content-hash-invalidated sidecar.
- **The cockpit.** The 1.3.9-finished Editorial Pass — a new category just
  drops in.

Zero new dependencies (fastembed + genai are both already in the tree).

---

## P0 — Entity description retrieval (reuses the existing vector index)

For each entity, assemble the paragraphs that describe it.

- `drift::EntityDescriptions { entity, kind, snippets: Vec<DescriptionSnippet> }`
  where `DescriptionSnippet { chapter, paragraph: Uuid, text }`.
- Retrieval: query the existing `VectorEngine` with the entity name plus a few
  descriptive probes (`"<name> appearance"`, `"<name> manner / voice"`,
  `"<name> condition"`), take the top-k hits, then **keep only paragraphs that
  actually mention the entity name** (the lexicon match — retrieval can drift
  topically, the name filter anchors it), dedup by paragraph id, order by
  chapter.
- Pure assembly over the search results; unit-tested with a stub search so it
  runs offline. The embedding/search call is the only impure edge.

**Deliverable:** given a project, a per-entity list of chapter-ordered
description snippets. Cap at `drift.max_snippets` per entity (default ~8) so
the judge prompt stays bounded — `log` what was dropped.

---

## P1 — The drift judge + `inkhaven drift scan` + sidecar

- `inkhaven drift scan [--json] [--provider <p>]` — for each entity with ≥2
  snippets, an AI pass judges contradictions. Mirrors `facts_scan::check`:
  resolve the provider, send the entity's snippets (each tagged with its
  chapter), parse a pipe-delimited reply (`chapter_a | chapter_b | why`, with a
  none-sentinel + header skipping, like `parse_conflicts`).
- `drift::DriftReport { version, content_hash, conflicts: Vec<DriftConflict> }`
  with `DriftConflict { entity, kind, a, b, chapter_a, chapter_b, paragraph_b,
  detail }`. Sidecar `.inkhaven/drift.json` via `io_atomic`; `compute_hash`
  over the retrieved snippet set so it re-runs only when the descriptions
  change (order-independent, like the facts-check hash).
- `--json` prints the report for a CI gate; the human view prints
  `entity: a ⟷ b ↳ why` grouped by entity.

**Deliverable:** `drift scan` writes the sidecar and prints conflicts; cached,
provider-overridable, offline-safe degrade (no provider → clear error, no
panic).

---

## P2 — Editorial Pass integration

- `editorial::from_drift_conflict(c) -> EditorialFinding` — category `drift`,
  `Severity::Warn`, message `drift: {entity} — "{a}" ⟷ "{b}"`, hint = the
  judge's reason, location = the **later** description's paragraph
  (`paragraph_b`) so `Enter` lands where the divergence shows.
- `cli/editorial::collect` reads the `drift.json` sidecar alongside the facts /
  tension / continuity ones; `inkhaven edit --deep` runs `drift scan` first
  (degrading gracefully when it can't).
- `drift` is **jump-only** — there's no honest single-paragraph auto-rewrite
  for "the tavern's atmosphere changed across 18 chapters"; the fix is the
  author reconciling the two passages. (`fix_spec` returns `None`, so no `✎`.)

**Deliverable:** drift contradictions appear in `inkhaven edit` and the
`Ctrl+V Shift+R` cockpit, ranked with everything else, jumpable.

---

## P3 — Story-bible surfacing

The 1.3.8 story bible (`Ctrl+V Shift+L`) already lists each entity; now it
shows *how it was described over time*.

- Under each Character / Place / Artefact, render its chapter-ordered
  description trail (the P0 snippets, truncated), and a **⚠ drift** badge on
  the rows the P1 sidecar flagged as contradictory.
- `Enter` on a flagged row jumps to the divergent paragraph.
- Reuses P0 retrieval + the P1 sidecar — no new computation in the TUI; the
  bible reads what `drift scan` already wrote.

**Deliverable:** the story bible becomes a continuity-at-a-glance view, with
drift called out in place.

---

## P4 — Docs + 1.3.10 release cut

- **Tutorial 70** — semantic drift: the retrieve-then-judge model, `drift
  scan`, the `drift` category in `edit`, the story-bible trail; contrast with
  `facts check` (hard) and clarify it's retrieval-bounded + AI-judged.
- **CONFIGURATION.md** — a `drift` block: `max_snippets`, `top_k`, a
  similarity floor, which entity kinds to scan, model reuse.
- **KEYBINDING.md** / quick-help — the cockpit gains nothing new chord-wise,
  but the `OpenStoryBible` description gains the drift trail.
- RELEASE_NOTES/1.3.10.md + index row; top README; version bump; signed tag
  `v1.3.10`; `cargo publish`; merge to main; open the next cycle.

---

## Honest limitations (state them; don't hide them)

- **Retrieval-bounded.** An entity described in a paragraph that never names it
  won't be retrieved — the name filter that kills topical false-positives also
  misses pronoun-only description. `log` the coverage.
- **AI-judged.** `drift scan` needs a provider and costs tokens (bounded by
  `max_snippets`); the Editorial Pass reads the cached sidecar deterministically.
- **Needs the vector index populated.** A freshly-imported project may need a
  reindex before retrieval returns anything — detect an empty index and say so.

## Out of scope (carryovers)

- **The Whole-Book AI Editor** (general retrieve-then-reason over any query) —
  the 1.4 headline this cut is the bridge to.
- PDF N-up / booklet presets; CMYK-JPEG grayscale; ePub inline images + popup
  footnotes; sixth supported language; TUI `edit --deep` trigger.

## Phase order

P0 (retrieval) is the foundation; P1 (judge) consumes it; P2 (editorial) and
P3 (bible) both consume P1's sidecar. Sequence: **P0 → P1 → P2 → P3 → P4**.
