# RESRCH-5 — Synthesis & Maintenance (track proposal)

| | |
|---|---|
| **Status** | **R5-A + R5-D shipped 1.5.8-dev**; R5-B/C/E/F open |
| **Builds on** | RESRCH-1..4 + UX + Undisputed (all shipped) — the corpus, provenance, the trust ladder, SOURCES-1, `/triangulate` |
| **Theme** | The program so far is about **acquisition** — getting facts *in*, cited, cross-checked. Two things it still can't do: **turn the corpus into output**, and **keep it healthy over time**. RESRCH-5 adds both, reusing the retrieval + provenance + SOURCES-1 machinery. Nothing here needs a new crate. |

## Grounding (verified)

- **RAG retrieval over the Facts book already exists** — `book_rag::retrieval::retrieve(store, hierarchy,
  &cfg.book_rag, book_id, query) -> Vec<RetrievedPassage>` where `RetrievedPassage { id, breadcrumb, body,
  score, is_hit }` (`src/book_rag/mod.rs:21`). The research app already drives it (`/diff`,
  `find_near_duplicate`, `rag::build_context`). Synthesis, outline, and gaps are all "retrieve → prompt →
  stream" over this.
- **Provenance is on every fact** — `.inkhaven/fact-sources.json`, `SourceRecord { origin, detail, query,
  thread, created_at }` (RFC3339). Drives *cite the tier* (synthesis), *find `model`-origin facts*
  (upgrade), and *age* (staleness).
- **SOURCES-1 can already emit BibTeX** — `BibEntry::to_bibtex()` (`src/sources/mod.rs:161`) and
  `compile_bibtex(&[BibEntry]) -> (String, usize)` (`:196`); the auto-cited entries live in the Sources
  book's **Research** chapter (RESRCH-3). `/bibliography` is a walk + `compile_bibtex`.
- **`/triangulate` + the structured sources** (Wikidata / OpenAlex / arXiv) are the engine for `/upgrade`.
- **Streaming + language** — `spawn_chat_stream` (TUI) / `collect_blocking` (CLI-batch) + the
  `resolve_prose_language` plumbing every RESRCH pass already uses.
- **Already shipped (not in this track):** the "make trust visible" maintenance idea landed as **UX-P2**
  (per-fact tier glyphs in the tree). RESRCH-5's maintenance half is therefore the *active* upkeep items.

## Part A — Synthesis: corpus → output (the biggest gap)

Everything grounds *in*; nothing composes *out*. This is where the research pays off.

| Phase | Content |
|---|---|
| **R5-A — `/synthesize <topic>`** | Retrieve the facts related to a topic (`retrieve` over the Facts book), then stream a **grounded synthesis** that uses *only* those facts, **cites each by its breadcrumb + provenance tier**, and flags where the corpus is thin — a mini overview built from your own verified corpus, in the project language. Read-only (a chat turn; `/fact` or `y` to keep it). **✅ Shipped 1.5.8-dev** — `run_synthesize` retrieves up to 24 passages with their provenance tier, streams a cited synthesis. |
| **R5-B — `/outline <topic>`** | The same retrieval, but the output is a **structured chapter/section outline**, each point citing the facts that support it — the **research → writing bridge**. Copy it into the manuscript, or `/note` it. |
| **R5-C — `/gaps <topic>`** | Retrieve, then ask the model what's **missing** ("you have the aqueduct's capacity but not its date"). Output a question list; optionally write it to a file to seed **`--batch`** (R2-F), closing the loop research → gaps → batch → facts. |
| **R5-D — `/bibliography [→ out]`** | Walk the Sources book's **Research** chapter (the `BibEntry`s auto-filed by R3-B and `/import`), `compile_bibtex`, and emit a formatted **`.bib` / references section** — the citations you accrued become a real manuscript bibliography. CLI `inkhaven research --bibliography [--out FILE]`. **✅ Shipped 1.5.8-dev** — `collect_research_bibentries` + `compile_bibtex`; `/bibliography` (chat) and `--bibliography [--out]` (CLI). |

## Part B — Maintenance: keep the corpus healthy

A knowledge base decays; nothing tends it yet.

| Phase | Content |
|---|---|
| **R5-E — `/upgrade`** | Find **`model`-origin** facts (the speculative tier) and, for each, try to **re-ground it on a structured source** via the `/triangulate` engine (Wikidata / scholarly). When corroborated, **raise its provenance tier** (record new provenance + the citation) — turning guesses into cited facts over time. **Non-destructive to the fact text**; only the provenance is upgraded (and the tree tier glyph follows). |
| **R5-F — Hygiene** | *Staleness report* — flag `web` / `model` facts older than `N` days (from `created_at`) for re-verification. *Dead-source detection* — `web`-origin facts whose URL now 404s (a `reqwest` HEAD; degrades offline). *Insert-time contradiction guard* — extend the dedup guard so a `/fact` that **contradicts** an existing fact warns before it commits (not just similarity). |

## Dependency posture
- **No new runtime crates** — Part A reuses `retrieve` + `compile_bibtex` + the streaming path; Part B
  reuses provenance + `/triangulate` + the existing `reqwest` (dead-source HEAD) + the dedup retriever.
- **Read-only / advisory** everywhere except the deliberate acts the user already takes (`/fact`,
  `/upgrade`'s provenance record). No prose is ever rewritten (the standing rule).
- **Language-respecting** — every LLM pass builds its prompt in the project language.

## Recommended first cut
**R5-A (`/synthesize`) + R5-D (`/bibliography`).** The two highest-value, lowest-risk items — both pure
reuse of retrieval + SOURCES-1 — and together they deliver the headline: **the corpus finally produces
output** (a cited synthesis and a real bibliography). R5-B/C (outline, gaps) extend synthesis; R5-E/F
(upgrade, hygiene) are the maintenance follow-on.

## Relationship to RESRCH-6
RESRCH-6 (deep / agentic research — `/deep`, citation snowballing, full-text) is the *acquisition*
frontier; RESRCH-5 is the *synthesis + upkeep* frontier. RESRCH-5 should land first — you want to compose
and maintain what you have before scaling how much you pull in.
