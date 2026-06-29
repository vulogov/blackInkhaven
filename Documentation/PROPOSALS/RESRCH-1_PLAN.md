# RESRCH-1 — Research Assistant (`inkhaven research`)

| | |
|---|---|
| **RFC** | RESRCH-1 |
| **Title** | A purpose-built TUI mode for AI-assisted research that transfers verified findings into the Facts / Notes corpus |
| **Status** | In progress — 1.5.0 |
| **Author** | Vladimir Ulogov |
| **New dependency** | none |
| **Scope decision** | Full feature in 1.5.0 (all 21 PRs); extracted research targets **Facts + Notes** (the existing Research system book is left untouched) |

A separate TUI application screen (`inkhaven research`) where the author conducts AI-assisted research
and transfers verified findings — with a mandatory confirmation step — directly into the **Facts**
(ground truth) or **Notes** (speculative) system books, immediately available to every other Inkhaven
feature. Left pane = Facts tree (navigate + manual entry); right pane = streaming RAG chat; a two-line
query prompt with a `/command` namespace; named, resumable threads.

## Audit corrections (the RFC was written against a partly-fabricated surface)

1. **`facts.duckdb` does not exist.** The NARR-1 plan states outright it "never existed." Facts are
   paragraphs in the **Facts system book**, indexed in the shared HNSW document store. The RFC's
   front-matter, §8, and §24 integration table all reference a phantom `facts.duckdb`. → Read the Facts
   *book* via the hierarchy + the shared HNSW (`store.search_document_text` /
   `book_rag::retrieval::retrieve`). **No new DuckDB file; correct, but for the right reason.**
2. **Async architecture (§7) is fabricated.** The TUI is a **synchronous crossterm `poll()`/`read()`
   loop**, not `tokio::select!`. Streaming = `ai::stream::spawn_chat_stream(...) ->
   tokio::sync::mpsc::UnboundedReceiver<StreamMsg>` drained with `try_recv()` each poll tick (see
   `src/tui/inference.rs`, `src/ai/stream.rs`). There is no `AppEvent::recv()` main loop. → The research
   app reuses `spawn_chat_stream` + `StreamMsg` (`Token`/`Done`/`Error`) + poll-drain, mirroring the AI
   pane. The `AppEvent` enum in §6 is replaced by: crossterm key events handled inline + a
   `StreamMsg` receiver polled per tick. A tokio runtime Handle must be entered (as the writing app
   does) so `spawn_chat_stream`'s `tokio::spawn` doesn't panic.
3. **`store::create_paragraph` does not exist.** Real: `Store::create_node(...)` then
   `Store::update_paragraph_content(node, bytes)` — the latter **auto-reembeds** via `reembed_document`.
   So the RFC's "fastembed::embed(body) → HNSW::insert(node_id, embedding)" is a single
   `update_paragraph_content` call, not a manual embed+insert.
4. **`store::node_by_slug_path` does not exist.** Real reverse resolver: `Hierarchy::find_by_path(path)`
   (`src/store/hierarchy.rs:261`); forward is `Hierarchy::slug_path(node)` (line 333). `/goto` uses
   `find_by_path`.
5. **RAG assembly (§8/§16) reinvents existing infrastructure.** `book_rag::retrieval::retrieve(store,
   hierarchy, &cfg.book_rag, book_id, query)` already does book-scoped HNSW retrieval, and
   `book_rag::compose_context_prefix(passages)` formats it. → Research RAG = `retrieve` scoped to the
   **Facts book id**, not a hand-rolled 30/50/20 token-budget assembler. Pinned-node text is prepended
   to that prefix (the one genuinely new piece). `/diff` reuses the same `retrieve` (or
   `store.search_document_text`) on the last response.
6. **Test baseline wrong.** RFC says 2,186 post-MYTH-1; actual is **2,146**. Target: **2,146 → ~2,218**.

### Confirmed real (RFC got these right)
- Deps `genai`, `tokio`, `tokio-stream`, `fastembed`, `tui-textarea` — present.
- **Notes** + **Facts** system books exist (`SYSTEM_TAG_NOTES`/`SYSTEM_TAG_FACTS`); a dedicated
  **Research** book also exists (`("research","Research")`) — intentionally left untouched per the scope
  decision.
- HNSW document store + fastembed (`storage/embedding.rs` `embed`/`embed_batch`; `storage/document.rs`
  `search_document_text`/`vector_count`); `AiMode::Facts` scope; **BOOK_RAG-1** (`book_rag::retrieval`,
  `ink.book_rag.*`).
- `tui-textarea` is the editor widget; `.inkhaven/` sidecar pattern is established (threads JSON fits).

### Design notes
- **No new system books, no new DuckDB files, no new runtime crates.** One sidecar dir:
  `.inkhaven/research-threads/<slug>.json`. One `research:` config block.
- The separate-app boundary (§4) is real and clean: a second `App`-like struct with its own synchronous
  event loop, sharing only on-disk state. It must enter a tokio runtime Handle before streaming.
- Insertion auto-reembeds (correction #3), so inserted facts are immediately retrievable by `/diff`,
  the writing-mode RAG, and every Facts consumer — no extra indexing code (§24 holds, via the shared
  HNSW not a phantom `facts.duckdb`).

## Phases (PR order, from RFC §26, corrected)

| Phase | Content |
|---|---|
| R-P1 | `inkhaven research` subcommand + terminal init/teardown + min-width(80) guard + outer layout skeleton (placeholder panes) + `q`/`Ctrl+C` exit; enter tokio runtime Handle |
| R-P2 | Thread storage (G3): `ResearchThread` + JSON serde + `.inkhaven/research-threads/` create/load/save + default thread |
| R-P3 | Thread picker (Enter open / n new / d delete-confirm / Esc exit) + `--thread` bypass |
| R-P4 | Facts tree pane: TreeState rooted at Facts book, vim nav, fold/expand, status badge, `⬡` pin marker, `Ctrl+P` pin/unpin (state only, G4), `n` manual entry (G7) |
| R-P5 | Query prompt: 2-line tui-textarea, Tab focus cycle, Enter submit (no LLM yet), ↑/↓ history |
| R-P6 | AI chat pane: `Vec<ChatTurn>`, streaming render, j/k/g/G scroll, empty state |
| R-P7 | LLM integration: reuse `spawn_chat_stream`+`StreamMsg`+`try_recv` drain, research system prompt, session cost |
| R-P8 | RAG context: `book_rag::retrieval::retrieve` scoped to Facts book + pinned-node prepend (G4 complete) + F10 RAG mode toggle |
| R-P9 | Command dispatcher: `/command` parser, route table, unknown → status-bar error |
| R-P10 | `/fact`: extraction LLM call → JSON parse → `ConfirmationState` overlay (G1/G2 editable title+body) → `create_node`+`update_paragraph_content` → thread turn |
| R-P11 | `/note` (G12): same as `/fact`, target Notes book, note-extraction prompt variant |
| R-P12 | `/goto`: slug-path parse → `Hierarchy::find_by_path` → tree nav + ancestor expand |
| R-P13 | `/diff` (G9): embed last response → HNSW retrieve top-N → display (dedup pinned) |
| R-P14 | `/verify` (G6): claim-extraction heuristic → confidence probe → styled annotations (HIGH dim / MEDIUM normal / LOW bold ⚠) |
| R-P15 | `/chain` (G5): `→`-separated parse → `ChainState` sequential execution + step header + status indicator |
| R-P16 | Chat search (G8): `Ctrl+F` bar + match highlight + n/N nav |
| R-P17 | Hints bar (G11): context-sensitive per focus + `?` toggle + height collapse |
| R-P18 | Status bar: RAG mode / session cost / pinned / chain / transient messages |
| R-P19 | CLI `--list-threads` + `--export-thread` (table/json/md) |
| R-P20 | `research:` config block (defaults + validation) — threaded through P1–P19 |
| R-P21 | Integration tests: full `/fact` + `/note` round-trips, `/goto`, thread persistence, HNSW integration, chain, overlay state machine, RAG ordering |

## Out of scope (deferred per RFC §3, §27)
- Web search / document import (RESRCH-2).
- Multi-fact extraction per `/fact` call; automatic insertion without confirmation.
- Tab-completion on `/goto` and `/fact → path` (1.5.1).
- The existing **Research** system book as a target (scope decision: Facts + Notes only).
- NARR-2 / SERIES-1 (separate RFCs).
