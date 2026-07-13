# SCHOLAR_PLAN — Implementation plan: contradiction & tension analysis over collected facts

**Status:** implementation plan (grounded in current code). Sibling to the design in [`THEOLOGY_PHILOSOPHY_RFC.md`](THEOLOGY_PHILOSOPHY_RFC.md) §7.1. This is the *differentiator* — the analysis, not the source adapters.
**Date:** 2026-07 · **Branch:** 1.6.16-dev.

---

## 0. The problem, stated against the code

`/factcheck` already runs a "mutual consistency" pass, but it is **simple factchecking** and, structurally, a dead end for what we want:

- The gathered fact is `FactEntry { id: Uuid, location: String, text: String }` (`src/research/factcheck.rs:17`) — **no provenance**. Origin lives in a *separate* id-keyed sidecar `.inkhaven/fact-sources.json` (`Provenance::for_node`, `src/research/provenance.rs:112`) and is **never joined** into the factcheck path.
- The consistency prompt asks the model for `<a> ⇄ <b> — <what conflicts>` (`factcheck.rs:105`), but **the reply is never parsed** — `poll_factcheck` (`app.rs:2876`) concatenates it verbatim into one `consist_report: String`. There is no pair struct, no back-reference to `FactEntry.id`, no anchoring. (Contrast the *truth* pass, which **is** parsed — `verdicts.rs:141` `parse_report` — into `Verdict`s persisted to `fact-verdicts.json` with tree glyphs.)
- Grouping is structural only — `consistency_groups` partitions by Facts-tree branch (`factcheck.rs:147`), not by topic/meaning. **No semantic clustering primitive exists in-tree.**
- The result is a chat `String` (`finish_factcheck`, `app.rs:2929`) — no paragraph anchoring, no navigable per-contradiction surface.

**The single biggest lever:** turn contradiction findings from *opaque text* into *addressable objects* — `Clash { a, b, stance, topic, reason }` with each side joined to its `SourceRecord`. Everything else (source-attribution, grading, clustering, anchoring) builds on that.

## 0.1 Scope — SCHOLAR is a *relation engine* (contradiction AND confirmation)

SCHOLAR is not only about contradiction. It relates two bodies — the **Text** (the manuscript) and the **Research** (the collected Facts / ingested sources) — and reports **both** directions of the relation. The graded stance judge already spans it (`CONTRADICTS … AGREES`); we surface *both ends*, because a scholar wants "what backs my claim" as much as "what undermines it."

| | **Contradiction** (a risk) | **Confirmation** (support to cite) |
|---|---|---|
| **Text ↔ Research** | your claim opposed by a source | your claim backed by a source |
| **within Research** | sources disagree — P1 `/contradict` | sources *converge* — triangulation-of-agreement |
| **within Text** | prose self-contradiction → **Inner Socrates** (existing; SCHOLAR defers) | — |

Consequences for this plan:
- The per-claim pass (Slice 4) is **relate**, not just *confront*: it keeps `AGREES`/`QUALIFIES` as **confirmation** findings, not only `CONTRADICTS`/`TENSION`. Two finding kinds: a clash and a confirmation.
- **within-Research confirmation** (sources converging on a point) is a cheap add on the same graded judge — the `AGREES` aggregate.
- **within-Text** contradiction is **Inner Socrates'** job (it already presses prose for unstated premises, shifting terms, unanswered objections). SCHOLAR does **not** duplicate it.
- **Open track — Inner Socrates over Research:** Inner Socrates reads *prose* today. Pointing it at the *Facts/research corpus* (Socratic interrogation of the collected facts — "what does this fact assume? what would refute it?") is a distinct, complementary capability (questioning vs. detecting). Recommended as its **own** track (extend Inner Socrates' scope selector from the manuscript to a chosen book incl. Facts), not folded into the contradiction engine.

**What's a near-drop-in generalization** (cheap): the graded stance judge (generalize the `/triangulate` prompt, `app.rs:2465`); per-passage retrieval (`book_rag::retrieval::retrieve`, `retrieval.rs:21`); the numbered-line parser (clone `verdicts.rs:141`); the fact→provenance join (`for_node` exists, just unwired); the LLM call (`spawn_chat_stream`, `ai/stream.rs:53`); the Output-pane emit (the NF-CITE/XREF pattern).

**What must be built fresh:** the structured `Clash` type + its parser; the graded `Stance` enum + parser; a structured source-passage retrieval variant (the existing `research::rag::retrieve_sources` returns *names*, side-effecting — not reusable); **topical clustering** (no primitive — build a small one, LLM-grouping in v1); the persistent contradiction report + rendering; the anchored `kinds::CONTRADICTION` finding + editor chord.

---

## 1. New module + core types

**`src/research/contradiction.rs`** (new) — pure logic + prompts; wiring stays in `app.rs`.

```rust
/// A gathered fact joined with its recorded provenance.
pub(super) struct SourcedFact {
    pub id: Uuid,
    pub location: String,      // FactEntry.location breadcrumb
    pub text: String,
    pub origin: String,        // SourceRecord.origin ("archive", "web", "model", ...)
    pub source: String,        // SourceRecord.summary() — "archive: <detail>" / a locus if recorded
}

/// The graded relation between two claims (generalizes triangulate's SUPPORTS/CONTRADICTS/SILENT).
pub(super) enum Stance { Contradicts, Tension, Qualifies, Agrees, Silent }

/// One structured, addressable contradiction/tension finding.
pub(super) struct Clash {
    pub a: SourcedFact,
    pub b: Side,               // another fact, or a retrieved passage
    pub stance: Stance,
    pub topic: String,         // cluster label ("" until clustered)
    pub reason: String,
}
pub(super) enum Side { Fact(SourcedFact), Passage { name: String, body: String, id: Option<Uuid> } }

pub(super) struct ContradictionReport {
    pub clusters: Vec<TopicGroup>,     // { topic, clashes }
    pub scanned: usize, pub within_source: usize, pub cross_source: usize,
}
```

---

## 2. Slices (each ships independently, in order)

### Slice 1 — `SourcedFact`: join facts with provenance *(foundation)*
- `pub(super) fn gather_sourced(store, h, book_id, prov: &Provenance) -> Vec<SourcedFact>` — reuse `factcheck::gather` (`factcheck.rs:35`) then, per `FactEntry.id`, `prov.for_node(&id.to_string())` → fill `origin` + `source` (`SourceRecord::summary()`), defaulting to `origin="manual"` when absent.
- **Reuse:** `factcheck::gather`, `Provenance::for_node`. **Test:** a fact with a recorded `SourceRecord` surfaces its origin; one without → `manual`.

### Slice 2 — Structured, source-aware consistency *(parse the `⇄`, attach provenance)*
- New prompt `consistency_indexed_system` — same task as `factcheck.rs:105` but demands **numbered** output: `<i> ⇄ <j> — <what conflicts>` referencing the fact numbers (like the truth pass numbers its input).
- `pub(super) fn parse_clashes(reply: &str, facts: &[SourcedFact]) -> Vec<Clash>` — clone the numbered-line parser (`verdicts.rs:141`): parse `<i> ⇄ <j>` → map to `facts[i-1]`, `facts[j-1]`, `stance = Contradicts`, `reason`. Each side already carries provenance ⇒ **source-attributed contradiction** ("`archive: Kant …` ⇄ `web: …`").
- **Within-source** = filter clashes to `a.origin == b.origin`; **cross-source** = `!=`. Counts feed the report.
- Run per group (reuse `consistency_groups` structural partition to bound prompt size) → merge.
- **Reuse:** the consistency prompt + `consistency_groups` bounding; the `verdicts.rs` parser. **Test:** `parse_clashes("2 ⇄ 5 — differ on X\n", &facts)` links `facts[1]`/`facts[4]` with both origins; "No contradictions" → empty.
- **Surface:** a research-TUI `/contradict` command → a report grouped by within/cross-source, each clash citing both loci. (`/factcheck` unchanged for now; can adopt the engine later.)

### Slice 3 — Graded stance judge *(beyond binary)*
- New prompt generalizing `/triangulate` (`app.rs:2465`): per item, `<label>: CONTRADICTS | TENSION | QUALIFIES | AGREES | SILENT — <reason>`.
- `pub(super) fn parse_stance(line: &str) -> Option<(String, Stance, String)>` + `Stance::parse`. **Test:** each variant round-trips; unknown → None.
- Used by Slice 4's confront path (claim vs retrieved passages), and optionally to *re-grade* Slice 2's pairs from flat `Contradicts` to the finer scale.

### Slice 4 — Per-claim "relate" (retrieve + graded-judge over the corpus — contradiction AND confirmation)
- `retrieve_source_passages(store, cfg, query, k) -> Vec<RetrievedPassage>` (new, `src/research/rag.rs`) — structured variant of `retrieve_sources`: filter `store.search_text(query, k)` to `metadata.kind == "research_source"`, return `RetrievedPassage`-shaped structs (the existing fn returns *names* and is side-effecting — not reusable).
- `pub(super) fn relate(ai, cfg, store, h, claim, fact_book_id) -> Vec<Relation>` — retrieve nearest **facts** (`book_rag::retrieval::retrieve(fact_book_id, claim)`) + nearest **source passages** (above), batch into one graded-judge call (Slice 3). **Keep both ends:** `CONTRADICTS|TENSION` → contradiction findings (risks); `AGREES|QUALIFIES` → **confirmation** findings (support to cite); drop `SILENT`. `Relation { side, stance, reason }`.
- Two Text↔Research directions of the matrix (§0.1) served by one pass: opposition and support.
- **Bounding (matches existing style):** top-K = a config knob; one judge call per relate; no hard token cap (the budget hook only *informs* — `maybe_warn_budget`, `app.rs:3489`), so bound structurally like `TRUTH_CHUNK`/`CONSIST_MAX`.
- **Reuse:** `book_rag::retrieval::retrieve` + `RetrievedPassage` (`retrieval.rs:21`, `mod.rs:21`); `spawn_chat_stream`.

### Slice 5 — Topical clustering + the report *(the dialectical map)*
- **No clustering primitive exists**, and there's no exposed "embed these N and cluster" call. **v1 = LLM grouping** (one call): `cluster_by_topic(ai, facts) -> Vec<TopicGroup{label, idxs}>` — the model groups the fact list into topics and labels them. Cheaper and clearer than building k-means with no vector-access API; a later v2 can swap in an embedding cluster if a vector-fetch API is added to the store.
- Run Slice 2's graded consistency **within each topic** (bounds pairwise comparisons to intra-topic) → clashes carry `topic`.
- Assemble `ContradictionReport`; **persist** to `.inkhaven/contradictions.json` (mirror `fact-verdicts.json`); render in the research TUI grouped by topic, each clash = `stance` + both loci + reason.
- **Reuse:** the verdicts-sidecar persistence pattern.

### Slice 6 — Anchored findings + the editor "confront" chord
- New Output kind `kinds::CONTRADICTION` (types.rs) + filter group + glyph — the NF-CITE/XREF pattern.
- When `confront` runs on the open **manuscript** paragraph, emit each `Clash` as a paragraph-anchored finding (`Message::new(...).with_source_paragraph(pid)`), cited to the opposing locus.
- A `Ctrl+V` chord (free key, `Scope::Any`) → "confront open paragraph against sources" → findings in the Output pane. Mirrors the `Ctrl+V @/#/&` family.

---

## 3. Surfaces

| Surface | What | Where |
|---|---|---|
| `/contradict` (research TUI) | whole-corpus source-aware graded report, topic-grouped | `command.rs` + `app.rs` (new `start_/poll_contradict`) |
| `inkhaven research --contradict` | headless twin (report to stdout/`--out`) | `cli/mod.rs` + `research/mod.rs` + `contradict_cli` |
| `Ctrl+V` chord (editor) | confront open paragraph against sources → anchored findings | `keybind.rs` + `tui/app.rs` + `pane/output` |

---

## 4. Reuse vs. build-fresh (grounded)

**Reuse (near drop-in):** `factcheck::gather` (:35); `Provenance::for_node` (:112); the consistency prompt + `consistency_groups` bounding (:105/:147); the numbered-line parser `verdicts.rs:141`; `book_rag::retrieval::retrieve` + `RetrievedPassage`; the `/triangulate` judge prompt (:2465); `spawn_chat_stream` (`ai/stream.rs:53`) + `cost_for`/`maybe_warn_budget`; the Output-pane emit (kinds/filter/glyph); the verdicts-sidecar persistence.

**Build fresh:** `SourcedFact` join; `Clash`/`Stance` + their parsers; the indexed + graded prompts; `retrieve_source_passages` (structured); `cluster_by_topic` (LLM v1); `ContradictionReport` + `.inkhaven/contradictions.json` + rendering; `kinds::CONTRADICTION` + the confront chord.

---

## 5. Phasing to releases

- **P1 (a real, addressable contradiction report):** Slices 1+2 — `SourcedFact` join + structured source-aware consistency + `/contradict` + `--contradict`. Immediately useful; turns opaque text into cited, source-attributed pairs.
- **P2 (grading + paragraph confront):** Slices 3+4 — graded stance + retrieve-and-judge a claim against the corpus.
- **P3 (the dialectical map):** Slice 5 — topical clustering + the persistent report.
- **P4 (in-editor):** Slice 6 — anchored `kinds::CONTRADICTION` + the confront chord.

Each is a shippable slice; P1 alone already delivers "find contradictions *between my sources*, cited," which is the core ask.

---

## 6. Risks / decisions

1. **Clustering without a vector API.** LLM-grouping (v1) is a real dependency + cost; a deterministic embedding cluster needs a store `vector_of(id)` accessor that doesn't exist yet. *Decision:* LLM-group v1, revisit if a vector-fetch API lands.
2. **Provenance detail is a free string** (`SourceRecord.detail`) — a *locus* ("Quran 5:32") is only present if recorded there. The primary-source-loci feature (RFC §6) is what makes loci structured; until then, `source` is whatever `summary()` yields. Not a blocker for source-*attribution*, only for locus precision.
3. **Cost.** Deep analysis is more LLM-intensive; bound structurally (topics × intra-topic pairs × one judge call) and keep it author-invoked — the budget hook only informs, never blocks.
4. **`/factcheck` overlap.** Ship `/contradict` as the new engine; leave `/factcheck` as-is; optionally migrate its consistency section onto the structured engine later.

---

*Implementation plan, grounded in the 1.6.16-dev code. Slice boundaries are firm; prompt wording and the clustering approach may move.*
