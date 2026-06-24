# BOOK_RAG-1 — Chat with Your Book (implementation plan)

Records RFC BOOK_RAG-1 (Vladimir Ulogov, 2026-06-30) and grounds it against the
actual codebase. **Repurpose Book scope** in the AI pane from "entire book as
context" to retrieval-augmented generation: retrieve the semantically relevant
paragraphs, compose a focused context, answer grounded with markdown citations.

## Reality check — the RFC's infra assumptions vs. the code

The RFC is sound in intent; three load-bearing claims need correcting, and one
correction makes the feature *much* smaller than the RFC's 6.5-week estimate.

1. **No Tantivy — the stack is `.typ` files + DuckDB + Fastembed + vecstore.**
   The RFC names "DuckDB + Tantivy + fastembed + HNSW." There is **no Tantivy**
   (Cargo.toml lists it among "crates inkhaven never reached"), and no separate
   full-text engine at all. The real stack, confirmed by the author:
   - prose lives in **`.typ` files** on disk;
   - **DuckDB** holds metadata + blobs;
   - **Fastembed** produces embeddings;
   - **vecstore** (`src/storage/vector.rs`) is the HNSW vector index.

   Retrieval is therefore **pure-semantic**: Fastembed embed → vecstore HNSW
   search. This matches the RFC's own MVP decision (semantic only) — just drop
   the full-text-backup framing. **Hybrid retrieval is impossible** here (no
   full-text engine to combine with), so it's a hard non-goal, not a "deferred"
   one as the RFC §14 frames it.

2. **The retrieval primitive already exists.** `DocumentStorage::search_document_text(query, limit)`
   (src/storage/document.rs:161) already embeds a query string, runs the HNSW
   search, dedups the meta/content slots, and returns ranked documents. P0's
   "vector search integration" is *consuming this*, not building it.

3. **A RAG-grounded-chat pattern ALREADY SHIPPED** — for the Facts book. The
   **`Ctrl+B Shift+S` Facts semantic-search modal** (`open_facts_search` /
   `Modal::FactsSearch` / `facts_search_send`, app.rs:9027+) does precisely what
   BOOK_RAG-1 proposes: type a query → semantic search over a book → ground a
   chat in the retrieved handful "instead of loading the whole book." BOOK_RAG-1
   is the **generalization of this proven pattern from the Facts book to the
   manuscript Book scope.** The grounded-chat seeding, the modal flow, the
   multi-select, and the system-prompt-with-retrieved-context envelope are all
   templates to follow, not net-new design.

Other reuse points that already exist: the **AI scope** lives in `App.ai_mode`
(includes `Book` and `Facts`); chat streams through `spawn_chat_stream(client,
model, system, history, prompt, category)`; **cost sub-budgets** follow
`WorldStore`/`InnerSocratesStore::DAILY_CALL_CAP` + the `slow_preflight` pattern,
and `cost.*` config + the `inkhaven cost` dashboard + `ai::usage` per-category
tracking all landed in 1.3.34/1.3.37 (a `book_rag` category slots straight in).

**Net effect:** the RFC's substrate-building phases (retrieval engine,
grounded-chat machinery, cost tracking) are largely *already built*. The real new
work is generalizing Facts-search → Book scope, the citation contract +
validation, the transparency section, the embedding-refresh nudge, and config.
Realistic estimate: **~2.5–3.5 weeks**, not 6.5.

## Target version

The RFC says "Target version 1.4.0" — but **1.4.0 is already cut** (we're on
`1.4.1-dev`). Retarget to **1.4.x** (earliest 1.4.1). Not a 1.4.0 blocker.

## Re-scoped phases (reuse-first)

- **P1 — Book-scope retrieval + grounded context.** Generalize the Facts-search
  seeding: a `book_rag` retrieval over the Book pool (manuscript + included
  system books) via `search_document_text`, ±N context expansion, token-budget
  cap. Compose the system-prompt envelope. New `src/book_rag/` mirroring the
  Facts flow.
- **P2 — Citations + validation.** Default prompt template (Prompts book
  `book_rag/`, the existing 3-tier chain) requiring markdown `[id](#id)`
  citations; post-hoc validator flags any cited id NOT in the retrieval set
  inline. Reuse the AI pane markdown renderer + PANE-1 link navigation.
- **P3 — Retrieval transparency.** The collapsible "Retrieved passages" section
  above each response; reuse-indicator on multi-turn.
- **P4 — Conversation state + multi-turn.** Retrieve once per chat session;
  clear-history → re-retrieve; in-memory (no new tables), per the existing AI
  chat-history mechanism.
- **P5 — Embedding-refresh nudge + cost sub-budget.** Status-line notice +
  conditional "clear chat?" suggestion on refresh; a `book_rag` category in the
  existing `ai::usage` + `cost:` config (permissive defaults).
- **P6 — Config, multilingual templates, CLI/Bund, docs.** `book_rag:` HJSON
  block; per-language default templates (EN/RU/ES/FR/DE); `inkhaven book-rag`
  CLI; `ink.book_rag.*` words; tutorial.

## Decisions to confirm before P1
- **Default ON vs OFF.** RFC defaults `enabled: true`, silently repurposing Book
  scope for everyone on upgrade. Given the permissive-individual-tool ethos, an
  explicit opt-in (or a one-time notice) may be safer. *Needs your call.*
- **The legacy "entire book as context" fallback** — verify what Book scope
  *currently* assembles (the RFC assumes whole-book; confirm in P1) so the
  `enabled:false` fallback is faithful.

## Cut criteria
Each phase signed with tests (retrieval/budget/citation-validation property
tests; multilingual; multi-turn; refresh-nudge). No new deps, no new tables.
Folds into a 1.4.x release.
