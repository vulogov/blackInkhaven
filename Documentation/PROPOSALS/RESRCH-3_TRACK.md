# RESRCH-3 — Authoritative Sources (track proposal)

| | |
|---|---|
| **Status** | **R3-A / R3-B / R3-E shipped 1.5.5** · R3-C `/calc` + R3-D folder import brought forward (1.5.2) · R3-C `/world` display + R3-D Zotero/folder-watch open |
| **Builds on** | RESRCH-2 (`/web`, `/import`, provenance, the source-retrieval pipeline) |
| **Theme** | Where RESRCH-2 added retrieval *mechanisms* (web, documents), RESRCH-3 adds **authoritative, verifiable sources** — each with a dedicated `/` command, an origin tag, and a place on the trust ladder |

## The model — one pipeline, many adapters

R2-B/C built a generic pipeline: **fetch → chunk → embed as a `research_source` → retrieve + cite →
record provenance**. Every RESRCH-3 source is just an **adapter** into that pipeline (mirroring
`web.rs`'s Tavily/SearXNG providers) plus a **provenance origin tag**. Because `origin` is an open
string, new sources need no migration.

### The trust ladder (drives the factcheck-gate posture)

```
deterministic / structured   →  scholarly (DOI/ID)  →  web prose (R2-C)  →  model (closed-world)
   skip the gate                 relax the gate          gate (WC-P3)        gate
```

The more structured/verifiable the source, the less it needs the LLM factcheck-before-commit gate:
- **Deterministic** (computed values, the project's own simulation): the result *is* the proof — the
  gate is bypassed entirely (nothing can be fabricated).
- **Structured KB** (Wikidata triples, with Q-IDs + reference qualifiers): high verifiability — the gate
  is skipped or reduced to a citation check.
- **Scholarly** (OpenAlex/arXiv, DOI/arXiv-ID): the metadata is authoritative, but the abstract still
  feeds the model — relax, don't skip.
- **Web prose** (R2-C) and **model**: keep the gate.

This is the through-line of the track: each new source slots in at a trust tier, and the gate posture
follows automatically.

## Phases

### R3-A — Structured knowledge base: `/wikidata <query>` — **✅ Shipped 1.5.5-dev**

- Search Wikidata entities (REST/SPARQL, free, **no key**, `reqwest` already present), return each
  entity's **structured claims** (label, description, key properties → values) — citable by **Q-ID**.
- **Built:** `src/research/wikidata.rs` — `wbsearchentities` → `wbgetentities` (labels/descriptions/
  claims) → one batched label resolve; external-ID properties filtered; project-language labels;
  `origin=wikidata` (+ Q-ID) with the factcheck gate skipped. `research.wikidata` config (keyless).
- Provenance `origin=wikidata` (+ Q-ID); **top of the trust ladder** → the factcheck gate is skipped (a
  Q-ID-backed triple is already a verifiable fact).
- **Wikipedia is deliberately excluded.** Its prose carries well-documented editorial bias in
  politics / economics / history; Wikidata's structured triples (and their per-statement reference
  qualifiers) expose almost none of that surface. We ground on the structured facts, not the narrative.

### R3-B — Scholarly: `/openalex <query>`, `/arxiv <query>` — **✅ Shipped 1.5.5-dev**

- **OpenAlex** (works API, free, no key — "polite pool" via a `mailto`) and **arXiv** (Atom API, free,
  no key): return papers → title, authors, year, abstract, and a stable **DOI / arXiv-ID**.
- Provenance `origin=openalex` / `origin=arxiv` (+ the ID). **Scholarly tier** → relaxed gate.
- **Auto-citation:** a `/fact` derived from a paper can auto-create a **SOURCES-1 `BibEntry`** (that
  infrastructure already exists — `src/sources/`), so the fact and a real bibliography entry land
  together.
- **Built:** `src/research/scholarly.rs` — OpenAlex `works` (mailto polite pool, abstract rebuilt from
  the inverted index) + arXiv Atom (crate-free extraction, HTTPS). A `/fact` → `origin=openalex|arxiv`
  (+ DOI/ID), gate skipped, and `add_bibentry` writes a `BibEntry` into a **Research** chapter of the
  Sources book (dedup by cite key). `research.scholarly` config (`enabled`/`mailto`/`auto_cite`).

### R3-C — Deterministic / computational: `/calc`, `/world` (zero new crates, zero network)

The strongest fit with Inkhaven's zero-AI / no-fabrication ethos — see the *How* note below.
- **`/calc <expression>`** — a deterministic evaluator (arithmetic, **unit conversions**, physical
  constants) backed by the in-tree **Bund** VM (`rust_multistackvm`, already a dependency) + a small
  constants/units prelude. The computation *is* the proof; the result shows its steps. `origin=computed`
  → **gate bypassed**. **✅ Shipped 1.5.2** — `src/scripting/stdlib/calc.rs` (13 constants + 24
  conversion words, integer/float interchangeable); `/calc` in the research assistant records
  `origin=computed`.
- **`/world <query>`** — surface the project's own **WORLD-4/5/6 simulation** (astronomy, world-state)
  as a deterministic, internally-consistent fact source. `origin=simulation` → gate bypassed. No
  network; the data is already in the project. **✅ Shipped 1.5.6** — `/world` lists the layers,
  `/world <layer>` renders its facts (materialized book, or recomputed from `world.hjson`); a `/fact`
  records `origin=simulation` and skips the gate.

### R3-D — Author's library: Zotero / vault / folder-watch (the trust anchor — see note below)

- **Zotero / BibTeX import** — ingest the author's curated reference library. Simplest path: read a
  **BibTeX/CSL-JSON export** (reuses the existing `sources::parse_bibtex`; **no new crate**) → research
  sources + `BibEntry`s, with attached PDFs embedded via the R2-B path. `origin=library`.
  **✅ Shipped 1.5.6** — `/import <file.bib>` **and `<file.json>`** parse BibTeX / **CSL-JSON** →
  `BibEntry`s in the Sources book's Research chapter (dedup by cite key). *PDF-attachment embedding +
  direct `zotero.sqlite` read deferred.*
- **Vault import** — recursively import an **Obsidian-style Markdown vault** (a folder of linked notes),
  preserving note titles as source names. Extends `/import` to folders. No new crate. **✅ Shipped
  1.5.2** — `/import <folder>` (and `inkhaven research --import <folder>`) recurses over md/txt/pdf.
- **Folder-watch / sync** — a designated research folder re-imported on change (CLI `inkhaven research
  --sync <folder>`, or a manifest auto-imported on launch), so the corpus tracks the author's working
  files instead of one-shot `/import`. **✅ Shipped 1.5.6** — `--sync <folder>` registers a folder in
  `.inkhaven/research-sync.json` + imports it; each launch re-imports registered folders whose newest
  file changed (mtime-gated). No file-watcher crate.

### R3-E — Cross-cutting: triangulation — **✅ Shipped 1.5.5-dev (`/triangulate`)**

Once ≥ 2 sources exist, a fact's claim can be checked against **several** sources (web + Wikidata +
OpenAlex) and the **agreement/disagreement** reported — a far stronger gate than the model grading
itself. Folds into the WC-P3 confirmation gate (verdict becomes "3/3 sources agree" rather than the
model's self-assessment).

- **Built:** **`/triangulate [claim]`** (bare → last response) gathers evidence from the structured
  sources concurrently (Wikidata + OpenAlex + arXiv, `tokio::spawn` + join), then one LLM call judges
  **each source** `SUPPORTS | CONTRADICTS | SILENT` against the claim (judging *external* evidence, not
  its own output) and reports an `Agreement: n/m support` tally. Streams into chat via the shared path.
- **Gate-fold ✅ Shipped 1.5.5-dev:** `research.triangulate_gate` makes triangulation the automatic
  `/fact` gate for `model` / `web` / `document` facts (replacing the single-source self-check). On
  confirm it gathers evidence → judges each source → `SUPPORTS` with no `CONTRADICTS` inserts, else a
  second confirm (like the dedup / web gates). `computed` / `wikidata` / `openalex` / `arxiv` skip it
  (already authoritative). Off by default (network-heavy).

## Dependency posture
- `/wikidata`, `/openalex`, `/arxiv`: **no new crates** (reuse `reqwest` from R2-C); all keyless.
- `/calc`, `/world`: **no new crates, no network** (reuse Bund + WORLD-4/5/6).
- Zotero/vault: **no new crate** if importing a BibTeX/CSL-JSON export + Markdown folders (reuse
  `parse_bibtex` + the R2-B import path). Reading `zotero.sqlite` directly *would* want a SQLite reader —
  deferred; the export path avoids it.

## Recommended order
1. **R3-A Wikidata** — highest verifiability, keyless, reuses R2-C; the obvious first.
2. **R3-C deterministic (`/calc` + `/world`)** — zero-dep, un-fabricatable, the ethos fit.
3. **R3-B scholarly + SOURCES-1 auto-cite** — turns research into properly cited facts.
4. **R3-D author's library** — the trust anchor.
5. **R3-E triangulation** — once multiple sources exist.

## Relationship to RESRCH-2 (still open)
RESRCH-2's remaining items continue independently of this track:
- **R2-E — Trust & hygiene** (real per-model cost table, streamed extraction/factcheck/web display,
  chunked `/factcheck`, tab-completion). *Recommended next within RESRCH-2.*
- **R2-F — Batch / headless research**.
- **R2-C is built, pending the 1.5.2 cut.**

RESRCH-3 should begin only after RESRCH-2's hygiene pass (R2-E) lands — the new sources benefit from the
streamed display + cost model rather than inheriting that debt.
