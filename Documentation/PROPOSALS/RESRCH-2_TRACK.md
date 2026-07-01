# RESRCH-2 — Grounded Research (track proposal)

| | |
|---|---|
| **Status** | Proposed (track) |
| **Builds on** | RESRCH-1 (`inkhaven research`, shipped 1.5.0) |
| **Theme** | Move from the model's closed-world knowledge to real, **cited** sources — and make the Facts corpus trustworthy |

---

## Part 1 — Review of RESRCH-1 (what shipped in 1.5.0)

### What works well

- **Clean separate-app architecture.** `src/research/` is a self-contained TUI sharing only on-disk
  state. The RESRCH-1 audit corrected every fabricated piece of the RFC up front (no `facts.duckdb`;
  the synchronous crossterm loop + `spawn_chat_stream`, not `tokio::select`; `create_node` +
  `update_paragraph_content`; `find_by_path`; reuse of `book_rag::retrieval`), so the build never chased
  phantom infrastructure. **Zero new runtime crates.**
- **The confirmation discipline holds.** Every `/fact` / `/note` insertion goes through the editable
  overlay; an empty body is refused. Nothing reaches the corpus unreviewed, nothing edits prose.
- **Immediate reachability.** Inserts write the `.typ` file then `update_paragraph_content`
  (auto-reembed), so a new fact is instantly visible to `/diff`, the writing-mode RAG, and every Facts
  consumer — the whole point of the feature.
- **Breadth.** Full `/command` surface, named resumable threads, a first-class Facts tree (rename /
  add / delete / move / copy-cut-paste), themed colours, an arrow-editable prompt, `Ctrl+B h` reference,
  and `/factcheck` whole-corpus auditing.

### Honest gaps & debt (the bridge to RESRCH-2)

1. **No provenance.** The single biggest gap. An inserted fact carries **no source link** — the thread
   logs the `/fact` command, but the fact paragraph itself has no record of where it came from. For a
   knowledge base meant to be *ground truth*, "where did this come from, and when?" is unanswered.
2. **Closed-world knowledge.** The model's training data is the *only* external source — no web, no
   documents. Facts can be confidently wrong, and `/verify` / `/factcheck` are **self-assessment** (the
   model grading its own output), which is structurally weak: a model that's wrong is often wrong about
   being wrong.
3. **Coarse cost model.** `session_cost` is a fixed-rate `~` estimate (`EST_USD_PER_1K_TOKENS`), not a
   per-model price table.
4. **`/factcheck` scale.** The consistency pass sends **all** facts in one call — unbounded context for
   a large corpus; no chunked or pairwise strategy.
5. **No dedup enforcement.** `/diff` *shows* similar facts but nothing stops you inserting a duplicate.
6. **Smaller items.** No multi-fact extraction per call; no tab-completion on `/goto` / `→ path`
   (deferred from 1.5.1); extraction / factcheck show a status line but don't *stream* their output; the
   LLM pipelines are integration-only (pure logic is well unit-tested, the streamed paths are not).

RESRCH-2 is organised so the **foundation gaps (1, 2) come first**, and the hygiene items (3–6) ride
along where they naturally fit.

---

## Part 2 — The RESRCH-2 track

Six phases, each shippable on its own. Dependencies are called out honestly per phase.

### R2-A — Provenance & citations  ·  *no new crates*

Make every inserted fact record its origin. The thread turn already captures the `/fact` command;
extend insertion to also persist a **source record** per fact node:

- A sidecar `.inkhaven/fact-sources.json` mapping `node_id → { origin, detail, retrieved_at }`, where
  `origin ∈ { model, web, document, manual }` and `detail` is a URL / file path / "model knowledge".
  (Sidecar JSON is the established pattern, like `research-threads/`.)
- When a fact's research came from a SOURCES-1 citation, store the **cite key** instead and reuse the
  existing `BibEntry` infrastructure (`src/sources/`).
- Surface it: the confirmation overlay shows the pending source; `/factcheck` and a new `/sources`
  command list each fact's provenance; `--export-thread` includes it.

This is the keystone — once facts can come from the web or documents (R2-B/C), provenance is mandatory,
not optional. Ships first, zero new dependencies.

### R2-B — Document import  ·  *Markdown / text: no new crates; PDF: one new crate*

`/import <path>` (and `inkhaven research --import <path>`) ingests a local document as a retrieval
source:

- **Markdown / plain text — no new crate.** Chunk the file, embed the chunks via the document store's
  existing `add_document(metadata, content)` (which already embeds into the shared HNSW), tagged as a
  research source. The RAG assembler gains a **Sources** axis next to Facts; retrieved doc chunks are
  cited (R2-A) when a `/fact` derives from them.
- **PDF — a follow-on needing text extraction.** `src/pdf/` today is generation / manipulation only
  (PDF-1); it has **no text extraction**. PDF import therefore needs a text-extraction crate
  (`pdf-extract` or `lopdf`-based) — flagged as a **new dependency**, gated behind this sub-phase so the
  MD/text path ships crate-free first.

### R2-C — Web search & fetch  ·  *new direct dep (HTTP client) + a search provider*

The headline the RESRCH-1 RFC named. `/web <query>` (and a `research.web` config block) runs a
configured search API, fetches the top results, chunks + embeds them into a **session-scoped** source
set, and grounds the next answers — with citations to the fetched URLs.

Honest constraints:
- **New direct dependency:** an HTTP client. `reqwest` is already in the lockfile transitively (via
  `genai`), so a direct dep adds little compile cost, but it *is* a new direct dependency — not the
  "zero new crates" RESRCH-1 held to.
- **Network + API key.** Requires network access and a search-provider key in config (`research.web.*`).
  Headless/offline runs must degrade cleanly (the feature simply unavailable).
- **Caching + cost.** Fetched pages cached per session; web fetches counted in the cost display.
- The `/fact` pipeline is **unchanged**; only the RAG source expands. **Provenance (R2-A) becomes
  load-bearing** here — every web-derived fact carries its URL + retrieval date.

### R2-D — `/promote` (Note → Fact)  ·  *no new crates*

Promote a Notes paragraph to a verified Fact: re-run the extraction + confirmation overlay over the
note's text, carry its provenance, insert into Facts, and optionally retire the note. Named in the
RESRCH-1 RFC §27. Pure reuse of the R2-A + insertion machinery.

### R2-E — Trust & hygiene  ·  *no new crates*

The debt items from the review, grouped:
- **Real cost model** — a config-driven per-model price table (`cost.pricing`), replacing the fixed
  `~` rate; the budget note (R-P20) becomes meaningful.
- **Dedup-on-insert guard** — before a `/fact` insert, run the `/diff` retrieval; if a near-duplicate
  exists above a similarity threshold, warn in the overlay ("similar to facts/…; insert anyway?").
- **Chunked `/factcheck` consistency** — for large corpora, cluster facts (by tree branch or by
  embedding neighbourhood) and check consistency within + across clusters, instead of one all-facts
  call.
- **Streamed extraction / factcheck** — show tokens live (dim) instead of only a status line, reusing
  the chat streaming path.
- **Tab-completion** on `/goto` and `/fact → path` (the 1.5.1 deferral) — slug completion against the
  Facts tree.

### R2-F — Batch / headless research  ·  *no new crates*  ·  **✅ Shipped 1.5.6**

`inkhaven research --batch questions.txt` runs a question list non-interactively: query → extract →
(with `--auto-confirm` + a confidence threshold) insert facts that clear the bar, each with provenance,
and write a Markdown report. The confirmation rule relaxes **only** under the explicit flag + threshold
— the interactive default still confirms every insertion (the RESRCH-1 non-negotiable). Useful for
seeding a corpus from a research outline.

- **Built:** `src/research/batch.rs` — per question: Facts-grounded answer (`collect_blocking`) →
  `extract::parse` candidate → confidence probe (`--confidence`, default 0.7) → insert under
  `--auto-confirm` (else report as a candidate), `model` provenance (thread `"batch"`). Markdown report
  to `--out` / stdout.

---

## Sequencing & dependency summary

| Phase | What | New crates |
|---|---|---|
| R2-A | Provenance & citations (sidecar + SOURCES-1 cite keys) | none |
| R2-B | Document import — Markdown/text | none |
| R2-B′ | Document import — PDF text extraction | **1** (pdf-extract / lopdf) |
| R2-C | Web search & fetch | **1 direct** (HTTP client; reqwest already transitive) + a search API |
| R2-D | `/promote` note → fact | none |
| R2-E | Cost table · dedup guard · chunked factcheck · streamed output · tab-completion | none |
| R2-F | Batch / headless research | none |

**Recommended first cut:** R2-A (provenance) + R2-D (`/promote`) + the no-crate R2-E items — a
self-contained "trust" release with zero new dependencies that makes the existing corpus citable and
de-duplicated. External retrieval (R2-B/C) follows as its own release(s), where the new dependencies and
network surface are introduced deliberately.

## Out of scope (later tracks)
- Multi-modal sources (images, audio transcripts).
- Collaborative / shared corpora across projects (overlaps SERIES-1).
- Automated source-credibility scoring.
